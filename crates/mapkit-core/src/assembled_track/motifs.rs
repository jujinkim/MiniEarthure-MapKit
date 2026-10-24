//! Motif grammar: families, a per-seed style vector and concrete piece sequences.
//! Directions are drawn per motif; repetition and spacing rules live in `grow`.
use super::*;
use seed_streams::{draw, uniform};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Family { Sweep, Corner, Hairpins, SBend, Esses, Chicane, Spiral, RiseFall, Straight, Pipe, Halfpipe, Loop, Overpass, Jump, BoostRise }
pub(super) const FAMILIES: [Family; 15] = [Family::Sweep, Family::Corner, Family::Hairpins, Family::SBend, Family::Esses, Family::Chicane,
    Family::Spiral, Family::RiseFall, Family::Straight, Family::Pipe, Family::Halfpipe, Family::Loop, Family::Overpass, Family::Jump, Family::BoostRise];
/// (category, base weight, usable as a connector without the driving category)
const TABLE: [(&str, f64, bool); 15] = [("driving", 13.0, true), ("driving", 12.0, true), ("driving", 7.0, false), ("driving", 9.0, true),
    ("driving", 6.0, false), ("driving", 5.0, false), ("driving", 6.0, true), ("driving", 8.0, true), ("driving", 9.0, true),
    ("gimmick", 7.0, false), ("gimmick", 3.0, false), ("gimmick", 2.5, false), ("gimmick", 1.5, false), ("action", 6.0, false), ("action", 3.0, false)];
pub(super) const STYLE_WIDTHS: [u32; 4] = [600, 800, 1200, 1800];
const WIDTH_WEIGHTS: [f64; 4] = [35.0, 35.0, 20.0, 10.0];
impl Family {
    pub fn index(self) -> usize { self as usize }
    pub fn category(self) -> &'static str { TABLE[self.index()].0 }
    pub fn base(self) -> f64 { TABLE[self.index()].1 }
    pub fn basic(self) -> bool { TABLE[self.index()].2 }
    pub fn pipe(self) -> bool { matches!(self, Family::Pipe | Family::Halfpipe) }
}
/// Per-seed character: family multipliers ×0.5..1.5 and a width preference.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Style {
    pub families: [f64; 15],
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
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Motif {
    pub family: Family,
    pub steps: Vec<Step>,
    /// Jump approach and landing roads stay free of automatic obstacles.
    pub reserved: bool,
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
}
fn step(id: impl Into<String>, width: u32) -> Step { Step { id: id.into(), width, exit: None } }
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
            let prefix = if bore == 600 { "cylinder_wide" } else { "cylinder" };
            // U-turn tubes put both portal ramps side by side inside their clearance.
            let id = match draw(rng, 4) {
                0 => prefix.to_string(),
                1 => side(&format!("{prefix}_curve"), d),
                2 => format!("{prefix}_s_rise"),
                _ => format!("cylinder_spiral_{}_{}", if d < 0 { "left" } else { "right" }, if vertical(rng, c) { "up" } else { "down" }),
            };
            let mut out = vec![step("tube_entry", bore), step(id, bore), Step { id: "tube_exit".into(), width: bore, exit: Some(bore * 2) }];
            if c.action && chance(rng, 0.4) { out.push(boost(rng, bore * 2)); }
            out
        }
        Family::Halfpipe | Family::Loop | Family::Overpass => {
            let mut out = vec![];
            if c.lateral != 200 { out.push(Step { id: "straight".into(), width: 600, exit: Some(400) }); }
            match family {
                Family::Halfpipe => {
                    out.extend([step("tube_entry", 400), step("banked_chicane", 400), step("tube_exit", 400)]);
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
    };
    debug_assert!(steps.iter().all(|s| catalogue_ids().contains(&s.id.as_str()) && supported_widths(&s.id).contains(&s.width)), "{family:?} {steps:?}");
    Some(Motif { family, steps, reserved })
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
                    let c = Context { driving, action, lateral: if seed % 2 == 0 { 200 } else { 400 }, yaw: (seed as f64 - 200.0) * 4.0, height: (seed as i64 % 7 - 2) * 900 };
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
