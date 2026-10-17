use mapkit_core::{course::{effective_checkpoint_indices, Mode}, road_course::{suggest, Request}, MapDocument};

/// Synthetic n×n street grid, 100 m blocks, 8 m wide ground roads.
fn grid(n: i64) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    let block = 10_000;
    d.bounds.max = [block * (n - 1) + 2_000, block * (n - 1) + 2_000];
    d.bounds.min = [-2_000, -2_000];
    let id = |x: i64, z: i64| format!("n{x}-{z}");
    let mut nodes = vec![];
    let mut roads = vec![];
    for x in 0..n { for z in 0..n {
        nodes.push(serde_json::json!({"id":id(x,z),"position":[x*block,0,z*block],"level":0}));
        for (dx, dz) in [(1, 0), (0, 1)] {
            if x + dx >= n || z + dz >= n { continue; }
            roads.push(serde_json::json!({"id":format!("r{x}-{z}-{dx}{dz}"),"from":id(x,z),"to":id(x+dx,z+dz),
                "points":[[x*block,0,z*block],[(x+dx)*block,0,(z+dz)*block]],"widths_cm":[800],"surfaces":["asphalt"],
                "kind":"ground","clearance_cm":null,"sidewalk_cm":null}));
        }
    }}
    d.nodes = serde_json::from_value(nodes.into()).unwrap();
    d.roads = serde_json::from_value(roads.into()).unwrap();
    d
}
fn request(mode: Mode, seconds: u16) -> Request {
    Request { start_cm: [5_000, 0], heading: [1, 0], mode, duration_seconds: seconds, min_width_cm: 0 }
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
