use mapkit_core::assembled_track::{self as track, authoring, templates, Settings};

#[test]
fn new_seed_presets_have_flat_connected_ports_and_safe_geometry() {
    for id in [
        "banked_hairpin",
        "banked_hairpin_left",
        "spatial_s",
        "spatial_s_left",
        "cylinder_spiral_right_up",
        "cylinder_spiral_left_up",
        "cylinder_spiral_right_down",
        "cylinder_spiral_left_down",
    ] {
        for &width in track::supported_widths(id) {
            let mut s = authoring::Source::empty();
            s.settings.circuit = false;
            let ids = if id.starts_with("cylinder") {
                vec!["straight", "tube_entry", id, "tube_exit", "straight"]
            } else {
                vec!["straight", id, "straight"]
            };
            for (at, preset) in ids.iter().enumerate() {
                let mut i = authoring::instance(&format!("p{at}"), preset, width);
                if let Some(last) = s.instances.last() {
                    i = authoring::snap(&i, last).unwrap();
                    s.connections.push(authoring::Connection {
                        from: last.id.clone(),
                        to: i.id.clone(),
                    });
                }
                s.instances.push(i);
            }
            s.paths.push(authoring::Path {
                id: "base".into(),
                pieces: s.instances.iter().map(|i| i.id.clone()).collect(),
            });
            let a = authoring::compile(&s).unwrap();
            assert!(
                a.geometry_issues.is_empty(),
                "{id}/{width}: {:?}",
                a.geometry_issues
            );
            for p in &a.pieces {
                for end in [&p.path[0], p.path.last().unwrap()] {
                    assert_eq!(end.normal, [0, 1_000_000, 0], "{id}/{width} normal");
                    assert_eq!(end.forward[1], 0, "{id}/{width} tangent");
                }
            }
            a.validate_stored().unwrap();
        }
    }
}

#[test]
fn compound_templates_expand_to_editable_safe_v1_graphs() {
    for id in ["double_jump", "air_ring_chain", "grind_shortcut"] {
        let s = templates::source(id).unwrap();
        let a = authoring::compile(&s).unwrap();
        authoring::executable(&a).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        a.validate_stored().unwrap();
        let mut grounded = s.clone();
        grounded.grounded_supports = true;
        authoring::executable(&authoring::compile(&grounded).unwrap()).unwrap();
        if id == "double_jump" {
            assert_eq!(s.instances.iter().filter(|i| i.preset == "jump").count(), 2);
        } else if id == "air_ring_chain" {
            assert_eq!(s.actions.iter().filter(|a| a.kind == "air_ring").count(), 3);
        } else {
            assert_eq!(s.paths.len(), 2);
            assert!(!s.grind_lines.is_empty() && !s.structures.is_empty());
            assert!(a.routes[1].estimated_msec < a.routes[0].estimated_msec);
            assert!(s
                .checkpoints
                .iter()
                .all(|c| s.paths.iter().all(|p| p.pieces.contains(&c.piece))));
        }
        let mut base = authoring::Source::empty();
        base.settings.circuit = false;
        let mut anchor = authoring::instance("anchor", "straight", 800);
        anchor.position_cm = [2000, 300, -5000];
        anchor.rotation_mdeg = [0, 45000, 0];
        base.instances.push(anchor);
        base.paths.push(authoring::Path {
            id: "base".into(),
            pieces: vec!["anchor".into()],
        });
        let appended = templates::append(&base, id, "anchor", "test").unwrap();
        let a = authoring::compile(&appended).unwrap();
        assert!(
            a.geometry_issues.is_empty(),
            "placed {id}: {:?}",
            a.geometry_issues
        );
        assert_eq!(authoring::from_assembly(&a).instances, appended.instances);
        if id == "grind_shortcut" {
            let endpoint = appended.paths[0].pieces.last().unwrap();
            let extended = templates::append(&appended, "double_jump", endpoint, "next").unwrap();
            assert!(extended
                .paths
                .iter()
                .all(|p| p.pieces.last() == extended.paths[0].pieces.last()));
            assert!(authoring::compile(&extended)
                .unwrap()
                .geometry_issues
                .is_empty());
        }
    }
}

#[test]
fn seeded_compound_modules_reach_completed_routes() {
    let mut found = [false; 3];
    for seed in [26, 50, 114] {
        let a = track::assemble(&Settings {
            seed,
            circuit: false,
            ..Settings::default()
        })
        .unwrap();
        let s = a.seed_source.as_ref().unwrap();
        for (start, window) in s.instances.windows(5).enumerate() {
            if window.iter().map(|i| i.preset.as_str()).eq([
                "free_curve",
                "jump",
                "free_curve",
                "jump",
                "free_curve",
            ]) {
                found[0] = true;
                assert!(
                    !a.obstacles
                        .iter()
                        .any(|o| (start..start + 5).contains(&o.piece_index)),
                    "double jump recovery stays clear"
                );
            }
        }
        let rings: Vec<_> = s.actions.iter().filter(|a| a.kind == "air_ring").collect();
        if rings.len() >= 3 {
            found[1] = true;
            assert!(
                rings.iter().all(|r| s.paths[0].pieces.contains(&r.piece)),
                "ring chain is on the base route"
            );
            for ring in rings {
                let at = s.instances.iter().position(|i| i.id == ring.piece).unwrap();
                assert!(
                    !a.obstacles
                        .iter()
                        .any(|o| (at - 1..=at + 1).contains(&o.piece_index)),
                    "ring launch and landing stay clear"
                );
            }
        }
        if s.grind_lines.iter().any(|l| l.id.ends_with("-rail")) {
            found[2] = true;
            assert!(
                s.paths.len() > 1
                    && !s.paths[0]
                        .pieces
                        .iter()
                        .any(|id| id.ends_with("upper-shortcut")),
                "grind remains optional"
            );
            let checkpoints = authoring::effective_checkpoints(&a);
            assert!(
                checkpoints
                    .iter()
                    .all(|(piece, _)| a.routes.iter().all(|r| r.pieces.contains(piece))),
                "branch checkpoints remain shared"
            );
        }
    }
    assert_eq!(
        found, [true; 3],
        "all composed modules must survive completed-course selection"
    );
}

