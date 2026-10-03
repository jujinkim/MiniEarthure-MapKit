use mapkit_core::{assembled_track::{self as track, authoring::*}, assembled_preview, Vertex};
fn dot(a:[f64;3],b:[f64;3])->f64 {(0..3).map(|i|a[i]*b[i]).sum()}
fn sub(a:[f64;3],b:[f64;3])->[f64;3] {std::array::from_fn(|i|a[i]-b[i])}
fn cross(a:[f64;3],b:[f64;3])->[f64;3] {[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn world(v:Vertex,g:&mapkit_core::gimmick::Gimmick)->[f64;3] {
    let mut v=v.map(|x|x as f64*0.1);
    for (axis,a,b) in [(2,0,1),(0,1,2),(1,2,0)] {
        let t=(g.rotation_mdeg[axis] as f64/1000.0).to_radians();
        let (x,y)=(v[a],v[b]);v[a]=t.cos()*x-t.sin()*y;v[b]=t.sin()*x+t.cos()*y;
    }
    std::array::from_fn(|i|v[i]+g.position[i] as f64)
}
#[test]
fn actual_road_facets_remain_below_all_panel_tops() {
    for preset in ["straight","slope_up","curve_up","gentle90","spiral90_left_up"] {
        let mut source=Source::empty();let mut road=instance("road",preset,600);
        road.entry_width_cm=400;road.exit_width_cm=800;
        road.position_cm=[137,230,-191];road.rotation_mdeg=[13000,27000,8000];
        let p=piece(&road).unwrap();let sample=p.path.len()/2;
        source.instances.push(road);
        source.actions.push(Action {id:"panel".into(),kind:"jump_panel".into(),piece:"road".into(),sample,height_cm:200,panel_width_percent:50,panel_alignment:PanelAlignment::Center,landing:None});
        let document=track::document_from_assembly(compile(&source).unwrap()).unwrap();
        let geometry=assembled_preview(&document).unwrap();
        let g=document.gimmicks.iter().find(|g|g.id=="action-panel").unwrap();
        for part in &g.parts {
            let top:[_;3]=std::array::from_fn(|i|world(part.vertices[i],g));
            let centroid=std::array::from_fn(|j|(top[0][j]+top[1][j]+top[2][j])/3.0);
            let matched=geometry.triangles.iter().filter(|t|t.object_id=="assembled-road-0").any(|t| {
                let v=t.vertices.map(|p|p.map(|x|x as f64));
                let n=cross(sub(v[1],v[0]),sub(v[2],v[0]));let len=dot(n,n).sqrt();if len<1e-8 {return false;}
                let n=n.map(|x|x/len);
                if top.iter().any(|p|(dot(n,sub(*p,v[0]))-3.0).abs()>0.09) {return false;}
                (0..3).all(|i|dot(cross(sub(v[(i+1)%3],v[i]),sub(centroid,v[i])),n)>=-0.1)
            });
            assert!(matched,"{preset}: lifted top has no actual support facet: {top:?}");
        }
    }
}

#[test]
fn unsupported_action_anchor_is_rejected() {
    let mut source=Source::empty();let mut road=instance("gap","flight_curve",400);
    road.control_points=vec![[0,0,0],[0,100,200],[0,100,400],[0,0,600]];
    source.instances.push(road);
    source.actions.push(Action {id:"pad".into(),kind:"jump_panel".into(),piece:"gap".into(),sample:0,height_cm:200,panel_width_percent:50,panel_alignment:PanelAlignment::Center,landing:None});
    assert_eq!(compile(&source).unwrap_err().code,"E_TRACK_PANEL_SUPPORT");
}
