use mapkit_core::{*, assembled_track::{self as track,authoring::*}};
use mapkit_package::*;
use std::collections::BTreeMap;

fn base() -> MapDocument {
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.free_roam=true; d.bounds=Bounds{min:[-32000,-32000],max:[32000,32000]};d.cell_size_cm=3200;
    d.roads.clear();d.nodes.clear();d.repetitions.clear();d.buildings.clear();d.zones.clear();d.placements.clear();d.assets.clear();
    d
}

#[test]
fn overlay_preserves_document_and_removes_only_owned_products() {
    let mut d=base();
    let templates:BTreeMap<String,gimmick::Gimmick>=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    let mut independent=templates["loop"].clone();independent.id="independent-loop".into();
    d.gimmicks.push(independent.clone());
    let mut source=Source::empty();let mut piece=instance("road","straight",400);piece.position_cm=[8000,0,8000];source.instances.push(piece);
    let original=d.clone();
    let mut next=track::composite::apply_source(&d,&source).unwrap();
    assert_eq!(d,original);
    assert_eq!(next.map_id,d.map_id);assert_eq!(next.bounds,d.bounds);assert_eq!(next.environment,d.environment);assert_eq!(next.gimmicks[0],independent);
    let bytes=pack_bytes(next.clone(),BTreeMap::new()).unwrap();
    let package=read_bytes(&bytes).unwrap();let p=[8100,8350];
    let cell=package.document.cell_at(p).unwrap();let chunk=package.generate(cell,200000).unwrap();
    assert_eq!(chunk.spawn(&SpawnRequest{position_cm:p,surface_id:"assembled-road-0".into()}).unwrap()[1],0);
    assert!(chunk.spawn(&SpawnRequest{position_cm:p,surface_id:"terrain".into()}).is_err());
    assert!(chunk.triangles.iter().any(|t|t.object_id=="terrain"));
    assert!(!chunk.triangles.iter().any(|t|t.object_id=="assembled-venue-floor"));
    next=track::composite::apply_source(&next,&Source::empty()).unwrap();
    assert_eq!(next.gimmicks,d.gimmicks);
    let package=read_bytes(&pack_bytes(next,BTreeMap::new()).unwrap()).unwrap();
    let chunk=package.generate(cell,200000).unwrap();
    assert_eq!(chunk.spawn(&SpawnRequest{position_cm:p,surface_id:"terrain".into()}).unwrap()[1],0);
}

#[test]
fn free_roam_keeps_course_draft_but_geometry_hazards_block_packages() {
    let mut source=Source::empty();source.instances.push(instance("one","straight",400));
    let mut d=track::composite::apply_source(&base(),&source).unwrap();
    assert!(!d.assembled_track.as_ref().unwrap().issues.is_empty());
    assert!(pack_bytes(d.clone(),BTreeMap::new()).is_ok());
    d.free_roam=false;
    assert_eq!(pack_bytes(d,BTreeMap::new()).unwrap_err().code,"E_TRACK_DRAFT");
    source.instances.push(instance("two","straight",400));
    let d=track::composite::apply_source(&base(),&source).unwrap();
    assert_eq!(pack_bytes(d,BTreeMap::new()).unwrap_err().code,"E_TRACK_GEOMETRY");
}

#[test]
fn race_export_binds_the_current_overlay_course_without_completion_proof() {
    let mut source=Source::empty();source.settings.circuit=false;
    for (id,z) in [("start",-3000),("end",0)] {
        let mut p=instance(id,"free_curve",800);p.position_cm=[0,0,z];
        p.control_points=vec![[0,0,0],[0,0,1000],[0,0,2000],[0,0,3000]];
        source.instances.push(p);
    }
    source.connections.push(Connection{from:"start".into(),to:"end".into()});
    source.paths.push(Path{id:"race".into(),pieces:vec!["start".into(),"end".into()]});
    source.checkpoints=vec![Checkpoint{piece:"start".into(),sample:0},Checkpoint{piece:"end".into(),sample:0}];
    let mut b=base();b.free_roam=false;
    let d=track::composite::apply_source(&b,&source).unwrap();assert!(d.courses.is_empty());
    let p=read_bytes(&pack_bytes(d.clone(),BTreeMap::new()).unwrap()).unwrap();
    assert!(d.courses.is_empty());assert_eq!(p.document.courses.len(),1);
    let c=&p.document.courses[0];assert!(c.validation.is_none());
    assert_eq!(c.definition.world_content_hash,p.inspection.world_content_hash);
    mapkit_package::assembled_track::verify_course(&p.document,&p.inspection.world_content_hash,c).unwrap();
}

