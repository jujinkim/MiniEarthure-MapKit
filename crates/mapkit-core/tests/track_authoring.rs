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
