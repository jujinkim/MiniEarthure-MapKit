//! Motif grammar: families, a per-seed style vector and concrete piece sequences.
//! Directions are drawn per motif; repetition and spacing rules live in `grow`.
use super::*;
use seed_streams::{draw, uniform};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Family { Sweep, Corner, Hairpins, SBend, Esses, Chicane, Spiral, RiseFall, Straight, Pipe, Halfpipe, Loop, Overpass, Jump, BoostRise, Drop, JumpShortcut, GrindShortcut }
pub(super) const FAMILIES: [Family; 18] = [Family::Sweep, Family::Corner, Family::Hairpins, Family::SBend, Family::Esses, Family::Chicane,
    Family::Spiral, Family::RiseFall, Family::Straight, Family::Pipe, Family::Halfpipe, Family::Loop, Family::Overpass, Family::Jump, Family::BoostRise,
    Family::Drop, Family::JumpShortcut, Family::GrindShortcut];
/// (category, base weight, usable as a connector without the driving category)
const TABLE: [(&str, f64, bool); 18] = [("driving", 13.0, true), ("driving", 12.0, true), ("driving", 7.0, false), ("driving", 9.0, true),
    ("driving", 6.0, false), ("driving", 5.0, false), ("driving", 6.0, true), ("driving", 8.0, true), ("driving", 9.0, true),
    ("gimmick", 7.0, false), ("gimmick", 3.0, false), ("gimmick", 2.5, false), ("gimmick", 1.5, false), ("action", 6.0, false), ("action", 3.0, false),
    ("action", 4.0, false), ("gimmick", 6.0, false), ("gimmick", 6.0, false)];
