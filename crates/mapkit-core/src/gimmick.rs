//! Bounded declarative driving structures. There is no executable content.
use crate::*;
use std::collections::BTreeSet;

pub const MAX_GIMMICKS: usize = 128;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MotionKind { Static, Translate, Rotate, Boost, Launch }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub kind: MotionKind,
    pub delta_cm: Vertex,
    pub axis: u8,
    pub period_ms: u32,
    pub phase_ms: u32,
    pub impulse_cmps: Vertex,
    pub cooldown_ms: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gimmick {
    pub id: String,
    pub position: Vertex,
    pub rotation_mdeg: [i32; 3],
    pub scale_per_mille: [u32; 3],
    pub parts: Vec<CollisionConvex>,
    pub surface: Surface,
    pub color: [u8; 4],
    pub motion: Motion,
    /// Conservative full swept envelope AND the predicted pad landing area.
    pub safety_min_cm: Vertex,
    pub safety_max_cm: Vertex,
}
impl Gimmick {
    pub fn valid(&self) -> bool {
        if self.id.is_empty() || self.id.len() > 128 || self.parts.is_empty() || self.parts.len() > 32
            || self.rotation_mdeg.iter().any(|v| v.unsigned_abs() > 360_000)
            || self.scale_per_mille.iter().any(|v| !(100..=10_000).contains(v))
            || self.position.iter().any(|v| v.unsigned_abs() > 10_000_000)
            || self.parts.iter().any(|p| !p.valid(10_000))
            || self.motion.axis > 2 || !(250..=120_000).contains(&self.motion.period_ms)
            || self.motion.phase_ms >= self.motion.period_ms
            || self.motion.delta_cm.iter().any(|v| v.unsigned_abs() > 3200)
            || self.motion.impulse_cmps.iter().any(|v| v.unsigned_abs() > 5000)
            || !(100..=60_000).contains(&self.motion.cooldown_ms) { return false; }
        // Use an integer L1 sphere bound: encloses arbitrary Euler rotations and
        // continuous rotation, independent of platform trigonometry rounding.
        let radius = self.parts.iter().flat_map(|p| &p.vertices).map(|v|
            (0..3).map(|a| (v[a].unsigned_abs() * u64::from(self.scale_per_mille[a]) + 999) / 1000).sum::<u64>()
        ).max().unwrap() as i64;
        (0..3).all(|a| {
            let delta = if self.motion.kind == MotionKind::Translate { self.motion.delta_cm[a] } else { 0 };
            self.safety_min_cm[a] <= self.position[a] - radius + delta.min(0)
                && self.safety_max_cm[a] >= self.position[a] + radius + delta.max(0)
                && self.safety_min_cm[a].unsigned_abs() <= 10_000_000
                && self.safety_max_cm[a].unsigned_abs() <= 10_000_000
                && self.safety_max_cm[a] - self.safety_min_cm[a] <= 25_600
        })
    }
    pub fn intersects(&self, b: &Bounds) -> bool {
        (0..2).all(|a| self.safety_min_cm[a * 2] <= b.max[a] && self.safety_max_cm[a * 2] >= b.min[a])
    }
    pub fn memory_bytes(&self) -> u64 {
        // Source/JSON/Variant copies, physics proxies and procedural display meshes.
        16_384 + self.parts.len() as u64 * 32_768
    }
    /// Conservative authoring/grid occupancy. Keep compound parts separate so a
    /// hollow passage never becomes one solid box. Rotation uses its full sweep.
    pub fn occupancy_bounds(&self) -> Vec<(Vertex, Vertex)> {
        if self.motion.kind == MotionKind::Rotate {
            let radius = self.parts.iter().flat_map(|p| &p.vertices).map(|v|
                (0..3).map(|a| (v[a].unsigned_abs() * u64::from(self.scale_per_mille[a]) + 999) / 1000).sum::<u64>()
            ).max().unwrap_or(0) as i64;
            return vec![(self.position.map(|v| v-radius),self.position.map(|v| v+radius))];
        }
        let [rx,ry,rz] = self.rotation_mdeg.map(|v| (f64::from(v)/1000.0).to_radians());
        let (sx,cx)=rx.sin_cos(); let (sy,cy)=ry.sin_cos(); let (sz,cz)=rz.sin_cos();
        self.parts.iter().map(|part| {
            let mut low = [f64::INFINITY;3]; let mut high = [f64::NEG_INFINITY;3];
            for v in &part.vertices {
                let [x,y,z] = std::array::from_fn(|a| v[a] as f64 * f64::from(self.scale_per_mille[a])/1000.0);
                let (x,y)=(cz*x-sz*y,sz*x+cz*y);
                let (y,z)=(cx*y-sx*z,sx*y+cx*z);
                let point=[cy*x+sy*z,y,-sy*x+cy*z];
                for a in 0..3 { low[a]=low[a].min(point[a]);high[a]=high[a].max(point[a]); }
            }
            let delta=if self.motion.kind==MotionKind::Translate {self.motion.delta_cm} else {[0;3]};
            (std::array::from_fn(|a|self.position[a]+low[a].floor() as i64+delta[a].min(0)),
             std::array::from_fn(|a|self.position[a]+high[a].ceil() as i64+delta[a].max(0)))
        }).collect()
    }
}
pub fn validate(d: &MapDocument) -> Result<()> {
    let mut ids = BTreeSet::new();
    if d.gimmicks.len() > MAX_GIMMICKS { return Err(error("E_GIMMICK", "too many driving objects")); }
    for g in &d.gimmicks {
        if !g.valid() || !ids.insert(&g.id) || d.placements.iter().any(|p| p.id == g.id)
            || !(0..2).all(|a| g.safety_min_cm[a*2] >= d.bounds.min[a] && g.safety_max_cm[a*2] <= d.bounds.max[a]) {
            return Err(error("E_GIMMICK", format!("invalid motion, proxy or swept/landing bounds: {}", g.id)));
        }
    }
    Ok(())
}