#[test]
fn every_current_palette_piece_compiles_alongside_terrain() {
    for id in track::catalogue_ids() {
        let mut source=Source::empty();let mut i=instance("piece",id,400);i.position_cm=[0,0,0];
        if ["free_curve","flight_curve"].contains(&id) {i.control_points=vec![[0,0,0],[0,0,600],[600,0,1200],[1200,0,1200]];}
        source.instances.push(i);
        let d=track::composite::apply_source(&base(),&source).unwrap_or_else(|e|panic!("{id}: {e}"));
        track::verify_document(&d).unwrap();
        let p=&d.assembled_track.as_ref().unwrap().pieces[0];
        let lo=d.cell_at([p.reserved_min_cm[0],p.reserved_min_cm[2]]).unwrap();
        let hi=d.cell_at([p.reserved_max_cm[0],p.reserved_max_cm[2]]).unwrap();
        let prepared=PreparedMap::new(d).unwrap();
        for y in lo.y..=hi.y {for x in lo.x..=hi.x {
            let cell=Cell{x,y};let cost=prepared.estimate(cell,2000000).unwrap();
            let chunk=prepared.generate(cell,None,2000000).unwrap_or_else(|e|panic!("{id} {cell:?}: {e}"));
            assert!(chunk.triangles.iter().any(|t|t.object_id=="terrain"),"{id} {cell:?}");
            assert!(chunk.triangles.len() as u64<=cost.triangles,"{id} {cell:?}: estimate {} < {}",cost.triangles,chunk.triangles.len());
        }}
    }
}

#[test]
fn derived_supports_must_not_block_independent_solids() {
    let mut b=base();
    b.buildings.push(Building{id:"keep-under-track".into(),footprint:vec![[-300,-100],[300,-100],[300,900],[-300,900]],holes:vec![],base_cm:0,height_cm:300,usage:"commercial".into(),material:"concrete".into(),roof:"flat".into(),entrances:vec![]});
    let mut source=Source::empty();let mut p=instance("raised","straight",400);p.position_cm[1]=1000;source.instances.push(p);
    source.terrain_policies.insert("raised".into(),road_design::TerrainPolicy::Elevated);
    let d=track::composite::apply_source(&b,&source).unwrap();
    let e=pack_bytes(d,BTreeMap::new()).unwrap_err();
    assert_eq!(e.code,"E_TRACK_ENVIRONMENT");assert!(e.message.contains("support intersects keep-under-track"),"{e}");
}

#[test]
fn environment_interference_blocks_export_without_removing_user_objects() {
    let mut base=base();
    base.buildings.push(Building{id:"keep-building".into(),footprint:vec![[-300,300],[300,300],[300,600],[-300,600]],holes:vec![],base_cm:0,height_cm:500,usage:"commercial".into(),material:"concrete".into(),roof:"flat".into(),entrances:vec![]});
    let mut source=Source::empty();source.instances.push(instance("one","straight",400));
    let d=track::composite::apply_source(&base,&source).unwrap();
    let failure=pack_bytes(d.clone(),BTreeMap::new()).err().expect("occupied driving space must fail");
    assert_eq!(failure.code,"E_TRACK_ENVIRONMENT");assert!(failure.message.contains("keep-building"));
    assert_eq!(d.buildings,base.buildings);
    base.buildings.clear();base.terrain_base_cm=500;
    source.terrain_policies.insert("one".into(),road_design::TerrainPolicy::Preserve);
    let d=track::composite::apply_source(&base,&source).unwrap();
    assert_eq!(pack_bytes(d,BTreeMap::new()).err().unwrap().code,"E_TRACK_ENVIRONMENT");
}

#[test]
fn ordinary_preserved_terrain_must_clear_the_authored_surface() {
    let template:MapDocument=serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    let mut road=template.roads[0].clone();road.id="authored".into();road.from="a".into();road.to="b".into();road.kind=RoadKind::Ground;road.sidewalk_cm=Some(0);
    let design=road_design::from_points(&[[-3000,100,0],[3000,100,0]]).unwrap();
    road_design::compile(&mut road,design,600).unwrap();
    let mut d=base();d.terrain_base_cm=500;
    d.nodes=vec![RoadNode{id:"a".into(),position:road.points[0],level:0},RoadNode{id:"b".into(),position:*road.points.last().unwrap(),level:0}];
    d.roads.push(road);
    assert!(pack_bytes(d.clone(),BTreeMap::new()).is_ok());
    d.roads[0].design.as_mut().unwrap().terrain_policy=road_design::TerrainPolicy::Preserve;
    let original=d.clone();
    let e=pack_bytes(d.clone(),BTreeMap::new()).unwrap_err();
    assert_eq!(e.code,"E_ROAD_ENVIRONMENT");assert!(e.message.contains("authored at ["));assert_eq!(d,original);
    for p in &mut d.roads[0].design.as_mut().unwrap().control_points {p[1]+=600;}
    let design=d.roads[0].design.clone().unwrap();d=road_design::edit(&d,"authored",design,600).unwrap();
    assert!(pack_bytes(d,BTreeMap::new()).is_ok());
}
