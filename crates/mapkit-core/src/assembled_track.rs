//! Seeded, bounded track assembly. This is the sole catalogue and geometry owner.
//! Coordinates are centimetres; normals/directions use signed millionths.
use crate::gimmick::{Effect, Gimmick, Motion, MotionKind};
use crate::special_track::{SpecialTrack, TrackKind, TubeFrame};
use crate::*;

pub const WIDTH: i64 = 400;
pub const WALL: i64 = 60;
pub const TILE_CM: i64 = 800;
pub const START_PIECES: usize = 3;
pub const FINISH_ENTRY_CM: i64 = 800;
pub const FINISH_RADIUS_CM: i64 = 800;
pub const FINISH_WALL_CM: i64 = 120;
const SLOT: i64 = TILE_CM * 4;
const LOOP_RADIUS: u32 = 350;
const LOOP_WIDTH: u32 = 220;
const LOOP_OFFSET: i64 = 143;
const SPEED: i64 = 900;
const MAX_PIECES: usize = 512;
const MAX_SAMPLES: usize = 32_000;
pub mod authoring;
mod geometry;
mod junction;
mod layout;
pub const WIDTHS: &[u32] = &[200, 400, 600, 800, 1200];
mod grounding;
pub use grounding::Support;
mod obstacles;
pub use obstacles::Obstacle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub seed: u64,
    pub circuit: bool,
    pub duration_seconds: u16,
    pub difficulty: String,
    pub categories: Vec<String>,
    pub time_minutes: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: 1,
            circuit: true,
            duration_seconds: 60,
            difficulty: "normal".into(),
            categories: selection_ids().iter().map(|v| (*v).into()).collect(),
            time_minutes: 720,
        }
    }
}
impl Settings {
    pub fn normalized(&self) -> Result<Self> {
        if self.seed > 9_007_199_254_740_991
            || !duration_options(self.circuit)
                .iter()
                .any(|v| v.0 == self.duration_seconds)
            || !["easy", "normal", "hard"].contains(&self.difficulty.as_str())
            || self.time_minutes >= 1440
            || self.categories.is_empty()
            || self.categories.len() > selection_ids().len()
            || self
                .categories
                .iter()
                .any(|s| !selection_ids().contains(&s.as_str()))
        {
            return Err(error(
                "E_TRACK_SETTINGS",
                "invalid seed, duration, difficulty, time or piece selection",
            ));
        }
        let mut s = self.clone();
        s.categories.sort();
        s.categories.dedup();
        Ok(s)
    }
    pub fn max_laps(&self) -> u8 {
        duration_options(self.circuit)
            .iter()
            .find(|v| v.0 == self.duration_seconds)
            .map_or(1, |v| v.1)
    }
}
pub fn duration_options(circuit: bool) -> &'static [(u16, u8)] {
    if circuit {
        &[(60, 3), (90, 2), (120, 2)]
    } else {
        &[(60, 1), (90, 1), (120, 1)]
    }
}
fn basic_ids() -> &'static [&'static str] {
    &[
        "finish_plaza",
        "tube_entry",
        "tube_exit",
        "approach",
        "straight",
        "curve",
        "curve_left",
        "sharp_curve",
        "sharp_curve_left",
        "hairpin",
        "hairpin_left",
        "slope",
        "slope_up",
        "slope_down",
        "curve_up",
        "curve_down",
        "curve_left_up",
        "curve_left_down",
        "zigzag",
        "chicane",
        "straight_narrow",
        "chicane_narrow",
        "zigzag_narrow",
    ]
}
fn short_piece(id: &str) -> bool {
    ["acceleration_panel", "boost_chain"].contains(&id)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub position_cm: Vertex,
    pub normal: Vertex,
    pub forward: Vertex,
    pub mode: String,
    pub safe: bool,
    pub min_speed_cmps: u32,
    /// Half-width and normal clearance of the allowed drive/flight corridor.
    pub lateral_cm: u32,
    pub tube_radius_cm: u32,
    pub below_cm: u32,
    pub above_cm: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    pub rotation_mdeg: [i32; 3],
    pub control_points: Vec<Vertex>,
    pub id: String,
    pub width_cm: u32,
    pub entry_width_cm: u32,
    pub exit_width_cm: u32,
    pub chain_id: u32,
    pub chain_index: u32,
    pub chain_count: u32,
    pub ordinary: bool,
    pub origin_cm: Vertex,
    pub cube_span: u8,
    pub entry_speed_cmps: [u32; 2],
    pub connection_width_cm: u32,
    pub quarter_turns: u8,
    pub path: Vec<Sample>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_path: Vec<Sample>,
    pub reserved_min_cm: Vertex,
    pub reserved_max_cm: Vertex,
    pub reference_msec: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Assembly {
    pub authoring: Option<authoring::Source>,
    pub seed_source: Option<authoring::Source>,
    pub routes: Vec<authoring::Route>,
    pub issues: Vec<String>,
    pub settings: Settings,
    pub generator_fingerprint: String,
    pub catalogue_fingerprint: String,
    pub pieces: Vec<Piece>,
    pub supports: Vec<Support>,
    pub obstacles: Vec<Obstacle>,
    pub obstacle_eligible_length_cm: u64,
    pub obstacle_target_count: u32,
    pub length_cm: u64,
    pub estimated_msec: u32,
    pub ordinary_length_cm: u64,
    pub ordinary_straight_cm: u64,
    pub floor: VenueFloor,
    pub finish_plaza: Option<FinishPlaza>,
}

/// The fixed flat runway has separate support identities for each piece.
/// Resolve a grid anchor without generating a whole collision cell. Callers
/// must still verify the full footprint, wheel support and occupied volume.
pub fn start_surface_at(assembly: &Assembly, point: [i64; 2]) -> Option<String> {
    let route = assembly.routes.first()?;
    route
        .pieces
        .iter()
        .take(
            authoring::common_checkpoints(assembly)
                .first()
                .and_then(|(index, _)| route.pieces.iter().position(|i| i == index))
                .map_or(START_PIECES, |i| i + 1),
        )
        .find_map(|&i| {
            let p = &assembly.pieces[i];
            p.path
                .windows(2)
                .any(|w| {
                    let a = w[0].position_cm;
                    let b = w[1].position_cm;
                    let (dx, dz) = ((b[0] - a[0]) as f64, (b[2] - a[2]) as f64);
                    let length = dx * dx + dz * dz;
                    if length < 1.0 {
                        return false;
                    }
                    let t =
                        ((point[0] - a[0]) as f64 * dx + (point[1] - a[2]) as f64 * dz) / length;
                    let side = ((point[0] - a[0]) as f64 * dz - (point[1] - a[2]) as f64 * dx)
                        .abs()
                        / libm::sqrt(length);
                    (0.0..=1.0).contains(&t)
                        && side <= f64::from(w[0].lateral_cm.min(w[1].lateral_cm))
                        && w.iter().all(|s| s.safe && s.normal[1] > 990000)
                })
                .then(|| format!("assembled-road-{i}"))
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FinishPlaza {
    pub normal: Vertex,
    pub center_cm: Vertex,
    pub recovery_cm: Vertex,
    pub forward: Vertex,
    pub checkpoint_cm: Vertex,
    pub radius_cm: i64,
    pub entry_length_cm: i64,
    pub wall_height_cm: i64,
    pub piece_index: usize,
}
fn plaza_center(p: &Piece) -> Vertex {
    let half = i64::from(p.width_cm) / 2;
    add(
        p.origin_cm,
        geometry::rotate3(
            [
                0,
                0,
                FINISH_ENTRY_CM + round(libm::sqrt((FINISH_RADIUS_CM.pow(2) - half.pow(2)) as f64)),
            ],
            p.rotation_mdeg,
        ),
    )
}
fn finish_plaza(pieces: &[Piece]) -> Option<FinishPlaza> {
    let p = pieces.last()?;
    (p.id == "finish_plaza").then(|| FinishPlaza {
        normal: geometry::rotate3([0, 1_000_000, 0], p.rotation_mdeg),
        center_cm: plaza_center(p),
        recovery_cm: plaza_center(p),
        forward: geometry::rotate3([0, 0, 1_000_000], p.rotation_mdeg),
        checkpoint_cm: add(
            p.origin_cm,
            geometry::rotate3([0, 0, FINISH_ENTRY_CM / 2], p.rotation_mdeg),
        ),
        radius_cm: FINISH_RADIUS_CM,
        entry_length_cm: FINISH_ENTRY_CM,
        wall_height_cm: FINISH_WALL_CM,
        piece_index: pieces.len() - 1,
    })
}
fn race_length(p: &Piece) -> u64 {
    if p.id == "finish_plaza" {
        (FINISH_ENTRY_CM / 2) as u64
    } else {
        p.path
            .windows(2)
            .map(|w| distance(w[0].position_cm, w[1].position_cm))
            .sum()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VenueFloor {
    pub min_cm: Vertex,
    pub max_cm: Vertex,
}
pub fn catalogue_ids() -> &'static [&'static str] {
    &[
        "gentle45",
        "gentle45_left",
        "gentle90",
        "gentle90_left",
        "right90",
        "right90_left",
        "sharp135",
        "sharp135_left",
        "jump_panel",
        "free_curve",
        "flight_curve",
        "spiral90_right_up",
        "spiral90_right_down",
        "spiral90_left_up",
        "spiral90_left_down",
        "spiral180_right_up",
        "spiral180_right_down",
        "spiral180_left_up",
        "spiral180_left_down",
        "spiral360_right_up",
        "spiral360_right_down",
        "spiral360_left_up",
        "spiral360_left_down",
        "finish_plaza",
        "tube_entry",
        "tube_exit",
        "approach",
        "straight",
        "curve",
        "curve_left",
        "sharp_curve",
        "sharp_curve_left",
        "hairpin",
        "hairpin_left",
        "slope",
        "slope_up",
        "slope_down",
        "curve_up",
        "curve_down",
        "curve_left_up",
        "curve_left_down",
        "zigzag",
        "chicane",
        "straight_narrow",
        "chicane_narrow",
        "zigzag_narrow",
        "cylinder",
        "cylinder_curve",
        "cylinder_curve_left",
        "cylinder_uturn",
        "cylinder_uturn_left",
        "cylinder_s_rise",
        "cylinder_wide",
        "cylinder_wide_curve",
        "cylinder_wide_curve_left",
        "cylinder_wide_uturn",
        "cylinder_wide_uturn_left",
        "cylinder_wide_s_rise",
        "banked_chicane",
        "overpass",
        "roller_waves",
        "offset_jump",
        "sprint_lane",
        "loop",
        "spiral_up",
        "spiral_down",
        "jump",
        "acceleration_panel",
        "boost_chain",
        "air_ring",
    ]
}
/// Public choices are deliberately separate from resolved road presets.
pub fn selection_ids() -> &'static [&'static str] {
    &["driving", "gimmick", "action"]
}
pub fn category(id: &str) -> &'static str {
    if [
        "jump",
        "offset_jump",
        "jump_panel",
        "acceleration_panel",
        "boost_chain",
        "air_ring",
    ]
    .contains(&id)
    {
        "action"
    } else if pipe_piece(id) || ["loop", "overpass", "obstacles"].contains(&id) {
        "gimmick"
    } else {
        "driving"
    }
}
pub fn fingerprint() -> String {
    sha256(
        &[
            include_bytes!("assembled_track.rs").as_slice(),
            include_bytes!("special_track.rs").as_slice(),
            include_bytes!("assembled_track/layout.rs").as_slice(),
            include_bytes!("assembled_track/geometry.rs").as_slice(),
            include_bytes!("assembled_track/junction.rs").as_slice(),
            include_bytes!("assembled_track/authoring.rs").as_slice(),
            include_bytes!("assembled_track/obstacles.rs").as_slice(),
            include_bytes!("assembled_track/grounding.rs").as_slice(),
        ]
        .concat(),
    )
}
pub fn runtime_metadata(a: &Assembly) -> serde_json::Value {
    let mut value = serde_json::to_value(a).unwrap();
    value["grind_lines"] = serde_json::to_value(obstacles::grind_lines(a).iter().map(|l|l.resolved_json()).collect::<Vec<_>>()).unwrap();
    value["progress_checkpoints"]=serde_json::json!(authoring::common_checkpoints(a).into_iter().map(|(piece_index,sample_index)|serde_json::json!({"piece_index":piece_index,"sample_index":sample_index})).collect::<Vec<_>>());
    value
}
pub fn catalogue() -> serde_json::Value {
    serde_json::json!({"format_version":1,"width_cm":WIDTH,"wall_height_cm":WALL,
        "tile_size_cm":TILE_CM,"defaults":Settings::default(),"generator_fingerprint":fingerprint(),
        "selection_ids":selection_ids(),"obstacle_kinds":obstacles::KINDS,"basic_piece_ids":basic_ids(),"widths_cm":WIDTHS,"reference_speed_cmps":SPEED,
        "entries":catalogue_ids().iter().map(|id| serde_json::json!({"id":id,"category":category(id),"widths_cm":supported_widths(id),"ports":["entry","exit"]})).collect::<Vec<_>>(),
        "selection_groups":{"cylinder":["cylinder","cylinder_curve","cylinder_curve_left","cylinder_uturn","cylinder_uturn_left","cylinder_s_rise","cylinder_wide","cylinder_wide_curve","cylinder_wide_curve_left","cylinder_wide_uturn","cylinder_wide_uturn_left","cylinder_wide_s_rise"]},
        "duration_options":{"circuit":duration_options(true).iter().map(|v| serde_json::json!({"seconds":v.0,"max_laps":v.1})).collect::<Vec<_>>(),
            "sprint":duration_options(false).iter().map(|v| serde_json::json!({"seconds":v.0,"max_laps":v.1})).collect::<Vec<_>>()},
        "catalogue_fingerprint":catalogue_fingerprint(),
        "pieces":catalogue_ids().iter().map(|id| variant(id, catalogue_width(id), 400, 400)).collect::<Vec<_>>()})
}
pub fn catalogue_fingerprint() -> String {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        sha256(
            &canonical(
                &catalogue_ids()
                    .iter()
                    .map(|id| variant(id, catalogue_width(id), 400, 400))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        )
    })
    .clone()
}
pub fn supported_widths(id: &str) -> &'static [u32] {
    if ["loop", "banked_chicane", "overpass", "finish_plaza"].contains(&id) {
        &[400]
    } else if id.starts_with("cylinder") {
        &[200, 400, 600]
    } else {
        WIDTHS
    }
}
fn catalogue_width(id: &str) -> u32 {
    if id.ends_with("_narrow") || (id.starts_with("cylinder") && !id.starts_with("cylinder_wide")) {
        200
    } else {
        400
    }
}
fn round(v: f64) -> i64 {
    libm::round(v) as i64
}
fn unit(v: [f64; 3]) -> Vertex {
    let n = libm::sqrt(v.iter().map(|x| x * x).sum::<f64>()).max(1e-9);
    v.map(|x| round(x / n * 1_000_000.0))
}
fn rotate(mut p: Vertex, q: u8) -> Vertex {
    for _ in 0..q {
        p = [p[2], p[1], -p[0]];
    }
    p
}
fn add(a: Vertex, b: Vertex) -> Vertex {
    std::array::from_fn(|i| a[i] + b[i])
}
fn distance(a: Vertex, b: Vertex) -> u64 {
    round(libm::sqrt(
        (0..3).map(|i| ((a[i] - b[i]) as f64).powi(2)).sum(),
    )) as u64
}
fn line(points: &mut Vec<(Vertex, Vertex, String)>, a: Vertex, b: Vertex, mode: &str) {
    let n = (distance(a, b) / if mode == "cylinder" { 50 } else { 150 }).max(1) as i64;
    for i in 0..=n {
        if i == 0 && !points.is_empty() {
            continue;
        }
        points.push((
            std::array::from_fn(|j| a[j] + (b[j] - a[j]) * i / n),
            [0, 1_000_000, 0],
            mode.into(),
        ));
    }
}
fn connector(points: &mut Vec<(Vertex, Vertex, String)>, a: Vertex, b: Vertex, mode: &str) {
    let n = (distance(a, b) / 50).max(1);
    for i in 0..=n {
        if i == 0 && !points.is_empty() {
            continue;
        }
        let t = i as f64 / n as f64;
        let blend = t * t * (3.0 - 2.0 * t);
        points.push((
            [
                a[0] + round((b[0] - a[0]) as f64 * blend),
                a[1] + round((b[1] - a[1]) as f64 * blend),
                a[2] + round((b[2] - a[2]) as f64 * t),
            ],
            [0, 1_000_000, 0],
            mode.into(),
        ));
    }
}
/// Straight approach, circular bend, straight departure. The end tangent is exact.
fn bend(
    points: &mut Vec<(Vertex, Vertex, String)>,
    radius: i64,
    lead: i64,
    quarters: u8,
    sign: i64,
    mode: &str,
) {
    let origin = points.last().map_or([0; 3], |p| p.0);
    let q = if points.len() < 2 {
        0
    } else {
        let d = std::array::from_fn::<_, 3, _>(|j| {
            points[points.len() - 1].0[j] - points[points.len() - 2].0[j]
        });
        if d[0].abs() > d[2].abs() {
            if d[0] > 0 {
                1
            } else {
                3
            }
        } else if d[2] < 0 {
            2
        } else {
            0
        }
    };
    let transform = |v| add(origin, rotate(v, q));
    if lead > 0 {
        line(points, origin, transform([0, 0, lead]), mode);
    }
    let count = usize::from(quarters) * 32;
    for i in 0..=count {
        if i == 0 && !points.is_empty() {
            continue;
        }
        let t = i as f64 / count as f64 * f64::from(quarters) * std::f64::consts::FRAC_PI_2;
        points.push((
            transform([
                sign * round(radius as f64 * (1.0 - libm::cos(t))),
                0,
                lead + round(radius as f64 * libm::sin(t)),
            ]),
            [0, 1_000_000, 0],
            mode.into(),
        ));
    }
    if lead > 0 {
        let a = points.last().unwrap().0;
        let end = if quarters == 2 {
            [sign * 2 * radius, 0, 0]
        } else {
            [sign * (radius + lead), 0, radius + lead]
        };
        line(points, a, transform(end), mode);
    }
}

fn local_piece(id: &str) -> Piece {
    static CACHE: std::sync::OnceLock<Vec<Piece>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            catalogue_ids()
                .iter()
                .map(|id| build_local_piece(id))
                .collect()
        })
        .iter()
        .find(|p| p.id == id)
        .unwrap()
        .clone()
}
// Ease the helix grade in/out over one eighth turn; peak grade remains below 23%.
fn spiral_pitch(t: f64) -> f64 {
    let edge = t.min(1.0 - t);
    (if edge < 0.125 {
        (1.0 - libm::cos(edge / 0.125 * std::f64::consts::PI)) / 2.0
    } else {
        1.0
    }) / 0.875
}
fn spiral_rise(t: f64) -> f64 {
    if t > 0.5 {
        return 1.0 - spiral_rise(1.0 - t);
    }
    (if t < 0.125 {
        t / 2.0 - 0.125 * libm::sin(t / 0.125 * std::f64::consts::PI) / (2.0 * std::f64::consts::PI)
    } else {
        t - 0.0625
    }) / 0.875
}
fn build_local_piece(original_id: &str) -> Piece {
    let narrow = original_id.ends_with("_narrow");
    let id = original_id.trim_end_matches("_narrow");
    let tube_radius = if id.starts_with("cylinder_wide") || id == "banked_chicane" {
        200
    } else {
        100
    };
    let mut p: Vec<(Vertex, Vertex, String)> = vec![];
    let mut minimum = 0;
    match id {
        "finish_plaza" => {
            line(&mut p, [0, 0, 0], [0, 0, FINISH_ENTRY_CM / 2], "drive");
            line(
                &mut p,
                [0, 0, FINISH_ENTRY_CM / 2],
                [0, 0, FINISH_ENTRY_CM],
                "drive",
            );
        }
        "curve" | "curve_left" | "sharp_curve" | "sharp_curve_left" => {
            bend(
                &mut p,
                400,
                400,
                1,
                if id.ends_with("_left") { -1 } else { 1 },
                "drift",
            );
        }
        "slope_up" | "slope_down" | "curve_up" | "curve_down" | "curve_left_up"
        | "curve_left_down" => {
            let sign = if id.ends_with("_down") { -1.0 } else { 1.0 };
            let left = id.contains("left");
            if id.starts_with("curve") {
                bend(&mut p, 400, 400, 1, if left { -1 } else { 1 }, "drift");
            } else {
                line(&mut p, [0, 0, 0], [0, 0, TILE_CM], "drive");
            }
            // Sample the leads every 25 cm, including their exact flat joins.
            // Straight leads created by bend/line use 150 cm steps. Rebuild them
            // explicitly; the circular arc already has sub-20 cm samples.
            p.retain(|v| !id.starts_with("curve") || (v.0[2] >= 400 && v.0[0].abs() <= 400));
            if id.starts_with("curve") {
                let arc = p;
                p = (0..16)
                    .map(|i| ([0, 0, i * 25], [0, 1_000_000, 0], "drive".into()))
                    .collect();
                p.extend(arc);
                for i in 1..=16 {
                    p.push((
                        [if left { -400 - i * 25 } else { 400 + i * 25 }, 0, 800],
                        [0, 1_000_000, 0],
                        "drive".into(),
                    ));
                }
            } else {
                p = (0..=32)
                    .map(|i| ([0, 0, i * 25], [0, 1_000_000, 0], "drive".into()))
                    .collect();
            }
            for v in &mut p {
                let ease = |t: f64| t * t * (3.0 - 2.0 * t);
                let height = if !id.starts_with("curve") {
                    100.0 * ease(v.0[2] as f64 / 800.0)
                } else if v.0[2] <= 400 {
                    50.0 * ease(v.0[2] as f64 / 400.0)
                } else if v.0[0].abs() >= 400 {
                    50.0 + 50.0 * ease((v.0[0].abs() - 400) as f64 / 400.0)
                } else {
                    50.0
                };
                // Preserve flat end segments after centimetre quantization too.
                // Otherwise the first 25cm chord of a turning ramp can rise 1cm
                // despite the analytic zero endpoint derivative.
                let height = round(height);
                v.0[1] = (if height <= 1 {
                    0
                } else if height >= 99 {
                    100
                } else {
                    height
                }) * if sign < 0.0 { -1 } else { 1 };
            }
        }
        "hairpin" | "hairpin_left" => {
            bend(
                &mut p,
                400,
                0,
                2,
                if id.ends_with("_left") { -1 } else { 1 },
                "drift",
            );
        }
        "chicane" => {
            for sign in [-1, 1, 1, -1] {
                bend(&mut p, 400, 0, 1, sign, "drift");
            }
        }
        id if id.starts_with("cylinder") || id == "banked_chicane" => {
            let sign = if id.ends_with("_left") { -1 } else { 1 };
            let uturn = id.contains("uturn");
            let curve = id.contains("curve");
            if curve || uturn {
                bend(
                    &mut p,
                    400,
                    400,
                    if uturn { 2 } else { 1 },
                    sign,
                    "cylinder",
                );
            } else {
                line(&mut p, [0, 0, 0], [0, 0, 800], "cylinder");
                for sign in [-1, 1, 1, -1] {
                    bend(&mut p, 400, 0, 1, sign, "cylinder");
                }
                line(&mut p, [0, 0, 2400], [0, 0, SLOT], "cylinder");
            }
            if id == "banked_chicane" {
                for sample in &mut p {
                    sample.2 = "halfpipe".into();
                }
            }
            if id.ends_with("_s_rise") {
                // Smooth 0.8 m hill, with level entry/exit. Tangents define its frame.
                for v in &mut p {
                    let t = v.0[2] as f64 / SLOT as f64;
                    v.0[1] += round(80.0 * libm::sin(std::f64::consts::PI * t).powi(2));
                }
            }
        }
        "loop" => {
            minimum = 1500;
            connector(&mut p, [0, 0, 0], [-LOOP_OFFSET, 0, 600], "boost");
            line(
                &mut p,
                [-LOOP_OFFSET, 0, 600],
                [-LOOP_OFFSET, 0, 1000],
                "boost",
            );
            p.last_mut().unwrap().2 = "loop".into();
            let mesh = SpecialTrack {
                kind: TrackKind::Loop,
                radius_cm: LOOP_RADIUS,
                width_cm: LOOP_WIDTH,
                length_cm: 1600,
                centerline: vec![],
            }
            .mesh();
            // Same ribbon vertices as the existing loop, including separated ends.
            for (i, faces) in mesh.inner.chunks_exact(2).enumerate() {
                if i == 0 {
                    continue;
                }
                let pos = std::array::from_fn(|a| (faces[0][0][a] + faces[1][2][a]) / 200);
                let t = i as f64 * std::f64::consts::TAU / 256.0;
                p.push((
                    add(pos, [0, 0, 1000]),
                    [0, round(libm::cos(t) * 1e6), round(-libm::sin(t) * 1e6)],
                    "loop".into(),
                ));
            }
            let end = [
                LOOP_OFFSET,
                0,
                1000 + round(f64::from(LOOP_RADIUS) * 0.6 * std::f64::consts::PI),
            ];
            p.push((end, [0, 1_000_000, 0], "loop".into()));
            connector(&mut p, end, [0, 0, SLOT], "drive");
        }
        "spiral_up" | "spiral_down" => {
            let down = id == "spiral_down";
            connector(&mut p, [0, 0, 0], [-800, 0, 1600], "drive");
            for i in 1..=192 {
                let fraction = i as f64 / 192.0;
                let t = fraction * std::f64::consts::TAU;
                let direction = if down { -1.0 } else { 1.0 };
                let height = spiral_rise(fraction) * TILE_CM as f64 * direction;
                let pitch = spiral_pitch(fraction) * TILE_CM as f64
                    / (800.0 * std::f64::consts::TAU)
                    * direction;
                p.push((
                    [
                        -round(800.0 * libm::cos(t)),
                        round(height),
                        1600 + round(800.0 * libm::sin(t)),
                    ],
                    unit([-libm::sin(t) * pitch, 1.0, -libm::cos(t) * pitch]),
                    "spiral".into(),
                ));
            }
            let end = p.last().unwrap().0;
            connector(&mut p, end, [0, end[1], SLOT], "drive");
        }
        "slope" => {
            line(&mut p, [0, 0, 0], [0, 0, 400], "drive");
            line(&mut p, [0, 0, 400], [0, 250, 1600], "drive");
            line(&mut p, [0, 250, 1600], [0, 0, 2800], "drive");
            line(&mut p, [0, 0, 2800], [0, 0, SLOT], "drive");
        }
        "zigzag" => {
            // Two mirrored chicanes: eight compact 90-degree turns across a
            // 16 m lateral span. A wider lane must not turn this into a slalom
            // that can be driven straight through.
            for sign in [-1, 1, 1, -1, 1, -1, -1, 1] {
                bend(&mut p, 400, 0, 1, sign, "drift");
            }
        }
        "overpass" => {
            line(&mut p, [0, 0, 0], [0, 0, 400], "drift");
            for sign in [-1, 1, 1, -1, 1, -1, -1, 1] {
                bend(&mut p, 300, 0, 1, sign, "drift");
            }
            line(&mut p, [0, 0, 2800], [0, 0, SLOT], "drift");
        }
        "roller_waves" => {
            for i in 0..=128 {
                let t = i as f64 / 128.0;
                p.push((
                    [
                        0,
                        round(80.0 * libm::sin(t * std::f64::consts::PI * 4.0).powi(2)),
                        round(t * SLOT as f64),
                    ],
                    [0, 1_000_000, 0],
                    "roller".into(),
                ));
            }
        }
        "jump" | "air_ring" | "offset_jump" => {
            minimum = 1500;
            line(&mut p, [0, 0, 0], [0, 0, 1000], "drive");
            // Jump-height pad supplies the vertical impulse; a real open gap follows.
            for i in 1..=32 {
                let t = i as f64 / 32.0;
                p.push((
                    [
                        0,
                        round(4.0 * 200.0 * t * (1.0 - t)),
                        1000 + round(t * 1200.0),
                    ],
                    [0, 1_000_000, 0],
                    if i == 32 { "drive" } else { "flight" }.into(),
                ));
            }
            line(&mut p, [0, 0, 2200], [0, 0, SLOT], "drive");
        }
        "sprint_lane" => line(&mut p, [0, 0, 0], [0, 0, 1600], "drive"),
        _ => line(&mut p, [0, 0, 0], [0, 0, SLOT], "drive"),
    }
    if id == "offset_jump" {
        for sample in &mut p {
            let z = sample.0[2] as f64;
            let t = if z < 2200.0 {
                (z - 900.0) / 1300.0
            } else {
                (3200.0 - z) / 1000.0
            };
            let t = t.clamp(0.0, 1.0);
            sample.0[0] = -round(150.0 * t * t * (3.0 - 2.0 * t));
        }
    }
    let small = ["straight", "slope"].contains(&id);
    if small {
        for (v, _, _) in &mut p {
            v[2] /= 4;
            v[0] /= 4;
            v[1] /= 4;
        }
    } else if short_piece(id) {
        for (v, _, _) in &mut p {
            v[2] /= 2;
            v[0] = -round(80.0 * libm::sin(std::f64::consts::PI * v[2] as f64 / 1600.0).powi(2));
        }
    }
    let len = p.windows(2).map(|v| distance(v[0].0, v[1].0)).sum::<u64>();
    let mut path = p
        .iter()
        .enumerate()
        .map(|(i, (pos, normal, mode))| {
            let a = p[i.saturating_sub(1)].0;
            let b = p[(i + 1).min(p.len() - 1)].0;
            let normal = if i == 0 || i + 1 == p.len() {
                [0, 1_000_000, 0]
            } else if mode == "loop" || mode == "spiral" {
                *normal
            } else {
                let f: [f64; 3] = std::array::from_fn(|j| (b[j] - a[j]) as f64);
                unit([-f[0] * f[1], f[0] * f[0] + f[2] * f[2], -f[2] * f[1]])
            };
            let mut mode = mode.clone();
            if ["boost_chain", "acceleration_panel"].contains(&id) {
                mode = "boost".into();
            }
            let forward = if i == 0 {
                [0, 0, 1_000_000]
            } else if i + 1 == p.len() {
                if id.starts_with("hairpin") || id.contains("uturn") {
                    [0, 0, -1_000_000]
                } else if id == "curve"
                    || id == "sharp_curve"
                    || (id.contains("curve") && !id.contains("left"))
                {
                    [1_000_000, 0, 0]
                } else if id == "curve_left"
                    || id == "sharp_curve_left"
                    || (id.contains("curve") && id.contains("left"))
                {
                    [-1_000_000, 0, 0]
                } else {
                    [0, 0, 1_000_000]
                }
            } else {
                unit(std::array::from_fn(|j| (b[j] - a[j]) as f64))
            };
            Sample {
                position_cm: *pos,
                normal,
                forward,
                safe: ["drive", "drift", "bridge"].contains(&mode.as_str())
                    && !id.starts_with("cylinder")
                    && !["banked_chicane", "roller_waves", "offset_jump"].contains(&id)
                    && !["loop", "jump", "air_ring", "spiral_up", "spiral_down"].contains(&id),
                min_speed_cmps: minimum,
                tube_radius_cm: if ["cylinder", "halfpipe"].contains(&mode.as_str()) {
                    tube_radius
                } else {
                    0
                },
                lateral_cm: if ["cylinder", "halfpipe"].contains(&mode.as_str()) {
                    tube_radius
                } else if id == "loop" {
                    if mode == "loop" {
                        LOOP_WIDTH / 2
                    } else {
                        let end = 1000 + round(f64::from(LOOP_RADIUS) * 0.6 * std::f64::consts::PI);
                        let taper = if pos[2] < 1000 {
                            1.0 - pos[2] as f64 / 1000.0
                        } else {
                            (pos[2] - end) as f64 / (SLOT - end) as f64
                        };
                        LOOP_WIDTH / 2
                            + round(
                                (WIDTH as f64 - LOOP_WIDTH as f64) / 2.0 * taper.clamp(0.0, 1.0),
                            ) as u32
                    }
                } else {
                    WIDTH as u32 / 2
                },
                below_cm: if mode == "cylinder" { 50 } else { 150 },
                above_cm: if mode == "cylinder" {
                    250
                } else if mode == "flight" {
                    700
                } else {
                    300
                },
                mode,
            }
        })
        .collect::<Vec<_>>();
    if narrow {
        let total = path
            .windows(2)
            .map(|w| distance(w[0].position_cm, w[1].position_cm))
            .sum::<u64>() as f64;
        let mut station = 0.0;
        for i in 0..path.len() {
            if i > 0 {
                station += distance(path[i - 1].position_cm, path[i].position_cm) as f64;
            }
            let t = (station.min(total - station) / 300.0).clamp(0.0, 1.0);
            path[i].lateral_cm = 200 - round(100.0 * t * t * (3.0 - 2.0 * t)) as u32;
        }
    }
    let alternate_path = if id == "overpass" {
        (0..=64)
            .map(|i| {
                let z = i * 50;
                let edge = z.min(3200 - z) as f64;
                let direction = if z < 1600 { 1.0 } else { -1.0 };
                let t = ((edge - 400.0) / 600.0).clamp(0.0, 1.0);
                let lateral = (edge / 400.0).clamp(0.0, 1.0);
                // Separated ramps lead to a straight diagonal deck, crossing the
                // ground chicane at its centre with 2m vertical clearance.
                let (x, dx) = if (800..=2400).contains(&z) {
                    let u = (z - 800) as f64 / 1600.0;
                    (320.0 - 640.0 * spiral_rise(u), -0.4 * spiral_pitch(u))
                } else {
                    (
                        320.0 * lateral * lateral * (3.0 - 2.0 * lateral) * direction,
                        if lateral < 1.0 {
                            320.0 * 6.0 * lateral * (1.0 - lateral) / 400.0
                        } else {
                            0.0
                        },
                    )
                };
                let dy = if t > 0.0 && t < 1.0 {
                    200.0 * 6.0 * t * (1.0 - t) / 600.0 * direction
                } else {
                    0.0
                };
                Sample {
                    position_cm: [round(x), round(200.0 * t * t * (3.0 - 2.0 * t)), z],
                    normal: unit([-dx * dy, dx * dx + 1.0, -dy]),
                    forward: unit([dx, dy, 1.0]),
                    mode: "bridge".into(),
                    safe: true,
                    min_speed_cmps: 0,
                    lateral_cm: 100,
                    tube_radius_cm: 0,
                    below_cm: 35,
                    above_cm: 300,
                }
            })
            .collect()
    } else {
        vec![]
    };
    let lo = std::array::from_fn(|a| {
        path.iter()
            .chain(&alternate_path)
            .map(|s| s.position_cm[a])
            .min()
            .unwrap()
            - if a == 1 { 200 } else { 320 }
    });
    let hi = std::array::from_fn(|a| {
        path.iter()
            .chain(&alternate_path)
            .map(|s| s.position_cm[a])
            .max()
            .unwrap()
            + if a == 1 { 1600 } else { 320 }
    });
    Piece {
        rotation_mdeg: [0; 3],
        control_points: vec![],
        id: original_id.into(),
        width_cm: if narrow { 200 } else { WIDTH as u32 },
        entry_width_cm: WIDTH as u32,
        exit_width_cm: WIDTH as u32,
        chain_id: 0,
        chain_index: 0,
        chain_count: 0,
        ordinary: basic_ids().contains(&id),
        cube_span: if id == "chicane" {
            2
        } else if small {
            1
        } else if short_piece(id) || id == "sprint_lane" {
            2
        } else {
            4
        },
        entry_speed_cmps: [
            minimum,
            if id.starts_with("cylinder") {
                1200
            } else if id.contains("curve")
                || id.starts_with("hairpin")
                || ["chicane", "zigzag", "overpass"].contains(&id)
            {
                650
            } else {
                3000
            },
        ],
        connection_width_cm: WIDTH as u32,
        origin_cm: [0; 3],
        quarter_turns: 0,
        path,
        alternate_path,
        reserved_min_cm: lo,
        reserved_max_cm: hi,
        reference_msec: (len * 1000 / SPEED as u64) as u32,
    }
}

