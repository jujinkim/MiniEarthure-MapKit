use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
#[test]
fn prepared_region_rechecks_mutated_indexed_source_and_limits_cells() {
    let mut source = document().into_indexed_source().unwrap();
    let region = CellRegion { min: Cell { x: 0, y: 0 }, end: Cell { x: 1, y: 1 } };
    source.roads.reverse();
    source.nodes.reverse();
    let prepared = PreparedMap::new_region(source.clone(), region).unwrap();
    let expected = generate_with_occupancy(GenerationInput {
        document: &source, cell: region.min, heightgrid: None, max_triangles: 500_000,
    }, MAX_OCCUPIED_SOLIDS).unwrap();
    let actual = prepared.generate_with_occupancy(region.min, None, 500_000, Some(MAX_OCCUPIED_SOLIDS)).unwrap();
    assert_eq!(actual.chunk, expected.chunk);
    assert_eq!(actual.solids, expected.solids);
    assert!(prepared.estimate(region.end, 500_000).is_err());
    source.roads[0].widths_cm.clear();
    // Indexed topology survives public mutation; it must never skip validation.
    assert!(PreparedMap::new_region(source, region).is_err());
    let invalid_region = CellRegion { min: region.min, end: Cell { x: 999, y: 999 } };
    assert!(PreparedMap::new_region(document(), invalid_region).is_err());
}
#[test]
fn prepared_source_is_immutable_and_preserves_generation_and_budget_results() {
    let mut source = document();
    let prepared = PreparedMap::new(source.clone()).unwrap();
    for cell in source.cells() {
        let expected = generate(GenerationInput {
            document: &source,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        })
        .unwrap();
        assert_eq!(prepared.generate(cell, None, 500_000).unwrap(), expected);
        assert_eq!(
            prepared.estimate(cell, 500_000).unwrap(),
            estimate_generation(&source, cell, 500_000).unwrap()
        );
        assert_eq!(prepared.estimate(cell, 1).unwrap().triangles, 1);
        assert_eq!(
            prepared
                .generate_with_occupancy(cell, None, 500_000, Some(MAX_OCCUPIED_SOLIDS))
                .unwrap()
                .chunk,
            expected
        );
    }
    source.roads[0].widths_cm.clear();
    assert!(PreparedMap::new(source).is_err());
    assert!(prepared
        .generate(Cell { x: 0, y: 0 }, None, 500_000)
        .is_ok());
    assert!(prepared.estimate(Cell { x: -1, y: 0 }, 500_000).is_err());
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
    let occupied = generate_with_occupancy(
        GenerationInput {
            document: d,
            cell,
            heightgrid: grid,
            max_triangles: 500_000,
        },
        MAX_OCCUPIED_SOLIDS,
    )
    .unwrap();
    assert!(
        occupied.solids.len() as u64 <= cost.occupied_solids,
        "{cell:?}"
    );
    assert!(occupied
        .solids
        .iter()
        .all(|s| s.object_id.len() as u64 <= cost.max_object_id_bytes));
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
    d.assets.push(Asset { distant_path: None,
        convex_collision: vec![],
        material: None,
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
            position: [12800 + turn as i64 * 1000, 0, 10000],
            quarter_turns: turn,
            yaw_offset_mdeg: 0,
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

#[test]
fn authored_candidates_include_visual_anchors_and_proxy_overlap() {
    let source: mapkit_core::MapDocument = serde_json::from_str(include_str!("../../../examples/assets/document.json")).unwrap();
    let prepared = mapkit_core::PreparedMap::new(source).unwrap();
    for cell in prepared.cells() {
        let candidates = prepared.authored_placement_candidates(cell).unwrap();
        let generated = prepared.generate(cell, None, 500_000).unwrap();
        for triangle in &generated.triangles {
            if prepared.placements.iter().any(|p| p.id == triangle.object_id) {
                assert!(candidates.iter().any(|p| p.id == triangle.object_id));
            }
        }
        for placement in &prepared.placements {
            if prepared.cell_at([placement.position[0], placement.position[2]]) == Some(cell) {
                assert!(candidates.iter().any(|p| p.id == placement.id));
            }
        }
    }
}

/// A dense flat city: a 6×6 junction grid with sidewalks and markings, a marked
/// two-arm chain crossing cell borders, a tunnel, a bridge, paint islands that
/// force a dense tile split, buildings with parts, a forest and barriers.
fn dense_city() -> MapDocument {
    let mut d = document();
    d.bounds = Bounds { min: [0, 0], max: [38400, 38400] };
    d.cell_size_cm = 12800;
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    let node = |id: String, p: Vertex, level: i32| RoadNode { id, position: p, level };
    let road = |id: String, from: String, to: String, points: Vec<Vertex>, kind: RoadKind, sidewalk: Option<u32>, marked: bool| Road {
        design: None,
        snow_retention_percent: 100,
        id,
        from,
        to,
        widths_cm: vec![600; points.len() - 1],
        surfaces: vec![Surface::Asphalt; points.len() - 1],
        points,
        kind,
        clearance_cm: matches!(kind, RoadKind::Tunnel | RoadKind::Underpass).then_some(450),
        sidewalk_cm: sidewalk,
        markings: marked.then_some(RoadMarkings { color: None, lanes: 2, center_line: true, edge_lines: false, crosswalk_start: false, crosswalk_end: false, one_way: 0, stop_lines: false }),
    };
    let lines: Vec<i64> = (0..6).map(|i| 3200 + i * 6400).collect();
    for (i, &x) in lines.iter().enumerate() {
        for (j, &z) in lines.iter().enumerate() {
            d.nodes.push(node(format!("n{i}-{j}"), [x, 0, z], 0));
        }
    }
    for i in 0..6 {
        for j in 0..5 {
            let (a, b) = (lines[i], lines[j]);
            let (c, e) = (lines[j + 1], lines[j + 1]);
            d.roads.push(road(format!("h{i}-{j}"), format!("n{j}-{i}"), format!("n{}-{i}", j + 1), vec![[b, 0, a], [c, 0, a]], RoadKind::Ground, None, true));
            d.roads.push(road(format!("v{i}-{j}"), format!("n{i}-{j}"), format!("n{i}-{}", j + 1), vec![[a, 0, b], [a, 0, e]], RoadKind::Ground, Some(250), false));
        }
    }
    // Marked chain below the grid: three roads joined by two-arm nodes, crossing all three cell columns.
    for (k, x) in [0, 12000, 26000, 38400].iter().enumerate() {
        d.nodes.push(node(format!("c{k}"), [*x, 0, 800], 0));
    }
    for k in 0..3 {
        let (x0, x1) = ([0, 12000, 26000][k], [12000, 26000, 38400][k]);
        d.roads.push(road(format!("chain{k}"), format!("c{k}"), format!("c{}", k + 1), vec![[x0, 0, 800], [(x0 + x1) / 2, 0, 1200], [x1, 0, 800]], RoadKind::Ground, Some(200), true));
    }
    // A tunnel above the grid and a bridge left of it; neither crosses a ground road.
    d.nodes.push(node("t0".into(), [0, -700, 37400], -1));
    d.nodes.push(node("t1".into(), [38400, -700, 37400], -1));
    d.roads.push(road("tunnel".into(), "t0".into(), "t1".into(), vec![[0, -700, 37400], [38400, -700, 37400]], RoadKind::Tunnel, None, false));
    d.nodes.push(node("u0".into(), [0, -500, 36300], -1));
    d.nodes.push(node("u1".into(), [38400, -500, 36300], -1));
    d.roads.push(road("underpass".into(), "u0".into(), "u1".into(), vec![[0, -500, 36300], [38400, -500, 36300]], RoadKind::Underpass, None, false));
    d.nodes.push(node("b0".into(), [1400, 700, 4000], 1));
    d.nodes.push(node("b1".into(), [1400, 700, 34000], 1));
    d.roads.push(road("bridge".into(), "b0".into(), "b1".into(), vec![[1400, 700, 4000], [1400, 700, 34000]], RoadKind::Bridge, None, true));
    // Paint: 20 islands inside one block (more rings than one face may hold) and a 64-gon crossing roads.
    for k in 0..20 {
        let (x, z) = (16500 + (k % 5) * 900, 16500 + (k / 5) * 900);
        d.surface_areas.push(SurfaceArea { id: format!("island{k}"), polygon: vec![[x, z], [x + 400, z], [x + 400, z + 400], [x, z + 400]], surface: Surface::Dirt });
    }
    let ring: Vec<Point> = (0..64).map(|k| {
        let a = k as f64 / 64.0 * std::f64::consts::TAU;
        [25600 + (6000.0 * a.cos()) as i64, 25600 + (6000.0 * a.sin()) as i64]
    }).collect();
    d.surface_areas.push(SurfaceArea { id: "plaza".into(), polygon: ring, surface: Surface::Gravel });
    // Buildings beside roads, one with parts and one across a cell border.
    for (k, (x, z)) in [(4500, 4500), (10900, 10900), (12300, 23000), (30000, 8000), (23000, 30000)].iter().enumerate() {
        d.buildings.push(Building {
            id: format!("bld{k}"),
            footprint: vec![[*x, *z], [x + 1600, *z], [x + 1600, z + 1200], [*x, z + 1200]],
            holes: vec![],
            base_cm: 0,
            height_cm: 900 + k as u32 * 300,
            usage: "residential".into(),
            material: "concrete".into(),
            roof: "flat".into(),
            entrances: vec![],
            parts: if k == 0 { vec![BuildingPart { footprint: vec![[x + 1800, *z], [x + 2600, *z], [x + 2600, z + 800], [x + 1800, z + 800]], holes: vec![], base_cm: None, height_cm: Some(600) }] } else { vec![] },
        });
    }
    d.zones.push(Zone { id: "forest".into(), polygon: vec![[17000, 4000], [31000, 4000], [31000, 8800], [17000, 8800]], kind: ZoneKind::Forest, spacing_cm: 700, density_per_mille: 1000, exclusions: vec![], tree: None });
    d.barriers.push(mapkit_core::barrier::Barrier { id: "fence".into(), kind: mapkit_core::barrier::BarrierKind::Fence, line: vec![[4200, 7500], [20000, 7500], [20000, 14000]], height_cm: 120, thickness_cm: 10 });
    d.barriers.push(mapkit_core::barrier::Barrier { id: "wall".into(), kind: mapkit_core::barrier::BarrierKind::Wall, line: vec![[30000, 12000], [30000, 20000]], height_cm: 200, thickness_cm: 30 });
    d.normalize();
    d.validate().unwrap();
    d
}
#[test]
fn dense_flat_city_cells_are_bounded() {
    let d = dense_city();
    let mut ground_faces = 0;
    for cell in d.cells() {
        assert_bound(&d, cell, None);
        let chunk = generate(GenerationInput { document: &d, cell, heightgrid: None, max_triangles: 500_000 }).unwrap();
        ground_faces += chunk.triangles.iter().filter(|t| t.object_id == "terrain").count();
        assert!(chunk.triangles.iter().any(|t| t.object_id.ends_with(":sidewalk")), "{cell:?} has sidewalks");
    }
    assert!(ground_faces > 0);
    let prepared = PreparedMap::new(d.clone()).unwrap();
    for cell in d.cells() {
        assert_eq!(prepared.estimate(cell, 500_000).unwrap(), estimate_generation(&d, cell, 500_000).unwrap());
    }
    // The junction block with twenty paint islands splits its tile; the estimate still bounds it.
    let busy = Cell { x: 1, y: 1 };
    let chunk = generate(GenerationInput { document: &d, cell: busy, heightgrid: None, max_triangles: 500_000 }).unwrap();
    assert!(chunk.triangles.iter().any(|t| t.object_id.starts_with("island")));
    let cost = estimate_generation(&d, busy, 500_000).unwrap();
    assert!(cost.triangles < 60_000, "flat city estimate is near its output: {}", cost.triangles);
    assert!(cost.triangles >= chunk.triangles.len() as u64);
}
#[test]
fn road_style_bound_covers_styled_roads() {
    let d = dense_city();
    let mut covered = 0;
    for cell in d.cells() {
        let chunk = generate(GenerationInput { document: &d, cell, heightgrid: None, max_triangles: 500_000 }).unwrap();
        // The renderer keys one style per road and surface with a spawnable face in the chunk, marked or not.
        let styled: std::collections::BTreeSet<(&str, u8)> = chunk.triangles.iter().filter(|t| t.spawnable)
            .filter(|t| d.roads.iter().any(|r| r.id == t.object_id))
            .map(|t| (t.object_id.as_str(), t.surface as u8)).collect();
        let bound = road_style_bound(&d, cell).unwrap();
        assert!(styled.len() as u64 <= bound, "{cell:?}: {} styled roads > bound {bound}", styled.len());
        assert!(bound < d.roads.len() as u64 * 2, "{cell:?}: bound {bound} is cell-local");
        covered += styled.len();
    }
    assert!(covered > 0);
}