#[test]
fn ring_template_rejects_an_overhead_crossing() {
    let mut base = authoring::Source::empty();
    base.settings.circuit = false;
    base.instances
        .push(authoring::instance("anchor", "straight", 800));
    let mut crossing = authoring::instance("crossing", "straight", 800);
    crossing.position_cm = [-400, 600, 4000];
    crossing.rotation_mdeg = [0, 90000, 0];
    base.instances.push(crossing);
    base.paths.push(authoring::Path {
        id: "base".into(),
        pieces: vec!["anchor".into()],
    });
    assert_eq!(
        templates::append(&base, "air_ring_chain", "anchor", "ring")
            .unwrap_err()
            .code,
        "E_TRACK_CLEARANCE"
    );
}

#[test]
fn generation_ignores_preserved_difficulty_and_seeds_environment() {
    let settings = Settings {
        seed: 17,
        circuit: false,
        categories: vec!["driving".into()],
        ..Settings::default()
    };
    let expected = track::assemble(&settings).unwrap();
    for difficulty in ["easy", "hard"] {
        let mut other = settings.clone();
        other.difficulty = difficulty.into();
        other.time_minutes = 137;
        assert_eq!(track::assemble(&other).unwrap(), expected);
    }
    assert_eq!(expected.settings.difficulty, "normal");
    assert_eq!(
        serde_json::json!(expected.settings.time_minutes),
        track::track_seed_defaults(settings.seed).unwrap()["time_minutes"]
    );
    let mut stored = expected.clone();
    stored.settings.difficulty = "hard".into();
    stored.validate_stored().unwrap(); // old v1 preservation does not recompile
}

#[test]
fn representative_seed_courses_exhibit_distinct_shapes() {
    let mut measures = vec![];
    for seed in [1, 4, 7, 15, 17, 31, 42, 73, 101, 211] {
        let settings = Settings {
            seed,
            circuit: false,
            ..Settings::default()
        };
        let a = track::assemble(&settings).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let target = u32::from(settings.duration_seconds) * 1000;
        assert!(a.estimated_msec.abs_diff(target) <= target / 10);
        let points: Vec<_> = a.pieces.iter().flat_map(|p| &p.path).collect();
        let height = points.iter().map(|s| s.position_cm[1]).max().unwrap()
            - points.iter().map(|s| s.position_cm[1]).min().unwrap();
        let turns: f64 = a
            .pieces
            .iter()
            .flat_map(|p| p.path.windows(2))
            .map(|w| {
                let (a, b) = (w[0].forward, w[1].forward);
                (a[0] as f64 * b[2] as f64 - a[2] as f64 * b[0] as f64)
                    .atan2(a[0] as f64 * b[0] as f64 + a[2] as f64 * b[2] as f64)
                    .abs()
            })
            .sum();
        let straight = a
            .pieces
            .iter()
            .filter(|p| p.id.starts_with("straight") || p.id == "sprint_lane")
            .map(|p| u64::from(p.reference_msec))
            .sum::<u64>()
            * 100
            / u64::from(a.estimated_msec);
        let specials = a
            .pieces
            .iter()
            .filter(|p| track::category(&p.id) != "driving")
            .count();
        println!("seed {seed}: straight={straight}% turns={turns:.2}rad height={height}cm special={specials} time={}ms",a.estimated_msec);
        measures.push((straight, turns, height, specials));
    }
    assert!(
        measures
            .iter()
            .any(|m| m.0 >= 65 && m.1 < 10.0 && m.2 == 0 && m.3 == 0),
        "a quiet completed course must survive candidate selection: {measures:?}"
    );
    assert!(
        measures.iter().map(|m| m.0).max().unwrap() - measures.iter().map(|m| m.0).min().unwrap()
            >= 15
    );
    assert!(
        measures.iter().map(|m| m.2).max().unwrap() - measures.iter().map(|m| m.2).min().unwrap()
            >= 500
    );
    assert!(
        measures.iter().map(|m| m.3).max().unwrap() > measures.iter().map(|m| m.3).min().unwrap()
    );
}

#[test]
fn category_masks_shapes_and_duration_options_remain_bounded() {
    for (circuit, seconds, categories, seed) in [
        (false, 60, vec!["action"], 17),
        (false, 90, vec!["gimmick"], 42),
        (false, 120, vec!["driving"], 15),
        (true, 60, vec!["driving", "gimmick", "action"], 4),
        (true, 90, vec!["driving"], 15),
        (true, 120, vec!["driving", "gimmick", "action"], 31),
    ] {
        let s = Settings {
            seed,
            circuit,
            duration_seconds: seconds,
            categories: categories.iter().map(|s| (*s).into()).collect(),
            ..Settings::default()
        };
        let a = track::assemble(&s)
            .unwrap_or_else(|e| panic!("{circuit}/{seconds}/{categories:?}: {e:?}"));
        assert!(a.estimated_msec.abs_diff(u32::from(seconds) * 1000) <= u32::from(seconds) * 100);
        assert!(a
            .pieces
            .iter()
            .all(|p| track::category(&p.id) == "driving"
                || categories.contains(&track::category(&p.id))));
        if !categories.contains(&"gimmick") {
            assert!(a.obstacles.is_empty());
        }
        a.validate_stored().unwrap();
    }
}
