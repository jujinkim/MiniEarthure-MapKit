use mapkit_core::assembled_track::*;
use mapkit_package::{assembled_track as package, *};
use std::collections::BTreeMap;

#[test]
fn generated_walls_have_no_reverse_coplanar_duplicates() {
    let d = document(&Settings {
        seed: 1,
        circuit: false,
        duration_seconds: 60,
        gimmicks: vec![],
        ..Settings::default()
    })
    .unwrap();
    let assembly = d.assembled_track.as_ref().unwrap();
    let mut checked = 0;
    for piece in [
        assembly.pieces.first().unwrap(),
        assembly.pieces.last().unwrap(),
    ] {
        let p = piece.origin_cm;
        let cell = d.cell_at([p[0], p[2]]).unwrap();
        let chunk = mapkit_core::generate(mapkit_core::GenerationInput {
            document: &d,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        })
        .unwrap();
        let mut planes = BTreeMap::new();
        for face in chunk
            .triangles
            .iter()
            .filter(|f| f.object_id.starts_with("assembled-wall-"))
        {
            let [a, b, c] = face.vertices;
            let ab = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
            let ac = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
            let normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            for axis in [0, 2] {
                if a[axis] != b[axis] || a[axis] != c[axis] || normal[axis] == 0 {
                    continue;
                }
                let key = (face.object_id.clone(), axis, a[axis]);
                let sign = normal[axis].signum();
                if let Some(previous) = planes.insert(key, sign) {
                    assert_eq!(
                        previous, sign,
                        "one two-sided sheet, no overlapping reversed wall faces"
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 8,
        "straight and finish boundary fixtures include wall faces"
    );
}

#[test]
fn reproducible_roundtrip_and_modified_source_rejected() {
    let settings = Settings::default();
    let d = package::generate(&settings).unwrap();
    let bytes = pack_bytes(d.clone(), BTreeMap::new()).unwrap();
    assert_eq!(
        bytes,
        pack_bytes(package::generate(&settings).unwrap(), BTreeMap::new()).unwrap()
    );
    let p = read_bytes(&bytes).unwrap();
    assert_eq!(p.document.assembled_track, d.assembled_track);
    package::verify(
        &p.document,
        &p.inspection.world_content_hash,
        &p.document.courses[0],
    )
    .unwrap();
    let mut modified = p.document.to_document();
    modified.seed += 1;
    assert!(verify_document(&modified).is_err());
    let mut course = p.document.courses[0].definition.clone();
    course.checkpoints[1].radius_cm += 1;
    let course = mapkit_core::course::Course::from_definition(course, &p.document.bounds).unwrap();
    assert!(package::verify(&p.document, &p.inspection.world_content_hash, &course).is_err());
}
#[test]
fn cancellation_and_invalid_requests() {
    let token = mapkit_core::cancellation::CancellationToken::default();
    token.cancel();
    assert_eq!(
        token
            .run(|| assemble(&Settings::default()))
            .unwrap_err()
            .code,
        "E_CANCELLED"
    );
    let mut s = Settings::default();
    s.duration_seconds = 300;
    assert!(assemble(&s).is_err());
    let mut a = assemble(&Settings::default()).unwrap();
    a.pieces[0].path[1].position_cm[1] += 1;
    assert!(a.validate().is_err());
}

#[test]
fn pipe_portals_drop_from_raised_entry_and_onto_lower_departure() {
    for kind in ["cylinder", "banked_chicane"] {
        for seed in [7, 19] {
            let d = package::generate(&Settings {
                seed, circuit: false, duration_seconds: 60,
                gimmicks: vec![kind.into()], ..Settings::default()
            }).unwrap();
            let a = d.assembled_track.as_ref().unwrap();
            a.validate().unwrap();
            let mut portals = 0;
            for pieces in a.pieces.windows(3) {
                let [entry, pipe, exit] = pieces else { unreachable!() };
                if entry.id != "tube_entry" { continue; }
                assert!(pipe.id.starts_with("cylinder") || pipe.id == "banked_chicane");
                assert_eq!(exit.id, "tube_exit");
                let drop = (f64::from(pipe.width_cm) / 3.0).round() as i64;
                assert_eq!(entry.path.last().unwrap().position_cm[1] - pipe.path[0].position_cm[1], drop);
                assert_eq!(pipe.path.last().unwrap().position_cm[1] - exit.path[0].position_cm[1], drop);
                assert_eq!(entry.path[0].position_cm[1], exit.path.last().unwrap().position_cm[1]);
                let track = d.gimmicks.iter().find(|g| g.id == format!("track-{}-0", a.pieces.iter().position(|p| std::ptr::eq(p,pipe)).unwrap())).unwrap();
                assert_eq!(track.position[1] + track.track.as_ref().unwrap().centerline[0].floor_cm[1], pipe.path[0].position_cm[1]);
                portals += 1;
            }
            assert!(portals > 0);
            // A changed portal offset is rejected, not accepted as a loose gap.
            let mut altered = a.clone();
            let p = altered.pieces.iter_mut().find(|p| p.id == "tube_exit").unwrap();
            p.origin_cm[1] += 1;
            assert!(altered.validate().is_err());
        }
    }
}

#[test]
fn save_failure_never_replaces_original() {
    let path = std::env::temp_dir().join(format!(
        "mapkit-track-preserve-{}-{}.memap",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"original user artifact").unwrap();
    assert!(package::save(&Settings::default(), &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original user artifact");
    let mut a = assemble(&Settings::default()).unwrap();
    a.pieces.extend(a.pieces.clone());
    assert!(a.validate().is_err());
}

#[test]
fn required_gimmicks_seed_character_widths_and_chains() {
    let mut widths = [0usize; 3];
    let mut extremes = [1.0f64, 0.0f64];
    let mut turns = std::collections::BTreeSet::new();
    let mut heights = std::collections::BTreeSet::new();
    let mut layouts = std::collections::BTreeSet::new();
    for circuit in [true, false] {
        for seed in 0..24 {
            let s = Settings {
                seed,
                circuit,
                duration_seconds: 60,
                ..Settings::default()
            };
            let a = document(&s)
                .map(|d| d.assembled_track.unwrap())
                .unwrap_or_else(|e| panic!("seed {seed} circuit {circuit}: {e}"));
            for id in s.gimmicks.iter().filter(|id| id.as_str() != "obstacles") {
                assert!(
                    a.pieces
                        .iter()
                        .any(|p| p.id == *id || id == "cylinder" && p.id.starts_with("cylinder")),
                    "{seed}: {id}"
                );
            }
            for p in &a.pieces {
                assert!(p.chain_count <= 2);
                assert_eq!(p.origin_cm[0] % TILE_CM, 0);
                assert_eq!(p.origin_cm[2] % TILE_CM, 0);
                heights.insert(p.origin_cm[1]);
            }
            if seed < 4 {
                assert_eq!(a, assemble(&s).unwrap());
            }
            let plain = assemble(&Settings {
                gimmicks: vec![],
                duration_seconds: 120,
                ..s
            })
            .unwrap_or_else(|e| panic!("plain seed {seed} circuit {circuit}: {e}"));
            // Required-only layouts now contain mostly the 4m closure solver.
            // Measure width variety in the ordinary random-growth population.
            for p in &plain.pieces {
                if p.ordinary {
                    widths[match p.width_cm {
                        600 => 0,
                        400 => 1,
                        200 => 2,
                        _ => panic!(),
                    }] += 1;
                }
            }
            let direction_changes = plain
                .pieces
                .windows(2)
                .filter(|w| w[0].quarter_turns != w[1].quarter_turns)
                .count();
            turns.insert(direction_changes);
            let ratio = plain.ordinary_straight_cm as f64 / plain.ordinary_length_cm as f64;
            extremes[0] = extremes[0].min(ratio);
            extremes[1] = extremes[1].max(ratio);
            layouts.insert(format!(
                "{:?}",
                plain
                    .pieces
                    .iter()
                    .map(|p| (&p.id, p.origin_cm, p.width_cm))
                    .collect::<Vec<_>>()
            ));
        }
    }
    println!("widths={widths:?} measured_straight_extremes={extremes:?} turns={turns:?} heights_cm={heights:?}");
    assert!(turns.len() > 5 && heights.len() > 10);
    let total = widths.iter().sum::<usize>() as f64;
    for (i, expected) in [0.4, 0.4, 0.2].into_iter().enumerate() {
        assert!((widths[i] as f64 / total - expected).abs() < 0.17);
    }
    assert!(layouts.len() > 40);
}
#[test]
fn connected_bores_ramps_and_real_widths() {
    let mut bore_widths = std::collections::BTreeSet::new();
    let mut chain_lengths = std::collections::BTreeSet::new();
    for seed in 0..64 {
        let d = document(&Settings {
            seed,
            gimmicks: vec!["cylinder".into(), "sprint_lane".into()],
            ..Settings::default()
        })
        .unwrap_or_else(|e| panic!("tube seed {seed}: {e}"));
        let a = d.assembled_track.as_ref().unwrap();
        for (i, p) in a.pieces.iter().enumerate() {
            if p.id == "sprint_lane" {
                assert_eq!(
                    p.path
                        .last()
                        .unwrap()
                        .position_cm
                        .iter()
                        .zip(p.path[0].position_cm)
                        .map(|(a, b)| (*a - b).abs())
                        .sum::<i64>(),
                    1600
                );
                assert!(!d
                    .gimmicks
                    .iter()
                    .any(|g| g.id.starts_with(&format!("track-{i}-"))));
            }
            if p.id == "tube_entry" || p.id == "tube_exit" {
                let dy = (p.path.last().unwrap().position_cm[1] - p.path[0].position_cm[1]).abs();
                assert!((dy as f64 - f64::from(p.width_cm) / 3.0).abs() < 0.51);
                for w in p.path.windows(2) {
                    let dy = (w[0].position_cm[1] - w[1].position_cm[1]).abs() as f64;
                    let dx = (w[0].position_cm[0] - w[1].position_cm[0]) as f64;
                    let dz = (w[0].position_cm[2] - w[1].position_cm[2]) as f64;
                    assert!(dy / (dx * dx + dz * dz).sqrt() <= 0.12);
                }
            }
            if p.id.starts_with("cylinder") {
                bore_widths.insert(p.width_cm);
                chain_lengths.insert(p.chain_count);
                assert!(p.path.iter().all(|s| s.tube_radius_cm == p.width_cm / 2));
                let g = d
                    .gimmicks
                    .iter()
                    .find(|g| g.id == format!("track-{i}-0"))
                    .unwrap();
                let track = g.track.as_ref().unwrap();
                assert_eq!(track.radius_cm, p.width_cm / 2);
                assert_eq!(track.centerline[0].floor_cm[1], 0);
                if p.chain_index + 1 < p.chain_count {
                    let next = &a.pieces[i + 1];
                    assert_eq!(next.chain_id, p.chain_id);
                    assert_eq!(next.width_cm, p.width_cm);
                    assert_eq!(p.path.last().unwrap().position_cm, next.path[0].position_cm);
                    assert_eq!(p.path.last().unwrap().forward, next.path[0].forward);
                }
            }
            for pair in p.path.windows(2) {
                if pair[0].mode == "drift" && p.width_cm == 600 {
                    // 4m radius is larger than the widest half-lane.
                    assert!(pair[0].lateral_cm <= 300);
                }
            }
        }
    }
    assert_eq!(
        bore_widths,
        std::collections::BTreeSet::from([200, 400, 600])
    );
    assert!(chain_lengths.iter().all(|n| *n <= 2));
}
#[test]
fn venue_floor_is_collision_only_and_budgeted() {
    let d = document(&Settings {
        gimmicks: vec![],
        ..Settings::default()
    })
    .unwrap();
    let a = d.assembled_track.as_ref().unwrap();
    let generated = mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell { x: 0, y: 0 },
        heightgrid: None,
        max_triangles: 500_000,
    })
    .unwrap();
    let floor: Vec<_> = generated
        .triangles
        .iter()
        .filter(|f| f.object_id == "assembled-venue-floor")
        .collect();
    assert!(!floor.is_empty());
    assert!(floor
        .iter()
        .all(|f| !f.spawnable && f.vertices.iter().all(|v| v[1] == a.floor.min_cm[1])));
    let mut broken = a.clone();
    broken.pieces = vec![a.pieces[0].clone(); 513];
    assert!(broken.validate().is_err());
    assert!(mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell { x: 0, y: 0 },
        heightgrid: None,
        max_triangles: 1
    })
    .is_err());
}

#[test]
fn actual_chain_mesh_seams_and_wide_corner_clearance() {
    // Same-width bent sections share every quantized inner-face seam vertex.
    for width in [200, 400, 600] {
        let frames = vec![
            mapkit_core::special_track::TubeFrame {
                floor_cm: [0, 0, 0],
                normal: [0, 1_000_000, 0],
                forward: [0, 0, 1_000_000],
            },
            mapkit_core::special_track::TubeFrame {
                floor_cm: [0, 0, 100],
                normal: [0, 1_000_000, 0],
                forward: [0, 0, 1_000_000],
            },
        ];
        let track = mapkit_core::special_track::SpecialTrack {
            kind: mapkit_core::special_track::TrackKind::SweptCylinder,
            radius_cm: width / 2,
            width_cm: 400,
            length_cm: 1600,
            centerline: frames,
        };
        let mesh = track.mesh();
        for j in 0..128 {
            let a = mesh.inner[j * 2][1];
            let mut b = mesh.inner[j * 2][0];
            b[2] += 10000;
            assert_eq!(a, b, "open uniform bore seam");
        }
    }
    let c = catalogue();
    let pieces: Vec<Piece> = serde_json::from_value(c["pieces"].clone()).unwrap();
    let corner = pieces.iter().find(|p| p.id == "sharp_curve").unwrap();
    for p in &corner.path {
        if p.position_cm[2] < 400 || p.position_cm[0] > 400 {
            continue;
        }
        let radius =
            (((p.position_cm[0] - 400).pow(2) + (p.position_cm[2] - 400).pow(2)) as f64).sqrt();
        assert!(
            (radius - 400.0).abs() < 1.0 && radius - 300.0 > 99.0,
            "6m inner edge remains clear"
        );
    }
    assert_eq!(duration_options(true), &[(60, 3), (90, 2), (120, 2)]);
    assert_eq!(duration_options(false), &[(60, 1), (90, 1), (120, 1)]);
    for p in pieces.iter().filter(|p| p.id.starts_with("spiral")) {
        for w in p.path.windows(2) {
            let dy = (w[1].position_cm[1] - w[0].position_cm[1]).abs() as f64;
            let dx = (w[1].position_cm[0] - w[0].position_cm[0]) as f64;
            let dz = (w[1].position_cm[2] - w[0].position_cm[2]) as f64;
            assert!(dy / (dx * dx + dz * dz).sqrt() <= 0.23);
        }
    }
}

#[test]
fn finish_plaza_and_editable_free_roam_keep_exact_source_validation() {
    let settings = Settings {
        circuit: false,
        duration_seconds: 60,
        gimmicks: vec![],
        ..Settings::default()
    };
    let mut d = package::generate(&settings).unwrap();
    assert!(!d.free_roam);
    let a = d.assembled_track.as_ref().unwrap();
    let plaza = a.finish_plaza.as_ref().unwrap();
    assert_eq!(
        (plaza.entry_length_cm, plaza.radius_cm, plaza.wall_height_cm),
        (800, 800, 120)
    );
    assert_eq!(a.pieces.last().unwrap().id, "finish_plaza");
    assert_eq!(
        d.courses[0]
            .definition
            .checkpoints
            .last()
            .unwrap()
            .position_cm,
        plaza.checkpoint_cm
    );
    assert!(d.courses[0]
        .definition
        .checkpoints
        .iter()
        .all(|cp| cp.position_cm != plaza.center_cm));
    let p = &a.pieces[plaza.piece_index];
    assert!(
        p.path
            .iter()
            .any(|point| point.position_cm == plaza.checkpoint_cm),
        "finish must be an exact AI route sample"
    );
    assert_eq!(
        a.pieces[plaza.piece_index - 1]
            .path
            .last()
            .unwrap()
            .position_cm,
        p.path[0].position_cm
    );
    let prior_length: u64 = a.pieces[..plaza.piece_index]
        .iter()
        .map(|p| {
            p.path
                .windows(2)
                .map(|w| {
                    ((0..3)
                        .map(|i| ((w[1].position_cm[i] - w[0].position_cm[i]) as f64).powi(2))
                        .sum::<f64>()
                        .sqrt()
                        .round()) as u64
                })
                .sum::<u64>()
        })
        .sum();
    assert_eq!(a.length_cm, prior_length + 400);
    let center = plaza.center_cm;
    let chunk = mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell {
            x: (center[0] - d.bounds.min[0]).div_euclid(d.cell_size_cm as i64) as i32,
            y: (center[2] - d.bounds.min[1]).div_euclid(d.cell_size_cm as i64) as i32,
        },
        heightgrid: None,
        max_triangles: 500_000,
    })
    .unwrap();
    let road = format!("assembled-road-{}", plaza.piece_index);
    let floor: Vec<_> = chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == road)
        .collect();
    assert!(floor.len() >= 60);
    assert!(floor
        .iter()
        .all(|t| t.spawnable && t.vertices.iter().all(|v| v[1] == center[1])));
    let wall = format!("assembled-wall-{}", plaza.piece_index);
    assert!(chunk
        .triangles
        .iter()
        .any(|t| t.object_id == wall && t.vertices.iter().any(|v| v[1] == center[1] + 120)));
    let old = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    d.free_roam = true;
    package::reseal(&mut d).unwrap();
    let loaded = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    assert!(loaded.inspection.free_roam);
    assert_ne!(
        old.inspection.world_content_hash,
        loaded.inspection.world_content_hash
    );
    package::verify(
        &loaded.document,
        &loaded.inspection.world_content_hash,
        &loaded.document.courses[0],
    )
    .unwrap();
    assert!(package::verify(
        &loaded.document,
        &loaded.inspection.world_content_hash,
        &old.document.courses[0]
    )
    .is_err());
    let bytes = indexed::pack_source(d.clone(), BTreeMap::new(), 1).unwrap();
    let mut reader =
        indexed::IndexedReader::open(std::io::Cursor::new(bytes), 1024 * 1024 * 1024, None)
            .unwrap();
    assert!(reader.index().world.free_roam);
    reader
        .audit_summary(1024 * 1024 * 1024, &indexed::ReadEpoch::default().begin())
        .unwrap();
    d.seed += 1;
    assert!(package::reseal(&mut d).is_err());
    assert!(document(&Settings {
        gimmicks: vec![],
        ..Settings::default()
    })
    .unwrap()
    .assembled_track
    .unwrap()
    .finish_plaza
    .is_none());
}

