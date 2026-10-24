//! Single-pass motif growth over the padded piece grid. Counters change per
//! placed piece and are restored from per-motif snapshots when a motif pops.
use super::*;
use motifs::{Family, Motif, FAMILIES};
use seed_streams::{derive, uniform};
use std::ops::Range;

pub(super) const MAX_STRAIGHT_CM: u64 = 3200;
pub(super) const ATTEMPTS: u64 = 4;
const DRAWS: usize = 8;
const POPS_IN_ROW: usize = 3;
const POPS: usize = 48;
/// Forced homing gives up after this many more pieces.
const FORCED_PIECES: usize = 40;
pub(super) const HEIGHT_CM: (i64, i64) = (-1600, 3600);
/// Homing aims at the approach pose 40 m before the start, facing the start.
const APPROACH_CM: i64 = 4000;
pub(super) const START_WIDTH: u32 = 800;
/// Circuit growth keeps this box behind the start (x0, z0, x1, z1) free for the return.
const CORRIDOR_CM: [i64; 4] = [-2000, -5000, 2000, -100];

pub(super) fn yaw_mdeg(f: Vertex) -> i32 {
    round(libm::atan2(f[0] as f64, f[2] as f64) * 180000.0 / std::f64::consts::PI) as i32
}
/// Longest optional straight in `p` and its trailing run, continuing `tail`.
/// Horizontal heading decides, so grades and waves count as straight.
pub(super) fn straight_run(p: &Piece, mut tail: u64) -> (u64, u64) {
    if category(&p.id) != "driving" || ["tube_entry", "tube_exit", "approach", "finish_plaza"].contains(&p.id.as_str()) { return (0, 0); }
    let mut longest = tail;
    for w in p.path.windows(2) {
        let (a, b) = (w[0].forward, w[1].forward);
        if (a[0] as i128 * b[2] as i128 - a[2] as i128 * b[0] as i128).abs() < 5_000_000_000 {
            tail += distance(w[0].position_cm, w[1].position_cm);
            longest = longest.max(tail);
        } else { tail = 0; }
    }
    (longest, tail)
}
/// Horizontal heading change in degrees: (signed, right positive), left, right.
pub(super) fn turning(p: &Piece) -> (f64, f64, f64) {
    let (mut left, mut right) = (0.0, 0.0);
    for w in p.path.windows(2) {
        let (a, b) = (w[0].forward.map(|v| v as f64), w[1].forward.map(|v| v as f64));
        let t = libm::atan2(a[2] * b[0] - a[0] * b[2], a[0] * b[0] + a[2] * b[2]).to_degrees();
        if t > 0.0 { right += t } else { left -= t }
    }
    (right - left, left, right)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Counters {
    pub dist: u64,
    pub samples: usize,
    pub counted: usize,
    pub tail: u64,
    pub longest: u64,
    pub yaw: f64,
    pub left: f64,
    pub right: f64,
    pub loops: u8,
    pub overpasses: u8,
    pub last_pipe: Option<usize>,
    pub last_jump: Option<usize>,
    pub recent: [Option<Family>; 2],
}
struct Placed { start: usize, end: usize, before: Counters, reserved: bool }
pub(super) struct Grown {
    pub pieces: Vec<Piece>,
    pub reserved: Vec<Range<usize>>,
    pub stats: Stats,
}
/// Search effort of one attempt, for tests and tuning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats { pub attempts: u64, pub pops: usize, pub searches: usize, pub nodes: usize }
impl Stats {
    pub(super) fn add(&mut self, o: Stats) { (self.attempts, self.pops, self.searches, self.nodes) = (o.attempts, self.pops + o.pops, self.searches + o.searches, self.nodes + o.nodes); }
}

pub(super) struct Grow<'a> {
    pub s: &'a Settings,
    pub tier: &'static Tier,
    style: &'a motifs::Style,
    attempt: u64,
    pub pieces: Vec<Piece>,
    pub grid: track_grid::PieceGrid,
    pub cache: piece_cache::PieceCache,
    motifs: Vec<Placed>,
    pub c: Counters,
    retries: Vec<u64>,
    /// Earliest piece count for the next bounded closure search.
    pub next_search: usize,
    pub stats: Stats,
}
impl<'a> Grow<'a> {
    fn new(s: &'a Settings, style: &'a motifs::Style, attempt: u64) -> Self {
        let mut g = Self { s, tier: s.length.tier(), style, attempt, pieces: vec![], grid: Default::default(), cache: Default::default(),
            motifs: vec![], c: Counters::default(), retries: vec![], next_search: 0, stats: Stats { attempts: attempt + 1, ..Stats::default() } };
        let mut at = [0; 3];
        for _ in 0..START_PIECES {
            let p = g.cache.instance(&authoring::Instance { position_cm: at, ..authoring::instance("", "straight", START_WIDTH) });
            at = p.path.last().unwrap().position_cm;
            g.commit(p);
        }
        (g.c.counted, g.c.tail, g.c.longest) = (0, 0, 0);
        g
    }
    pub fn has(&self, category: &str) -> bool { self.s.categories.iter().any(|c| c == category) }
    pub fn end(&self) -> &Sample { self.pieces.last().unwrap().path.last().unwrap() }
    /// `id` placed on `end`, after a piece `previous`, exactly as compile builds it.
    pub fn piece(&mut self, id: &str, w: u32, exit: Option<u32>, end: &Sample, previous: &str) -> Piece {
        let mut origin = end.position_cm;
        origin[1] -= portal_drop(previous, id, w);
        self.cache.instance(&authoring::Instance { position_cm: origin, rotation_mdeg: [0, yaw_mdeg(end.forward), 0], entry_width_cm: end.lateral_cm * 2,
            exit_width_cm: exit.unwrap_or(w), ..authoring::instance("", id, w) })
    }
    /// Clear of every placed piece except `skip`, the predecessor it joins.
    pub fn clear(&self, p: &Piece, skip: Option<usize>) -> bool {
        !self.grid.candidates(p).into_iter().any(|j| Some(j) != skip && authoring::conflict(p, &self.pieces[j]))
    }
    /// Piece/sample budget, straight limit and height range after `tail`/`samples`/`count`.
    pub fn admissible(&self, p: &Piece, tail: u64, samples: usize, count: usize) -> bool {
        count < MAX_PIECES && samples + p.path.len() + p.alternate_path.len() <= MAX_SAMPLES
            && straight_run(p, tail).0 <= MAX_STRAIGHT_CM
            && (HEIGHT_CM.0..=HEIGHT_CM.1).contains(&p.path.last().unwrap().position_cm[1])
    }
    /// Add a piece that already passed its checks.
    pub fn commit(&mut self, p: Piece) {
        let (longest, tail) = straight_run(&p, self.c.tail);
        let (yaw, left, right) = turning(&p);
        let c = &mut self.c;
        c.dist += race_length(&p);
        c.samples += p.path.len() + p.alternate_path.len();
        c.counted += usize::from(motifs::counted(&p.id));
        (c.longest, c.tail) = (c.longest.max(longest), tail);
        (c.yaw, c.left, c.right) = (c.yaw + yaw, c.left + left, c.right + right);
        self.grid.push(&p);
        self.pieces.push(p);
    }
    fn push(&mut self, p: Piece) -> bool {
        let corridor = self.s.circuit && p.reserved_min_cm[0] < CORRIDOR_CM[2] && CORRIDOR_CM[0] < p.reserved_max_cm[0]
            && p.reserved_min_cm[2] < CORRIDOR_CM[3] && CORRIDOR_CM[1] < p.reserved_max_cm[2];
        if corridor || !self.admissible(&p, self.c.tail, self.c.samples, self.pieces.len()) || !self.clear(&p, Some(self.pieces.len() - 1)) { return false; }
        self.commit(p);
        true
    }
    fn truncate(&mut self, len: usize, c: Counters) {
        self.pieces.truncate(len);
        self.grid.truncate(len);
        self.c = c;
    }
    fn place(&mut self, m: &Motif) -> bool {
        let (start, before) = (self.pieces.len(), self.c);
        for s in &m.steps {
            let (end, previous) = (self.end().clone(), self.pieces.last().unwrap().id.clone());
            let p = self.piece(&s.id, s.width, s.exit, &end, &previous);
            if !self.push(p) {
                self.truncate(start, before);
                return false;
            }
        }
        let (index, c) = (self.motifs.len(), &mut self.c);
        c.recent = [Some(m.family), c.recent[0]];
        if m.family.pipe() { c.last_pipe = Some(index); }
        if m.family == Family::Jump { c.last_jump = Some(index); }
        c.loops += u8::from(m.family == Family::Loop);
        c.overpasses += u8::from(m.family == Family::Overpass);
        self.motifs.push(Placed { start, end: self.pieces.len(), before, reserved: m.reserved });
        true
    }
    fn pop(&mut self) -> bool {
        let Some(m) = self.motifs.pop() else { return false };
        self.truncate(m.start, m.before);
        true
    }
    /// End position, forward and length after `m`, from cached local shapes only.
    fn predict(&mut self, m: &Motif) -> (Vertex, Vertex, u64) {
        let end = self.end();
        let (mut position, mut forward, mut lateral) = (end.position_cm, end.forward, end.lateral_cm);
        let mut previous = self.pieces.last().unwrap().id.clone();
        let mut length = 0;
        for s in &m.steps {
            let rotation = [0, yaw_mdeg(forward), 0];
            let local = self.cache.local(&s.id, s.width, lateral * 2, s.exit.unwrap_or(s.width));
            let last = local.path.last().unwrap();
            position = add(position, geometry::rotate3(last.position_cm, rotation));
            position[1] -= portal_drop(&previous, &s.id, s.width);
            (forward, lateral) = (geometry::rotate3(last.forward, rotation), last.lateral_cm);
            length += race_length(local);
            previous.clone_from(&s.id);
        }
        (position, forward, length)
    }
    fn beta(&self) -> f64 {
        let (t, d) = (self.tier, self.c.dist as f64);
        if d >= t.forced_homing_cm as f64 { return 4.0; }
        let ramp = ((d - t.homing_start_cm as f64) / (t.forced_homing_cm - t.homing_start_cm) as f64).clamp(0.0, 1.0);
        t.homing_max * ramp * libm::sqrt(ramp)
    }
    /// exp(β·(distance gain + ½·heading agreement + height return)) towards the approach pose.
    fn homing(&self, position: Vertex, forward: Vertex, length: u64) -> f64 {
        let beta = self.beta();
        if beta == 0.0 { return 1.0; }
        let from = self.end().position_cm;
        let planar = |a: Vertex| libm::hypot(a[0] as f64, (a[2] + APPROACH_CM) as f64);
        let (d0, d1, len) = (planar(from), planar(position), (length as f64).max(100.0));
        let g = ((d0 - d1) / len).clamp(-1.0, 1.0);
        let k = (d1 / 6000.0).min(1.0);
        let t = [-position[0] as f64 / d1.max(1.0) * k, -(position[2] + APPROACH_CM) as f64 / d1.max(1.0) * k + (1.0 - k)];
        let f = libm::hypot(forward[0] as f64, forward[2] as f64).max(1.0);
        let h = (forward[0] as f64 * t[0] + forward[2] as f64 * t[1]) / f / libm::hypot(t[0], t[1]).max(1e-9);
        let v = ((from[1].abs() - position[1].abs()) as f64 / len * 4.0).clamp(-1.0, 1.0);
        libm::exp(beta * (g + 0.5 * h + v))
    }
    fn weight(&mut self, m: &Motif) -> f64 {
        let (f, n, c) = (m.family, self.motifs.len(), self.c);
        if c.recent == [Some(f), Some(f)] || f.pipe() && c.last_pipe.is_some_and(|i| n - i < 5)
            || f == Family::Jump && c.last_jump.is_some_and(|i| n - i < 4)
            || f == Family::Loop && c.loops >= 2 || f == Family::Overpass && c.overpasses >= 1
            || !self.s.circuit && c.counted + m.counted() > self.tier.sprint_pieces { return 0.0; }
        let mut w = f.base() * self.style.families[f.index()];
        if f.category() == "driving" && !self.has("driving") { w *= 0.5; }
        let (position, forward, length) = self.predict(m);
        if !(HEIGHT_CM.0..=HEIGHT_CM.1).contains(&position[1]) { return 0.0; }
        if self.s.circuit { w *= self.homing(position, forward, length); }
        w
    }
    /// One motif at the current step: ≤8 weighted draws without replacement.
    fn step(&mut self) -> bool {
        let n = self.motifs.len();
        if self.retries.len() <= n { self.retries.resize(n + 1, 0); }
        let (seed, attempt, retry) = (self.s.seed, self.attempt, self.retries[n]);
        let end = self.end();
        let context = motifs::Context { driving: self.has("driving"), action: self.has("action"), lateral: end.lateral_cm, yaw: self.c.yaw, height: end.position_cm[1] };
        let mut options = vec![];
        for f in FAMILIES {
            if !self.has(f.category()) && !f.basic() { continue; }
            let mut rng = derive(seed, seed_streams::MOTIF ^ f.index() as u64, attempt, n as u64, retry);
            let Some(m) = motifs::build(f, &mut rng, self.style, &context) else { continue };
            let w = self.weight(&m);
            if w > 0.0 { options.push((m, w)); }
        }
        let mut rng = derive(seed, seed_streams::PICK, attempt, n as u64, retry);
        for _ in 0..DRAWS {
            let total: f64 = options.iter().map(|o| o.1).sum();
            if !(total > 0.0) { break; }
            let mut ticket = uniform(&mut rng) * total;
            let i = options.iter().position(|o| { let hit = ticket < o.1; ticket -= o.1; hit }).unwrap_or(options.len() - 1);
            let (m, _) = options.remove(i);
            if self.place(&m) { return true; }
        }
        false
    }
    fn grown(self) -> Grown {
        Grown { reserved: self.motifs.iter().filter(|m| m.reserved).map(|m| m.start..m.end).collect(),
            pieces: self.pieces, stats: self.stats }
    }
}

