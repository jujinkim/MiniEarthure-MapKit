use mapkit_core::*;

fn source(kind: RoadKind, points: Vec<Vertex>) -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.roads.truncate(1);
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.bounds = Bounds {
        min: [0, 0],
        max: [10000, 10000],
    };
    for (id, p) in [("a", points[0]), ("b", *points.last().unwrap())] {
        d.nodes.push(RoadNode {
            id: id.into(),
            position: p,
            level: 0,
        });
    }
    let n = points.len() - 1;
    d.roads[0] = Road {
        id: "road".into(),
        from: "a".into(),
        to: "b".into(),
        points,
        widths_cm: vec![400; n],
        surfaces: vec![Surface::Asphalt; n],
        kind,
        clearance_cm: matches!(kind, RoadKind::Tunnel | RoadKind::Underpass).then_some(300),
        sidewalk_cm: Some(80),
        markings: Some(RoadMarkings {
            color: None,
            lanes: 4,
            center_line: true,
            edge_lines: true,
            crosswalk_start: false,
            crosswalk_end: false,
        }),
    };
    d
}
fn generated(d: &MapDocument, cell: Cell) -> GeneratedOccupancy {
    generate_with_occupancy(
        GenerationInput {
            document: d,
            cell,
            heightgrid: None,
            max_triangles: 200000,
        },
        20000,
    )
    .unwrap()
}
fn contains(s: &OccupiedSolid, p: Vertex) -> bool {
    if let SolidShape::Convex(c) = &s.shape {
        c.planes()
            .all(|(n, v)| (0..3).map(|i| n[i] * (p[i] - v[i]) as i128).sum::<i128>() <= 0)
    } else {
        false
    }
}
#[test]
fn bridge_base_rail_posts_caps_and_spawn_exclusion_match_collision() {
    for kind in [RoadKind::Elevated, RoadKind::Bridge] {
        let d = source(kind, vec![[1000, 600, 2500], [9000, 600, 2500]]);
        let c = generated(&d, Cell { x: 0, y: 0 });
        let solids = &c.solids;
        assert!(solids.iter().any(|s| contains(s, [2500, 675, 2708])));
        assert!(solids.iter().any(|s| contains(s, [2500, 675, 2292])));
        assert!(
            solids.iter().any(|s| contains(s, [992, 675, 2500])),
            "closed dead end"
        );
        assert!(
            !solids.iter().any(|s| contains(s, [2500, 650, 2500])),
            "full carriageway remains open"
        );
        assert_eq!(
            solids.iter().any(|s| contains(s, [2500, 610, 2710])),
            kind == RoadKind::Bridge
        );
        for convex in &c.chunk.asset_convexes {
            assert!(convex.shape.valid(1000000000));
        }
        assert!(c
            .chunk
            .triangles
            .iter()
            .filter(|t| t.object_id.contains(":safety:"))
            .all(|t| !t.spawnable));
        assert!(c
            .chunk
            .spawn(&SpawnRequest {
                position_cm: [2500, 2708],
                surface_id: "road:safety:metal".into()
            })
            .is_err());
        let cost = estimate_generation(&d, c.chunk.cell, 200000).unwrap();
        assert!(cost.asset_convexes >= c.chunk.asset_convexes.len() as u64);
        assert!(cost.occupied_solids >= solids.len() as u64);
        let key = archive_key(&"a".repeat(64), c.chunk.cell);
        let bytes = encode_archive(&c.chunk, &key, archive_limit(&cost)).unwrap();
        assert_eq!(
            decode_archive(&bytes, &key, c.chunk.cell, &cost).unwrap(),
            c.chunk
        );
    }
}
#[test]
fn junctions_bends_widths_slopes_and_input_order_preserve_open_passages() {
    for points in [
        vec![[1000, 600, 2500], [4800, 800, 2500], [4800, 1000, 8000]],
        vec![
            [1000, 600, 2500],
            [4300, 600, 2500],
            [5000, 600, 3200],
            [5700, 600, 3400],
            [8000, 600, 1800],
        ],
        vec![[1000, 600, 1000], [4800, 600, 4800], [7000, 600, 3000]],
    ] {
        let mut d = source(RoadKind::Bridge, points);
        let c = generated(&d, Cell { x: 0, y: 0 });
        assert!(c.chunk.asset_convexes.len() > 10);
        d.nodes.reverse();
        d.roads.reverse();
        assert_eq!(
            generated(&d, c.chunk.cell).chunk.hash().unwrap(),
            c.chunk.hash().unwrap()
        );
    }
    for arms in [3, 4] {
        let mut d = source(
            RoadKind::Elevated,
            vec![[1000, 600, 2500], [2500, 600, 2500]],
        );
        for (i, p) in [[4000, 600, 2500], [2500, 600, 4000], [2500, 600, 1000]]
            .into_iter()
            .take(arms - 1)
            .enumerate()
        {
            let id = format!("end{i}");
            d.nodes.push(RoadNode {
                id: id.clone(),
                position: p,
                level: 0,
            });
            let mut r = d.roads[0].clone();
            r.id = format!("arm{i}");
            r.from = "b".into();
            r.to = id;
            r.points = vec![[2500, 600, 2500], p];
            d.roads.push(r);
        }
        let c = generated(&d, Cell { x: 0, y: 0 });
        for p in [
            [2500, 640, 2500],
            [2250, 640, 2500],
            [2500, 640, 2750],
            [2750, 640, 2500],
        ] {
            assert!(
                !c.solids.iter().any(|s| contains(s, p)),
                "junction mouth blocked at {p:?}"
            );
        }
        let paint = d.road_paint(&d.bounds).unwrap();
        for path in paint.paths.values() {
            for s in path {
                assert!(
                    s.a[0] != 2500 || s.a[2] != 2500,
                    "no arbitrary through-junction stripe"
                );
            }
        }
    }
}
#[test]
fn adjacent_cells_share_unclipped_rail_heights_posts_and_paint_phase() {
    let d = source(
        RoadKind::Bridge,
        vec![[1000, 500, 2500], [9000, 1300, 2500]],
    );
    let a = generated(&d, Cell { x: 0, y: 0 });
    let b = generated(&d, Cell { x: 1, y: 0 });
    let common: Vec<_> = a.solids.iter().filter(|s| b.solids.contains(s)).collect();
    assert!(common.len() >= 6, "rails and crossing posts retained whole");
    let pa = d.road_paint(&d.cell_bounds(a.chunk.cell).unwrap()).unwrap();
    let pb = d.road_paint(&d.cell_bounds(b.chunk.cell).unwrap()).unwrap();
    assert_eq!(
        pa.paths["road"][0].station_cm,
        pb.paths["road"][0].station_cm
    );
    assert_eq!(pa.paths["road"][0].period_cm, pb.paths["road"][0].period_cm);
}
#[test]
fn degree_two_paint_curves_join_and_distinct_levels_do_not_connect() {
    let mut d = source(RoadKind::Bridge, vec![[1000, 600, 2500], [2500, 600, 2500]]);
    d.nodes.push(RoadNode {
        id: "c".into(),
        position: [2500, 600, 4500],
        level: 0,
    });
    let mut r = d.roads[0].clone();
    r.id = "turn".into();
    r.from = "b".into();
    r.to = "c".into();
    r.points = vec![[2500, 600, 2500], [2500, 600, 4500]];
    d.roads.push(r);
    let paint = d.road_paint(&d.bounds).unwrap();
    assert!(paint.paths["road"].len() > 3);
    assert!(paint.paths["turn"].len() > 3);
    assert_eq!(
        paint.paths["road"].last().unwrap().b,
        paint.paths["turn"][0].a
    );
    let mut r = d.roads[0].clone();
    r.id = "above".into();
    r.from = "upper-a".into();
    r.to = "upper-b".into();
    r.points = vec![[2200, 1600, 1000], [2200, 1600, 4500]];
    for (id, p) in [(r.from.clone(), r.points[0]), (r.to.clone(), r.points[1])] {
        d.nodes.push(RoadNode {
            id,
            position: p,
            level: 1,
        });
    }
    d.roads.push(r);
    let c = generated(&d, Cell { x: 0, y: 0 });
    assert_eq!(
        c.chunk
            .spawn(&SpawnRequest {
                position_cm: [2200, 2500],
                surface_id: "above".into()
            })
            .unwrap()[1],
        1600
    );
    assert_eq!(d.connected_roads("b").unwrap(), ["road", "turn"]);
}

