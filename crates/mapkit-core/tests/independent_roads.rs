use mapkit_core::*;
use mapkit_core::road_design::{self, Design, TerrainPolicy};

fn fixture(rise: i64) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.bounds = Bounds {min:[0,0],max:[6400,3200]};
    d.cell_size_cm=3200;
    d.roads.truncate(1);
    d.nodes.truncate(2);
    d.buildings.clear(); d.placements.clear(); d.repetitions.clear(); d.zones.clear();
    let r=&mut d.roads[0];
    r.id="designed".into(); r.kind=RoadKind::Ground; r.from="a".into(); r.to="b".into(); r.sidewalk_cm=Some(0);
    let design=Design {control_points:vec![[0,100,1600],[2000,100,1600],[4400,100+rise,1600],[6400,100+rise,1600]],terrain_policy:TerrainPolicy::AutoFit,shoulder_cm:600};
    road_design::compile(r,design,600).unwrap();
    d.nodes=vec![RoadNode{id:"a".into(),position:r.points[0],level:0},RoadNode{id:"b".into(),position:*r.points.last().unwrap(),level:0}];
    for x in 0..2 {d.heightmaps.push(Heightmap {cell:Cell{x,y:0},path:format!("h{x}.png"),spacing_cm:200,offset_cm:0,step_cm:1,source_accuracy_cm:None});}
    d
}

#[test]
fn actual_collision_ignores_short_terrain_bumps_across_width_and_cell_seam() {
    for rise in [0,500] {
        let d=fixture(rise);
        let grid=HeightGrid {side:17,heights_cm:(0..289).map(|i|if i%2==0 {170} else {-130}).collect()};
        let original=grid.heights_cm.clone();
        let chunks:Vec<_>=(0..2).map(|x|generate(GenerationInput {document:&d,cell:Cell{x,y:0},heightgrid:Some(&grid),max_triangles:200000}).unwrap()).collect();
        for x in (0..6400).step_by(50) {for z in [1320,1460,1600,1740,1880] {
            let c=&chunks[usize::from(x>=3200)];
            let p=c.spawn(&SpawnRequest {position_cm:[x,z],surface_id:"designed".into()}).unwrap();
            // Invert the analytic x(t), independently of generated triangles.
            let controls=&d.roads[0].design.as_ref().unwrap().control_points;
            let (mut lo,mut hi)=(0.0,1.0);
            for _ in 0..50 {let mid=(lo+hi)/2.0;if road_design::evaluate(controls,mid).0[0]<(x as f64) {lo=mid;} else {hi=mid;}}
            let expected=road_design::evaluate(controls,(lo+hi)/2.0).0[1];
            let actual=c.triangles.iter().filter(|f| f.object_id=="designed").find_map(|f| {
                let [a,b,c]=f.vertices.map(|p|p.map(|v|v as f64));
                let det=(b[2]-c[2])*(a[0]-c[0])+(c[0]-b[0])*(a[2]-c[2]);
                let u=((b[2]-c[2])*(x as f64-c[0])+(c[0]-b[0])*(z as f64-c[2]))/det;
                let v=((c[2]-a[2])*(x as f64-c[0])+(a[0]-c[0])*(z as f64-c[2]))/det;
                (u>=-1e-8 && v>=-1e-8 && u+v<=1.0+1e-8).then_some(u*a[1]+v*b[1]+(1.0-u-v)*c[1])
            }).unwrap();
            assert!((actual-expected).abs()<=1.0,"x={x} z={z}: {p:?}, collision={actual}, design={expected}");
            assert!(c.spawn(&SpawnRequest {position_cm:[x,z],surface_id:"terrain".into()}).is_err());
        }}
        for z in (1320..1880).step_by(50) {
            let r=SpawnRequest{position_cm:[3200,z],surface_id:"designed".into()};
            assert_eq!(chunks[0].spawn(&r).unwrap(),chunks[1].spawn(&r).unwrap());
        }
        assert_eq!(grid.heights_cm,original);
    }
}

#[test]
fn delete_restores_original_terrain_and_preserve_policy_does_not_fit_shoulders() {
    let mut d=fixture(0);
    let grid=HeightGrid {side:17,heights_cm:vec![-120;289]};
    let build=|d:&MapDocument|generate(GenerationInput{document:d,cell:Cell{x:0,y:0},heightgrid:Some(&grid),max_triangles:200000}).unwrap();
    let p=SpawnRequest {position_cm:[1600,1100],surface_id:"terrain".into()};
    assert!(build(&d).spawn(&p).unwrap()[1]>-120);
    d.roads[0].design.as_mut().unwrap().terrain_policy=TerrainPolicy::Preserve;
    assert_eq!(build(&d).spawn(&p).unwrap()[1],-120);
    d.roads.clear();
    assert_eq!(build(&d).spawn(&SpawnRequest{position_cm:[1600,1600],..p}).unwrap()[1],-120);
}

