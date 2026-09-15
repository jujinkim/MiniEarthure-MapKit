//! Declarative environment authoring. Simulation and game transport are consumers.
use crate::{error, Bounds, Point, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONCEPTS: [&str; 7] = ["polar", "metropolis", "countryside", "middle-eastern", "desert", "jungle", "southeast-asian"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentRegion {
    pub id: String,
    pub concept: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub architecture: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub climate: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub settlement: String,
    pub polygon: Vec<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LightBinding {
    pub asset_id: String,
    pub window_materials: Vec<u16>,
    pub bulb_materials: Vec<u16>,
    pub position_cm: [i32; 3],
    pub range_cm: u32,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentProfile {
    pub version: u32,
    pub concept: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub architecture: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub climate: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub settlement: String,
    pub latitude_mdeg: i32,
    pub longitude_mdeg: i32,
    pub utc_offset_minutes: i32,
    pub sunrise_minutes: u16,
    pub sunset_minutes: u16,
    pub regions: Vec<EnvironmentRegion>,
    pub lights: Vec<LightBinding>,
}

impl EnvironmentProfile {
    pub fn validate(&self, bounds: &Bounds, assets: &[crate::Asset]) -> Result<()> {
        let bad = || error("E_ENVIRONMENT", "Invalid or excessive environment profile");
        if !dimensions(&self.architecture, &self.climate, &self.settlement) || self.version != 1 || !CONCEPTS.contains(&self.concept.as_str())
            || !(-90000..=90000).contains(&self.latitude_mdeg)
            || !(-180000..=180000).contains(&self.longitude_mdeg)
            || !(-720..=840).contains(&self.utc_offset_minutes)
            || self.sunrise_minutes >= 1440 || self.sunset_minutes >= 1440
            || self.sunrise_minutes == self.sunset_minutes
            || self.regions.len() > 64 || self.lights.len() > 256 { return Err(bad()); }
        let mut ids = std::collections::BTreeSet::new();
        for region in &self.regions {
            if !dimensions(&region.architecture, &region.climate, &region.settlement) || region.id.is_empty() || region.id.len() > 128 || !ids.insert(&region.id)
                || !CONCEPTS.contains(&region.concept.as_str())
                || !(3..=64).contains(&region.polygon.len())
                || !crate::polygon_valid(&region.polygon, bounds) { return Err(bad()); }
            // Overlaps have explicit ordered precedence, as in the editor list.
        }
        ids.clear();
        for light in &self.lights {
            if !ids.insert(&light.asset_id) || !assets.iter().any(|a| a.id == light.asset_id)
                || light.window_materials.len() > 64 || light.bulb_materials.len() > 16
                || light.position_cm.iter().any(|p| !(-100000..=100000).contains(p))
                || light.range_cm > 5000 || (!light.bulb_materials.is_empty() && light.range_cm < 100)
                || light.window_materials.iter().chain(&light.bulb_materials).collect::<std::collections::BTreeSet<_>>().len() != light.window_materials.len()+light.bulb_materials.len() { return Err(bad()); }
        }
        Ok(())
    }
}

fn dimensions(architecture: &str, climate: &str, settlement: &str) -> bool {
    ["", "modern", "rural", "adobe", "timber", "tropical"].contains(&architecture)
        && ["", "temperate", "polar", "arid", "tropical"].contains(&climate)
        && ["", "urban", "village", "sparse", "wilderness"].contains(&settlement)
}
