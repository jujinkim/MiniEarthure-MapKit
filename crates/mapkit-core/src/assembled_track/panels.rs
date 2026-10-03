//! Panels clip the production road triangles, never an interpolated flat pad.
use super::*;
use crate::curve_sampling::{cross, norm};

type Point = [f64; 3];
fn sub(a: Point, b: Point) -> Point { std::array::from_fn(|j| a[j]-b[j]) }
fn clip(mut poly: Vec<Point>, axis: usize, limit: f64, lower: bool) -> Vec<Point> {
    let old=std::mem::take(&mut poly);
    for i in 0..old.len() {
        let (a,b)=(old[i],old[(i+1)%old.len()]);
        let inside=|p: Point| if lower {p[axis]>=limit} else {p[axis]<=limit};
        if inside(a) {poly.push(a);}
        if inside(a)!=inside(b) {
            let t=(limit-a[axis])/(b[axis]-a[axis]);
            poly.push(std::array::from_fn(|j|a[j]+(b[j]-a[j])*t));
        }
    }
    poly
}
struct PanelSurface<'a> { g: &'a mut Gimmick, basis: [[f64;3];3], half: f64, area: f64 }
impl TrackGeometry for PanelSurface<'_> {
    fn triangle(&mut self, v: [Vertex;3], _: Surface, _: &str, driving: bool) -> Result<()> {
        if !driving {return Ok(());}
        cancellation::checkpoint()?;
        let points=v.map(|p|std::array::from_fn(|j|(0..3).map(|k|(p[k]-self.g.position[k]) as f64*self.basis[k][j]).sum()));
        // Only the supporting sheet near this frame; no upper/lower road or back face.
        let normal=norm(cross(sub(points[1],points[0]),sub(points[2],points[0])));
        if normal[1]<0.5 || points.iter().all(|p|p[1]>50.0) || points.iter().all(|p|p[1]< -50.0) {return Ok(());}
        let mut poly=points.to_vec();
        for (axis,limit,lower) in [(0,-self.half,true),(0,self.half,false),(2,-100.0,true),(2,100.0,false)] {
            poly=clip(poly,axis,limit,lower);
        }
        if poly.len()<3 {return Ok(());}
        for i in 1..poly.len()-1 {
            let triangle=[poly[0],poly[i],poly[i+1]];
            let area=cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]))[1]*0.5;
            if area<0.01 {continue;}
            // Millimetre local coordinates retain the 3cm normal lift through
            // rotations. Each triangle prism remains exactly convex after rounding.
            let top=triangle.map(|p|std::array::from_fn(|j|round((p[j]+normal[j]*3.0)*10.0)));
            let raw=cross(sub(top[1].map(|v|v as f64),top[0].map(|v|v as f64)),sub(top[2].map(|v|v as f64),top[0].map(|v|v as f64)));
            if raw[1]<=0.0 {continue;}
            if self.g.parts.len()==32 {return Err(error("E_TRACK_PANEL_BUDGET","panel exceeds the existing 32-part limit"));}
            let mut vertices=top.to_vec();
            vertices.extend(top.map(|mut p|{p[1]-=40;p}));
            let part=CollisionConvex {vertices,faces:vec![[0,1,2],[5,4,3],[0,3,4],[0,4,1],[1,4,5],[1,5,2],[2,5,3],[2,3,0]]};
            if !part.valid(10_000) {return Err(error("E_TRACK_PANEL_SUPPORT","panel surface cannot be represented within convex bounds"));}
            self.g.parts.push(part);
            self.area+=area;
        }
        Ok(())
    }
    fn solid(&mut self, _: &str, _: SolidShape) -> Result<()> {Ok(())}
}
struct Discard;
impl walls::WallSink for Discard {fn push(&mut self, _: walls::Volume) {}}

