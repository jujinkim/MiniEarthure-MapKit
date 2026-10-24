use mapkit_core::assembled_track::{self as track, authoring, templates, Assembly, GenerationStats, Length, Settings};
use std::sync::atomic::{AtomicUsize, Ordering};

fn settings(seed: u64, circuit: bool, length: Length) -> Settings {
    Settings { seed, circuit, length, ..Settings::default() }
}
/// Generate in parallel; results keep the input order.
fn generate(all: &[Settings]) -> Vec<mapkit_core::Result<(Assembly, GenerationStats)>> {
    let next = AtomicUsize::new(0);
    let mut out: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8).map(|_| scope.spawn(|| {
            let mut done = vec![];
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(s) = all.get(i) else { break };
                done.push((i, track::assemble_with_stats(s)));
            }
            done
        })).collect();
        workers.into_iter().flat_map(|w| w.join().unwrap()).collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}
fn base(a: &Assembly) -> Vec<&track::Piece> { a.routes[0].pieces.iter().map(|&i| &a.pieces[i]).collect() }
fn length(p: &track::Piece) -> u64 {
    p.path.windows(2).map(|w| (0..3).map(|j| ((w[1].position_cm[j] - w[0].position_cm[j]) as f64).powi(2)).sum::<f64>().sqrt().round() as u64).sum()
}
/// Signed horizontal turn of a piece in degrees, right positive, with its left/right parts.
fn turn(p: &track::Piece) -> (f64, f64, f64) {
    let (mut left, mut right) = (0.0, 0.0);
    for w in p.path.windows(2) {
        let (a, b) = (w[0].forward.map(|v| v as f64), w[1].forward.map(|v| v as f64));
        let t = (a[2] * b[0] - a[0] * b[2]).atan2(a[0] * b[0] + a[2] * b[2]).to_degrees();
        if t > 0.0 { right += t } else { left -= t }
    }
    (right - left, left, right)
}
/// Independent straight oracle: optional driving road after the runway; every
/// special piece, flight, portal and the finish resets the run.
fn longest_straight(a: &Assembly) -> u64 {
    let (mut run, mut longest) = (0, 0);
    for (i, p) in base(a).into_iter().enumerate() {
        if i < 3 || track::category(&p.id) != "driving" || ["tube_entry", "tube_exit", "finish_plaza"].contains(&p.id.as_str())
            || p.path.iter().any(|s| s.mode == "flight") { run = 0; continue; }
        for w in p.path.windows(2) {
            let (f, g) = (w[0].forward, w[1].forward);
            if ((f[0] as f64 * g[2] as f64 - f[2] as f64 * g[0] as f64) / 1e12).abs() < 0.005 {
                run += (0..3).map(|j| ((w[1].position_cm[j] - w[0].position_cm[j]) as f64).powi(2)).sum::<f64>().sqrt().round() as u64;
                longest = longest.max(run);
            } else { run = 0; }
        }
    }
    longest
}
fn hairpin(id: &str) -> bool { id.starts_with("hairpin") || id.starts_with("banked_hairpin") }
/// Two consecutive hairpins of opposite hand joined only by ≥8 m of straight or grade.
fn alternating_hairpins(a: &Assembly) -> bool {
    let pieces = base(a);
    let pins: Vec<_> = (0..pieces.len()).filter(|&i| hairpin(&pieces[i].id)).collect();
    pins.windows(2).any(|w| {
        let between = &pieces[w[0] + 1..w[1]];
        pieces[w[0]].id.ends_with("_left") != pieces[w[1]].id.ends_with("_left")
            && between.iter().all(|p| ["straight", "sprint_lane", "slope_up", "slope_down"].contains(&p.id.as_str()))
            && (pieces[w[0]].id.starts_with("banked") || between.iter().map(|p| length(p)).sum::<u64>() >= 790)
    })
}
/// A chicane/zigzag/S preset, or opposite turns of ≥40° with at most one straight between.
fn s_or_chicane(a: &Assembly) -> bool {
    let pieces = base(a);
    pieces.iter().any(|p| ["chicane", "zigzag", "spatial_s", "spatial_s_left"].contains(&p.id.as_str()))
        || (0..pieces.len()).any(|i| {
            let t = turn(pieces[i]).0;
            pieces[i + 1..].iter().take(2).find(|p| turn(p).0.abs() > 1.0).is_some_and(|q| {
                let u = turn(q).0;
                t.abs() >= 40.0 && u.abs() >= 40.0 && t.signum() != u.signum()
            })
        })
}
fn jump(id: &str) -> bool { ["jump", "offset_jump", "air_ring", "jump_panel"].contains(&id) }
fn boost(id: &str) -> bool { ["acceleration_panel", "boost_chain"].contains(&id) }
/// Sprint size: everything except the runway, the finish and pipe portals.
fn counted(a: &Assembly) -> usize {
    base(a).iter().skip(3).filter(|p| !["finish_plaza", "tube_entry", "tube_exit"].contains(&p.id.as_str())).count()
}
const CONNECTORS: &[&str] = &["straight", "sprint_lane", "finish_plaza", "tube_entry", "tube_exit", "free_curve", "flight_curve", "right90", "right90_left", "gentle45",
    "gentle45_left", "slope_up", "slope_down", "spiral90_right_up", "spiral90_left_up", "spiral90_right_down", "spiral90_left_down"];
