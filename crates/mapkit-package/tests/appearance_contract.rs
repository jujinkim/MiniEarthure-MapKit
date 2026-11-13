use mapkit_core::{appearance::{self, Domain}, *};
use mapkit_package::*;
use std::collections::BTreeMap;
fn document() -> MapDocument { serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap() }
fn styled() -> MapDocument {
    let mut d = document();
    d.ground_style_id = "ground/grassland".into();
    d.roads[0].style_id = "road/street/asphalt".into();
    d.roads[0].sidewalk_cm = Some(100);
    d.roads[0].sidewalk_style_id = "ground/pavers".into();
    d.buildings[0].style_id = "building/venetian/hotel/plaster".into();
    for z in &mut d.zones { z.style_id = "vegetation/conifer".into(); }
    d.surface_areas.push(SurfaceArea { id: "plaza".into(), polygon: vec![[40000,40000],[45000,40000],[45000,45000],[40000,45000]], surface: Surface::Concrete, style_id: "ground/stone_paving".into() });
    d.barriers.push(barrier::Barrier { id: "quay".into(), line: vec![[46000,40000],[46000,45000]], kind: barrier::BarrierKind::Wall, height_cm: 100, thickness_cm: 30, style_id: "barrier/quay/stone".into() });
    d
}
#[test]
fn styles_survive_pack_and_regional_storage_without_changing_physics() {
    let styled = styled();
    styled.validate().unwrap();
    let mut plain = styled.clone();
    plain.ground_style_id.clear();
    for r in &mut plain.roads { r.style_id.clear(); r.sidewalk_style_id.clear(); }
    for b in &mut plain.buildings { b.style_id.clear(); }
    for a in &mut plain.surface_areas { a.style_id.clear(); }
    for b in &mut plain.barriers { b.style_id.clear(); }
    for z in &mut plain.zones { z.style_id.clear(); }
    let generate_cell = |d: &MapDocument| generate(GenerationInput { document: d, cell: Cell{x:0,y:0}, heightgrid: None, max_triangles: 500_000 }).unwrap();
    assert_eq!(canonical(&generate_cell(&plain)).unwrap(), canonical(&generate_cell(&styled)).unwrap());
    let before = read_bytes(&pack_bytes(plain, BTreeMap::new()).unwrap()).unwrap();
    let bytes = pack_bytes(styled.clone(), BTreeMap::new()).unwrap();
    let package = read_bytes(&bytes).unwrap();
    assert_ne!(before.inspection.world_content_hash, package.inspection.world_content_hash);
    assert_eq!(package.document.ground_style_id, styled.ground_style_id);
    assert_eq!(package.document.buildings[0].style_id, styled.buildings[0].style_id);
    assert_eq!(pack_bytes(package.document.clone(), package.files.clone()).unwrap(), bytes);
    let region = region_source(&styled, CellRegion { min: Cell{x:0,y:0}, end: Cell{x:1,y:1} }).unwrap();
    region.validate_source().unwrap();
    assert_eq!(region.ground_style_id, styled.ground_style_id);
    assert_eq!(region.buildings[0].style_id, styled.buildings[0].style_id);
    assert_eq!(region.roads.iter().find(|r| r.id == styled.roads[0].id).unwrap().sidewalk_style_id, styled.roads[0].sidewalk_style_id);
}
#[test]
fn catalogue_compositions_are_strict_and_omissions_are_unchanged() {
    let d = document();
    assert!(!String::from_utf8(canonical(&d).unwrap()).unwrap().contains("style_id"));
    let c = appearance::catalog();
    assert_eq!(c.version, 1);
    for a in c.architectures.keys() { for k in c.building_types.keys() {
        assert!(appearance::valid(&format!("building/{a}/{k}/{}", c.architectures[a].material), Domain::Building));
    }}
    for k in c.ground.keys() { assert!(appearance::valid(&format!("ground/{k}"), Domain::Ground)); }
    // Per-use architecture defaults name catalogue types, materials, roofs and known modules.
    let modules: std::collections::BTreeSet<&String> = c.architectures.values().flat_map(|a| &a.modules).chain(c.building_types.values().flat_map(|k| &k.modules)).collect();
    for (a, arch) in &c.architectures { for kind in arch.uses.keys() {
        assert!(c.building_types.contains_key(kind), "{a} use {kind}");
        assert!(appearance::valid(&format!("building/{a}/{kind}/{}/{}/{}", arch.material_for(kind), arch.roof_for(kind), arch.roof_material_for(kind)), Domain::Building), "{a} use {kind}");
        assert!(arch.modules_for(kind).iter().all(|m| modules.contains(m)), "{a} use {kind} modules");
    }}
    let house = appearance::building("building/korean_modern/detached/plaster").unwrap();
    assert_eq!((house.roof, house.roof_material), ("hipped", "dark_tile"));
    let house = appearance::building("building/japanese_modern/detached/plaster").unwrap();
    assert_eq!((house.roof, house.roof_material), ("gabled", "dark_tile"));
    let shed = appearance::building("building/japanese_modern/warehouse/plaster").unwrap();
    assert_eq!((shed.roof, shed.roof_material), ("gabled", "metal"));
    let flats = appearance::building("building/korean_modern/apartment/concrete").unwrap();
    assert_eq!((flats.roof, flats.roof_material), ("flat", "concrete"));
    // Display palettes name catalogue climates, terrains and grounds only.
    for (climate, table) in &c.climate_ground {
        assert!(c.climates.contains(climate));
        assert!(table.keys().all(|k| c.ground.contains_key(k)));
    }
    for (terrain, table) in &c.terrain_ground {
        assert!(c.terrains.contains(terrain));
        assert!(table.keys().all(|k| c.ground.contains_key(k)));
    }
    for k in c.ground.keys() {
        assert_eq!(appearance::ground_color_for(&format!("ground/{k}"), "temperate", "plain"), appearance::color(&format!("ground/{k}"), false));
    }
    for bad in ["building/unknown/school/stone", "building/venetian/anything/stone", "building/cycladic/religious/blue_dome", "ground/asphalt", "building/modern/school/../../file"] {
        let mut d = styled(); d.buildings[0].style_id = bad.into();
        assert_eq!(d.validate().unwrap_err().code, "E_APPEARANCE");
    }
    let mut d = styled(); d.roads[0].sidewalk_cm = Some(0);
    assert_eq!(d.validate().unwrap_err().code, "E_APPEARANCE");
}
#[test]
fn courtyard_parts_inherit_the_selected_type_without_altering_their_bounds() {
    let mut d = styled();
    let b = &mut d.buildings[0];
    b.parts.push(BuildingPart { footprint: vec![[32000,30000],[34000,30000],[34000,32000],[32000,32000]], holes: vec![], base_cm: None, height_cm: Some(900) });
    assert_eq!(b.solid(1).style_id, b.style_id);
    assert_eq!(b.solid(1).height_cm, 900);
    d.validate().unwrap();
}
