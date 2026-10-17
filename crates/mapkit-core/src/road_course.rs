//! Read-only course proposals over the authored road graph. The document is never
//! changed; consumers seal the returned gates as an ordinary v1 course.
use crate::course::{Checkpoint, CheckpointShape, Mode, PlacementMode};
use crate::{cancellation, error, MapDocument, Point, Result, Road, Vertex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Shared with seed tracks: requested seconds are converted at this speed.
pub const REFERENCE_SPEED_CMPS: u64 = 900;
pub const MIN_SECONDS: u16 = 20;
pub const MAX_SECONDS: u16 = 300;
const START_REACH_CM: f64 = 5_000.0;
const MAX_EXPANSIONS: usize = 200_000;
const MAX_DEPTH: usize = 512;
/// Arms sharper than this are near U-turns and never continue a route.
const MAX_TURN_DEG: f64 = 150.0;
/// A junction that turns at least this much always carries a gate.
const TURN_GATE_DEG: f64 = 30.0;
const MIN_SPACING_CM: f64 = 5_000.0;
const MAX_GATES: usize = 64;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Horizontal map position; the nearest eligible road centreline is used.
    pub start_cm: Point,
    /// Preferred horizontal travel direction; [0,0] keeps the road's own order.
    #[serde(default)]
    pub heading: [i64; 2],
    pub mode: Mode,
    pub duration_seconds: u16,
    /// Roads with any narrower segment are not used.
    #[serde(default)]
    pub min_width_cm: u32,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub blocking: bool,
    pub road: Option<String>,
    pub position_cm: Option<Vertex>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Suggestion {
    pub mode: Mode,
    pub start_direction: [i64; 2],
    pub gates: Vec<Checkpoint>,
    /// Ordered road IDs the route follows (the start road may appear at both ends).
    pub roads: Vec<String>,
    pub length_cm: u64,
    pub target_cm: u64,
    pub diagnostics: Vec<Diagnostic>,
    pub applicable: bool,
}
impl Suggestion {
    fn issue(&mut self, code: &str, message: impl Into<String>, blocking: bool, road: Option<&str>, position: Option<Vertex>) {
        self.diagnostics.push(Diagnostic { code: code.into(), message: message.into(), blocking, road: road.map(Into::into), position_cm: position });
    }
}

#[derive(Clone)]
struct Leg { road: usize, points: Vec<Vertex>, end: String }
#[derive(Clone)]
struct Route { legs: Vec<Leg>, length: f64 }

fn h(a: Vertex, b: Vertex) -> [f64; 2] { [(b[0] - a[0]) as f64, (b[2] - a[2]) as f64] }
fn len3(a: Vertex, b: Vertex) -> f64 { (0..3).map(|i| (b[i] - a[i]) as f64).map(|v| v * v).sum::<f64>().sqrt() }
fn polyline(points: &[Vertex]) -> f64 { points.windows(2).map(|w| len3(w[0], w[1])).sum() }
fn angle(a: [f64; 2], b: [f64; 2]) -> f64 {
    let n = a[0].hypot(a[1]) * b[0].hypot(b[1]);
    if n <= 0.0 { return 0.0; }
    ((a[0] * b[0] + a[1] * b[1]) / n).clamp(-1.0, 1.0).acos().to_degrees()
}
/// First nonzero horizontal direction leaving `points[0]`.
fn exit(points: &[Vertex]) -> [f64; 2] {
    points.windows(2).map(|w| h(w[0], w[1])).find(|d| d[0] != 0.0 || d[1] != 0.0).unwrap_or([0.0, 0.0])
}
fn entry(points: &[Vertex]) -> [f64; 2] {
    points.windows(2).rev().map(|w| h(w[0], w[1])).find(|d| d[0] != 0.0 || d[1] != 0.0).unwrap_or([0.0, 0.0])
}
fn oriented(r: &Road, forward: bool) -> Vec<Vertex> {
    let mut p = r.points.clone();
    if !forward { p.reverse(); }
    p
}
fn gate(position: Vertex, width_cm: u32, surface: &str) -> Checkpoint {
    Checkpoint { position_cm: position, radius_cm: (width_cm / 2 + 30).clamp(100, 100_000), shape: CheckpointShape::Sphere,
        placement_mode: PlacementMode::RoadSnap, surface_id: surface.into() }
}

struct Search<'a> {
    d: &'a MapDocument,
    arms: BTreeMap<&'a str, Vec<(usize, bool)>>,
    positions: BTreeMap<&'a str, Vertex>,
    target: f64,
    circuit: bool,
    home: String,
    excluded: usize,
    expansions: usize,
    exhausted: bool,
    best: Option<Route>,
}
impl Search<'_> {
    fn score(&self, length: f64) -> f64 { (length - self.target).abs() }
    fn offer(&mut self, route: &Route) {
        let better = match &self.best {
            None => true,
            Some(b) if self.circuit => self.score(route.length) < self.score(b.length),
            Some(b) => route.length > b.length,
        };
        if better { self.best = Some(route.clone()); }
    }
    fn done(&self) -> bool {
        self.best.as_ref().is_some_and(|b| if self.circuit { self.score(b.length) <= self.target * 0.1 } else { b.length >= self.target })
    }
    fn walk(&mut self, route: &mut Route, visited: &mut BTreeSet<String>, tail: f64) -> Result<()> {
        cancellation::checkpoint()?;
        if self.done() || route.legs.len() >= MAX_DEPTH { return Ok(()); }
        if self.expansions >= MAX_EXPANSIONS { self.exhausted = true; return Ok(()); }
        self.expansions += 1;
        let last = route.legs.last().unwrap();
        let node = last.end.clone();
        let incoming = entry(&last.points);
        let here = self.positions[node.as_str()];
        let toward_home = self.positions.get(self.home.as_str()).map(|&p| h(here, p));
        let late = self.circuit && route.length > self.target * 0.4;
        let mut options: Vec<(f64, &str, usize, bool)> = Vec::new();
        for &(road, forward) in self.arms.get(node.as_str()).map(Vec::as_slice).unwrap_or(&[]) {
            if road == self.excluded { continue; }
            let r = &self.d.roads[road];
            let points = oriented(r, forward);
            let turn = angle(incoming, exit(&points));
            if turn > MAX_TURN_DEG { continue; }
            let order = match toward_home { Some(home) if late => angle(exit(&points), home), _ => turn };
            options.push((order, r.id.as_str(), road, forward));
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
        for (_, _, road, forward) in options {
            let r = &self.d.roads[road];
            let points = oriented(r, forward);
            let end = if forward { r.to.clone() } else { r.from.clone() };
            let length = route.length + polyline(&points);
            if self.circuit {
                if end == self.home {
                    let closed = Route { legs: [route.legs.clone(), vec![Leg { road, points, end }]].concat(), length: length + tail };
                    if self.score(closed.length) <= self.target * 0.6 { self.offer(&closed); }
                    if self.done() { return Ok(()); }
                    continue;
                }
                let back = self.positions.get(end.as_str()).zip(self.positions.get(self.home.as_str()))
                    .map_or(f64::INFINITY, |(&a, &b)| h(a, b)[0].hypot(h(a, b)[1]));
                if length + back + tail > self.target * 1.6 { continue; }
            }
            if visited.contains(&end) { continue; }
            visited.insert(end.clone());
            route.legs.push(Leg { road, points, end: end.clone() });
            let before = route.length;
            route.length = length;
            if !self.circuit { self.offer(route); }
            self.walk(route, visited, tail)?;
            route.length = before;
            route.legs.pop();
            visited.remove(&end);
            if self.done() { return Ok(()); }
        }
        Ok(())
    }
}

