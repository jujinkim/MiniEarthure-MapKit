//! Corner shortcuts made of ordinary roads: a straight ground chord across the inside
//! of a turning junction, joined by splitting the two course roads at new graph nodes.
use super::*;
use crate::RoadNode;

pub const MAX_SHORTCUTS: u8 = 4;
const MIN_TURN_DEG: f64 = 60.0;
/// Distances from the junction tried for both ends, longest first (1:8 city blocks).
const REACH_CM: [f64; 3] = [1_500.0, 1_000.0, 600.0];
/// Split points keep this much road before the far end of each host road.
const END_MARGIN_CM: f64 = 200.0;
/// Extra horizontal clearance from buildings, water and other roads.
const CLEAR_CM: f64 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    pub id: String,
    /// Exact current records that the shortcut splits; apply refuses stale ones.
    pub replaces: Vec<Road>,
    pub roads: Vec<Road>,
    pub nodes: Vec<RoadNode>,
}
pub(super) struct Proposed {
    pub document: MapDocument,
    pub shortcuts: Vec<Shortcut>,
    /// Course distance ranges the shortcuts bypass; gates inside are omitted.
    pub bypassed: Vec<(f64, f64)>,
    /// Original road indices that were split.
    pub split: BTreeSet<usize>,
}

fn point_at(points: &[Vertex], distance: f64) -> Vertex {
    let mut left = distance;
    for w in points.windows(2) {
        let l = len3(w[0], w[1]);
        if l >= left && l > 0.0 { return std::array::from_fn(|k| w[0][k] + libm::round((w[1][k] - w[0][k]) as f64 * left / l) as i64); }
        left -= l;
    }
    *points.last().unwrap()
}
fn segment_gap(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    let cross = |o: [f64; 2], p: [f64; 2], q: [f64; 2]| (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 { return 0.0; }
    let point = |p: [f64; 2], s: [f64; 2], e: [f64; 2]| {
        let (dx, dz) = (e[0] - s[0], e[1] - s[1]);
        let n = dx * dx + dz * dz;
        let t = if n == 0.0 { 0.0 } else { (((p[0] - s[0]) * dx + (p[1] - s[1]) * dz) / n).clamp(0.0, 1.0) };
        (p[0] - s[0] - t * dx).hypot(p[1] - s[1] - t * dz)
    };
    point(a, c, d).min(point(b, c, d)).min(point(c, a, b)).min(point(d, a, b))
}
fn inside(p: [f64; 2], ring: &[Point]) -> bool {
    let mut odd = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (ax, az, bx, bz) = (a[0] as f64, a[1] as f64, b[0] as f64, b[1] as f64);
        if (az > p[1]) != (bz > p[1]) && p[0] < ax + (p[1] - az) * (bx - ax) / (bz - az) { odd = !odd; }
    }
    odd
}
/// The chord keeps half its width plus clearance from every independent solid footprint.
fn clear(d: &MapDocument, a: Vertex, b: Vertex, width: u32, hosts: &[&str]) -> bool {
    let (p, q) = ([a[0] as f64, a[2] as f64], [b[0] as f64, b[2] as f64]);
    let half = f64::from(width) / 2.0 + CLEAR_CM;
    let (lo, hi) = ([p[0].min(q[0]), p[1].min(q[1])], [p[0].max(q[0]), p[1].max(q[1])]);
    let near = |ring: &[Point], reach: f64| {
        ring.iter().any(|v| v[0] as f64 >= lo[0] - reach && v[0] as f64 <= hi[0] + reach && v[1] as f64 >= lo[1] - reach && v[1] as f64 <= hi[1] + reach)
    };
    let blocks = |ring: &[Point], reach: f64| {
        if ring.len() < 3 { return false; }
        if inside(p, ring) || inside(q, ring) || inside([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], ring) { return true; }
        (0..ring.len()).any(|i| {
            let (s, e) = (ring[i], ring[(i + 1) % ring.len()]);
            segment_gap(p, q, [s[0] as f64, s[1] as f64], [e[0] as f64, e[1] as f64]) < reach
        })
    };
    for building in &d.buildings {
        if (near(&building.footprint, half + 100_000.0)) && blocks(&building.footprint, half) { return false; }
    }
    for water in &d.water_bodies {
        if blocks(&water.polygon, half) { return false; }
    }
    for r in &d.roads {
        if hosts.contains(&r.id.as_str()) { continue; }
        for (s, w) in r.points.windows(2).enumerate() {
            let reach = half + f64::from(r.widths_cm.get(s).copied().unwrap_or(0)) / 2.0;
            if segment_gap(p, q, [w[0][0] as f64, w[0][2] as f64], [w[1][0] as f64, w[1][2] as f64]) < reach { return false; }
        }
    }
    true
}
/// Splits `road` at `p` into `from → node` (keeping the ID) and `node → to`.
fn split(road: &Road, p: Vertex, node: &str, second: &str) -> Option<(Road, Road)> {
    let mut best: Option<(f64, usize)> = None;
    for (s, w) in road.points.windows(2).enumerate() {
        let gap = segment_gap([p[0] as f64, p[2] as f64], [p[0] as f64, p[2] as f64], [w[0][0] as f64, w[0][2] as f64], [w[1][0] as f64, w[1][2] as f64]);
        if best.is_none_or(|b| gap < b.0) { best = Some((gap, s)); }
    }
    let (gap, s) = best?;
    if gap > 1.0 { return None; }
    let near = |v: Vertex| h(v, p)[0].hypot(h(v, p)[1]) < 50.0;
    if near(road.points[0]) || near(*road.points.last().unwrap()) { return None; }
    let mut a = road.clone();
    let mut b = road.clone();
    let (cut, head, tail) = if near(road.points[s]) { (road.points[s], s, s) }
        else if near(road.points[s + 1]) { (road.points[s + 1], s + 1, s + 1) }
        else { (p, s + 1, s) };
    a.points = [road.points[..head].to_vec(), vec![cut]].concat();
    b.points = [vec![cut], road.points[tail + 1..].to_vec()].concat();
    if a.points.len() < 2 || b.points.len() < 2 { return None; }
    let segments = |range: std::ops::Range<usize>| (road.widths_cm[range.clone()].to_vec(), road.surfaces[range].to_vec());
    // Segment s is shared by both halves when the cut is interior to it.
    (a.widths_cm, a.surfaces) = segments(0..a.points.len() - 1);
    let start = road.widths_cm.len() - (b.points.len() - 1);
    (b.widths_cm, b.surfaces) = segments(start..road.widths_cm.len());
    a.to = node.into();
    b.from = node.into();
    b.id = second.into();
    if let Some(m) = &mut a.markings { m.crosswalk_end = false; }
    if let Some(m) = &mut b.markings { m.crosswalk_start = false; }
    Some((a, b))
}

