//! Seeded, bounded track assembly. This is the sole catalogue and geometry owner.
//! Coordinates are centimetres; normals/directions use signed millionths.
use crate::gimmick::{Effect, Gimmick, Motion, MotionKind};
use crate::special_track::{SpecialTrack, TrackKind};
use crate::*;

pub const WIDTH: i64 = 600;
pub const WALL: i64 = 120;
pub const TILE_CM: i64 = 800;
const SLOT: i64 = TILE_CM * 4;
const TURN: i64 = TILE_CM * 2;
const LOOP_RADIUS: u32 = 250;
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
    pub minutes: u8,
    pub difficulty: String,
    pub gimmicks: Vec<String>,
    pub time_minutes: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: 1,
            circuit: true,
            minutes: 3,
            difficulty: "normal".into(),
            gimmicks: catalogue_ids()
                .iter()
                .filter(|v| !["straight", "curve", "slope", "zigzag"].contains(v))
                .map(|v| (*v).into())
                .collect(),
            time_minutes: 720,
        }
    }
}
impl Settings {
    pub fn normalized(&self) -> Result<Self> {
        if self.seed > 9_007_199_254_740_991
            || ![1, 3, 5].contains(&self.minutes)
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
    pub origin_cm: Vertex,
    pub cube_span: u8,
    pub entry_speed_cmps: [u32; 2],
    pub connection_width_cm: u32,
    pub quarter_turns: u8,
    pub path: Vec<Sample>,
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
}
pub fn catalogue_ids() -> &'static [&'static str] {
    &[
        "straight",
        "curve",
        "slope",
        "zigzag",
        "cylinder",
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
        "catalogue_fingerprint":catalogue_fingerprint(),
        "pieces":catalogue_ids().iter().map(|id| local_piece(id)).collect::<Vec<_>>()})
}
pub fn catalogue_fingerprint() -> String {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        sha256(
            &canonical(
                &catalogue_ids()
                    .iter()
                    .map(|id| local_piece(id))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        )
    })
    .clone()
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
    let n = (distance(a, b) / 150).max(1) as i64;
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
fn build_local_piece(id: &str) -> Piece {
    let mut p: Vec<(Vertex, Vertex, String)> = vec![];
    let mut minimum = 0;
    match id {
        "curve" => {
            for i in 0..=32 {
                let t = i as f64 / 32.0 * std::f64::consts::FRAC_PI_2;
                p.push((
                    [
                        round(TURN as f64 * (1.0 - libm::cos(t))),
                        0,
                        round(TURN as f64 * libm::sin(t)),
                    ],
                    [0, 1_000_000, 0],
                    "drive".into(),
                ));
            }
        }
        "loop" => {
            minimum = 1500;
            connector(&mut p, [0, 0, 0], [-LOOP_OFFSET, 0, 1000], "boost");
            p.last_mut().unwrap().2 = "loop".into();
            let mesh = SpecialTrack {
                kind: TrackKind::Loop,
                radius_cm: LOOP_RADIUS,
                width_cm: LOOP_WIDTH,
                length_cm: 1600,
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
            for i in 1..=128 {
                let t = i as f64 / 128.0 * std::f64::consts::TAU;
                p.push((
                    [
                        -round(800.0 * libm::cos(t)),
                        if down {
                            -i * TILE_CM / 128
                        } else {
                            i * TILE_CM / 128
                        },
                        1600 + round(800.0 * libm::sin(t)),
                    ],
                    [0, 1_000_000, 0],
                    "spiral".into(),
                ));
            }
            let end = p.last().unwrap().0;
            connector(&mut p, end, [0, end[1], SLOT], "drive");
        }
        "cylinder" => {
            line(&mut p, [0, 0, 0], [0, 0, 800], "drive");
            line(&mut p, [0, 0, 800], [0, 0, 2400], "cylinder");
            line(&mut p, [0, 0, 2400], [0, 0, SLOT], "drive");
        }
        "slope" => {
            line(&mut p, [0, 0, 0], [0, 0, 400], "drive");
            line(&mut p, [0, 0, 400], [0, 250, 1600], "drive");
            line(&mut p, [0, 250, 1600], [0, 0, 2800], "drive");
            line(&mut p, [0, 0, 2800], [0, 0, SLOT], "drive");
        }
        "zigzag" => {
            for i in 0..=96 {
                let t = i as f64 / 96.0;
                p.push((
                    [
                        round(
                            250.0
                                * libm::sin(t * std::f64::consts::TAU)
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
        "jump" | "air_ring" => {
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
    let small = ["straight", "slope", "zigzag"].contains(&id);
    if small {
        for (v, _, _) in &mut p {
            v[2] /= 4;
            v[0] /= 4;
            v[1] /= 4;
        }
    }
    let len = p.windows(2).map(|v| distance(v[0].0, v[1].0)).sum::<u64>();
    let path = p
        .iter()
        .enumerate()
        .map(|(i, (pos, normal, mode))| {
            let a = p[i.saturating_sub(1)].0;
            let b = p[(i + 1).min(p.len() - 1)].0;
            let normal = if mode == "loop" {
                *normal
            } else {
                let f: [f64; 3] = std::array::from_fn(|j| (b[j] - a[j]) as f64);
                unit([-f[0] * f[1], f[0] * f[0] + f[2] * f[2], -f[2] * f[1]])
            };
            let mut mode = mode.clone();
            if id == "cylinder" && (800..=2400).contains(&pos[2]) {
                mode = "cylinder".into();
            }
            if id.contains("obstacle") && (900..=2200).contains(&pos[2]) {
                mode = "avoid".into();
            }
            if ["boost_chain", "acceleration_panel"].contains(&id) {
                mode = "boost".into();
            }
            let forward = if i == 0 {
                [0, 0, 1_000_000]
            } else if i + 1 == p.len() {
                if id == "curve" {
                    [1_000_000, 0, 0]
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
                safe: mode == "drive"
                    && !["loop", "jump", "air_ring", "spiral_up", "spiral_down"].contains(&id),
                min_speed_cmps: minimum,
                tube_radius_cm: if mode == "cylinder" { 600 } else { 0 },
                lateral_cm: if mode == "cylinder" {
                    700
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
                    300
                },
                below_cm: if mode == "cylinder" { 50 } else { 150 },
                above_cm: if mode == "cylinder" {
                    1350
                } else if mode == "flight" {
                    700
                } else {
                    300
                },
                mode,
            }
        })
        .collect::<Vec<_>>();
    let lo = std::array::from_fn(|a| {
        path.iter().map(|s| s.position_cm[a]).min().unwrap() - if a == 1 { 200 } else { 320 }
    });
    let hi = std::array::from_fn(|a| {
        path.iter().map(|s| s.position_cm[a]).max().unwrap() + if a == 1 { 1600 } else { 320 }
    });
    Piece {
        id: id.into(),
        cube_span: if small { 1 } else { 4 },
        entry_speed_cmps: [minimum, 3000],
        connection_width_cm: WIDTH as u32,
        origin_cm: [0; 3],
        quarter_turns: 0,
        path,
        reserved_min_cm: lo,
        reserved_max_cm: hi,
        reference_msec: (len * 1000 / SPEED as u64) as u32,
    }
}
fn placed(id: &str, origin: Vertex, q: u8) -> Piece {
    let mut p = local_piece(id);
    p.origin_cm = origin;
    p.quarter_turns = q;
    for s in &mut p.path {
        s.position_cm = add(rotate(s.position_cm, q), origin);
        s.normal = rotate(s.normal, q);
        s.forward = rotate(s.forward, q);
    }
    let a = add(rotate(p.reserved_min_cm, q), origin);
    let b = add(rotate(p.reserved_max_cm, q), origin);
    p.reserved_min_cm = std::array::from_fn(|i| a[i].min(b[i]));
    p.reserved_max_cm = std::array::from_fn(|i| a[i].max(b[i]));
    p
}
fn next(rng: &mut u64) -> u64 {
    *rng = rng.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *rng;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn skeleton(settings: &Settings, count: usize) -> Assembly {
    let mut rng = settings.seed;
    let mut pieces = vec![];
    let mut origin = [0; 3];
    let mut return_spiral = "";
    let mut return_at = usize::MAX;
    let spacing = match settings.difficulty.as_str() {
        "easy" => 6,
        "hard" => 2,
        _ => 4,
    };
    let candidates = settings
        .gimmicks
        .iter()
        .filter(|s| s.as_str() != "curve" && s.as_str() != "straight")
        .collect::<Vec<_>>();
    for side in 0..if settings.circuit { 4 } else { 1 } {
        for index in 0..count {
            // A complete stable slot separates difficult pieces, including the grid.
            let mut id = if index % spacing == 1 && !candidates.is_empty() {
                candidates[next(&mut rng) as usize % candidates.len()].as_str()
            } else if (side * count + index) % 4 == 3 {
                if next(&mut rng) % 2 == 0 {
                    "slope"
                } else {
                    "zigzag"
                }
            } else {
                "straight"
            };
            if !return_spiral.is_empty() && index == return_at {
                id = return_spiral;
                return_spiral = "";
            } else if id.starts_with("spiral") {
                if index + 2 >= count {
                    id = "zigzag";
                } else {
                    return_spiral = if id == "spiral_up" {
                        "spiral_down"
                    } else {
                        "spiral_up"
                    };
                    return_at = index + 2;
                }
            }
            if pieces.iter().rev().take(4).count() == 4
                && pieces.iter().rev().take(4).all(|p: &Piece| p.id == id)
            {
                id = if id == "zigzag" { "slope" } else { "zigzag" };
            }
            let small = ["straight", "slope", "zigzag"].contains(&id);
            for tile in 0..if small { 4 } else { 1 } {
                let mut selected = id;
                if small && tile == 3 {
                    selected = if next(&mut rng) % 2 == 0 {
                        "slope"
                    } else {
                        "zigzag"
                    };
                }
                if pieces.iter().rev().take(4).count() == 4
                    && pieces
                        .iter()
                        .rev()
                        .take(4)
                        .all(|p: &Piece| p.id == selected)
                {
                    selected = if selected == "zigzag" {
                        "slope"
                    } else {
                        "zigzag"
                    };
                }
                let p = placed(selected, origin, side as u8);
                origin = p.path.last().unwrap().position_cm;
                pieces.push(p);
            }
        }
        if settings.circuit {
            let p = placed("curve", origin, side as u8);
            origin = p.path.last().unwrap().position_cm;
            pieces.push(p);
        }
    }
    let length_cm = pieces
        .iter()
        .flat_map(|p| p.path.windows(2))
        .map(|p| distance(p[0].position_cm, p[1].position_cm))
        .sum();
    let estimated_msec = pieces.iter().map(|p| p.reference_msec).sum();
    Assembly {
        settings: settings.clone(),
        generator_fingerprint: fingerprint(),
        catalogue_fingerprint: catalogue_fingerprint(),
        pieces,
        length_cm,
        estimated_msec,
    }
}
pub fn assemble(settings: &Settings) -> Result<Assembly> {
    let s = settings.normalized()?;
    let target = u32::from(s.minutes) * 60_000;
    let max = if s.circuit { 28 } else { 110 };
    let mut best: Option<Assembly> = None;
    for n in 1..=max {
        cancellation::checkpoint()?;
        let a = skeleton(&s, n);
        if best
            .as_ref()
            .is_none_or(|b| a.estimated_msec.abs_diff(target) < b.estimated_msec.abs_diff(target))
        {
            best = Some(a);
        }
    }
    let a = best.unwrap();
    a.validate()?;
    Ok(a)
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
            || self.pieces.iter().map(|p| p.path.len()).sum::<usize>() > MAX_SAMPLES
            || self.generator_fingerprint != fingerprint()
            || self.catalogue_fingerprint != catalogue_fingerprint()
        {
            return Err(fail());
        }

        if self.estimated_msec == 0 || self.estimated_msec > 600_000 {
            return Err(fail());
        }
        // Circuit wrap is part of the same run; no five identical pieces.
        for i in 0..self.pieces.len() {
            if !self.settings.circuit && i + 4 >= self.pieces.len() {
                break;
            }
            if (1..5).all(|n| self.pieces[(i + n) % self.pieces.len()].id == self.pieces[i].id) {
                return Err(fail());
            }
        }
        let mut length = 0;
        let mut time = 0;
        for (i, p) in self.pieces.iter().enumerate() {
            cancellation::checkpoint()?;
            if !catalogue_ids().contains(&p.id.as_str())
                || p.quarter_turns > 3
                || p.path.len() < 2
                || *p != placed(&p.id, p.origin_cm, p.quarter_turns)
            {
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
                    p.reserved_min_cm[a] + 350 < other.reserved_max_cm[a] - 350
                        && other.reserved_min_cm[a] + 350 < p.reserved_max_cm[a] - 350
                }) {
                    return Err(fail());
                }
            }
        }
        if length != self.length_cm
            || time != self.estimated_msec
            || (self.settings.circuit
                && self.pieces[0].path[0].position_cm
                    != self.pieces.last().unwrap().path.last().unwrap().position_cm)
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
            "cylinder" => vec![("cylinder", 1600)],
            "jump" => vec![("acceleration_panel", 350), ("jump", 950)],
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
                position: add(p.origin_cm, rotate([0, 0, *z], p.quarter_turns)),
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
                "loop" | "cylinder" => {
                    g.track = Some(SpecialTrack {
                        kind: if *id == "loop" {
                            TrackKind::Loop
                        } else {
                            TrackKind::Cylinder
                        },
                        radius_cm: if *id == "loop" { LOOP_RADIUS } else { 600 },
                        width_cm: if *id == "loop" { LOOP_WIDTH } else { 600 },
                        length_cm: 1600,
                    })
                }
                "fixed_obstacle" | "moving_obstacle" | "rotating_obstacle" => {
                    // Occupy one lane, leaving at least four metres for avoidance.
                    g.position = add(g.position, rotate([200, 0, 0], p.quarter_turns));
                    g.parts.push(box_part([0, 50, 0], [100, 100, 150]));
                    if *id == "moving_obstacle" {
                        g.motion.kind = MotionKind::Translate;
                        g.motion.delta_cm = rotate([-100, 0, 0], p.quarter_turns);
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
                    g.parts.push(box_part([0, -2, 0], [550, 4, 200]));
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
    for (index, p) in a.pieces.iter().enumerate() {
        for w in p.path.windows(2) {
            cancellation::checkpoint()?;
            let special = w.iter().any(|s| s.mode == "flight")
                || w.iter()
                    .all(|s| ["loop", "cylinder"].contains(&s.mode.as_str()));
            let edges = |s: &Sample| {
                let n = s.normal.map(|v| v as f64 / 1e6);
                let f = s.forward.map(|v| v as f64 / 1e6);
                let right = if s.mode == "loop" {
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
                        s.position_cm[j]
                            + right[j] * i64::from(s.lateral_cm.min(WIDTH as u32 / 2)) * side
                                / 1_000_000
                    })
                })
            };
            let [al, ar] = edges(&w[0]);
            let [bl, br] = edges(&w[1]);
            let id = format!("assembled-road-{index}");
            if !special {
                b.quad([al, bl, br, ar], Surface::Asphalt, &id, true)?;
                let min = std::array::from_fn(|j| {
                    [al, ar, bl, br].iter().map(|v| v[j]).min().unwrap()
                        - if j == 1 { 10 } else { 0 }
                });
                let max =
                    std::array::from_fn(|j| [al, ar, bl, br].iter().map(|v| v[j]).max().unwrap());
                b.solid(&id, SolidShape::Box { min, max })?;
            }
            if w.iter()
                .any(|s| ["cylinder", "flight"].contains(&s.mode.as_str()))
            {
                continue;
            }
            for (a0, b0) in [(al, bl), (ar, br)] {
                let a1 = std::array::from_fn(|j| a0[j] + w[0].normal[j] * WALL / 1_000_000);
                let b1 = std::array::from_fn(|j| b0[j] + w[1].normal[j] * WALL / 1_000_000);
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
        .map(|p| p.path.len() as u64 - 1)
        .sum::<u64>();
    (segments * 50, segments * 3)
}