/// Rules every generated track keeps, whatever its category mask.
fn assert_rules(a: &Assembly, label: &str) {
    let pieces = base(a);
    assert!(pieces.iter().all(|p| p.id != "jump_panel"), "{label}: jump_panel piece");
    for i in 0..pieces.len() {
        let next = if i + 1 < pieces.len() { Some(pieces[i + 1]) } else if a.settings.circuit { Some(pieces[0]) } else { None };
        assert!(!(boost(&pieces[i].id) && next.is_some_and(|n| jump(&n.id))), "{label}: boost directly before a jump at {i}");
        let after = if i + 2 < pieces.len() || a.settings.circuit { Some(pieces[(i + 2) % pieces.len()]) } else { None };
        assert!(!(boost(&pieces[i].id) && after.is_some_and(|n| n.id == "flight_curve")), "{label}: boost directly before a drop takeoff at {i}");
    }
    assert!(a.obstacles.iter().all(|o| !o.kind.starts_with("ramp_")), "{label}: automatic ramp");
    let source = a.seed_source.as_ref().unwrap();
    assert!(source.jump_shortcuts.len() <= 2 && source.grind_lines.iter().filter(|l| l.id.ends_with("-rail")).count() <= 2, "{label}: shortcuts");
    for p in a.pieces.iter().filter(|p| jump(&p.id) || p.id == "flight_curve") {
        assert!(p.path.iter().any(|s| s.mode == "flight"), "{label}: {} without flight samples", p.id);
    }
    for action in source.actions.iter().filter(|a| a.kind == "jump_panel") {
        let landing = action.landing.as_ref().unwrap_or_else(|| panic!("{label}: jump panel without landing"));
        let at = |id: &str| a.pieces[source.instances.iter().position(|i| i.id == id).unwrap()].path.clone();
        assert!(at(&action.piece)[action.sample].position_cm[1] > at(&landing.piece)[landing.sample].position_cm[1], "{label}: drop lands higher");
    }
    assert!(longest_straight(a) <= 3200, "{label}: straight {}", longest_straight(a));
    for p in &a.pieces {
        assert!(track::category(&p.id) == "driving" && (a.settings.categories.iter().any(|c| c == "driving") || CONNECTORS.contains(&p.id.as_str()))
            || a.settings.categories.iter().any(|c| c == track::category(&p.id)), "{label}: {} outside the category mask", p.id);
    }
    if !a.settings.categories.iter().any(|c| c == "gimmick") { assert!(a.obstacles.is_empty(), "{label}: obstacles without gimmicks"); }
    let narrow = track::generation_summary(a).narrow_road;
    assert!(narrow.sections <= 1 && narrow.length_cm <= 4000 && narrow.length_cm <= a.length_cm / 20, "{label}: narrow {narrow:?}");
    assert!(a.pieces.len() <= 512 && a.pieces.iter().map(|p| p.path.len() + p.alternate_path.len()).sum::<usize>() <= 32000);
    if a.settings.circuit {
        let (end, start) = (pieces.last().unwrap().path.last().unwrap(), &pieces[0].path[0]);
        assert_eq!(end.position_cm, start.position_cm, "{label}: open circuit");
    } else {
        assert_eq!(counted(a), a.settings.length.tier().sprint_pieces, "{label}: sprint size");
        assert_eq!(pieces.last().unwrap().id, "finish_plaza");
    }
}

