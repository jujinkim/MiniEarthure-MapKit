use mapkit_core::{MapDocument, environment::EnvironmentProfile};
use mapkit_package::*;
use std::collections::BTreeMap;
fn document() -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.recipe_version = 1;
    d.environment = Some(serde_json::from_value(serde_json::json!({"version":1,"concept":"countryside",
        "architecture":"timber","climate":"polar","settlement":"sparse",
        "latitude_mdeg":78000,"longitude_mdeg":16000,"utc_offset_minutes":60,
        "sunrise_minutes":360,"sunset_minutes":1080,"regions":[{"id":"town","concept":"metropolis","polygon":[[0,0],[10000,0],[10000,10000]]}],"lights":[]})).unwrap());
    d
}
#[test]
fn environment_roundtrip_hash_and_optional_profile() {
    let d = document();
    let bytes = pack_bytes(d.clone(),BTreeMap::new()).unwrap();
    let a = read_bytes(&bytes).unwrap();
    assert_eq!(a.document.environment,d.environment);
    assert_eq!(pack_bytes(a.document.clone(),a.files.clone()).unwrap(),bytes);
    let mut changed = d.clone();
    changed.environment.as_mut().unwrap().climate = "temperate".into();
    let b = read_bytes(&pack_bytes(changed,BTreeMap::new()).unwrap()).unwrap();
    assert_ne!(a.inspection.world_content_hash,b.inspection.world_content_hash);
    let mut old = d;
    old.environment=None;
    assert!(old.validate_source().is_ok());
    assert!(!serde_json::to_string(&old).unwrap().contains("environment"));
}
#[test]
fn invalid_authoring_is_rejected_before_rendering() {
    let d=document();
    let mut e: EnvironmentProfile = d.environment.clone().unwrap();
    e.regions[0].polygon=vec![[0,0],[10000,10000],[0,10000],[10000,0]];
    assert!(e.validate(&d.bounds,&d.assets).is_err());
    e=d.environment.unwrap();
    e.latitude_mdeg=90001;
    assert!(e.validate(&d.bounds,&d.assets).is_err());
    e.latitude_mdeg=78000;
    e.climate="arbitrary".into();
    assert!(e.validate(&d.bounds,&d.assets).is_err());
}

#[test]
fn regional_headers_omit_asset_bindings_and_cells_keep_relevant_bindings() {
    use mapkit_core::{source_metadata, region_source, Cell, CellRegion};
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/assets/document.json")).unwrap();
    d.recipe_version=1;
    let mut e=document().environment.unwrap();
    e.regions.clear();
    e.lights.push(serde_json::from_value(serde_json::json!({"asset_id":"tetra","window_materials":[0],"bulb_materials":[],"position_cm":[0,0,0],"range_cm":0,"color":[255,220,160]})).unwrap());
    d.environment=Some(e.clone());
    d.validate_source().unwrap();
    let header=source_metadata(&d);
    header.validate_source().unwrap();
    assert!(header.environment.unwrap().lights.is_empty());
    let region=region_source(&d,CellRegion { min:Cell{x:0,y:0},end:Cell{x:1,y:1} }).unwrap();
    region.validate_source().unwrap();
    assert_eq!(region.environment.unwrap().lights,e.lights);
}
