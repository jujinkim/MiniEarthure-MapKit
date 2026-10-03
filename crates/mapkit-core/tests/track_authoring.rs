use mapkit_core::assembled_track::{self as track, authoring::*};
#[test]
fn seed_duration_probe() {
    for circuit in [false, true] {
        let s = track::Settings {
            seed: 42,
            circuit,
            ..Default::default()
        };
        let a = track::assemble(&s).expect("connected duration candidate");
        println!(
            "mode={circuit} time={} pieces={}",
            a.estimated_msec,
            a.pieces.len()
        );
        assert!(a.estimated_msec.abs_diff(60000) <= 6000);
        assert_eq!(a, track::assemble(&s).unwrap());
    }
}
#[test]
fn free_placement_and_draft_source() {
    let mut source = Source::empty();
    let mut i = instance("a", "gentle45", 1200);
    i.position_cm = [113, 437, 251];
    i.rotation_mdeg = [0, 17300, 0];
    source.instances.push(i);
    let a = compile(&source).unwrap();
    assert!(!a.issues.is_empty());
    a.validate().unwrap();
    let mut corrupt = a;
    corrupt.pieces[0].path[0].position_cm[0] += 1;
    assert!(corrupt.validate().is_err());
}

#[test]
fn composed_shortcut_is_connected_with_shared_progress() {
    let source = shortcut_source();
    let a = compile(&source).unwrap();
    assert!(a.issues.is_empty(), "{:?}", a.issues);
    assert!(a.routes[1].estimated_msec < a.routes[0].estimated_msec);
    assert!(a.pieces.iter().any(|p| p.id == "flight_curve"));
    assert_eq!(a.pieces[3].width_cm, 800);
    assert_eq!(a.pieces[7].width_cm, 200);
    a.validate().unwrap();
}

#[test]
fn widths_curves_helices_and_continuous_jump() {
    for &w in track::WIDTHS {
        for preset in [
            "gentle45",
            "gentle90",
            "right90",
            "sharp135",
            "hairpin",
            "spiral90_left_up",
            "spiral180_right_down",
            "spiral360_left_up",
        ] {
            let p = piece(&instance("p", preset, w)).unwrap();
            assert_eq!(p.path[0].lateral_cm, w / 2);
            assert_eq!(p.path.last().unwrap().lateral_cm, w / 2);
            assert_eq!(p.path[0].normal, [0, 1000000, 0]);
            assert_eq!(p.path.last().unwrap().normal, [0, 1000000, 0]);
            for sample in &p.path {
                let dot: i128 = sample
                    .normal
                    .iter()
                    .zip(sample.forward)
                    .map(|(a, b)| i128::from(*a) * i128::from(b))
                    .sum();
                assert!(dot.abs() < 2000000, "{preset} w={w} frame twist");
            }
            let radius = if preset.starts_with("gentle") {
                1600.max(w * 4)
            } else if preset.starts_with("right") {
                400.max(w / 2 + 200)
            } else if preset.starts_with("spiral") {
                800.max(w / 2 + 700)
            } else {
                300.max(w / 2 + 100)
            };
            assert!(radius > w / 2);
            let mut source = Source::empty();
            source.instances.push(instance("p", preset, w));
            assert!(
                !compile(&source)
                    .unwrap()
                    .issues
                    .iter()
                    .any(|s| s.contains("self intersection")),
                "{preset} w={w}"
            );
        }
    }
    let jump = piece(&instance("jump", "jump_panel", 400)).unwrap();
    assert!(jump.path.iter().all(|s| s.mode != "flight"));
    let gap = piece(&instance("gap", "jump", 400)).unwrap();
    assert!(gap.path.iter().any(|s| s.mode == "flight"));
    let mut wide = instance("wide", "straight", 1200);
    wide.entry_width_cm = 200;
    let p = piece(&wide).unwrap();
    assert_eq!(p.path[0].lateral_cm, 100);
    assert_eq!(p.path.last().unwrap().lateral_cm, 600);
}
#[test]
fn category_combinations_time_bounds_and_exclusion() {
    for mask in 1..8 {
        for circuit in [false, true] {
            for seconds in [60, 90, 120] {
                let categories = track::selection_ids()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, s)| s.to_string())
                    .collect();
                let settings = track::Settings {
                    seed: 42,
                    circuit,
                    duration_seconds: seconds,
                    categories,
                    ..Default::default()
                };
                let a = track::assemble(&settings).unwrap_or_else(|e| {
                    panic!("mask={mask} circuit={circuit} seconds={seconds}: {e:?}")
                });
                let target = u32::from(seconds) * 1000;
                assert!(a.estimated_msec.abs_diff(target) <= target / 10);
                for p in &a.pieces {
                    if ![
                        "straight",
                        "finish_plaza",
                        "tube_entry",
                        "tube_exit",
                        "free_curve",
                        // Basic return modules replace the former always-available free curve.
                        "right90", "right90_left", "gentle45", "gentle45_left",
                        "slope_up", "slope_down", "spiral360_right_up", "spiral360_left_up",
                        "spiral360_right_down", "spiral360_left_down",
                    ]
                    .contains(&p.id.as_str())
                    {
                        assert!(settings
                            .categories
                            .iter()
                            .any(|c| c == track::category(&p.id)));
                    }
                }
                println!(
                    "mask={mask} circuit={circuit} seconds={seconds} actual={}",
                    a.estimated_msec
                );
            }
        }
    }
    let mut empty = track::Settings::default();
    empty.categories.clear();
    assert!(track::assemble(&empty).is_err());
}