#[test]
fn generation_is_deterministic_and_recompiles_from_its_seed_source() {
    let all: Vec<_> = Length::ALL.iter().flat_map(|&l| [true, false].into_iter().flat_map(move |c| (0..8).map(move |s| settings(s, c, l)))).collect();
    let first = generate(&all);
    let second = generate(&all);
    for ((s, a), b) in all.iter().zip(&first).zip(&second) {
        let label = format!("{s:?}");
        let (a, b) = (&a.as_ref().unwrap_or_else(|e| panic!("{label}: {e:?}")).0, &b.as_ref().unwrap().0);
        assert_eq!(a, b, "{label}");
        assert_eq!(a.settings.difficulty, "normal");
        assert!(a.authoring.is_none() && a.issues.is_empty());
        let source = a.seed_source.clone().unwrap();
        // A fresh thread has an empty compile cache.
        let mut compiled = std::thread::spawn(move || authoring::compile(&source).map(|c| (c, source))).join().unwrap().unwrap();
        compiled.0.authoring = None;
        compiled.0.seed_source = Some(compiled.1);
        assert_eq!(&compiled.0, a, "{label}: compile(seed_source)");
        a.validate().unwrap();
        assert_rules(a, &label);
        if !s.circuit { assert_eq!(counted(a), [30, 45, 60][s.length as usize], "{label}"); }
    }
    let document = track::document_from_assembly(first[0].as_ref().unwrap().0.clone()).unwrap();
    track::verify_document(&document).unwrap();
}

#[test]
fn circuits_close_for_every_seed_and_tracks_vary() {
    let all: Vec<_> = Length::ALL.iter().flat_map(|&l| (0..16).map(move |s| settings(s, true, l))).collect();
    let results = generate(&all);
    let sprints: Vec<_> = Length::ALL.iter().flat_map(|&l| (0..16).map(move |s| settings(s, false, l))).collect();
    let sprint_results = generate(&sprints);
    let mut families = std::collections::BTreeMap::<String, usize>::new();
    let (mut chains, mut esses, mut tracks) = (0, 0, 0);
    for (tier, l) in Length::ALL.iter().enumerate() {
        let mut lengths = vec![];
        let (mut attempts, mut searches, mut nodes, mut pops) = (vec![0; 5], 0, 0, 0);
        for (s, r) in all.iter().zip(&results).chain(sprints.iter().zip(&sprint_results)).filter(|(s, _)| s.length == *l) {
            let label = format!("{s:?}");
            let (a, stats) = r.as_ref().unwrap_or_else(|e| panic!("{label}: {e:?}"));
            assert_rules(a, &label);
            let (_, left, right) = base(a).iter().map(|p| turn(p)).fold((0.0, 0.0, 0.0), |x, y| (x.0 + y.0, x.1 + y.1, x.2 + y.2));
            let share = left / (left + right);
            assert!((0.25..=0.75).contains(&share), "{label}: left share {share:.2}");
            tracks += 1;
            chains += usize::from(alternating_hairpins(a));
            esses += usize::from(s_or_chicane(a));
            for p in base(a) { *families.entry(p.id.trim_end_matches("_left").into()).or_default() += 1; }
            if s.circuit {
                lengths.push(a.length_cm);
                attempts[stats.attempts as usize] += 1;
                (searches, nodes, pops) = (searches + stats.searches, nodes + stats.nodes, pops + stats.pops);
            }
        }
        let mean = lengths.iter().sum::<u64>() / lengths.len() as u64;
        println!("{l:?} circuit: mean {:.1} m, min {:.1} m, max {:.1} m; attempts {attempts:?}, A* searches {searches}, nodes {nodes}, pops {pops}",
            mean as f64 / 100.0, *lengths.iter().min().unwrap() as f64 / 100.0, *lengths.iter().max().unwrap() as f64 / 100.0);
        let sprint: Vec<_> = sprints.iter().zip(&sprint_results).filter(|(s, _)| s.length == *l).map(|(_, r)| r.as_ref().unwrap().0.length_cm).collect();
        println!("{l:?} sprint: mean {:.1} m, min {:.1} m, max {:.1} m", sprint.iter().sum::<u64>() as f64 / 1600.0,
            *sprint.iter().min().unwrap() as f64 / 100.0, *sprint.iter().max().unwrap() as f64 / 100.0);
        if tier > 0 {
            let previous: u64 = all.iter().zip(&results).filter(|(s, _)| s.length == Length::ALL[tier - 1]).map(|(_, r)| r.as_ref().unwrap().0.length_cm).sum();
            assert!(previous < lengths.iter().sum::<u64>(), "{l:?} circuits are not longer than the previous tier");
        }
    }
    println!("pieces: {families:?}");
    println!("alternating hairpin chains {chains}/{tracks}, S/chicane {esses}/{tracks}");
    assert!(chains * 100 >= tracks * 30, "hairpin chains {chains}/{tracks}");
    assert!(esses * 100 >= tracks * 50, "S/chicane {esses}/{tracks}");
    let distinct: std::collections::BTreeSet<_> = results.iter().map(|r| r.as_ref().unwrap().0.length_cm).collect();
    assert!(distinct.len() > 40, "shapes repeat: {}", distinct.len());
}

