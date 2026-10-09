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
        d.road_paint(&d.cell_bounds(*cell)?).map_err(|mut e|{e.message=format!("road paint cell {cell:?}: {}",e.message);e})?;
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
