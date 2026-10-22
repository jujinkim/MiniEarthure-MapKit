use mapkit_package::*;
use std::path::Path;

#[test]
fn distant_display_is_bounded_deterministic_and_preserves_generation() {
    for name in ["minimal", "roads", "courtyard", "assets"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name);
        let (document, files) = read_project(&path).unwrap();
        let package = read_bytes(&pack_bytes(document, files).unwrap()).unwrap();
        let margin = package.visual_margin_cm().unwrap();
        assert!(margin >= 184);
        if name == "assets" {
            assert_eq!(margin, 641, "5 m by 4 m horizontal GLB bounds include every quarter turn");
        }
        for cell in package.document.cells() {
            let original = package.generate(cell, 500_000).unwrap();
            let hash = original.hash().unwrap();
            let view = package.distant_from_generated(&original).unwrap();
            let generated = package.generate_distant(cell).unwrap();
            assert_eq!(view.vertices, generated.vertices);
            assert_eq!(view.colors, generated.colors);
            assert_eq!(view.light_data, generated.light_data);
            assert_eq!(view.decoration, generated.decoration);
            assert_eq!(view.vertices.len(), view.colors.len());
            assert!(
                view.vertices.len() as u64 <= package.distant_triangle_bound(cell).unwrap() * 3
            );
            assert!(view.vertices.iter().flatten().all(|v| v.is_finite()));
            assert_eq!(
                hash,
                package.generate(cell, 500_000).unwrap().hash().unwrap()
            );
            // Assets use independent visual proxies. Pure road/terrain surfaces
            // retain the exact reflected integer-centimetre triangle boundaries.
            if original.objects.is_empty() && package.document.assets.is_empty() && package.document.buildings.is_empty() {
                assert_eq!(view.vertices.len(), original.triangles.len() * 3);
            }
        }
    }
}

