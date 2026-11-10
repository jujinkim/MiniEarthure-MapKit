//! Display-only street-life lanes of a packed cell (docs/STREET_LIFE.md):
//! walk lanes along the generated sidewalk tops (`<road>:sidewalk`), corner
//! links through connected tops, junction crossings between kerb mouths and an
//! ambience mix of the cell's ground. Presentation only: never collision,
//! never Runtime state, never part of a generated hash.
use crate::road_style::{crosswalk_centres, dot, inside, perp, scene, segment_distance, sub, unit, vertical, P2};
use godot::prelude::*;
use mapkit_core::{Bounds, MapDocument, RoadPaint};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Scene metres (x, y, z).
type P3 = [f64; 3];

/// Per-cell caps: walk points (lanes and links), lane and link entries, crossings.
pub struct Caps { pub points: usize, pub entries: usize, pub crossings: usize }
pub const CAPS: Caps = Caps { points: 2048, entries: 512, crossings: 256 };
/// Exact presentation bytes: 12 B per point, 16 B per lane or link, 24 B per
/// crossing (two mouths) and 1 B for its signal flag, 16 B for `ambience`.
pub const POINT_BYTES: u64 = 12;
pub const ENTRY_BYTES: u64 = 16;
pub const CROSSING_BYTES: u64 = 25;
pub const AMBIENCE_BYTES: u64 = 16;

/// Lane sampling (scene metres): probe step across, sample step along, the
/// longest simplified span, the turn kept as a vertex, the shortest run.
const PROBE_STEP: f64 = 0.1;
const SAMPLE_STEP: f64 = 1.0;
const SPAN: f64 = 4.0;
const TURN_DEGREES: f64 = 10.0;
const SHORTEST_RUN: f64 = 2.0;
/// Corner links join lane ends of different roads on one connected top within
/// LINK_REACH; ends within MERGE (the field's endpoint merge) need no link.
const LINK_REACH: f64 = 6.0;
const MERGE: f64 = 1.5;
/// Kerb mouth past the carriageway half width; a mouth joins the nearest lane
/// within MOUTH_REACH; a signal head within SIGNAL_REACH of the crossing line
/// carried on over both sidewalks (the importer's poles stand past the kerb)
/// marks the crossing.
const MOUTH_OUT: f64 = 0.15;
const MOUTH_REACH: f64 = 2.5;
const SIGNAL_REACH: f64 = 3.0;
/// Walkers per map metre of lane: base, bus shelter, subway entrance or
/// station sign, crosswalk mouth, x commercial footprint share, food or shop
/// POI; clamped to DENSITY_MAX.
const DENSITY_BASE: f64 = 0.06;
const DENSITY_SHELTER: f64 = 0.06;
const DENSITY_STATION: f64 = 0.09;
const DENSITY_CROSSWALK: f64 = 0.03;
const DENSITY_COMMERCIAL: f64 = 0.06;
const DENSITY_POI: f64 = 0.03;
const DENSITY_MAX: f64 = 0.30;
/// Footfall radii in source metres (÷ the map scale 1:D, D = 3 without one).
const SHELTER_SOURCE_M: f64 = 40.0;
const STATION_SOURCE_M: f64 = 150.0;
const CROSSWALK_SOURCE_M: f64 = 30.0;
const COMMERCIAL_SOURCE_M: f64 = 40.0;
const DEFAULT_DENOMINATOR: f64 = 3.0;
/// Water share samples per cell axis.
const WATER_SAMPLES: usize = 16;

#[derive(Default, Debug)]
pub struct WalkLanes {
    pub points: Vec<P3>,
    /// (first point, count, half width m, walkers per map metre).
    pub lanes: Vec<[f64; 4]>,
    pub links: Vec<[f64; 4]>,
    /// Two kerb mouths per crossing.
    pub crossings: Vec<P3>,
    pub signals: Vec<u8>,
    /// Area shares of the cell: grass, asphalt, building, water.
    pub ambience: Option<[f64; 4]>,
    /// (requested, cap) of the first quantity over its cap.
    pub overflow: Option<(u64, u64)>,
}

pub fn bytes(points: usize, entries: usize, crossings: usize, ambience: bool) -> u64 {
    points as u64 * POINT_BYTES + entries as u64 * ENTRY_BYTES + crossings as u64 * CROSSING_BYTES + if ambience { AMBIENCE_BYTES } else { 0 }
}

impl WalkLanes {
    /// The bytes `presentation_bytes` counts once these lanes are published.
    #[cfg(test)]
    pub fn bytes(&self) -> u64 {
        bytes(self.points.len(), self.lanes.len() + self.links.len(), self.signals.len(), self.ambience.is_some())
    }
    /// Over-cap lanes are dropped: a display degradation; Authoring scopes warn.
    pub fn note(&self) {
        let Some((actual, limit)) = self.overflow else { return };
        mapkit_core::profile::note(mapkit_core::Warning {
            code: "walk_lane_points".into(),
            actual,
            limit,
            message: "Street-life walk lanes over the per-cell cap are omitted in this cell".into(),
            runtime_rejects: false,
        });
    }
    pub fn publish(&self, presentation: &mut VarDictionary) {
        if let Some(a) = self.ambience {
            presentation.set("ambience", Vector4::new(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32));
        }
        if self.lanes.is_empty() { return; }
        let v3 = |p: &P3| Vector3::new(p[0] as f32, p[1] as f32, p[2] as f32);
        let v4 = |e: &[f64; 4]| Vector4::new(e[0] as f32, e[1] as f32, e[2] as f32, e[3] as f32);
        presentation.set("walk_points", &self.points.iter().map(v3).collect::<PackedVector3Array>());
        presentation.set("walk_lanes", &self.lanes.iter().map(v4).collect::<PackedVector4Array>());
        presentation.set("walk_links", &self.links.iter().map(v4).collect::<PackedVector4Array>());
        presentation.set("walk_crossings", &self.crossings.iter().map(v3).collect::<PackedVector3Array>());
        presentation.set("walk_crossing_signals", &PackedByteArray::from(self.signals.as_slice()));
    }
}

