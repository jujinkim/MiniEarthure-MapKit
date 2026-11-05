//! Display-only detail of a cell's barrier pieces (OSM or authored fences,
//! walls and hedges), listed at presentation time from the generated
//! collision prisms (`asset_convexes` under the barrier ID, mapkit-core
//! barrier.rs). Generation, collision, the generated hash and documents are
//! unchanged; barrier_detail_meshes.gd draws runs and posts inside the
//! prisms and the renderer stops drawing the prism faces.
//!
//! Each prism is matched to its source segment; its ends are classified by
//! rebuilding the owner cell's cuts (cell rect, the 800 cm length lattice
//! and the cell's heightmap grid lines and tile diagonals, as
//! `Spans::visit` cuts them): an end on one of those cuts continues into the
//! next piece, an extension end past a polyline vertex is a joint, any other
//! end is a span end (a line end, or a road or structure gate). Runs are
//! clamped to their segment at joints that turn by HARD_JOINT_DEGREES or
//! more and reach a mitre's length past gentler joints, so neighbouring runs
//! never overlap coplanar faces. At a hard joint of a wall or hedge the
//! segment starting there adds a corner run (RUN_CORNER) over the outer
//! wedge the clamped runs leave open: centred on the vertex along that
//! segment, CORNER_SHRINK narrower, shorter and lower than the prisms, so it
//! stays inside the segment's start extension and main prisms and none of its
//! faces is coplanar with a run's (the far boxes, hedges and giwa caps
//! draw it; other walls' near corner piers already cover the wedge). Fences
//! keep their corner post. Posts and piers stand on the barrier's arc-length
//! lattice (one bay of its design), each owned by the piece whose clamped
//! half-open arc range holds it, so adjacent pieces and cells never repeat
//! one; span ends add an end or gate post just inside the span, hard joints
//! one corner post owned by the segment starting there, and lattice posts
//! within 0.3 bay of those give way: of any gate of the barrier in this cell
//! (a gate across a cell seam is not seen), of hard joints and open line
//! ends, and of a gentle closed line's closing post at arc 0.
//!
//! Frames: a run's x is the ground vector of its clamped centreline (scene
//! metres, rising with the planar terrain under it), y the height and z the
//! prism's own (possibly skewed) full-width side vector, oriented so the
//! basis is right-handed (the meshes are symmetric in z). A post's x is the
//! segment direction (with the rise) per real metre times its size, so its
//! slanted sides follow the prism's. Parts reach 0.12 m (fences) or 0.2 m
//! (walls, hedges) real below the centreline ground, inside the prism's
//! 25 cm embed at every map scale.
use mapkit_core::{barrier::{Barrier, MAX_PIECE_CM}, Bounds, Cell, MapDocument, Point, Result};
use mapkit_package::barrier_design as design;
use std::collections::BTreeMap;

/// Floats per run: origin (3), ground vector (3), side vector x, z (2),
/// height, design bits, phase at the origin (fraction of a bay), end flags,
/// index into the cell's barrier IDs. All in scene metres.
pub(crate) const RUN_FLOATS: usize = 13;
/// Floats per post: origin (3), x vector (3), side vector x, z (2), height,
/// design bits, post kind (POST_*), size factor (gate posts and fits),
/// index into the cell's barrier IDs.
pub(crate) const POST_FLOATS: usize = 13;
/// Per-cell caps: a cell over either lists nothing and keeps drawing its
/// prism faces (the busiest real-city cell has about 700 pieces).
pub(crate) const MAX_RUNS: usize = 8192;
pub(crate) const MAX_POSTS: usize = 16384;
pub(crate) const HARD_JOINT_DEGREES: f64 = 15.0;
/// Run end flags.
pub(crate) const START_SPAN_END: u32 = 1;
pub(crate) const END_SPAN_END: u32 = 2;
pub(crate) const START_HARD_JOINT: u32 = 4;
pub(crate) const END_HARD_JOINT: u32 = 8;
/// A wall or hedge corner run over a hard joint's outer wedge.
pub(crate) const RUN_CORNER: u32 = 16;
/// Corner runs keep this share of the prism's half width (along and across)
/// and height, inside the prisms and never coplanar with a run's faces.
const CORNER_SHRINK: f64 = 0.98;
/// Post kinds.
pub(crate) const POST_LATTICE: u32 = 0;
pub(crate) const POST_END: u32 = 1;
pub(crate) const POST_GATE: u32 = 2;
pub(crate) const POST_CORNER: u32 = 3;
/// Half sizes along the barrier (real metres) of the post meshes
/// (barrier_detail_meshes.gd, caps included) per fence family, and of the
/// wall pier mesh end, gate and corner piers scale (lattice piers keep
/// their family's own size; brick and block 0.2 m, precast H-posts 0.075 m).
pub(crate) const FENCE_POST_HALF_M: [f64; 5] = [0.035, 0.035, 0.032, 0.045, 0.026];
pub(crate) const PIER_HALF_M: f64 = 0.2;
const GATE_SIZE: f64 = 1.25;
/// Across-half sizes (real metres) of the fence post meshes per family.
pub(crate) const FENCE_POST_ACROSS_M: [f64; 5] = [0.035, 0.025, 0.032, 0.045, 0.026];
/// Clearance (cm) kept inside the prism by posts.
const CLEARANCE_CM: f64 = 0.1;

pub(crate) struct Listed {
    pub(crate) runs: Vec<f32>,
    pub(crate) posts: Vec<f32>,
    pub(crate) ids: Vec<String>,
    pub(crate) capped: bool,
}

