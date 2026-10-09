//! Independent ordinary-road alignment and reversible terrain fitting. All
//! coordinates are source centimetres; heightmaps remain immutable inputs.
use crate::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerrainPolicy { AutoFit, Preserve, Elevated }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Design {
    /// Joined cubic Bezier spans: 3*n+1 controls, including both road nodes.
    pub control_points: Vec<Vertex>,
    pub terrain_policy: TerrainPolicy,
    pub shoulder_cm: u32,
}

/// Explicit opt-in from a polyline. This does not run while opening a project.
pub fn from_points(points: &[Vertex]) -> Result<Design> {
    if points.len()<2 || points.len()>1025 {return Err(error("E_ROAD_DESIGN","2..1025 authoring anchors required"));}
    let tangents:Vec<Vertex>=(0..points.len()).map(|i| {
        let p=points[i];let a=points[i.saturating_sub(1)];let b=points[(i+1).min(points.len()-1)];
        let mut tangent:Vertex=std::array::from_fn(|j|(b[j]-a[j])/6);
        let limit=libm::hypot((p[0]-a[0]) as f64,(p[2]-a[2]) as f64).min(libm::hypot((b[0]-p[0]) as f64,(b[2]-p[2]) as f64))/3.0;
        let horizontal=libm::hypot(tangent[0] as f64,tangent[2] as f64);
        if i>0 && i+1<points.len() && horizontal>limit && horizontal>0.0 {tangent=tangent.map(|v|libm::round(v as f64*limit/horizontal) as i64);}
        if i==0 || i+1==points.len() {for j in [0,2] {tangent[j]=(b[j]-a[j])/3;} tangent[1]=0;}
        else {
            let cap=(p[1]-a[1]).abs().min((b[1]-p[1]).abs())/2;
            tangent[1]=if (p[1]-a[1]).signum()!=(b[1]-p[1]).signum() {0} else {tangent[1].clamp(-cap,cap)};
        }
        tangent
    }).collect();
    let mut controls=vec![points[0]];
    for i in 0..points.len()-1 {
        let (a,b)=(points[i],points[i+1]);
        controls.extend([std::array::from_fn(|j|a[j]+tangents[i][j]),std::array::from_fn(|j|b[j]-tangents[i+1][j]),b]);
    }
    Ok(Design{control_points:controls,terrain_policy:TerrainPolicy::AutoFit,shoulder_cm:2400})
}

/// One graph edit includes shared nodes and the incident road ends. Unchanged
/// scenery and source heightmaps are retained byte-for-byte by the caller.
pub fn edit(document: &MapDocument, id: &str, design: Design, width: u32) -> Result<MapDocument> {
    let mut d=document.clone();
    let at=d.roads.iter().position(|r|r.id==id).ok_or_else(||error("E_REFERENCE","road missing"))?;
    let ends=[(d.roads[at].from.clone(),design.control_points.first().copied()),(d.roads[at].to.clone(),design.control_points.last().copied())];
    compile(&mut d.roads[at],design,width)?;
    for (node,point) in ends {
        let point=point.ok_or_else(||error("E_ROAD_DESIGN","empty alignment"))?;
        let n=d.nodes.iter_mut().find(|n|n.id==node).ok_or_else(||error("E_REFERENCE","road node missing"))?;
        n.position=point;
        for r in &mut d.roads {
            if r.id==id {continue;}
            for first in [true,false] {
                if (if first {&r.from} else {&r.to})!=&node {continue;}
                let last=r.points.len()-1;
                let index=if first {0} else {last};
                let old=r.points[index];
                r.points[index]=point;
                if let Some(mut design)=r.design.clone() {
                    let end=if first {0} else {design.control_points.len()-1};
                    let handle=if first {1} else {end-1};
                    design.control_points[end]=point;
                    for j in 0..3 {design.control_points[handle][j]+=point[j]-old[j];}
                    compile(r,design,r.widths_cm[0])?;
                }
            }
        }
    }
    if let Some(mut source)=d.assembled_track.as_ref().and_then(|a|a.authoring.clone()).filter(|s|!s.road_connections.is_empty()) {
        assembled_track::composite::align(&d,&mut source)?;
        d=assembled_track::composite::apply_source(&d,&source)?;
    }
    assembled_track::surface::refresh(&mut d)?;
    d.validate()?;
    Ok(d)
}