/// Exact bytes of the street-life keys a presentation carries (decorate).
pub fn presentation_bytes(presentation: &VarDictionary) -> u64 {
    let len = |key: &str| -> usize {
        presentation.get(key).map_or(0, |v| {
            v.try_to::<PackedVector3Array>().map(|a| a.len())
                .or_else(|_| v.try_to::<PackedVector4Array>().map(|a| a.len()))
                .or_else(|_| v.try_to::<PackedByteArray>().map(|a| a.len()))
                .unwrap_or(0)
        })
    };
    bytes(len("walk_points"), len("walk_lanes") + len("walk_links"), len("walk_crossing_signals"), presentation.contains_key("ambience"))
}

struct Top { corners: [P3; 3], flat: [P2; 3], road: String }
impl Top {
    fn height(&self, p: P2) -> f64 {
        let [a, b, c] = self.flat;
        let (e, f, g) = (sub(b, a), sub(c, a), sub(p, a));
        let det = e[0] * f[1] - e[1] * f[0];
        if det.abs() < 1e-12 { return self.corners[0][1]; }
        let u = (g[0] * f[1] - g[1] * f[0]) / det;
        let v = (e[0] * g[1] - e[1] * g[0]) / det;
        self.corners[0][1] + u * (self.corners[1][1] - self.corners[0][1]) + v * (self.corners[2][1] - self.corners[0][1])
    }
    fn centroid(&self) -> P3 {
        let c = self.corners;
        [(c[0][0] + c[1][0] + c[2][0]) / 3.0, (c[0][1] + c[1][1] + c[2][1]) / 3.0, (c[0][2] + c[1][2] + c[2][2]) / 3.0]
    }
}

/// Sidewalk tops of the cell on a 2 m grid, with their connected components
/// (tops sharing a vertex: corner aprons belong to one road's faces).
struct Tops { tops: Vec<Top>, component: Vec<usize>, grid: HashMap<(i64, i64), Vec<usize>> }
const GRID: f64 = 2.0;
fn grid_cell(p: P2) -> (i64, i64) { ((p[0] / GRID).floor() as i64, (p[1] / GRID).floor() as i64) }
impl Tops {
    fn new(roads: &BTreeMap<&str, &mapkit_core::Road>, ids: &[&str], corners: &[[[i64; 3]; 3]]) -> Tops {
        let mut tops = Vec::new();
        let mut keys: Vec<[[i64; 3]; 3]> = Vec::new();
        for (i, v) in corners.iter().enumerate() {
            let Some(road) = ids[i].strip_suffix(":sidewalk").filter(|r| roads.contains_key(r)) else { continue };
            if vertical(v) { continue; }
            let c = v.map(|p| [p[0] as f64 / 100.0, p[1] as f64 / 100.0, -(p[2] as f64) / 100.0]);
            tops.push(Top { corners: c, flat: v.map(scene), road: road.to_string() });
            keys.push(*v);
        }
        let mut parent: Vec<usize> = (0..tops.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i { parent[i] = parent[parent[i]]; i = parent[i]; }
            i
        }
        let mut first = HashMap::<[i64; 3], usize>::new();
        for (i, v) in keys.iter().enumerate() {
            for p in v {
                let j = *first.entry(*p).or_insert(i);
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b { parent[a.max(b)] = a.min(b); }
            }
        }
        let component = (0..tops.len()).map(|i| root(&mut parent, i)).collect();
        let mut grid = HashMap::<(i64, i64), Vec<usize>>::new();
        for (i, t) in tops.iter().enumerate() {
            let p = t.flat;
            let lo = grid_cell([p[0][0].min(p[1][0]).min(p[2][0]), p[0][1].min(p[1][1]).min(p[2][1])]);
            let hi = grid_cell([p[0][0].max(p[1][0]).max(p[2][0]), p[0][1].max(p[1][1]).max(p[2][1])]);
            if (hi.0 - lo.0 + 1) * (hi.1 - lo.1 + 1) > 4096 { continue; }
            for x in lo.0..=hi.0 { for y in lo.1..=hi.1 { grid.entry((x, y)).or_default().push(i); } }
        }
        Tops { tops, component, grid }
    }
    /// The top under `p` (the highest where tops overlap) with its height.
    fn at(&self, p: P2) -> Option<(usize, f64)> { self.of(p, None) }
    /// Likewise among the tops of `road` only, or of every road.
    fn of(&self, p: P2, road: Option<&str>) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for &i in self.grid.get(&grid_cell(p))? {
            if road.is_some_and(|r| self.tops[i].road != r) || !inside(self.tops[i].flat, p) { continue; }
            let y = self.tops[i].height(p);
            if best.is_none_or(|b| y > b.1) { best = Some((i, y)); }
        }
        best
    }
}

/// A lane or link polyline; `ends` holds the top components of its ends.
#[derive(Clone)]
struct Run { points: Vec<P3>, half: f64, density: f64, road: String, ends: [usize; 2] }

fn flat(p: P3) -> P2 { [p[0], p[2]] }
fn distance(a: P3, b: P3) -> f64 { let d = sub(flat(a), flat(b)); dot(d, d).sqrt() }
fn length(points: &[P3]) -> f64 { points.windows(2).map(|w| distance(w[0], w[1])).sum() }
fn lerp(a: P3, b: P3, t: f64) -> P3 { std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t) }

/// One vertex per SPAN or less, and at every turn above TURN_DEGREES.
fn simplify(samples: &[P3]) -> Vec<P3> {
    if samples.len() <= 2 { return samples.to_vec(); }
    let turn = TURN_DEGREES.to_radians().cos();
    let mut kept = vec![samples[0]];
    for i in 1..samples.len() - 1 {
        let (a, b) = (unit(sub(flat(samples[i]), flat(samples[i - 1]))), unit(sub(flat(samples[i + 1]), flat(samples[i]))));
        if dot(a, b) < turn || distance(*kept.last().unwrap(), samples[i + 1]) > SPAN { kept.push(samples[i]); }
    }
    kept.push(*samples.last().unwrap());
    kept
}