/// Per barrier: cumulative segment lengths (cm), the arc positions of hard
/// joints and open line ends (sorted), turn angles per vertex and the design.
struct Line {
    cum: Vec<f64>,
    blocks: Vec<f64>,
    turn: Vec<f64>,
    closed: bool,
    /// A closed line whose closing turn is gentle: its posts run round
    /// through arc 0 = cum[n - 1].
    gentle_closure: bool,
    bits: design::Bits,
    bay_cm: f64,
}
impl Line {
    fn new(b: &Barrier, denominator: u64) -> Self {
        let n = b.line.len();
        let closed = n > 2 && b.line[0] == b.line[n - 1];
        let mut cum = vec![0.0];
        for s in b.line.windows(2) {
            let u = sub(s[1], s[0]);
            cum.push(cum.last().unwrap() + (u[0] * u[0] + u[1] * u[1]).sqrt());
        }
        // Turn at vertex j (0..n-1): between segment j-1 and j; a closed
        // line's first vertex joins its last and first segments.
        let mut turn = vec![0.0; n];
        for (j, angle) in turn.iter_mut().enumerate() {
            let (before, after) = if j > 0 && j + 1 < n { (j - 1, j) } else if closed && j == 0 { (n - 2, 0) } else { continue };
            let (u, v) = (sub(b.line[before + 1], b.line[before]), sub(b.line[after + 1], b.line[after]));
            *angle = (u[0] * v[1] - u[1] * v[0]).abs().atan2(u[0] * v[0] + u[1] * v[1]).to_degrees();
        }
        let mut blocks: Vec<f64> = (1..n - 1).filter(|&j| turn[j] >= HARD_JOINT_DEGREES).map(|j| cum[j]).collect();
        let gentle_closure = closed && turn[0] < HARD_JOINT_DEGREES;
        if !gentle_closure {
            blocks.push(0.0);
            blocks.push(cum[n - 1]);
        }
        blocks.sort_by(f64::total_cmp);
        let bits = design::design(&b.id, b.kind, b.height_cm, denominator);
        let bay_cm = design::bay_m(bits) * 100.0 / f64::from(design::denominator(bits));
        Self { cum, blocks, turn, closed, gentle_closure, bits, bay_cm }
    }
    /// Arc length of the whole line (cm).
    fn total(&self) -> f64 {
        *self.cum.last().unwrap()
    }
    /// Whether a hard joint or line end lies within `reach` of arc `s`
    /// (a closed line's end is its start).
    fn blocked(&self, s: f64, reach: f64) -> bool {
        let i = self.blocks.partition_point(|&b| b < s - reach);
        self.blocks.get(i).is_some_and(|&b| b <= s + reach)
    }
}

fn sub(a: Point, b: Point) -> [f64; 2] {
    [(a[0] - b[0]) as f64, (a[1] - b[1]) as f64]
}
/// `Span::point`: the rounded point at `t` of `a + (b - a) t`.
fn point(a: Point, u: [f64; 2], t: f64) -> Point {
    [(a[0] as f64 + u[0] * t).round() as i64, (a[1] as f64 + u[1] * t).round() as i64]
}

/// Whether `e` (at `t` on the segment `a + u t`) is one of the owner cell's
/// cuts of that segment: the cell rect, the length lattice and the cell's
/// heightmap grid lines and diagonals, with `Spans::visit`'s arithmetic.
fn on_cut(e: Point, a: Point, u: [f64; 2], t: f64, bounds: &Bounds, spacing: Option<f64>) -> bool {
    let start = [a[0] as f64, a[1] as f64];
    for axis in 0..2 {
        if u[axis] != 0.0 {
            for edge in [bounds.min[axis] as f64, bounds.max[axis] as f64] {
                if point(a, u, (edge - start[axis]) / u[axis]) == e {
                    return true;
                }
            }
        }
    }
    let lattice = |offset: f64, dir: f64, base: f64, step: f64| {
        if dir == 0.0 {
            return false;
        }
        let x = (t * dir + offset - base) / step;
        ((x.floor() as i64 - 1)..=(x.ceil() as i64 + 1)).any(|k| point(a, u, (base + k as f64 * step - offset) / dir) == e)
    };
    if lattice(0.0, (u[0] * u[0] + u[1] * u[1]).sqrt(), 0.0, MAX_PIECE_CM) {
        return true;
    }
    let Some(s) = spacing else { return false };
    let origin = bounds.min;
    lattice(start[0], u[0], origin[0] as f64, s)
        || lattice(start[1], u[1], origin[1] as f64, s)
        || lattice(start[0] - start[1], u[0] - u[1], (origin[0] - origin[1]) as f64, s)
}

/// (span end, gate) of a matched piece's start and end: a joint's extension
/// end or one of the owner cell's cuts continues; a line end is a span end;
/// any other end is a gate (a road or structure corridor).
fn ends(b: &Barrier, line: &Line, piece: &Piece, bounds: &Bounds, spacing: Option<f64>) -> [(bool, bool); 2] {
    let count = b.line.len();
    let i = piece.segment;
    let (a, end) = (b.line[i], b.line[i + 1]);
    let u = sub(end, a);
    let length = (u[0] * u[0] + u[1] * u[1]).sqrt();
    let half = f64::from(b.thickness_cm) / 2.0;
    let (joint_before, joint_after) = (i > 0 || line.closed, i + 2 < count || line.closed);
    let classify = |e: Point, t: f64, start: bool| -> (bool, bool) {
        if start && joint_before && e == point(a, u, -half / length) { return (false, false); }
        if !start && joint_after && e == point(a, u, 1.0 + half / length) { return (false, false); }
        if (start && i == 0 && !line.closed && e == a) || (!start && i + 2 == count && !line.closed && e == end) { return (true, false); }
        if on_cut(e, a, u, t, bounds, spacing) { return (false, false); }
        (true, true)
    };
    [classify(piece.p, piece.tp, true), classify(piece.q, piece.tq, false)]
}

