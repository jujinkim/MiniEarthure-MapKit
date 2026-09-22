//! Public course authoring contract. Completion and vehicle physics belong to consumers.
use crate::{canonical, error, sha256, Bounds, Result, Vertex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const COURSE_VERSION: u32 = 1;
pub const COURSE_BYTES: usize = 15_000;
pub const VALIDATION_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_COURSES: usize = 64;

pub fn valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn label(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode { Circuit, Sprint }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlacementMode { Free, RoadSnap }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointShape { Sphere, Hemisphere }
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartMode { Ground, Air }
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
        if !valid_hash(&self.sha256) || !valid_hash(&self.world_content_hash)
            || !valid_hash(&self.geometry_hash) || self.bytes == 0 || self.bytes as usize > VALIDATION_BYTES
            || self.path != format!("course-validation/{}.mevalidation", self.sha256) {
            return Err(error("E_COURSE_VALIDATION", "invalid bounded validation reference"));
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
impl CourseBody {
    pub fn validate(&self, bounds: &Bounds) -> Result<()> {
        if !label(&self.map_id, 128) || !label(&self.display_name, 256)
            || !valid_hash(&self.world_content_hash) || !(2..=64).contains(&self.checkpoints.len())
            || self.start_direction == [0, 0] || self.start_direction.iter().any(|v| v.unsigned_abs() > 1_000_000_000) {
            return Err(error("E_COURSE", "invalid course identity, direction or checkpoint count"));
        }
        for cp in &self.checkpoints {
            if !bounds.contains([cp.position_cm[0], cp.position_cm[2]])
                || cp.position_cm.iter().any(|v| v.unsigned_abs() > 1_000_000_000)
                || !(100..=100_000).contains(&cp.radius_cm) || cp.surface_id.len() > 256
                || cp.surface_id.chars().any(char::is_control) {
                return Err(error("E_CHECKPOINT", "checkpoint exceeds position, radius or hint limits"));
            }
        }
        Ok(())
    }
    pub fn geometry_hash(&self) -> Result<String> {
        // Names and driving content are separate identities. No reference points back into this hash.
        Ok(sha256(&canonical(&serde_json::json!({"map_id":self.map_id, "mode":self.mode,
            "start_mode":self.start_mode, "start_direction":self.start_direction,
            "checkpoints":self.checkpoints.iter().map(|c| serde_json::json!({
                "position_cm":c.position_cm,"radius_cm":c.radius_cm,"shape":c.shape})).collect::<Vec<_>>()}))?))
    }
}
impl Course {
    pub fn from_definition(definition: CourseBody, bounds: &Bounds) -> Result<Self> {
        definition.validate(bounds)?;
        let course_id = sha256(&canonical(&definition)?);
        let value = Self { format: "miniearthure-course".into(), format_version: COURSE_VERSION,
            course_id, definition, validation: None };
        value.validate_document(bounds)?;
        Ok(value)
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
        if let Some(reference) = &self.validation { reference.validate()?; }
        if canonical(self)?.len() > COURSE_BYTES { return Err(error("E_RACE_SIZE", "course exceeds byte limit")); }
        Ok(())
    }
    pub fn validate(&self, world: &str, bounds: &Bounds) -> Result<()> {
        self.validate_document(bounds)?;
        if self.definition.world_content_hash != world {
            return Err(error("E_COURSE_WORLD", "course requires validation on this driving content"));
        }
        Ok(())
    }
}
// Godot JSON emits integral floating tokens; reject fractions before typed decoding.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > COURSE_BYTES { return Err(error("E_RACE_SIZE", "course exceeds byte limit")); }
    let mut v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| error("E_COURSE_JSON", e.to_string()))?;
    fn integers(v: &mut serde_json::Value) -> Result<()> {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() != 0.0 || f.abs() > 9_007_199_254_740_991.0 { return Err(error("E_COURSE_JSON", "exact integer required")); }
                *n = (f as i64).into();
            },
            serde_json::Value::Array(a) => for v in a { integers(v)?; },
            serde_json::Value::Object(o) => for v in o.values_mut() { integers(v)?; },
            _ => (),
        }
        Ok(())
    }
    integers(&mut v)?;
    serde_json::from_value(v).map_err(|e| error("E_COURSE_JSON", e.to_string()))
}