fn point_at(points: &[P3], at: f64) -> P3 {
    let mut left = at;
    for w in points.windows(2) {
        let l = distance(w[0], w[1]);
        if left <= l && l > 0.0 { return lerp(w[0], w[1], left / l); }
        left -= l;
    }
    *points.last().unwrap()
}

fn area(points: &[[i64; 2]]) -> f64 {
    let n = points.len();
    (0..n).map(|i| { let (a, b) = (points[i], points[(i + 1) % n]); a[0] as f64 * b[1] as f64 - b[0] as f64 * a[1] as f64 }).sum::<f64>().abs() * 0.5
}
fn centroid(points: &[[i64; 2]]) -> [f64; 2] {
    let n = points.len().max(1) as f64;
    [points.iter().map(|p| p[0] as f64).sum::<f64>() / n, points.iter().map(|p| p[1] as f64).sum::<f64>() / n]
}

/// Footfall sources around the cell in map centimetres.
struct Footfall { shelters: Vec<[f64; 2]>, stations: Vec<[f64; 2]>, shops: Vec<[f64; 2]>, buildings: Vec<([f64; 2], f64, bool)>, signals: Vec<P2>, radius: [f64; 4] }
impl Footfall {
    fn new(d: &MapDocument, bounds: &Bounds) -> Footfall {
        let denominator = mapkit_core::osm_source_denominator(d).map_or(DEFAULT_DENOMINATOR, |v| v as f64);
        let radius = [SHELTER_SOURCE_M, STATION_SOURCE_M, CROSSWALK_SOURCE_M, COMMERCIAL_SOURCE_M].map(|m| m * 100.0 / denominator);
        // Signal heads stand up to the widest kerb plus SIGNAL_REACH from a crossing.
        let kerb = d.roads.iter().map(|r| r.widths_cm.iter().copied().max().unwrap_or(0) as f64 / 2.0 + d.sidewalk_width_cm(r) as f64).fold(0.0, f64::max);
        let reach = radius.iter().copied().fold(SIGNAL_REACH * 100.0 + kerb, f64::max);
        let near = |p: [f64; 2]| p[0] >= bounds.min[0] as f64 - reach && p[0] <= bounds.max[0] as f64 + reach && p[1] >= bounds.min[1] as f64 - reach && p[1] <= bounds.max[1] as f64 + reach;
        let mut f = Footfall { shelters: vec![], stations: vec![], shops: vec![], buildings: vec![], signals: vec![], radius };
        for p in &d.placements {
            let at = [p.position[0] as f64, p.position[2] as f64];
            if !near(at) { continue; }
            match p.asset_id.as_str() {
                "builtin:bus_shelter" => f.shelters.push(at),
                "builtin:subway_entrance" | "builtin:station_sign" => f.stations.push(at),
                "builtin:signal_h" | "builtin:signal_v" => f.signals.push([at[0] / 100.0, -at[1] / 100.0]),
                _ => {}
            }
        }
        for poi in &d.pois {
            let at = [poi.position[0] as f64, poi.position[1] as f64];
            if near(at) && matches!(poi.category.as_str(), "food" | "shop" | "shopping") { f.shops.push(at); }
        }
        for b in &d.buildings {
            for k in 0..=b.parts.len() {
                let footprint = if k == 0 { &b.footprint } else { &b.parts[k - 1].footprint };
                let at = centroid(footprint);
                if near(at) { f.buildings.push((at, area(footprint), b.usage == "commercial")); }
            }
        }
        f
    }
    /// Walkers per map metre of a lane whose middle is `p` (scene metres).
    fn density(&self, p: P2, mouths: &[P2]) -> f64 {
        let m = [p[0] * 100.0, -p[1] * 100.0];
        let within = |list: &[[f64; 2]], r: f64| list.iter().any(|q| (q[0] - m[0]).powi(2) + (q[1] - m[1]).powi(2) <= r * r);
        let mut density = DENSITY_BASE;
        if within(&self.shelters, self.radius[0]) { density += DENSITY_SHELTER; }
        if within(&self.stations, self.radius[1]) { density += DENSITY_STATION; }
        let crossing = self.radius[2] / 100.0;
        if mouths.iter().any(|q| { let d = sub(*q, p); dot(d, d) <= crossing * crossing }) { density += DENSITY_CROSSWALK; }
        let (mut all, mut commercial) = (0.0, 0.0);
        for (q, a, c) in &self.buildings {
            if (q[0] - m[0]).powi(2) + (q[1] - m[1]).powi(2) > self.radius[3] * self.radius[3] { continue; }
            all += a;
            if *c { commercial += a; }
        }
        if all > 0.0 { density += DENSITY_COMMERCIAL * commercial / all; }
        if within(&self.shops, self.radius[3]) { density += DENSITY_POI; }
        density.min(DENSITY_MAX)
    }
}

