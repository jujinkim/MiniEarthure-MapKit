use mapkit_core::*;

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
fn input(d: &MapDocument, cell: Cell) -> GenerationInput<'_> {
    GenerationInput {
        document: d,
        cell,
        heightgrid: None,
        max_triangles: 500_000,
    }
}
fn contains(solid: &OccupiedSolid, p: Vertex) -> bool {
    match &solid.shape {
        SolidShape::SlopedPrism { .. } => panic!("legacy fixture"),
        SolidShape::Box { min, max } => (0..3).all(|a| min[a] <= p[a] && p[a] <= max[a]),
        SolidShape::TriangularPrism {
            footprint,
            bottom_cm,
            top_cm,
        } => p[1] >= *bottom_cm && p[1] <= *top_cm && point_in_polygon([p[0], p[2]], footprint),
    }
}

#[test]
fn optional_sidecar_preserves_all_generated_bytes_and_tree_proxies() {
    let d = document();
    for y in 0..2 {
        for x in 0..2 {
            let cell = Cell { x, y };
            let ordinary = generate(input(&d, cell)).unwrap();
            let result = generate_with_occupancy(input(&d, cell), 4096).unwrap();
            assert_eq!(
                canonical(&ordinary).unwrap(),
                canonical(&result.chunk).unwrap()
            );
            assert_eq!(ordinary.hash().unwrap(), result.chunk.hash().unwrap());
            for object in &ordinary.objects {
                assert_eq!(object.asset_id, "builtin:tree");
                let solid = result
                    .solids
                    .iter()
                    .find(|s| s.object_id == object.id)
                    .unwrap();
                let [x, h, z] = object.position;
                assert_eq!(
                    solid.shape,
                    SolidShape::Box {
                        min: [x - 20, h, z - 20],
                        max: [x + 20, h + 400, z + 20]
                    }
                );
            }
            if x == 1 && y == 1 {
                assert!(!ordinary.objects.is_empty());
            }
        }
    }
}

#[test]
fn concave_building_keeps_notch_empty_and_enclosed_interior_occupied() {
    let mut d = document();
    d.zones.clear();
    d.buildings[0].footprint = vec![
        [1000, 1000],
        [9000, 1000],
        [9000, 3000],
        [3000, 3000],
        [3000, 9000],
        [1000, 9000],
    ];
    d.buildings[0].base_cm = -100;
    d.buildings[0].height_cm = 1600;
    let result = generate_with_occupancy(input(&d, Cell { x: 0, y: 0 }), 4).unwrap();
    assert_eq!(result.solids.len(), 4);
    for p in [[2000, 500, 8000], [8000, 500, 2000], [2000, -100, 2000]] {
        assert!(result.solids.iter().any(|s| contains(s, p)));
    }
    for p in [[8000, 500, 8000], [2000, -101, 2000], [2000, 1501, 2000]] {
        assert!(!result.solids.iter().any(|s| contains(s, p)));
    }
    // The inside point is not on a wall or roof triangle.
    assert!(result
        .chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == "building-1")
        .all(|t| !t.spawnable));
}

#[test]
fn whole_cell_inside_building_retains_full_prisms_without_local_walls() {
    let mut d = document();
    d.bounds.max = [153600, 153600];
    d.zones.clear();
    d.buildings[0].footprint = vec![[100, 100], [153500, 100], [153500, 153500], [100, 153500]];
    let result = generate_with_occupancy(input(&d, Cell { x: 1, y: 1 }), 2).unwrap();
    assert_eq!(result.solids.len(), 2);
    assert!(result
        .solids
        .iter()
        .any(|s| contains(s, [75000, 500, 76000])));
    assert!(result
        .chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == "building-1")
        .all(|t| t.vertices.iter().all(|p| p[1] == 1500)));
}

#[test]
fn rotated_odd_sized_asset_proxies_keep_exact_extents_across_cells() {
    let mut d = document();
    d.zones.clear();
    d.buildings.clear();
    d.assets.push(Asset {
        id: "asset".into(),
        path: "asset.glb".into(),
        attribution: Attribution {
            source: "Synthetic".into(),
            license: "MIT".into(),
            notice: "".into(),
        },
        collision: vec![CollisionBox {
            center: [101, 100, -99],
            size_cm: [701, 501, 199],
        }],
    });
    let expected = [
        ([50951, -150, 51002], [51652, 351, 51201]),
        ([51200, -150, 50951], [51399, 351, 51652]),
        ([50749, -150, 51200], [51450, 351, 51399]),
        ([51002, -150, 50749], [51201, 351, 51450]),
    ];
    for turn in 0..4 {
        d.placements = vec![Placement {
            id: "proxy".into(),
            asset_id: "asset".into(),
            position: [51200, 0, 51200],
            quarter_turns: turn,
        }];
        for cell in [Cell { x: 0, y: 0 }, Cell { x: 1, y: 1 }] {
            let result = generate_with_occupancy(input(&d, cell), 1).unwrap();
            let (min, max) = expected[turn as usize];
            assert_eq!(
                result.solids,
                vec![OccupiedSolid {
                    object_id: "proxy".into(),
                    shape: SolidShape::Box { min, max }
                }]
            );
        }
    }
}

#[test]
fn budgets_fail_closed_and_normalized_order_is_stable() {
    let mut d = document();
    let cell = Cell { x: 0, y: 0 };
    assert_eq!(
        generate_with_occupancy(input(&d, cell), 1)
            .unwrap_err()
            .code,
        "E_BUDGET"
    );
    assert_eq!(
        generate_with_occupancy(input(&d, cell), 200_001)
            .unwrap_err()
            .code,
        "E_BUDGET"
    );
    let mut second = d.buildings[0].clone();
    second.id = "another".into();
    d.buildings.push(second);
    let first = generate_with_occupancy(input(&d, cell), 4).unwrap();
    d.buildings.reverse();
    let reversed = generate_with_occupancy(input(&d, cell), 4).unwrap();
    assert_eq!(first.solids, reversed.solids);
    assert_eq!(first.chunk.hash().unwrap(), reversed.chunk.hash().unwrap());
    // Solids belonging entirely to another cell do not consume this allowance.
    d.zones.clear();
    assert!(generate_with_occupancy(input(&d, Cell { x: 1, y: 1 }), 0)
        .unwrap()
        .solids
        .is_empty());
}

#[test]
fn boundary_trunk_requires_neighbor_owner_cell_and_keeps_unclipped_volume() {
    let mut d = document();
    d.cell_size_cm = 50000;
    d.buildings.clear();
    d.zones[0].polygon = vec![
        [49000, 30000],
        [51000, 30000],
        [51000, 32000],
        [49000, 32000],
    ];
    let left = generate_with_occupancy(input(&d, Cell { x: 0, y: 0 }), 100).unwrap();
    let right = generate_with_occupancy(input(&d, Cell { x: 1, y: 0 }), 100).unwrap();
    let id = "orchard-1:50:31";
    assert!(!left.solids.iter().any(|s| s.object_id == id));
    let solid = right.solids.iter().find(|s| s.object_id == id).unwrap();
    assert_eq!(
        solid.shape,
        SolidShape::Box {
            min: [49980, 0, 30980],
            max: [50020, 400, 31020]
        }
    );
    assert!(contains(solid, [49999, 200, 31000]));
    // Existing face clipping is intentionally unchanged by the optional sidecar.
    assert!(right
        .chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == id)
        .all(|t| t.vertices.iter().all(|p| p[0] >= 50000)));
}