#[test]
fn category_masks_are_respected() {
    let mut all = vec![];
    for mask in 1..8u32 {
        for (circuit, length, seed) in [(true, Length::Short, 4u64), (false, Length::Medium, 42), (true, Length::Long, 7 + u64::from(mask)), (false, Length::Short, 17)] {
            let categories = track::selection_ids().iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, s)| s.to_string()).collect();
            all.push(Settings { seed, circuit, length, categories, ..Settings::default() });
        }
    }
    for (s, r) in all.iter().zip(generate(&all)) {
        let label = format!("{s:?}");
        let (a, _) = r.unwrap_or_else(|e| panic!("{label}: {e:?}"));
        assert_rules(&a, &label);
        let special = a.pieces.iter().filter(|p| track::category(&p.id) != "driving").count();
        if s.categories.iter().any(|c| c != "driving") { assert!(special > 0, "{label}: no special piece"); }
        a.validate_stored().unwrap();
    }
    let mut empty = Settings::default();
    empty.categories.clear();
    assert!(track::assemble(&empty).is_err());
}

#[test]
fn generation_ignores_preserved_difficulty_and_seeds_environment() {
    let settings = Settings { seed: 17, circuit: false, categories: vec!["driving".into()], ..Settings::default() };
    let expected = track::assemble(&settings).unwrap();
    for difficulty in ["easy", "hard"] {
        let mut other = settings.clone();
        other.difficulty = difficulty.into();
        other.time_minutes = 137;
        assert_eq!(track::assemble(&other).unwrap(), expected);
    }
    assert_eq!(expected.settings.difficulty, "normal");
    assert_eq!(serde_json::json!(expected.settings.time_minutes), track::track_seed_defaults(settings.seed).unwrap()["time_minutes"]);
    let mut stored = expected.clone();
    stored.settings.difficulty = "hard".into();
    stored.validate_stored().unwrap(); // old v1 preservation does not recompile
}

#[test]
fn cancelled_track_search_does_not_publish_or_cancel_replacement() {
    let token = mapkit_core::cancellation::CancellationToken::default();
    token.cancel();
    assert_eq!(token.run(|| track::assemble(&Settings::default())).unwrap_err().code, "E_CANCELLED");
    let replacement = mapkit_core::cancellation::CancellationToken::default();
    assert!(!replacement.is_cancelled());
    replacement.run(mapkit_core::cancellation::checkpoint).unwrap();
}

