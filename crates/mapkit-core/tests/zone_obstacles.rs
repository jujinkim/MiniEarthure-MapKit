use mapkit_core::*;

#[test]
fn custom_canopies_avoid_roads_buildings_and_authored_placements() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    let road = d.roads[0].clone();
    d.recipe_version=7; d.cell_size_cm=3200; d.bounds=Bounds { min:[0,0],max:[3200,3200] };
    d.nodes.clear();d.roads.clear();d.buildings.clear();d.zones.clear();d.assets.clear();d.placements.clear();
    d.nodes.push(RoadNode { id:"a".into(),position:[0,0,1000],level:0 });
    d.nodes.push(RoadNode { id:"b".into(),position:[3200,0,1000],level:0 });
    d.roads.push(Road { id:"road".into(), from:"a".into(),to:"b".into(),points:vec![[0,0,1000],[3200,0,1000]],widths_cm:vec![200],surfaces:vec![Surface::Asphalt],kind:RoadKind::Ground,sidewalk_cm:Some(0),clearance_cm:None,..road });
    d.assets.push(Asset { id:"tree".into(),path:"tree.glb".into(),
        attribution:Attribution { source:"synthetic".into(),license:"MIT".into(),notice:"Original fixture".into() },
        collision:vec![CollisionBox { center:[0,100,0],size_cm:[100,200,100] }],convex_collision:vec![],material:None });
    d.placements.push(Placement { id:"fixture".into(),asset_id:"tree".into(),position:[1000,0,2000],quarter_turns:0 });
    d.buildings.push(Building { id:"building".into(),footprint:vec![[1900,1900],[2100,1900],[2100,2100],[1900,2100]],holes:vec![],base_cm:0,height_cm:300,usage:"public".into(),material:"concrete".into(),roof:"flat".into(),entrances:vec![] });
    d.zones.push(Zone { id:"garden".into(),polygon:vec![[100,100],[3100,100],[3100,3100],[100,3100]],kind:ZoneKind::Orchard,spacing_cm:500,density_per_mille:1000,exclusions:vec![],tree:Some(ZoneTree { asset_id:"tree".into(),radius_cm:150,clearance_cm:50 }) });
    let g=generate(GenerationInput { document:&d,cell:Cell { x:0,y:0 },heightgrid:None,max_triangles:500_000 }).unwrap();
    let trees:Vec<_>=g.objects.iter().filter(|p| p.id.starts_with("garden:")).collect();
    assert!(!trees.is_empty());
    for p in trees {
        assert!((p.position[2]-1000).abs()>=300,"road and its clearance");
        for [x,y] in [[1000,2000],[2000,2000]] {
            assert!((p.position[0]-x).abs()>200 || (p.position[2]-y).abs()>200,"authored obstacle");
        }
    }
}
