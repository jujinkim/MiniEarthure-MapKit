use mapkit_core::*;
fn document() -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.placements.clear(); d.assets.clear(); d.repetitions.clear(); d.heightmaps.clear();
    d.bounds = Bounds{min:[0,0],max:[6400,3200]}; d.cell_size_cm=3200;
    d.water_bodies = vec![water::WaterBody{id:"lake".into(),polygon:vec![[100,100],[6200,100],[6200,3000],[100,3000]],islands:vec![vec![[1000,1000],[2000,1000],[2000,2000],[1000,2000]]],surface_cm:200,bottom_cm:-1000,flow_cm_s:[20,0]}];
    d
}
#[test]
fn water_is_non_solid_and_spawn_query_respects_islands_bridge_and_bottom() {
    let d=document(); d.validate().unwrap();
    let mut c=generate(GenerationInput{document:&d,cell:Cell{x:0,y:0},heightgrid:None,max_triangles:20000}).unwrap();
    assert_eq!(c.water_bodies.len(),1);
    assert!(!c.water_bodies[0].surface.is_empty());
    assert!(c.triangles.iter().all(|t| t.vertices.iter().all(|p| p[1] != 200)));
    assert!(c.spawn_options([500,500]).unwrap().is_empty());
    assert!(!c.spawn_options([1500,1500]).unwrap().is_empty());
    assert!(!c.spawn_options([1000,1500]).unwrap().is_empty()); // closed dry island
    assert!(water::sample(&d.water_bodies,[500,-1001,500]).is_none());
    assert!(water::sample(&d.water_bodies,[500,201,500]).is_none());
    assert!(water::sample(&d.water_bodies,[100,200,500]).is_some());
    c.triangles.push(Triangle{vertices:[[100,400,100],[900,400,100],[100,400,900]],surface:Surface::Concrete,object_id:"bridge".into(),spawnable:true});
    assert_eq!(c.spawn_options([200,200]).unwrap()[0].surface_id,"bridge");
    assert_eq!(c.hash().unwrap(),sha256(&canonical(&c).unwrap()));
}
#[test]
fn water_seams_region_cost_archive_and_hash_are_deterministic() {
    let d=document(); let p=PreparedMap::new(d.clone()).unwrap();
    for cell in d.cells() {
        let c=p.generate(cell,None,20000).unwrap();
        let cost=p.estimate(cell,20000).unwrap();
        assert!(cost.water_bytes >= canonical(&c.water_bodies).unwrap().len() as u64);
        let key=archive_key("water-test",cell);
        let bytes=encode_archive(&c,&key,archive_limit(&cost)).unwrap();
        assert_eq!(decode_archive(&bytes,&key,cell,&cost).unwrap(),c);
        assert!(decode_archive(&bytes[..bytes.len()-1],&key,cell,&cost).is_err());
        let local=region_source(&d,CellRegion{min:cell,end:Cell{x:cell.x+1,y:cell.y+1}}).unwrap();
        assert_eq!(generate(GenerationInput{document:&local,cell,heightgrid:None,max_triangles:20000}).unwrap(),c);
        let bounds=d.cell_bounds(cell).unwrap();
        assert!(c.water_bodies[0].surface.iter().flatten().all(|v| bounds.contains([v[0],v[2]])));
        assert!(c.water_bodies[0].body.contains([3200,100,500]));
    }
    let mut changed=d.clone(); changed.water_bodies[0].flow_cm_s=[30,0];
    assert_ne!(PreparedMap::new(changed).unwrap().generate(Cell{x:0,y:0},None,20000).unwrap().hash().unwrap(),p.generate(Cell{x:0,y:0},None,20000).unwrap().hash().unwrap());
}
#[test]
fn water_rejects_invalid_islands_heights_flow_and_shared_ids() {
    let d=document();
    for mutation in 0..4 { let mut bad=d.clone(); match mutation {
        0=>bad.water_bodies[0].bottom_cm=201,
        1=>bad.water_bodies[0].flow_cm_s=[1001,0],
        2=>bad.water_bodies[0].islands.push(vec![[0,0],[300,0],[300,300]]),
        _=>bad.water_bodies.push(bad.water_bodies[0].clone()),
    }; assert!(bad.validate().is_err()); }
    let mut bodies=d.water_bodies.clone(); let mut upper=bodies[0].clone(); upper.id="upper".into();upper.bottom_cm=400;upper.surface_cm=600;bodies.push(upper);
    assert_eq!(water::sample(&bodies,[500,500,500]).unwrap().id,"upper");
    assert!(water::sample(&bodies,[500,300,500]).is_none());
}
