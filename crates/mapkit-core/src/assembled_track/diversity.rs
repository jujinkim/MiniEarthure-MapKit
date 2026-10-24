//! Generation-only taxonomy. Repetition families and stored v1 data are independent.
use super::*;
use std::collections::BTreeSet;

pub(super) struct Kind {
    pub id: &'static str,
    pub category: &'static str,
    pub variants: &'static [&'static str],
}
macro_rules! kind {
    ($id:literal, $category:literal, $($variant:literal),+ $(,)?) => {
        Kind { id: $id, category: $category, variants: &[$($variant),+] }
    };
}
// Every generatable registered driving form participates, even when placement
// fails. Templates contribute their expanded forms, never an extra template type.
pub(super) const PIECES: &[Kind] = &[
    kind!(
        "straight",
        "driving",
        "straight",
        "sprint_lane",
        "straight_narrow",
        "straight_extra_wide",
        "straight_ultra_wide"
    ),
    kind!("slope", "driving", "slope", "slope_up", "slope_down"),
    kind!(
        "graded_curve",
        "driving",
        "curve_up",
        "curve_down",
        "curve_left_up",
        "curve_left_down"
    ),
    kind!("gentle45", "driving", "gentle45", "gentle45_left"),
    kind!(
        "gentle90",
        "driving",
        "gentle90",
        "gentle90_left",
        "gentle90_extra_wide",
        "gentle90_extra_wide_left",
        "gentle90_ultra_wide",
        "gentle90_ultra_wide_left"
    ),
    kind!(
        "right90",
        "driving",
        "right90",
        "right90_left",
        "right90_extra_wide",
        "right90_extra_wide_left",
        "right90_ultra_wide",
        "right90_ultra_wide_left"
    ),
    kind!("zigzag", "driving", "zigzag", "zigzag_narrow"),
    kind!("chicane", "driving", "chicane", "chicane_narrow"),
    kind!("sharp135", "driving", "sharp135", "sharp135_left"),
    kind!("hairpin", "driving", "hairpin", "hairpin_left"),
    kind!(
        "spiral90",
        "driving",
        "spiral90_right_up",
        "spiral90_left_up",
        "spiral90_right_down",
        "spiral90_left_down"
    ),
    kind!(
        "spiral180",
        "driving",
        "spiral180_right_up",
        "spiral180_left_up",
        "spiral180_right_down",
        "spiral180_left_down"
    ),
    kind!(
        "spiral360",
        "driving",
        "spiral360_right_up",
        "spiral360_left_up",
        "spiral360_right_down",
        "spiral360_left_down",
        "spiral_up",
        "spiral_down"
    ),
    kind!(
        "banked_hairpin",
        "driving",
        "banked_hairpin",
        "banked_hairpin_left"
    ),
    kind!("spatial_s", "driving", "spatial_s", "spatial_s_left"),
    kind!("curve", "driving", "curve", "curve_left"),
    kind!("sharp_curve", "driving", "sharp_curve", "sharp_curve_left"),
    kind!("roller_waves", "driving", "roller_waves"),
    kind!("cylinder", "gimmick", "cylinder", "cylinder_wide"),
    kind!(
        "cylinder_curve",
        "gimmick",
        "cylinder_curve",
        "cylinder_curve_left",
        "cylinder_wide_curve",
        "cylinder_wide_curve_left"
    ),
    kind!(
        "cylinder_uturn",
        "gimmick",
        "cylinder_uturn",
        "cylinder_uturn_left",
        "cylinder_wide_uturn",
        "cylinder_wide_uturn_left"
    ),
    kind!(
        "cylinder_s_rise",
        "gimmick",
        "cylinder_s_rise",
        "cylinder_wide_s_rise"
    ),
    kind!(
        "cylinder_spiral",
        "gimmick",
        "cylinder_spiral_right_up",
        "cylinder_spiral_left_up",
        "cylinder_spiral_right_down",
        "cylinder_spiral_left_down"
    ),
    kind!("banked_chicane", "gimmick", "banked_chicane"),
    kind!("overpass", "gimmick", "overpass"),
    kind!("loop", "gimmick", "loop"),
    kind!("jump_panel", "action", "jump_panel"),
    kind!("jump", "action", "jump", "double_jump"),
    kind!("offset_jump", "action", "offset_jump"),
    kind!("acceleration_panel", "action", "acceleration_panel"),
    kind!("boost_chain", "action", "boost_chain"),
    kind!("air_ring", "action", "air_ring", "air_ring_chain"),
];
pub(super) const GIMMICKS: &[Kind] = &[
    kind!("ramp_low", "gimmick", "ramp_low"),
    kind!("ramp_standard", "gimmick", "ramp_standard"),
    kind!("ramp_triple", "gimmick", "ramp_triple"),
    kind!(
        "quarterpipe",
        "gimmick",
        "quarterpipe_left",
        "quarterpipe_right"
    ),
    kind!("grind_rail", "gimmick", "grind_rail"),
    kind!("fixed_obstacle", "gimmick", "fixed_obstacle"),
    kind!("moving_obstacle", "gimmick", "moving_obstacle"),
    kind!("rotating_obstacle", "gimmick", "rotating_obstacle"),
    kind!("jump_barrier", "gimmick", "jump_barrier"),
    kind!("slalom_gates", "gimmick", "slalom_gates"),
    kind!("swing_gates", "gimmick", "swing_gates"),
    kind!("piston_gates", "gimmick", "piston_gates"),
    kind!("jump_platforms", "gimmick", "jump_platforms"),
    kind!("jump_panel", "action", "jump_panel"),
    kind!("acceleration_panel", "action", "acceleration_panel"),
    kind!("boost_chain", "action", "boost_chain"),
    kind!("air_ring", "action", "air_ring"),
];
fn enabled(s: &Settings, k: &Kind) -> bool {
    s.categories.iter().any(|c| c == k.category)
}
pub(super) fn piece_kind(p: &Piece) -> Option<&'static str> {
    if p.id == "free_curve" {
        // Expanded large corners have no preset identity. Classify their actual
        // path; straight preparation/recovery and short closure seams add no type.
        if race_length(p) <= 1600 {
            return None;
        }
        let turn: f64 = p
            .path
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0].forward, w[1].forward);
                (a[0] as f64 * b[2] as f64 - a[2] as f64 * b[0] as f64)
                    .atan2(a[0] as f64 * b[0] as f64 + a[2] as f64 * b[2] as f64)
                    .abs()
            })
            .sum();
        if turn < 0.2 {
            return None;
        }
        let a = p.path.first()?;
        let b = p.path.last()?;
        return Some(
            if turn > 5.0 && a.position_cm[1].abs_diff(b.position_cm[1]) > 200 {
                "spiral360"
            } else if a.forward[0] * b.forward[0] + a.forward[2] * b.forward[2] < -700_000_000_000 {
                "hairpin"
            } else {
                "spatial_s"
            },
        );
    }
    PIECES
        .iter()
        .find(|k| k.variants.contains(&p.id.as_str()))
        .map(|k| k.id)
}
pub(super) fn obstacle_kind(id: &str) -> Option<&'static str> {
    GIMMICKS
        .iter()
        .find(|k| k.variants.contains(&id))
        .map(|k| k.id)
}
pub(super) fn placed<'a>(pieces: impl IntoIterator<Item = &'a Piece>) -> BTreeSet<&'static str> {
    pieces.into_iter().filter_map(piece_kind).collect()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TypeSummary {
    pub used: usize,
    pub total: usize,
    pub target: usize,
    pub used_types: Vec<String>,
    pub missing_types: Vec<String>,
}
impl TypeSummary {
    fn new(registry: &[Kind], s: &Settings, seen: &BTreeSet<&str>) -> Self {
        let mut used_types = vec![];
        let mut missing_types = vec![];
        for k in registry.iter().filter(|k| enabled(s, k)) {
            if seen.contains(k.id) {
                used_types.push(k.id.into());
            } else {
                missing_types.push(k.id.into());
            }
        }
        let total = used_types.len() + missing_types.len();
        Self {
            used: used_types.len(),
            total,
            target: total.div_ceil(2),
            used_types,
            missing_types,
        }
    }
    fn deficit(&self) -> u64 {
        self.target.saturating_sub(self.used) as u64 * 1_000_000 / self.target.max(1) as u64
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GenerationSummary {
    pub pieces: TypeSummary,
    pub gimmicks: TypeSummary,
    pub narrow_road: super::narrow::NarrowSummary,
}
impl GenerationSummary {
    pub(super) fn deficit(&self) -> (u64, u64) {
        let a = self.pieces.deficit();
        let b = self.gimmicks.deficit();
        (a.max(b), a + b)
    }
    pub fn targets_met(&self) -> bool {
        self.deficit() == (0, 0)
    }
}
/// Only generated response metadata; never a field in Assembly or a package.
pub fn generation_summary(a: &Assembly) -> GenerationSummary {
    let indices: BTreeSet<_> = a
        .routes
        .iter()
        .flat_map(|r| r.pieces.iter().copied())
        .collect();
    let source = a.authoring.as_ref().or(a.seed_source.as_ref());
    let mut pieces = placed(
        indices
            .iter()
            .filter(|&&i| {
                !source
                    .and_then(|s| s.instances.get(i))
                    .is_some_and(|p| p.id.ends_with("rail-entry") || p.id.ends_with("rail-exit"))
            })
            .map(|&i| &a.pieces[i]),
    );
    let mut gimmicks: BTreeSet<_> = a
        .obstacles
        .iter()
        .filter(|o| indices.contains(&o.piece_index))
        .filter_map(|o| obstacle_kind(&o.kind))
        .collect();
    for &i in &indices {
        let id = a.pieces[i].id.as_str();
        if let Some(k) = obstacle_kind(id) {
            gimmicks.insert(k);
        }
        if ["jump", "offset_jump", "air_ring"].contains(&id) {
            gimmicks.insert("jump_panel");
        }
    }
    if let Some(s) = source {
        for action in &s.actions {
            if s.instances
                .iter()
                .position(|i| i.id == action.piece)
                .is_some_and(|i| indices.contains(&i))
            {
                if let Some(k) = obstacle_kind(&action.kind) {
                    gimmicks.insert(k);
                }
                if action.kind == "air_ring" {
                    pieces.insert("air_ring");
                }
            }
        }
        if !s.structures.is_empty()
            && s.grind_lines.iter().any(|line| {
                s.instances.iter().enumerate().any(|(i, p)| {
                    indices.contains(&i)
                        && p.preset == "flight_curve"
                        && p.control_points.len() == line.control_points.len()
                        && p.control_points
                            .iter()
                            .zip(&line.control_points)
                            .all(|(p, l)| p[0] == l[0] && p[2] == l[2] && l[1] - p[1] == 40)
                })
            })
        {
            gimmicks.insert("grind_rail");
        }
        if s.jump_shortcuts.iter().any(|j| {
            s.instances
                .iter()
                .position(|i| i.id == j.flight_piece)
                .is_some_and(|i| indices.contains(&i))
        }) {
            gimmicks.insert("jump_platforms");
        }
    }
    GenerationSummary {
        pieces: TypeSummary::new(PIECES, &a.settings, &pieces),
        gimmicks: TypeSummary::new(GIMMICKS, &a.settings, &gimmicks),
        narrow_road: super::narrow::measure(a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(ids: &[&str]) -> Assembly {
        finish::assembly(
            ids.iter()
                .map(|id| materialize(&catalogue_piece(id)))
                .collect(),
            &Settings::default(),
        )
    }
    #[test]
    fn variants_share_types_but_distinct_driving_forms_do_not() {
        for id in catalogue_ids().iter().filter(|id| {
            ![
                "tube_entry",
                "tube_exit",
                "approach",
                "finish_plaza",
                "free_curve",
                "flight_curve",
            ]
            .contains(id)
        }) {
            assert!(
                piece_kind(&catalogue_piece(id)).is_some(),
                "registered form {id}"
            );
        }
        for ids in [
            ["curve", "curve_left"],
            ["straight", "straight_ultra_wide"],
            ["cylinder_curve", "cylinder_wide_curve_left"],
            ["cylinder_spiral_left_up", "cylinder_spiral_right_down"],
            ["curve_left_up", "curve_down"],
        ] {
            assert_eq!(generation_summary(&fixture(&ids)).pieces.used, 1, "{ids:?}");
        }
        let a = fixture(&[
            "cylinder",
            "cylinder_curve",
            "cylinder_uturn",
            "cylinder_s_rise",
            "cylinder_spiral_right_up",
            "jump_panel",
            "jump",
            "offset_jump",
        ]);
        let s = generation_summary(&a);
        assert_eq!(s.pieces.used, 8);
        let excluded = generation_summary(&fixture(&[
            "tube_entry",
            "tube_exit",
            "approach",
            "finish_plaza",
        ]));
        assert_eq!(excluded.pieces.used, 0);
        assert_eq!(s.pieces.total, PIECES.len());
        assert_eq!(s.gimmicks.total, GIMMICKS.len());
        assert_eq!(
            obstacle_kind("quarterpipe_left"),
            obstacle_kind("quarterpipe_right")
        );
    }
    #[test]
    fn expanded_templates_and_retained_routes_supply_only_actual_types() {
        let a = authoring::compile(&templates::source("double_jump").unwrap()).unwrap();
        assert_eq!(generation_summary(&a).pieces.used_types, vec!["jump"]);
        let a = authoring::compile(&templates::source("air_ring_chain").unwrap()).unwrap();
        let summary = generation_summary(&a);
        assert!(summary.pieces.used_types.contains(&"jump".into()));
        assert!(summary.pieces.used_types.contains(&"air_ring".into()));
        assert_eq!(summary.gimmicks.used, 2);
        let source = corner_shortcuts::source("grind_s_right", 800, true).unwrap();
        let with = authoring::compile(&source).unwrap();
        assert!(generation_summary(&with)
            .gimmicks
            .used_types
            .contains(&"grind_rail".into()));
        let without =
            authoring::compile(&corner_shortcuts::source("grind_s_right", 800, false).unwrap())
                .unwrap();
        assert!(!generation_summary(&without)
            .gimmicks
            .used_types
            .contains(&"grind_rail".into()));
        let special = |a: &Assembly| a.routes[0].pieces.iter().map(|&i| &a.pieces[i]).filter(|p| category(&p.id) != "driving").map(race_length).sum::<u64>();
        assert_eq!(special(&with), special(&without));
    }
    #[test]
    fn complete_and_constrained_fixtures_keep_the_same_denominators() {
        let a = fixture(&PIECES.iter().map(|k| k.variants[0]).collect::<Vec<_>>());
        let s = generation_summary(&a);
        assert!(s.pieces.used >= s.pieces.target);
        let seen = GIMMICKS.iter().map(|k| k.id).collect();
        let complete = TypeSummary::new(GIMMICKS, &a.settings, &seen);
        assert_eq!(complete.used, complete.total);
        let s = generation_summary(&fixture(&["straight"]));
        assert!(!s.targets_met());
        assert_eq!(s.pieces.missing_types.len() + s.pieces.used, s.pieces.total);
        assert_eq!(s.pieces.target, s.pieces.total.div_ceil(2));
        let mut driving = fixture(&["straight"]);
        driving.settings.categories = vec!["driving".into()];
        assert_eq!(generation_summary(&driving).gimmicks.total, 0);
        assert!(!serde_json::to_value(driving)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("generation_summary"));
    }
}