pub(super) fn propose(d: &MapDocument, request: &Request, route: &Route, starts: &[f64], start_road: usize) -> Result<Proposed> {
    let mut out = Proposed { document: d.clone(), shortcuts: vec![], bypassed: vec![], split: BTreeSet::new() };
    let referenced = |road: &Road| d.surface_attachments.iter().any(|a| a.surface.surface_id == road.id)
        || d.assembled_track.as_ref().and_then(|a| a.authoring.as_ref()).is_some_and(|s| s.road_connections.iter().any(|c| c.road == road.id));
    for i in 0..route.legs.len().saturating_sub(1) {
        cancellation::checkpoint()?;
        if out.shortcuts.len() >= usize::from(request.shortcuts) { break; }
        let (inn, next) = (&route.legs[i], &route.legs[i + 1]);
        let hosts = [inn.road, next.road];
        if inn.road == next.road || hosts.iter().any(|&r| r == start_road || out.split.contains(&r)) { continue; }
        if hosts.iter().any(|&r| { let r = &d.roads[r]; r.kind != crate::RoadKind::Ground || r.design.is_some() || referenced(r) }) { continue; }
        if angle(entry(&inn.points), exit(&next.points)) < MIN_TURN_DEG { continue; }
        let junction = &inn.end;
        let level = d.nodes.iter().find(|n| &n.id == junction).map_or(0, |n| n.level);
        let width = design_width(&d.roads[inn.road]).min(design_width(&d.roads[next.road]));
        for reach in REACH_CM {
            let (before, after) = (polyline(&inn.points), polyline(&next.points));
            if before < reach + END_MARGIN_CM || after < reach + END_MARGIN_CM { continue; }
            let (a, b) = (point_at(&inn.points, before - reach), point_at(&next.points, reach));
            if !clear(&out.document, a, b, width, &[d.roads[inn.road].id.as_str(), d.roads[next.road].id.as_str()]) { continue; }
            let base = crate::sha256(format!("{}:{}:{}:{reach}", d.map_id, d.roads[inn.road].id, d.roads[next.road].id).as_bytes())[..12].to_string();
            let id = format!("shortcut-{base}");
            let (entry_node, exit_node) = (format!("{id}-entry"), format!("{id}-exit"));
            let second = |road: &Road| format!("{}-{}", road.id.chars().take(100).collect::<String>(), &base[..8]);
            if out.document.roads.iter().any(|r| r.id == id || r.id == second(&d.roads[inn.road]) || r.id == second(&d.roads[next.road]))
                || out.document.nodes.iter().any(|n| n.id == entry_node || n.id == exit_node) { continue; }
            let Some((ia, ib)) = split(&d.roads[inn.road], a, &entry_node, &second(&d.roads[inn.road])) else { continue; };
            let Some((oa, ob)) = split(&d.roads[next.road], b, &exit_node, &second(&d.roads[next.road])) else { continue; };
            let chord = Road { design: None, snow_retention_percent: d.roads[inn.road].snow_retention_percent, id: id.clone(),
                from: entry_node.clone(), to: exit_node.clone(), points: vec![a, b], widths_cm: vec![width],
                surfaces: vec![d.roads[inn.road].surfaces.first().copied().unwrap_or(crate::Surface::Asphalt)],
                kind: crate::RoadKind::Ground, clearance_cm: None, sidewalk_cm: None, markings: None };
            let nodes = vec![RoadNode { id: entry_node.clone(), position: a, level }, RoadNode { id: exit_node.clone(), position: b, level }];
            let shortcut = Shortcut { id, replaces: vec![d.roads[inn.road].clone(), d.roads[next.road].clone()], roads: vec![ia, ib, oa, ob, chord], nodes };
            let Ok(candidate) = apply(&out.document, std::slice::from_ref(&shortcut)) else { continue; };
            out.document = candidate;
            out.bypassed.push((starts[i + 1] - reach, starts[i + 1] + reach));
            out.split.extend(hosts);
            out.shortcuts.push(shortcut);
            break;
        }
    }
    Ok(out)
}

