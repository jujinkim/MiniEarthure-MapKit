use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/assets/document.json")).unwrap()
}
fn generated(d: &MapDocument, cell: Cell) -> GeneratedOccupancy {
    generate_with_occupancy(
        GenerationInput {
            document: d,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        },
        20000,
    )
    .unwrap()
}
#[test]
fn convex_admission_rejects_open_inverted_nonconvex_degenerate_and_oversized_inputs() {
    let d = document();
    d.validate().unwrap();
    let source = d.assets[0].convex_collision[0].clone();
    assert!(source.valid(100_000));
    let mut bad = source.clone();
    bad.faces.pop();
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.faces[0].swap(0, 1);
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.vertices[3] = bad.vertices[0];
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.faces[0][0] = 255;
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.vertices[3][1] = 100_001;
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.faces.push(bad.faces[0]);
    assert!(!bad.valid(100_000));
    let mut bad = source.clone();
    bad.vertices.push([0, 20, 0]);
    assert!(!bad.valid(100_000));
    for version in 1..=3 {
        let mut d = d.clone();
        d.recipe_version = version;
        assert_eq!(d.validate().unwrap_err().code, "E_VERSION");
    }
    for mutate in [0, 1, 2] {
        let mut d = d.clone();
        let m = d.assets[1].material.as_mut().unwrap();
        match mutate {
            0 => m.metallic_per_mille = 1001,
            1 => m.roughness_per_mille = 1001,
            _ => m.albedo_texture = Some("tetra".into()),
        };
        assert_eq!(d.validate().unwrap_err().code, "E_ASSET");
    }
}
#[test]
fn exact_proxy_transform_seams_budget_archives_and_order_are_preserved() {
    let mut d = document();
    d.normalize();
    let mut all_objects = vec![];
    for cell in [
        Cell { x: 0, y: 0 },
        Cell { x: 1, y: 0 },
        Cell { x: 0, y: 1 },
        Cell { x: 1, y: 1 },
    ] {
        let result = generated(&d, cell);
        let c = &result.chunk;
        let cost = estimate_generation(&d, cell, 500_000).unwrap();
        assert!(cost.asset_convexes >= c.asset_convexes.len() as u64);
        assert!(cost.triangles >= c.triangles.len() as u64);
        assert!(cost.occupied_solids >= result.solids.len() as u64);
        assert_eq!(c.hash().unwrap(), sha256(&canonical(c).unwrap()));
        let key = archive_key(&"a".repeat(64), cell);
        let bytes = encode_archive(c, &key, archive_limit(&cost)).unwrap();
        assert_eq!(decode_archive(&bytes, &key, cell, &cost).unwrap(), *c);
        for n in [0, 8, 156, bytes.len() - 1] {
            assert!(decode_archive(&bytes[..n], &key, cell, &cost).is_err());
        }
        all_objects.extend(c.objects.iter().map(|o| o.id.clone()));
        if cell.y == 0 {
            let solid = c
                .asset_convexes
                .iter()
                .find(|c| c.object_id == "tetra-a")
                .unwrap();
            assert_eq!(
                solid.shape,
                d.assets
                    .iter()
                    .find(|a| a.id == "tetra")
                    .unwrap()
                    .convex_collision[0]
                    .placed(d.placements.iter().find(|p| p.id == "tetra-a").unwrap())
            );
            assert!(result.solids.iter().any(|s| s.object_id == "tetra-a"
                && matches!(&s.shape,SolidShape::Convex(shape) if shape==&solid.shape)));
        }
        let mut reordered = d.clone();
        reordered.assets.reverse();
        reordered.placements.reverse();
        reordered.normalize();
        assert_eq!(
            c.hash().unwrap(),
            generated(&reordered, cell).chunk.hash().unwrap()
        );
    }
    all_objects.sort();
    all_objects.dedup();
    assert_eq!(all_objects.len(), d.placements.len());
    let cell = Cell { x: 0, y: 0 };
    assert!(generate_with_occupancy(
        GenerationInput {
            document: &d,
            cell,
            heightgrid: None,
            max_triangles: 500_000
        },
        0
    )
    .is_err());
}
#[test]
fn recipe_three_frozen_geometry_survives_display_extension() {
    let d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    let c = generated(&d, Cell { x: 0, y: 0 }).chunk;
    assert!(c.asset_convexes.is_empty());
    // Existing canonical source still omits all new defaults.
    let raw = String::from_utf8(canonical(&d).unwrap()).unwrap();
    assert!(!raw.contains("convex_collision"));
    assert!(!String::from_utf8(canonical(&c).unwrap())
        .unwrap()
        .contains("asset_convexes"));
}
