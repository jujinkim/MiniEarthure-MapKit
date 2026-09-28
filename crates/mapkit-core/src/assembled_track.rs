//! Seeded, bounded track assembly. This is the sole catalogue and geometry owner.
//! Coordinates are centimetres; normals/directions use signed millionths.
use crate::gimmick::{Effect, Gimmick, Motion, MotionKind};
use crate::special_track::{SpecialTrack, TrackKind, TubeFrame};
use crate::*;

pub const WIDTH: i64 = 400;
pub const WALL: i64 = 60;
pub const TILE_CM: i64 = 800;
const SLOT: i64 = TILE_CM * 4;
const LOOP_RADIUS: u32 = 350;
const LOOP_WIDTH: u32 = 220;
const LOOP_OFFSET: i64 = 143;
const SPEED: i64 = 900;
const MAX_PIECES: usize = 512;
const MAX_SAMPLES: usize = 32_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub seed: u64,
    pub circuit: bool,
    pub duration_seconds: u16,
    pub difficulty: String,
    pub gimmicks: Vec<String>,
    pub time_minutes: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: 1,
            circuit: true,
            duration_seconds: 60,
            difficulty: "normal".into(),
            gimmicks: catalogue_ids()
                .iter()
                .filter(|v| {
                    !basic_ids().contains(v) && (!v.starts_with("cylinder_") || **v == "cylinder")
                })
                .map(|v| (*v).into())
                .collect(),
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
            || self.gimmicks.len() > catalogue_ids().len()
            || self
                .gimmicks
                .iter()
                .any(|s| !catalogue_ids().contains(&s.as_str()))
        {
            return Err(error(
                "E_TRACK_SETTINGS",
                "invalid seed, duration, difficulty, time or piece selection",
            ));
        }
        let mut s = self.clone();
        s.gimmicks.sort();
        s.gimmicks.dedup();
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
        &[(30, 3), (60, 3), (120, 2)]
    } else {
        &[(60, 1), (120, 1), (180, 1)]
    }
}
fn basic_ids() -> &'static [&'static str] {
    &[
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
        "zigzag",
        "chicane",
        "straight_narrow",
        "chicane_narrow",
        "zigzag_narrow",
    ]
}
fn short_piece(id: &str) -> bool {
    [
        "fixed_obstacle",
        "moving_obstacle",
        "rotating_obstacle",
        "acceleration_panel",
        "boost_chain",
    ]
    .contains(&id)
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
    pub settings: Settings,
    pub generator_fingerprint: String,
    pub catalogue_fingerprint: String,
    pub pieces: Vec<Piece>,
    pub length_cm: u64,
    pub estimated_msec: u32,
    pub straight_target_percent: u32,
    pub ordinary_length_cm: u64,
    pub ordinary_straight_cm: u64,
    pub floor: VenueFloor,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VenueFloor {
    pub min_cm: Vertex,
    pub max_cm: Vertex,
}
pub fn catalogue_ids() -> &'static [&'static str] {
    &[
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
        "jump_barrier",
        "overpass",
        "roller_waves",
        "offset_jump",
        "slalom_gates",
        "swing_gates",
        "piston_gates",
        "sprint_lane",
        "loop",
        "spiral_up",
        "spiral_down",
        "jump",
        "fixed_obstacle",
        "moving_obstacle",
        "rotating_obstacle",
        "acceleration_panel",
        "boost_chain",
        "air_ring",
    ]
}
pub fn fingerprint() -> String {
    sha256(
        &[
            include_bytes!("assembled_track.rs").as_slice(),
            include_bytes!("special_track.rs").as_slice(),
        ]
        .concat(),
    )
}
pub fn catalogue() -> serde_json::Value {
    serde_json::json!({"format_version":1,"width_cm":WIDTH,"wall_height_cm":WALL,
        "tile_size_cm":TILE_CM,"defaults":Settings::default(),"generator_fingerprint":fingerprint(),
        "basic_piece_ids":basic_ids(),"widths_cm":[600,400,200],"width_weights":[2,2,1],
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
        "slalom_gates" | "swing_gates" | "piston_gates" => {
            for i in 0..=128 {
                let t = i as f64 / 128.0;
                p.push((
                    [
                        round(
                            -105.0
                                * libm::sin(
                                    t * std::f64::consts::PI * 4.0 - std::f64::consts::FRAC_PI_2,
                                )
                                * libm::sin(t * std::f64::consts::PI).powi(2),
                        ),
                        0,
                        round(t * SLOT as f64),
                    ],
                    [0, 1_000_000, 0],
                    "drive".into(),
                ));
            }
        }
        "jump_barrier" => {
            for i in 0..=64 {
                p.push((
                    [0, 0, i * 50],
                    [0, 1_000_000, 0],
                    if i == 23 { "jump_trigger" } else { "hurdle" }.into(),
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
            if id.contains("obstacle") && (450..=1100).contains(&pos[2]) {
                mode = "avoid".into();
            }
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
                    || (id.contains("curve") && !id.ends_with("_left"))
                {
                    [1_000_000, 0, 0]
                } else if id == "curve_left"
                    || id == "sharp_curve_left"
                    || (id.contains("curve") && id.ends_with("_left"))
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
                    && ![
                        "banked_chicane",
                        "jump_barrier",
                        "roller_waves",
                        "offset_jump",
                        "swing_gates",
                        "piston_gates",
                    ]
                    .contains(&id)
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
        } else if short_piece(id) {
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
fn width(rng: &mut u64) -> u32 {
    [600, 600, 400, 400, 200][next(rng) as usize % 5]
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
fn variant(id: &str, width: u32, entry: u32, exit: u32) -> Piece {
    let mut p = local_piece(id);
    p.width_cm = width;
    p.entry_width_cm = entry;
    p.exit_width_cm = exit;
    p.connection_width_cm = entry;
    if ["tube_entry", "tube_exit"].contains(&id) {
        let tiles = ramp_tiles(width);
        let length = tiles as i64 * TILE_CM;
        let height = round(f64::from(width) / 3.0);
        let down = id == "tube_exit";
        p.path = (0..=tiles * 32)
            .map(|i| {
                let t = i as f64 / (tiles * 32) as f64;
                let mut sample = p.path[0].clone();
                let sign = if down { -1.0 } else { 1.0 };
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
        let total = p
            .path
            .windows(2)
            .map(|w| distance(w[0].position_cm, w[1].position_cm))
            .sum::<u64>() as f64;
        let mut station = 0.0;
        for i in 0..p.path.len() {
            if i > 0 {
                station += distance(p.path[i - 1].position_cm, p.path[i].position_cm) as f64;
            }
            if id == "loop" || id == "banked_chicane" || id == "overpass" {
                continue;
            }
            let ease = |t: f64| {
                let t = t.clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            };
            let w = width as f64
                + (entry as f64 - width as f64) * (1.0 - ease(station / 300.0))
                + (exit as f64 - width as f64) * (1.0 - ease((total - station) / 300.0));
            p.path[i].lateral_cm = round(w / 2.0) as u32;
        }
    }
    // Dedicated structures keep their own dimensions; their outer ports are 4m.
    p.reference_msec = (p
        .path
        .windows(2)
        .map(|w| distance(w[0].position_cm, w[1].position_cm))
        .sum::<u64>()
        * 1000
        / SPEED as u64) as u32;
    p
}
fn materialize(p: &Piece) -> Piece {
    let mut out = variant(&p.id, p.width_cm, p.entry_width_cm, p.exit_width_cm);
    out.origin_cm = p.origin_cm;
    out.quarter_turns = p.quarter_turns;
    out.chain_id = p.chain_id;
    out.chain_index = p.chain_index;
    out.chain_count = p.chain_count;
    out.ordinary = p.ordinary;
    for v in out.path.iter_mut().chain(&mut out.alternate_path) {
        v.position_cm = add(rotate(v.position_cm, p.quarter_turns), p.origin_cm);
        v.forward = rotate(v.forward, p.quarter_turns);
        v.normal = rotate(v.normal, p.quarter_turns);
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
    let mut p = variant(id, w, w, w);
    p.origin_cm = *origin;
    p.quarter_turns = heading;
    p.ordinary = ordinary;
    p.chain_id = chain[0];
    p.chain_index = chain[1];
    p.chain_count = chain[2];
    p = materialize(&p);
    *origin = p.path.last().unwrap().position_cm;
    pieces.push(p);
}
#[derive(Clone)]
struct Block {
    id: String,
    count: usize,
    width: u32,
    balance: bool,
}
impl Block {
    fn tiles(&self) -> usize {
        let unit = if short_piece(&self.id) { 2 } else { 4 };
        self.count * unit * (if self.balance { 2 } else { 1 })
            + if self.id == "cylinder" {
                2 * ramp_tiles(self.width)
            } else {
                2
            }
    }
}
fn required(s: &Settings) -> Result<Vec<Block>> {
    let mut rng = s.seed ^ 0x814d229a;
    let mut ids: Vec<_> = s
        .gimmicks
        .iter()
        .filter(|id| !basic_ids().contains(&id.as_str()))
        .map(|s| family(s).to_string())
        .collect();
    ids.sort();
    ids.dedup();
    let mut blocks = vec![];
    for id in ids {
        let mut count = 1;
        while next(&mut rng) % 2 == 0 {
            count += 1;
            if count > MAX_PIECES {
                return Err(error(
                    "E_TRACK_BUDGET",
                    "required special chain exceeds piece budget",
                ));
            }
        }
        let w = if ["loop", "banked_chicane", "overpass"].contains(&id.as_str()) {
            400
        } else {
            width(&mut rng)
        };
        blocks.push(Block {
            balance: id.starts_with("spiral"),
            id,
            count,
            width: w,
        });
    }
    for i in (1..blocks.len()).rev() {
        let j = next(&mut rng) as usize % (i + 1);
        blocks.swap(i, j);
    }
    Ok(blocks)
}
fn statistics(pieces: &[Piece]) -> (u64, u64, u64, u32, VenueFloor) {
    let (mut length, mut ordinary, mut straight, mut time) = (0, 0, 0, 0);
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for p in pieces {
        time += p.reference_msec;
        for v in p.path.iter().chain(&p.alternate_path) {
            for j in 0..3 {
                lo[j] = lo[j].min(v.position_cm[j]);
                hi[j] = hi[j].max(v.position_cm[j]);
            }
        }
        for w in p.path.windows(2) {
            let d = distance(w[0].position_cm, w[1].position_cm);
            length += d;
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
    let floor_y = lo[1] - 235; // Always below the lowest tube bottom, including 6m bores.
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
fn candidate(
    s: &Settings,
    blocks: &[Block],
    attempt: u64,
    target_percent: u32,
) -> Result<Assembly> {
    let mut rng = s.seed ^ attempt.wrapping_mul(0xa0761d6478bd642f);
    let mut sides: Vec<Vec<&Block>> = vec![vec![]; 4];
    for block in blocks {
        let side = next(&mut rng) as usize % 4;
        sides[side].push(block);
    }
    let needs: Vec<usize> = sides
        .iter()
        .enumerate()
        .map(|(i, b)| b.iter().map(|v| v.tiles()).sum::<usize>() + if i == 0 { 3 } else { 0 })
        .collect();
    let target_tiles = (u32::from(s.duration_seconds) * SPEED as u32 / TILE_CM as u32) as usize;
    let extra = (target_tiles.saturating_sub(needs.iter().sum::<usize>()) / 4).max(4);
    let stretch = (attempt as usize % 8) * extra / 4;
    let a = needs[0].max(needs[2]).max(6) + stretch;
    let b = needs[1].max(needs[3]).max(6) + (7 - attempt as usize % 8) * extra / 4;
    let runs = [a, b, a, b];
    let mut pieces = vec![];
    let mut origin = [0; 3];
    let mut heading = 0;
    let mut chain = 0;
    for side in 0..if s.circuit { 4 } else { 3 } {
        // For a sprint, reserve every selected block on the three traversed sides.
        let mut list = sides[side].clone();
        if !s.circuit && side == 2 {
            list.extend(sides[3].iter());
        }
        let need = list.iter().map(|b| b.tiles()).sum::<usize>() + if side == 0 { 3 } else { 0 };
        let mut remaining = runs[side].max(need) - need;
        if side == 0 {
            for _ in 0..3 {
                push_piece(
                    &mut pieces,
                    &mut origin,
                    heading,
                    "straight",
                    400,
                    false,
                    [0; 3],
                );
            }
        }
        let filler =
            |remaining: &mut usize, pieces: &mut Vec<Piece>, origin: &mut Vertex, rng: &mut u64| {
                let chance = f64::from(target_percent) * 2512.0
                    / (f64::from(target_percent) * 2512.0 + (100 - target_percent) as f64 * 800.0);
                let curved = *remaining >= 2
                    && !pieces.iter().rev().take(4).all(|p| p.id == "chicane")
                    && next(rng) % 10000 >= (chance * 10000.0) as u64;
                let id = if curved {
                    "chicane"
                } else if pieces.iter().rev().take(4).all(|p| p.id == "straight")
                    || next(rng) % 5 == 0
                {
                    "slope"
                } else {
                    "straight"
                };
                push_piece(pieces, origin, heading, id, width(rng), true, [0; 3]);
                *remaining -= if curved { 2 } else { 1 };
            };
        for block in list {
            let before = if remaining == 0 {
                0
            } else {
                next(&mut rng) as usize % (remaining + 1)
            };
            let stop = remaining - before;
            while remaining > stop {
                filler(&mut remaining, &mut pieces, &mut origin, &mut rng);
            }
            chain += 1;
            let tube = block.id == "cylinder";
            push_piece(
                &mut pieces,
                &mut origin,
                heading,
                if tube { "tube_entry" } else { "straight" },
                block.width,
                false,
                [0; 3],
            );
            for i in 0..block.count {
                push_piece(
                    &mut pieces,
                    &mut origin,
                    heading,
                    &block.id,
                    block.width,
                    false,
                    [chain, i as u32, block.count as u32],
                );
            }
            if block.balance {
                chain += 1;
                let opposite = if block.id == "spiral_up" {
                    "spiral_down"
                } else {
                    "spiral_up"
                };
                for i in 0..block.count {
                    push_piece(
                        &mut pieces,
                        &mut origin,
                        heading,
                        opposite,
                        block.width,
                        false,
                        [chain, i as u32, block.count as u32],
                    );
                }
            }
            push_piece(
                &mut pieces,
                &mut origin,
                heading,
                if tube { "tube_exit" } else { "straight" },
                block.width,
                false,
                [0; 3],
            );
        }
        while remaining > 0 {
            filler(&mut remaining, &mut pieces, &mut origin, &mut rng);
        }
        if s.circuit || side < 2 {
            push_piece(
                &mut pieces,
                &mut origin,
                heading,
                "sharp_curve",
                width(&mut rng),
                true,
                [0; 3],
            );
            heading = (heading + 1) % 4;
        }
        if pieces.len() > MAX_PIECES {
            return Err(error(
                "E_TRACK_BUDGET",
                "mandatory pieces and connecting roads exceed budget",
            ));
        }
    }
    // Match shared cross sections, with transitions entirely inside each road.
    for i in 0..pieces.len() {
        let prev = if i == 0 {
            if s.circuit {
                pieces.len() - 1
            } else {
                0
            }
        } else {
            i - 1
        };
        let next = (i + 1).min(pieces.len() - 1);
        let next = if i + 1 == pieces.len() && s.circuit {
            0
        } else {
            next
        };
        let joint = |a: &Piece, b: &Piece| {
            if a.id.starts_with("cylinder") || b.id.starts_with("cylinder") {
                a.width_cm.min(b.width_cm)
            } else if ["loop", "banked_chicane", "overpass"].contains(&a.id.as_str())
                || ["loop", "banked_chicane", "overpass"].contains(&b.id.as_str())
            {
                400
            } else {
                (a.width_cm + b.width_cm) / 2
            }
        };
        pieces[i].entry_width_cm = joint(&pieces[prev], &pieces[i]);
        pieces[i].exit_width_cm = joint(&pieces[i], &pieces[next]);
    }
    pieces = pieces.iter().map(materialize).collect();
    let (length_cm, ordinary_length_cm, ordinary_straight_cm, estimated_msec, floor) =
        statistics(&pieces);
    Ok(Assembly {
        settings: s.clone(),
        generator_fingerprint: fingerprint(),
        catalogue_fingerprint: catalogue_fingerprint(),
        pieces,
        length_cm,
        estimated_msec,
        straight_target_percent: target_percent,
        ordinary_length_cm,
        ordinary_straight_cm,
        floor,
    })
}
pub fn assemble(settings: &Settings) -> Result<Assembly> {
    let s = settings.normalized()?;
    let blocks = required(&s)?;
    let mut rng = s.seed ^ 0x51eed;
    let ratio = 10 + (next(&mut rng) % 81) as u32;
    let target = u32::from(s.duration_seconds) * 1000;
    let score = |a: &Assembly| {
        let actual = a.ordinary_straight_cm as f64 / a.ordinary_length_cm.max(1) as f64;
        (actual - f64::from(ratio) / 100.0).abs() * 150.0
            + f64::from(a.estimated_msec.abs_diff(target)) / 1000.0
    };
    let mut best = None;
    for attempt in 0..24 {
        cancellation::checkpoint()?;
        if let Ok(a) = candidate(&s, &blocks, attempt, ratio) {
            if a.validate().is_ok() && best.as_ref().is_none_or(|b| score(&a) < score(b)) {
                best = Some(a);
            }
        }
    }
    best.ok_or_else(||error("E_TRACK_BUDGET","no connected collision-free layout fits the search, cell and memory budgets with every required gimmick"))
}

impl Assembly {
    pub fn validate(&self) -> Result<()> {
        let fail = || {
            error(
                "E_TRACK_ASSEMBLY",
                "assembly exceeds time, connection, catalogue or resource constraints",
            )
        };
        if self.settings.normalized()? != self.settings
            || self.pieces.is_empty()
            || self.pieces.len() > MAX_PIECES
            || self
                .pieces
                .iter()
                .map(|p| p.path.len() + p.alternate_path.len())
                .sum::<usize>()
                > MAX_SAMPLES
            || self.generator_fingerprint != fingerprint()
            || self.catalogue_fingerprint != catalogue_fingerprint()
        {
            return Err(fail());
        }

        if self.estimated_msec == 0 {
            return Err(fail());
        }
        for id in self
            .settings
            .gimmicks
            .iter()
            .filter(|id| !basic_ids().contains(&id.as_str()))
        {
            if !self.pieces.iter().any(|p| family(&p.id) == family(id)) {
                return Err(fail());
            }
        }
        // Only ordinary roads retain the repeated-piece limit.
        for i in 0..self.pieces.len() {
            if !self.settings.circuit && i + 4 >= self.pieces.len() {
                break;
            }
            if self.pieces[i].ordinary
                && (1..5).all(|n| {
                    let p = &self.pieces[(i + n) % self.pieces.len()];
                    p.ordinary && p.id == self.pieces[i].id
                })
            {
                return Err(fail());
            }
        }
        let stats = statistics(&self.pieces);
        if (stats.0, stats.1, stats.2, stats.3, stats.4)
            != (
                self.length_cm,
                self.ordinary_length_cm,
                self.ordinary_straight_cm,
                self.estimated_msec,
                self.floor.clone(),
            )
        {
            return Err(fail());
        }
        let mut length = 0;
        let mut time = 0;
        for (i, p) in self.pieces.iter().enumerate() {
            cancellation::checkpoint()?;
            if !catalogue_ids().contains(&p.id.as_str())
                || p.quarter_turns > 3
                || p.path.len() < 2
                || ![200, 400, 600].contains(&p.width_cm)
                || *p != materialize(p)
            {
                return Err(fail());
            }
            if p.chain_count > 0 {
                if p.chain_id == 0 || p.chain_index >= p.chain_count || p.chain_index as usize > i {
                    return Err(fail());
                }
                let first = i - p.chain_index as usize;
                if first + p.chain_count as usize > self.pieces.len() {
                    return Err(fail());
                }
                for (offset, member) in self.pieces[first..first + p.chain_count as usize]
                    .iter()
                    .enumerate()
                {
                    if member.chain_id != p.chain_id
                        || member.chain_index != offset as u32
                        || member.chain_count != p.chain_count
                        || family(&member.id) != family(&p.id)
                        || member.width_cm != p.width_cm
                    {
                        return Err(fail());
                    }
                }
            } else if p.chain_id != 0 || p.chain_index != 0 {
                return Err(fail());
            }
            if i > 0 {
                let previous = self.pieces[i - 1].path.last().unwrap();
                if previous.position_cm != p.path[0].position_cm
                    || previous.forward != p.path[0].forward
                    || previous.normal != p.path[0].normal
                {
                    return Err(fail());
                }
            }
            length += p
                .path
                .windows(2)
                .map(|v| distance(v[0].position_cm, v[1].position_cm))
                .sum::<u64>();
            time += p.reference_msec;
            // Reserved slots may touch their immediate neighbours only. Test the
            // interior footprint so connection margins are not treated as overlap.
            for other in self.pieces.iter().take(i.saturating_sub(1)) {
                if i + 1 == self.pieces.len() && std::ptr::eq(other, &self.pieces[0]) {
                    continue;
                }
                if (0..3).all(|a| {
                    p.reserved_min_cm[a] < other.reserved_max_cm[a]
                        && other.reserved_min_cm[a] < p.reserved_max_cm[a]
                }) {
                    // The width-dependent exclusion includes lane,
                    // shell and discretization margin without filling a curved bay.
                    if p.path.iter().chain(&p.alternate_path).any(|s| {
                        other.path.iter().chain(&other.alternate_path).any(|t| {
                            (s.position_cm[1] - t.position_cm[1]).abs() < 250
                                && (s.position_cm[0] - t.position_cm[0]).pow(2)
                                    + (s.position_cm[2] - t.position_cm[2]).pow(2)
                                    < (i64::from(s.lateral_cm + t.lateral_cm) + 60).pow(2)
                        })
                    }) {
                        return Err(fail());
                    }
                }
            }
        }
        if length != self.length_cm
            || time != self.estimated_msec
            || (self.settings.circuit && {
                let start = &self.pieces[0].path[0];
                let end = self.pieces.last().unwrap().path.last().unwrap();
                start.position_cm != end.position_cm
                    || start.forward != end.forward
                    || start.normal != end.normal
            })
        {
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
fn gimmicks(a: &Assembly) -> Vec<Gimmick> {
    let mut out = vec![];
    for (index, p) in a.pieces.iter().enumerate() {
        let specs: Vec<(&str, i64)> = match p.id.as_str() {
            "loop" => vec![("acceleration_panel", 350), ("loop", 1000)],
            id if id.starts_with("cylinder") => vec![("cylinder", 0)],
            "banked_chicane" => vec![("halfpipe", 0)],
            "jump_barrier" => vec![("barrier", 1400)],
            "jump" | "offset_jump" => vec![("acceleration_panel", 350), ("jump", 950)],
            "slalom_gates" | "swing_gates" | "piston_gates" => vec![
                (p.id.as_str(), 800),
                (p.id.as_str(), 1600),
                (p.id.as_str(), 2400),
            ],
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
            "fixed_obstacle" | "moving_obstacle" | "rotating_obstacle" | "acceleration_panel" => {
                vec![(p.id.as_str(), 1600)]
            }
            _ => vec![],
        };
        for (n, (id, z)) in specs.iter().enumerate() {
            let mut g = Gimmick {
                id: format!("track-{index}-{n}"),
                position: add(
                    p.origin_cm,
                    rotate(
                        [0, 0, if short_piece(&p.id) { *z / 2 } else { *z }],
                        p.quarter_turns,
                    ),
                ),
                rotation_mdeg: [0, i32::from(p.quarter_turns) * 90_000, 0],
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
                                    floor_cm: if *id == "cylinder" {
                                        add(
                                            s.position_cm,
                                            [0, -round(f64::from(p.width_cm) / 3.0), 0],
                                        )
                                    } else {
                                        s.position_cm
                                    },
                                    normal: s.normal,
                                    forward: s.forward,
                                })
                                .collect()
                        },
                    })
                }
                "slalom_gates" | "swing_gates" | "piston_gates" => {
                    let side = if n % 2 == 0 { 1 } else { -1 };
                    g.position = add(g.position, rotate([side * 110, 0, 0], p.quarter_turns));
                    g.parts.push(box_part([0, 30, 0], [140, 60, 35]));
                    g.color = if *id == "piston_gates" {
                        [160, 85, 235, 255]
                    } else {
                        [245, 168, 35, 255]
                    };
                    if *id == "swing_gates" {
                        g.motion.kind = MotionKind::Rotate;
                        g.motion.period_ms = 3000;
                        g.motion.phase_ms = n as u32 * 700;
                    }
                    if *id == "piston_gates" {
                        g.motion.kind = MotionKind::Translate;
                        g.motion.delta_cm = [0, 180, 0];
                        g.motion.period_ms = 2400;
                        g.motion.phase_ms = n as u32 * 700;
                    }
                }
                "barrier" => {
                    g.parts
                        .push(box_part([0, 18, 0], [p.width_cm as i64, 36, 25]));
                    g.color = [245, 168, 35, 255];
                }
                "fixed_obstacle" | "moving_obstacle" | "rotating_obstacle" => {
                    // Keep an open avoidance lane on the narrower track.
                    g.position = add(g.position, rotate([5, 0, 0], p.quarter_turns));
                    g.parts.push(box_part([0, 50, 0], [40, 80, 60]));
                    if *id == "moving_obstacle" {
                        g.motion.kind = MotionKind::Translate;
                        g.motion.delta_cm = rotate([-20, 0, 0], p.quarter_turns);
                    }
                    if *id == "rotating_obstacle" {
                        g.motion.kind = MotionKind::Rotate;
                    }
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
pub fn document(settings: &Settings) -> Result<MapDocument> {
    let a = assemble(settings)?;
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
    let mut d = MapDocument {
        assembled_track: Some(a.clone()),
        water_bodies: vec![],
        gimmicks: gimmicks(&a),
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
    let expected = document(&a.settings)?;
    let mut actual = d.clone();
    actual.courses.clear();
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
    for (index, p) in a.pieces.iter().enumerate() {
        for (branch, path) in [(false, &p.path), (true, &p.alternate_path)] {
            for w in path.windows(2) {
                cancellation::checkpoint()?;
                let special = w.iter().any(|s| s.mode == "flight")
                    || w.iter()
                        .all(|s| ["loop", "cylinder", "halfpipe"].contains(&s.mode.as_str()));
                let edges = |s: &Sample| {
                    let n = s.normal.map(|v| v as f64 / 1e6);
                    let f = s.forward.map(|v| v as f64 / 1e6);
                    let right = if s.mode == "spiral" {
                        let center = add(p.origin_cm, rotate([0, 0, 1600], p.quarter_turns));
                        unit([
                            (center[0] - s.position_cm[0]) as f64,
                            0.0,
                            (center[2] - s.position_cm[2]) as f64,
                        ])
                    } else if s.mode == "loop" {
                        rotate([1_000_000, 0, 0], p.quarter_turns)
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
                    let strips = if p.id.starts_with("spiral") { 8 } else { 1 };
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
                    }
                    let min = std::array::from_fn(|j| {
                        [al, ar, bl, br].iter().map(|v| v[j]).min().unwrap()
                            - if j == 1 { 10 } else { 0 }
                    });
                    let max = std::array::from_fn(|j| {
                        [al, ar, bl, br].iter().map(|v| v[j]).max().unwrap()
                    });
                    b.solid(&id, SolidShape::Box { min, max })?;
                }
                if w.iter()
                    .any(|s| ["cylinder", "halfpipe", "flight"].contains(&s.mode.as_str()))
                {
                    continue;
                }
                let station = |s: &Sample| {
                    let f = rotate([0, 0, 1], p.quarter_turns);
                    (0..3)
                        .map(|j| (s.position_cm[j] - p.origin_cm[j]) * f[j])
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
                for (a0, b0) in [(al, bl), (ar, br)] {
                    let a1 = std::array::from_fn(|j| a0[j] + w[0].normal[j] * wall / 1_000_000);
                    let b1 = std::array::from_fn(|j| b0[j] + w[1].normal[j] * wall / 1_000_000);
                    b.quad(
                        [a0, a1, b1, b0],
                        Surface::Concrete,
                        &format!("assembled-wall-{index}"),
                        false,
                    )?;
                    b.quad(
                        [b0, b1, a1, a0],
                        Surface::Concrete,
                        &format!("assembled-wall-{index}"),
                        false,
                    )?;
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
        .map(|p| p.path.len() as u64 - 1 + p.alternate_path.len().saturating_sub(1) as u64)
        .sum::<u64>();
    (segments * 100 + 10, segments * 3 + 1)
}
