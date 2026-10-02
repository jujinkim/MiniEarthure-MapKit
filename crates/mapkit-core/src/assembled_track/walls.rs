//! One closed barrier mesh for rendering, collision and occupied-volume admission.
use super::*;
type Ring = [Vertex; 4];
pub(super) struct Volume { pub rings: [Ring; 2], pub id: String }

pub(super) fn edges(s: &Sample, rotation: [i32;3]) -> [Vertex;2] {
    if s.mode != "loop" { return geometry::ribbon_edges(s,0); }
    let right=geometry::rotate3([1_000_000,0,0],rotation);
    [-1,1].map(|side|std::array::from_fn(|j|
        s.position_cm[j]+right[j]*i64::from(s.lateral_cm)*side/1_000_000))
}

pub(super) fn outward_at(path: &[Sample], at: usize, side: i64, rotation: [i32;3]) -> Vertex {
    let edge = |i: usize| edges(&path[i],rotation)[usize::from(side > 0)];
    let here=edge(at);
    let n=path[at].normal.map(|v|v as f64/1e6);
    // A one-centimetre ordinary-ribbon perturbation must not turn a 50cm offset
    // back on itself. Estimate tangents over one barrier thickness, including
    // loop crowns where adaptive intervals can be shorter than the offset.
    // Positions and lane widths remain the original quantized ribbon vertices.
    let tangent_span=WALL_THICKNESS_CM as u64;
    let mut before=at;
    while before>0 {
        before-=1;
        if distance(edge(before),here)>=tangent_span {break;}
    }
    let mut after=at;
    while after+1<path.len() {
        after+=1;
        if distance(edge(after),here)>=tangent_span {break;}
    }
    let directions = [before,after];
    let mut normals=vec![];
    for (j,i) in directions.into_iter().enumerate() {
        if i==at {continue;}
        let e=edge(i);
        let d=std::array::from_fn::<_,3,_>(|k|(e[k]-here[k]) as f64 * if j==0 {-1.0} else {1.0});
        normals.push(unit([n[1]*d[2]-n[2]*d[1],n[2]*d[0]-n[0]*d[2],n[0]*d[1]-n[1]*d[0]]).map(|v|v as f64/1e6*side as f64));
    }
    let bisector=unit(std::array::from_fn(|j|normals.iter().map(|n|n[j]).sum())).map(|v|v as f64/1e6);
    let cosine=(0..3).map(|j|bisector[j]*normals[0][j]).sum::<f64>().max(0.5);
    let ideal=std::array::from_fn::<_,3,_>(|j|bisector[j]*WALL_THICKNESS_CM as f64/cosine);
    let mut result=ideal.map(round);
    if path[at].mode=="loop" {
        // Choose the nearest integer offset that also preserves face-normal
        // thickness. A rotated short crown chord can lose just over 1cm when
        // three independently rounded components all point inward.
        let local: Vec<_>=[at.saturating_sub(1),(at+1).min(path.len()-1)].into_iter().filter(|&i|i!=at).map(|i| {
            let e=edge(i);let d=std::array::from_fn::<_,3,_>(|j|(e[j]-here[j]) as f64);
            unit([n[1]*d[2]-n[2]*d[1],n[2]*d[0]-n[0]*d[2],n[0]*d[1]-n[1]*d[0]]).map(|v|v as f64/1e6)
        }).collect();
        let error=|v:Vertex|local.iter().map(|n|((0..3).map(|j|n[j]*v[j] as f64).sum::<f64>().abs()-WALL_THICKNESS_CM as f64).abs()).fold(0.0,f64::max);
        if error(result)>1.0 {
            let original=result;let mut best=error(result);
            for x in -1..=1 {for y in -1..=1 {for z in -1..=1 {
                let candidate=add(original,[x,y,z]);let cost=error(candidate);
                if cost<best {best=cost;result=candidate;}
            }}}
        }
    }
    result
}

pub(super) fn ring(base: Vertex, outward: Vertex, up: Vertex, reverse: bool) -> Ring {
    let mut result = [base, add(base, up), add(add(base, outward), up), add(base, outward)];
    if reverse { result.reverse(); }
    result
}

