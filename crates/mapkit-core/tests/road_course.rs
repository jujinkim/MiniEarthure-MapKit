use mapkit_core::{course::{effective_checkpoint_indices, Mode}, road_course::{apply_extras, suggest, Extras, Request, Suggestion}, MapDocument};

fn extras(s: &Suggestion) -> Extras {
    Extras { shortcuts: s.shortcuts.clone(), road_designs: s.road_designs.clone(), attachments: s.attachments.clone() }
}

/// Synthetic n×n street grid, 100 m blocks, 8 m wide ground roads.
fn grid(n: i64) -> MapDocument { scaled_grid(n, 10_000, 800) }
/// Street grid with the given block length and road width.
fn scaled_grid(n: i64, block: i64, width: u32) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.bounds.max = [block * (n - 1) + 8_000, block * (n - 1) + 8_000];
    d.bounds.min = [-8_000, -8_000];
    let id = |x: i64, z: i64| format!("n{x}-{z}");
    let mut nodes = vec![];
    let mut roads = vec![];
    for x in 0..n { for z in 0..n {
        nodes.push(serde_json::json!({"id":id(x,z),"position":[x*block,0,z*block],"level":0}));
        for (dx, dz) in [(1, 0), (0, 1)] {
            if x + dx >= n || z + dz >= n { continue; }
            roads.push(serde_json::json!({"id":format!("r{x}-{z}-{dx}{dz}"),"from":id(x,z),"to":id(x+dx,z+dz),
                "points":[[x*block,0,z*block],[(x+dx)*block,0,(z+dz)*block]],"widths_cm":[width],"surfaces":["asphalt"],
                "kind":"ground","clearance_cm":null,"sidewalk_cm":null}));
        }
    }}
    d.nodes = serde_json::from_value(nodes.into()).unwrap();
    d.roads = serde_json::from_value(roads.into()).unwrap();
    d
}
fn request(mode: Mode, seconds: u16) -> Request {
    Request { start_cm: [5_000, 0], heading: [1, 0], mode, duration_seconds: seconds, min_width_cm: 0, gimmicks: 0, seed: 0, shortcuts: 0 }
}