fn next(rng: &mut u64) -> u64 {
    *rng = rng.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *rng;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn family(id: &str) -> &str {
    if id.starts_with("cylinder") {
        "cylinder"
    } else {
        id
    }
}
fn ramp_tiles(width: u32) -> usize {
    ((f64::from(width) / 3.0 * 1.5 / 0.12) / TILE_CM as f64).ceil() as usize
}
fn pipe_piece(id: &str) -> bool {
    id.starts_with("cylinder") || id == "banked_chicane"
}
/// Only authored outer portals drop. Internal tube joints remain flush.
fn portal_drop(previous: &str, next: &str, width: u32) -> i64 {
    if previous == "tube_entry" && pipe_piece(next) || pipe_piece(previous) && next == "tube_exit" {
        round(f64::from(width) / 3.0)
    } else {
        0
    }
}
fn variant(id: &str, width: u32, entry: u32, exit: u32) -> Piece {
    let mut p = local_piece(id);
    geometry::shape(&mut p, width.max(entry).max(exit));
    p.width_cm = width;
    p.entry_width_cm = entry;
    p.exit_width_cm = exit;
    p.connection_width_cm = entry;
    if ["tube_entry", "tube_exit"].contains(&id) {
        let tiles = ramp_tiles(width);
        let length = tiles as i64 * TILE_CM;
        let height = round(f64::from(width) / 3.0);
        p.path = (0..=tiles * 32)
            .map(|i| {
                let t = i as f64 / (tiles * 32) as f64;
                let mut sample = p.path[0].clone();
                // Entry rises to the portal; departure starts below the pipe
                // floor and rises back to ordinary road level without a lip.
                let sign = 1.0;
                let dy = sign * height as f64 * 6.0 * t * (1.0 - t) / length as f64;
                sample.position_cm = [
                    0,
                    round(sign * height as f64 * t * t * (3.0 - 2.0 * t)),
                    round(length as f64 * t),
                ];
                sample.forward = unit([0.0, dy, 1.0]);
                sample.normal = unit([0.0, 1.0, -dy]);
                sample.safe = false;
                sample.mode = "drive".into();
                sample
            })
            .collect();
        p.cube_span = tiles as u8;
    }
    if id.starts_with("cylinder") {
        for v in &mut p.path {
            v.tube_radius_cm = width / 2;
            v.lateral_cm = width / 2;
            v.safe = false;
        }
    } else {
        if !["loop", "banked_chicane", "overpass"].contains(&id) {
            geometry::taper(&mut p.path, width, entry, exit);
        }
    }

    if id == "finish_plaza" {
        let mut center = p.path.last().unwrap().clone();
        center.position_cm = plaza_center(&p);
        center.lateral_cm = FINISH_RADIUS_CM as u32;
        p.path.push(center);
    }
    // Dedicated structures keep their own dimensions; their outer ports are 4m.
    p.reference_msec = (race_length(&p) * 1000 / SPEED as u64) as u32;
    p
}
fn materialize(p: &Piece) -> Piece {
    let mut out = variant(&p.id, p.width_cm, p.entry_width_cm, p.exit_width_cm);
    out.origin_cm = p.origin_cm;
    out.quarter_turns = p.quarter_turns;
    out.rotation_mdeg = p.rotation_mdeg;
    out.control_points = p.control_points.clone();
    if !p.control_points.is_empty() {
        out.path = geometry::bezier(
            &p.control_points,
            p.width_cm,
            p.entry_width_cm,
            p.exit_width_cm,
        );
        if p.id == "flight_curve" {
            for s in &mut out.path {
                s.mode = "flight".into();
                s.safe = false;
                s.min_speed_cmps = 1500;
                s.above_cm = 700;
            }
        }
    }
    if out.id != "finish_plaza" {
        geometry::refine(&mut out.path);
        geometry::refine(&mut out.alternate_path);
    }
    out.chain_id = p.chain_id;
    out.chain_index = p.chain_index;
    out.chain_count = p.chain_count;
    out.ordinary = p.ordinary;
    for v in out.path.iter_mut().chain(&mut out.alternate_path) {
        v.position_cm = add(
            geometry::rotate3(v.position_cm, p.rotation_mdeg),
            p.origin_cm,
        );
        v.forward = geometry::rotate3(v.forward, p.rotation_mdeg);
        v.normal = geometry::rotate3(v.normal, p.rotation_mdeg);
    }
    out.reserved_min_cm = std::array::from_fn(|j| {
        out.path
            .iter()
            .chain(&out.alternate_path)
            .map(|v| v.position_cm[j])
            .min()
            .unwrap()
            - if j == 1 {
                p.width_cm as i64
            } else {
                p.width_cm as i64 / 2 + 60
            }
    });
    out.reserved_max_cm = std::array::from_fn(|j| {
        out.path
            .iter()
            .chain(&out.alternate_path)
            .map(|v| v.position_cm[j])
            .max()
            .unwrap()
            + if j == 1 {
                1600
            } else {
                p.width_cm as i64 / 2 + 60
            }
    });
    if out.id == "finish_plaza" {
        let center = plaza_center(&out);
        for j in [0, 2] {
            out.reserved_min_cm[j] = out.reserved_min_cm[j].min(center[j] - FINISH_RADIUS_CM - 60);
            out.reserved_max_cm[j] = out.reserved_max_cm[j].max(center[j] + FINISH_RADIUS_CM + 60);
        }
    }
    out.reference_msec = (race_length(&out) * 1000 / SPEED as u64) as u32;
    out
}
fn push_piece(
    pieces: &mut Vec<Piece>,
    origin: &mut Vertex,
    heading: u8,
    id: &str,
    w: u32,
    ordinary: bool,
    chain: [u32; 3],
) {
    if let Some(previous) = pieces.last() {
        origin[1] -= portal_drop(&previous.id, id, w);
    }
    let mut p = variant(id, w, w, w);
    p.origin_cm = *origin;
    p.quarter_turns = heading;
    p.rotation_mdeg = [0, i32::from(heading) * 90000, 0];
    p.ordinary = ordinary;
    p.chain_id = chain[0];
    p.chain_index = chain[1];
    p.chain_count = chain[2];
    p = materialize(&p);
    *origin = p.path.last().unwrap().position_cm;
    pieces.push(p);
}
fn statistics(pieces: &[Piece]) -> (u64, u64, u64, u32, VenueFloor) {
    let (mut length, mut ordinary, mut straight, mut time) = (0, 0, 0, 0);
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for p in pieces {
        time += p.reference_msec;
        for v in p.path.iter().chain(&p.alternate_path) {
            let (_, _, bottom, top) = geometry::volume(v);
            lo[1] = lo[1].min(bottom);
            hi[1] = hi[1].max(top);
            for j in 0..3 {
                lo[j] = lo[j].min(v.position_cm[j]);
                hi[j] = hi[j].max(v.position_cm[j]);
            }
        }
        length += race_length(p);
        for w in p.path.windows(2) {
            let d = distance(w[0].position_cm, w[1].position_cm);
            if !p.ordinary {
                continue;
            }
            ordinary += d;
            // Horizontal heading, not the piece name: slopes count as straight.
            let a = w[0].forward;
            let b = w[1].forward;
            if (a[0] as i128 * b[2] as i128 - a[2] as i128 * b[0] as i128).abs() < 5_000_000_000 {
                straight += d;
            }
        }
    }
    let floor_y = lo[1] - 235; // Below every road and shell; never an AI/spawn surface.
    (
        length,
        ordinary,
        straight,
        time,
        VenueFloor {
            min_cm: [lo[0] - 1600, floor_y, lo[2] - 1600],
            max_cm: [hi[0] + 1600, floor_y, hi[2] + 1600],
        },
    )
}
pub fn assemble(settings: &Settings) -> Result<Assembly> {
    layout::assemble(settings)
}

impl Assembly {
    pub fn validate(&self) -> Result<()> {
        if let Some(source) = &self.authoring {
            if *self != authoring::compile(source)? {
                return Err(error(
                    "E_TRACK_MODIFIED",
                    "compiled geometry differs from authoring source",
                ));
            }
            return Ok(());
        }
        if let Some(source) = &self.seed_source {
            let mut expected = authoring::compile(source)?;
            expected.authoring = None;
            expected.seed_source = Some(source.clone());
            if *self != expected || !self.issues.is_empty() {
                return Err(error("E_TRACK_MODIFIED", "seed graph compilation differs"));
            }
            return Ok(());
        }
        let fail = || {
            error(
                "E_TRACK_ASSEMBLY",
                "invalid compiled seed geometry, routes or resource budget",
            )
        };
        if self.settings.normalized()? != self.settings
            || self.pieces.len() < START_PIECES
            || self.pieces.len() > MAX_PIECES
            || self
                .pieces
                .iter()
                .map(|p| p.path.len() + p.alternate_path.len())
                .sum::<usize>()
                > MAX_SAMPLES
            || self.generator_fingerprint != fingerprint()
            || self.catalogue_fingerprint != catalogue_fingerprint()
            || !layout::valid_runs(&self.pieces, self.settings.circuit)
        {
            return Err(fail());
        }
        for p in &self.pieces {
            cancellation::checkpoint()?;
            if !catalogue_ids().contains(&p.id.as_str())
                || !supported_widths(&p.id).contains(&p.width_cm)
                || p.path.len() < 2
                || *p != materialize(p)
                || geometry::self_intersects(p)
            {
                return Err(fail());
            }
        }
        for route in &self.routes {
            if route.pieces.is_empty() || route.pieces.iter().any(|i| *i >= self.pieces.len()) {
                return Err(fail());
            }
            for pair in route.pieces.windows(2) {
                let a = &self.pieces[pair[0]];
                let b = &self.pieces[pair[1]];
                let mut end = a.path.last().unwrap().clone();
                end.position_cm[1] -= portal_drop(&a.id, &b.id, b.width_cm);
                if !authoring::joined(&end, &b.path[0]) {
                    return Err(fail());
                }
            }
            if self.settings.circuit
                && !authoring::joined(
                    self.pieces[*route.pieces.last().unwrap()]
                        .path
                        .last()
                        .unwrap(),
                    &self.pieces[route.pieces[0]].path[0],
                )
            {
                return Err(fail());
            }
        }
        for i in 0..self.pieces.len() {
            for j in 0..i.saturating_sub(1) {
                if self.settings.circuit && i + 1 == self.pieces.len() && j == 0 {
                    continue;
                }
                if authoring::conflict(&self.pieces[i], &self.pieces[j]) {
                    return Err(fail());
                }
            }
        }
        if self.routes.is_empty() {
            return Err(fail());
        }
        let base: Vec<_> = self.routes[0]
            .pieces
            .iter()
            .map(|i| self.pieces[*i].clone())
            .collect();
        let stats = statistics(&base);
        if (stats.0, stats.1, stats.2, stats.3)
            != (
                self.length_cm,
                self.ordinary_length_cm,
                self.ordinary_straight_cm,
                self.estimated_msec,
            )
        {
            return Err(fail());
        }
        let (obstacles, eligible, target) = obstacles::place(self)?;
        if self.obstacles != obstacles
            || self.obstacle_eligible_length_cm != eligible
            || self.obstacle_target_count != target
        {
            return Err(fail());
        }
        let mut grounded = self.clone();
        grounding::apply(&mut grounded)?;
        if self.floor != grounded.floor || self.supports != grounded.supports {
            return Err(fail());
        }
        Ok(())
    }
}

fn box_part(center: Vertex, size: Vertex) -> CollisionConvex {
    let vertices = (0..8)
        .map(|i| {
            std::array::from_fn(|a| {
                center[a]
                    + if i & (1 << a) == 0 {
                        -size[a] / 2
                    } else {
                        size[a] / 2
                    }
            })
        })
        .collect();
    CollisionConvex {
        vertices,
        faces: vec![
            [0, 2, 3],
            [0, 3, 1],
            [4, 5, 7],
            [4, 7, 6],
            [0, 1, 5],
            [0, 5, 4],
            [2, 6, 7],
            [2, 7, 3],
            [0, 4, 6],
            [0, 6, 2],
            [1, 3, 7],
            [1, 7, 5],
        ],
    }
}
fn road_gimmicks(a: &Assembly) -> Vec<Gimmick> {
    let mut out = vec![];
    for (index, p) in a.pieces.iter().enumerate() {
        let specs: Vec<(&str, i64)> = match p.id.as_str() {
            "loop" => vec![("acceleration_panel", 350), ("loop", 1000)],
            id if id.starts_with("cylinder") => vec![("cylinder", 0)],
            "banked_chicane" => vec![("halfpipe", 0)],
            "jump" | "offset_jump" | "jump_panel" => {
                vec![("acceleration_panel", 350), ("jump", 950)]
            }
            "air_ring" => vec![
                ("acceleration_panel", 350),
                ("jump", 950),
                ("air_ring", 1600),
            ],
            "boost_chain" => vec![
                ("acceleration_panel", 600),
                ("acceleration_panel", 1600),
                ("acceleration_panel", 2600),
            ],
            "acceleration_panel" => {
                vec![(p.id.as_str(), 1600)]
            }
            _ => vec![],
        };
        for (n, (id, z)) in specs.iter().enumerate() {
            let mut g = Gimmick {
                id: format!("track-{index}-{n}"),
                position: add(
                    p.origin_cm,
                    geometry::rotate3(
                        [0, 0, if short_piece(&p.id) { *z / 2 } else { *z }],
                        p.rotation_mdeg,
                    ),
                ),
                rotation_mdeg: p.rotation_mdeg,
                scale_per_mille: [1000; 3],
                parts: vec![],
                track: None,
                effect: None,
                surface: Surface::Asphalt,
                color: [60, 160, 230, 255],
                motion: Motion {
                    kind: MotionKind::Static,
                    delta_cm: [0; 3],
                    axis: 1,
                    period_ms: 4000,
                    phase_ms: 0,
                    impulse_cmps: [0; 3],
                    cooldown_ms: 1500,
                },
                safety_min_cm: [0; 3],
                safety_max_cm: [0; 3],
            };
            match *id {
                "loop" | "cylinder" | "halfpipe" => {
                    g.track = Some(SpecialTrack {
                        kind: if *id == "loop" {
                            TrackKind::Loop
                        } else if *id == "halfpipe" {
                            TrackKind::SweptHalfPipe
                        } else {
                            TrackKind::SweptCylinder
                        },
                        radius_cm: if *id == "loop" {
                            LOOP_RADIUS
                        } else if *id == "halfpipe" {
                            200
                        } else {
                            p.width_cm / 2
                        },
                        width_cm: if *id == "loop" {
                            LOOP_WIDTH
                        } else {
                            WIDTH as u32
                        },
                        length_cm: 1600,
                        centerline: if *id == "loop" {
                            vec![]
                        } else {
                            variant(&p.id, p.width_cm, p.entry_width_cm, p.exit_width_cm)
                                .path
                                .iter()
                                .filter(|s| ["cylinder", "halfpipe"].contains(&s.mode.as_str()))
                                .map(|s| TubeFrame {
                                    floor_cm: s.position_cm,
                                    normal: s.normal,
                                    forward: s.forward,
                                })
                                .collect()
                        },
                    })
                }
                "air_ring" => {
                    g.position[1] += 200;
                    for side in [-1, 1] {
                        g.parts.push(box_part([side * 275, 0, 0], [50, 600, 30]));
                        g.parts.push(box_part([0, side * 275, 0], [500, 50, 30]));
                    }
                    g.motion.kind = MotionKind::AirRing;
                    g.effect = Some(Effect {
                        strength_percent: 50,
                        jump_height_cm: 200,
                        ring_radius_cm: 250,
                    });
                }
                _ => {
                    g.parts
                        .push(box_part([0, -2, 0], [p.width_cm as i64 - 50, 4, 200]));
                    g.motion.kind = if *id == "jump" {
                        MotionKind::JumpHeight
                    } else {
                        MotionKind::TargetSpeed
                    };
                    g.effect = Some(Effect {
                        strength_percent: 100,
                        jump_height_cm: 200,
                        ring_radius_cm: 250,
                    });
                }
            }
            let radius = g.track.as_ref().map_or(2500, |t| t.bound_radius());
            g.safety_min_cm = g.position.map(|v| v - radius);
            g.safety_max_cm = g.position.map(|v| v + radius);
            out.push(g);
        }
    }
    out
}
fn gimmicks(a: &Assembly) -> Vec<Gimmick> {
    let mut out = road_gimmicks(a);
    out.extend(
        a.obstacles
            .iter()
            .enumerate()
            .map(|(i, o)| obstacles::gimmick(a, o, i)),
    );
    out
}
pub fn verify_products(d: &MapDocument, a: &Assembly) -> Result<()> {
    let mut expected = gimmicks(a);
    expected.extend(authoring::action_gimmicks(a)?);
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    let mut actual = d.gimmicks.clone();
    actual.sort_by(|a, b| a.id.cmp(&b.id));
    let mut expected_lines=obstacles::grind_lines(a); expected_lines.sort_by(|a,b|a.id.cmp(&b.id));
    let mut actual_lines=d.grind_lines.clone(); actual_lines.sort_by(|a,b|a.id.cmp(&b.id));
    if actual != expected || actual_lines != expected_lines
        || d.seed != a.settings.seed
        || !d.nodes.is_empty()
        || !d.roads.is_empty()
        || !d.heightmaps.is_empty()
        || !d.placements.is_empty()
    {
        return Err(error(
            "E_TRACK_MODIFIED",
            "track products differ from their authoring source",
        ));
    }
    Ok(())
}
pub fn document(settings: &Settings) -> Result<MapDocument> {
    let a = assemble(settings)?;
    document_from_assembly(a)
}
pub fn document_from_assembly(a: Assembly) -> Result<MapDocument> {
    let settings = a.settings.clone();
    let mut bounds = Bounds {
        min: [i64::MAX; 2],
        max: [i64::MIN; 2],
    };
    for p in &a.pieces {
        for j in 0..2 {
            bounds.min[j] = bounds.min[j].min(p.reserved_min_cm[j * 2] - 4000);
            bounds.max[j] = bounds.max[j].max(p.reserved_max_cm[j * 2] + 4000);
        }
    }
    if a.pieces.is_empty() {
        bounds = Bounds {
            min: [-3200, -3200],
            max: [3200, 3200],
        };
    }
    let mut objects = gimmicks(&a);
    objects.extend(authoring::action_gimmicks(&a)?);
    for g in &objects {
        for j in 0..2 {
            bounds.min[j] = bounds.min[j].min(g.safety_min_cm[j * 2]);
            bounds.max[j] = bounds.max[j].max(g.safety_max_cm[j * 2]);
        }
    }
    let grind_lines=obstacles::grind_lines(&a);
    for line in &grind_lines {
        for point in &line.control_points { for j in 0..2 {
            bounds.min[j]=bounds.min[j].min(point[j*2]-400);
            bounds.max[j]=bounds.max[j].max(point[j*2]+400);
        }}
    }
    let mut d = MapDocument {
        free_roam: false,
        assembled_track: Some(a.clone()),
        water_bodies: vec![],
        grind_lines,
        gimmicks: objects,
        courses: vec![],
        environment: Some(environment::EnvironmentProfile {
            version: 1,
            concept: "countryside".into(),
            architecture: "".into(),
            climate: "temperate".into(),
            settlement: "sparse".into(),
            start_minutes: Some(settings.time_minutes),
            ground_color: Some([70, 120, 145]),
            latitude_mdeg: 35000,
            longitude_mdeg: 0,
            utc_offset_minutes: 0,
            sunrise_minutes: 360,
            sunset_minutes: 1080,
            regions: vec![],
            lights: vec![],
        }),
        indexed_topology: false,
        map_id: format!("track-{}", &sha256(&canonical(&a)?)[..24]),
        revision: 1,
        bounds,
        cell_size_cm: 12800,
        seed: settings.seed,
        recipe_version: 1,
        theme: "default".into(),
        terrain_base_cm: -2000,
        heightmaps: vec![],
        nodes: vec![],
        roads: vec![],
        surface_areas: vec![],
        buildings: vec![],
        zones: vec![],
        assets: vec![],
        placements: vec![],
        repetitions: vec![],
        attributions: vec![],
        provenance: Provenance {
            tool_id: "mapkit-track".into(),
            version: "1".into(),
            build_id: fingerprint(),
            fingerprint: fingerprint(),
            first_created: "2026-09-27T00:00:00Z".into(),
            last_edited: "2026-09-27T00:00:00Z".into(),
        },
    };
    d.normalize();
    d.validate()?;
    Ok(d)
}
/// Full equality against fresh deterministic source, never a producer success flag.
pub fn verify_document(d: &MapDocument) -> Result<()> {
    let a = d
        .assembled_track
        .as_ref()
        .ok_or_else(|| error("E_TRACK_REQUIRED", "map is not an assembled track"))?;
    a.validate()?;
    let expected = if let Some(source) = &a.authoring {
        document_from_assembly(authoring::compile(source)?)?
    } else {
        document(&a.settings)?
    };
    let mut actual = d.clone();
    actual.courses.clear();
    actual.free_roam = expected.free_roam;
    actual.provenance = expected.provenance.clone();
    actual.attributions = expected.attributions.clone();
    actual.normalize();
    if actual != expected {
        return Err(error(
            "E_TRACK_MODIFIED",
            "map content or assembly differs from current generator",
        ));
    }
    Ok(())
}

/// Faces and occupancy share this geometry; background never enters this path.
pub(crate) fn generate(a: &Assembly, b: &mut crate::generation::Builder) -> Result<()> {
    let f = &a.floor;
    b.quad(
        [
            [f.min_cm[0], f.min_cm[1], f.min_cm[2]],
            [f.min_cm[0], f.min_cm[1], f.max_cm[2]],
            [f.max_cm[0], f.min_cm[1], f.max_cm[2]],
            [f.max_cm[0], f.min_cm[1], f.min_cm[2]],
        ],
        Surface::Concrete,
        "assembled-venue-floor",
        false,
    )?;
    b.solid(
        "assembled-venue-floor",
        SolidShape::Box {
            min: [f.min_cm[0], f.min_cm[1] - 50, f.min_cm[2]],
            max: f.max_cm,
        },
    )?;
    for support in &a.supports {
        let id = format!("assembled-support-{}", support.piece_index);
        emit_shape(&support.shape, &id, b)?;
    }
    for (index, p) in a.pieces.iter().enumerate() {
        generate_piece(p, index, b, &junction::neighbors(a, index))?;
    }
    Ok(())
}

fn generate_piece(p: &Piece, index: usize, b: &mut impl TrackGeometry, neighbors: &[&Piece]) -> Result<()> {
    if p.id == "finish_plaza" {
        return generate_plaza(p, index, b);
    }
    for (branch, path) in [(false, &p.path), (true, &p.alternate_path)] {
        for w in path.windows(2) {
            cancellation::checkpoint()?;
            let special = w.iter().any(|s| s.mode == "flight")
                || w.iter()
                    .all(|s| ["loop", "cylinder", "halfpipe"].contains(&s.mode.as_str()));
            let edges = |s: &Sample| {
                let n = s.normal.map(|v| v as f64 / 1e6);
                let f = s.forward.map(|v| v as f64 / 1e6);
                let right = if s.mode == "loop" {
                    geometry::rotate3([1_000_000, 0, 0], p.rotation_mdeg)
                } else {
                    unit([
                        n[1] * f[2] - n[2] * f[1],
                        n[2] * f[0] - n[0] * f[2],
                        n[0] * f[1] - n[1] * f[0],
                    ])
                };
                [-1, 1].map(|side| {
                    std::array::from_fn(|j| {
                        s.position_cm[j] + right[j] * i64::from(s.lateral_cm) * side / 1_000_000
                    })
                })
            };
            let [al, ar] = edges(&w[0]);
            let [bl, br] = edges(&w[1]);
            let id = format!(
                "assembled-road-{index}{}",
                if branch { "-bridge" } else { "" }
            );
            if !special {
                // A wide twisted helix quad creates a diagonal ridge. Subdivide
                // across the lane so wheel contacts follow the swept surface.
                let strips = geometry::strips(&w[0], &w[1]);
                let mix = |a: Vertex, b: Vertex, n: i64| {
                    std::array::from_fn(|j| a[j] + (b[j] - a[j]) * n / strips)
                };
                for strip in 0..strips {
                    b.quad(
                        [
                            mix(al, ar, strip),
                            mix(bl, br, strip),
                            mix(bl, br, strip + 1),
                            mix(al, ar, strip + 1),
                        ],
                        Surface::Asphalt,
                        &id,
                        true,
                    )?;
                    b.quad(
                        [
                            lower(mix(ar, al, strips - strip)),
                            lower(mix(ar, al, strips - strip - 1)),
                            lower(mix(br, bl, strips - strip - 1)),
                            lower(mix(br, bl, strips - strip)),
                        ],
                        Surface::Concrete,
                        &format!("assembled-shell-{index}"),
                        false,
                    )?;
                }
                slab_sides([al, bl, br, ar], &format!("assembled-shell-{index}"), b)?;
                let min = std::array::from_fn(|j| {
                    [al, ar, bl, br].iter().map(|v| v[j]).min().unwrap()
                        - if j == 1 { 10 } else { 0 }
                });
                let max =
                    std::array::from_fn(|j| [al, ar, bl, br].iter().map(|v| v[j]).max().unwrap());
                b.solid(&id, SolidShape::Box { min, max })?;
            }
            if w.iter()
                .any(|s| ["cylinder", "halfpipe", "flight"].contains(&s.mode.as_str()))
            {
                continue;
            }
            let station = |s: &Sample| {
                let f = geometry::rotate3([0, 0, 1_000_000], p.rotation_mdeg);
                (0..3)
                    .map(|j| (s.position_cm[j] - p.origin_cm[j]) * f[j] / 1_000_000)
                    .sum::<i64>()
            };
            if p.id == "overpass" && (station(&w[0]) < 400 || station(&w[1]) > 2800) {
                continue;
            }
            let wall = if branch {
                30
            } else if p.id.starts_with("cylinder") || p.id == "banked_chicane" {
                25 + 30 * station(&w[0]).clamp(0, 400) / 400
            } else if w[0].mode == "loop" {
                120
            } else {
                WALL
            };
            for (edge_a, edge_b) in [(al, bl), (ar, br)] {
              let alternate = if branch { &p.path } else { &p.alternate_path };
              for (a0, b0) in junction::visible(edge_a, edge_b, neighbors, alternate) {
                let a1 = std::array::from_fn(|j| a0[j] + w[0].normal[j] * wall / 1_000_000);
                let b1 = std::array::from_fn(|j| b0[j] + w[1].normal[j] * wall / 1_000_000);
                b.quad(
                    [a0, a1, b1, b0],
                    Surface::Concrete,
                    &format!("assembled-wall-{index}"),
                    false,
                )?;
                // One geometric sheet: renderers/colliders handle both
                // sides. A reversed duplicate makes every shared edge
                // non-manifold and defeats native CCD edge suppression.
                let min = std::array::from_fn(|j| {
                    [a0, b0, a1, b1].iter().map(|v| v[j]).min().unwrap() - 2
                });
                let max = std::array::from_fn(|j| {
                    [a0, b0, a1, b1].iter().map(|v| v[j]).max().unwrap() + 2
                });
                b.solid(
                    &format!("assembled-wall-{index}"),
                    SolidShape::Box { min, max },
                )?;
              }
            }
        }
    }
    Ok(())
}
pub(crate) fn cost(a: &Assembly, bounds: &Bounds) -> (u64, u64) {
    let segments = a
        .pieces
        .iter()
        .filter(|p| {
            (0..2).all(|j| {
                p.reserved_min_cm[j * 2] <= bounds.max[j]
                    && p.reserved_max_cm[j * 2] >= bounds.min[j]
            })
        })
        .map(|p| {
            if p.id == "finish_plaza" {
                128
            } else {
                p.path.len() as u64 - 1 + p.alternate_path.len().saturating_sub(1) as u64
            }
        })
        .sum::<u64>();
    let supports = a
        .supports
        .iter()
        .filter(|s| {
            (0..2).all(|j| {
                s.shape.bounds().min[j] <= bounds.max[j] && s.shape.bounds().max[j] >= bounds.min[j]
            })
        })
        .count() as u64;
    (
        segments * 200 + 10 + supports * 60,
        segments * 3 + 1 + supports,
    )
}

fn generate_plaza(p: &Piece, index: usize, b: &mut impl TrackGeometry) -> Result<()> {
    let center = plaza_center(p);
    let half = i64::from(p.width_cm) / 2;
    let transform = |v| add(p.origin_cm, geometry::rotate3(v, p.rotation_mdeg));
    let road = format!("assembled-road-{index}");
    let wall = format!("assembled-wall-{index}");
    let entry = [
        transform([-half, 0, 0]),
        transform([-half, 0, FINISH_ENTRY_CM]),
        transform([half, 0, FINISH_ENTRY_CM]),
        transform([half, 0, 0]),
    ];
    b.quad(entry, Surface::Asphalt, &road, true)?;
    slab_shell(entry, &format!("assembled-shell-{index}"), b)?;
    let mut boundary = vec![];
    let opening = libm::asin(half as f64 / FINISH_RADIUS_CM as f64);
    for i in 0..=64 {
        let angle = -std::f64::consts::PI
            + opening
            + (2.0 * std::f64::consts::PI - 2.0 * opening) * i as f64 / 64.0;
        boundary.push(add(
            center,
            geometry::rotate3(
                [
                    round(FINISH_RADIUS_CM as f64 * libm::sin(angle)),
                    0,
                    round(FINISH_RADIUS_CM as f64 * libm::cos(angle)),
                ],
                p.rotation_mdeg,
            ),
        ));
    }
    // Exact seam endpoints prevent quantization cracks at the entrance chord.
    boundary[0] = entry[1];
    boundary[64] = entry[2];
    for i in 0..boundary.len() {
        let a = boundary[i];
        let c = boundary[(i + 1) % boundary.len()];
        b.triangle([center, a, c], Surface::Asphalt, &road, true)?;
        b.triangle(
            [lower(center), lower(c), lower(a)],
            Surface::Concrete,
            &format!("assembled-shell-{index}"),
            false,
        )?;
        b.quad(
            [a, c, lower(c), lower(a)],
            Surface::Concrete,
            &format!("assembled-shell-{index}"),
            false,
        )?;
        let vertices = vec![center, a, c, lower(center), lower(a), lower(c)];
        b.solid(
            &road,
            SolidShape::Convex(CollisionConvex {
                vertices,
                faces: vec![
                    [0, 1, 2],
                    [5, 4, 3],
                    [0, 3, 4],
                    [0, 4, 1],
                    [1, 4, 5],
                    [1, 5, 2],
                    [2, 5, 3],
                    [2, 3, 0],
                ],
            }),
        )?;
    }
    b.solid(
        &road,
        SolidShape::Box {
            min: std::array::from_fn(|j| {
                entry.iter().map(|v| v[j]).min().unwrap() - if j == 1 { 10 } else { 0 }
            }),
            max: std::array::from_fn(|j| entry.iter().map(|v| v[j]).max().unwrap()),
        },
    )?;
    for (a, c) in boundary
        .windows(2)
        .map(|w| (w[0], w[1]))
        .chain([(entry[0], entry[1]), (entry[2], entry[3])])
    {
        let up = geometry::rotate3([0, FINISH_WALL_CM, 0], p.rotation_mdeg);
        b.quad(
            [a, add(a, up), add(c, up), c],
            Surface::Concrete,
            &wall,
            false,
        )?;
        b.solid(
            &wall,
            SolidShape::Box {
                min: std::array::from_fn(|j| {
                    [a, c, add(a, up), add(c, up)]
                        .iter()
                        .map(|v| v[j])
                        .min()
                        .unwrap()
                        - 2
                }),
                max: std::array::from_fn(|j| {
                    [a, c, add(a, up), add(c, up)]
                        .iter()
                        .map(|v| v[j])
                        .max()
                        .unwrap()
                        + 2
                }),
            },
        )?;
    }
    Ok(())
}

// Both execution and grounding use this tessellator; no second road model.
trait TrackGeometry {
    fn triangle(
        &mut self,
        v: [Vertex; 3],
        surface: Surface,
        id: &str,
        spawnable: bool,
    ) -> Result<()>;
    fn solid(&mut self, id: &str, shape: SolidShape) -> Result<()>;
    fn quad(&mut self, v: [Vertex; 4], surface: Surface, id: &str, spawnable: bool) -> Result<()> {
        self.triangle([v[0], v[1], v[2]], surface, id, spawnable)?;
        self.triangle([v[0], v[2], v[3]], surface, id, spawnable)
    }
}
impl TrackGeometry for crate::generation::Builder {
    fn triangle(
        &mut self,
        v: [Vertex; 3],
        surface: Surface,
        id: &str,
        spawnable: bool,
    ) -> Result<()> {
        self.triangle(v, surface, id, spawnable)
    }
    fn solid(&mut self, id: &str, shape: SolidShape) -> Result<()> {
        self.solid(id, shape)
    }
}
fn lower(mut v: Vertex) -> Vertex {
    v[1] -= 10;
    v
}
fn slab_shell(v: [Vertex; 4], id: &str, b: &mut impl TrackGeometry) -> Result<()> {
    b.quad(
        [lower(v[3]), lower(v[2]), lower(v[1]), lower(v[0])],
        Surface::Concrete,
        id,
        false,
    )?;
    slab_sides(v, id, b)
}
fn slab_sides(v: [Vertex; 4], id: &str, b: &mut impl TrackGeometry) -> Result<()> {
    // End caps at every sample would create internal collision seams; only sides.
    for [a, c] in [[v[0], v[1]], [v[2], v[3]]] {
        b.quad([a, c, lower(c), lower(a)], Surface::Concrete, id, false)?;
    }
    Ok(())
}
fn emit_shape(shape: &CollisionConvex, id: &str, b: &mut impl TrackGeometry) -> Result<()> {
    b.solid(id, SolidShape::Convex(shape.clone()))?;
    for face in &shape.faces {
        b.triangle(
            face.map(|i| shape.vertices[i as usize]),
            Surface::Concrete,
            id,
            false,
        )?;
    }
    Ok(())
}

/// Common public line sampling, independent of road instances.
pub fn grind_path(points: &[Vertex], width: u32) -> Vec<Sample> {
    let mut p=geometry::bezier(points,width,width,width);
    geometry::refine(&mut p);
    let mut out=vec![];
    for w in p.windows(2) {
        let steps=(distance(w[0].position_cm,w[1].position_cm).div_ceil(35)).max(1);
        for i in 0..steps {
            if out.len()>32_000 {return out;}
            let t=i as f64/steps as f64;
            let mut s=w[0].clone();
            s.position_cm=std::array::from_fn(|j|round(w[0].position_cm[j] as f64*(1.0-t)+w[1].position_cm[j] as f64*t));
            s.forward=unit(std::array::from_fn(|j|w[0].forward[j] as f64*(1.0-t)+w[1].forward[j] as f64*t));
            out.push(s);
        }
    }
    if let Some(last)=p.pop() {out.push(last);}
    out
}