/// One matched prism: its barrier, segment and raw end parameters.
struct Piece {
    barrier: usize,
    segment: usize,
    tp: f64,
    tq: f64,
    p: Point,
    q: Point,
    n: [i64; 2],
    ground: [f64; 2],
}

/// The source segment of a prism and its raw end parameters. Candidates
/// share the prism's rounded side vector and hold both ends within 1 cm of
/// their line inside their kept range; near-collinear neighbours can both
/// qualify for a few-centimetre piece, so the one whose cuts explain more of
/// the piece's ends wins (joint extension ends, segment ends, the owner
/// cell's cuts), then the one holding the piece's midpoint, then the first.
fn locate(b: &Barrier, p: Point, q: Point, n: [i64; 2], bounds: &Bounds, spacing: Option<f64>) -> Option<(usize, f64, f64)> {
    let count = b.line.len();
    let closed = count > 2 && b.line[0] == b.line[count - 1];
    let half = f64::from(b.thickness_cm) / 2.0;
    let mut best: Option<((u32, bool), usize, f64, f64)> = None;
    for (i, s) in b.line.windows(2).enumerate() {
        let u = sub(s[1], s[0]);
        let length = (u[0] * u[0] + u[1] * u[1]).sqrt();
        // The prism's side vector is the segment's rounded normal.
        if [(-u[1] / length * half).round() as i64, (u[0] / length * half).round() as i64] != n {
            continue;
        }
        let param = |e: Point| {
            let w = sub(e, s[0]);
            ((w[0] * u[0] + w[1] * u[1]) / (length * length), (u[0] * w[1] - u[1] * w[0]).abs() / length)
        };
        let ((tp, dp), (tq, dq)) = (param(p), param(q));
        let (before, after) = (i > 0 || closed, i + 2 < count || closed);
        let lo = if before { -half / length } else { 0.0 };
        let hi = 1.0 + if after { half / length } else { 0.0 };
        let slack = 1.0 / length;
        if dp > 1.0 || dq > 1.0 || tp < lo - slack || tq > hi + slack || tp >= tq {
            continue;
        }
        let explained = |e: Point, t: f64| u32::from((before && e == point(s[0], u, lo)) || (after && e == point(s[0], u, hi))
            || e == s[0] || e == s[1] || on_cut(e, s[0], u, t, bounds, spacing));
        let mid = (tp + tq) / 2.0;
        let score = (explained(p, tp) + explained(q, tq), (0.0..=1.0).contains(&mid));
        if best.is_none_or(|(held, ..)| score > held) {
            best = Some((score, i, tp, tq));
        }
    }
    best.map(|(_, i, tp, tq)| (i, tp, tq))
}

fn scene(x: f64, y: f64, z_map: f64) -> [f32; 3] {
    let s = mapkit_core::WORLD_SCALE / 100.0;
    [(x * s) as f32, (y * s) as f32, (-z_map * s) as f32]
}