#[test]
fn sprint_follows_roads_to_the_requested_length() {
    let d = grid(6);
    let before = d.clone();
    let result = suggest(&d, &request(Mode::Sprint, 60)).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert_eq!(d, before);
    assert_eq!(result.length_cm, 54_000);
    assert_eq!(result.gates[0].position_cm, [5_000, 0, 0]);
    assert_eq!(result.start_direction, [1_000_000, 0]);
    assert!(result.gates.len() >= 3 && result.gates.len() <= 64);
    assert_eq!(effective_checkpoint_indices(&result.gates).len(), result.gates.len());
    assert!(result.gates.iter().all(|g| d.roads.iter().any(|r| r.id == g.surface_id)));
    assert_eq!(result, suggest(&d, &request(Mode::Sprint, 60)).unwrap());
}
#[test]
fn heading_reverses_the_start_road() {
    let d = grid(6);
    let mut r = request(Mode::Sprint, 30);
    r.heading = [-1, 0];
    let result = suggest(&d, &r).unwrap();
    assert_eq!(result.start_direction, [-1_000_000, 0]);
}
#[test]
fn circuit_returns_to_the_start_with_turn_gates() {
    let d = grid(6);
    let result = suggest(&d, &request(Mode::Circuit, 60)).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert_eq!(result.roads.first(), result.roads.last());
    assert!((result.length_cm as f64 - 54_000.0).abs() <= 54_000.0 * 0.25, "{}", result.length_cm);
    assert!(result.gates.len() >= 4);
    assert_eq!(effective_checkpoint_indices(&result.gates).len(), result.gates.len());
    // Every corner of the loop carries a gate at its junction.
    let corners = result.gates.iter().filter(|g| g.position_cm[0] % 10_000 == 0 && g.position_cm[2] % 10_000 == 0).count();
    assert!(corners >= 3, "{:?}", result.gates);
}
#[test]
fn far_start_and_bad_requests_are_reported() {
    let mut d = grid(3);
    d.bounds.max = [40_000, 40_000];
    let mut r = request(Mode::Sprint, 60);
    r.start_cm = [35_000, 35_000];
    let result = suggest(&d, &r).unwrap();
    assert!(!result.applicable);
    assert_eq!(result.diagnostics[0].code, "E_ROAD_COURSE_START");
    r.duration_seconds = 5;
    assert!(suggest(&d, &r).is_err());
    let mut narrow = request(Mode::Sprint, 60);
    narrow.min_width_cm = 1_000;
    assert_eq!(suggest(&d, &narrow).unwrap().diagnostics[0].code, "E_ROAD_COURSE_START");
}
#[test]
fn short_network_warns_without_blocking() {
    let d = grid(2);
    let result = suggest(&d, &request(Mode::Sprint, 120)).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert!(result.diagnostics.iter().any(|d| d.code == "W_ROAD_COURSE_LENGTH"));
    assert!(result.length_cm < 108_000);
}
#[test]
fn seeded_gimmicks_attach_to_forward_course_roads() {
    let d = grid(6);
    let mut r = request(Mode::Circuit, 90);
    r.gimmicks = 4;
    r.seed = 11;
    let result = suggest(&d, &r).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert!(!result.attachments.is_empty() && result.attachments.len() <= 4, "{:?}", result.diagnostics);
    assert_eq!(result, suggest(&d, &r).unwrap());
    assert!(result.attachments.iter().all(|a| result.roads.contains(&a.surface.surface_id)));
    // Unaligned synthetic roads receive default alignments only through explicit apply.
    assert!(!result.road_designs.is_empty() && d.roads.iter().all(|r| r.design.is_none()));
    let applied = mapkit_core::road_course::apply_extras(&d, &extras(&result)).unwrap();
    assert_eq!(applied.surface_attachments.len(), result.attachments.len());
    assert!(mapkit_core::road_course::apply_extras(&applied, &extras(&result)).is_err());
    assert!(applied.gimmicks.len() >= result.attachments.len());
    r.gimmicks = 17;
    assert!(suggest(&d, &r).is_err());
}
#[test]
fn city_scale_blocks_still_receive_gimmicks() {
    // 1:8 import: about 12.5 m blocks and 1 m primary roads.
    let d = scaled_grid(8, 1_250, 100);
    let r = Request { start_cm: [600, 0], heading: [1, 0], mode: Mode::Circuit, duration_seconds: 20, min_width_cm: 0, gimmicks: 4, seed: 3, shortcuts: 0 };
    let result = suggest(&d, &r).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert!(!result.attachments.is_empty(), "{:?}", result.diagnostics);
    mapkit_core::road_course::apply_extras(&d, &extras(&result)).unwrap();
}
#[test]
fn corner_shortcuts_split_course_roads_and_bypass_corner_gates() {
    let d = grid(6);
    let mut r = request(Mode::Circuit, 60);
    let plain = suggest(&d, &r).unwrap();
    r.shortcuts = 2;
    let result = suggest(&d, &r).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert_eq!(result.shortcuts.len(), 2, "{:?}", result.diagnostics);
    assert!(result.gates.len() < plain.gates.len());
    assert_eq!(result, suggest(&d, &r).unwrap());
    let applied = apply_extras(&d, &extras(&result)).unwrap();
    assert_eq!(applied.roads.len(), d.roads.len() + 2 * 3);
    assert_eq!(applied.nodes.len(), d.nodes.len() + 2 * 2);
    // The new junctions and chords generate with the ordinary road pipeline.
    for node in applied.nodes.iter().filter(|n| n.id.starts_with("shortcut-")) {
        let cell = applied.cell_at([node.position[0], node.position[2]]).unwrap();
        let chunk = mapkit_core::generate(mapkit_core::GenerationInput { document: &applied, cell, heightgrid: None, max_triangles: 400_000 }).unwrap();
        assert!(chunk.triangles.iter().any(|t| t.object_id.starts_with("shortcut-")), "{cell:?}");
    }
    // Every gate still lies on a road of the applied map, and stale proposals are refused.
    assert!(apply_extras(&applied, &extras(&result)).is_err());
    // Shortcuts and gimmicks combine; gimmicks avoid the split roads.
    r.gimmicks = 4; r.seed = 5;
    let both = suggest(&d, &r).unwrap();
    assert!(both.applicable, "{:?}", both.diagnostics);
    let split: Vec<&String> = both.shortcuts.iter().flat_map(|s| s.replaces.iter().map(|r| &r.id)).collect();
    assert!(both.attachments.iter().all(|a| !split.contains(&&a.surface.surface_id)));
    apply_extras(&d, &extras(&both)).unwrap();
}
#[test]
fn buildings_on_the_inside_corner_block_a_shortcut() {
    let mut d = grid(6);
    let mut blocks = vec![];
    for x in 0..5 { for z in 0..5 {
        let (x0, z0) = (x * 10_000 + 700, z * 10_000 + 700);
        blocks.push(serde_json::json!({"id":format!("b{x}-{z}"),"footprint":[[x0,z0],[x0+8_600,z0],[x0+8_600,z0+8_600],[x0,z0+8_600]],
            "base_cm":0,"height_cm":800,"usage":"commercial","material":"concrete","roof":"flat"}));
    }}
    d.buildings = serde_json::from_value(blocks.into()).unwrap();
    let mut r = request(Mode::Circuit, 60);
    r.shortcuts = 2;
    let result = suggest(&d, &r).unwrap();
    assert!(result.applicable, "{:?}", result.diagnostics);
    assert!(result.shortcuts.is_empty());
    assert!(result.diagnostics.iter().any(|d| d.code == "W_ROAD_COURSE_SHORTCUTS"));
}
#[test]
fn city_scale_corners_receive_shortcuts() {
    let d = scaled_grid(8, 1_250, 100);
    let r = Request { start_cm: [600, 0], heading: [1, 0], mode: Mode::Circuit, duration_seconds: 20, min_width_cm: 0, gimmicks: 0, seed: 0, shortcuts: 2 };
    let result = suggest(&d, &r).unwrap();
    assert!(result.applicable && !result.shortcuts.is_empty(), "{:?}", result.diagnostics);
    apply_extras(&d, &extras(&result)).unwrap();
}
/// Buildings fill every block corner (from `setback` to `size` off both streets),
/// so ground chords are blocked while long elevated chords find open ground for ramps.
fn built_up(block: i64, width: u32, setback: i64, size: i64, height: u32) -> MapDocument {
    let mut d = scaled_grid(6, block, width);
    let mut blocks = vec![];
    for x in 0..5 { for z in 0..5 { for (cx, cz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let span = |c: i64, base: i64| if c == 0 { (base + setback, base + size) } else { (base + block - size, base + block - setback) };
        let ((x0, x1), (z0, z1)) = (span(cx, x * block), span(cz, z * block));
        blocks.push(serde_json::json!({"id":format!("b{x}-{z}-{cx}{cz}"),"footprint":[[x0,z0],[x1,z0],[x1,z1],[x0,z1]],
            "base_cm":0,"height_cm":height,"usage":"commercial","material":"concrete","roof":"flat"}));
    }}}
    d.buildings = serde_json::from_value(blocks.into()).unwrap();
    d.normalize();
    d
}
/// Steepest grade over horizontal spans of at least 2 m (centimetre rounding aside).
fn steepest(points: &[[i64; 3]]) -> f64 {
    let run = |a: [i64; 3], b: [i64; 3]| ((b[0] - a[0]) as f64).hypot((b[2] - a[2]) as f64);
    (0..points.len()).filter_map(|i| (i + 1..points.len()).find(|&j| run(points[i], points[j]) >= 200.0)
        .map(|j| ((points[j][1] - points[i][1]) as f64).abs() / run(points[i], points[j]))).fold(0.0, f64::max)
}
#[test]
fn elevated_shortcuts_ramp_over_buildings_that_block_ground_chords() {
    use mapkit_core::road_course::ShortcutForm;
    let d = built_up(6_000, 150, 300, 2_000, 80);
    let mut r = Request { start_cm: [3_000, 0], heading: [1, 0], mode: Mode::Circuit, duration_seconds: 60, min_width_cm: 0, gimmicks: 0, seed: 0, shortcuts: 4 };
    let mut forms = std::collections::BTreeSet::new();
    for seed in 0..8 {
        r.seed = seed;
        let result = suggest(&d, &r).unwrap();
        assert!(result.applicable, "{:?}", result.diagnostics);
        assert!(!result.shortcuts.is_empty(), "seed {seed}: {:?}", result.diagnostics);
        assert_eq!(result, suggest(&d, &r).unwrap());
        for s in &result.shortcuts {
            let chord = s.roads.iter().find(|r| r.id == s.id).unwrap();
            assert_ne!(s.form, ShortcutForm::Ground);
            assert_eq!(chord.kind, mapkit_core::RoadKind::Elevated);
            assert!(chord.points.iter().map(|p| p[1]).max().unwrap() >= 80 + 50, "{:?}", chord.points);
            let limit = if s.form == ShortcutForm::Ramp { 0.12 } else { 0.30 };
            assert!(steepest(&chord.points) <= limit, "{:?} {}", s.form, steepest(&chord.points));
            forms.insert(format!("{:?}", s.form));
        }
        let applied = apply_extras(&d, &extras(&result)).unwrap();
        assert_eq!(applied.buildings, d.buildings);
        // Every junction, the deck and the buildings below generate together.
        let mut cells = std::collections::BTreeSet::new();
        for s in &result.shortcuts {
            let chord = s.roads.iter().find(|r| r.id == s.id).unwrap();
            let middle = chord.points[chord.points.len() / 2];
            cells.extend(s.nodes.iter().map(|n| [n.position[0], n.position[2]]).chain([[middle[0], middle[2]]]).map(|p| applied.cell_at(p).unwrap()));
        }
        for cell in cells {
            let chunk = mapkit_core::generate(mapkit_core::GenerationInput { document: &applied, cell, heightgrid: None, max_triangles: 400_000 }).unwrap();
            assert!(chunk.triangles.iter().any(|t| t.object_id.starts_with("shortcut-")), "{cell:?}");
            assert!(!chunk.building_prisms.is_empty(), "{cell:?}");
        }
    }
    assert_eq!(forms.len(), 2, "{forms:?}");
    // A deck too low for a taller roof is never proposed.
    let mut tall = d.clone();
    for b in &mut tall.buildings { b.height_cm = 2_000; }
    r.seed = 0;
    assert!(suggest(&tall, &r).unwrap().shortcuts.is_empty());
}
#[test]
fn seeded_races_pick_start_direction_extras_and_environment_from_one_seed() {
    use mapkit_core::road_course::{seed, SeedRequest};
    let d = grid(6);
    let request = |seed: u64| SeedRequest { seed, mode: Mode::Circuit, duration_seconds: 60, min_width_cm: 0, gimmicks: 4, shortcuts: 2 };
    let first = seed(&d, &request(7)).unwrap();
    assert_eq!(first, seed(&d, &request(7)).unwrap());
    assert!(first.suggestion.applicable, "{:?}", first.suggestion.diagnostics);
    assert!(!first.suggestion.attachments.is_empty() && !first.suggestion.shortcuts.is_empty(), "{:?}", first.suggestion.diagnostics);
    let environment = mapkit_core::assembled_track::track_seed_defaults(7).unwrap();
    assert_eq!(u64::from(first.time_minutes), environment["time_minutes"].as_u64().unwrap());
    assert_eq!(first.weather, environment["weather"].as_str().unwrap());
    // The resolved start reproduces the same proposal through the ordinary request.
    let direct = suggest(&d, &Request { start_cm: first.start_cm, heading: first.heading, mode: Mode::Circuit, duration_seconds: 60,
        min_width_cm: 0, gimmicks: 4, seed: 7, shortcuts: 2 }).unwrap();
    assert_eq!(direct, first.suggestion);
    apply_extras(&d, &extras(&first.suggestion)).unwrap();
    // Different seeds start in different places or directions.
    let starts: std::collections::BTreeSet<_> = (0..8).map(|s| { let r = seed(&d, &request(s)).unwrap(); (r.start_cm, r.heading) }).collect();
    assert!(starts.len() > 4, "{starts:?}");
    // Gates only, and maps without roads are refused.
    let gates_only = seed(&d, &SeedRequest { gimmicks: 0, shortcuts: 0, ..request(7) }).unwrap();
    assert!(gates_only.suggestion.attachments.is_empty() && gates_only.suggestion.shortcuts.is_empty());
    let mut empty = d.clone();
    empty.roads.clear();
    assert_eq!(seed(&empty, &request(7)).unwrap_err().code, "E_ROAD_COURSE_START");
}
