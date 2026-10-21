//! Corner shortcuts made of ordinary roads: a straight chord across the inside of a
//! turning junction, joined by splitting the two course roads at new graph nodes.
//! Where buildings block a ground chord, an elevated chord ramps over them.
use super::*;
use crate::road_design::TerrainPolicy;
use crate::RoadNode;

pub const MAX_SHORTCUTS: u8 = 4;
const MIN_TURN_DEG: f64 = 60.0;
/// Distances from the junction tried for both ends, longest first (1:8 city blocks).
const REACH_CM: [f64; 3] = [1_500.0, 1_000.0, 600.0];
/// Split points keep this much road before the far end of each host road.
const END_MARGIN_CM: f64 = 200.0;
/// Extra horizontal clearance from buildings, water and other roads.
const CLEAR_CM: f64 = 30.0;
/// Elevated chords need room for both ramps, so they also try longer reaches.
const ELEVATED_REACH_CM: [f64; 5] = [4_000.0, 3_000.0, 2_000.0, 1_500.0, 1_000.0];
/// Full deck height starts this far beyond the corridor reach of a covered building.
const DECK_MARGIN_CM: f64 = 50.0;
/// Shortest flat deck between the two ramps.
const MIN_DECK_CM: f64 = 100.0;
/// Shortest ground stub between a split junction and an elevated chord.
const STUB_CM: f64 = 300.0;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    /// Straight ground chord.
    #[default]
    Ground,
    /// Elevated chord over buildings with gentle ramps (at most 12% grade).
    Ramp,
    /// Elevated chord with short steep ramps (at most 30% grade); fast cars may lift
    /// off its crests and land on the deck or the descent.
    Crest,
}
impl Form {
    /// Largest grade and the ramp handle fractions at the low and the high end.
    fn profile(self) -> (f64, f64, f64) {
        match self { Form::Crest => (0.30, 0.45, 0.10), _ => (0.12, 1.0 / 3.0, 1.0 / 3.0) }
    }
    /// Run per unit rise that keeps the steepest point of a ramp span at the grade.
    fn run_per_rise(self) -> f64 {
        let (grade, low, high) = self.profile();
        // Bezier (0,0) (low,0) (1-high,1) (1,1) in (run, rise).
        let steepest = (1..1_000).map(|i| f64::from(i) / 1_000.0).map(|t| {
            let u = 1.0 - t;
            let run = 3.0 * (u * u * low + 2.0 * u * t * (1.0 - high - low) + t * t * high);
            6.0 * u * t / run
        }).fold(0.0, f64::max);
        // Aim one point below the limit so centimetre rounding never exceeds it.
        steepest / (grade - 0.01)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    pub id: String,
    #[serde(default)]
    pub form: Form,
    /// Exact current records that the shortcut splits; apply refuses stale ones.
    pub replaces: Vec<Road>,
    /// `[kept, new]` halves per replaced road, then the ground chord, or the entry
    /// ground stub, the elevated chord and the exit ground stub.
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
/// Whether a solid ring comes within `reach` of the horizontal chord `p`–`q`.
fn blocks(ring: &[Point], p: [f64; 2], q: [f64; 2], reach: f64) -> bool {
    if ring.len() < 3 { return false; }
    let (lo, hi) = ([p[0].min(q[0]), p[1].min(q[1])], [p[0].max(q[0]), p[1].max(q[1])]);
    let outside = |axis: usize| ring.iter().all(|v| (v[axis] as f64) < lo[axis] - reach) || ring.iter().all(|v| (v[axis] as f64) > hi[axis] + reach);
    if outside(0) || outside(1) { return false; }
    if inside(p, ring) || inside(q, ring) || inside([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], ring) { return true; }
    (0..ring.len()).any(|i| {
        let (s, e) = (ring[i], ring[(i + 1) % ring.len()]);
        segment_gap(p, q, [s[0] as f64, s[1] as f64], [e[0] as f64, e[1] as f64]) < reach
    })
}
/// The chord keeps half its width plus clearance from water and roads other than its hosts.
fn clear_of_roads(d: &MapDocument, a: Vertex, b: Vertex, width: u32, hosts: &[&str]) -> bool {
    let (p, q) = ([a[0] as f64, a[2] as f64], [b[0] as f64, b[2] as f64]);
    let half = f64::from(width) / 2.0 + CLEAR_CM;
    if d.water_bodies.iter().any(|water| blocks(&water.polygon, p, q, half)) { return false; }
    for r in &d.roads {
        if hosts.contains(&r.id.as_str()) { continue; }
        for (s, w) in r.points.windows(2).enumerate() {
            let reach = half + f64::from(r.widths_cm.get(s).copied().unwrap_or(0)) / 2.0;
            if segment_gap(p, q, [w[0][0] as f64, w[0][2] as f64], [w[1][0] as f64, w[1][2] as f64]) < reach { return false; }
        }
    }
    true
}
/// A ground chord also keeps that clearance from every building footprint.
fn clear(d: &MapDocument, a: Vertex, b: Vertex, width: u32, hosts: &[&str]) -> bool {
    let (p, q) = ([a[0] as f64, a[2] as f64], [b[0] as f64, b[2] as f64]);
    let half = f64::from(width) / 2.0 + CLEAR_CM;
    !d.buildings.iter().any(|building| blocks(&building.footprint, p, q, half)) && clear_of_roads(d, a, b, width, hosts)
}
/// Ground point `distance` from `a` horizontally toward `b`, at `a`'s height.
fn toward(a: Vertex, b: Vertex, distance: f64) -> Vertex {
    let [dx, dz] = h(a, b);
    let length = dx.hypot(dz);
    [a[0] + libm::round(dx * distance / length) as i64, a[1], a[2] + libm::round(dz * distance / length) as i64]
}
/// Height of a straight sampled chord at horizontal distance `s` from its start.
fn height_at(points: &[Vertex], s: f64) -> f64 {
    let mut left = s.max(0.0);
    for w in points.windows(2) {
        let l = h(w[0], w[1])[0].hypot(h(w[0], w[1])[1]);
        if l >= left && l > 0.0 { return w[0][1] as f64 + (w[1][1] - w[0][1]) as f64 * left / l; }
        left -= l;
    }
    points.last().unwrap()[1] as f64
}
/// Elevated chord from `a` to `b` (on `chord`'s ends): one flat deck at the shared
/// roof-to-deck gap above the tallest building the corridor covers, and ramps that
/// already clear each lower building they pass. The chord stays away from other
/// roads, so it needs no separated crossing.
fn elevated(d: &MapDocument, chord: &Road, width: u32, hosts: &[&str], form: Form) -> Option<Road> {
    let (a, b) = (chord.points[0], *chord.points.last().unwrap());
    let (p, q) = ([a[0] as f64, a[2] as f64], [b[0] as f64, b[2] as f64]);
    let length = (q[0] - p[0]).hypot(q[1] - p[1]);
    if length <= 0.0 || !clear_of_roads(d, a, b, width, hosts) { return None; }
    let axis = [(q[0] - p[0]) / length, (q[1] - p[1]) / length];
    let half = f64::from(width) / 2.0 + CLEAR_CM;
    // (first and last covered distance along the chord, required deck height)
    let covered: Vec<(f64, f64, i64)> = d.buildings.iter().filter_map(|building| {
        let (t0, t1) = crate::placement::corridor_extent(&building.footprint, [a[0], a[2]], [b[0], b[2]], half)?;
        let top = building.base_cm + i64::from(building.height_cm) + crate::placement::roof_rise(building)
            + crate::placement::BUILDING_DECK_CLEARANCE_CM + 1;
        Some((t0 * length - DECK_MARGIN_CM, t1 * length + DECK_MARGIN_CM, top))
    }).collect();
    let deck = covered.iter().map(|c| c.2).max()?;
    // Level leads keep each ground/structure junction apron on the terrain: the road
    // plan joins a designed road at its first sample beyond 1.5 widths + 2 m.
    let lead = f64::from(width) * 1.5 + 250.0;
    let run = form.run_per_rise();
    let (up, down) = (run * (deck - a[1]) as f64, run * (deck - b[1]) as f64);
    let (rise, top, fall) = (lead + up, length - lead - down, length - lead);
    if up <= 0.0 || down <= 0.0 || top - rise < MIN_DECK_CM { return None; }
    let (_, low, high) = form.profile();
    let at = |s: f64, y: i64| -> Vertex { [a[0] + libm::round(axis[0] * s) as i64, y, a[2] + libm::round(axis[1] * s) as i64] };
    let level = |from: f64, to: f64, y: i64| [at(from + (to - from) / 3.0, y), at(from + (to - from) * 2.0 / 3.0, y), at(to, y)];
    let mut control_points = vec![a];
    control_points.extend(level(0.0, lead, a[1]));
    control_points.extend([at(lead + low * up, a[1]), at(rise - high * up, deck), at(rise, deck)]);
    control_points.extend(level(rise, top, deck));
    control_points.extend([at(top + high * down, deck), at(fall - low * down, b[1]), at(fall, b[1])]);
    control_points.extend(level(fall, length, b[1]));
    *control_points.last_mut().unwrap() = b;
    let mut road = chord.clone();
    crate::profile::runtime(|| crate::road_design::compile(&mut road, Design { control_points, terrain_policy: TerrainPolicy::Elevated, shoulder_cm: 2400 }, width)).ok()?;
    // Ramps rise and fall monotonically, so each building's lowest deck is at an end.
    covered.iter().all(|&(lo, hi, top)| [lo, hi].iter().all(|&s| height_at(&road.points, s.clamp(0.0, length)) >= top as f64))
        .then_some(road)
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

/// Seeded order of the two elevated forms for the `index`-th shortcut.
fn elevated_forms(seed: u64, index: usize) -> [Form; 2] {
    let hash = crate::sha256(format!("{seed}:elevated:{index}").as_bytes());
    if usize::from_str_radix(&hash[..2], 16).unwrap() % 2 == 0 { [Form::Ramp, Form::Crest] } else { [Form::Crest, Form::Ramp] }
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
        let host_ids = [d.roads[inn.road].id.as_str(), d.roads[next.road].id.as_str()];
        // Ground chords first; where buildings block them, the seeded elevated forms.
        let attempts = REACH_CM.iter().map(|&r| (Form::Ground, r))
            .chain(elevated_forms(request.seed, out.shortcuts.len()).into_iter().flat_map(|f| ELEVATED_REACH_CM.iter().map(move |&r| (f, r))));
        for (form, reach) in attempts {
            cancellation::checkpoint()?;
            let (before, after) = (polyline(&inn.points), polyline(&next.points));
            if before < reach + END_MARGIN_CM || after < reach + END_MARGIN_CM { continue; }
            let (a, b) = (point_at(&inn.points, before - reach), point_at(&next.points, reach));
            if form == Form::Ground && !clear(&out.document, a, b, width, &host_ids) { continue; }
            let key = format!("{}:{}:{}:{reach}", d.map_id, host_ids[0], host_ids[1]);
            let base = crate::sha256(if form == Form::Ground { key } else { format!("{key}:{form:?}") }.as_bytes())[..12].to_string();
            let id = format!("shortcut-{base}");
            let (entry_node, exit_node) = (format!("{id}-entry"), format!("{id}-exit"));
            let second = |road: &Road| format!("{}-{}", road.id.chars().take(100).collect::<String>(), &base[..8]);
            let (lift_node, drop_node) = (format!("{id}-up"), format!("{id}-down"));
            let (stub_in, stub_out) = (format!("{id}-in"), format!("{id}-out"));
            if out.document.roads.iter().any(|r| [&id, &second(&d.roads[inn.road]), &second(&d.roads[next.road]), &stub_in, &stub_out].contains(&&r.id))
                || out.document.nodes.iter().any(|n| [&entry_node, &exit_node, &lift_node, &drop_node].contains(&&n.id)) { continue; }
            let Some((ia, ib)) = split(&d.roads[inn.road], a, &entry_node, &second(&d.roads[inn.road])) else { continue; };
            let Some((oa, ob)) = split(&d.roads[next.road], b, &exit_node, &second(&d.roads[next.road])) else { continue; };
            let ground = |id: &str, from: &str, to: &str, points: Vec<Vertex>| Road { design: None, snow_retention_percent: d.roads[inn.road].snow_retention_percent,
                id: id.into(), from: from.into(), to: to.into(), points, widths_cm: vec![width],
                surfaces: vec![d.roads[inn.road].surfaces.first().copied().unwrap_or(crate::Surface::Asphalt)],
                kind: crate::RoadKind::Ground, clearance_cm: None, sidewalk_cm: None, markings: None };
            let mut nodes = vec![RoadNode { id: entry_node.clone(), position: a, level }, RoadNode { id: exit_node.clone(), position: b, level }];
            let mut roads = vec![ia, ib, oa, ob];
            if form == Form::Ground {
                roads.push(ground(&id, &entry_node, &exit_node, vec![a, b]));
            } else {
                // Short ground stubs keep the split junctions ordinary; the structure
                // joins each stub end-on, where every approach mouth stays separate.
                // Undesigned ground roads take the terrain height, which a proposal can only
                // know when the map has no heightmaps.
                if !d.heightmaps.is_empty() { continue; }
                let stub = f64::from(width).max(STUB_CM);
                let base = |mut p: Vertex| { p[1] = d.terrain_base_cm; p };
                let (lift, drop) = (base(toward(a, b, stub)), base(toward(b, a, stub)));
                if !clear(&out.document, a, lift, width, &host_ids) || !clear(&out.document, drop, b, width, &host_ids) { continue; }
                let Some(chord) = elevated(&out.document, &ground(&id, &lift_node, &drop_node, vec![lift, drop]), width, &host_ids, form) else { continue; };
                roads.extend([ground(&stub_in, &entry_node, &lift_node, vec![a, lift]), chord, ground(&stub_out, &drop_node, &exit_node, vec![drop, b])]);
                nodes.extend([RoadNode { id: lift_node.clone(), position: lift, level }, RoadNode { id: drop_node.clone(), position: drop, level }]);
            }
            let shortcut = Shortcut { id, form, replaces: vec![d.roads[inn.road].clone(), d.roads[next.road].clone()], roads, nodes };
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