#[test]
fn full_orientation_snap_and_pipe_portals() {
    for width in [200, 400, 600] {
        let mut source = Source::empty();
        source.settings.circuit = false;
        let mut previous = instance("road", "straight", width);
        previous.position_cm = [311, 713, -251];
        previous.rotation_mdeg = [11300, 17300, 7100];
        source.instances.push(previous.clone());
        for (id, preset) in [
            ("entry", "tube_entry"),
            ("pipe", "cylinder"),
            ("exit", "tube_exit"),
            ("end", "straight"),
        ] {
            let next = snap(&instance(id, preset, width), &previous).unwrap();
            source.connections.push(Connection {
                from: previous.id.clone(),
                to: id.into(),
            });
            source.instances.push(next.clone());
            previous = next;
        }
        let a = compile(&source).unwrap();
        assert!(
            !a.issues.iter().any(|i| i.contains("ports do not meet")),
            "{:?}",
            a.issues
        );
        for pair in a.pieces.windows(2) {
            let n = pair[0].path.last().unwrap().normal;
            assert!(n
                .iter()
                .zip(pair[1].path[0].normal)
                .all(|(a, b)| (a - b).abs() < 100));
        }
    }
}
#[test]
fn free_curve_vertical_frame_and_degenerate_rejection() {
    let mut i = instance("vertical", "free_curve", 400);
    i.control_points = vec![[0, 0, 0], [0, 0, 1000], [0, 1000, 1000], [0, 2000, 1000]];
    let p = piece(&i).unwrap();
    for pair in p.path.windows(2) {
        let dot: i64 = pair[0]
            .normal
            .iter()
            .zip(pair[1].normal)
            .map(|(a, b)| a * b)
            .sum();
        assert!(dot > 990_000_000_000, "abrupt curve twist");
    }
    i.control_points = vec![[0; 3]; 4];
    assert!(piece(&i).is_err());
}

#[test]
fn tilted_ribbon_edges_reserve_vertical_clearance() {
    let mut source = Source::empty();
    source.instances.push(instance("lower", "straight", 400));
    let mut upper = instance("upper", "straight", 1200);
    upper.position_cm = [0, 500, 0];
    upper.rotation_mdeg = [0, 0, 90000];
    source.instances.push(upper);
    let a = compile(&source).unwrap();
    assert!(a
        .issues
        .iter()
        .any(|i| i.contains("road clearance collision")));
    assert!(a.floor.min_cm[1] < -100);
}