#[test]
fn native_building_reduction_preserves_roofs_courtyards_and_cell_cuts() {
    use mapkit_core::{Building, MapDocument};
    let square = vec![[1000,1000],[7000,1000],[7000,7000],[1000,7000]];
    let hole = vec![[3000,3000],[3000,5000],[5000,5000],[5000,3000]];
    let concave = vec![[1000,1000],[7000,1000],[7000,3000],[3000,3000],[3000,7000],[1000,7000]];
    for (footprint, holes, roof) in [
        (square.clone(), vec![], "flat"),
        (square.clone(), vec![], "gable"),
        (square, vec![hole], "flat"),
        (concave, vec![], "flat"),
    ] {
        for shift in [0, 8000] {
            let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
            d.nodes.clear(); d.roads.clear(); d.zones.clear(); d.buildings.clear();
            d.bounds.max = [25600,25600]; d.cell_size_cm = 12800;
            let moved = |ring: &Vec<[i64;2]>| ring.iter().map(|p| [p[0]+shift,p[1]+shift]).collect();
            let building = Building { id:"native".into(), footprint:moved(&footprint),
                holes:holes.iter().map(moved).collect(), base_cm:100, height_cm:1500,
                usage:"public".into(), material:"brick".into(), roof:roof.into(), entrances:vec![],parts:vec![] };
            d.buildings.push(building.clone());
            let p = read_bytes(&pack_bytes(d, Default::default()).unwrap()).unwrap();
            let mut removed = 0;
            for cell in p.document.cells() {
                let chunk = p.generate(cell,500_000).unwrap();
                let original_hash = chunk.hash().unwrap();
                let far = p.generate_distant(cell).unwrap();
                assert!(far.vertices.len() <= chunk.triangles.len()*3);
                removed += chunk.triangles.len()-far.vertices.len()/3;
                let bounds = p.document.cell_bounds(cell).unwrap();
                let boundary = |tri: &mapkit_core::Triangle| {
                    let points = tri.vertices.map(|v| [v[0],v[2]]);
                    // Every roof/bottom triangle, footprint/hole wall and cell-cut
                    // wall must survive. Only internal vertical faces may vanish.
                    let cross = |a:[i64;2],b:[i64;2],c:[i64;2]| (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
                    cross(points[0],points[1],points[2]) != 0
                        || [0,1].into_iter().any(|axis| [bounds.min[axis],bounds.max[axis]].into_iter()
                            .any(|limit| points.iter().all(|v| v[axis] == limit)))
                        || std::iter::once(&building.footprint).chain(&building.holes).any(|ring|
                            (0..ring.len()).any(|i| points.iter().all(|&v| cross(ring[i],ring[(i+1)%ring.len()],v) == 0)))
                };
                let mut actual = far.vertices.chunks_exact(3).peekable();
                for tri in &chunk.triangles {
                    let expected = [0,2,1].map(|i| {
                        let v = tri.vertices[i]; [v[0] as f32*0.01,v[1] as f32*0.01,-v[2] as f32*0.01]
                    });
                    if actual.peek().is_some_and(|v| **v == expected) { actual.next(); }
                    else { assert!(tri.object_id == "native" && !boundary(tri), "lost exterior: {roof} {shift} {cell:?} {tri:?}"); }
                }
                assert!(actual.next().is_none(), "output must be an exact ordered source subset");
                assert_eq!(original_hash, p.generate(cell,500_000).unwrap().hash().unwrap());
                assert!(p.distant_workspace_bytes(cell).unwrap() >= chunk.triangles.len() as u64*128);
            }
            assert!(removed > 0, "expected internal faces: {roof} {shift}");
            if roof == "flat" && holes.is_empty() && footprint.len() == 4 && shift == 0 {
                assert_eq!(removed,4,"two triangular prisms reduce to the exact twelve-triangle box");
            }
        }
    }
}

#[test]
fn native_building_walls_carry_use_colours_and_a_seeded_window_role() {
    use mapkit_core::{Building, MapDocument};
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.zones.clear(); d.buildings.clear();
    d.bounds.max = [25600,25600]; d.cell_size_cm = 12800;
    let kinds = [("concrete","residential"),("brick","residential"),("wood","industrial"),("concrete","commercial"),("concrete","public")];
    for (index, (material, usage)) in kinds.iter().enumerate() {
        let x = 1000 + index as i64 * 2000;
        d.buildings.push(Building { id: format!("b{index}"), footprint: vec![[x,1000],[x+1000,1000],[x+1000,2000],[x,2000]],
            holes: vec![], base_cm: 0, height_cm: 900, usage: usage.to_string(), material: material.to_string(),
            roof: "flat".into(), entrances: vec![], parts: vec![] });
    }
    let p = read_bytes(&pack_bytes(d, Default::default()).unwrap()).unwrap();
    let cell = p.document.cells()[0];
    let chunk = p.generate(cell, 500_000).unwrap();
    let far = p.generate_distant(cell).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    for (index, triangle) in chunk.triangles.iter().enumerate() {
        let Some(building) = p.document.buildings.iter().find(|b| b.id == triangle.object_id) else { continue };
        let vertex = [0,2,1].map(|i| { let v = triangle.vertices[i]; [v[0] as f32*0.01, v[1] as f32*0.01, -v[2] as f32*0.01] });
        let Some(position) = far.vertices.chunks_exact(3).position(|v| v == vertex) else { continue };
        let expected = distant::building_color(&building.material, &building.usage);
        assert_eq!(far.colors[position*3], expected, "{} {} triangle {index}", building.material, building.usage);
        let light = far.light_data[position*3];
        assert_eq!(light[1], 3., "native walls use the distant building role");
        assert!((0.0..=1.0).contains(&light[0]));
        seen.entry(building.id.clone()).or_insert(light[0]);
    }
    assert_eq!(seen.len(), kinds.len(), "every building survives in the far mesh");
    let seeds: std::collections::BTreeSet<_> = seen.values().map(|s| s.to_bits()).collect();
    assert_eq!(seeds.len(), kinds.len(), "object seeds differ per building");
    let colours: std::collections::BTreeSet<_> = kinds.iter().map(|(m,u)| distant::building_color(m,u)).collect();
    assert_eq!(colours.len(), kinds.len(), "material/use pairs are distinct colours");
    assert_eq!(distant::building_color("concrete","residential"), [212,202,184,255]);
    assert_eq!(distant::building_color("brick","commercial"), [174,130,115,255]);
}