/// Builds the cell's walk lanes, links, crossings and ambience from the packed
/// faces (`ids` per face, `corners` in map centimetres, `surfaces` as packed).
#[allow(clippy::too_many_arguments)]
pub fn build(d: &MapDocument, paint: &RoadPaint, roads: &BTreeMap<&str, &mapkit_core::Road>, ids: &[&str], corners: &[[[i64; 3]; 3]], surfaces: &[u8], bounds: &Bounds, caps: &Caps) -> WalkLanes {
    let mut out = WalkLanes { ambience: Some(ambience(d, roads, ids, corners, surfaces, bounds)), ..Default::default() };
    let tops = Tops::new(roads, ids, corners);
    if tops.tops.is_empty() { return out; }
    // Lanes: the middle of each side's sidewalk band along each paint segment,
    // sampled every SAMPLE_STEP on the segment's own grid (so neighbouring
    // cells meet at one sample) over the part near this cell. The band (of the
    // road's own tops: a corner apron of the crossing road is not its band) is
    // probed at the first sample of every run and again where a sample leaves
    // the tops (a driveway, a corner gap, the cell edge), splitting the run.
    let near = |p: f64, lo: f64, hi: f64, m: f64| p >= lo - m && p <= hi + m;
    let lo = [bounds.min[0] as f64 / 100.0, -(bounds.max[1] as f64) / 100.0];
    let hi = [bounds.max[0] as f64 / 100.0, -(bounds.min[1] as f64) / 100.0];
    let mut runs: Vec<Run> = Vec::new();
    for (id, r) in roads {
        let Some(path) = paint.paths.get(*id) else { continue };
        let sidewalk = d.sidewalk_width_cm(r);
        if sidewalk == 0 { continue; }
        let width = sidewalk as f64 / 100.0;
        for side in [1.0, -1.0] {
            let mut samples: Vec<Option<(P3, usize, f64)>> = Vec::new();
            let mut last: Option<P2> = None;
            for s in path {
                let (a, b) = (scene(s.a), scene(s.b));
                let span = dot(sub(b, a), sub(b, a)).sqrt();
                if span < 0.05 { continue; }
                let (dir, n) = (unit(sub(b, a)), perp(unit(sub(b, a))));
                let curb = s.width_cm / 200.0;
                let reach = curb + width + 1.0;
                let probe = |c: P2| -> Option<(f64, f64)> {
                    let mut band: Option<(f64, f64)> = None;
                    for step in 0..=((reach - curb) / PROBE_STEP + 1e-6) as usize {
                        let o = curb + step as f64 * PROBE_STEP;
                        let hit = tops.of([c[0] + n[0] * side * o, c[1] + n[1] * side * o], Some(*id)).is_some();
                        match (hit, band) {
                            (true, None) => band = Some((o, o)),
                            (true, Some((inner, _))) => band = Some((inner, o)),
                            (false, Some(_)) => break,
                            _ => {}
                        }
                    }
                    // A corner apron widens the band: keep the sidewalk's own width.
                    band.map(|(inner, outer)| (inner, outer.min(inner + width)))
                };
                // The part of the segment whose band can reach this cell.
                let margin = reach + SAMPLE_STEP;
                let (mut t0, mut t1) = (0.0f64, span);
                for k in 0..2 {
                    if dir[k].abs() < 1e-12 {
                        if !near(a[k], lo[k], hi[k], margin) { t1 = -1.0; }
                        continue;
                    }
                    let (u, v) = ((lo[k] - margin - a[k]) / dir[k], (hi[k] + margin - a[k]) / dir[k]);
                    t0 = t0.max(u.min(v));
                    t1 = t1.min(u.max(v));
                }
                if t0 > t1 { samples.push(None); last = None; continue; }
                let steps = (span / SAMPLE_STEP).ceil() as usize;
                let (k0, k1) = (((t0 / SAMPLE_STEP).floor() as usize).min(steps), ((t1 / SAMPLE_STEP).ceil() as usize).min(steps));
                let continues = k0 == 0 && last.is_some_and(|q| { let e = sub(q, a); dot(e, e) < 1e-6 });
                if !continues { samples.push(None); }
                let on_band = |c: P2, (inner, outer): (f64, f64)| {
                    let offset = (inner + outer) * 0.5;
                    let half = ((outer - inner) * 0.5 - 0.08).max(0.10);
                    let p = [c[0] + n[0] * side * offset, c[1] + n[1] * side * offset];
                    tops.at(p).map(|(i, y)| ([p[0], y, p[1]], tops.component[i], half))
                };
                let mut band: Option<(f64, f64)> = None;
                for k in k0 + usize::from(continues)..=k1 {
                    let t = (k as f64 * SAMPLE_STEP).min(span);
                    let c = [a[0] + dir[0] * t, a[1] + dir[1] * t];
                    let fresh = band.is_none();
                    if fresh { band = probe(c); }
                    let mut sample = band.and_then(|b| on_band(c, b));
                    if sample.is_none() && !fresh {
                        // The run ends here; a new one may start on a new band.
                        samples.push(None);
                        band = probe(c);
                        sample = band.and_then(|b| on_band(c, b));
                    }
                    if sample.is_none() { band = None; }
                    samples.push(sample);
                }
                last = if k1 == steps { Some(b) } else { None };
            }
            for group in samples.split(Option::is_none) {
                let list: Vec<(P3, usize, f64)> = group.iter().map(|s| s.unwrap()).collect();
                if list.len() < 2 { continue; }
                let points: Vec<P3> = list.iter().map(|s| s.0).collect();
                if length(&points) < SHORTEST_RUN { continue; }
                let half = list.iter().map(|s| s.2).fold(f64::MAX, f64::min);
                runs.push(Run { points: simplify(&points), half, density: 0.0, road: id.to_string(), ends: [list[0].1, list[list.len() - 1].1] });
            }
        }
    }
    if runs.is_empty() { return out; }
    // Corner links: each lane end to the nearest end of another road on the
    // same connected top, through the top centroids nearest the chord.
    let ends: Vec<(usize, usize)> = (0..runs.len()).flat_map(|i| [(i, 0), (i, 1)]).collect();
    let end_point = |(i, e): (usize, usize)| if e == 0 { runs[i].points[0] } else { *runs[i].points.last().unwrap() };
    let mut pairs = BTreeSet::new();
    for &x in &ends {
        let mut best: Option<((usize, usize), f64)> = None;
        for &y in &ends {
            if runs[y.0].road == runs[x.0].road || runs[y.0].ends[y.1] != runs[x.0].ends[x.1] { continue; }
            let gap = distance(end_point(x), end_point(y));
            if gap > MERGE && gap <= LINK_REACH && best.is_none_or(|b| gap < b.1) { best = Some((y, gap)); }
        }
        if let Some((y, _)) = best { pairs.insert(if x < y { (x, y) } else { (y, x) }); }
    }
    let mut links: Vec<Run> = Vec::new();
    for (x, y) in pairs {
        let (a, b, component) = (end_point(x), end_point(y), runs[x.0].ends[x.1]);
        let mut points = vec![a];
        let steps = (distance(a, b) / SAMPLE_STEP).ceil() as usize;
        for k in 1..steps {
            let q = lerp(a, b, k as f64 / steps as f64);
            let p = match tops.at(flat(q)) {
                Some((i, y)) if tops.component[i] == component => [q[0], y, q[2]],
                _ => {
                    let mut nearest = (f64::MAX, q);
                    for (i, t) in tops.tops.iter().enumerate() {
                        if tops.component[i] != component { continue; }
                        let c = t.centroid();
                        let gap = distance(c, q);
                        if gap < nearest.0 { nearest = (gap, c); }
                    }
                    nearest.1
                }
            };
            if distance(p, *points.last().unwrap()) > 0.05 { points.push(p); }
        }
        if distance(b, *points.last().unwrap()) > 0.05 || points.len() == 1 { points.push(b); }
        let half = runs[x.0].half.min(runs[y.0].half);
        links.push(Run { points, half, density: 0.0, road: String::new(), ends: [component, component] });
    }
    let footfall = Footfall::new(d, bounds);
    // Crossings: kerb mouths on the sidewalk tops either side of each painted
    // crosswalk, each joined to the nearest lane by a short link.
    let mut crossings: Vec<(P3, P3, u8)> = Vec::new();
    for (centre, dir, half, road) in crosswalk_centres(paint, roads) {
        let across = perp(dir);
        let kerb = half + roads.get(road).map_or(0.0, |r| d.sidewalk_width_cm(r) as f64 / 100.0);
        let mouth = |s: f64| { let p = [centre[0] + across[0] * s * (half + MOUTH_OUT), centre[1] + across[1] * s * (half + MOUTH_OUT)]; tops.at(p).map(|(_, y)| [p[0], y, p[1]]) };
        let (Some(a), Some(b)) = (mouth(1.0), mouth(-1.0)) else { continue };
        let (left, right) = ([centre[0] - across[0] * kerb, centre[1] - across[1] * kerb], [centre[0] + across[0] * kerb, centre[1] + across[1] * kerb]);
        let signalled = footfall.signals.iter().any(|q| segment_distance(*q, left, right) <= SIGNAL_REACH);
        crossings.push((a, b, u8::from(signalled)));
    }
    for &(a, b, _) in &crossings {
        for m in [a, b] {
            let mut best: Option<(usize, usize, f64, f64)> = None; // run, segment, t, gap
            for (i, run) in runs.iter().enumerate() {
                for (k, w) in run.points.windows(2).enumerate() {
                    let (p, q) = (flat(w[0]), flat(w[1]));
                    let e = sub(q, p);
                    let t = (dot(sub(flat(m), p), e) / dot(e, e).max(1e-12)).clamp(0.0, 1.0);
                    let gap = distance(lerp(w[0], w[1], t), m);
                    if gap <= MOUTH_REACH && best.is_none_or(|b| gap < b.3) { best = Some((i, k, t, gap)); }
                }
            }
            let Some((i, k, t, _)) = best else { continue };
            let q = lerp(runs[i].points[k], runs[i].points[k + 1], t);
            let (near_start, near_end) = (k == 0 && distance(q, runs[i].points[0]) < 0.05, k + 2 == runs[i].points.len() && distance(q, runs[i].points[k + 1]) < 0.05);
            if !near_start && !near_end {
                // Split the lane at the mouth's foot so it becomes a node.
                let mut tail = runs[i].clone();
                tail.points = std::iter::once(q).chain(runs[i].points[k + 1..].iter().copied()).collect();
                runs[i].points.truncate(k + 1);
                if distance(q, runs[i].points[k]) > 0.05 { runs[i].points.push(q); } else { runs[i].points[k] = q; }
                if distance(tail.points[0], tail.points[1]) < 0.05 { tail.points.remove(0); tail.points[0] = q; }
                runs.push(tail);
            }
            if distance(q, m) > 0.05 {
                links.push(Run { points: vec![q, m], half: runs[i].half, density: 0.0, road: String::new(), ends: [0, 0] });
            }
        }
    }
    let mouths: Vec<P2> = crossings.iter().flat_map(|c| [flat(c.0), flat(c.1)]).collect();
    for run in runs.iter_mut() {
        let m = point_at(&run.points, length(&run.points) * 0.5);
        run.density = footfall.density(flat(m), &mouths);
    }
    // Caps: lanes first, then links, then crossings; the rest is dropped.
    let wanted = (runs.iter().chain(&links).map(|r| r.points.len()).sum::<usize>(), runs.len() + links.len(), crossings.len());
    for (list, lanes) in [(&runs, true), (&links, false)] {
        for run in list {
            if out.points.len() + run.points.len() > caps.points || out.lanes.len() + out.links.len() >= caps.entries { continue; }
            let entry = [out.points.len() as f64, run.points.len() as f64, run.half, if lanes { run.density } else { 0.0 }];
            out.points.extend(run.points.iter().copied());
            if lanes { out.lanes.push(entry) } else { out.links.push(entry) }
        }
    }
    for &(a, b, s) in crossings.iter().take(caps.crossings) {
        out.crossings.extend([a, b]);
        out.signals.push(s);
    }
    out.overflow = if wanted.0 > caps.points { Some((wanted.0 as u64, caps.points as u64)) }
        else if wanted.1 > caps.entries { Some((wanted.1 as u64, caps.entries as u64)) }
        else if wanted.2 > caps.crossings { Some((wanted.2 as u64, caps.crossings as u64)) }
        else { None };
    if out.lanes.is_empty() { out.links.clear(); out.crossings.clear(); out.signals.clear(); out.points.clear(); }
    out
}

