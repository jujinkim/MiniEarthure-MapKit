use mapkit_core::{Cell, GeneratedChunk, SpawnRequest, Surface, Triangle};
fn chunk() -> GeneratedChunk {
    GeneratedChunk { building_prisms: vec![], format_version: 6, cell: Cell { x: 0, y: 0 }, objects: vec![], triangles: vec![
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