#[test]
fn flight_export_needs_declared_approach_landing_and_supported_progress() {
    let mut source = shortcut_source();
    source.actions.clear();
    let a = compile(&source).unwrap();
    assert!(a
        .issues
        .iter()
        .any(|i| i.contains("declared supported landing")));
    source = shortcut_source();
    source.checkpoints[0] = Checkpoint {
        piece: "jump-entry".into(),
        sample: 0,
    };
    let a = compile(&source).unwrap();
    assert!(a
        .issues
        .iter()
        .any(|i| i.contains("supported driving surface")));
}

fn straight(id: &str, start: [i64; 3], end: [i64; 3]) -> Instance {
    let mut road = instance(id, "free_curve", 800);
    road.position_cm = start;
    road.control_points = (0..=3).map(|n| std::array::from_fn(|j| (end[j] - start[j]) * n / 3)).collect();
    road
}

#[test]
fn finite_straight_clearance_allows_eight_metre_six_radius_corners() {
    for side in [-1, 1] {
        for yaw in [0, 17300, 45000, 90000, 137250, 180000, 270000] {
            let mut source = Source::empty();
            source.settings.circuit = false;
            let mut corner = instance("bend", "free_curve", 800);
            corner.control_points = vec![[0, 0, 0], [0, 0, 331], [side * 269, 0, 600], [side * 600, 0, 600]];
            source.instances = vec![
                straight("approach", [0, 0, -3000], [0, 0, 0]), corner,
                straight("departure", [side * 600, 0, 600], [side * 3000, 0, 600]),
            ];
            let angle = f64::from(yaw).to_radians() / 1000.0;
            for road in &mut source.instances {
                let [x, y, z] = road.position_cm;
                road.position_cm = [113 + (x as f64 * angle.cos() + z as f64 * angle.sin()).round() as i64,
                    y + 437, -251 + (-x as f64 * angle.sin() + z as f64 * angle.cos()).round() as i64];
                road.rotation_mdeg = [0, yaw, 0];
            }
            source.connections = vec![Connection { from: "approach".into(), to: "bend".into() },
                Connection { from: "bend".into(), to: "departure".into() }];
            source.paths = vec![Path { id: "route".into(), pieces: vec!["approach".into(), "bend".into(), "departure".into()] }];
            source.checkpoints = vec![Checkpoint { piece: "approach".into(), sample: 0 },
                Checkpoint { piece: "departure".into(), sample: 0 }];
            let assembly = compile(&source).unwrap();
            assert!(assembly.issues.is_empty(), "side={side}, yaw={yaw}: {:?}", assembly.issues);
            assembly.validate().unwrap();
        }
    }
}

#[test]
fn finite_straight_clearance_keeps_crossings_overlaps_margins_and_height() {
    let approach = straight("approach", [0, 0, -3000], [0, 0, 0]);
    for (label, other, collision) in [
        ("crossing", straight("other", [-1500, 0, -1500], [1500, 0, -1500]), true),
        ("overlap", straight("other", [0, 0, -2900], [0, 0, 100]), true),
        ("corner overlap", straight("other", [400, 0, 400], [2800, 0, 400]), true),
        ("corner clearance", straight("other", [550, 0, 550], [2950, 0, 550]), true),
        ("corner touching", straight("other", [560, 0, 560], [2960, 0, 560]), true),
        ("corner separated", straight("other", [562, 0, 562], [2962, 0, 562]), false),
        ("parallel clearance", straight("other", [959, 0, -3000], [959, 0, 0]), true),
        ("end clearance", straight("other", [0, 0, 159], [0, 0, 2559]), true),
        ("end touching", straight("other", [0, 0, 160], [0, 0, 2560]), true),
        ("end separated", straight("other", [0, 0, 161], [0, 0, 2561]), false),
        ("shared port", straight("other", [0, 0, 0], [0, 0, 2400]), false),
        ("low overpass", straight("other", [-1500, 249, -1500], [1500, 249, -1500]), true),
        ("vertical boundary", straight("other", [-1500, 250, -1500], [1500, 250, -1500]), false),
        ("clear overpass", straight("other", [-1500, 251, -1500], [1500, 251, -1500]), false),
    ] {
        let mut source = Source::empty();
        source.instances = vec![approach.clone(), other];
        let assembly = compile(&source).unwrap();
        assert_eq!(assembly.issues.iter().any(|i| i.contains("road clearance collision")), collision, "{label}: {:?}", assembly.issues);
    }
}