#[test]
fn stale_samples_and_unbounded_authoring_are_rejected() {
    let mut d=fixture(0);d.roads[0].points[1][1]+=10;
    assert_eq!(road_design::verify(&d.roads[0]).unwrap_err().code,"E_ROAD_DESIGN");
    let mut design=d.roads[0].design.clone().unwrap();design.control_points=vec![[0;3];3076];
    assert_eq!(road_design::samples(&design).unwrap_err().code,"E_ROAD_DESIGN");
}

#[test]
fn elevated_placement_keeps_a_separate_ground_level() {
    let mut d=fixture(0);d.heightmaps.clear();
    let mut design=d.roads[0].design.clone().unwrap();
    for p in &mut design.control_points {p[1]=1000;}
    design.terrain_policy=TerrainPolicy::Elevated;
    d=road_design::edit(&d,"designed",design.clone(),600).unwrap();
    assert_eq!(d.roads[0].kind,RoadKind::Elevated);
    let build=|d:&MapDocument|generate(GenerationInput{document:d,cell:Cell{x:0,y:0},heightgrid:None,max_triangles:200000}).unwrap();
    let point=SpawnRequest{position_cm:[1600,1600],surface_id:"terrain".into()};
    assert_eq!(build(&d).spawn(&point).unwrap()[1],d.terrain_base_cm);
    assert_eq!(build(&d).spawn(&SpawnRequest{surface_id:"designed".into(),..point.clone()}).unwrap()[1],1000);
    design.terrain_policy=TerrainPolicy::AutoFit;d=road_design::edit(&d,"designed",design,600).unwrap();
    assert_eq!(d.roads[0].kind,RoadKind::Ground);assert!(build(&d).spawn(&point).is_err());
}

#[test]
fn dense_curved_sidewalks_stay_within_existing_cell_work_and_geometry_budgets() {
    let mut d=fixture(0);
    let design=Design{control_points:vec![[0,100,400],[2800,100,400],[3600,100,2800],[6400,100,2800]],terrain_policy:TerrainPolicy::AutoFit,shoulder_cm:600};
    road_design::compile(&mut d.roads[0],design,600).unwrap();
    d.roads[0].sidewalk_cm=Some(200);
    d.roads[0].markings=Some(RoadMarkings{lanes:2,center_line:true,edge_lines:true,crosswalk_start:false,crosswalk_end:false,color:None});
    d.nodes[0].position=d.roads[0].points[0];d.nodes[1].position=*d.roads[0].points.last().unwrap();
    let grid=HeightGrid{side:17,heights_cm:(0..289).map(|i|i as i64%17*3).collect()};
    let prepared=PreparedMap::new(d).unwrap();
    for x in 0..2 {
        let cell=Cell{x,y:0};let cost=prepared.estimate(cell,200000).unwrap();
        let chunk=prepared.generate(cell,Some(&grid),200000).unwrap();
        let paint=prepared.road_paint(&prepared.cell_bounds(cell).unwrap()).unwrap();
        assert!(!paint.paths["designed"].is_empty() && paint.paths["designed"].len()<=128);
        assert!(!paint.edges["designed"].is_empty() && paint.edges["designed"].len()<=128);
        assert!(chunk.triangles.iter().any(|t|t.object_id=="designed:sidewalk" && t.spawnable));
        assert!(chunk.triangles.len() as u64<=cost.triangles);
    }
}

#[test]
fn shared_surface_actions_follow_edits_and_use_real_road_triangles() {
    use assembled_track::{surface,authoring::PanelAlignment};
    let mut d=fixture(500);d.bounds=Bounds{min:[-6400,-6400],max:[12800,12800]};
    let attachment=surface::Attachment{id:"jump".into(),surface:surface::Reference{surface_id:"designed".into(),station_cm:2400},kind:"jump_panel".into(),height_cm:200,panel_width_percent:50,panel_alignment:PanelAlignment::Center,side:1};
    let d=surface::apply(&d,vec![attachment]).unwrap();
    let original=d.gimmicks[0].position;
    assert!(!d.gimmicks[0].parts.is_empty());
    let mut design=d.roads[0].design.clone().unwrap();
    for p in &mut design.control_points {p[1]+=200;}
    let edited=road_design::edit(&d,"designed",design,600).unwrap();
    assert_eq!(edited.gimmicks[0].position,[original[0],original[1]+200,original[2]]);
    assert_eq!(edited.heightmaps,d.heightmaps);
    let saved=canonical(&edited).unwrap();
    let reopened:MapDocument=serde_json::from_slice(&saved).unwrap();
    reopened.validate().unwrap();assert_eq!(reopened,edited);
}