/// Replaces split records in place and appends the rest, so unrelated road
/// indices stay stable; then validates the whole document.
pub fn apply(d: &MapDocument, shortcuts: &[Shortcut]) -> Result<MapDocument> {
    if shortcuts.len() > usize::from(MAX_SHORTCUTS) {
        return Err(error("E_ROAD_COURSE_REQUEST", "too many proposed shortcuts"));
    }
    let mut next = d.clone();
    for s in shortcuts {
        for (n, old) in s.replaces.iter().enumerate() {
            let at = next.roads.iter().position(|r| r == old)
                .ok_or_else(|| error("E_ROAD_COURSE_STALE", format!("{}: road {} changed since the proposal", s.id, old.id)))?;
            let first = s.roads.get(n * 2).filter(|r| r.id == old.id)
                .ok_or_else(|| error("E_ROAD_COURSE_REQUEST", "shortcut must keep each split road ID"))?;
            next.roads[at] = first.clone();
        }
        // Split pairs are [kept, new] per replaced road, then added roads.
        next.roads.extend(s.roads.iter().enumerate().filter(|(i, _)| *i >= s.replaces.len() * 2 || i % 2 == 1).map(|(_, r)| r.clone()));
        next.nodes.extend(s.nodes.iter().cloned());
    }
    next.normalize();
    next.validate()?;
    Ok(next)
}
