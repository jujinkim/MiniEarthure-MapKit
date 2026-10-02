//! One closed barrier mesh for rendering, collision and occupied-volume admission.
use super::*;
type Ring = [Vertex; 4];
pub(super) struct Volume { pub rings: [Ring; 2], pub id: String }

pub(super) fn outward_at(path: &[Sample], at: usize, side: i64) -> Vertex {
    let edge = |i: usize| geometry::ribbon_edges(&path[i],0)[usize::from(side > 0)];
    let here=edge(at);
    let n=path[at].normal.map(|v|v as f64/1e6);
    let directions = [if at>0 { at-1 } else { at }, if at+1<path.len() { at+1 } else { at }];
    let mut normals=vec![];
    for (j,i) in directions.into_iter().enumerate() {
        if i==at {continue;}
        let e=edge(i);
        let d=std::array::from_fn::<_,3,_>(|k|(e[k]-here[k]) as f64 * if j==0 {-1.0} else {1.0});
        normals.push(unit([n[1]*d[2]-n[2]*d[1],n[2]*d[0]-n[0]*d[2],n[0]*d[1]-n[1]*d[0]]).map(|v|v as f64/1e6*side as f64));
    }
    let bisector=unit(std::array::from_fn(|j|normals.iter().map(|n|n[j]).sum())).map(|v|v as f64/1e6);
    let cosine=(0..3).map(|j|bisector[j]*normals[0][j]).sum::<f64>().max(0.5);
    std::array::from_fn(|j|round(bisector[j]*WALL_THICKNESS_CM as f64/cosine))
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
    for v in volumes {
        cancellation::checkpoint()?;
        let [a, c] = v.rings;
        let mut faces = vec![];
        for i in 0..4 {
            let j = (i+1)%4;
            let q = [i as u8, j as u8, (j+4) as u8, (i+4) as u8];
            faces.extend([[q[0],q[1],q[2]],[q[0],q[2],q[3]]]);
            b.quad([a[i],a[j],c[j],c[i]], Surface::Concrete, &v.id, false)?;
        }
        for (index, mut ring) in [a,c].into_iter().enumerate() {
            let mut key = ring; key.sort();
            if index == 0 { ring.reverse(); }
            if caps[&key] == 1 { b.quad(ring, Surface::Concrete, &v.id, false)?; }
        }
        faces.extend([[3,2,1],[3,1,0],[4,5,6],[4,6,7]]);
        b.solid(&v.id, SolidShape::Convex(CollisionConvex {
            vertices: a.into_iter().chain(c).collect(), faces,
        }))?;
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
        for id in ["straight","gentle90","curve_up","curve_left_down","spiral_up","straight_narrow","finish_plaza"] {
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
            assert!(edges.values().all(|count|*count==2),"{id}: closed manifold walls");
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