/// Common surface frames for ports, attached tools and checkpoint authoring.
/// Imported terrain-following roads must first be explicitly designed, or use
/// a collision probe (whose height comes from the supplied terrain payload).
pub fn path(road: &Road) -> Result<Vec<assembled_track::Sample>> {
    let design=road.design.as_ref().ok_or_else(||error("E_ROAD_SURFACE","terrain-following road requires an actual collision probe"))?;
    let samples=probes(design)?;
    if !samples.iter().map(|p|p.0).eq(road.points.iter().copied()) {return Err(error("E_ROAD_DESIGN","stored road samples differ from controls"));}
    let mut out=Vec::with_capacity(road.points.len());
    for (i,(position_cm,direction)) in samples.into_iter().enumerate() {
        let f=crate::curve_sampling::norm(direction);
        let side=crate::curve_sampling::norm([f[2],0.0,-f[0]]);
        let n=crate::curve_sampling::norm(crate::curve_sampling::cross(f,side));
        out.push(assembled_track::Sample {position_cm,ribbon_cm:Some(section(position_cm,direction,road.widths_cm[i.min(road.widths_cm.len()-1)])),
            forward:f.map(|v|libm::round(v*1e6) as i64),normal:n.map(|v|libm::round(v*1e6) as i64),
            mode:"drive".into(),safe:true,min_speed_cmps:0,lateral_cm:road.widths_cm[i.min(road.widths_cm.len()-1)]/2,
            tube_radius_cm:0,below_cm:10,above_cm:300});
    }
    Ok(out)
}

pub fn evaluate(c: &[Vertex], t: f64) -> ([f64; 3], [f64; 3]) {
    let u = 1.0-t;
    (std::array::from_fn(|j| u*u*u*c[0][j] as f64 + 3.0*u*u*t*c[1][j] as f64
        + 3.0*u*t*t*c[2][j] as f64 + t*t*t*c[3][j] as f64),
     std::array::from_fn(|j| 3.0*(u*u*(c[1][j]-c[0][j]) as f64
        + 2.0*u*t*(c[2][j]-c[1][j]) as f64 + t*t*(c[3][j]-c[2][j]) as f64)))
}

pub fn samples(design: &Design) -> Result<Vec<Vertex>> {
    Ok(probes(design)?.into_iter().map(|p|p.0).collect())
}

fn probes(design: &Design) -> Result<Vec<(Vertex,[f64;3])>> {
    let c = &design.control_points;
    if c.len()<4 || c.len()>3073 || !(c.len()-1).is_multiple_of(3)
        || design.shoulder_cm>5000 || c.iter().flatten().any(|n| n.unsigned_abs()>10_000_000) {
        return Err(error("E_ROAD_DESIGN", "bounded cubic controls and shoulder required"));
    }
    let mut out = Vec::new();
    for at in (3..c.len()-1).step_by(3) {
        let a=crate::curve_sampling::norm(std::array::from_fn(|j|(c[at][j]-c[at-1][j]) as f64));
        let b=crate::curve_sampling::norm(std::array::from_fn(|j|(c[at+1][j]-c[at][j]) as f64));
        if (0..3).map(|j|a[j]*b[j]).sum::<f64>()<0.9999 {
            return Err(error("E_ROAD_DESIGN",format!("control {at}: adjacent curve tangents must agree")));
        }
    }
    for at in (0..c.len()-1).step_by(3) {
        cancellation::checkpoint()?;
        let curve = &c[at..at+4];
        let eval = |t| {
            let (p,tangent) = evaluate(curve,t);
            crate::curve_sampling::Probe { points: [p;5], tangent, normal:[0.0,1.0,0.0] }
        };
        // Quarter-probe refinement catches inflections, unlike a midpoint test.
        let ts = crate::curve_sampling::parameters(eval,&[0.0,1.0],400.0,0.25,3.0f64.to_radians());
        for t in ts.into_iter().skip(usize::from(at>0)) {
            let (p,v) = evaluate(curve,t);
            if libm::hypot(v[0],v[2]) < 0.01 {
                return Err(error("E_ROAD_DESIGN", "ordinary road tangent must have horizontal extent"));
            }
            let p = p.map(|n|libm::round(n) as i64);
            if out.last().is_some_and(|last:&(Vertex,[f64;3])|last.0[0]==p[0] && last.0[2]==p[2] && (last.0[1]-p[1]).abs()<=1) {
                if t==1.0 && at+4==c.len() && out.len()>1 {*out.last_mut().unwrap()=(p,v);}
                continue;
            }
            if out.last().map(|last|last.0)!=Some(p) { out.push((p,v)); }
            if out.len()>16384 {return Err(error("E_BUDGET", "road sampling limit"));}
        }
    }
    if out.len()<2 || out.windows(2).any(|s|s[0].0[0]==s[1].0[0] && s[0].0[2]==s[1].0[2]) {
        return Err(error("E_ROAD_DESIGN", "collapsed ordinary road span"));
    }
    Ok(out)
}

