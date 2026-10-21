//! Authored barrier polylines (hedges, fences, walls). Each kept span becomes
//! solid terrain-following prisms; road corridors cut gates automatically.
//! Gating is a spatial-index broad phase plus an exact capsule interval per
//! candidate; only corridors near a barrier are built, so work grows with
//! barrier segments × local roads, never all pairs. Pieces are visited, never
//! collected, and every per-cell visit stops at the caller's output cap.
use crate::bounds_index::BoundsIndex;
use crate::generation::Builder;
use crate::*;

pub const MAX_POINTS: usize = 4096;
pub const HEIGHT_CM: std::ops::RangeInclusive<u32> = 10..=1000;
pub const THICKNESS_CM: std::ops::RangeInclusive<u32> = 5..=100;
/// Clearance beyond a corridor's half width (plus sidewalk) and the barrier's
/// own half thickness.
pub const GATE_MARGIN_CM: f64 = 50.0;
/// The prism base is this far below the lowest terrain under each piece end.
pub const EMBED_CM: i64 = 25;
/// Longest generated piece, measured along the source segment from its start.
pub const MAX_PIECE_CM: f64 = 800.0;
/// Bridge/elevated/tunnel occupied band around the driving surface: deck below
/// it, headroom above it (a tunnel uses its clearance). Only a barrier whose
/// span from base to top meets that band is gated.
pub const DECK_CM: i64 = 100;
pub const HEADROOM_CM: i64 = 250;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BarrierKind {
    Hedge,
    Fence,
    Wall,
}
impl BarrierKind {
    /// Contact surface of the generated (never spawnable) faces.
    pub fn surface(self) -> Surface {
        match self {
            BarrierKind::Hedge => Surface::Grass,
            BarrierKind::Fence | BarrierKind::Wall => Surface::Concrete,
        }
    }
    /// Shared display material key (`barrier:<kind>`).
    pub fn material_key(self) -> &'static str {
        match self {
            BarrierKind::Hedge => "barrier:hedge",
            BarrierKind::Fence => "barrier:fence",
            BarrierKind::Wall => "barrier:wall",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Barrier {
    pub id: String,
    pub kind: BarrierKind,
    /// Centreline `[x, y]` in local centimetres; consecutive points differ.
    /// A closed perimeter repeats its first point.
    #[schemars(length(min = 2, max = 4096))]
    pub line: Vec<Point>,
    /// Visible height above the terrain at the centreline.
    #[schemars(range(min = 10, max = 1000))]
    pub height_cm: u32,
    #[schemars(range(min = 5, max = 100))]
    pub thickness_cm: u32,
}

const PIECES: crate::profile::Budget = crate::profile::Budget { code: "barrier_pieces", limit: 1_000_000,
    guard: 8_000_000, message: "Barriers create more wall pieces than the game allows", runtime_rejects: true };
const WORK: crate::profile::Budget = crate::profile::Budget { code: "barrier_work", limit: 8_000_000,
    guard: 64_000_000, message: "Barrier road-gate checks need more work than the game allows", runtime_rejects: true };

fn tick(work: &mut usize, amount: usize) -> Result<()> {
    crate::cancellation::checkpoint()?;
    *work = work.saturating_add(amount);
    if *work as u64 > WORK.limit {
        WORK.check(*work as u64, || error("E_BUDGET", "barrier road gate work limit"))
    } else {
        Ok(())
    }
}
fn sorting(n: usize) -> usize {
    n.saturating_mul(n.max(1).ilog2() as usize + 1)
}

pub(crate) fn validate(d: &MapDocument) -> Result<()> {
    for b in &d.barriers {
        if !(2..=MAX_POINTS).contains(&b.line.len())
            || !HEIGHT_CM.contains(&b.height_cm)
            || !THICKNESS_CM.contains(&b.thickness_cm)
            || b.line.iter().any(|p| !d.bounds.contains(*p))
            || b.line.windows(2).any(|s| s[0] == s[1])
        {
            return Err(error(
                "E_GEOMETRY",
                format!("invalid barrier {}: 2..4096 distinct consecutive in-bounds points, height 10..1000 cm, thickness 5..100 cm", b.id),
            ));
        }
    }
    if !d.barriers.is_empty() {
        Spans::new(d)?;
    }
    Ok(())
}

/// A road or track corridor around which barriers are cut. `level` is the
/// driving-surface height at `a`/`b` and the headroom of a structural corridor;
/// ground corridors (`None`) always cut.
#[derive(Debug)]
struct Capsule {
    a: [f64; 2],
    b: [f64; 2],
    radius: f64,
    level: Option<[i64; 3]>,
}
fn f(p: Point) -> [f64; 2] {
    [p[0] as f64, p[1] as f64]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
/// `t` interval where `p + t u` lies within `radius` of `e`.
fn disc(p: [f64; 2], u: [f64; 2], e: [f64; 2], radius: f64) -> Option<(f64, f64)> {
    let w = sub(p, e);
    let a = dot(u, u);
    let b = dot(w, u);
    let c = dot(w, w) - radius * radius;
    let discriminant = b * b - a * c;
    if a == 0.0 || discriminant < 0.0 {
        return None;
    }
    let s = libm::sqrt(discriminant);
    Some(((-b - s) / a, (-b + s) / a))
}
/// `t` interval of `p + t u` inside the (convex) capsule; the union of its two
/// end discs and its rectangle is again one interval.
fn interval(p: [f64; 2], u: [f64; 2], c: &Capsule, radius: f64) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut add = |range: Option<(f64, f64)>| {
        if let Some((a, b)) = range.filter(|(a, b)| a <= b) {
            lo = lo.min(a);
            hi = hi.max(b);
        }
    };
    add(disc(p, u, c.a, radius));
    add(disc(p, u, c.b, radius));
    let v = sub(c.b, c.a);
    let length = libm::sqrt(dot(v, v));
    if length > 0.0 {
        let axis = [v[0] / length, v[1] / length];
        let normal = [-axis[1], axis[0]];
        let offset = sub(p, c.a);
        let mut range = Some((f64::NEG_INFINITY, f64::INFINITY));
        for (direction, min, max) in [(axis, 0.0, length), (normal, -radius, radius)] {
            let (o, s) = (dot(offset, direction), dot(u, direction));
            range = range.and_then(|(a, b)| {
                if s == 0.0 {
                    (o >= min && o <= max).then_some((a, b))
                } else {
                    let (t0, t1) = ((min - o) / s, (max - o) / s);
                    Some((a.max(t0.min(t1)), b.min(t0.max(t1))))
                }
            });
        }
        add(range);
    }
    (lo < hi).then_some((lo, hi))
}
/// Bounds of a capsule widened by the widest barrier half thickness, so one
/// query per barrier segment finds every capsule that can cut it.
fn capsule_bounds(c: &Capsule) -> Bounds {
    let r = c.radius + f64::from(*THICKNESS_CM.end()) / 2.0 + 2.0;
    Bounds {
        min: [libm::floor(c.a[0].min(c.b[0]) - r) as i64, libm::floor(c.a[1].min(c.b[1]) - r) as i64],
        max: [libm::ceil(c.a[0].max(c.b[0]) + r) as i64, libm::ceil(c.a[1].max(c.b[1]) + r) as i64],
    }
}
/// Road/track corridors whose bounds come near a barrier segment, split into
/// always-cutting ground corridors and level-tested structural corridors.
fn corridors(d: &MapDocument, near: &BoundsIndex) -> Result<(Vec<Capsule>, Vec<Capsule>)> {
    let (mut ground, mut structural) = (Vec::new(), Vec::new());
    for r in &d.roads {
        crate::cancellation::checkpoint()?;
        let level = |p: Vertex| match r.kind {
            RoadKind::Ground | RoadKind::Underpass => None,
            RoadKind::Tunnel => Some(i64::from(r.clearance_cm.unwrap_or(0))).map(|h| [p[1], p[1], h]),
            RoadKind::Bridge | RoadKind::Elevated => Some([p[1], p[1], HEADROOM_CM]),
        };
        let mut road = Vec::new();
        for (i, s) in r.points.windows(2).enumerate() {
            let level = level(s[0]).map(|[a, _, h]| [a, s[1][1], h]);
            road.push((Capsule { a: f([s[0][0], s[0][2]]), b: f([s[1][0], s[1][2]]), radius: f64::from(r.widths_cm[i]) / 2.0, level }, false));
        }
        // Junction aprons and road ends: a disc of the full end width.
        let last = r.points.len() - 1;
        for (p, w) in [(r.points[0], r.widths_cm[0]), (r.points[last], r.widths_cm[last - 1])] {
            let c = f([p[0], p[2]]);
            road.push((Capsule { a: c, b: c, radius: f64::from(w), level: level(p) }, false));
        }
        // Near test before the (possibly building-dependent) sidewalk width:
        // the widest sidewalk (1000 cm) keeps it conservative.
        let mut any = false;
        for (c, kept) in &mut road {
            let mut area = capsule_bounds(c);
            for a in 0..2 { area.min[a] -= 1000; area.max[a] += 1000; }
            *kept = near.hits(&area);
            any |= *kept;
        }
        if !any { continue; }
        let extra = f64::from(crate::placement::sidewalk_width(d, r)) + GATE_MARGIN_CM;
        for (mut c, kept) in road {
            if !kept { continue; }
            c.radius += extra;
            if c.level.is_some() { structural.push(c) } else { ground.push(c) }
        }
    }
    if let Some(track) = &d.assembled_track {
        for piece in &track.pieces {
            for path in [&piece.path, &piece.alternate_path] {
                for pair in path.windows(2) {
                    crate::cancellation::checkpoint()?;
                    // Occupied band of the whole ribbon: both edges of a banked
                    // sample (position ± lateral · side), headroom along the
                    // normal, below the surface too when the track is inverted.
                    let band = [&pair[0], &pair[1]].map(ribbon_band);
                    // Surface at the band's low edge: deck below it, headroom to its top.
                    let headroom = band.iter().map(|[lo, hi]| hi - lo).max().unwrap();
                    let c = Capsule {
                        a: f([pair[0].position_cm[0], pair[0].position_cm[2]]),
                        b: f([pair[1].position_cm[0], pair[1].position_cm[2]]),
                        radius: f64::from(pair[0].lateral_cm.max(pair[1].lateral_cm)) + GATE_MARGIN_CM,
                        level: Some([band[0][0], band[1][0], headroom]),
                    };
                    if near.hits(&capsule_bounds(&c)) { structural.push(c); }
                }
            }
        }
    }
    Ok((ground, structural))
}

/// Occupied height band of a track sample's whole ribbon: both edges of a
/// banked sample (position ± lateral · side), headroom along the normal and,
/// when inverted, below the surface as well.
pub(crate) fn ribbon_band(s: &crate::assembled_track::Sample) -> [i64; 2] {
    let (n, fw) = (s.normal.map(|v| v as f64 / 1e6), s.forward.map(|v| v as f64 / 1e6));
    let rise = libm::ceil((n[2] * fw[0] - n[0] * fw[2]).abs() * f64::from(s.lateral_cm)) as i64;
    let below = if n[1] < 0.0 { HEADROOM_CM } else { 0 };
    [s.position_cm[1] - rise - below, s.position_cm[1] + rise + HEADROOM_CM]
}

/// One kept part of a barrier segment `a → b`, parametric `t0 < t1`. Kept
/// parts at polyline joints extend half the thickness past the joint so
/// corners close. Every value derives only from this segment and corridors
/// near it, so regional source subsets reproduce identical local pieces.
#[derive(Debug, Clone)]
pub(crate) struct Span {
    barrier: usize,
    a: Point,
    b: Point,
    t0: f64,
    t1: f64,
}
impl Span {
    fn at(&self, t: f64) -> [f64; 2] {
        [self.a[0] as f64 + (self.b[0] - self.a[0]) as f64 * t, self.a[1] as f64 + (self.b[1] - self.a[1]) as f64 * t]
    }
    fn point(&self, t: f64) -> Point {
        self.at(t).map(|v| libm::round(v) as i64)
    }
    fn direction(&self) -> [f64; 2] {
        sub(f(self.b), f(self.a))
    }
}
/// Count of lattice values `base + k * step` in the closed range.
fn crossings(c0: f64, c1: f64, base: f64, step: f64) -> u64 {
    let (lo, hi) = (c0.min(c1), c0.max(c1));
    (libm::floor((hi - base) / step) - libm::ceil((lo - base) / step) + 1.0).max(0.0) as u64
}

#[derive(Debug)]
pub(crate) struct Spans {
    spans: Vec<Span>,
    index: BoundsIndex,
    structural: Vec<Capsule>,
    structural_index: BoundsIndex,
}
impl Spans {
    pub(crate) fn new(d: &MapDocument) -> Result<Self> {
        let mut work = 0;
        let mut spans = Vec::new();
        let (mut structural, mut structural_index) = (Vec::new(), BoundsIndex::new(&[]));
        if !d.barriers.is_empty() {
            let segments: Vec<Bounds> = d.barriers.iter().flat_map(|b| {
                let half = i64::from(b.thickness_cm) / 2 + 1;
                b.line.windows(2).map(move |s| {
                    let mut area = crate::bounds_index::bounds(s);
                    for a in 0..2 { area.min[a] -= half; area.max[a] += half; }
                    area
                })
            }).collect();
            // Barrier-proportional indices are bounded by the input vertex and
            // piece budgets; `barrier_work` charges corridor gating only.
            crate::cancellation::checkpoint()?;
            let near = BoundsIndex::new(&segments);
            let (ground, kept_structural) = corridors(d, &near)?;
            tick(&mut work, sorting(ground.len()) + sorting(kept_structural.len()))?;
            let index = BoundsIndex::new(&ground.iter().map(capsule_bounds).collect::<Vec<_>>());
            structural_index = BoundsIndex::new(&kept_structural.iter().map(capsule_bounds).collect::<Vec<_>>());
            structural = kept_structural;
            let grid = d.heightmaps.iter().map(|h| h.spacing_cm).min().map(f64::from);
            let (cell, origin) = (f64::from(d.cell_size_cm), f(d.bounds.min));
            let mut pieces = 0u64;
            for (barrier_index, barrier) in d.barriers.iter().enumerate() {
                let n = barrier.line.len();
                let closed = barrier.line[0] == barrier.line[n - 1];
                let half = f64::from(barrier.thickness_cm) / 2.0;
                for (i, s) in barrier.line.windows(2).enumerate() {
                    let (a, b) = (s[0], s[1]);
                    let (p, u) = (f(a), sub(f(b), f(a)));
                    let length = libm::sqrt(dot(u, u));
                    let lo = if i > 0 || closed { -half / length } else { 0.0 };
                    let hi = 1.0 + if i + 2 < n || closed { half / length } else { 0.0 };
                    let mut area = crate::bounds_index::bounds(&[a, b]);
                    for axis in 0..2 {
                        area.min[axis] -= half as i64 + 1;
                        area.max[axis] += half as i64 + 1;
                    }
                    let mut query = 0;
                    let candidates = index.query(&area, &mut query)?;
                    tick(&mut work, query + candidates.len() + 1)?;
                    let mut cuts: Vec<_> = candidates.into_iter()
                        .filter_map(|c| interval(p, u, &ground[c], ground[c].radius + half))
                        .collect();
                    cuts.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
                    let mut start = lo;
                    for (cut_lo, cut_hi) in cuts.into_iter().chain([(hi, f64::INFINITY)]) {
                        let end = cut_lo.min(hi);
                        if end > start {
                            let span = Span { barrier: barrier_index, a, b, t0: start, t1: end };
                            if span.point(span.t0) != span.point(span.t1) {
                                // Upper bound of the pieces any cell cuts from this span
                                // (before structure-level gates, which per-cell output
                                // caps bound): seams, length lattice, and grid lines and
                                // diagonals at the finest heightmap spacing.
                                let (p0, p1) = (span.at(span.t0), span.at(span.t1));
                                let seams = crossings(p0[0], p1[0], origin[0], cell) + crossings(p0[1], p1[1], origin[1], cell);
                                let along = length * (span.t1 - span.t0);
                                let mut bound = 2 + seams + (along / MAX_PIECE_CM) as u64;
                                if let Some(step) = grid {
                                    bound += crossings(p0[0], p1[0], origin[0], step) + crossings(p0[1], p1[1], origin[1], step)
                                        + crossings(p0[0] - p0[1], p1[0] - p1[1], origin[0] - origin[1], step) + 3 * (seams + 1);
                                }
                                pieces = pieces.saturating_add(bound);
                                // Every visited piece tests each nearby structure.
                                let mut area = crate::bounds_index::bounds(&[span.point(span.t0), span.point(span.t1)]);
                                for axis in 0..2 { area.min[axis] -= half as i64 + 1; area.max[axis] += half as i64 + 1; }
                                let mut query = 0;
                                let nearby = structural_index.query(&area, &mut query)?.len();
                                tick(&mut work, query + nearby.saturating_mul(bound as usize))?;
                                if pieces > PIECES.limit {
                                    PIECES.check(pieces, || error("E_LIMIT", "too many barrier pieces"))?;
                                }
                                spans.push(span);
                            }
                        }
                        start = start.max(cut_hi);
                        if start >= hi {
                            break;
                        }
                    }
                }
            }
        }
        let bounds: Vec<_> = spans.iter().map(|s| crate::bounds_index::bounds(&[s.point(s.t0), s.point(s.t1)])).collect();
        crate::cancellation::checkpoint()?;
        Ok(Self { index: BoundsIndex::new(&bounds), spans, structural, structural_index })
    }

    /// Visit the raw pieces owned by `cell` until `visit` returns false. Each span
    /// is cut at cell seams, every `MAX_PIECE_CM` along its source segment and, in
    /// a cell with a heightmap, at its grid lines and tile diagonals, so the
    /// terrain is planar under every piece's centreline. The centre point owns a
    /// piece. Only one span's cut list is held at a time.
    fn visit(&self, d: &MapDocument, cell: Cell, mut visit: impl FnMut(&Span, f64, f64) -> Result<bool>) -> Result<()> {
        let bounds = d.cell_bounds(cell)?;
        let origin = bounds.min;
        let spacing = d.heightmaps.iter().find(|h| h.cell == cell).map(|h| i64::from(h.spacing_cm));
        let mut work = 0;
        for i in self.index.query(&bounds, &mut work)? {
            crate::cancellation::checkpoint()?;
            let span = &self.spans[i];
            let start = f(span.a);
            let direction = span.direction();
            // Closed cell rectangle on the line.
            let mut range = (span.t0, span.t1);
            for axis in 0..2 {
                let (min, max) = (bounds.min[axis] as f64, bounds.max[axis] as f64);
                if direction[axis] == 0.0 {
                    if start[axis] < min || start[axis] > max { range = (1.0, 0.0); }
                } else {
                    let (t0, t1) = ((min - start[axis]) / direction[axis], (max - start[axis]) / direction[axis]);
                    range = (range.0.max(t0.min(t1)), range.1.min(t0.max(t1)));
                }
            }
            if range.0 >= range.1 {
                continue;
            }
            let mut cuts = vec![range.0, range.1];
            let lattice = |offset: f64, dir: f64, base: f64, step: f64, cuts: &mut Vec<f64>| {
                if dir == 0.0 { return; }
                let (a, b) = ((range.0 * dir + offset - base) / step, (range.1 * dir + offset - base) / step);
                for k in libm::ceil(a.min(b)) as i64..=libm::floor(a.max(b)) as i64 {
                    cuts.push((base + k as f64 * step - offset) / dir);
                }
            };
            // Length lattice: t = k * MAX_PIECE_CM / segment length from `a`.
            lattice(0.0, libm::sqrt(dot(direction, direction)), 0.0, MAX_PIECE_CM, &mut cuts);
            if let Some(s) = spacing.map(|s| s as f64) {
                lattice(start[0], direction[0], origin[0] as f64, s, &mut cuts);
                lattice(start[1], direction[1], origin[1] as f64, s, &mut cuts);
                // Tile diagonals: x - y = origin_x - origin_y + k * spacing.
                lattice(start[0] - start[1], direction[0] - direction[1], (origin[0] - origin[1]) as f64, s, &mut cuts);
            }
            cuts.retain(|t| *t >= range.0 && *t <= range.1);
            cuts.sort_by(|a, b| a.total_cmp(b));
            cuts.dedup();
            for pair in cuts.windows(2) {
                if span.point(pair[0]) == span.point(pair[1]) || d.cell_at(span.point((pair[0] + pair[1]) / 2.0)) != Some(cell) {
                    continue;
                }
                if !visit(span, pair[0], pair[1])? {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Structural corridors overlapping `[ta, tb]` of `span`, as clipped intervals
    /// with the corridor's occupied band at the interval centre.
    fn structures(&self, d: &MapDocument, span: &Span, ta: f64, tb: f64) -> Result<Vec<(f64, f64, [i64; 2])>> {
        let half = f64::from(d.barriers[span.barrier].thickness_cm) / 2.0;
        let mut area = crate::bounds_index::bounds(&[span.point(ta), span.point(tb)]);
        for a in 0..2 { area.min[a] -= half as i64 + 1; area.max[a] += half as i64 + 1; }
        let mut out = Vec::new();
        let mut work = 0;
        for c in self.structural_index.query(&area, &mut work)? {
            let capsule = &self.structural[c];
            let Some((lo, hi)) = interval(f(span.a), span.direction(), capsule, capsule.radius + half) else { continue };
            let (lo, hi) = (lo.max(ta), hi.min(tb));
            if lo >= hi { continue; }
            let m = span.at((lo + hi) / 2.0);
            let v = sub(capsule.b, capsule.a);
            let along = if dot(v, v) > 0.0 { (dot(sub(m, capsule.a), v) / dot(v, v)).clamp(0.0, 1.0) } else { 0.0 };
            let [ha, hb, headroom] = capsule.level.unwrap();
            let surface = libm::round(ha as f64 + (hb - ha) as f64 * along) as i64;
            out.push((lo, hi, [surface - DECK_CM, surface + headroom]));
        }
        Ok(out)
    }

    /// Triangle, convex/solid and ID-byte upper bounds for one cell. Counting
    /// stops once `cap` pieces are seen; the caller's output limit then fails.
    pub(crate) fn cost(&self, d: &MapDocument, cell: Cell, cap: u64) -> Result<(u64, u64, u64)> {
        let (mut triangles, mut count, mut id) = (0u64, 0u64, 0u64);
        self.visit(d, cell, |span, ta, tb| {
            let barrier = &d.barriers[span.barrier];
            // A structure gate splits a piece in two at most.
            let pieces = 1 + self.structures(d, span, ta, tb)?.len() as u64;
            let (p, q) = (span.point(ta), span.point(tb));
            let half = i64::from(barrier.thickness_cm) / 2 + 2;
            let inside = (0..2).all(|a| p[a].min(q[a]) - half >= d.bounds.min[a] && p[a].max(q[a]) + half <= d.bounds.max[a]);
            triangles = triangles.saturating_add(pieces * if inside { 12 } else { 60 });
            count = count.saturating_add(pieces);
            id = id.max(barrier.id.len() as u64);
            Ok(count <= cap)
        })?;
        Ok((triangles, count, id))
    }
}

/// Solid parallelepiped per piece: vertical sides, ends and base/top planes
/// along the centreline. Unclipped and emitted only by the owner cell, like
/// centre-owned trunks; a neighbouring owner may protrude by half a thickness.
pub(crate) fn generate(d: &MapDocument, cell: Cell, grid: Option<&HeightGrid>, spans: &Spans, b: &mut Builder) -> Result<()> {
    let bounds = d.cell_bounds(cell)?;
    let spacing = d.heightmaps.iter().find(|h| h.cell == cell).map_or(d.cell_size_cm, |h| h.spacing_cm) as i64;
    let side = (d.cell_size_cm as i64 / spacing + 1) as usize;
    // Source terrain planes, as `terrain_height` and the terrain mesh share them.
    let height = |p: Point| crate::terrain_height(&bounds, spacing, side, grid, d.terrain_base_cm, p);
    let saved = b.bounds.clone();
    b.bounds = d.bounds.clone();
    let result = spans.visit(d, cell, |span, ta, tb| {
        let barrier = &d.barriers[span.barrier];
        // Structures at the barrier's level cut it; decks above / tunnels below do not.
        let mut cuts = Vec::new();
        for (lo, hi, band) in spans.structures(d, span, ta, tb)? {
            let ground = height(span.point((lo + hi) / 2.0));
            if band[0] < ground + i64::from(barrier.height_cm) && ground - EMBED_CM < band[1] {
                cuts.push((lo, hi));
            }
        }
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let mut start = ta;
        for (lo, hi) in cuts.into_iter().chain([(tb, tb)]) {
            if lo.min(tb) > start {
                emit(barrier, span, start, lo.min(tb), grid.map(|_| (&bounds, spacing)), &height, b)?;
            }
            start = start.max(hi);
        }
        Ok(true)
    });
    b.bounds = saved;
    result
}

fn emit(barrier: &Barrier, span: &Span, ta: f64, tb: f64, lattice: Option<(&Bounds, i64)>,
    height: &impl Fn(Point) -> i64, b: &mut Builder) -> Result<()> {
    let (p, q) = (span.point(ta), span.point(tb));
    if p == q {
        return Ok(());
    }
    let direction = span.direction();
    let length = libm::sqrt(dot(direction, direction));
    let half = f64::from(barrier.thickness_cm) / 2.0;
    let n = [libm::round(-direction[1] / length * half) as i64, libm::round(direction[0] / length * half) as i64];
    // Rounded ends must keep the frame's orientation.
    if (q[0] - p[0]) as i128 * n[1] as i128 - (q[1] - p[1]) as i128 * n[0] as i128 <= 0 {
        return Ok(());
    }
    let corner = |e: Point, side: i64| [e[0] + side * n[0], e[1] + side * n[1]];
    // Base below the lowest terrain under each end, then lowered so no source
    // grid vertex under the footprint rises above the base plane (cross slopes).
    let mut base = [p, q].map(|e| [e, corner(e, -1), corner(e, 1)].map(height).into_iter().min().unwrap() - EMBED_CM);
    if let Some((cell, step)) = lattice {
        let corners = [corner(p, -1), corner(p, 1), corner(q, -1), corner(q, 1)];
        let lo: [i64; 2] = std::array::from_fn(|a| corners.iter().map(|c| c[a]).min().unwrap());
        let hi: [i64; 2] = std::array::from_fn(|a| corners.iter().map(|c| c[a]).max().unwrap());
        let (dx, dy, len2) = ((q[0] - p[0]) as f64, (q[1] - p[1]) as f64, ((q[0] - p[0]) as f64).powi(2) + ((q[1] - p[1]) as f64).powi(2));
        let mut deficit = 0i64;
        let mut sample = |point: Point| {
            let along = ((((point[0] - p[0]) as f64) * dx + ((point[1] - p[1]) as f64) * dy) / len2).clamp(0.0, 1.0);
            let plane = base[0] as f64 + (base[1] - base[0]) as f64 * along;
            deficit = deficit.max(libm::ceil(plane - (height(point) - EMBED_CM) as f64) as i64);
        };
        // Terrain is planar between the footprint's grid vertices and the
        // creases its long edges cross (grid lines and tile diagonals).
        let first = |a: usize| (lo[a] - cell.min[a]).div_euclid(step) * step + cell.min[a];
        let mut x = first(0);
        while x <= hi[0] {
            let mut y = first(1);
            while y <= hi[1] {
                if x >= lo[0] && y >= lo[1] {
                    sample([x, y]);
                }
                y += step;
            }
            x += step;
        }
        for side in [-1, 1] {
            let (a, e) = (corner(p, side), corner(q, side));
            let (u, v) = ([(e[0] - a[0]) as f64, (e[1] - a[1]) as f64], [a[0] as f64, a[1] as f64]);
            for (offset, dir, base) in [(v[0], u[0], cell.min[0]), (v[1], u[1], cell.min[1]),
                (v[0] - v[1], u[0] - u[1], cell.min[0] - cell.min[1])] {
                if dir == 0.0 { continue; }
                let (t0, t1) = ((offset - base as f64) / step as f64, (offset + dir - base as f64) / step as f64);
                for k in libm::ceil(t0.min(t1)) as i64..=libm::floor(t0.max(t1)) as i64 {
                    let t = ((base + k * step) as f64 - offset) / dir;
                    sample([libm::round(v[0] + u[0] * t) as i64, libm::round(v[1] + u[1] * t) as i64]);
                }
            }
        }
        base = base.map(|v| v - deficit);
    }
    let ends = [(p, base[0], height(p)), (q, base[1], height(q))];
    let vertices: Vec<Vertex> = (0..8).map(|i| {
        let (point, bottom, ground) = ends[i & 1];
        let side = if i & 4 != 0 { 1 } else { -1 };
        let y = if i & 2 != 0 { ground + i64::from(barrier.height_cm) } else { bottom };
        [point[0] + side * n[0], y, point[1] + side * n[1]]
    }).collect();
    let shape = CollisionConvex { vertices, faces: vec![
        [0,4,6],[0,6,2],[1,3,7],[1,7,5],
        [0,1,5],[0,5,4],[2,6,7],[2,7,3],
        [0,2,3],[0,3,1],[4,5,7],[4,7,6]] };
    if !shape.valid(100_000_000) {
        return Ok(());
    }
    b.solid(&barrier.id, SolidShape::Convex(shape.clone()))?;
    for face in &shape.faces {
        b.triangle(face.map(|i| shape.vertices[i as usize]), barrier.kind.surface(), &barrier.id, false)?;
    }
    if !b.distant {
        b.chunk.asset_convexes.push(GeneratedConvex { object_id: barrier.id.clone(), shape });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(normal: Vertex, forward: Vertex) -> crate::assembled_track::Sample {
        crate::assembled_track::Sample { position_cm: [0, 1000, 0], ribbon_cm: None, normal, forward, mode: "road".into(), safe: true,
            min_speed_cmps: 0, lateral_cm: 400, tube_radius_cm: 0, below_cm: 0, above_cm: 0 }
    }
    #[test]
    fn ribbon_band_covers_banked_edges_and_inverted_track() {
        assert_eq!(ribbon_band(&sample([0, 1_000_000, 0], [0, 0, 1_000_000])), [1000, 1000 + HEADROOM_CM]);
        // 45 degree bank about the forward (+y) axis: edges 283 cm below/above.
        let banked = ribbon_band(&sample([707_107, 707_107, 0], [0, 0, 1_000_000]));
        assert!(banked[0] <= 1000 - 282 && banked[1] >= 1000 + 282 + HEADROOM_CM, "{banked:?}");
        let inverted = ribbon_band(&sample([0, -1_000_000, 0], [0, 0, 1_000_000]));
        assert!(inverted[0] <= 1000 - HEADROOM_CM);
    }
}
