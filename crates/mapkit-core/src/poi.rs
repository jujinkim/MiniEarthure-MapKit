//! Map-owned, nonphysical facility information. Coordinates are local centimetres.
use crate::*;

pub const MAX_POIS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointOfInterest {
    #[schemars(length(min = 1, max = 128))]
    pub id: String,
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    #[schemars(length(min = 1, max = 128))]
    pub category: String,
    pub position: Point,
    pub source: Attribution,
}

pub fn validate(pois: &[PointOfInterest], bounds: &Bounds) -> Result<()> {
    if pois.len() > MAX_POIS {
        return Err(error("E_POI_LIMIT", "too many points of interest"));
    }
    for poi in pois {
        for (value, limit) in [(&poi.name, 256), (&poi.category, 128)] {
            if value.trim().is_empty()
                || value.chars().count() > limit
                || value.chars().any(char::is_control)
            {
                return Err(error(
                    "E_POI",
                    "facility name/category must be bounded nonempty text",
                ));
            }
        }
        if !bounds.contains(poi.position) {
            return Err(error("E_POI", "facility coordinate outside map bounds"));
        }
        poi.source.validate("facility source")?;
    }
    Ok(())
}