#[test]
fn new_seed_presets_have_flat_connected_ports_and_safe_geometry() {
    for id in ["banked_hairpin", "banked_hairpin_left", "spatial_s", "spatial_s_left", "cylinder_spiral_right_up", "cylinder_spiral_left_up",
        "cylinder_spiral_right_down", "cylinder_spiral_left_down"] {
        for &width in track::supported_widths(id) {
            let mut s = authoring::Source::empty();
            s.settings.circuit = false;
            let ids = if id.starts_with("cylinder") { vec!["straight", "tube_entry", id, "tube_exit", "straight"] } else { vec!["straight", id, "straight"] };
            for (at, preset) in ids.iter().enumerate() {
                let mut i = authoring::instance(&format!("p{at}"), preset, width);
                if let Some(last) = s.instances.last() {
                    i = authoring::snap(&i, last).unwrap();
                    s.connections.push(authoring::Connection { from: last.id.clone(), to: i.id.clone() });
                }
                s.instances.push(i);
            }
            s.paths.push(authoring::Path { id: "base".into(), pieces: s.instances.iter().map(|i| i.id.clone()).collect() });
            let a = authoring::compile(&s).unwrap();
            assert!(a.geometry_issues.is_empty(), "{id}/{width}: {:?}", a.geometry_issues);
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
        let mut base = authoring::Source::empty();
        base.settings.circuit = false;
        let mut anchor = authoring::instance("anchor", "straight", 800);
        anchor.position_cm = [2000, 300, -5000];
        anchor.rotation_mdeg = [0, 45000, 0];
        base.instances.push(anchor);
        base.paths.push(authoring::Path { id: "base".into(), pieces: vec!["anchor".into()] });
        let appended = templates::append(&base, id, "anchor", "test").unwrap();
        let a = authoring::compile(&appended).unwrap();
        assert!(a.geometry_issues.is_empty(), "placed {id}: {:?}", a.geometry_issues);
        assert_eq!(authoring::from_assembly(&a).instances, appended.instances);
    }
}

#[test]
fn ring_and_grind_templates_reject_crossings() {
    for (id, position, code) in [("air_ring_chain", [-400, 600, 4000], "E_TRACK_CLEARANCE"), ("grind_shortcut", [0, 40, 7000], "E_TRACK_CLEARANCE")] {
        let mut base = authoring::Source::empty();
        base.settings.circuit = false;
        base.instances.push(authoring::instance("anchor", "straight", 800));
        let mut crossing = authoring::instance("crossing", "straight", 800);
        crossing.position_cm = position;
        crossing.rotation_mdeg = [0, 90000, 0];
        base.instances.push(crossing);
        base.paths.push(authoring::Path { id: "base".into(), pieces: vec!["anchor".into()] });
        assert_eq!(templates::append(&base, id, "anchor", "x").unwrap_err().code, code);
    }
}

#[test]
fn pipes_jumps_shortcuts_and_narrow_roads_follow_their_rules() {
    let all: Vec<_> = (0..128).map(|s| settings(s, false, Length::Short)).collect();
    let results = generate(&all);
    let (mut narrow, mut drops, mut jumps, mut rails, mut documents) = (0, 0, 0, 0, 0);
    let mut shapes = std::collections::BTreeSet::new();
    let mut piped = 0;
    for (s, r) in all.iter().zip(&results) {
        let label = format!("{s:?}");
        let (a, _) = r.as_ref().unwrap_or_else(|e| panic!("{label}: {e:?}"));
        assert_rules(a, &label);
        let source = a.seed_source.as_ref().unwrap();
        narrow += usize::from(track::generation_summary(a).narrow_road.sections == 1);
        drops += source.actions.iter().filter(|a| a.kind == "jump_panel").count();
        jumps += source.jump_shortcuts.len();
        rails += source.grind_lines.iter().filter(|l| l.id.ends_with("-rail")).count();
        let pipes: Vec<_> = base(a).into_iter().filter(|p| p.id.starts_with("cylinder") || p.id == "banked_chicane").collect();
        if let Some(p) = pipes.first() {
            piped += 1;
            shapes.insert((p.id.clone(), p.width_cm, p.control_points.clone()));
        }
        if !pipes.is_empty() && documents < 6 {
            documents += 1;
            let d = track::document_from_assembly(a.clone()).unwrap();
            let tracks: Vec<_> = d.gimmicks.iter().filter_map(|g| g.track.as_ref()).collect();
            assert!(!tracks.is_empty() && tracks.iter().all(|t| t.valid()), "{label}: special track");
            for g in d.gimmicks.iter().filter(|g| g.track.as_ref().is_some_and(|t| t.kind == mapkit_core::special_track::TrackKind::SweptHalfPipe)) {
                let p = &a.pieces[g.id.split('-').nth(1).unwrap().parse::<usize>().unwrap()];
                let t = g.track.as_ref().unwrap();
                assert_eq!((t.radius_cm, t.width_cm), (p.width_cm / 2, p.width_cm), "{label}: halfpipe bore");
            }
        }
    }
    println!("narrow {narrow}/128, drops {drops}, jump shortcuts {jumps}, grind shortcuts {rails}, distinct first pipes {}/{piped}", shapes.len());
    assert!((8..=26).contains(&narrow), "narrow share {narrow}/128");
    assert!(shapes.len() * 100 >= piped * 80, "pipe shapes repeat: {}/{piped}", shapes.len());
    assert!(drops > 0 && jumps > 0 && rails > 0, "drops {drops}, jump shortcuts {jumps}, grind shortcuts {rails}");
}