pub(super) fn fit(a: &Assembly, index: usize, g: &mut Gimmick, lateral: u32) -> Result<()> {
    // Use the actual encoded Euler basis, so display and geometry agree exactly.
    let basis: [[f64;3];3]=std::array::from_fn(|j| {
        geometry::rotate3(std::array::from_fn(|k|if j==k {1_000_000} else {0}),g.rotation_mdeg).map(|v|v as f64/1e6)
    });
    let basis=std::array::from_fn(|i|std::array::from_fn(|j|basis[j][i]));
    g.scale_per_mille=[100;3];
    g.parts.clear();
    let mut surface=PanelSurface {g,basis,half:(lateral as f64-25.0).max(0.0),area:0.0};
    generate_piece(&a.pieces[index],index,&mut surface,&[],&mut Discard)?;
    // Connected seams share the same source triangles. Unrelated stacked roads
    // cannot supply a missing panel surface.
    for p in junction::neighbors(a,index) {
        generate_piece(p,index,&mut surface,&[],&mut Discard)?;
    }
    if surface.area<1.0 || surface.g.parts.is_empty() {
        return Err(error("E_TRACK_PANEL_SUPPORT","panel has no supporting road triangles"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assembly(id: &str) -> Assembly {
        let mut source=authoring::Source::empty();
        source.instances.push(authoring::instance("road",id,400));
        authoring::compile(&source).unwrap()
    }
    fn panel(a: &Assembly, sample: usize) -> Gimmick {
        let s=&a.pieces[0].path[sample];
        Gimmick {id:"panel".into(),position:s.position_cm,rotation_mdeg:geometry::euler(geometry::basis(s)),scale_per_mille:[1000;3],parts:vec![],curved_faces:vec![],track:None,effect:Some(Effect {strength_percent:100,jump_height_cm:200,ring_radius_cm:250}),surface:Surface::Asphalt,color:[255,113,35,255],motion:Motion {kind:MotionKind::TargetSpeed,delta_cm:[0;3],axis:1,period_ms:4000,phase_ms:0,impulse_cmps:[0;3],cooldown_ms:1500},safety_min_cm:s.position_cm.map(|v|v-5000),safety_max_cm:s.position_cm.map(|v|v+5000)}
    }
    #[test]
    fn panels_follow_surface_and_preserve_limits() {
        for id in ["straight","slope_up","gentle90","curve_up","spiral90_left_up"] {
            let a=assembly(id); let at=a.pieces[0].path.len()/2;
            let mut g=panel(&a,at);
            fit(&a,0,&mut g,200).unwrap_or_else(|e|panic!("{id}: {e:?}"));
            assert!(g.valid(),"{id}");
            assert!(g.parts.len()<=32);
            let mut repeated=panel(&a,at); fit(&a,0,&mut repeated,200).unwrap(); assert_eq!(g,repeated);
        }
        let a=assembly("straight");let mut g=panel(&a,2);g.position[1]+=100;
        assert_eq!(fit(&a,0,&mut g,200).unwrap_err().code,"E_TRACK_PANEL_SUPPORT");
        let mut g=panel(&a,2);let mut sink=PanelSurface {g:&mut g,basis:[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]],half:175.0,area:0.0};
        let v=[[-100,0,0],[0,0,100],[100,0,0]].map(|p|add(p,sink.g.position));
        for _ in 0..32 {sink.triangle(v,Surface::Asphalt,"road",true).unwrap();}
        assert_eq!(sink.triangle(v,Surface::Asphalt,"road",true).unwrap_err().code,"E_TRACK_PANEL_BUDGET");
    }
    #[test]
    fn connected_seam_and_taper_are_supported_without_other_levels() {
        let mut source=authoring::Source::empty();
        let mut first=authoring::instance("a","straight",600);
        first.entry_width_cm=400;
        let second=authoring::snap(&authoring::instance("b","straight",600),&first).unwrap();
        source.instances=vec![first,second];
        source.connections.push(authoring::Connection {from:"a".into(),to:"b".into()});
        let a=authoring::compile(&source).unwrap();
        let last=a.pieces[0].path.len()-1;
        let mut g=panel(&a,last);fit(&a,0,&mut g,300).unwrap();
        let z:Vec<_>=g.parts.iter().flat_map(|p|&p.vertices).map(|v|v[2]).collect();
        assert_eq!((*z.iter().min().unwrap(),*z.iter().max().unwrap()),(-1000,1000));
        let mut shifted=a.clone();for s in &mut shifted.pieces[1].path {s.position_cm[1]+=500;if let Some(edges)=&mut s.ribbon_cm {for v in edges {v[1]+=500;}}}
        fit(&shifted,0,&mut g,300).unwrap();
        assert!(g.parts.iter().flat_map(|p|&p.vertices).all(|v|v[2]<=0));
        let mut g=panel(&a,1);fit(&a,0,&mut g,200).unwrap();assert!(g.valid());
    }
    #[test]
    fn flat_panel_is_three_centimetres_above_real_surface() {
        let a=assembly("straight");let mut g=panel(&a,2);fit(&a,0,&mut g,200).unwrap();
        for part in g.parts {for v in &part.vertices[..3] {assert_eq!(v[1],30);}}
    }
}
