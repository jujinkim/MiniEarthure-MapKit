//! Bounded declarative driving structures. There is no executable content.
use crate::*;
use std::collections::BTreeSet;

pub const MAX_GIMMICKS: usize = 128;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MotionKind { Static, Translate, Rotate, Boost, Launch, TargetSpeed, JumpHeight, AirRing }
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
pub struct Effect {
    pub strength_percent: u8,
    pub jump_height_cm: u32,
    /// Ring aperture lies in local XY, centred at the record origin; +Z passes.
    pub ring_radius_cm: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gimmick {
    pub id: String,
    pub position: Vertex,
    pub rotation_mdeg: [i32; 3],
    pub scale_per_mille: [u32; 3],
    pub parts: Vec<CollisionConvex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<special_track::SpecialTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
    pub surface: Surface,
    pub color: [u8; 4],
    pub motion: Motion,
    /// Conservative full swept envelope AND the predicted pad landing area.
    pub safety_min_cm: Vertex,
    pub safety_max_cm: Vertex,
}
impl Gimmick {
    pub fn valid(&self) -> bool {
        if self.id.is_empty() || self.id.len() > 128 || (self.track.is_none() && self.parts.is_empty()) || self.parts.len() > 32
            || self.rotation_mdeg.iter().any(|v| v.unsigned_abs() > 360_000)
            || self.scale_per_mille.iter().any(|v| !(100..=10_000).contains(v))
            || self.position.iter().any(|v| v.unsigned_abs() > 10_000_000)
            || self.parts.iter().any(|p| !p.valid(10_000))
            || self.motion.axis > 2 || !(250..=120_000).contains(&self.motion.period_ms)
            || self.motion.phase_ms >= self.motion.period_ms
            || self.motion.delta_cm.iter().any(|v| v.unsigned_abs() > 3200)
            || self.motion.impulse_cmps.iter().any(|v| v.unsigned_abs() > 5000)
            || !(100..=60_000).contains(&self.motion.cooldown_ms) { return false; }
        let active = matches!(self.motion.kind, MotionKind::TargetSpeed | MotionKind::JumpHeight | MotionKind::AirRing);
        if active != self.effect.is_some() { return false; }
        if let Some(e)=&self.effect {
            if !(1..=100).contains(&e.strength_percent) || !(10..=1000).contains(&e.jump_height_cm)
                || !(75..=600).contains(&e.ring_radius_cm) { return false; }
        }
        if let Some(track)=&self.track {
            if !track.valid() || !self.parts.is_empty() || self.motion.kind!=MotionKind::Static
                || self.scale_per_mille!=[1000;3] { return false; }
        }
        // Use an integer L1 sphere bound: encloses arbitrary Euler rotations and
        // continuous rotation, independent of platform trigonometry rounding.
        let radius = self.parts.iter().flat_map(|p| &p.vertices).map(|v|
            (0..3).map(|a| (v[a].unsigned_abs() * u64::from(self.scale_per_mille[a]) + 999) / 1000).sum::<u64>()
        ).max().unwrap_or(0) as i64;
        let radius = self.track.as_ref().map_or(radius, |t| t.bound_radius());
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
        16_384 + self.parts.len() as u64 * 32_768 + self.track.as_ref().map_or(0, |t| t.tile_count() as u64 * 4096)
    }
    /// Track interiors are never generated spawn or recovery surfaces, even if
    /// a normal road/terrain triangle happens to lie underneath the structure.
    pub fn excludes_spawn(&self, position: Vertex) -> bool {
        let Some(t)=&self.track else {return false};
        let [rx,ry,rz]=self.rotation_mdeg.map(|v|f64::from(v)*std::f64::consts::PI/180000.0);
        let [x,y,z]=std::array::from_fn(|a|(position[a]-self.position[a]) as f64);
        let (x,z)=(libm::cos(ry)*x-libm::sin(ry)*z,libm::sin(ry)*x+libm::cos(ry)*z);
        let (y,z)=(libm::cos(rx)*y+libm::sin(rx)*z,-libm::sin(rx)*y+libm::cos(rx)*z);
        let (x,y)=(libm::cos(rz)*x+libm::sin(rz)*y,-libm::sin(rz)*x+libm::cos(rz)*y);
        let r=f64::from(t.radius_cm); let w=f64::from(t.width_cm);
        match t.kind {
            special_track::TrackKind::Loop => x.abs()<=1.5*w+50.0 && z>=-2.0*r-50.0 && z<=4.0*r+50.0 && y>=-50.0 && y<=2.0*r+50.0,
            special_track::TrackKind::Cylinder => z.abs()<=f64::from(t.length_cm)/2.0+50.0 && x*x+(y-r)*(y-r)<=(1.12*r+50.0).powi(2),
        }
    }
    pub fn occupied_count(&self) -> u64 {
        self.track.as_ref().map_or(self.parts.len() as u64, |t| t.tile_count() as u64)
    }
    /// Expanded geometry is only a bridge representation, never authored source.
    pub fn resolved_json(&self) -> serde_json::Value {
        let mut value=serde_json::to_value(self).unwrap();
        value["memory_bytes"]=serde_json::json!(self.memory_bytes());
        if let Some(t)=&self.track { value["track_mesh"]=serde_json::to_value(t.mesh()).unwrap(); }
        value
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
        let (sx,cx)=(libm::sin(rx),libm::cos(rx)); let (sy,cy)=(libm::sin(ry),libm::cos(ry)); let (sz,cz)=(libm::sin(rz),libm::cos(rz));
        let vertices: Vec<Vec<Vertex>> = if let Some(t)=&self.track {
            t.mesh().tiles.into_iter().map(|(lo,hi)| (0..8).map(|i|std::array::from_fn(|a|if i&(1<<a)==0 {lo[a]} else {hi[a]})).collect()).collect()
        } else { self.parts.iter().map(|p|p.vertices.clone()).collect() };
        vertices.iter().map(|vertices| {
            let mut low = [f64::INFINITY;3]; let mut high = [f64::NEG_INFINITY;3];
            for v in vertices {
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