/// Drop jumps off elevated sections; off if runtime landing physics disagrees.
pub(super) const DROP_JUMPS: bool = true;
/// A drop needs its takeoff this far above the lowest placed road.
pub(super) const DROP_HEIGHT_CM: i64 = 400;
pub(super) const STYLE_WIDTHS: [u32; 4] = [600, 800, 1200, 1800];
const WIDTH_WEIGHTS: [f64; 4] = [35.0, 35.0, 20.0, 10.0];
impl Family {
    pub fn index(self) -> usize { self as usize }
    pub fn category(self) -> &'static str { TABLE[self.index()].0 }
    pub fn base(self) -> f64 { TABLE[self.index()].1 }
    pub fn basic(self) -> bool { TABLE[self.index()].2 }
    pub fn pipe(self) -> bool { matches!(self, Family::Pipe | Family::Halfpipe) }
    pub fn jump(self) -> bool { matches!(self, Family::Jump | Family::Drop) }
}
/// Per-seed character: family multipliers ×0.5..1.5 and a width preference.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Style {
    pub families: [f64; 18],
    pub widths: [f64; 4],
}
impl Style {
    pub fn new(seed: u64) -> Self {
        let mut rng = seed ^ seed_streams::STYLE;
        let families = std::array::from_fn(|_| 0.5 + uniform(&mut rng));
        let widths = std::array::from_fn(|i| WIDTH_WEIGHTS[i] * (0.5 + uniform(&mut rng)));
        Self { families, widths }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Step {
    pub id: String,
    pub width: u32,
    pub exit: Option<u32>,
    /// Local cubic control points: pipe shapes, drop jumps and shortcut roads.
    pub cp: Vec<Vertex>,
}
/// A shortcut template whose base route this motif places; finish adds its branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shortcut { Jump(usize), Grind(&'static str, u32) }
impl Shortcut {
    /// The template source, with (`branch`) or without its optional route.
    pub fn source(self, branch: bool) -> Result<authoring::Source> {
        let mut s = match self { Shortcut::Jump(n) => jump_shortcuts::source(n, branch)?, Shortcut::Grind(id, w) => corner_shortcuts::source(id, w, branch)? };
        narrow::prepare_fragment(&mut s);
        Ok(s)
    }
    pub fn prefix(self) -> &'static str { if matches!(self, Shortcut::Jump(_)) { "j" } else { "r" } }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Motif {
    pub family: Family,
    pub steps: Vec<Step>,
    /// Jump approach and landing roads stay free of automatic obstacles.
    pub reserved: bool,
    pub shortcut: Option<Shortcut>,
}
impl Motif {
    pub fn counted(&self) -> usize { self.steps.iter().filter(|s| counted(&s.id)).count() }
}
/// Pieces that count towards a sprint size: no portal ramps or the finish.
pub(super) fn counted(id: &str) -> bool {
    !["finish_plaza", "tube_entry", "tube_exit"].contains(&id)
}
pub(super) struct Context {
    pub driving: bool,
    pub action: bool,
    /// Half width at the current end.
    pub lateral: u32,
    /// Accumulated signed yaw in degrees, right positive.
    pub yaw: f64,
    pub height: i64,
    /// Lowest placed road height.
    pub low: i64,
    /// Platform count of the next unused jump shortcut ticket.
    pub jump_ticket: Option<usize>,
    pub grind_ticket: bool,
}
fn step(id: impl Into<String>, width: u32) -> Step { Step { id: id.into(), width, exit: None, cp: vec![] } }
fn shaped(id: &str, width: u32, cp: Vec<Vertex>) -> Step { Step { id: id.into(), width, exit: None, cp } }
/// Base-route roads of a shortcut template as local steps (each starts at its own origin heading +Z).
fn template_steps(s: &authoring::Source) -> Vec<Step> {
    s.paths[0].pieces.iter().map(|id| {
        let i = s.instances.iter().find(|i| &i.id == id).unwrap();
        let (a, b) = (i.control_points[0], i.control_points[1]);
        let yaw = round(libm::atan2((b[0] - a[0]) as f64, (b[2] - a[2]) as f64) * 180000.0 / std::f64::consts::PI) as i32;
        let cp = i.control_points.iter().map(|p| geometry::rotate3([p[0] - a[0], p[1] - a[1], p[2] - a[2]], [0, -yaw, 0])).collect();
        Step { id: i.preset.clone(), width: i.width_cm, exit: Some(i.exit_width_cm), cp }
    }).collect()
}
/// Cubic control points of a seeded pipe centreline: a curve, U-turn, S, rising
/// S or wave of straight leads and ≤90° arcs (k = 4/3·tan(φ/4)·R). Empty when
/// no draw fits the pipe source rules; the preset shape is then used.
pub(super) fn pipe_shape(rng: &mut u64, bore: u32, d: i8, halfpipe: bool) -> Vec<Vertex> {
    enum Op { Line(f64), Arc(f64, f64) }
    let min = i64::from(pipe_min_radius_cm(bore));
    let radius = |rng: &mut u64, lo: i64| { let lo = (lo.max(min) + 99) / 100 * 100; (lo + 100 * draw(rng, ((1600 - lo) / 100 + 1) as u64) as i64) as f64 };
    let lead = |rng: &mut u64| (400 + 100 * draw(rng, 13)) as f64;
    let turn = |rng: &mut u64, lo: u64, hi: u64| ((lo + 15 * draw(rng, (hi - lo) / 15 + 1)) as f64).to_radians();
    let d = f64::from(d);
    for _ in 0..8 {
        let kind = draw(rng, if halfpipe { 4 } else { 5 });
        let mut ops = vec![Op::Line(lead(rng))];
        match kind {
            0 => { let t = turn(rng, 45, 180); ops.push(Op::Arc(d * t, radius(rng, 0))); }
            1 => ops.push(Op::Arc(d * std::f64::consts::PI, radius(rng, 1000))),
            3 => {
                let (t, r) = (turn(rng, 30, 60), radius(rng, 0));
                ops.extend([Op::Arc(d * t, r), Op::Arc(-2.0 * d * t, r), Op::Arc(d * t, r)]);
            }
            _ => {
                let t = turn(rng, 45, 90);
                ops.push(Op::Arc(d * t, radius(rng, 0)));
                if uniform(rng) < 0.5 { ops.push(Op::Line(lead(rng) / 2.0)); }
                ops.push(Op::Arc(-d * t, radius(rng, 0)));
            }
        }
        ops.push(Op::Line(lead(rng)));
        // Horizontal points with their stations; heading 0 is +Z, positive turns right (+X).
        let (mut at, mut heading, mut station) = ([0.0f64; 2], 0.0f64, 0.0);
        let mut points = vec![([0.0f64; 2], 0.0, 0.0)];
        let forward = |h: f64| [libm::sin(h), libm::cos(h)];
        for op in &ops {
            let pieces: Vec<(f64, f64)> = match *op {
                Op::Line(l) => vec![(l, 0.0)],
                Op::Arc(a, r) => { let n = (a.abs() / std::f64::consts::FRAC_PI_2 - 1e-9).ceil().max(1.0); vec![(r, a / n); n as usize] }
            };
            for (r, a) in pieces {
                let f0 = forward(heading);
                let (end, k, length) = if a == 0.0 { ([at[0] + f0[0] * r, at[1] + f0[1] * r], r / 3.0, r) } else {
                    let right = [f0[1], -f0[0]];
                    let s = a.signum();
                    let e = [at[0] + r * libm::sin(a.abs()) * f0[0] + s * r * (1.0 - libm::cos(a.abs())) * right[0],
                        at[1] + r * libm::sin(a.abs()) * f0[1] + s * r * (1.0 - libm::cos(a.abs())) * right[1]];
                    (e, 4.0 / 3.0 * libm::tan(a.abs() / 4.0) * r, r * a.abs())
                };
                heading += a;
                let f1 = forward(heading);
                points.push(([at[0] + f0[0] * k, at[1] + f0[1] * k], k, station + length / 3.0));
                points.push(([end[0] - f1[0] * k, end[1] - f1[1] * k], k, station + length * 2.0 / 3.0));
                station += length;
                points.push((end, 0.0, station));
                at = end;
            }
        }
        // Eased rise between the leads; anchors and handles keep the tangent level at both ends.
        let (a, b) = (points[3].2, points[points.len() - 4].2);
        let span = (b - a).max(1.0);
        let rise = if kind == 4 { let h = (100 + 50 * draw(rng, 5)) as f64 * if uniform(rng) < 0.5 { -1.0 } else { 1.0 }; h.clamp(-0.08 * span, 0.08 * span) } else { 0.0 };
        let height = |s: f64| { let u = ((s - a) / span).clamp(0.0, 1.0); (rise * u * u * (3.0 - 2.0 * u), rise * 6.0 * u * (1.0 - u) / span) };
        let mut cp = vec![];
        for (i, (p, k, s)) in points.iter().enumerate() {
            let y = match i % 3 {
                0 => height(*s).0,
                1 => { let (y, slope) = height(points[i - 1].2); y + slope * k }
                _ => { let (y, slope) = height(points[i + 1].2); y - slope * k }
            };
            cp.push([round(p[0]), round(y), round(p[1])]);
        }
        if pipe_control_issue(if halfpipe { "banked_chicane" } else { "cylinder" }, bore, &cp).is_none() { return cp; }
    }
    vec![]
}
fn side(base: &str, d: i8) -> String { if d < 0 { format!("{base}_left") } else { base.into() } }
/// Extra/ultra wide presets keep the base geometry under their own identity.
fn wide(id: String, w: u32) -> String {
    let table: &[(&str, &str)] = if w == 1800 { &ULTRA_WIDE_PRESETS } else if w == 1200 { &EXTRA_WIDE_PRESETS } else { &[] };
    table.iter().find(|(_, base)| *base == id).map_or(id, |(wide, _)| (*wide).into())
}
fn pick<T: Copy>(rng: &mut u64, items: &[T]) -> T { items[draw(rng, items.len() as u64) as usize] }
fn chance(rng: &mut u64, p: f64) -> bool { uniform(rng) < p }
/// Independent per-motif direction; past ±540° of accumulated yaw it leans back.
fn direction(rng: &mut u64, c: &Context) -> i8 {
    let right = if c.yaw > 540.0 { 0.15 } else if c.yaw < -540.0 { 0.85 } else if c.yaw > 270.0 { 0.35 } else if c.yaw < -270.0 { 0.65 } else { 0.5 };
    if chance(rng, right) { 1 } else { -1 }
}
fn width(rng: &mut u64, style: &Style, c: &Context, allowed: &[u32]) -> u32 {
    let options: Vec<_> = STYLE_WIDTHS.iter().enumerate().filter(|(_, w)| allowed.contains(w) && (c.driving || **w <= 800)).collect();
    let total: f64 = options.iter().map(|(i, _)| style.widths[*i]).sum();
    let mut ticket = uniform(rng) * total;
    for (i, w) in &options {
        if ticket < style.widths[*i] { return **w; }
        ticket -= style.widths[*i];
    }
    *options.last().unwrap().1
}
const ROAD: [u32; 3] = [600, 800, 1200];
const ULTRA: [u32; 4] = [600, 800, 1200, 1800];
const NARROW: [u32; 1] = [600];
fn boost(rng: &mut u64, w: u32) -> Step { step(pick(rng, &["acceleration_panel", "boost_chain"]), w) }
fn vertical(rng: &mut u64, c: &Context) -> bool {
    if c.height > 1600 { false } else if c.height < -400 { true } else { chance(rng, 0.5) }
}
/// Level straight long enough that the pieces on both sides keep their full
/// clearance (more than the width plus both wall/clearance margins).
fn link(w: u32) -> Step { step(if w <= 600 { "straight" } else { "sprint_lane" }, w) }
/// A concrete sequence for one family, or None when its preconditions fail.
/// Sharp corners and 8 m grades are shorter than the clearance between their
/// neighbours, so they carry a level link or stay at 600 cm.
pub(super) fn build(family: Family, rng: &mut u64, style: &Style, c: &Context) -> Option<Motif> {
    let basic = !c.driving;
    if basic && family.category() == "driving" && !family.basic() { return None; }
    let d = direction(rng, c);
    let mut reserved = false;
    let steps: Vec<Step> = match family {
        Family::Sweep => {
            let kind = if basic { 0 } else { draw(rng, 5) };
            let w = width(rng, style, c, if kind == 2 { &ULTRA } else { &ROAD });
            match kind {
                0 => vec![step(side("gentle45", d), w)],
                1 => vec![step(side("gentle45", d), w), step(side("gentle45", d), w)],
                2 => vec![step(wide(side("gentle90", d), w), w)],
                3 => vec![step(side("curve", d), w)],
                _ => vec![step(format!("{}{}", side("curve", d), if vertical(rng, c) { "_up" } else { "_down" }), w)],
            }
        }
        Family::Corner => {
            let kind = if basic { 0 } else { draw(rng, 4) };
            let w = width(rng, style, c, &ROAD);
            match kind {
                0 => vec![step(wide(side("right90", d), w), w), link(w)],
                1 => vec![step(side("sharp_curve", d), w), link(w)],
                2 => vec![step(side("sharp135", d), w), link(w)],
                _ => vec![step(side("sharp_curve", d), w), link(w), step(side("sharp_curve", d), w)],
            }
        }
        Family::Hairpins => {
            let k = 2 + draw(rng, 3) as usize;
            let w = width(rng, style, c, &ROAD);
            let banked = w >= 1200;
            let mut out = vec![];
            for i in 0..k {
                let turn = if i % 2 == 0 { d } else { -d };
                out.push(step(side(if banked { "banked_hairpin" } else { "hairpin" }, turn), w));
                if i + 1 < k && !banked {
                    out.push(if w == 600 && chance(rng, 0.3) { step(if vertical(rng, c) { "slope_up" } else { "slope_down" }, w) } else { link(w) });
                }
            }
            if c.action && chance(rng, 0.45) { out.push(boost(rng, w)); }
            out
        }
        Family::SBend => {
            let kind = if basic { draw(rng, 2) } else { draw(rng, 5) };
            let w = width(rng, style, c, &ROAD);
            match kind {
                0 => vec![step(side("gentle45", d), w), step(side("gentle45", -d), w)],
                1 => vec![step(wide(side("right90", d), w), w), link(w), step(wide(side("right90", -d), w), w)],
                2 => vec![step(side("curve", d), w), step(side("curve", -d), w)],
                3 => vec![step(side("spatial_s", d), w)],
                _ => vec![step(side("sharp_curve", d), w), link(w), step(side("sharp_curve", -d), w)],
            }
        }
        Family::Esses => {
            let n = 3 + draw(rng, 2) as usize;
            let w = width(rng, style, c, &ROAD);
            let base = pick(rng, &["gentle45", "curve", "sharp_curve"]);
            let mut out = vec![];
            for i in 0..n {
                if i > 0 && base == "sharp_curve" { out.push(link(w)); }
                out.push(step(side(base, if i % 2 == 0 { d } else { -d }), w));
            }
            out
        }
        Family::Chicane => {
            let w = width(rng, style, c, &ROAD);
            match draw(rng, 3) {
                0 => vec![step("chicane", w)],
                1 => vec![step("zigzag", w)],
                _ => vec![link(w), step(side("sharp_curve", d), w), step(side("sharp_curve", -d), w), link(w)],
            }
        }
        Family::Spiral => {
            let w = width(rng, style, c, &ROAD);
            let degrees = if basic { 90 } else { pick(rng, &[90, 180, 180, 360, 360]) };
            vec![step(format!("spiral{degrees}_{}_{}", if d < 0 { "left" } else { "right" }, if vertical(rng, c) { "up" } else { "down" }), w)]
        }
        Family::RiseFall => {
            let up = vertical(rng, c);
            let (rise, fall) = if up { ("slope_up", "slope_down") } else { ("slope_down", "slope_up") };
            let kind = if basic { draw(rng, 2) } else { draw(rng, 6) };
            let w = width(rng, style, c, if [3, 4].contains(&kind) { &ROAD } else { &NARROW });
            match kind {
                0 => vec![step(rise, w), step(rise, w)],
                1 => vec![step(rise, w), step(fall, w)],
                2 => vec![step("slope", w)],
                3 => vec![step("roller_waves", w)],
                4 => vec![step(format!("{}_up", side("curve", d)), w), step(format!("{}_down", side("curve", -d)), w)],
                _ => vec![step(rise, w), step("straight", w), step(fall, w)],
            }
        }
        Family::Straight => {
            let w = width(rng, style, c, &ULTRA);
            match w {
                1800 => vec![step("straight_ultra_wide", w); 3],
                1200 if chance(rng, 0.5) => vec![step("straight_extra_wide", w); 2],
                600 if chance(rng, 0.5) => vec![step("straight", w)],
                _ => vec![step("sprint_lane", w)],
            }
        }
        Family::Pipe => {
            let bore = pick(rng, &[400, 600]);
            // A seeded control-point tube, or now and then a helix preset.
            let (id, cp) = if draw(rng, 8) == 0 {
                (format!("cylinder_spiral_{}_{}", if d < 0 { "left" } else { "right" }, if vertical(rng, c) { "up" } else { "down" }), vec![])
            } else {
                let cp = pipe_shape(rng, bore, d, false);
                (if cp.is_empty() { side(&format!("{}_curve", if bore == 600 { "cylinder_wide" } else { "cylinder" }), d) } else { "cylinder".into() }, cp)
            };
            let mut out = vec![step("tube_entry", bore), shaped(&id, bore, cp), Step { exit: Some(bore * 2), ..step("tube_exit", bore) }];
            if c.action && chance(rng, 0.4) { out.push(boost(rng, bore * 2)); }
            out
        }
        Family::Halfpipe | Family::Loop | Family::Overpass => {
            let bore = if family == Family::Halfpipe { pick(rng, &[400, 600]) } else { 400 };
            let mut out = vec![];
            if c.lateral != bore / 2 { out.push(Step { exit: Some(bore), ..step("straight", 600) }); }
            match family {
                Family::Halfpipe => {
                    let cp = pipe_shape(rng, bore, d, true);
                    let bore = if cp.is_empty() { 400 } else { bore };
                    if cp.is_empty() && c.lateral != 200 { out = vec![Step { exit: Some(400), ..step("straight", 600) }]; } else if cp.is_empty() { out.clear(); }
                    out.extend([step("tube_entry", bore), shaped("banked_chicane", bore, cp), step("tube_exit", bore)]);
                    if c.action && chance(rng, 0.4) { out.push(boost(rng, 600)); }
                }
                Family::Loop => out.push(step("loop", 400)),
                _ => out.push(step("overpass", 400)),
            }
            out
        }
        Family::Jump => {
            reserved = true;
            let w = width(rng, style, c, &ROAD);
            vec![step("sprint_lane", w), step(pick(rng, &["jump", "offset_jump", "air_ring"]), w), step("sprint_lane", w)]
        }
        Family::BoostRise => {
            if c.height > 2400 { return None; }
            let w = width(rng, style, c, &ROAD);
            let rise = match draw(rng, 3) {
                0 if w == 600 => "slope_up".to_string(),
                1 => format!("spiral90_{}_up", if d < 0 { "left" } else { "right" }),
                _ if basic => format!("spiral90_{}_up", if d < 0 { "left" } else { "right" }),
                _ => format!("{}_up", side("curve", d)),
            };
            vec![boost(rng, w), step(rise, w)]
        }
        Family::Drop => {
            if !DROP_JUMPS || c.height - c.low < DROP_HEIGHT_CM { return None; }
            reserved = true;
            let w = width(rng, style, c, &ROAD);
            // Level takeoff, a falling parabola y = -D·(z/L)² and a landing that eases level.
            let (length, fall) = (1600 + 200 * draw(rng, 5) as i64, 250 + 50 * draw(rng, 4) as i64);
            // The landing keeps a supported runway: slope 2D/L ≤ 0.4 (normal ≥ 0.92).
            let fall = fall.min(c.height - c.low - 150).min(length / 5);
            let slope = 2.0 * fall as f64 / length as f64;
            let settle = round(slope * 600.0);
            vec![shaped("free_curve", w, vec![[0, 0, 0], [0, 0, 533], [0, 0, 1067], [0, 0, 1600]]),
                shaped("flight_curve", w, vec![[0, 0, 0], [0, 0, length / 3], [0, -fall / 3, length * 2 / 3], [0, -fall, length]]),
                Step { exit: Some(w), ..shaped("free_curve", w, vec![[0, 0, 0], [0, -settle, 600], [0, -settle, 1000], [0, -settle, 1600 + 200 * draw(rng, 3) as i64]]) }]
        }
        Family::JumpShortcut | Family::GrindShortcut => {
            let shortcut = if family == Family::JumpShortcut {
                Shortcut::Jump(c.jump_ticket?)
            } else {
                if !c.grind_ticket { return None; }
                let ids = &corner_shortcuts::IDS;
                let mut id = pick(rng, ids);
                if id.contains("left") != (d < 0) { id = ids.iter().copied().find(|v| *v == id.replace(if d < 0 { "right" } else { "left" }, if d < 0 { "left" } else { "right" })).unwrap_or(id); }
                Shortcut::Grind(id, pick(rng, &[600, 800]))
            };
            let base = shortcut.source(false).ok()?;
            return Some(Motif { family, steps: template_steps(&base), reserved: true, shortcut: Some(shortcut) });
        }
    };
    debug_assert!(steps.iter().all(|s| catalogue_ids().contains(&s.id.as_str()) && supported_widths(&s.id).contains(&s.width)), "{family:?} {steps:?}");
    Some(Motif { family, steps, reserved, shortcut: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_family_builds_catalogue_pieces_in_supported_widths() {
        let style = Style::new(9);
        for f in FAMILIES {
            for seed in 0..400u64 {
                for (driving, action) in [(true, true), (false, false)] {
                    let c = Context { driving, action, lateral: if seed % 2 == 0 { 200 } else { 400 }, yaw: (seed as f64 - 200.0) * 4.0, height: (seed as i64 % 7 - 2) * 900,
                        low: -1600 + (seed as i64 % 5) * 400, jump_ticket: (seed % 3 == 0).then_some(5 + seed as usize % 4), grind_ticket: seed % 2 == 0 };
                    let mut rng = seed;
                    let Some(m) = build(f, &mut rng, &style, &c) else { continue };
                    assert_eq!(m.family, f);
                    for s in &m.steps {
                        assert!(catalogue_ids().contains(&s.id.as_str()), "{f:?} {}", s.id);
                        assert!(supported_widths(&s.id).contains(&s.width), "{f:?} {} {}", s.id, s.width);
                        assert_ne!(s.id, "jump_panel");
                        if !driving && category(&s.id) == "driving" && f.category() == "driving" {
                            assert!(f.basic() && s.width <= 800, "{f:?} {}", s.id);
                        }
                    }
                    for pair in m.steps.windows(2) {
                        assert!(!(["acceleration_panel", "boost_chain"].contains(&pair[0].id.as_str())
                            && ["jump", "offset_jump", "air_ring"].contains(&pair[1].id.as_str())));
                    }
                    if f == Family::Hairpins {
                        let turns: Vec<_> = m.steps.iter().filter(|s| s.id.contains("hairpin")).map(|s| s.id.ends_with("_left")).collect();
                        assert!(turns.len() >= 2 && turns.windows(2).all(|w| w[0] != w[1]));
                    }
                }
            }
        }
        assert_ne!(Style::new(1), Style::new(2));
        assert_eq!(Style::new(3), Style::new(3));
    }
}
