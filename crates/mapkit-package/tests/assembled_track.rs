use mapkit_core::assembled_track::*;
use mapkit_package::{assembled_track as package, *};
use std::collections::BTreeMap;

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
fn durations_closure_and_no_five_repeats() {
    for circuit in [false, true] {
        for &(duration_seconds, _) in duration_options(circuit) {
            for seed in [0, 1, 42, 9007199254740991] {
                for gimmicks in [vec![], vec!["loop".into()], Settings::default().gimmicks] {
                    let settings = Settings {
                        seed,
                        duration_seconds,
                        circuit,
                        gimmicks,
                        ..Settings::default()
                    };
                    let a = assemble(&settings).unwrap_or_else(|e| panic!("{settings:?}: {e}"));
                    a.validate().unwrap();
                    for p in &a.pieces {
                        assert!(
                            p.origin_cm.iter().all(|v| v % TILE_CM == 0),
                            "cube-grid origin"
                        );
                    }
                    let target = u32::from(duration_seconds) * 1000;
                    assert!(a.estimated_msec.abs_diff(target) < 45000);
                    for w in a.pieces.windows(5) {
                        assert!(w.iter().any(|p| p.id != w[0].id));
                    }
                }
            }
        }
    }
}
#[test]
fn every_piece_geometry_and_background() {
    for id in catalogue_ids()
        .iter()
        .filter(|v| !["straight", "curve", "curve_left"].contains(v))
    {
        let settings = Settings {
            gimmicks: vec![(*id).into()],
            ..Settings::default()
        };
        let d = package::generate(&settings).unwrap_or_else(|e| panic!("{id}: {e}"));
        let p = read_bytes(&pack_bytes(d, BTreeMap::new()).unwrap()).unwrap();
        let mut seen = false;
        for cell in p.document.cells() {
            let generated = p.generate_with_occupancy(cell, 500_000, 100_000).unwrap();
            assert!(generated
                .chunk
                .triangles
                .iter()
                .all(|t| t.object_id != "terrain"));
            seen |= !generated.chunk.triangles.is_empty();
        }
        assert!(seen);
    }
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
fn grid_ports_grades_difficulty_and_hollow_ring() {
    for difficulty in ["easy", "normal", "hard"] {
        for id in catalogue_ids() {
            document(&Settings {
                duration_seconds: 120,
                difficulty: difficulty.into(),
                gimmicks: vec![(*id).into()],
                ..Settings::default()
            })
            .unwrap_or_else(|e| panic!("{difficulty} {id}: {e}"));
        }
    }
    let catalogue = catalogue();
    for value in catalogue["pieces"].as_array().unwrap() {
        let p: Piece = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(p.path[0].forward, [0, 0, 1_000_000]);
        assert_eq!(
            p.path.last().unwrap().forward,
            if p.id.starts_with("hairpin") {
                [0, 0, -1_000_000]
            } else if p.id == "curve" || p.id == "sharp_curve" {
                [1_000_000, 0, 0]
            } else if p.id == "curve_left" || p.id == "sharp_curve_left" {
                [-1_000_000, 0, 0]
            } else {
                [0, 0, 1_000_000]
            }
        );
        for axis in 0..3 {
            let extent = p.path.iter().map(|s| s.position_cm[axis]).max().unwrap()
                - p.path.iter().map(|s| s.position_cm[axis]).min().unwrap();
            assert!(
                extent <= i64::from(p.cube_span) * TILE_CM,
                "{} cube envelope",
                p.id
            );
        }
        if p.id == "slope" || p.id.starts_with("spiral") {
            for pair in p.path.windows(2) {
                let dy = (pair[1].position_cm[1] - pair[0].position_cm[1]).abs() as f64;
                let dx = (pair[1].position_cm[0] - pair[0].position_cm[0]) as f64;
                let dz = (pair[1].position_cm[2] - pair[0].position_cm[2]) as f64;
                assert!(
                    dy / (dx * dx + dz * dz).sqrt() <= 0.23,
                    "{} drivable grade",
                    p.id
                );
            }
        }
    }
    let s = Settings {
        gimmicks: vec!["cylinder".into()],
        ..Settings::default()
    };
    let density = |difficulty: &str| {
        (0..16)
            .map(|seed| {
                assemble(&Settings {
                    seed,
                    difficulty: difficulty.into(),
                    duration_seconds: 120,
                    ..s.clone()
                })
                .unwrap()
                .pieces
                .iter()
                .filter(|p| p.id == "cylinder")
                .count()
            })
            .sum::<usize>()
    };
    assert!(density("hard") > density("easy"));
    let doc = document(&Settings {
        gimmicks: vec!["air_ring".into()],
        duration_seconds: 120,
        ..Settings::default()
    })
    .unwrap();
    let ring = doc
        .gimmicks
        .iter()
        .find(|g| g.motion.kind == mapkit_core::gimmick::MotionKind::AirRing)
        .unwrap();
    assert_eq!(ring.parts.len(), 4);
    for part in &ring.parts {
        assert!(
            (0..2).any(|axis| part.vertices.iter().all(|v| v[axis] >= 250)
                || part.vertices.iter().all(|v| v[axis] <= -250)),
            "aperture stays open"
        );
    }
}

#[test]
fn circuits_have_seeded_bays_both_turns_and_bounded_straights() {
    let mut outlines = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let a = assemble(&Settings {
            seed,
            duration_seconds: 120,
            gimmicks: vec![],
            ..Settings::default()
        })
        .unwrap();
        let mut corners = vec![];
        let mut run_start = a.pieces[0].origin_cm;
        let mut left = 0;
        for p in &a.pieces {
            if p.id.contains("curve") || p.id.starts_with("hairpin") {
                left += usize::from(p.id.ends_with("_left"));
                let run =
                    (p.origin_cm[0] - run_start[0]).abs() + (p.origin_cm[2] - run_start[2]).abs();
                assert!(run <= 4800, "seed {seed}: long unbroken heading {run}");
                corners.push((p.origin_cm, p.quarter_turns, p.id.clone()));
                run_start = p.path.last().unwrap().position_cm;
            }
        }
        assert!(
            left >= 1 && corners.len() >= 6,
            "seed {seed}: rectangular outline"
        );
        outlines.insert(corners);
        // Ports meet in position, tangent and normal, including the circuit seam.
        let start = &a.pieces[0].path[0];
        let end = a.pieces.last().unwrap().path.last().unwrap();
        assert_eq!(
            (start.position_cm, start.forward, start.normal),
            (end.position_cm, end.forward, end.normal)
        );
        assert_eq!(a, assemble(&a.settings).unwrap());
    }
    assert!(
        outlines.len() >= 28,
        "seeds must change the course outline, not just its contents"
    );
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
fn short_lap_rules_tight_geometry_and_smooth_spiral() {
    assert_eq!(duration_options(true), &[(30, 3), (60, 3), (120, 2)]);
    assert_eq!(duration_options(false), &[(60, 1), (120, 1), (180, 1)]);
    for circuit in [false, true] {
        for &(duration_seconds, laps) in duration_options(circuit) {
            let s = Settings {
                circuit,
                duration_seconds,
                ..Settings::default()
            };
            assert_eq!(s.max_laps(), laps);
            let a = assemble(&s).unwrap();
            assert!(
                a.estimated_msec
                    .abs_diff(u32::from(duration_seconds) * 1000)
                    < 20000
            );
            assert!(a
                .pieces
                .iter()
                .any(|p| p.id.starts_with("sharp_curve") || p.id.starts_with("hairpin")));
        }
    }
    let c = catalogue();
    let pieces: Vec<Piece> = serde_json::from_value(c["pieces"].clone()).unwrap();
    let zig = pieces.iter().find(|p| p.id == "zigzag").unwrap();
    let swing = zig.path.iter().map(|p| p.position_cm[0]).max().unwrap()
        - zig.path.iter().map(|p| p.position_cm[0]).min().unwrap();
    assert!(
        swing > WIDTH,
        "a straight line must not fit through the chicane"
    );
    let tube = pieces.iter().find(|p| p.id == "cylinder").unwrap();
    assert!(tube
        .path
        .iter()
        .filter(|p| p.mode == "cylinder")
        .all(|p| p.tube_radius_cm == 200));
    for id in ["spiral_up", "spiral_down"] {
        let p = pieces.iter().find(|p| p.id == id).unwrap();
        let arc: Vec<_> = p.path.iter().filter(|s| s.mode == "spiral").collect();
        assert_eq!(arc.len(), 192);
        assert!(
            (arc[1].position_cm[1] - arc[0].position_cm[1]).abs() <= 1,
            "smooth grade entry"
        );
        assert!(
            (arc[191].position_cm[1] - arc[190].position_cm[1]).abs() <= 1,
            "smooth grade exit"
        );
    }
}