pub(super) fn cut(rings: [Ring; 2], neighbors: &[&Piece], alternate: &[Sample], id: &str, out: &mut Vec<Volume>) {
    for (lo, hi) in junction::visible_volume(rings, neighbors, alternate) {
        let at = |t: f64| std::array::from_fn(|i| std::array::from_fn(|j|
            round(rings[0][i][j] as f64 + (rings[1][i][j]-rings[0][i][j]) as f64*t)));
        let clipped = [at(lo), at(hi)];
        if clipped[0] != clipped[1] { out.push(Volume { rings: clipped, id: id.into() }); }
    }
}

pub(super) fn emit(volumes: &[Volume], b: &mut impl TrackGeometry) -> Result<()> {
    // Cancel matching internal end faces, including piece seams. No reversed
    // duplicate sheet or internal cap reaches native CCD edge classification.
    let mut caps = std::collections::BTreeMap::<Ring, usize>::new();
    for v in volumes { for mut ring in v.rings { ring.sort(); *caps.entry(ring).or_default() += 1; } }
    // At a tight loop crown, centimetre quantization can collapse an edge or
    // fold a sub-centimetre face onto its neighbour. Cancel those internal
    // opposing faces too; they must not become reversed native CCD features.
    let mut triangles = std::collections::BTreeMap::<[Vertex;3], (i32, &str)>::new();
    let mut quad = |mut ring: Ring, id| {
        let first=(0..4).min_by_key(|&i|ring[i]).unwrap();
        ring.rotate_left(first);
        for mut face in [[ring[0],ring[1],ring[2]],[ring[0],ring[2],ring[3]]] {
            let u=std::array::from_fn::<_,3,_>(|j|i128::from(face[1][j]-face[0][j]));
            let v=std::array::from_fn::<_,3,_>(|j|i128::from(face[2][j]-face[0][j]));
            if [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]] == [0;3] {continue;}
            let index=(0..3).min_by_key(|&i|face[i]).unwrap();
            face.rotate_left(index);
            let orientation=if face[1]<face[2] {1} else {-1};
            face.sort();
            triangles.entry(face).or_insert((0,id)).0 += orientation;
        }
    };
    for v in volumes {
        cancellation::checkpoint()?;
        let [a, c] = v.rings;
        let mut faces = vec![];
        for i in 0..4 {
            let j = (i+1)%4;
            let q = [i as u8, j as u8, (j+4) as u8, (i+4) as u8];
            faces.extend([[q[0],q[1],q[2]],[q[0],q[2],q[3]]]);
            quad([a[i],a[j],c[j],c[i]], v.id.as_str());
        }
        for (index, mut ring) in [a,c].into_iter().enumerate() {
            let mut key = ring; key.sort();
            if index == 0 { ring.reverse(); }
            if caps[&key] == 1 { quad(ring, v.id.as_str()); }
        }
        faces.extend([[3,2,1],[3,1,0],[4,5,6],[4,6,7]]);
        b.solid(&v.id, SolidShape::Convex(CollisionConvex {
            vertices: a.into_iter().chain(c).collect(), faces,
        }))?;
    }
    for (mut face, (orientation,id)) in triangles {
        if orientation==0 {continue;}
        if orientation<0 {face.swap(1,2);}
        b.triangle(face, Surface::Concrete, id, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mesh { triangles: Vec<[Vertex;3]>, solids: usize, spawnable: bool }
    impl TrackGeometry for Mesh {
        fn triangle(&mut self, v: [Vertex;3], _: Surface, _: &str, spawnable: bool) -> Result<()> {
            self.triangles.push(v); self.spawnable |= spawnable; Ok(())
        }
        fn solid(&mut self, _: &str, _: SolidShape) -> Result<()> { self.solids+=1; Ok(()) }
    }
    #[test]
    fn closed_outward_walls_on_straight_curve_slope_taper_and_plaza() {
        for id in ["straight","gentle90","curve_up","curve_left_down","spiral_up","straight_narrow","loop","finish_plaza"] {
            let p=materialize(&variant(id,600,400,600));
            let mut discarded=Mesh::default(); let mut volumes=vec![];
            generate_piece(&p,0,&mut discarded,&[],&mut volumes).unwrap();
            assert!(!volumes.is_empty(),"{id}");
            let mut out=Mesh::default(); emit(&volumes,&mut out).unwrap();
            assert_eq!(out.solids,volumes.len());
            assert!(!out.spawnable,"wall tops must never add spawn candidates");
            let mut edges=std::collections::BTreeMap::new();
            for face in &out.triangles { for i in 0..3 {
                let mut edge=[face[i],face[(i+1)%3]]; edge.sort();
                *edges.entry(edge).or_insert(0)+=1;
            }}
            let invalid: Vec<_> = edges.iter().filter(|(_,count)|**count!=2).take(12).collect();
            assert!(invalid.is_empty(),"{id}: closed manifold walls: {invalid:?}");
            for volume in &volumes { for ring in volume.rings {
                let width=distance(ring[0],ring[3]);
                assert!((48..=72).contains(&width),"{id}: quantized width {width}");
            }}
        }
    }
    #[test]
    fn wide_walls_keep_gimmick_layout_duration() {
        let s=Settings {seed:42,circuit:false,duration_seconds:90,categories:vec!["gimmick".into()],..Default::default()};
        let a=assemble(&s).unwrap();
        assert!(a.estimated_msec.abs_diff(90000)<=9000);
    }
    #[test]
    fn loop_wall_normal_thickness_is_fifty_cm() {
        let mut piece=variant("loop",LOOP_WIDTH,400,400);
        for rotation in [[0,0,0],[12000,35000,7000]] {
            piece.rotation_mdeg=rotation;
            let p=materialize(&piece);
            for at in 1..p.path.len()-1 {
                let s=&p.path[at];
                if s.mode!="loop" {continue;}
                for side in [-1,1] {
                    let outward=outward_at(&p.path,at,side,rotation);
                    let a=edges(s,rotation)[usize::from(side>0)];
                    for neighbor in [at-1,at+1] {
                        let b=edges(&p.path[neighbor],rotation)[usize::from(side>0)];
                        let d=std::array::from_fn::<_,3,_>(|j|(b[j]-a[j]) as f64);
                        let up=s.normal.map(|v|v as f64/1e6);
                        let normal=unit([d[1]*up[2]-d[2]*up[1],d[2]*up[0]-d[0]*up[2],d[0]*up[1]-d[1]*up[0]]);
                        let thickness=(0..3).map(|j|normal[j] as f64/1e6*outward[j] as f64).sum::<f64>().abs();
                        assert!((49.0..=51.0).contains(&thickness),"rotation={rotation:?} at={at} thickness={thickness}");
                    }
                }
            }
        }
    }
    #[test]
    fn ordinary_wall_face_normal_thickness_stays_within_centimetre_quantization() {
        for id in ["straight","gentle90","curve_up","curve_left_down","spiral_up","spiral180_right_up","straight_narrow"] {
            let p=materialize(&variant(id,600,400,600));
            for at in 0..p.path.len()-1 { for side in [-1,1] {
                let a=edges(&p.path[at],p.rotation_mdeg)[usize::from(side>0)];
                let b=edges(&p.path[at+1],p.rotation_mdeg)[usize::from(side>0)];
                let d=std::array::from_fn::<_,3,_>(|j|(b[j]-a[j]) as f64);
                let up=p.path[at].normal.map(|v|v as f64/1e6);
                let normal=unit([d[1]*up[2]-d[2]*up[1],d[2]*up[0]-d[0]*up[2],d[0]*up[1]-d[1]*up[0]]);
                let outward=outward_at(&p.path,at,side,p.rotation_mdeg);
                let thickness=(0..3).map(|j|normal[j] as f64/1e6*outward[j] as f64).sum::<f64>().abs();
                assert!((48.0..=52.0).contains(&thickness),"{id} at={at} thickness={thickness}");
            }}
        }
    }
    #[test]
    fn full_volume_bounds_include_wall_and_adjacent_piece_caps_cancel() {
        let p=materialize(&variant("straight",400,400,400));
        let (center,radius,_,_)=geometry::volume(&p.path[0]);
        assert_eq!(center,p.path[0].position_cm);
        assert!(radius>=250);
        let a=ring([0,0,0],[-50,0,0],[0,120,0],false);
        let b=ring([0,0,100],[-50,0,0],[0,120,0],false);
        let c=ring([0,0,200],[-50,0,0],[0,120,0],false);
        let mut mesh=Mesh::default();
        emit(&[Volume{rings:[a,b],id:"piece-a".into()},Volume{rings:[b,c],id:"piece-b".into()}],&mut mesh).unwrap();
        assert_eq!(mesh.triangles.len(),20,"two joined boxes have no interior caps");
    }
}
