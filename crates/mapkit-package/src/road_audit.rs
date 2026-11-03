//! Collision regression sampled independently of the terrain mesh at 0.5 m.
use crate::*;
pub(crate) fn height(v:[Vertex;3],p:[f64;2])->Option<f64> {
    height_with_margin(v,p,-1e-8)
}
fn height_with_margin(v:[Vertex;3],p:[f64;2],margin:f64)->Option<f64> {
    let [a,b,c]=v.map(|p|p.map(|v|v as f64));
    let det=(b[2]-c[2])*(a[0]-c[0])+(c[0]-b[0])*(a[2]-c[2]);
    if det.abs()<0.0001 {return None;}
    let u=((b[2]-c[2])*(p[0]-c[0])+(c[0]-b[0])*(p[1]-c[2]))/det;
    let v=((c[2]-a[2])*(p[0]-c[0])+(a[0]-c[0])*(p[1]-c[2]))/det;
    (u>=margin && v>=margin && u+v<=1.0-margin).then_some(u*a[1]+v*b[1]+(1.0-u-v)*c[1])
}
/// Presentation drops paint over the shader cap; authoring keeps it an error.
fn paint_budget(d:&MapDocument,bounds:&Bounds)->Result<()> {
    let paint=d.road_paint(bounds)?;let cap=mapkit_core::RoadPaint::SEGMENTS;
    if let Some((id,count))=paint.overflow.iter().next() {
        return Err(error("E_BUDGET",format!("{id} has {count} local path/exterior segments, over {cap}")));
    }
    for r in d.roads.iter().filter(|r|paint.paths.contains_key(&r.id)) {
        if let (_,Some(count))=paint.joined(d,r) {return Err(error("E_BUDGET",format!("{} has {count} joined local path segments, over {cap}",r.id)));}
    }
    Ok(())
}
pub fn audit(package:&Package,max_grade:f64)->Result<serde_json::Value> {
    let d=&package.document;
    let mut queries:BTreeMap<Cell,Vec<(String,[f64;2],f64)>>=BTreeMap::new();
    let mut grade=0.0f64;
    for road in &d.roads {
        if road.design.is_none(){return Err(error("E_ROAD_AUDIT",format!("{} has no authored profile",road.id)));}
        let controls=&road.design.as_ref().unwrap().control_points;
        for at in (0..controls.len()-1).step_by(3) {for step in 0..=128 {
            let (_,v)=mapkit_core::road_design::evaluate(&controls[at..at+4],step as f64/128.0);
            grade=grade.max(v[1].abs()/libm::hypot(v[0],v[2]));
        }}
        let path=mapkit_core::road_design::path(road)?;
        let mut station=0.0;
        let mut next=0.0;
        for pair in path.windows(2) {
            let a=pair[0].position_cm.map(|v|v as f64);let b=pair[1].position_cm.map(|v|v as f64);
            let length=libm::hypot(b[0]-a[0],b[2]-a[2]);
            while next<=station+length {
                let t=((next-station)/length).clamp(0.0,1.0);
                let center:[f64;3]=std::array::from_fn(|j|a[j]+(b[j]-a[j])*t);
                let ribbons=[pair[0].ribbon_cm.unwrap(),pair[1].ribbon_cm.unwrap()];
                for lateral in [-0.9,-0.45,0.0,0.45,0.9] {
                    let p=std::array::from_fn(|j|{
                        let left=ribbons[0][0][j*2] as f64*(1.0-t)+ribbons[1][0][j*2] as f64*t;
                        center[j*2]+(left-center[j*2])*lateral
                    });
                    if let Some(cell)=d.cell_at([libm::floor(p[0]) as i64,libm::floor(p[1]) as i64]) {queries.entry(cell).or_default().push((road.id.clone(),p,center[1]));}
                }
                next+=50.0;
            }
            station+=length;
        }
    }
    if grade>max_grade {return Err(error("E_ROAD_GRADE",format!("maximum design grade {grade:.5} exceeds {max_grade}")));}
    let mut count=0u64;let mut max_error=0.0f64;
    for (cell,points) in &queries {
        cancellation::checkpoint()?;
        paint_budget(d,&d.cell_bounds(*cell)?).map_err(|mut e|{e.message=format!("road paint cell {cell:?}: {}",e.message);e})?;
        let actual=package.generate(*cell,500_000).map_err(|mut e|{e.message=format!("road audit cell {cell:?}: {}",e.message);e})?;
        let reference=mapkit_core::road_design::design_triangles(d,&d.cell_bounds(*cell)?)?;
        let mut probes=points.clone();
        for (id,v) in &reference {
            let center:[f64;3]=std::array::from_fn(|j|v.iter().map(|p|p[j] as f64).sum::<f64>()/3.0);
            if d.cell_at([libm::floor(center[0]) as i64,libm::floor(center[2]) as i64])!=Some(*cell) {continue;}
            if d.nodes.iter().any(|n|libm::hypot(center[0]-n.position[0] as f64,center[2]-n.position[2] as f64)<1500.0) {
                probes.push((id.clone(),[center[0],center[2]],center[1]));
            }
        }
        for (id,p,profile) in &probes {
            let expected:Vec<_>=reference.iter().filter_map(|(_,v)|height(*v,*p)).filter(|h|(h-profile).abs()<200.0).collect();
            if expected.is_empty(){
                // The exact junction footprint trims nominal endpoint ribbons.
                // Its actual fans are sampled above, independently of each arm.
                let road=d.roads.iter().find(|r|r.id==*id).unwrap();
                if [road.points[0],*road.points.last().unwrap()].iter().any(|n|libm::hypot(p[0]-n[0] as f64,p[1]-n[2] as f64)<road.widths_cm[0] as f64*1.5) {continue;}
                return Err(error("E_ROAD_AUDIT",format!("{id} at {p:?}: design ribbon has a gap")));
            }
            let mut hits:Vec<_>=actual.triangles.iter().filter(|t|t.spawnable && d.roads.iter().any(|r|r.id==t.object_id)).filter_map(|t|height(t.vertices,*p)).filter(|h|(h-profile).abs()<200.0).collect();
            hits.sort_by(f64::total_cmp);hits.dedup_by(|a,b|(*a-*b).abs()<0.01);
            if hits.is_empty(){return Err(error("E_ROAD_AUDIT",format!("{id} at {p:?}: collision ribbon has a gap")));}
            for h in &hits {
                let deviation=expected.iter().map(|e|(e-h).abs()).fold(f64::INFINITY,f64::min);
                max_error=max_error.max(deviation);
                if deviation>1.00001 {return Err(error("E_ROAD_AUDIT",format!("{id} at {p:?}: collision differs by {deviation:.4} cm")));}
            }
            if hits.last().unwrap()-hits[0]>1.00001 {return Err(error("E_ROAD_AUDIT",format!("{id} at {p:?}: incompatible collision heights {hits:?}")));}
            for t in actual.triangles.iter().filter(|t|t.spawnable && t.object_id=="terrain") {
                if height_with_margin(t.vertices,*p,1e-8).is_some_and(|h|(h-hits[0]).abs()<50.0) {return Err(error("E_ROAD_AUDIT",format!("{id} at {p:?}: duplicate terrain contact")));}
            }
            count+=1;
        }
    }
    Ok(serde_json::json!({"sample_spacing_cm":50,"width_lines":5,"collision_samples":count,"road_cells":queries.len(),"max_design_deviation_cm":max_error,"max_sampled_grade":grade}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paint_over_the_shader_cap_stays_an_audit_error() {
        let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        d.roads.truncate(1);d.nodes.clear();d.buildings.clear();d.zones.clear();
        d.bounds=Bounds{min:[0,0],max:[20000,10000]};
        let wiggle:Vec<Vertex>=(0..=300).map(|i|{let x=2000+i*50;[x,0,5000+(200.0*(x as f64*std::f64::consts::TAU/2000.0).sin()).round() as i64]}).collect();
        let n=wiggle.len()-1;
        let r=&mut d.roads[0];
        r.points=wiggle;r.widths_cm=vec![400;n];r.surfaces=vec![Surface::Asphalt;n];r.design=None;r.from="a".into();r.to="b".into();
        r.markings=Some(RoadMarkings{color:None,lanes:2,center_line:true,edge_lines:true,crosswalk_start:false,crosswalk_end:false,one_way:0,stop_lines:false,lane_width_cm:0});
        for (id,p) in [("a",r.points[0]),("b",*r.points.last().unwrap())] {d.nodes.push(RoadNode{id:id.into(),position:p,level:0});}
        assert!(!d.road_paint(&d.bounds).unwrap().overflow.is_empty(),"presentation degrades");
        let e=paint_budget(&d,&d.bounds).unwrap_err();
        assert_eq!(e.code,"E_BUDGET");assert!(e.message.contains("local path/exterior segments"),"{}",e.message);
        d.roads[0].markings=None;
        paint_budget(&d,&d.bounds).unwrap();
        // Two degree-two halves within the cap whose joined path is over it.
        d.roads[0].markings=Some(RoadMarkings{color:None,lanes:2,center_line:true,edge_lines:false,crosswalk_start:false,crosswalk_end:false,one_way:0,stop_lines:false,lane_width_cm:0});
        let mut b=d.roads[0].clone();
        let half=b.points.len()/2;
        b.id="b".into();b.from="m".into();b.points.drain(..half);b.widths_cm.truncate(b.points.len()-1);b.surfaces.truncate(b.points.len()-1);
        let a=&mut d.roads[0];a.to="m".into();a.points.truncate(half+1);a.widths_cm.truncate(half);a.surfaces.truncate(half);
        d.nodes.push(RoadNode{id:"m".into(),position:b.points[0],level:0});d.nodes[1].position=*b.points.last().unwrap();
        d.roads.push(b);
        let paint=d.road_paint(&d.bounds).unwrap();
        assert!(paint.overflow.is_empty()&&paint.paths.values().all(|p|p.len()<=RoadPaint::SEGMENTS));
        assert!(paint.joined(&d,&d.roads[0]).1.is_some(),"{:?}",paint.paths.values().map(Vec::len).collect::<Vec<_>>());
        let e=paint_budget(&d,&d.bounds).unwrap_err();
        assert_eq!(e.code,"E_BUDGET");assert!(e.message.contains("joined local path segments"),"{}",e.message);
    }
    #[test]
    fn audit_reports_a_designed_road_over_the_paint_cap() {
        let (mut d,files)=crate::read_project(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"),"/../../examples/roads"))).unwrap();
        d.roads.truncate(1);d.nodes.clear();d.buildings.clear();d.zones.clear();
        let wiggle:Vec<Vertex>=(0..=300).map(|i|{let x=500+i*30;[x,0,2500+(300.0*(x as f64*std::f64::consts::TAU/1000.0).sin()).round() as i64]}).collect();
        let r=&mut d.roads[0];
        r.points=wiggle.clone();r.from="a".into();r.to="b".into();r.kind=RoadKind::Ground;
        r.markings=Some(RoadMarkings{color:None,lanes:2,center_line:true,edge_lines:false,crosswalk_start:false,crosswalk_end:false,one_way:0,stop_lines:false,lane_width_cm:0});
        mapkit_core::road_design::compile(r,mapkit_core::road_design::from_points(&wiggle).unwrap(),400).unwrap();
        for (id,p) in [("a",r.points[0]),("b",*r.points.last().unwrap())] {d.nodes.push(RoadNode{id:id.into(),position:p,level:0});}
        let (package,_)=Profile::Authoring.run(||crate::Package::from_project(d,files));
        let e=audit(&package.unwrap(),0.12).unwrap_err();
        assert_eq!(e.code,"E_BUDGET");assert!(e.message.starts_with("road paint cell"),"{}",e.message);
    }
}