#[test]
fn shared_corner_rail_moves_collision_and_interaction_together() {
    use assembled_track::{surface,authoring::PanelAlignment};
    let mut d=fixture(0);d.bounds=Bounds{min:[-6400,-6400],max:[12800,12800]};d.heightmaps.clear();
    let design=Design{control_points:vec![[0,100,0],[552,100,0],[1000,100,448],[1000,100,1000]],terrain_policy:TerrainPolicy::AutoFit,shoulder_cm:600};
    d=road_design::edit(&d,"designed",design,1200).unwrap();
    let item=surface::Attachment{id:"corner-rail-with-a-long-user-chosen-name".into(),surface:surface::Reference{surface_id:"designed".into(),station_cm:785},kind:"grind_rail".into(),height_cm:200,panel_width_percent:100,panel_alignment:PanelAlignment::Center,side:1};
    let d=surface::apply(&d,vec![item]).unwrap();assert_eq!(d.grind_lines.len(),1);
    let old=d.grind_lines[0].control_points.clone();
    let mut design=d.roads[0].design.clone().unwrap();for p in &mut design.control_points{p[1]+=100;}
    let edited=road_design::edit(&d,"designed",design,1200).unwrap();surface::verify(&edited).unwrap();
    assert_eq!(edited.grind_lines[0].control_points,old.iter().map(|p|[p[0],p[1]+100,p[2]]).collect::<Vec<_>>());
    let cleared=surface::apply(&edited,vec![]).unwrap();assert!(cleared.grind_lines.is_empty() && cleared.gimmicks.is_empty());
}

#[test]
fn ordinary_road_ports_realign_connected_tracks_atomically() {
    use assembled_track::{authoring as a,composite,surface};
    let mut d=fixture(0);d.bounds=Bounds{min:[-6400,-6400],max:[19200,12800]};d.heightmaps.clear();d.free_roam=true;
    let mut source=a::Source::empty();source.instances.push(a::instance("entry","straight",400));source.instances.push(a::instance("next","straight",400));
    source.connections.push(a::Connection{from:"entry".into(),to:"next".into()});
    source.road_connections.push(a::RoadConnection{road:"designed".into(),start:false,instance:"entry".into()});
    let d=composite::apply_source(&d,&source).unwrap();
    composite::verify_connections(&d).unwrap();
    assert_eq!(d.assembled_track.as_ref().unwrap().pieces[0].path[0].position_cm,*d.roads[0].points.last().unwrap());
    let attachment=surface::Attachment{id:"near".into(),surface:surface::Reference{surface_id:"designed".into(),station_cm:2000},kind:"acceleration_panel".into(),height_cm:200,panel_width_percent:50,panel_alignment:a::PanelAlignment::Center,side:1};
    let d=surface::apply(&d,vec![attachment]).unwrap();
    let mut design=d.roads[0].design.clone().unwrap();for p in &mut design.control_points{p[1]+=200;}
    let edited=road_design::edit(&d,"designed",design,600).unwrap();
    surface::verify(&edited).unwrap();composite::verify_connections(&edited).unwrap();
    assert_eq!(edited.assembled_track.as_ref().unwrap().pieces[0].path[0].position_cm[1],300);
    assert_eq!(edited.assembled_track.as_ref().unwrap().pieces[1].path[0].position_cm[1],300);
    assert_eq!(d.roads[0].points[0][1],100);
}

#[test]
fn explicit_junctions_and_separated_crossings_do_not_pull_heights() {
    let mut d=fixture(0);d.heightmaps.clear();d.bounds.max=[6400,6400];
    let mut r=d.roads[0].clone();r.id="cross".into();r.from="c".into();r.to="e".into();
    let mut design=road_design::from_points(&[[3200,100,0],[3200,100,6400]]).unwrap();
    road_design::compile(&mut r,design.clone(),600).unwrap();
    d.nodes.extend([RoadNode{id:"c".into(),position:r.points[0],level:0},RoadNode{id:"e".into(),position:*r.points.last().unwrap(),level:0}]);d.roads.push(r);
    assert_eq!(d.validate().unwrap_err().code,"E_ROAD_INTERSECTION");
    for p in &mut design.control_points{p[1]+=600;}
    design.terrain_policy=TerrainPolicy::Elevated;d.roads[1].kind=RoadKind::Bridge;
    road_design::compile(&mut d.roads[1],design,600).unwrap();d.nodes[2].position[1]=700;d.nodes[3].position[1]=700;
    d.validate().unwrap();
    let c=generate(GenerationInput{document:&d,cell:Cell{x:1,y:0},heightgrid:None,max_triangles:200000}).unwrap();
    assert_eq!(c.spawn(&SpawnRequest{position_cm:[3200,1600],surface_id:"designed".into()}).unwrap()[1],100);
    assert_eq!(c.spawn(&SpawnRequest{position_cm:[3200,1600],surface_id:"cross".into()}).unwrap()[1],700);
}