#[test]
fn malformed_paths_reserved_ids_cancel_and_budgets_fail_with_context() {
    let mut d = source(
        RoadKind::Bridge,
        vec![
            [1000, 600, 1000],
            [4000, 600, 4000],
            [1000, 600, 4000],
            [4000, 600, 1000],
        ],
    );
    let e = d.validate().unwrap_err();
    assert_eq!(e.code, "E_GEOMETRY");
    assert!(e.message.contains("road") && e.message.contains("2500"));
    d = source(RoadKind::Bridge, vec![[1000, 600, 2500], [9000, 600, 2500]]);
    let token = cancellation::CancellationToken::default();
    token.cancel();
    assert_eq!(
        token
            .run(|| generate(GenerationInput {
                document: &d,
                cell: Cell { x: 0, y: 0 },
                heightgrid: None,
                max_triangles: 10000
            }))
            .unwrap_err()
            .code,
        "E_CANCELLED"
    );
    assert_eq!(
        generate_with_occupancy(
            GenerationInput {
                document: &d,
                cell: Cell { x: 0, y: 0 },
                heightgrid: None,
                max_triangles: 200000
            },
            1
        )
        .unwrap_err()
        .code,
        "E_BUDGET"
    );
    d.placements.push(Placement {
        id: "road:safety:metal".into(),
        asset_id: "builtin:fence".into(),
        position: [500, 600, 500],
        quarter_turns: 0,
    });
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    d.placements[0].id = "manual-fence".into();
    d.placements[0].position = [2500, 600, 2708];
    let e = d.validate().unwrap_err();
    assert!(e.message.contains("manual-fence") && e.message.contains("road"));
}

