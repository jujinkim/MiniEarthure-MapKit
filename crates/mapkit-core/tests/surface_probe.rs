use mapkit_core::{Cell, GeneratedChunk, SpawnRequest, Surface, Triangle};
fn chunk() -> GeneratedChunk {
    GeneratedChunk { asset_convexes: vec![], building_prisms: vec![], format_version: 6, cell: Cell { x: 0, y: 0 }, objects: vec![], triangles: vec![
        Triangle { vertices: [[0,0,0],[1000,0,0],[0,0,1000]], surface: Surface::Asphalt, object_id: "ground".into(), spawnable: true },
        Triangle { vertices: [[0,700,0],[1000,1200,0],[0,700,1000]], surface: Surface::Concrete, object_id: "bridge".into(), spawnable: true },
        Triangle { vertices: [[0,2000,0],[1000,2000,0],[0,2000,1000]], surface: Surface::Concrete, object_id: "roof".into(), spawnable: false },
    ] }
}
#[test]
fn probe_preserves_spawn_height_and_distinguishes_stacked_surface_normals() {
    let c = chunk();
    let request = SpawnRequest { position_cm: [200,200], surface_id: "bridge".into() };
    let sample = c.surface_probe(&request).unwrap();
    assert_eq!(sample.position_cm, c.spawn(&request).unwrap());
    assert_eq!(sample.position_cm, [200,800,200]);
    assert_eq!(sample.normal_q, [-447214,894427,0]);
    assert_eq!(c.surface_probe(&SpawnRequest { surface_id: "ground".into(), ..request }).unwrap().normal_q, [0,1_000_000,0]);
}
#[test]
fn winding_does_not_flip_probe_up_and_roofs_or_missing_surfaces_fail() {
    let mut c = chunk();
    for t in &mut c.triangles { t.vertices.swap(0,2); }
    let request = SpawnRequest { position_cm: [200,200], surface_id: "bridge".into() };
    assert_eq!(c.surface_probe(&request).unwrap().normal_q, [-447214,894427,0]);
    for id in ["roof", "missing"] {
        assert!(c.surface_probe(&SpawnRequest { surface_id: id.into(), ..request.clone() }).is_err());
    }
    assert!(c.surface_probe(&SpawnRequest { position_cm: [2000,2000], ..request }).is_err());
}

#[test]
fn thin_valid_triangle_keeps_exact_cross_product_before_normalizing() {
    let mut c = chunk();
    c.triangles[0].vertices = [[0,0,0],[1_000_000_000,0,999_999_999],[999_999_999,0,999_999_998]];
    let request = SpawnRequest { position_cm: [0,0], surface_id: "ground".into() };
    assert_eq!(c.surface_probe(&request).unwrap().normal_q, [0,1_000_000,0]);
}

#[test]
fn options_match_spawn_order_and_reject_overflow_atomically() {
    let mut c = chunk();
    let before = c.hash().unwrap();
    let options = c.spawn_options([200, 200]).unwrap();
    assert_eq!(options.iter().map(|v| v.surface_id.as_str()).collect::<Vec<_>>(), ["bridge", "ground"]);
    for option in options {
        assert_eq!(option.position_cm, c.spawn(&SpawnRequest { position_cm: [200,200], surface_id: option.surface_id }).unwrap());
    }
    assert_eq!(before, c.hash().unwrap());
    let template = c.triangles[0].clone();
    c.triangles = (0..64).map(|i| Triangle { object_id: format!("road-{i:03}"), ..template.clone() }).collect();
    // Duplicated triangles and remote identities do not consume candidate slots.
    c.triangles.extend(c.triangles.clone());
    c.triangles.extend((0..1000).map(|i| Triangle { object_id: format!("remote-{i}"), vertices: [[5000,0,5000],[6000,0,5000],[5000,0,6000]], ..template.clone() }));
    assert_eq!(c.spawn_options([200,200]).unwrap().len(), 64);
    c.triangles.push(Triangle { object_id: "overflow".into(), ..template.clone() });
    assert_eq!(c.spawn_options([200,200]).unwrap_err().code, "E_SURFACE_LIMIT");
    assert!(c.spawn_options([-1,-1]).unwrap().is_empty());
    c.triangles = vec![template];
    assert_eq!(c.spawn_options([200,200]).unwrap().len(), 1);
}

#[test]
fn options_bound_escaped_identity_and_keep_first_intersection() {
    let mut c = chunk();
    c.triangles.truncate(1);
    c.triangles[0].object_id = "\u{0001}".repeat(256);
    let options = c.spawn_options([200,200]).unwrap();
    assert!(serde_json::to_vec(&options).unwrap().len() < 1664);
    c.triangles[0].object_id.push('x');
    assert_eq!(c.spawn_options([200,200]).unwrap_err().code, "E_SURFACE_LIMIT");
    c.triangles[0].object_id = "same".into();
    let mut higher = c.triangles[0].clone();
    higher.vertices.iter_mut().for_each(|v| v[1] = 900);
    c.triangles.push(higher);
    assert_eq!(c.spawn_options([200,200]).unwrap()[0].position_cm[1], 0);
}
