//! Road paint over the shader cap degrades display; it never fails a cell.
use mapkit_core::*;

fn marked(id: &str, points: Vec<Vertex>) -> Road {
    let n = points.len() - 1;
    Road { style_id: String::new(), sidewalk_style_id: String::new(), design: None, snow_retention_percent: 100, id: id.into(), from: format!("{id}:a"), to: format!("{id}:b"),
        points, widths_cm: vec![400; n], surfaces: vec![Surface::Asphalt; n], kind: RoadKind::Bridge,
        clearance_cm: None, sidewalk_cm: Some(80),
        markings: Some(RoadMarkings { color: None, lanes: 2, center_line: true, edge_lines: true, crosswalk_start: false,
            crosswalk_end: false, one_way: 0, stop_lines: false, lane_width_cm: 0 , median_width_cm: 0, forward_lanes: 0}) }
}
fn document(roads: Vec<Road>) -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.roads.clear();d.nodes.clear();d.buildings.clear();d.zones.clear();
    d.bounds = Bounds { min: [0, 0], max: [40000, 20000] };
    for r in &roads {
        for (id, p) in [(&r.from, r.points[0]), (&r.to, *r.points.last().unwrap())] {
            d.nodes.push(RoadNode { id: id.clone(), position: p, level: 0 });
        }
    }
    d.roads = roads;
    d
}
/// Densely sampled straight road: every source sample is collinear.
fn straight(id: &str, z: i64) -> Road {
    marked(id, (0..=300).map(|i| [2000 + i * 30, 0, z]).collect())
}
/// Gentle wiggle sampled finer than 1 cm of chord error keeps nearly every sample.
fn wiggle(id: &str, z: i64) -> Road {
    marked(id, (0..=300).map(|i| {
        let x = 2000 + i * 50;
        [x, 0, z + (200.0 * (x as f64 * std::f64::consts::TAU / 2000.0).sin()).round() as i64]
    }).collect())
}

#[test]
fn dense_undesigned_paint_is_compacted() {
    let d = document(vec![straight("dense", 5000)]);
    let paint = d.road_paint(&d.bounds).unwrap();
    assert!(paint.overflow.is_empty());
    assert!(!paint.paths["dense"].is_empty() && paint.paths["dense"].len() <= 4, "{}", paint.paths["dense"].len());
    assert!(!paint.edges["dense"].is_empty() && paint.edges["dense"].len() <= 8, "{}", paint.edges["dense"].len());
    // Stations are measured before compaction and still span the whole road.
    let last = paint.paths["dense"].last().unwrap();
    assert!((last.station_cm + ((last.b[0] - last.a[0]) as f64).hypot((last.b[2] - last.a[2]) as f64) - last.total_cm).abs() < 0.5);
}

#[test]
fn overflowing_road_loses_only_its_own_paint() {
    let d = document(vec![wiggle("wiggle", 5000), straight("plain", 15000)]);
    let paint = d.road_paint(&d.bounds).unwrap();
    assert_eq!(paint.overflow.keys().collect::<Vec<_>>(), ["wiggle"]);
    assert!(paint.overflow["wiggle"] > RoadPaint::SEGMENTS);
    assert!(!paint.paths.contains_key("wiggle") && !paint.edges.contains_key("wiggle"));
    assert!(!paint.paths["plain"].is_empty() && !paint.edges["plain"].is_empty());
    assert!(paint.paths.values().all(|v| v.len() <= RoadPaint::SEGMENTS));
    assert!(paint.edges.values().all(|v| v.len() <= RoadPaint::SEGMENTS));
    // Per-cell bounds degrade the same way instead of failing.
    let cell = d.cell_at([12000, 5000]).unwrap();
    let local = d.road_paint(&d.cell_bounds(cell).unwrap()).unwrap();
    assert!(local.overflow.contains_key("wiggle") || local.paths.contains_key("wiggle"));
    assert!(local.paths.values().all(|v| v.len() <= RoadPaint::SEGMENTS));
}

/// Distance from `p` to the polyline `line` in plan (x, z).
fn to_line(p: [f64; 2], line: &[Vertex]) -> f64 {
    line.windows(2).map(|s| {
        let (a, b) = ([s[0][0] as f64, s[0][2] as f64], [s[1][0] as f64, s[1][2] as f64]);
        let d = [b[0] - a[0], b[1] - a[1]];
        let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1])).clamp(0.0, 1.0);
        (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t)
    }).fold(f64::INFINITY, f64::min)
}

#[test]
fn compacted_edge_lines_keep_their_inward_orientation() {
    // Dense straight and bent undesigned bridges, clipped by cell-like bounds
    // through their middles: compaction must not reverse any border chain.
    let mut bend: Vec<Vertex> = (0..=100).map(|i| [2000 + i * 30, 0, 12000]).collect();
    bend.extend((1..30).map(|i| {
        let t = (i as f64 * 3.0).to_radians();
        [5000 + (3000.0 * t.sin()).round() as i64, 0, 15000 - (3000.0 * t.cos()).round() as i64]
    }));
    bend.extend((0..=130).map(|i| [8000, 0, 15000 + i * 30]));
    let d = document(vec![straight("straight", 5000), marked("bend", bend)]);
    for bounds in [Bounds { min: [5000, 0], max: [10000, 20000] }, Bounds { min: [0, 0], max: [7000, 20000] },
        Bounds { min: [6000, 13000], max: [12000, 20000] }, d.bounds.clone()] {
        let paint = d.road_paint(&bounds).unwrap();
        assert!(paint.overflow.is_empty(), "{:?} {:?}", paint.overflow, paint.paths.iter().map(|(k, v)| (k, v.len())).collect::<Vec<_>>());
        assert!(!paint.edges.is_empty());
        for r in &d.roads {
            for e in paint.edges.get(&r.id).into_iter().flatten() {
                // Shader space is (x, -z); urban_surface.gdshader shifts a border by
                // (dir.y, -dir.x) * 0.16 m, which must move it toward the centre line.
                let (a, b) = ([e[0][0] as f64, -e[0][2] as f64], [e[1][0] as f64, -e[1][2] as f64]);
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                let dir = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let shifted = [mid[0] + dir[1] * 16.0, mid[1] - dir[0] * 16.0];
                let side = to_line([mid[0], -mid[1]], &r.points);
                // End caps cross the centre line's end; side borders run beside it.
                assert!(side < 100.0 || to_line([shifted[0], -shifted[1]], &r.points) < side,
                    "{} border {e:?} in {bounds:?} points outward", r.id);
            }
        }
    }
}