#[test]
fn free_roam_is_required_boolean() {
    let d = package::generate(&Settings {
        gimmicks: vec![],
        ..Settings::default()
    })
    .unwrap();
    let mut value = serde_json::to_value(&d).unwrap();
    value.as_object_mut().unwrap().remove("free_roam");
    assert!(serde_json::from_value::<mapkit_core::MapDocument>(value.clone()).is_err());
    value["free_roam"] = serde_json::json!(1);
    assert!(serde_json::from_value::<mapkit_core::MapDocument>(value).is_err());
}

#[test]
fn attachments_preserve_roads_all_durations_and_reject_retired_requests() {
    for circuit in [true, false] {
        for (duration, laps) in duration_options(circuit) {
            for all in [false, true] {
                let mut s = Settings {
                    seed: 42,
                    circuit,
                    duration_seconds: *duration,
                    ..Settings::default()
                };
                s.gimmicks.retain(|id| all && id != "obstacles");
                let plain = assemble(&s).unwrap();
                assert_eq!(s.max_laps(), *laps);
                s.gimmicks.push("obstacles".into());
                let decorated = assemble(&s).unwrap();
                assert_eq!(
                    plain.pieces, decorated.pieces,
                    "mode={circuit} duration={duration} all={all}"
                );
                assert_eq!(plain.length_cm, decorated.length_cm);
                assert_eq!(plain.estimated_msec, decorated.estimated_msec);
                assert_eq!(plain.floor, decorated.floor);
                assert!(!decorated.obstacles.is_empty());
                assert!(decorated.obstacles.len() <= decorated.obstacle_target_count as usize);
                println!(
                    "TIME_SAMPLE {}",
                    serde_json::json!({"seed":42,"circuit":circuit,"seconds":duration,"selection":if all {"all"} else {"obstacles_only"},"estimated_msec":decorated.estimated_msec,"length_cm":decorated.length_cm,"pieces":decorated.pieces.len(),"obstacles":decorated.obstacles.len(),"target_obstacles":decorated.obstacle_target_count,"eligible_cm":decorated.obstacle_eligible_length_cm,"road_identical_when_disabled":true})
                );
                let mut bad = decorated.clone();
                bad.obstacles[0].station_cm += 1;
                assert!(bad.validate().is_err());
                let mut bad = decorated.clone();
                bad.obstacles[0].path = if bad.obstacles[0].path == "main" {
                    "alternate"
                } else {
                    "main"
                }
                .into();
                assert!(bad.validate().is_err());
                let mut bad = decorated.clone();
                bad.obstacles[0].avoid_lateral_cm += 1;
                assert!(bad.validate().is_err());
                let mut bad = decorated.clone();
                bad.obstacle_eligible_length_cm += 1;
                assert!(bad.validate().is_err());
                if decorated.estimated_msec > u32::from(*duration) * 1000 {
                    for id in s.gimmicks.iter().filter(|id| id.as_str() != "obstacles") {
                        assert_eq!(
                            decorated
                                .pieces
                                .iter()
                                .filter(|p| p.id == *id
                                    || id == "cylinder" && p.id.starts_with("cylinder"))
                                .count(),
                            1,
                            "no optional gimmicks remain on overrun"
                        );
                    }
                }
            }
        }
        for retired in [30, 180, 300] {
            assert_eq!(
                Settings {
                    circuit,
                    duration_seconds: retired,
                    ..Settings::default()
                }
                .normalized()
                .unwrap_err()
                .code,
                "E_TRACK_SETTINGS"
            );
        }
    }
    for id in [
        "fixed_obstacle",
        "moving_obstacle",
        "rotating_obstacle",
        "jump_barrier",
        "slalom_gates",
        "swing_gates",
        "piston_gates",
        "straight",
        "cylinder_curve",
    ] {
        assert!(!selection_ids().contains(&id));
        assert!(Settings {
            gimmicks: vec![id.into()],
            ..Settings::default()
        }
        .normalized()
        .is_err());
    }
    for id in [
        "fixed_obstacle",
        "moving_obstacle",
        "rotating_obstacle",
        "jump_barrier",
        "slalom_gates",
        "swing_gates",
        "piston_gates",
    ] {
        assert!(!catalogue_ids().contains(&id));
    }
}
