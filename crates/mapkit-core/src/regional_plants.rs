//! Bounded regional plant silhouettes shared with Godot; scene-metre templates.
use crate::MapDocument;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};
#[derive(Deserialize)]
pub struct Face { pub vertices: [[f32;3];3], pub leaf: bool }
#[derive(Deserialize)]
pub struct Plant { pub color: [u8;3], pub variants: Vec<Vec<Face>> }
pub fn catalog()-> &'static BTreeMap<String,Plant> {
    static DATA: OnceLock<BTreeMap<String,Plant>>=OnceLock::new();
    DATA.get_or_init(||serde_json::from_str(include_str!("../../../godot/regional_plants.json")).expect("regional plant templates"))
}
pub fn zone_style<'a>(d:&'a MapDocument,id:&str)-> &'a str {
    crate::generated_zone_id(id).and_then(|z|d.zones.binary_search_by(|v|v.id.as_str().cmp(z)).ok()).map_or("",|i|d.zones[i].style_id.as_str())
}
pub fn kind(style:&str,tree:bool)->Option<&'static str> {
    match (style,tree) {
        ("vegetation/conifer",true)=>Some("conifer"),
        ("vegetation/palm",true)=>Some("palm"),
        ("vegetation/orchard",true)=>Some("orchard"),
        ("vegetation/scrub",false)=>Some("scrub"),
        ("vegetation/polar_scrub",false)=>Some("polar"),
        _=>None,
    }
}