/// One attempt: grow motifs until the sprint size or a circuit closure, or give up.
pub(super) fn grow(s: &Settings, style: &motifs::Style, attempt: u64) -> Result<std::result::Result<Grown, Stats>> {
    let mut g = Grow::new(s, style, attempt);
    let tier = g.tier;
    let (mut in_row, mut forced_at) = (0, None);
    loop {
        cancellation::checkpoint()?;
        cancellation::progress("searching", g.pieces.len() as u64, None, "pieces");
        let placed = if s.circuit {
            if g.c.dist >= tier.forced_homing_cm as u64 && g.pieces.len() > *forced_at.get_or_insert(g.pieces.len()) + FORCED_PIECES { return Ok(Err(g.stats)); }
            if g.c.dist >= tier.closure_cm as u64 && closure::close(&mut g)? { return Ok(Ok(g.grown())); }
            g.step()
        } else if g.c.counted == tier.sprint_pieces {
            let (end, previous) = (g.end().clone(), g.pieces.last().unwrap().id.clone());
            let p = g.piece("finish_plaza", 400, None, &end, &previous);
            if g.push(p) { return Ok(Ok(g.grown())); }
            false
        } else { g.step() };
        if placed { in_row = 0; continue; }
        if in_row == POPS_IN_ROW || g.stats.pops == POPS || !g.pop() { return Ok(Err(g.stats)); }
        (in_row, g.stats.pops) = (in_row + 1, g.stats.pops + 1);
        let n = g.motifs.len();
        g.retries[n] += 1;
    }
}