pub fn compile(road: &mut Road, design: Design, width_cm: u32) -> Result<()> {
    if !(100..=10000).contains(&width_cm) { return Err(error("E_ROAD_DESIGN", "invalid road width")); }
    let points = samples(&design).map_err(|mut e|{e.message=format!("{}: {}",road.id,e.message);e})?;
    let surface = road.surfaces.first().copied().unwrap_or(Surface::Asphalt);
    road.widths_cm = vec![width_cm; points.len()-1];
    road.surfaces = vec![surface; points.len()-1];
    road.points = points;
    if design.terrain_policy==TerrainPolicy::Elevated && road.kind==RoadKind::Ground {road.kind=RoadKind::Elevated;}
    else if design.terrain_policy==TerrainPolicy::AutoFit && road.kind==RoadKind::Elevated {road.kind=RoadKind::Ground;}
    road.design = Some(design);
    Ok(())
}

pub fn validate(road: &Road) -> Result<()> {
    if let Some(design)=&road.design {
        let c=&design.control_points;
        if c.len()<4 || c.len()>3073 || !(c.len()-1).is_multiple_of(3) || design.shoulder_cm>5000
            || c.first()!=road.points.first() || c.last()!=road.points.last()
            || c.iter().flatten().any(|n|n.unsigned_abs()>10_000_000) {
            return Err(error("E_ROAD_DESIGN","invalid stored alignment controls/endpoints"));
        }
    }
    Ok(())
}
pub fn verify(road:&Road)->Result<()> {
    validate(road)?;
    if let Some(design) = &road.design {
        if samples(design)? != road.points {
            return Err(error("E_ROAD_DESIGN", format!("{} sampled alignment differs from its controls",road.id)));
        }
    }
    Ok(())
}

pub(crate) fn validate_crossings(d: &MapDocument) -> Result<()> {
    if d.roads.iter().all(|r|r.design.is_none()) {return Ok(());}
    let mut work=0;
    let segments: Vec<_>=d.roads.iter().flat_map(|r|r.points.windows(2).map(move |p|(r,p))).collect();
    let bounds:Vec<_>=segments.iter().map(|(_,p)|crate::bounds_index::bounds(&[[p[0][0],p[0][2]],[p[1][0],p[1][2]]])).collect();
    let index=crate::bounds_index::BoundsIndex::new(&bounds);
    for (i,(r,p)) in segments.iter().enumerate() {
        for j in index.query(&bounds[i],&mut work)?.into_iter().filter(|&j|j>i) {
            crate::roads::tick(&mut work,1)?;
            let (other,q)=segments[j];
            if r.id==other.id || r.design.is_none() && other.design.is_none() {continue;}
            if !intersects([p[0][0],p[0][2]],[p[1][0],p[1][2]],[q[0][0],q[0][2]],[q[1][0],q[1][2]]) {continue;}
            let (a,b,c,e)=(p[0],p[1],q[0],q[1]);
            let den=(b[0]-a[0]) as f64*(e[2]-c[2]) as f64-(b[2]-a[2]) as f64*(e[0]-c[0]) as f64;
            if den.abs()<1e-8 {continue;}
            let t=((c[0]-a[0]) as f64*(e[2]-c[2]) as f64-(c[2]-a[2]) as f64*(e[0]-c[0]) as f64)/den;
            let u=((c[0]-a[0]) as f64*(b[2]-a[2]) as f64-(c[2]-a[2]) as f64*(b[0]-a[0]) as f64)/den;
            let at:Vertex=std::array::from_fn(|k|a[k]+libm::round((b[k]-a[k]) as f64*t) as i64);
            let height=c[1] as f64+(e[1]-c[1]) as f64*u;
            let gap=(at[1] as f64-height).abs();
            if gap>=300.0 && (r.kind!=RoadKind::Ground || other.kind!=RoadKind::Ground) {continue;}
            let shared=d.nodes.iter().any(|n|n.position==at && [&r.from,&r.to].contains(&&n.id) && [&other.from,&other.to].contains(&&n.id));
            if !shared {return Err(error("E_ROAD_INTERSECTION",format!("{} / {} at {at:?}: explicit junction or separated bridge/tunnel required",r.id,other.id)));}
        }
    }
    Ok(())
}

