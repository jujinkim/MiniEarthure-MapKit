//! Public course authoring contract. Completion and vehicle physics belong to consumers.
use crate::{canonical, error, sha256, Bounds, Result, Vertex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const COURSE_VERSION: u32 = 1;
pub const COURSE_BYTES: usize = 15_000;
pub const VALIDATION_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_COURSES: usize = 64;

pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn label(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Circuit,
    Sprint,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlacementMode {
    Free,
    RoadSnap,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointShape {
    Sphere,
    Hemisphere,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartMode {
    Ground,
    Air,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub position_cm: Vertex,
    pub radius_cm: u32,
    pub shape: CheckpointShape,
    /// Authoring hints only; neither field restricts checkpoint passage.
    pub placement_mode: PlacementMode,
    pub surface_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CourseBody {
    pub map_id: String,
    pub display_name: String,
    pub world_content_hash: String,
    pub mode: Mode,
    pub start_mode: StartMode,
    /// Horizontal direction in map x/z coordinates. Nonzero, normalized by the consumer.
    pub start_direction: [i64; 2],
    pub checkpoints: Vec<Checkpoint>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidationReference {
    pub sha256: String,
    pub bytes: u32,
    pub world_content_hash: String,
    pub geometry_hash: String,
    pub path: String,
}
impl ValidationReference {
    pub fn validate(&self) -> Result<()> {
        if !valid_hash(&self.sha256)
            || !valid_hash(&self.world_content_hash)
            || !valid_hash(&self.geometry_hash)
            || self.bytes == 0
            || self.bytes as usize > VALIDATION_BYTES
            || self.path != format!("course-validation/{}.mevalidation", self.sha256)
        {
            return Err(error(
                "E_COURSE_VALIDATION",
                "invalid bounded validation reference",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Course {
    pub format: String,
    pub format_version: u32,
    pub course_id: String,
    pub definition: CourseBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationReference>,
}
impl Checkpoint {
    /// Intersection of closed spheres/upward hemispheres. At a fixed height the
    /// horizontal radii are concave; their maximum occurs at the weighted centre.
    pub fn overlaps(&self, other: &Self) -> bool {
        let a = self.position_cm.map(|v| v as f64);
        let b = other.position_cm.map(|v| v as f64);
        let ra = self.radius_cm as f64;
        let rb = other.radius_cm as f64;
        let low = (a[1] - if self.shape == CheckpointShape::Sphere {ra} else {0.0})
            .max(b[1] - if other.shape == CheckpointShape::Sphere {rb} else {0.0});
        let high = (a[1]+ra).min(b[1]+rb);
        if low > high { return false; }
        let y = ((a[1]*rb+b[1]*ra)/(ra+rb).max(1.0)).clamp(low,high);
        let reach = (ra*ra-(y-a[1]).powi(2)).max(0.0).sqrt()
            +(rb*rb-(y-b[1]).powi(2)).max(0.0).sqrt();
        (a[0]-b[0]).hypot(a[2]-b[2]) <= reach
    }
}
impl CourseBody {
    /// Stable first-wins indices; source files and their hashes stay untouched.
    pub fn effective_indices(&self) -> Vec<usize> {
        let mut kept: Vec<usize> = Vec::new();
        for (i, cp) in self.checkpoints.iter().enumerate() {
            if !kept.iter().any(|&j| cp.overlaps(&self.checkpoints[j])) { kept.push(i); }
        }
        kept
    }
    pub fn validate(&self, bounds: &Bounds) -> Result<()> {
        if !label(&self.map_id, 128)
            || !label(&self.display_name, 256)
            || !valid_hash(&self.world_content_hash)
            || self.checkpoints.len() > 64
            || self.start_direction == [0, 0]
            || self
                .start_direction
                .iter()
                .any(|v| v.unsigned_abs() > 1_000_000_000)
        {
            return Err(error(
                "E_COURSE",
                "invalid course identity, direction or checkpoint count",
            ));
        }
        for cp in &self.checkpoints {
            if !bounds.contains([cp.position_cm[0], cp.position_cm[2]])
                || cp
                    .position_cm
                    .iter()
                    .any(|v| v.unsigned_abs() > 1_000_000_000)
                || !(100..=100_000).contains(&cp.radius_cm)
                || cp.surface_id.len() > 256
                || cp.surface_id.chars().any(char::is_control)
            {
                return Err(error(
                    "E_CHECKPOINT",
                    "checkpoint exceeds position, radius or hint limits",
                ));
            }
        }
        Ok(())
    }
    pub fn geometry_hash(&self) -> Result<String> {
        // Names and driving content are separate identities. No reference points back into this hash.
        Ok(sha256(&canonical(
            &serde_json::json!({"map_id":self.map_id, "mode":self.mode,
            "start_mode":self.start_mode, "start_direction":self.start_direction,
            "checkpoints":self.checkpoints.iter().map(|c| serde_json::json!({
                "position_cm":c.position_cm,"radius_cm":c.radius_cm,"shape":c.shape})).collect::<Vec<_>>()}),
        )?))
    }
}
impl Course {
    /// Derived current driving view, shared by courses, AI, recovery and evidence.
    /// Removing ambiguous gates changes driving identity and invalidates old evidence.
    pub fn effective(&self) -> Self {
        let indices = self.definition.effective_indices();
        if indices.len() == self.definition.checkpoints.len() { return self.clone(); }
        let mut view = self.clone();
        view.definition.checkpoints = indices.into_iter().map(|i| self.definition.checkpoints[i].clone()).collect();
        view.course_id = sha256(&canonical(&view.definition).expect("typed course serialization"));
        view.validation = None;
        view
    }
    pub fn from_definition(definition: CourseBody, bounds: &Bounds) -> Result<Self> {
        definition.validate(bounds)?;
        let course_id = sha256(&canonical(&definition)?);
        let value = Self {
            format: "miniearthure-course".into(),
            format_version: COURSE_VERSION,
            course_id,
            definition,
            validation: None,
        };
        value.validate_document(bounds)?;
        Ok(value.effective())
    }
    pub fn seal(bytes: &[u8], world: &str, bounds: &Bounds) -> Result<Self> {
        let definition = decode(bytes)?;
        let course = Self::from_definition(definition, bounds)?;
        course.validate(world, bounds)?;
        Ok(course)
    }
    pub fn read(bytes: &[u8], world: &str, bounds: &Bounds) -> Result<Self> {
        let course: Self = decode(bytes)?;
        course.validate(world, bounds)?;
        Ok(course)
    }
    pub fn validate_document(&self, bounds: &Bounds) -> Result<()> {
        if self.format != "miniearthure-course" || self.format_version != COURSE_VERSION {
            return Err(error("E_COURSE_VERSION", "current course v1 required"));
        }
        self.definition.validate(bounds)?;
        if self.course_id != sha256(&canonical(&self.definition)?) {
            return Err(error("E_COURSE_HASH", "course identity mismatch"));
        }
        if let Some(reference) = &self.validation {
            reference.validate()?;
        }
        if canonical(self)?.len() > COURSE_BYTES {
            return Err(error("E_RACE_SIZE", "course exceeds byte limit"));
        }
        Ok(())
    }
    pub fn validate(&self, world: &str, bounds: &Bounds) -> Result<()> {
        self.validate_document(bounds)?;
        if self.definition.world_content_hash != world {
            return Err(error(
                "E_COURSE_WORLD",
                "course requires validation on this driving content",
            ));
        }
        Ok(())
    }
}
// Godot JSON emits integral floating tokens; reject fractions before typed decoding.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > COURSE_BYTES {
        return Err(error("E_RACE_SIZE", "course exceeds byte limit"));
    }
    let mut v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| error("E_COURSE_JSON", e.to_string()))?;
    fn integers(v: &mut serde_json::Value) -> Result<()> {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() != 0.0 || f.abs() > 9_007_199_254_740_991.0 {
                    return Err(error("E_COURSE_JSON", "exact integer required"));
                }
                *n = (f as i64).into();
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    integers(v)?;
                }
            }
            serde_json::Value::Object(o) => {
                for v in o.values_mut() {
                    integers(v)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    integers(&mut v)?;
    serde_json::from_value(v).map_err(|e| error("E_COURSE_JSON", e.to_string()))
}

/// Conservative chassis/wheel envelope in map centimetres. Authority supplies it;
/// endpoints interpolate continuously within one physical motion segment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capsule {
    pub a: Vertex,
    pub b: Vertex,
    pub radius_cm: u32,
}
impl Capsule {
    pub fn point(position: Vertex) -> Self { Self {a:position,b:position,radius_cm:0} }
    pub fn valid_at(&self, position: Vertex) -> bool {
        self.radius_cm<=100_000 && [self.a,self.b].iter().all(|p|
            p.iter().all(|v| v.unsigned_abs()<=1_000_000_000) &&
            (0..3).all(|i| (p[i]-position[i]).unsigned_abs()<=100_000))
    }
    pub fn separation(&self, cp: &Checkpoint) -> f64 {
        separation(self.a.map(|v|v as f64),self.b.map(|v|v as f64),self.radius_cm as f64,cp)
    }
}
fn separation(a: [f64;3], b: [f64;3], radius: f64, cp: &Checkpoint) -> f64 {
    let a: [f64;3]=std::array::from_fn(|i|a[i]-cp.position_cm[i] as f64);
    let b: [f64;3]=std::array::from_fn(|i|b[i]-cp.position_cm[i] as f64);
    let d: [f64;3]=std::array::from_fn(|i|b[i]-a[i]);
    let at=|t:f64|-> [f64;3] {std::array::from_fn(|i|a[i]+d[i]*t)};
    let sphere=cp.shape==CheckpointShape::Sphere || a[1].min(b[1])>=0.0;
    let norm=d.iter().map(|v|v*v).sum::<f64>();
    let closest=if norm==0.0 {0.0} else {(-a.iter().zip(d).map(|(a,d)|a*d).sum::<f64>()/norm).clamp(0.0,1.0)};
    let distance=|p:[f64;3]| {
        if sphere || p[1]>=0.0 {(p.iter().map(|v|v*v).sum::<f64>().sqrt()-cp.radius_cm as f64).max(0.0)}
        else {(p[0].hypot(p[2])-cp.radius_cm as f64).max(0.0).hypot(p[1])}
    };
    if sphere { return distance(at(closest))-radius; }
    // Distance to a convex hemisphere along a segment is convex.
    let (mut lo,mut hi)=(0.0,1.0);
    for _ in 0..32 {
        let l=(lo*2.0+hi)/3.0; let r=(lo+hi*2.0)/3.0;
        if distance(at(l))<distance(at(r)) {hi=r;} else {lo=l;}
    }
    distance(at((lo+hi)*0.5)).min(distance(a)).min(distance(b))-radius
}
/// Earliest swept entry, with a Lipschitz distance bound; no discrete tunnelling.
/// A stationary envelope and an envelope already inside an armed gate cannot pass.
pub fn capsule_entry(from: Capsule, to: Capsule, cp: &Checkpoint) -> Option<f64> {
    if from==to || from.separation(cp)<=0.001 {return None;}
    let length=|a:Vertex,b:Vertex| (0..3).map(|i|(a[i]-b[i]) as f64).map(|v|v*v).sum::<f64>().sqrt();
    let speed=length(from.a,to.a).max(length(from.b,to.b))+(from.radius_cm as f64-to.radius_cm as f64).abs();
    if speed<=0.001 {return None;}
    let distance=|t:f64| separation(std::array::from_fn(|i|from.a[i] as f64+(to.a[i]-from.a[i]) as f64*t),
        std::array::from_fn(|i|from.b[i] as f64+(to.b[i]-from.b[i]) as f64*t),
        from.radius_cm as f64+(to.radius_cm as f64-from.radius_cm as f64)*t,cp);
    fn search(f:&impl Fn(f64)->f64, speed:f64, lo:f64, hi:f64, depth:u8, budget:&mut usize)->Option<f64> {
        if *budget==0 {return None;} *budget-=1;
        let mid=(lo+hi)*0.5;
        let gap=f(mid);
        if gap-speed*(hi-lo)*0.5>0.001 {return None;}
        if depth==24 {return (gap<=0.002).then_some(mid);}
        search(f,speed,lo,mid,depth+1,budget).or_else(||search(f,speed,mid,hi,depth+1,budget))
    }
    search(&distance,speed,0.0,1.0,0,&mut 2048)
}
