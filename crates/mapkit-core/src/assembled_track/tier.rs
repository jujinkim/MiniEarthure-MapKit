//! Seed length tiers. A tier fixes lap count, sprint size and circuit homing;
//! it is never a duration target or a difficulty rating.
use super::*;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Length {
    #[default]
    Short,
    Medium,
    Long,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tier {
    pub circuit_laps: u8,
    pub sprint_pieces: usize,
    /// Homing weight β ceiling; β = homing_max · ramp^1.5 between start and forced.
    pub homing_max: f64,
    pub homing_start_cm: i64,
    pub closure_cm: i64,
    pub forced_homing_cm: i64,
}
const TIERS: [Tier; 3] = [
    Tier { circuit_laps: 3, sprint_pieces: 30, homing_max: 3.0, homing_start_cm: 15_000, closure_cm: 30_000, forced_homing_cm: 70_000 },
    Tier { circuit_laps: 2, sprint_pieces: 45, homing_max: 1.8, homing_start_cm: 30_000, closure_cm: 50_000, forced_homing_cm: 110_000 },
    Tier { circuit_laps: 2, sprint_pieces: 60, homing_max: 1.0, homing_start_cm: 50_000, closure_cm: 75_000, forced_homing_cm: 160_000 },
];
impl Length {
    pub const ALL: [Length; 3] = [Length::Short, Length::Medium, Length::Long];
    pub fn tier(self) -> &'static Tier {
        &TIERS[self as usize]
    }
    pub fn id(self) -> &'static str {
        ["short", "medium", "long"][self as usize]
    }
    pub fn label(self) -> &'static str {
        ["Short", "Medium", "Long"][self as usize]
    }
    pub fn max_laps(self, circuit: bool) -> u8 {
        if circuit { self.tier().circuit_laps } else { 1 }
    }
}
pub(super) fn options(circuit: bool) -> serde_json::Value {
    serde_json::json!(Length::ALL.iter().map(|l| if circuit {
        serde_json::json!({"length":l,"max_laps":l.max_laps(true)})
    } else {
        serde_json::json!({"length":l,"max_laps":1,"pieces":l.tier().sprint_pieces})
    }).collect::<Vec<_>>())
}