/// Area shares of the cell (grass, asphalt, building, water): projected areas
/// of its grass ground, carriageway and building roof faces, and the share of
/// WATER_SAMPLES² points inside a water body.
fn ambience(d: &MapDocument, roads: &BTreeMap<&str, &mapkit_core::Road>, ids: &[&str], corners: &[[[i64; 3]; 3]], surfaces: &[u8], bounds: &Bounds) -> [f64; 4] {
    let cell = ((bounds.max[0] - bounds.min[0]) as f64 * (bounds.max[1] - bounds.min[1]) as f64 / 10000.0).max(1.0);
    let buildings: BTreeSet<&str> = d.buildings.iter().map(|b| b.id.as_str()).collect();
    let mut shares = [0.0f64; 4];
    for (i, v) in corners.iter().enumerate() {
        if vertical(v) { continue; }
        let [a, b, c] = v.map(scene);
        let (e, f) = (sub(b, a), sub(c, a));
        let surface = (e[0] * f[1] - e[1] * f[0]).abs() * 0.5;
        let k = if roads.contains_key(ids[i]) { 1 } else if buildings.contains(ids[i]) { 2 } else if surfaces.get(i) == Some(&4) { 0 } else { continue };
        shares[k] += surface;
    }
    for s in shares.iter_mut().take(3) { *s = (*s / cell).min(1.0); }
    let waters: Vec<_> = d.water_bodies.iter().filter(|w| {
        let (lo, hi) = w.polygon.iter().fold(([i64::MAX; 2], [i64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        hi[0] >= bounds.min[0] && lo[0] <= bounds.max[0] && hi[1] >= bounds.min[1] && lo[1] <= bounds.max[1]
    }).collect();
    if !waters.is_empty() {
        let mut wet = 0;
        for j in 0..WATER_SAMPLES {
            for i in 0..WATER_SAMPLES {
                let p = [bounds.min[0] + ((bounds.max[0] - bounds.min[0]) as f64 * (i as f64 + 0.5) / WATER_SAMPLES as f64) as i64,
                    bounds.min[1] + ((bounds.max[1] - bounds.min[1]) as f64 * (j as f64 + 0.5) / WATER_SAMPLES as f64) as i64];
                if waters.iter().any(|w| w.contains_horizontal(p)) { wet += 1; }
            }
        }
        shares[3] = wet as f64 / (WATER_SAMPLES * WATER_SAMPLES) as f64;
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapkit_core::{Profile, RoadPaintSegment};

    fn junction() -> (MapDocument, RoadPaint, Vec<(&'static str, [[i64; 3]; 3])>) {
        let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        let mut a = d.roads[0].clone();
        a.id = "a".into();
        a.sidewalk_cm = Some(100);
        a.markings = Some(mapkit_core::RoadMarkings { color: None, lanes: 2, center_line: true, edge_lines: false,
            crosswalk_start: false, crosswalk_end: false, one_way: 0, stop_lines: false, lane_width_cm: 0 });
        let mut b = a.clone();
        b.id = "b".into();
        b.markings.as_mut().unwrap().crosswalk_end = true;
        let mut bare = a.clone();
        bare.id = "bare".into();
        bare.sidewalk_cm = Some(0);
        d.roads = vec![a, b, bare];
        d.placements.clear();
        d.buildings.clear();
        d.pois.clear();
        d.water_bodies.clear();
        d.attributions.clear();
        let mut paint = RoadPaint::default();
        let segment = |a: [i64; 3], b: [i64; 3], total: f64| RoadPaintSegment { a, b, width_cm: 600.0, station_cm: 0.0, period_cm: 240.0, total_cm: total };
        // Road a runs along map +x (carriageway map z -300..300); road b along
        // map +z from a's north kerb (carriageway map x 1400..2000).
        paint.paths.insert("a".into(), vec![segment([0, 0, 0], [2000, 0, 0], 2000.0)]);
        paint.paths.insert("b".into(), vec![segment([1700, 0, 300], [1700, 0, 2000], 1700.0)]);
        paint.paths.insert("bare".into(), vec![segment([0, 0, 4000], [2000, 0, 4000], 2000.0)]);
        let mut faces: Vec<(&str, [[i64; 3]; 3])> = Vec::new();
        for t in quad(0, -300, 2000, 300, 0) { faces.push(("a", t)); }
        for t in quad(1400, 300, 2000, 2000, 0) { faces.push(("b", t)); }
        // a's north top, its corner apron turning north (x 1200..1300, z
        // 300..700, a's faces), b's west top from the apron's corner, a's south
        // top and b's east top.
        for t in quad(0, 300, 1200, 400, 3) { faces.push(("a:sidewalk", t)); }
        for t in quad(1200, 300, 1300, 700, 3) { faces.push(("a:sidewalk", t)); }
        for t in quad(1300, 700, 1400, 2000, 3) { faces.push(("b:sidewalk", t)); }
        for t in quad(0, -400, 2000, -300, 3) { faces.push(("a:sidewalk", t)); }
        for t in quad(2000, 300, 2100, 2000, 3) { faces.push(("b:sidewalk", t)); }
        // The curb wall of a's north top, which no lane uses.
        faces.push(("a:sidewalk", [[0, 0, 300], [1200, 0, 300], [1200, 3, 300]]));
        // A road without a sidewalk.
        for t in quad(0, 3700, 2000, 4300, 0) { faces.push(("bare", t)); }
        (d, paint, faces)
    }
    fn run(d: &MapDocument, paint: &RoadPaint, faces: &[(&str, [[i64; 3]; 3])], caps: &Caps) -> WalkLanes {
        run_in(d, paint, faces, caps, Bounds { min: [0, -1000], max: [5000, 5000] })
    }
    fn run_in(d: &MapDocument, paint: &RoadPaint, faces: &[(&str, [[i64; 3]; 3])], caps: &Caps, bounds: Bounds) -> WalkLanes {
        let roads: BTreeMap<&str, &mapkit_core::Road> = d.roads.iter().map(|r| (r.id.as_str(), r)).collect();
        let ids: Vec<&str> = faces.iter().map(|f| f.0).collect();
        let corners: Vec<_> = faces.iter().map(|f| f.1).collect();
        let surfaces = vec![0u8; faces.len()];
        build(d, paint, &roads, &ids, &corners, &surfaces, &bounds, caps)
    }
    fn polyline(w: &WalkLanes, e: [f64; 4]) -> &[P3] { &w.points[e[0] as usize..(e[0] + e[1]) as usize] }
    fn quad(x0: i64, z0: i64, x1: i64, z1: i64, y: i64) -> [[[i64; 3]; 3]; 2] { [[[x0, y, z0], [x1, y, z0], [x1, y, z1]], [[x0, y, z0], [x1, y, z1], [x0, y, z1]]] }

    #[test]
    fn a_two_road_junction_walks_its_sidewalk_tops_round_the_corner_and_across() {
        let (d, paint, faces) = junction();
        let w = run(&d, &paint, &faces, &CAPS);
        let (_, _, all) = junction();
        let tops: Vec<[P2; 3]> = all.iter().filter(|f| f.0.ends_with(":sidewalk") && !vertical(&f.1)).map(|f| f.1.map(scene)).collect();
        let on_top = |p: &P3| tops.iter().any(|t| inside(*t, [p[0], p[2]])) && (p[1] - 0.03).abs() < 1e-9;
        assert!(w.lanes.len() >= 4, "both sides of both roads: {:?}", w.lanes);
        for &e in &w.lanes {
            assert!(polyline(&w, e).iter().all(on_top), "lane points lie inside sidewalk tops at their height");
            assert!(e[2] >= 0.10 && e[2] <= 0.5, "half width inside the band: {e:?}");
            assert!(e[3] >= DENSITY_BASE && e[3] <= DENSITY_MAX, "density: {e:?}");
        }
        // Lanes run along the band middles: a north at scene z -3.5, a south at 3.5, b west at x 13.5, b east at 20.5.
        let near = |p: &P3, x: Option<f64>, z: Option<f64>| x.is_none_or(|x| (p[0] - x).abs() < 1e-6) && z.is_none_or(|z| (p[2] - z).abs() < 1e-6);
        let lane_on = |x: Option<f64>, z: Option<f64>| w.lanes.iter().any(|&e| polyline(&w, e).iter().all(|p| near(p, x, z)));
        assert!(lane_on(None, Some(-3.5)) && lane_on(None, Some(3.5)) && lane_on(Some(13.5), None) && lane_on(Some(20.5), None), "{:?}", w.points);
        // The corner link joins a's north lane end to b's west lane start through the apron.
        let a_end = w.lanes.iter().map(|&e| polyline(&w, e)).filter(|l| l.iter().all(|p| near(p, None, Some(-3.5)))).flat_map(|l| [l[0], l[l.len() - 1]]).collect::<Vec<_>>();
        let b_end = w.lanes.iter().map(|&e| polyline(&w, e)).filter(|l| l.iter().all(|p| near(p, Some(13.5), None))).flat_map(|l| [l[0], l[l.len() - 1]]).collect::<Vec<_>>();
        let joined = w.links.iter().any(|&e| {
            let l = polyline(&w, e);
            let (s, t) = (l[0], l[l.len() - 1]);
            l.len() >= 2 && l.iter().all(on_top)
                && ((a_end.contains(&s) && b_end.contains(&t)) || (a_end.contains(&t) && b_end.contains(&s)))
        });
        assert!(joined, "a corner link joins the two roads: {:?} / {:?}", w.links, w.points);
        // b's crosswalk (1 m in from its end) has kerb mouths on the tops beside it.
        assert_eq!(w.crossings.len(), 2);
        for m in &w.crossings { assert!(on_top(m), "crossing mouths take the sidewalk-top height: {m:?}"); }
        let xs: Vec<f64> = w.crossings.iter().map(|m| m[0]).collect();
        assert!(xs.iter().any(|x| (x - 13.85).abs() < 1e-6) && xs.iter().any(|x| (x - 20.15).abs() < 1e-6), "{xs:?}");
        assert_eq!(w.signals, [0], "no signal head near the crossing");
        // Each mouth is a lane node: some link ends exactly at it.
        for m in &w.crossings {
            assert!(w.links.iter().any(|&e| { let l = polyline(&w, e); l[0] == *m || l[l.len() - 1] == *m }), "mouth {m:?} joins a lane");
        }
        // The crossing raises the density of the lanes near it.
        assert!(w.lanes.iter().any(|e| (e[3] - (DENSITY_BASE + DENSITY_CROSSWALK)).abs() < 1e-9));
        assert!(w.overflow.is_none());
        let amb = w.ambience.unwrap();
        assert!(amb[1] > 0.0 && amb[0] == 0.0 && amb[2] == 0.0 && amb[3] == 0.0, "asphalt share only: {amb:?}");
        assert_eq!(w.bytes(), w.points.len() as u64 * 12 + (w.lanes.len() + w.links.len()) as u64 * 16 + 24 + 1 + 16, "decorate bytes follow the formula");
    }

    #[test]
    fn signals_mark_crossings_and_bare_roads_give_no_lanes() {
        let (mut d, paint, faces) = junction();
        d.placements.push(mapkit_core::Placement { id: "sig".into(), asset_id: "builtin:signal_h".into(), position: [1500, 0, 1900], quarter_turns: 0, yaw_offset_mdeg: 0 });
        let w = run(&d, &paint, &faces, &CAPS);
        assert_eq!(w.signals, [1], "a signal head within 3 m marks the crossing");
        // A pole where osm_furniture.signals() stands it on b's arm: past the
        // kerb by half width + sidewalk + (30 + 4 + 2) cm x 1/3, and
        // sqrt(2) x 3 m + 12 cm from the node (b's trimmed end 2 m + 3 m on),
        // over 4 m from the crossing's centre.
        let pole = [1700 + 300 + 100 + 12, 0, 2500 - 424 - 12];
        d.placements[0].position = pole;
        assert!(((pole[0] - 1700) as f64).hypot((pole[2] - 1900) as f64) > 400.0);
        assert_eq!(run(&d, &paint, &faces, &CAPS).signals, [1], "a pole beside the kerb marks the crossing");
        d.placements[0].position = [pole[0] + 600, 0, pole[2]];
        assert_eq!(run(&d, &paint, &faces, &CAPS).signals, [0], "a pole 6 m past the kerb does not");
        // Only the bare road, its carriageway and tops tagged as its sidewalk:
        // a road without a sidewalk width gets no walk keys, ambience kept.
        let mut bare: Vec<_> = faces.iter().filter(|f| f.0 == "bare").copied().collect();
        for t in quad(0, 4300, 2000, 4400, 3) { bare.push(("bare:sidewalk", t)); }
        let w = run(&d, &paint, &bare, &CAPS);
        assert!(w.points.is_empty() && w.lanes.is_empty() && w.links.is_empty() && w.crossings.is_empty());
        assert!(w.ambience.is_some());
        assert_eq!(w.bytes(), AMBIENCE_BYTES);
    }

    #[test]
    fn long_segments_keep_their_lanes_across_cells_and_driveway_gaps() {
        // Road a alone, one 20 m segment cut by the cell edge at x 6 m: the
        // segment's middle lies in the east cell, where a driveway gap (x 9..11)
        // also interrupts the north top.
        let (d, mut paint, _) = junction();
        paint.paths.retain(|id, _| id == "a");
        let west: Vec<(&str, [[i64; 3]; 3])> = [("a", quad(0, -300, 600, 300, 0)), ("a:sidewalk", quad(0, 300, 600, 400, 3)), ("a:sidewalk", quad(0, -400, 600, -300, 3))]
            .into_iter().flat_map(|(id, q)| q.map(|t| (id, t))).collect();
        let east: Vec<(&str, [[i64; 3]; 3])> = [("a", quad(600, -300, 2000, 300, 0)), ("a:sidewalk", quad(600, 300, 900, 400, 3)), ("a:sidewalk", quad(1100, 300, 2000, 400, 3)),
            ("a:sidewalk", quad(600, -400, 2000, -300, 3))].into_iter().flat_map(|(id, q)| q.map(|t| (id, t))).collect();
        let w = run_in(&d, &paint, &west, &CAPS, Bounds { min: [0, -1000], max: [600, 1000] });
        let e = run_in(&d, &paint, &east, &CAPS, Bounds { min: [600, -1000], max: [2000, 1000] });
        let spans = |w: &WalkLanes, z: f64| -> Vec<(f64, f64)> {
            let mut list: Vec<(f64, f64)> = w.lanes.iter().map(|&l| polyline(w, l)).filter(|l| l.iter().all(|p| (p[2] - z).abs() < 1e-6))
                .map(|l| (l[0][0].min(l[l.len() - 1][0]), l[0][0].max(l[l.len() - 1][0]))).collect();
            list.sort_by(|a, b| a.0.total_cmp(&b.0));
            list
        };
        assert_eq!(spans(&w, -3.5), [(0.0, 6.0)], "west north: {:?}", w.points);
        assert_eq!(spans(&w, 3.5), [(0.0, 6.0)], "west south: {:?}", w.points);
        assert_eq!(spans(&e, -3.5), [(6.0, 9.0), (11.0, 20.0)], "east north splits at the driveway: {:?}", e.points);
        assert_eq!(spans(&e, 3.5), [(6.0, 20.0)], "east south: {:?}", e.points);
        // The neighbours' lane ends meet within the field's endpoint merge.
        for z in [-3.5, 3.5] { assert!((spans(&e, z)[0].0 - spans(&w, z)[0].1).abs() <= MERGE); }
    }

    #[test]
    fn caps_drop_lanes_and_raise_the_profile_note() {
        let (d, paint, faces) = junction();
        let full = run(&d, &paint, &faces, &CAPS);
        let w = run(&d, &paint, &faces, &Caps { points: 8, entries: 512, crossings: 256 });
        assert!(w.points.len() <= 8 && w.lanes.len() < full.lanes.len());
        assert_eq!(w.overflow, Some((full.points.len() as u64, 8)));
        let (_, warnings) = Profile::Authoring.run(|| { w.note(); Ok(()) });
        let warnings: Vec<_> = warnings.iter().map(|w| (w.code.as_ref(), w.actual, w.limit, w.runtime_rejects)).collect();
        assert_eq!(warnings, [("walk_lane_points", full.points.len() as u64, 8, false)]);
        assert!(Profile::Runtime.run(|| { w.note(); Ok(()) }).1.is_empty());
        let w = run(&d, &paint, &faces, &Caps { points: 2048, entries: 2, crossings: 0 });
        assert!(w.lanes.len() + w.links.len() <= 2 && w.crossings.is_empty() && w.overflow.is_some());
    }

    #[test]
    fn footfall_radii_follow_the_map_scale() {
        let (mut d, _, _) = junction();
        let bounds = Bounds { min: [0, 0], max: [5000, 5000] };
        d.placements.push(mapkit_core::Placement { id: "stop".into(), asset_id: "builtin:bus_shelter".into(), position: [3000, 0, 0], quarter_turns: 0, yaw_offset_mdeg: 0 });
        // Without a recorded scale D = 3: the shelter radius is 40 m / 3.
        let f = Footfall::new(&d, &bounds);
        assert!((f.density([17.0, 0.0], &[]) - (DENSITY_BASE + DENSITY_SHELTER)).abs() < 1e-9, "13 m from the shelter");
        assert!((f.density([15.0, 0.0], &[]) - DENSITY_BASE).abs() < 1e-9, "15 m away");
        d.attributions.push(mapkit_core::Attribution { source: "osm#layer".into(), license: "ODbL-1.0".into(),
            notice: r#"{"adapter":"osm-extract-v1","authored_units":{"metres_per_unit":1,"source_denominator":1}}"#.into() });
        assert!((Footfall::new(&d, &bounds).density([15.0, 0.0], &[]) - (DENSITY_BASE + DENSITY_SHELTER)).abs() < 1e-9, "1:1 reaches 40 m");
        d.buildings.retain(|_| false);
        d.buildings.push(serde_json::from_value(serde_json::json!({"id":"shop","footprint":[[2900,-100],[3100,-100],[3100,100],[2900,100]],
            "base_cm":0,"height_cm":900,"usage":"commercial","material":"concrete","roof":"flat"})).unwrap());
        let f = Footfall::new(&d, &bounds);
        assert!((f.density([30.0, 0.0], &[]) - (DENSITY_BASE + DENSITY_SHELTER + DENSITY_COMMERCIAL)).abs() < 1e-9);
    }
}
