//! Optional v1 display identities. Catalogue composition never changes contact surfaces.
use crate::{error, MapDocument, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Deserialize)]
pub struct Ground { pub surface: String, pub color: [u8; 3], pub detail: String }
#[derive(Debug, Deserialize)]
pub struct Material { pub color: [u8; 3], pub roof_color: [u8; 3], pub roughness_per_mille: u16, pub metallic_per_mille: u16, pub detail: String }
#[derive(Debug, Deserialize)]
pub struct Architecture {
    pub material: String, pub roof: String, pub roof_material: String, pub modules: Vec<String>,
    /// Optional per-building-type defaults of this architecture. A present field
    /// replaces the architecture default for that use (`modules` replaces the set).
    #[serde(default)]
    pub uses: BTreeMap<String, UseStyle>,
    /// Optional display-only wall shades by wall material: a solid whose wall
    /// material is a key shows one entry, chosen by its stable identity.
    #[serde(default)]
    pub wall_palette: BTreeMap<String, Vec<String>>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseStyle {
    pub material: Option<String>, pub roof: Option<String>, pub roof_material: Option<String>, pub modules: Option<Vec<String>>,
    /// Display-only roof materials a default (catalogue) roof varies over, one per
    /// solid by its stable identity. Explicit roof selections never vary.
    pub roof_palette: Option<Vec<String>>,
}
impl Architecture {
    fn use_style(&self, kind: &str) -> Option<&UseStyle> { self.uses.get(kind) }
    /// Producer default wall material for a building of `kind`.
    pub fn material_for(&self, kind: &str) -> &str { self.use_style(kind).and_then(|u| u.material.as_deref()).unwrap_or(&self.material) }
    pub fn roof_for(&self, kind: &str) -> &str { self.use_style(kind).and_then(|u| u.roof.as_deref()).unwrap_or(&self.roof) }
    pub fn roof_material_for(&self, kind: &str) -> &str { self.use_style(kind).and_then(|u| u.roof_material.as_deref()).unwrap_or(&self.roof_material) }
    pub fn modules_for(&self, kind: &str) -> &[String] { self.use_style(kind).and_then(|u| u.modules.as_deref()).unwrap_or(&self.modules) }
    pub fn roof_palette_for(&self, kind: &str) -> &[String] { self.use_style(kind).and_then(|u| u.roof_palette.as_deref()).unwrap_or(&[]) }
    pub fn wall_palette_for(&self, material: &str) -> &[String] { self.wall_palette.get(material).map_or(&[], Vec::as_slice) }
}
#[derive(Debug, Deserialize)]
pub struct BuildingType { pub facade_kind: u8, pub modules: Vec<String> }
#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub version: u32,
    pub ground: BTreeMap<String, Ground>,
    pub materials: BTreeMap<String, Material>,
    pub architectures: BTreeMap<String, Architecture>,
    pub building_types: BTreeMap<String, BuildingType>,
    pub road_types: Vec<String>, pub vegetation_types: Vec<String>, pub barrier_types: Vec<String>,
    pub climates: Vec<String>, pub settlements: Vec<String>, pub terrains: Vec<String>, pub roof_shapes: Vec<String>,
    /// Optional display-only ground palettes by climate, then by terrain (which
    /// wins), keyed by ground name. Absent entries keep the `ground` colour.
    #[serde(default)]
    pub climate_ground: BTreeMap<String, BTreeMap<String, [u8; 3]>>,
    #[serde(default)]
    pub terrain_ground: BTreeMap<String, BTreeMap<String, [u8; 3]>>,
}
pub fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../../godot/regional_catalog.json")).expect("built-in appearance catalogue"))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain { Ground, Road, Building, Vegetation, Barrier }
/// IDs are compositions of stable catalogue names, not free-form material paths.
/// Empty IDs retain the existing v1 appearance exactly.
pub fn valid(id: &str, domain: Domain) -> bool {
    if id.is_empty() { return true; }
    if id.len() > 128 || !id.is_ascii() { return false; }
    let p: Vec<_> = id.split('/').collect();
    let c = catalog();
    match (domain, p.as_slice()) {
        (Domain::Ground, ["ground", kind]) => c.ground.contains_key(*kind),
        (Domain::Road, ["road", kind, ground]) => c.road_types.iter().any(|v| v == kind) && c.ground.contains_key(*ground),
        (Domain::Building, ["building", architecture, kind, material]) => c.architectures.contains_key(*architecture) && c.building_types.contains_key(*kind) && c.materials.contains_key(*material),
        (Domain::Building, ["building", architecture, kind, material, shape, roof]) => c.architectures.contains_key(*architecture) && c.building_types.contains_key(*kind) && c.materials.contains_key(*material) && c.roof_shapes.iter().any(|v| v == shape) && c.materials.contains_key(*roof)
            && (*roof != "blue_tile" || (*architecture == "cycladic" && *kind == "religious" && *shape == "dome")),
        (Domain::Vegetation, ["vegetation", kind]) => c.vegetation_types.iter().any(|v| v == kind),
        (Domain::Barrier, ["barrier", kind, material]) => c.barrier_types.iter().any(|v| v == kind) && c.materials.contains_key(*material),
        _ => false,
    }
}
pub fn validate(d: &MapDocument) -> Result<()> {
    let check = |id: &str, domain, object: &str| {
        crate::cancellation::checkpoint()?;
        if valid(id, domain) { Ok(()) } else { Err(error("E_APPEARANCE", format!("Invalid {domain:?} style {id:?} on {object}"))) }
    };
    check(&d.ground_style_id, Domain::Ground, "ground")?;
    for r in &d.roads {
        check(&r.style_id, Domain::Road, &r.id)?;
        check(&r.sidewalk_style_id, Domain::Ground, &r.id)?;
        if !r.sidewalk_style_id.is_empty() && r.sidewalk_cm == Some(0) { return Err(error("E_APPEARANCE", format!("Sidewalk style without sidewalk on {}", r.id))); }
    }
    for s in &d.surface_areas { check(&s.style_id, Domain::Ground, &s.id)?; }
    for b in &d.buildings { check(&b.style_id, Domain::Building, &b.id)?; }
    for z in &d.zones { check(&z.style_id, Domain::Vegetation, &z.id)?; }
    for b in &d.barriers { check(&b.style_id, Domain::Barrier, &b.id)?; }
    Ok(())
}
/// Mean sRGB used by both near and distant display; no colour-only random type selection.
pub fn color(id: &str, roof: bool) -> Option<[u8; 4]> {
    if let Some(b) = building(id) {
        let m = &catalog().materials[if roof { b.roof_material } else { b.material }];
        let rgb = if roof { m.roof_color } else { m.color };
        return Some([rgb[0], rgb[1], rgb[2], 255]);
    }
    let p: Vec<_> = id.split('/').collect();
    let c = catalog();
    let rgb = match p.as_slice() {
        ["ground", kind] | ["road", _, kind] => c.ground.get(*kind)?.color,
        ["building", _, _, material] | ["barrier", _, material] => {
            let m = c.materials.get(*material)?;
            if roof { m.roof_color } else { m.color }
        },
        _ => return None,
    };
    Some([rgb[0], rgb[1], rgb[2], 255])
}