#[test]
fn rounded_paths_keep_usable_width_through_bends_and_width_transitions() {
    for (points, widths) in [
        (
            vec![[1000, 600, 2500], [4000, 600, 2500], [4000, 600, 6000]],
            vec![400, 400],
        ),
        (
            vec![[1000, 600, 2500], [4000, 600, 2500], [6500, 600, 5000]],
            vec![400, 600],
        ),
        (
            vec![
                [1000, 600, 2500],
                [3000, 600, 2500],
                [3800, 600, 3300],
                [4600, 600, 3300],
            ],
            vec![400, 400, 400],
        ),
    ] {
        let mut d = source(RoadKind::Bridge, points);
        d.roads[0].widths_cm = widths;
        let paint = d.road_paint(&d.bounds).unwrap();
        let mut chunks = std::collections::BTreeMap::new();
        for segment in &paint.paths["road"] {
            let dx = (segment.b[0] - segment.a[0]) as f64;
            let dz = (segment.b[2] - segment.a[2]) as f64;
            let length = libm::hypot(dx, dz);
            for t in [0.25, 0.5, 0.75] {
                for side in [-0.4, 0.0, 0.4] {
                    let p = [
                        libm::round(
                            segment.a[0] as f64 + dx * t - dz / length * segment.width_cm * side,
                        ) as i64,
                        libm::round(
                            segment.a[2] as f64 + dz * t + dx / length * segment.width_cm * side,
                        ) as i64,
                    ];
                    let cell = d.cell_at(p).unwrap();
                    let chunk = chunks.entry(cell).or_insert_with(|| generated(&d, cell));
                    assert!(
                        chunk
                            .chunk
                            .spawn(&SpawnRequest {
                                position_cm: p,
                                surface_id: "road".into()
                            })
                            .is_ok(),
                        "road at {p:?}: rounded lane lost deck support"
                    );
                }
            }
        }
    }
}

#[test]
fn acute_ground_approach_unions_and_quantized_tangencies_remain_simple() {
    for arms in [
        vec![
            ([-4000, 0], 550),
            ([3600, 0], 550),
            ([-200, -3400], 300),
            ([0, 1100], 280),
        ],
        vec![([0, -2000], 300), ([-4000, 1400], 300), ([-640, 0], 350)],
        vec![
            ([-2550, -150], 280),
            ([-550, 3250], 280),
            ([-650, -3600], 280),
            ([2200, 600], 280),
        ],
    ] {
        let center = [5000, 0, 5000];
        let mut d = source(
            RoadKind::Ground,
            vec![[5000 + arms[0].0[0], 0, 5000 + arms[0].0[1]], center],
        );
        d.roads[0].widths_cm = vec![arms[0].1];
        for (i, (offset, width)) in arms.iter().enumerate().skip(1) {
            let p = [center[0] + offset[0], 0, center[2] + offset[1]];
            let mut road = d.roads[0].clone();
            road.id = format!("branch{i}");
            road.from = "b".into();
            road.to = format!("end{i}");
            road.points = vec![center, p];
            road.widths_cm = vec![*width];
            d.nodes.push(RoadNode {
                id: road.to.clone(),
                position: p,
                level: 0,
            });
            d.roads.push(road);
        }
        let cell = d.cell_at([5000, 5000]).unwrap();
        let c = generated(&d, cell);
        assert!(!c.chunk.triangles.is_empty());
        assert!(d.road_paint(&d.cell_bounds(cell).unwrap()).is_ok());
        d.roads.reverse();
        d.nodes.reverse();
        assert_eq!(
            c.chunk.hash().unwrap(),
            generated(&d, cell).chunk.hash().unwrap()
        );
    }
}
