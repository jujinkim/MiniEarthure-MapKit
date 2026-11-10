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
fn native_building_walls_carry_record_palettes_and_a_seeded_window_role() {
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
        // Walls take the record's body colour, roofs its roof colour (shared palette).
        let [a, b, c] = triangle.vertices;
        let (u, v) = ([b[0]-a[0], b[1]-a[1], b[2]-a[2]], [c[0]-a[0], c[1]-a[1], c[2]-a[2]]);
        let n = [(u[1]*v[2]-u[2]*v[1]) as f64, (u[2]*v[0]-u[0]*v[2]) as f64, (u[0]*v[1]-u[1]*v[0]) as f64];
        let roof = (n[1] / (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt()).abs() >= 0.5;
        // One box per record: its solid seed hashes the ID and the 900 cm top.
        let look = distant::facade_look(distant::facade_record(&building.id), distant::facade_solid(&building.id, 900),
            distant::facade_own_share(&format!("{}:{}", building.material, building.usage)));
        let expected = distant::facade_color(&building.material, &building.usage, look, roof);
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

#[test]
fn record_palettes_follow_the_shared_table_and_seed_bits() {
    let table: serde_json::Value = serde_json::from_str(include_str!("../../../godot/facade_palette.json")).unwrap();
    let entry = |path: &[&str], index: u32| -> [u8; 4] {
        let mut value = &table;
        for key in path { value = &value[*key]; }
        let rgb = &value[index as usize];
        [rgb[0].as_u64().unwrap() as u8, rgb[1].as_u64().unwrap() as u8, rgb[2].as_u64().unwrap() as u8, 255]
    };
    for record in [0u32, 7, 8, 63, 0x5a5, 4095] {
        assert_eq!(distant::facade_color("concrete", "residential", record, false), entry(&["body", "concrete:residential"], record & 7));
        assert_eq!(distant::facade_color("concrete", "commercial", record, false), entry(&["body", "concrete:commercial"], record & 7));
        assert_eq!(distant::facade_color("concrete", "school", record, false), entry(&["body", "concrete:residential"], record & 7), "other uses read as residential");
        assert_eq!(distant::facade_color("brick", "public", record, false), entry(&["body", "brick"], record & 7), "brick keeps its own bodies");
        assert_eq!(distant::facade_color("concrete", "industrial", record, true), entry(&["roof", "concrete"], (record >> 3) & 7));
        assert_eq!(distant::facade_color("wood", "residential", record, true), entry(&["roof", "wood"], (record >> 3) & 7));
    }
    let record = distant::facade_record("import-x-building-1");
    assert!(record < 4096, "a 12-bit record seed");
    assert_eq!(record, distant::facade_record("import-x-building-1"), "stable per record ID");
    // Own looks: the key's share of solids (apartment complexes fewer), by the
    // shader's PCG of the solid bits; known answers pin the hash and salt.
    assert_eq!(distant::facade_own_share("concrete:residential"), 0.15);
    assert_eq!(distant::facade_own_share("concrete:commercial"), 0.35);
    assert_eq!(distant::facade_own_share("brick:residential"), 0.35);
    assert_eq!(table["own_share"]["default"].as_f64().unwrap() as f32, 0.35);
    // The facade_validator known answers of the shader hash.
    assert_eq!([distant::facade_pcg(1759714724), distant::facade_pcg(1 ^ 1759714724), distant::facade_pcg(4095 ^ 1759714724)], [3561045559, 1454332419, 462394992]);
    // The same known answers as facade_validator.gd `looks`.
    assert_eq!([distant::facade_look(77, 1, 0.35), distant::facade_look(77, 2048, 0.35), distant::facade_look(77, 2048, 0.15),
        distant::facade_look(77, 0, 0.35), distant::facade_look(77, 4095, 0.35), distant::facade_look(77, 9, 0.15)], [1, 2048, 77, 77, 77, 9]);
    for share in [0.15f32, 0.35] {
        let own = (0..4096u32).filter(|&solid| distant::facade_look(4096 + 7, solid, share) == solid).count();
        assert!((own as f32 / 4096.0 - share).abs() < 0.02, "own looks follow the share {share}: {own}");
        for solid in 0..4096u32 {
            let look = distant::facade_look(77, solid, share);
            assert!(look == solid || look == 77, "a look is the solid's or the record's");
            if share == 0.15 && look == solid { assert_eq!(distant::facade_look(77, solid, 0.35), solid, "a lower share keeps a subset"); }
        }
    }
    assert_eq!(distant::facade_solid("b", 900), distant::facade_solid("b", 900));
    assert_ne!(distant::facade_solid("b", 900), distant::facade_solid("b", 1200), "solids of other tops vary");
    for list in ["accent"] { assert_eq!(table[list].as_array().unwrap().len(), 8); }
    for (_, rows) in table["body"].as_object().unwrap().iter().chain(table["roof"].as_object().unwrap().iter()) {
        assert_eq!(rows.as_array().unwrap().len(), 8, "eight entries per palette");
    }
}

#[test]
fn city_trees_become_bounded_silhouettes() {
    use mapkit_core::{MapDocument, Zone, ZoneKind, ZoneTree};
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.zones.clear(); d.buildings.clear();
    d.bounds.max = [25600, 25600]; d.cell_size_cm = 12800;
    // A street-size zone and a park zone with the widest city-tree footprint.
    for (index, kind) in [ZoneKind::Forest, ZoneKind::Orchard].into_iter().enumerate() {
        let x = 1000 + index as i64 * 12000;
        d.zones.push(Zone { id: format!("park-{index}"), polygon: vec![[x, 1000], [x + 11000, 1000], [x + 11000, 23000], [x, 23000]],
            kind, spacing_cm: 700, density_per_mille: 750, exclusions: vec![],
            tree: Some(ZoneTree { asset_id: "builtin:city_tree".into(), radius_cm: [120, 200][index], clearance_cm: 30 }) });
    }
    let p = read_bytes(&pack_bytes(d, Default::default()).unwrap()).unwrap();
    let (mut trees, mut wide) = (0, 0);
    for cell in p.document.cells() {
        let chunk = p.generate(cell, 500_000).unwrap();
        let count = chunk.objects.iter().filter(|o| o.asset_id == "builtin:city_tree").count();
        trees += count;
        let far = p.generate_distant(cell).unwrap();
        let bound = p.distant_triangle_bound(cell).unwrap();
        assert!(far.vertices.len() as u64 <= bound * 3, "{cell:?}: {} > {bound}", far.vertices.len() / 3);
        // The 12-triangle collision trunk is omitted; each tree adds one
        // 24-triangle silhouette, which the trunk and the per-object allowance
        // of the bound cover.
        for tree in chunk.objects.iter().filter(|o| o.asset_id == "builtin:city_tree") {
            assert_eq!(chunk.triangles.iter().filter(|t| t.object_id == tree.id).count(), 12, "{}", tree.id);
        }
        let solids = chunk.triangles.iter().filter(|t| !chunk.objects.iter().any(|o| o.id == t.object_id)).count();
        assert_eq!(far.vertices.len() / 3, solids + count * 24, "{cell:?}");
        // Each silhouette stays inside its footprint; park-zone crowns are wide.
        for tree in chunk.objects.iter().filter(|o| o.asset_id == "builtin:city_tree") {
            let at = [tree.position[0] as f32 * 0.01, -tree.position[2] as f32 * 0.01];
            let reach = far.vertices.iter().map(|p| (p[0] - at[0]).hypot(p[2] - at[1])).filter(|&r| r < 3.).fold(0f32, f32::max);
            let footprint = if tree.id.starts_with("park-1:") { wide += 1; 2.0 } else { 1.2 };
            assert!(reach <= footprint + 1e-3 && reach >= footprint * 0.6, "{}: {reach} m of {footprint} m", tree.id);
        }
    }
    assert!(trees > 100 && wide > 30, "fixture zones plant trees: {trees}, {wide} wide");
}

#[test]
fn city_streetlights_keep_their_pole_faces_in_distant_cells() {
    // emit_placement's omit_proxy and distant_from_parts' omitted set agree:
    // neither drops city streetlight poles (distant_tree is false for them),
    // in any cell.
    use mapkit_core::{MapDocument, Placement};
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.zones.clear(); d.buildings.clear(); d.placements.clear();
    d.bounds.max = [25600, 25600]; d.cell_size_cm = 12800;
    for (index, (id, _, _)) in mapkit_core::CITY_STREETLIGHTS.iter().enumerate() {
        let x = 2000 + index as i64 * 1500;
        d.placements.push(Placement { id: format!("lamp-{index}"), asset_id: (*id).into(), position: [x, 0, 6400], quarter_turns: (index % 4) as u8, yaw_offset_mdeg: [0, 30000][index % 2] });
    }
    // A yawed pole across the cell border x = 12800 (a convex proxy).
    d.placements.push(Placement { id: "lamp-z".into(), asset_id: "builtin:city_streetlight_255".into(), position: [12802, 0, 9000], quarter_turns: 0, yaw_offset_mdeg: 20000 });
    let p = read_bytes(&pack_bytes(d, Default::default()).unwrap()).unwrap();
    let (mut owners, mut border) = (0, Vec::new());
    for cell in p.document.cells() {
        assert!(p.document.placements.iter().all(|l| !mapkit_core::distant_tree(&l.asset_id)));
        let chunk = p.generate(cell, 500_000).unwrap();
        let far = p.generate_distant(cell).unwrap();
        assert_eq!(far.vertices.len() / 3, chunk.triangles.len(), "{cell:?}: every pole face stays in the distant mesh");
        assert!(far.vertices.len() as u64 <= p.distant_triangle_bound(cell).unwrap() * 3);
        owners += chunk.objects.iter().filter(|o| mapkit_core::city_streetlight(&o.asset_id).is_some()).count();
        for lamp in &p.document.placements {
            let faces = chunk.triangles.iter().filter(|t| t.object_id == lamp.id).count();
            if p.document.cell_at([lamp.position[0], lamp.position[2]]) == Some(cell) { assert!(faces >= 8, "{}: {faces}", lamp.id); }
        }
        if chunk.triangles.iter().any(|t| t.object_id == "lamp-z") { border.push(cell); }
    }
    assert_eq!(owners, mapkit_core::CITY_STREETLIGHTS.len() + 1, "every id once plus the border pole");
    // The neighbour keeps its clipped part of the yawed pole too.
    assert_eq!(border, vec![mapkit_core::Cell { x: 0, y: 0 }, mapkit_core::Cell { x: 1, y: 0 }]);
}

/// `MAPKIT_DISTANT_BOUND_MEMAP=<package> cargo test -p mapkit-package --test distant -- --ignored`
/// checks every cell of a real package: the distant mesh stays within its
/// admission bound (city-tree silhouettes included).
#[test]
#[ignore = "needs a local package path"]
fn real_package_distant_meshes_stay_within_their_bounds() {
    let path = std::env::var("MAPKIT_DISTANT_BOUND_MEMAP").expect("MAPKIT_DISTANT_BOUND_MEMAP");
    let p = read_bytes(&std::fs::read(path).unwrap()).unwrap();
    let (mut actual, mut bound, mut trees, mut tightest) = (0u64, 0u64, 0usize, f64::MAX);
    for cell in p.document.cells() {
        let far = p.generate_distant(cell).unwrap();
        let limit = p.distant_triangle_bound(cell).unwrap();
        let triangles = far.vertices.len() as u64 / 3;
        assert!(triangles <= limit, "{cell:?}: {triangles} > {limit}");
        actual += triangles;
        bound += limit;
        if limit > 0 { tightest = tightest.min((limit - triangles) as f64 / limit as f64); }
        trees += p.generate(cell, 500_000).map_or(0, |c| c.objects.iter().filter(|o| o.asset_id == "builtin:city_tree").count());
    }
    println!("cells {} far triangles {actual} bound {bound} city trees {trees} smallest headroom {:.1}%", p.document.cells().len(), tightest * 100.);
}