#[test]
fn straight_clearance_detects_phase_shifted_one_centimetre_overlap() {
    let mut source = Source::empty();
    source.instances = vec![
        straight("approach", [0, 0, -3000], [0, 0, 0]),
        straight("other", [959, 0, -2900], [959, 0, 100]),
    ];
    let assembly = compile(&source).unwrap();
    assert!(assembly.issues.iter().any(|i| i.contains("road clearance collision")),
        "959cm lateral / 100cm longitudinal offset: {:?}", assembly.issues);
}

fn assert_road_collision(roads: Vec<Instance>, expected: bool, context: &str) {
    let mut source = Source::empty();
    source.instances = roads;
    let assembly = compile(&source).unwrap();
    assert_eq!(assembly.issues.iter().any(|i| i.contains("road clearance collision")),
        expected, "{context}: {:?}", assembly.issues);
}

#[test]
fn straight_clearance_is_independent_of_sample_phase_and_piece_order() {
    for width in [400, 800, 1200] {
        // Include unequal widths: each ribbon keeps its own wall/vehicle margin.
        let boundary = (800 + width) / 2 + 160;
        for offset in [-199, -100, -1, 0, 1, 50, 99, 100, 199] {
            for (gap, expected) in [(-1, true), (0, true), (1, false)] {
                let a = straight("a", [0, 0, -3000], [0, 0, 0]);
                let x = i64::from(boundary) + gap;
                let mut b = straight("b", [x, 0, -3000 + offset], [x, 0, offset]);
                b.width_cm = width;
                b.entry_width_cm = width;
                b.exit_width_cm = width;
                let context = format!("width={width} offset={offset} gap={gap}");
                assert_road_collision(vec![a.clone(), b.clone()], expected, &context);
                assert_road_collision(vec![b, a], expected, &context);
            }
        }
    }
}

#[test]
fn straight_clearance_rotations_translations_and_vertical_intervals() {
    for yaw in [0, 17300, 45000, 90000, 137250, 180000, 270000] {
        for origin in [[113, 437, -251], [-90000, -500, 70000]] {
            for (label, mut b, expected) in [
                ("phase shifted overlap", straight("b", [959, 0, -2900], [959, 0, 100]), true),
                // Leave room for conservative integer ribbon rounding after rotation.
                ("separated", straight("b", [965, 0, -2900], [965, 0, 100]), false),
                ("crossing", straight("b", [-1500, 0, -1500], [1500, 0, -1500]), true),
                ("low overpass", straight("b", [959, 249, -2900], [959, 249, 100]), true),
                ("vertical boundary", straight("b", [959, 250, -2900], [959, 250, 100]), false),
                ("underpass boundary", straight("b", [959, -250, -2900], [959, -250, 100]), false),
            ] {
                let mut a = straight("a", [0, 0, -3000], [0, 0, 0]);
                let angle = f64::from(yaw).to_radians() / 1000.0;
                for road in [&mut a, &mut b] {
                    let [x, y, z] = road.position_cm;
                    road.position_cm = [origin[0] + (x as f64 * angle.cos() + z as f64 * angle.sin()).round() as i64,
                        origin[1] + y, origin[2] + (-x as f64 * angle.sin() + z as f64 * angle.cos()).round() as i64];
                    road.rotation_mdeg = [0, yaw, 0];
                }
                assert_road_collision(vec![a, b], expected, &format!("{label} yaw={yaw} origin={origin:?}"));
            }
        }
    }
}

#[test]
fn invalid_action_and_landing_sample_still_rejected() {
    let original = shortcut_source();
    let assembly = compile(&original).unwrap();
    for landing in [false, true] {
        let mut source = original.clone();
        let action = &mut source.actions[0];
        let (name, sample) = if landing {
            let cp = action.landing.as_mut().unwrap();
            (&cp.piece, &mut cp.sample)
        } else { (&action.piece, &mut action.sample) };
        let index = source.instances.iter().position(|i| &i.id == name).unwrap();
        *sample = assembly.pieces[index].path.len();
        assert_eq!(compile(&source).unwrap_err().code, "E_TRACK_SOURCE");
    }
}