/// Bounded, deterministic route search: straightest arms first, circuits bend home
/// after 40% of the target. Overlapping and over-budget gates are reported, not hidden.
pub fn suggest(d: &MapDocument, request: &Request) -> Result<Suggestion> {
    if !(MIN_SECONDS..=MAX_SECONDS).contains(&request.duration_seconds)
        || request.heading.iter().any(|v| v.unsigned_abs() > 1_000_000_000)
        || !d.bounds.contains(request.start_cm) {
        return Err(error("E_ROAD_COURSE_REQUEST", "start must lie inside the map and duration must be 20–300 s"));
    }
    let target = (u64::from(request.duration_seconds) * REFERENCE_SPEED_CMPS) as f64;
    let circuit = request.mode == Mode::Circuit;
    let mut out = Suggestion { mode: request.mode.clone(), start_direction: [0, 1_000_000], gates: vec![], roads: vec![], length_cm: 0,
        target_cm: target as u64, diagnostics: vec![], applicable: false };
    let positions: BTreeMap<&str, Vertex> = d.nodes.iter().map(|n| (n.id.as_str(), n.position)).collect();
    let usable: Vec<usize> = (0..d.roads.len()).filter(|&i| {
        let r = &d.roads[i];
        r.points.len() >= 2 && r.from != r.to && positions.contains_key(r.from.as_str()) && positions.contains_key(r.to.as_str())
            && r.widths_cm.iter().all(|&w| w >= request.min_width_cm)
    }).collect();
    let mut arms: BTreeMap<&str, Vec<(usize, bool)>> = BTreeMap::new();
    for &i in &usable {
        arms.entry(d.roads[i].from.as_str()).or_default().push((i, true));
        arms.entry(d.roads[i].to.as_str()).or_default().push((i, false));
    }
    // Nearest centreline projection; ties prefer the lower road ID and segment.
    let mut nearest: Option<(f64, &str, usize, usize, f64)> = None;
    for &i in &usable {
        cancellation::checkpoint()?;
        let r = &d.roads[i];
        for (s, w) in r.points.windows(2).enumerate() {
            let [dx, dz] = h(w[0], w[1]);
            let n = dx * dx + dz * dz;
            let t = if n == 0.0 { 0.0 } else { (((request.start_cm[0] - w[0][0]) as f64 * dx + (request.start_cm[1] - w[0][2]) as f64 * dz) / n).clamp(0.0, 1.0) };
            let gap = ((request.start_cm[0] - w[0][0]) as f64 - t * dx).hypot((request.start_cm[1] - w[0][2]) as f64 - t * dz);
            if nearest.as_ref().is_none_or(|b| (gap, r.id.as_str(), s) < (b.0, b.1, b.2)) { nearest = Some((gap, r.id.as_str(), i, s, t)); }
        }
    }
    let Some((_, _, start_road, segment, t)) = nearest.filter(|n| n.0 <= START_REACH_CM) else {
        out.issue("E_ROAD_COURSE_START", "Choose a start on a drivable road", true, None, None);
        return Ok(out);
    };
    let r = &d.roads[start_road];
    let (a, b) = (r.points[segment], r.points[segment + 1]);
    let start: Vertex = std::array::from_fn(|k| a[k] + libm::round((b[k] - a[k]) as f64 * t) as i64);
    let along = h(a, b);
    let forward = request.heading == [0, 0] || along[0] * request.heading[0] as f64 + along[1] * request.heading[1] as f64 >= 0.0;
    // Ahead: start → travel end node. Behind: other node → start (closes a circuit).
    let (ahead, behind, end, home) = if forward {
        ([vec![start], r.points[segment + 1..].to_vec()].concat(), [r.points[..=segment].to_vec(), vec![start]].concat(), r.to.clone(), r.from.clone())
    } else {
        let mut ahead = r.points[..=segment].to_vec(); ahead.push(start); ahead.reverse();
        let mut behind = vec![start]; behind.extend_from_slice(&r.points[segment + 1..]); behind.reverse();
        (ahead, behind, r.from.clone(), r.to.clone())
    };
    let first = Leg { road: start_road, points: ahead, end: end.clone() };
    let mut search = Search { d, arms, positions, target, circuit, home: home.clone(), excluded: start_road, expansions: 0, exhausted: false, best: None };
    let mut route = Route { length: polyline(&first.points), legs: vec![first] };
    let tail = polyline(&behind);
    if !circuit { search.offer(&route); }
    let mut visited: BTreeSet<String> = [end.clone(), home.clone()].into_iter().collect();
    search.walk(&mut route, &mut visited, tail)?;
    let Some(mut best) = search.best.take() else {
        let message = if search.exhausted { "No loop found within the search budget; try another start or duration" } else { "No loop returns to this start; try another start or duration" };
        out.issue("E_ROAD_COURSE_ROUTE", message, true, Some(&r.id), Some(start));
        return Ok(out);
    };
    if circuit { best.legs.push(Leg { road: start_road, points: behind, end: home }); }
    else if best.length > target {
        // Finish inside the last road at the requested length.
        let leg = best.legs.last_mut().unwrap();
        let mut left = target - (best.length - polyline(&leg.points));
        let mut cut = vec![leg.points[0]];
        for w in leg.points.clone().windows(2) {
            let l = len3(w[0], w[1]);
            if l >= left && l > 0.0 {
                cut.push(std::array::from_fn(|k| w[0][k] + libm::round((w[1][k] - w[0][k]) as f64 * left / l) as i64));
                break;
            }
            left -= l; cut.push(w[1]);
        }
        leg.points = cut;
        best.length = target;
    }
    if (best.length - target).abs() > target * 0.25 {
        out.issue("W_ROAD_COURSE_LENGTH", format!("Route is {} m for a {} m target", (best.length / 100.0).round(), (target / 100.0).round()), false, None, None);
    }
    // Candidates: start, finish, turning junctions, then even fill. Earlier wins on overlap.
    let width = |road: usize| d.roads[road].widths_cm.iter().copied().max().unwrap_or(100);
    let mut chain: Vec<(f64, Vertex, usize)> = Vec::new();
    let mut at = 0.0;
    let mut turns: Vec<(f64, Vertex, u32, String)> = Vec::new();
    for (i, leg) in best.legs.iter().enumerate() {
        for (j, &p) in leg.points.iter().enumerate() {
            if j > 0 { at += len3(leg.points[j - 1], p); }
            if j > 0 || i == 0 { chain.push((at, p, leg.road)); }
        }
        if let Some(next) = best.legs.get(i + 1) {
            if angle(entry(&leg.points), exit(&next.points)) >= TURN_GATE_DEG {
                let r = &d.roads[next.road];
                turns.push((at, *leg.points.last().unwrap(), width(leg.road).max(width(next.road)), r.id.clone()));
            }
        }
    }
    let total = at;
    let mut keyed: Vec<(f64, Checkpoint, bool)> = vec![(0.0, gate(start, width(start_road), &r.id), true)];
    let finish = chain.last().copied().unwrap();
    if !circuit { keyed.push((total, gate(finish.1, width(finish.2), &d.roads[finish.2].id), true)); }
    for (distance, position, w, road) in &turns { keyed.push((*distance, gate(*position, *w, road), true)); }
    let spacing = MIN_SPACING_CM.max(total / 48.0);
    let mut fill = vec![];
    let mut marks: Vec<f64> = keyed.iter().map(|k| k.0).collect();
    if circuit { marks.push(total); }
    marks.sort_by(f64::total_cmp);
    for w in marks.windows(2) {
        let parts = ((w[1] - w[0]) / spacing).floor() as usize;
        for k in 1..parts.max(1) {
            let target = w[0] + (w[1] - w[0]) * k as f64 / parts as f64;
            let index = chain.partition_point(|c| c.0 < target).clamp(1, chain.len() - 1);
            let (a, b) = (chain[index - 1], chain[index]);
            let f = if b.0 > a.0 { (target - a.0) / (b.0 - a.0) } else { 0.0 };
            let p: Vertex = std::array::from_fn(|i| a.1[i] + libm::round((b.1[i] - a.1[i]) as f64 * f) as i64);
            fill.push((target, gate(p, width(b.2), &d.roads[b.2].id), false));
        }
    }
    keyed.extend(fill);
    let mut accepted: Vec<(f64, Checkpoint)> = Vec::new();
    for (distance, cp, required) in keyed {
        cancellation::checkpoint()?;
        if accepted.iter().any(|(_, other)| other.overlaps(&cp)) {
            if required { out.issue("W_ROAD_COURSE_CHECKPOINT_OVERLAP", "Overlapping checkpoint omitted", false, Some(&cp.surface_id), Some(cp.position_cm)); }
            continue;
        }
        if accepted.len() >= MAX_GATES {
            if required { out.issue("E_ROAD_COURSE_CHECKPOINT_LIMIT", "Route needs more than 64 checkpoints; shorten it", true, Some(&cp.surface_id), Some(cp.position_cm)); }
            continue;
        }
        accepted.push((distance, cp));
    }
    accepted.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.gates = accepted.into_iter().map(|(_, cp)| cp).collect();
    let direction = exit(&best.legs[0].points);
    let norm = direction[0].hypot(direction[1]);
    if norm > 0.0 { out.start_direction = [libm::round(direction[0] / norm * 1e6) as i64, libm::round(direction[1] / norm * 1e6) as i64]; }
    out.roads = best.legs.iter().map(|l| d.roads[l.road].id.clone()).collect();
    out.length_cm = total.round() as u64;
    if out.gates.len() < 2 {
        out.issue("E_ROAD_COURSE_CHECKPOINT_COUNT", "Route needs at least two non-overlapping checkpoints", true, Some(&r.id), Some(start));
    }
    if out.gates.iter().any(|g| !d.bounds.contains([g.position_cm[0], g.position_cm[2]])) {
        out.issue("E_ROAD_COURSE_BOUNDS", "A checkpoint lies outside the map bounds", true, None, None);
    }
    out.applicable = !out.diagnostics.iter().any(|d| d.blocking);
    Ok(out)
}
