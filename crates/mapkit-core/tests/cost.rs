use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
fn assert_bound(d: &MapDocument, cell: Cell, grid: Option<&HeightGrid>) {
    let cost = estimate_generation(d, cell, 500_000).unwrap();
    let chunk = generate(GenerationInput {
        document: d,
        cell,
        heightgrid: grid,
        max_triangles: 500_000,
    })
    .unwrap();
    let occupied = generate_with_occupancy(GenerationInput {
        document: d, cell, heightgrid: grid, max_triangles: 500_000,
    }, MAX_OCCUPIED_SOLIDS).unwrap();
    assert!(occupied.solids.len() as u64 <= cost.occupied_solids, "{cell:?}");
    assert!(occupied.solids.iter().all(|s| s.object_id.len() as u64 <= cost.max_object_id_bytes));
    assert_eq!(chunk, occupied.chunk);
    assert!(chunk.triangles.len() as u64 <= cost.triangles, "{cell:?}");
    assert!(chunk.objects.len() as u64 <= cost.objects, "{cell:?}");
    for triangle in &chunk.triangles {
        assert!(triangle.object_id.len() as u64 <= cost.max_object_id_bytes);
    }
    for object in &chunk.objects {
        assert!(object.id.len() as u64 <= cost.max_object_id_bytes);
    }
    assert_eq!(cost, estimate_generation(d, cell, 500_000).unwrap());
}
#[test]
fn clipping_structures_and_sparse_cells_are_bounded() {
    let mut d = document();
    d.cell_size_cm = 12800;
    d.bounds.max = [101333, 99999];
    d.nodes.retain(|n| n.id == "west" || n.id == "east");
    d.nodes[1].position = [101333, 20, 25600];
    d.roads.truncate(1);
    d.roads[0].points.last_mut().unwrap()[0] = 101333;
    d.roads[0].kind = RoadKind::Tunnel;
    d.roads[0].clearance_cm = Some(500);
    d.zones[0].kind = ZoneKind::Forest;
    d.normalize();
    for y in 0..8 {
        for x in 0..8 {
            assert_bound(&d, Cell { x, y }, None);
        }
    }
    let empty = estimate_generation(&d, Cell { x: 0, y: 0 }, 500_000).unwrap();
    let forest = estimate_generation(&d, Cell { x: 4, y: 4 }, 500_000).unwrap();
    assert!(forest.objects > empty.objects);
    assert!(forest.triangles > empty.triangles);
}
#[test]
fn sampled_terrain_and_partial_edges_are_bounded_without_decoding() {
    let mut d = document();
    d.roads.clear();
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.bounds.max = [100111, 99999];
    let cell = Cell { x: 1, y: 1 };
    d.heightmaps.push(Heightmap {
        cell,
        path: "terrain.png".into(),
        spacing_cm: 200,
        offset_cm: 0,
        step_cm: 1,
        source_accuracy_cm: None,
    });
    let grid = HeightGrid {
        side: 257,
        heights_cm: (0..257 * 257).map(|i| (i % 71) as i64).collect(),
    };
    assert_bound(&d, cell, Some(&grid));
    assert_eq!(
        estimate_generation(&d, cell, 500_000)
            .unwrap()
            .height_samples,
        257 * 257
    );
    assert_eq!(estimate_generation(&d, cell, 3).unwrap().triangles, 3);
    assert!(estimate_generation(&d, Cell { x: 2, y: 2 }, 500_000).is_err());
}

#[test]
fn rotated_proxies_concave_buildings_and_negative_origin_are_bounded() {
    let mut d = document();
    d.bounds.min = [-12800, -12800];
    d.cell_size_cm = 12800;
    d.zones.clear();
    d.buildings[0].footprint = vec![
        [12000, 12000],
        [15000, 12000],
        [15000, 13000],
        [13000, 13000],
        [13000, 15000],
        [12000, 15000],
    ];
    d.assets.push(Asset { convex_collision: vec![], material: None,
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
    for turn in 0..4 {
        d.placements.push(Placement {
            id: format!("proxy-{turn}"),
            asset_id: "asset".into(),
            position: [12800 + turn as i64 * 100, 0, 12800],
            quarter_turns: turn,
        });
    }
    d.normalize();
    for y in 0..4 {
        for x in 0..4 {
            assert_bound(&d, Cell { x, y }, None);
        }
    }
    let before = estimate_generation(&d, Cell { x: 2, y: 2 }, 500_000).unwrap();
    d.placements.reverse();
    d.roads.reverse();
    d.nodes.reverse();
    assert_eq!(
        before,
        estimate_generation(&d, Cell { x: 2, y: 2 }, 500_000).unwrap()
    );
}