/// [`color`] of a ground or road style under the map's resolved climate and
/// terrain ([`crate::environment::EnvironmentProfile::ground_design`]):
/// `terrain_ground`, then `climate_ground`, then the catalogue colour. Other
/// styles keep [`color`]. Display only; contact surfaces never change.
pub fn ground_color_for(id: &str, climate: &str, terrain: &str) -> Option<[u8; 4]> {
    let c = catalog();
    let p: Vec<_> = id.split('/').collect();
    let (["ground", kind] | ["road", _, kind]) = p.as_slice() else { return color(id, false) };
    c.ground.get(*kind)?;
    let rgb = c.terrain_ground.get(terrain).and_then(|t| t.get(*kind))
        .or_else(|| c.climate_ground.get(climate).and_then(|t| t.get(*kind)))
        .map_or_else(|| c.ground[*kind].color, |rgb| *rgb);
    Some([rgb[0], rgb[1], rgb[2], 255])
}

/// Natural cover whose steep faces may show bare rock at display time.
pub fn vegetated_ground(kind: &str) -> bool {
    matches!(kind, "managed_lawn" | "grassland" | "scrub" | "forest_floor" | "cropland" | "orchard" | "wetland")
}

/// Resolved optional roof components; four-component IDs take the architecture's
/// catalogue default for the building type.
#[derive(Clone, Copy)]
pub struct BuildingStyle<'a> { pub architecture: &'a str, pub kind: &'a str, pub material: &'a str, pub roof: &'a str, pub roof_material: &'a str, pub explicit_roof: bool }
fn pick<'a>(palette: &'a [String], identity: &str) -> &'a str {
    let hash = crate::sha256(identity.as_bytes());
    &palette[u32::from_str_radix(&hash[..8], 16).unwrap() as usize % palette.len()]
}
impl<'a> BuildingStyle<'a> {
    /// The roof material one solid displays: a catalogue-default roof takes the
    /// use's palette entry chosen by `identity` (stable per record and solid).
    pub fn roof_material_of(&self, identity: &str) -> &'a str {
        let palette = catalog().architectures[self.architecture].roof_palette_for(self.kind);
        if self.explicit_roof || palette.is_empty() { return self.roof_material; }
        pick(palette, identity)
    }
    /// The wall shade one solid displays from the architecture's `wall_palette`
    /// for its wall material (independent of the roof choice); else the material.
    pub fn wall_material_of(&self, identity: &str) -> &'a str {
        let palette = catalog().architectures[self.architecture].wall_palette_for(self.material);
        if palette.is_empty() { return self.material; }
        pick(palette, &format!("{identity}/wall"))
    }
}
pub fn building(id: &str) -> Option<BuildingStyle<'_>> {
    let p: Vec<_> = id.split('/').collect();
    if !matches!(p.len(), 4 | 6) || p[0] != "building" { return None; }
    let a = catalog().architectures.get(p[1])?;
    Some(BuildingStyle { architecture: p[1], kind: p[2], material: p[3], roof: p.get(4).copied().unwrap_or(a.roof_for(p[2])), roof_material: p.get(5).copied().unwrap_or(a.roof_material_for(p[2])), explicit_roof: p.len() == 6 })
}

/// Find appearance from semantic ownership, never from low-resolution cover over a road.
pub fn triangle_style<'a>(d: &'a MapDocument, id: &str, spawnable: bool) -> &'a str {
    if let Ok(i) = d.buildings.binary_search_by(|b| b.id.as_str().cmp(id)) { return &d.buildings[i].style_id; }
    if let Some(owner) = id.strip_suffix(":sidewalk") { if let Ok(i) = d.roads.binary_search_by(|r|r.id.as_str().cmp(owner)) { return &d.roads[i].sidewalk_style_id; } }
    if let Ok(i) = d.roads.binary_search_by(|r| r.id.as_str().cmp(id)) { return if spawnable { &d.roads[i].style_id } else { "" }; }
    if let Ok(i) = d.surface_areas.binary_search_by(|a| a.id.as_str().cmp(id)) { return &d.surface_areas[i].style_id; }
    if let Ok(i) = d.barriers.binary_search_by(|b| b.id.as_str().cmp(id)) { return &d.barriers[i].style_id; }
    if id == "terrain" || id == "ground" || id.starts_with("ground:") { return &d.ground_style_id; }
    ""
}