/// Cross-sections share exact coordinates on both sides of an interior station.
/// Analytic tangents avoid amplifying centimetre position rounding into jagged
/// edges when adaptive sampling places neighboring points very close together.
pub(crate) fn cross_sections(road:&Road)->Result<Vec<[Vertex;2]>> {
    let sampled=probes(road.design.as_ref().ok_or_else(||error("E_ROAD_DESIGN","authored road required"))?)?;
    if !sampled.iter().map(|p|p.0).eq(road.points.iter().copied()){return Err(error("E_ROAD_DESIGN","stored road samples differ from controls"));}
    Ok(sampled.into_iter().enumerate().map(|(i,(p,v))|section(p,v,road.widths_cm[i.min(road.widths_cm.len()-1)])).collect())
}
fn section(p:Vertex,direction:[f64;3],width:u32)->[Vertex;2] {
    let dx=direction[0];let dz=direction[2];
    let length = libm::hypot(dx,dz).max(1.0);
    let half = width as f64/2.0;
    let offset = [libm::round(-dz/length*half) as i64,libm::round(dx/length*half) as i64];
    [[p[0]+offset[0],p[1],p[2]+offset[1]], [p[0]-offset[0],p[1],p[2]-offset[1]]]
}

/// Independent design reference before clipping against cell/terrain triangles.
pub fn design_triangles(d:&MapDocument,bounds:&Bounds)->Result<Vec<(String,[Vertex;3])>> {
    Ok(crate::road_plan::plan(d,bounds)?.0.into_iter().filter(|p|p.road.design.is_some()).map(|p|(p.road.id.clone(),p.v)).collect())
}

/// Cell-local source plan. Fitting never scans a whole world for every vertex.
pub(crate) struct Fitter<'a> { segments: Vec<(Vertex,Vertex,f64,f64,&'a str)> }
impl<'a> Fitter<'a> {
    pub fn new(d: &'a MapDocument,bounds:&Bounds)->Self {
        let mut segments=Vec::new();
        let mut add=|a:Vertex,b:Vertex,width:u32,shoulder:u32,id:&'a str| {
            let margin=i64::from(width)/2+i64::from(shoulder);
            if crate::road_plan::hit(&[a,b],bounds,margin) {segments.push((a,b,width as f64/2.0,shoulder as f64,id));}
        };
        for r in &d.roads {
            let Some(design)=&r.design else {continue;};
            if design.terrain_policy!=TerrainPolicy::AutoFit || r.kind!=RoadKind::Ground {continue;}
            for (i,p) in r.points.windows(2).enumerate() {add(p[0],p[1],r.widths_cm[i],design.shoulder_cm+r.sidewalk_cm.unwrap_or(0),&r.id);}
        }
        if let Some(a)=d.assembled_track.as_ref().filter(|a|a.terrain_integration()) {
            for (i,p) in a.pieces.iter().enumerate() {
                if a.terrain_policy(i)!=TerrainPolicy::AutoFit {continue;}
                for path in [&p.path,&p.alternate_path] {for pair in path.windows(2) {
                    if pair.iter().any(|s|!s.safe || s.normal[1]<900000) {continue;}
                    add(pair[0].position_cm,pair[1].position_cm,pair[0].lateral_cm*2,600,&a.authoring.as_ref().unwrap().instances[i].id);
                }}
            }
        }
        Self{segments}
    }
    pub fn height(&self,p:Point,original:i64)->i64 {
        let mut best:Option<(f64,&str,f64)>=None;
        for &(a,b,half,shoulder,id) in &self.segments {
            let (dx,dz)=((b[0]-a[0]) as f64,(b[2]-a[2]) as f64);
            if dx*dx+dz*dz<1.0 {continue;}
            let along=(((p[0]-a[0]) as f64*dx+(p[1]-a[2]) as f64*dz)/(dx*dx+dz*dz)).clamp(0.0,1.0);
            let edge=libm::hypot(p[0] as f64-a[0] as f64-along*dx,p[1] as f64-a[2] as f64-along*dz)-half;
            if edge>shoulder || best.as_ref().is_some_and(|(n,owner,_)|(edge,id)>=(*n,*owner)) {continue;}
            let t=if shoulder==0.0 {1.0} else {(edge/shoulder).clamp(0.0,1.0)};
            let weight=1.0-t*t*(3.0-2.0*t);
            let height=a[1] as f64+along*(b[1]-a[1]) as f64;
            best=Some((edge,id,original as f64*(1.0-weight)+height*weight));
        }
        best.map_or(original,|(_,_,h)|libm::round(h) as i64)
    }
}