/// The cell's runs and posts from its prisms: `convexes` yields each
/// generated convex's object ID and 24 vertex values (map cm, x/y/z).
pub(crate) fn list<'a>(d: &MapDocument, cell: Cell, convexes: impl Iterator<Item = (&'a str, &'a [i64])>) -> Result<Listed> {
    let mut listed = Listed { runs: Vec::new(), posts: Vec::new(), ids: Vec::new(), capped: false };
    if d.barriers.is_empty() {
        return Ok(listed);
    }
    let bounds = d.cell_bounds(cell)?;
    let spacing = d.heightmaps.iter().find(|h| h.cell == cell).map(|h| f64::from(h.spacing_cm));
    let denominator = mapkit_core::osm_source_denominator(d).unwrap_or(1);
    let find = |id: &str| d.barriers.binary_search_by(|b| b.id.as_str().cmp(id)).ok().or_else(|| d.barriers.iter().position(|b| b.id == id));
    let mut pieces = Vec::new();
    for (id, v) in convexes {
        mapkit_core::cancellation::checkpoint()?;
        if v.len() != 24 {
            continue;
        }
        let Some(barrier) = find(id) else { continue };
        let b = &d.barriers[barrier];
        let at = |k: usize| [v[k * 3], v[k * 3 + 1], v[k * 3 + 2]];
        // Vertex bits: 0 the end (p, q), 1 the top, 2 the side (+n).
        let (p, q) = ([(at(0)[0] + at(4)[0]) / 2, (at(0)[2] + at(4)[2]) / 2], [(at(1)[0] + at(5)[0]) / 2, (at(1)[2] + at(5)[2]) / 2]);
        let n = [(at(4)[0] - at(0)[0]) / 2, (at(4)[2] - at(0)[2]) / 2];
        // An unmatched prism (none expected) still draws as a plain run.
        let (segment, tp, tq) = locate(b, p, q, n, &bounds, spacing).unwrap_or((usize::MAX, 0.0, 1.0));
        let height = b.height_cm as i64;
        pieces.push(Piece { barrier, segment, tp, tq, p, q, n, ground: [(at(2)[1] - height) as f64, (at(3)[1] - height) as f64] });
    }
    let mut lines: BTreeMap<usize, Line> = BTreeMap::new();
    // Gate arcs of each barrier in this cell: lattice posts give way to
    // them in every piece, not only the piece the gate ends.
    let mut gates: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for piece in pieces.iter().filter(|piece| piece.segment != usize::MAX) {
        let b = &d.barriers[piece.barrier];
        let line = lines.entry(piece.barrier).or_insert_with(|| Line::new(b, denominator));
        let u = sub(b.line[piece.segment + 1], b.line[piece.segment]);
        let length = (u[0] * u[0] + u[1] * u[1]).sqrt();
        for ((_, gate), t) in ends(b, line, piece, &bounds, spacing).into_iter().zip([piece.tp, piece.tq]) {
            if gate { gates.entry(piece.barrier).or_default().push(line.cum[piece.segment] + t * length); }
        }
    }
    let mut names: BTreeMap<usize, usize> = BTreeMap::new();
    for piece in &pieces {
        mapkit_core::cancellation::checkpoint()?;
        let b = &d.barriers[piece.barrier];
        let line = lines.entry(piece.barrier).or_insert_with(|| Line::new(b, denominator));
        let index = *names.entry(piece.barrier).or_insert_with(|| {
            listed.ids.push(b.id.clone());
            listed.ids.len() - 1
        }) as f32;
        let height = (f64::from(b.height_cm) * mapkit_core::WORLD_SCALE / 100.0) as f32;
        let bits = line.bits;
        if piece.segment == usize::MAX {
            let (o, e) = (scene(piece.p[0] as f64, piece.ground[0], piece.p[1] as f64), scene(piece.q[0] as f64, piece.ground[1], piece.q[1] as f64));
            let (x, z) = (scene((piece.q[0] - piece.p[0]) as f64, 0.0, (piece.q[1] - piece.p[1]) as f64), scene(2.0 * piece.n[0] as f64, 0.0, 2.0 * piece.n[1] as f64));
            let sign = if x[0] * z[2] - x[2] * z[0] > 0.0 { 1.0 } else { -1.0 };
            listed.runs.extend([o[0], o[1], o[2], e[0] - o[0], e[1] - o[1], e[2] - o[2], sign * z[0], sign * z[2], height, bits as f32, 0.0, 0.0, index]);
            continue;
        }
        let count = b.line.len();
        let i = piece.segment;
        let (a, end) = (b.line[i], b.line[i + 1]);
        let u = sub(end, a);
        let length = (u[0] * u[0] + u[1] * u[1]).sqrt();
        let half = f64::from(b.thickness_cm) / 2.0;
        // The prism's half width square to its own centreline.
        let half_perp = {
            let axis = sub(piece.q, piece.p);
            ((axis[0] * piece.n[1] as f64 - axis[1] * piece.n[0] as f64) / (axis[0] * axis[0] + axis[1] * axis[1]).sqrt()).abs()
        };
        let joint_before = i > 0 || line.closed;
        let joint_after = i + 2 < count || line.closed;
        // Turn at the segment's start and end vertices (a closed line's last
        // vertex is its first).
        let (turn_start, turn_end) = (line.turn[i], line.turn[if i + 2 == count && line.closed { 0 } else { i + 1 }]);
        let [(start_end, start_gate), (end_end, end_gate)] = ends(b, line, piece, &bounds, spacing);
        // Only a joint's extension lies past its vertex (the length lattice
        // always cuts a segment's start, so the start extension is a piece
        // of its own). Clamp it to the vertex at a hard joint, to a mitre's
        // reach past a gentle one.
        let mitre = |turn: f64| (half_perp * (turn.to_radians() / 2.0).tan() + 0.05) / length;
        let (hard_start, hard_end) = (joint_before && turn_start >= HARD_JOINT_DEGREES, joint_after && turn_end >= HARD_JOINT_DEGREES);
        let ta = if joint_before { piece.tp.max(if hard_start { 0.0 } else { -mitre(turn_start) }) } else { piece.tp };
        let tb = if joint_after { piece.tq.min(if hard_end { 1.0 } else { 1.0 + mitre(turn_end) }) } else { piece.tq };
        // The pieces that start or end at a hard vertex.
        let start_hard = hard_start && piece.tp <= 1e-9 && piece.tq > 1e-9;
        let end_hard = hard_end && piece.tq >= 1.0 - 1e-9 && piece.tp < 1.0 - 1e-9;
        let ground = |t: f64| piece.ground[0] + (piece.ground[1] - piece.ground[0]) * (t - piece.tp) / (piece.tq - piece.tp);
        // Points along the prism's own (rounded) centreline p -> q, which
        // can sit a few millimetres off the source line: runs and posts
        // follow the prism they are drawn in.
        let centre = |t: f64| {
            let f = (t - piece.tp) / (piece.tq - piece.tp);
            [piece.p[0] as f64 + (piece.q[0] - piece.p[0]) as f64 * f, piece.p[1] as f64 + (piece.q[1] - piece.p[1]) as f64 * f]
        };
        let axis = sub(piece.q, piece.p);
        let span = (axis[0] * axis[0] + axis[1] * axis[1]).sqrt();
        let arc = |t: f64| line.cum[i] + t * length;
        let kind = design::kind(bits);
        let dd = f64::from(design::denominator(bits));
        // Right-handed frames: side vector z with det[x, up, z] > 0.
        let side = {
            let z = scene(2.0 * piece.n[0] as f64, 0.0, 2.0 * piece.n[1] as f64);
            let x = scene(axis[0], 0.0, axis[1]);
            if x[0] * z[2] - x[2] * z[0] > 0.0 { [z[0], z[2]] } else { [-z[0], -z[2]] }
        };
        if tb > ta + 1e-9 {
            let (o, e) = (centre(ta), centre(tb));
            let origin = scene(o[0], ground(ta), o[1]);
            let tip = scene(e[0], ground(tb), e[1]);
            let flags = u32::from(start_end) * START_SPAN_END | u32::from(end_end) * END_SPAN_END
                | u32::from(start_hard) * START_HARD_JOINT | u32::from(end_hard) * END_HARD_JOINT;
            listed.runs.extend([origin[0], origin[1], origin[2], tip[0] - origin[0], tip[1] - origin[1], tip[2] - origin[2], side[0], side[1],
                height, bits as f32, (arc(ta) / line.bay_cm).rem_euclid(1.0) as f32, flags as f32, index]);
        }
        // Walls and hedges: the corner run over a hard joint's outer wedge,
        // owned by the segment starting there (see the module notes).
        if kind != design::KIND_FENCE && start_hard {
            let reach = half_perp * CORNER_SHRINK / length;
            let (o, e) = (centre(-reach), centre(reach));
            let origin = scene(o[0], ground(-reach), o[1]);
            let tip = scene(e[0], ground(reach), e[1]);
            let k = CORNER_SHRINK as f32;
            listed.runs.extend([origin[0], origin[1], origin[2], tip[0] - origin[0], tip[1] - origin[1], tip[2] - origin[2], side[0] * k, side[1] * k,
                height * k, bits as f32, (arc(-reach) / line.bay_cm).rem_euclid(1.0) as f32, RUN_CORNER as f32, index]);
        }
        if kind == design::KIND_HEDGE {
            continue;
        }
        // Posts and piers: a size factor (gate posts 1.25) kept inside the
        // prism across, and past the vertex for corner posts, at every scale.
        let family = design::family(bits) as usize;
        let fence = kind == design::KIND_FENCE;
        let along_m = if fence { FENCE_POST_HALF_M[family] } else { PIER_HALF_M };
        let fit = |size: f64, corner: bool| {
            let mut size = size;
            if fence { size = size.min((half_perp - CLEARANCE_CM) * dd / (100.0 * FENCE_POST_ACROSS_M[family])); }
            if corner { size = size.min((half.min(half_perp) - CLEARANCE_CM) * dd / (100.0 * along_m)); }
            size.max(0.05)
        };
        let slope = (piece.ground[1] - piece.ground[0]) / span;
        let mut post = |t: f64, kind_of: u32, size: f64| {
            let c = centre(t);
            let origin = scene(c[0], ground(t), c[1]);
            // Centimetres per mesh metre (real) at this size, with the rise.
            let k = 100.0 * size / dd;
            let x = scene(axis[0] / span * k, slope * k, axis[1] / span * k);
            listed.posts.extend([origin[0], origin[1], origin[2], x[0], x[1], x[2], side[0], side[1], height, bits as f32, kind_of as f32, size as f32, index]);
        };
        // The clamped, half-open arc range this piece owns.
        let (owned_lo, owned_hi) = (arc(piece.tp.max(0.0)), arc(piece.tq.min(1.0)));
        let reach = 0.3 * line.bay_cm;
        if design::has_lattice(bits) && owned_hi > owned_lo {
            let mut span_ends: Vec<f64> = [(start_end, arc(piece.tp)), (end_end, arc(piece.tq))].into_iter().filter(|e| e.0).map(|e| e.1).collect();
            span_ends.extend(gates.get(&piece.barrier).into_iter().flatten());
            // A gentle closed line's post at arc 0 also stands at its total.
            if line.gentle_closure { span_ends.push(line.total()); }
            let mut k = (owned_lo / line.bay_cm).ceil() as i64;
            while (k as f64) * line.bay_cm < owned_hi {
                let s = k as f64 * line.bay_cm;
                k += 1;
                if s < owned_lo || line.blocked(s, reach) || span_ends.iter().any(|e| (e - s).abs() < reach && !(line.gentle_closure && s == 0.0 && *e == line.total())) {
                    continue;
                }
                // A post never straddles a joint or its piece's ends (a
                // terrain crease or another segment lies beyond): within its
                // half length it steps back into its piece.
                let size = fit(1.0, false);
                let clear = along_m * size * 100.0 / dd + CLEARANCE_CM;
                let (lo, hi) = (arc(piece.tp.max(0.0)) + clear, arc(piece.tq.min(1.0)) - clear);
                if lo > hi {
                    continue;
                }
                post((s.clamp(lo, hi) - line.cum[i]) / length, POST_LATTICE, size);
            }
        }
        // End and gate posts just inside the span.
        for (is_end, gate, t, inward) in [(start_end, start_gate, piece.tp, 1.0), (end_end, end_gate, piece.tq, -1.0)] {
            if !is_end {
                continue;
            }
            // Inside its own piece (a crease may lie beyond): a span-end
            // piece too short for the post takes a slimmer one.
            let mut size = fit(if gate { GATE_SIZE } else { 1.0 }, false);
            let room = (piece.tq - piece.tp) * length / 2.0 - CLEARANCE_CM;
            let needed = along_m * size * 100.0 / dd;
            if needed > room {
                size *= room.max(0.0) / needed;
                if size < 0.4 {
                    continue;
                }
            }
            let offset = (along_m * size * 100.0 / dd + CLEARANCE_CM) / length;
            post(t + inward * offset, if gate { POST_GATE } else { POST_END }, size);
        }
        // One corner post at a hard joint, owned by the segment starting there.
        if start_hard && piece.tp <= 0.0 && 0.0 < piece.tq {
            post(0.0, POST_CORNER, fit(1.0, true));
        }
    }
    if listed.runs.len() / RUN_FLOATS > MAX_RUNS || listed.posts.len() / POST_FLOATS > MAX_POSTS {
        return Ok(Listed { runs: Vec::new(), posts: Vec::new(), ids: Vec::new(), capped: true });
    }
    Ok(listed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapkit_core::barrier::BarrierKind;

    /// The minimal example (ground road along y = 25600, bridge along x =
    /// 25600, 512 m cells) at 1:3 with a sloped 400 cm heightmap in cell
    /// (0, 0) and test barriers.
    fn document() -> MapDocument {
        let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        d.buildings.clear();
        d.zones.clear();
        d.attributions.push(mapkit_core::Attribution { source: "osm#layer".into(), license: "ODbL-1.0".into(),
            notice: r#"{"adapter":"osm-extract-v1","authored_units":{"metres_per_unit":1,"source_denominator":3}}"#.into() });
        let barrier = |id: &str, kind, line: Vec<Point>, height_cm, thickness_cm| Barrier { id: id.into(), kind, line, height_cm, thickness_cm };
        d.barriers = vec![
            // Crosses the ground road (a gate) and the cell seam at y = 51200.
            barrier("fence-gate", BarrierKind::Fence, vec![[10000, 20000], [10000, 60000]], 40, 5),
            // A 45° (hard) and then a 6° (gentle) turn.
            barrier("fence-bend", BarrierKind::Fence, vec![[30000, 5000], [38100, 5000], [44100, 11000], [44500, 11500]], 67, 5),
            barrier("hedge-1", BarrierKind::Hedge, vec![[5000, 40000], [9000, 40000]], 27, 20),
            barrier("wall-loop", BarrierKind::Wall, vec![[60000, 5000], [70000, 5000], [70000, 15000], [60000, 15000], [60000, 5000]], 67, 10),
            // A right-angled hedge corner.
            barrier("hedge-corner", BarrierKind::Hedge, vec![[5000, 45000], [9000, 45000], [9000, 49000]], 27, 20),
            // A gentle closed ring (12° turns at every vertex, the closing one included).
            barrier("fence-ring", BarrierKind::Fence, {
                let mut ring: Vec<Point> = (0..30).map(|k| {
                    let a = (k as f64 * 12.0).to_radians();
                    [40000 + (3000.0 * a.cos()).round() as i64, 30000 + (3000.0 * a.sin()).round() as i64]
                }).collect();
                ring.push(ring[0]);
                ring
            }, 40, 5),
        ];
        d.barriers.sort_by(|a, b| a.id.cmp(&b.id));
        d.heightmaps.push(mapkit_core::Heightmap { cell: Cell { x: 0, y: 0 }, path: "h.png".into(), spacing_cm: 400, offset_cm: 0, step_cm: 1, source_accuracy_cm: None });
        d.validate().unwrap();
        d
    }
    fn generated(d: &MapDocument, cell: Cell) -> mapkit_core::GeneratedChunk {
        let side = 51200 / 400 + 1;
        // Rises 3 cm per 400 cm along x and 2 cm per 400 cm along y.
        let grid = mapkit_core::HeightGrid { side, heights_cm: (0..side * side).map(|i| ((i % side) * 3 + (i / side) * 2) as i64).collect() };
        mapkit_core::generate(mapkit_core::GenerationInput { document: d, cell, heightgrid: (cell == Cell { x: 0, y: 0 }).then_some(&grid), max_triangles: 500_000 }).unwrap()
    }
    fn listed(d: &MapDocument, cell: Cell) -> Listed {
        let chunk = generated(d, cell);
        let flat: Vec<(String, Vec<i64>)> = chunk.asset_convexes.iter().map(|c| (c.object_id.clone(), c.shape.vertices.iter().flatten().copied().collect())).collect();
        list(d, cell, flat.iter().map(|(id, v)| (id.as_str(), v.as_slice()))).unwrap()
    }
    struct Run { start: [f32; 3], end: [f32; 3], phase: f32, flags: u32, bits: u32, id: String }
    fn runs(l: &Listed) -> Vec<Run> {
        l.runs.chunks(RUN_FLOATS).map(|r| Run { start: [r[0], r[1], r[2]], end: [r[0] + r[3], r[1] + r[4], r[2] + r[5]], phase: r[10], flags: r[11] as u32, bits: r[9] as u32,
            id: l.ids[r[12] as usize].clone() }).collect()
    }
    struct Post { at: [f32; 3], kind: u32, id: String }
    fn posts(l: &Listed) -> Vec<Post> {
        l.posts.chunks(POST_FLOATS).map(|p| Post { at: [p[0], p[1], p[2]], kind: p[10] as u32, id: l.ids[p[12] as usize].clone() }).collect()
    }
    fn close(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
    }

    #[test]
    fn span_ends_are_line_ends_and_gates_never_seams_lattice_or_grid_cuts() {
        let d = document();
        let lists = [listed(&d, Cell { x: 0, y: 0 }), listed(&d, Cell { x: 0, y: 1 })];
        let gate: Vec<Run> = lists.iter().flat_map(runs).filter(|r| r.id == "fence-gate").collect();
        // Cut at the 400 cm grid, the tile diagonals do not cross a line along y, the 800 cm lattice and the seam.
        assert!(gate.len() > 40, "{}", gate.len());
        let mut ends = Vec::new();
        for r in &gate {
            assert!((r.start[0] - 100.0).abs() < 1e-3 && (r.end[0] - 100.0).abs() < 1e-3);
            if r.flags & START_SPAN_END != 0 { ends.push(-r.start[2]); }
            if r.flags & END_SPAN_END != 0 { ends.push(-r.end[2]); }
            assert_eq!(r.flags & (START_HARD_JOINT | END_HARD_JOINT), 0);
        }
        ends.sort_by(f32::total_cmp);
        assert_eq!(ends.len(), 4, "line ends and both sides of the road gate: {ends:?}");
        assert!((ends[0] - 200.0).abs() < 1e-3 && (ends[3] - 600.0).abs() < 1e-3, "{ends:?}");
        assert!(ends[1] < 256.0 - 4.0 && ends[2] > 256.0 + 4.0 && ends[2] - ends[1] < 30.0, "the gate around the road: {ends:?}");
        // Runs follow the sloped ground of cell (0, 0): 2 cm up per 400 cm along y.
        for r in gate.iter().filter(|r| -r.end[2] < 512.0) {
            let ground = |z: f32| (-z * 100.0 / 400.0 * 2.0 + 10000.0 / 400.0 * 3.0) / 100.0;
            assert!((r.start[1] - ground(r.start[2])).abs() < 0.011 && (r.end[1] - ground(r.end[2])).abs() < 0.011, "{:?} {:?}", r.start, r.end);
        }
    }

    #[test]
    fn posts_stand_once_per_bay_across_pieces_and_cells() {
        let d = document();
        let lists = [listed(&d, Cell { x: 0, y: 0 }), listed(&d, Cell { x: 0, y: 1 })];
        let bits = runs(&lists[0]).into_iter().find(|r| r.id == "fence-gate").unwrap().bits;
        let bay = (design::bay_m(bits) * 100.0 / 3.0) as f32 / 100.0;
        let mut at: Vec<(f32, u32)> = lists.iter().flat_map(posts).filter(|p| p.id == "fence-gate").map(|p| (-p.at[2], p.kind)).collect();
        at.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(at.len() > 40, "{}", at.len());
        for pair in at.windows(2) {
            assert!(pair[1].0 - pair[0].0 > 0.29 * bay, "posts {pair:?} closer than 0.3 bay ({bay} m)");
        }
        for (y, kind) in &at {
            if *kind == POST_LATTICE {
                // On the lattice, or stepped back inside its piece by at most
                // its half length (plus clearance) from a cut.
                let k = (y - 200.0) / bay;
                assert!((k - k.round()).abs() * bay < 0.0135, "lattice post at {y} off the {bay} m lattice");
            }
        }
        assert_eq!(at.iter().filter(|p| p.1 == POST_END).count(), 2);
        assert_eq!(at.iter().filter(|p| p.1 == POST_GATE).count(), 2);
        assert_eq!(at.iter().filter(|p| p.1 == POST_CORNER).count(), 0);
    }

    #[test]
    fn phases_continue_from_piece_to_piece() {
        let d = document();
        let mut all: Vec<Run> = [Cell { x: 0, y: 0 }, Cell { x: 0, y: 1 }].iter().flat_map(|c| runs(&listed(&d, *c))).filter(|r| r.id == "fence-gate").collect();
        all.sort_by(|a, b| b.start[2].total_cmp(&a.start[2]));
        let bay = (design::bay_m(all[0].bits) * 100.0 / 3.0) as f32 / 100.0;
        let mut joined = 0;
        for pair in all.windows(2) {
            if !close([pair[0].end[0], 0.0, pair[0].end[2]], [pair[1].start[0], 0.0, pair[1].start[2]], 1e-4) { continue; }
            joined += 1;
            // Cell (0, 0) has the sloped heightmap, cell (0, 1) flat ground.
            if pair[0].end[2] > -512.0 { assert!(close(pair[0].end, pair[1].start, 1e-4), "joined runs share their ground point"); }
            let length = -(pair[0].end[2] - pair[0].start[2]);
            let carried = (pair[0].phase + length / bay).rem_euclid(1.0);
            let delta = (carried - pair[1].phase).abs();
            assert!(delta.min(1.0 - delta) < 1e-3, "phase {carried} -> {}", pair[1].phase);
        }
        assert!(joined > 30, "{joined}");
    }

    #[test]
    fn hard_joints_clamp_with_one_corner_post_and_gentle_joints_mitre() {
        let d = document();
        let l = listed(&d, Cell { x: 0, y: 0 });
        let bend: Vec<Run> = runs(&l).into_iter().filter(|r| r.id == "fence-bend").collect();
        let corner = [381.0f32, 0.0, -50.0];
        let flat = |p: [f32; 3]| [p[0], 0.0, p[2]];
        assert_eq!(bend.iter().filter(|r| close(flat(r.end), corner, 1e-4)).count(), 1, "the first segment ends at the hard vertex");
        assert_eq!(bend.iter().filter(|r| close(flat(r.start), corner, 1e-4)).count(), 1, "the second starts there");
        assert!(bend.iter().any(|r| r.flags & END_HARD_JOINT != 0) && bend.iter().any(|r| r.flags & START_HARD_JOINT != 0));
        let corners: Vec<Post> = posts(&l).into_iter().filter(|p| p.id == "fence-bend" && p.kind == POST_CORNER).collect();
        assert_eq!(corners.len(), 1);
        assert!(close(flat(corners[0].at), corner, 1e-4));
        // The 6° joint at (441, 110): the first run reaches a mitre past the
        // vertex (about 0.1 cm), the second starts as far before it.
        let gentle = [441.0f32, 0.0, -110.0];
        // (The next segment's start extension, its own piece, reaches as far
        // back before it.)
        let reaching: Vec<&Run> = bend.iter().filter(|r| close(flat(r.end), gentle, 0.005) && !close(flat(r.start), gentle, 0.01)).collect();
        assert_eq!(reaching.len(), 1);
        let back = bend.iter().find(|r| close(flat(r.end), gentle, 1e-5)).expect("the start extension ends at the vertex");
        assert!(back.start[0] < gentle[0] && close(flat(back.start), gentle, 0.005));
        assert!(!close(flat(reaching[0].end), gentle, 1e-5) && reaching[0].end[0] > gentle[0], "{:?}", reaching[0].end);
        assert!(!posts(&l).iter().any(|p| p.id == "fence-bend" && close(flat(p.at), gentle, 0.01)));
    }

    #[test]
    fn walls_get_corner_piers_and_hedges_no_posts() {
        let d = document();
        let l = listed(&d, Cell { x: 1, y: 0 });
        let piers: Vec<Post> = posts(&l).into_iter().filter(|p| p.id == "wall-loop").collect();
        let bits = runs(&l).into_iter().find(|r| r.id == "wall-loop").unwrap().bits;
        assert_eq!(piers.iter().filter(|p| p.kind == POST_CORNER).count(), 4, "one pier at each corner of the closed loop");
        assert!(piers.iter().all(|p| p.kind == POST_CORNER || p.kind == POST_LATTICE));
        assert_eq!(piers.iter().any(|p| p.kind == POST_LATTICE), design::has_lattice(bits));
        let near = listed(&d, Cell { x: 0, y: 0 });
        assert!(runs(&near).iter().any(|r| r.id == "hedge-1") && !posts(&near).iter().any(|p| p.id == "hedge-1"));
        // Every hard wall joint also carries a corner run over its outer wedge.
        let corners: Vec<Run> = runs(&l).into_iter().filter(|r| r.id == "wall-loop" && r.flags & RUN_CORNER != 0).collect();
        assert_eq!(corners.len(), 4, "one corner run at each corner of the loop");
    }

    #[test]
    fn hard_hedge_and_wall_joints_fill_the_outer_wedge_inside_the_prisms() {
        let d = document();
        let l = listed(&d, Cell { x: 0, y: 0 });
        let hedge: Vec<Run> = runs(&l).into_iter().filter(|r| r.id == "hedge-corner").collect();
        let corners: Vec<&Run> = hedge.iter().filter(|r| r.flags & RUN_CORNER != 0).collect();
        assert_eq!(corners.len(), 1, "one corner run, owned by the segment starting at the vertex");
        assert!(!posts(&l).iter().any(|p| p.id == "hedge-corner"), "hedges have no posts");
        let c = corners[0];
        let mid = [(c.start[0] + c.end[0]) / 2.0, (c.start[2] + c.end[2]) / 2.0];
        assert!((mid[0] - 90.0).abs() < 1e-3 && (mid[1] + 450.0).abs() < 1e-3, "centred on the vertex: {mid:?}");
        // Along the second segment (+y map = -z scene), 0.98 x the half width each way.
        let along = (c.end[2] - c.start[2]).abs();
        assert!((c.end[0] - c.start[0]).abs() < 1e-4 && (along - 2.0 * 0.098).abs() < 2e-3, "{along}");
        // The plain runs still stop at the vertex.
        let flat = |p: [f32; 3]| [p[0], 0.0, p[2]];
        assert_eq!(hedge.iter().filter(|r| r.flags & RUN_CORNER == 0 && close(flat(r.end), [90.0, 0.0, -450.0], 1e-4)).count(), 1);
        // Fences keep their corner post and get no corner run.
        assert!(!runs(&l).iter().any(|r| r.id == "fence-bend" && r.flags & RUN_CORNER != 0));
    }

    #[test]
    fn lattice_posts_run_round_a_gentle_closed_line_once() {
        let d = document();
        let l = listed(&d, Cell { x: 0, y: 0 });
        let ring: Vec<Post> = posts(&l).into_iter().filter(|p| p.id == "fence-ring").collect();
        let bits = runs(&l).into_iter().find(|r| r.id == "fence-ring").unwrap().bits;
        let bay = (design::bay_m(bits) * 100.0 / 3.0) as f32 / 100.0;
        assert!(ring.len() > 20 && ring.iter().all(|p| p.kind == POST_LATTICE), "{}", ring.len());
        let mut angles: Vec<f32> = ring.iter().map(|p| (-p.at[2] - 300.0).atan2(p.at[0] - 400.0)).collect();
        angles.sort_by(f32::total_cmp);
        let gaps: Vec<f32> = angles.windows(2).map(|w| w[1] - w[0]).chain([angles[0] + std::f32::consts::TAU - angles[angles.len() - 1]]).collect();
        let least = gaps.iter().copied().fold(f32::MAX, f32::min) * 30.0;
        assert!(least > 0.29 * bay, "posts {least} m apart round the closure (bay {bay} m)");
    }

    #[test]
    fn lists_are_deterministic_and_ignore_other_convexes() {
        let d = document();
        let cell = Cell { x: 0, y: 0 };
        let chunk = generated(&d, cell);
        let mut flat: Vec<(String, Vec<i64>)> = chunk.asset_convexes.iter().map(|c| (c.object_id.clone(), c.shape.vertices.iter().flatten().copied().collect())).collect();
        let a = list(&d, cell, flat.iter().map(|(id, v)| (id.as_str(), v.as_slice()))).unwrap();
        flat.push(("tree".into(), vec![0; 24]));
        flat.push(("fence-gate".into(), vec![0; 12]));
        let b = list(&d, cell, flat.iter().map(|(id, v)| (id.as_str(), v.as_slice()))).unwrap();
        assert!(!a.runs.is_empty() && !a.capped);
        assert_eq!((a.runs.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), a.posts.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), &a.ids),
            (b.runs.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.posts.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), &b.ids));
        // Every run's frame is right-handed with up as y.
        for r in a.runs.chunks(RUN_FLOATS) {
            assert!(r[3] * r[7] - r[5] * r[6] > 0.0);
        }
    }
}
