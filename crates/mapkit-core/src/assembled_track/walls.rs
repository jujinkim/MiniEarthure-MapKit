//! One closed barrier mesh for rendering, collision and occupied-volume admission.
use super::*;
type Ring = [Vertex; 4];
pub(super) struct Volume { pub rings: [Ring; 2], pub id: String }
pub(super) const MAX_TETRAHEDRA: u64 = 12;
// Vec growth, IDs, all tetrahedron faces in the cancellation BTreeMap, and
// per-section triangulation/convex workspace. This is a reservation, not RSS.
pub(super) const SCRATCH_PER_VOLUME: u64 = 512 + MAX_TETRAHEDRA * 4 * 256;
pub(super) trait WallSink { fn push(&mut self, volume: Volume); }
impl WallSink for Vec<Volume> { fn push(&mut self, v: Volume) { Vec::push(self,v); } }
pub(super) struct Cost<'a> { pub bounds: &'a Bounds, pub all: u64, pub triangles: u64, pub solids: u64 }
impl WallSink for Cost<'_> {
    fn push(&mut self, volume: Volume) {
        self.all += 1;
        if (0..2).all(|j| {
            let points=volume.rings.iter().flatten();
            points.clone().map(|v|v[j*2]).min().unwrap()<=self.bounds.max[j]
                && points.map(|v|v[j*2]).max().unwrap()>=self.bounds.min[j]
        }) {
            // Only one bounded section is materialized for the estimate. Folded
            // sections can expose more than the usual twelve boundary triangles.
            let parts=tetrahedra(volume.rings);
            self.solids += parts.len() as u64;
            let mut faces=std::collections::BTreeMap::<[Vertex;3],i32>::new();
            for part in parts { for indices in part.faces {
                let mut face=indices.map(|i|part.vertices[i as usize]);
                let first=(0..3).min_by_key(|&i|face[i]).unwrap();face.rotate_left(first);
                let sign=if face[1]<face[2] {1} else {-1};face.sort();
                *faces.entry(face).or_default()+=sign;
            }}
            self.triangles += faces.values().filter(|&&n|n!=0).count() as u64;
        }
    }
}

fn edges_with_right(s: &Sample, right: Vertex) -> [Vertex;2] {
    if s.mode != "loop" { return geometry::ribbon_edges(s,0); }
    [-1,1].map(|side|std::array::from_fn(|j|
        s.position_cm[j]+right[j]*i64::from(s.lateral_cm)*side/1_000_000))
}

/// Only lives while one path is tessellated. Adjacent segments share quantized
/// edges and offsets; lazy offsets skip the same non-wall samples as before.
pub(super) struct PreparedPath {
    pub edges: Vec<[Vertex; 2]>,
    outward: Vec<[Option<Vertex>; 2]>,
}
impl PreparedPath {
    pub fn new(path: &[Sample], rotation: [i32; 3]) -> Result<Self> {
        cancellation::checkpoint()?;
        let right=geometry::rotate3([1_000_000,0,0],rotation);
        let mut edges=Vec::with_capacity(path.len());
        for (i,s) in path.iter().enumerate() {
            cancellation::checkpoint()?;
            if i==0 || i+1==path.len() {
                // Saved v1 pieces can contain independently rounded endpoint
                // ribbons. Tessellate their declared port centre/frame exactly
                // as a newly compiled neighbour, without rewriting the source.
                let mut port=s.clone();port.ribbon_cm=None;
                edges.push(edges_with_right(&port,right));
            } else {
                edges.push(edges_with_right(s,right));
            }
        }
        Ok(Self { edges, outward: vec![[None; 2]; path.len()] })
    }
    pub fn outward_at(&mut self, path: &[Sample], at: usize, side: i64) -> Vertex {
        let index=usize::from(side>0);
        if let Some(value)=self.outward[at][index] { return value; }
        let value=outward_from_edges(path,at,side,|i|self.edges[i][index]);
        self.outward[at][index]=Some(value);
        value
    }
    pub fn scratch_bytes(samples: usize) -> u64 {
        samples as u64 * (std::mem::size_of::<[Vertex;2]>() + std::mem::size_of::<[Option<Vertex>;2]>()) as u64
    }
}

#[cfg(test)]
pub(super) fn edges(s: &Sample, rotation: [i32;3]) -> [Vertex;2] {
    edges_with_right(s,geometry::rotate3([1_000_000,0,0],rotation))
}
#[cfg(test)]
pub(super) fn outward_at(path: &[Sample], at: usize, side: i64, rotation: [i32;3]) -> Vertex {
    outward_from_edges(path,at,side,|i|edges(&path[i],rotation)[usize::from(side>0)])
}

fn outward_from_edges(path: &[Sample], at: usize, side: i64, edge: impl Fn(usize) -> Vertex) -> Vertex {
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

pub(super) fn cut(rings: [Ring; 2], neighbors: &junction::Prepared, alternate: &junction::Prepared, id: &str, out: &mut impl WallSink) -> Result<()> {
    for (lo, hi) in junction::visible_volume(rings, neighbors, alternate)? {
        let at = |t: f64| std::array::from_fn(|i| std::array::from_fn(|j|
            round(rings[0][i][j] as f64 + (rings[1][i][j]-rings[0][i][j]) as f64*t)));
        let clipped = [at(lo), at(hi)];
        if clipped[0] != clipped[1] { out.push(Volume { rings: clipped, id: id.into() }); }
    }
    Ok(())
}

// Pull each boundary triangle to the least visible quantized vertex. A
// warped eight-vertex strip is not necessarily convex; each nonzero tetrahedron
// is. Exact integer volume, never an epsilon, decides whether a part disappears.
fn tetrahedra(rings: [Ring; 2]) -> Vec<CollisionConvex> {
    let [a, b] = rings;
    let mut quads = vec![[a[3], a[2], a[1], a[0]], b];
    for i in 0..4 {
        let j = (i + 1) % 4;
        quads.push([a[i], a[j], b[j], b[i]]);
    }
    let mut boundary = vec![];
    for quad in quads { boundary.extend(triangulate(quad)); }
    let points: std::collections::BTreeSet<_> = a.into_iter().chain(b).collect();
    if boundary.iter().all(|&[a,b,c]| points.iter().all(|&p|determinant([a,b,c,p])==0)) {
        return vec![];
    }
    let visible = |p| boundary.iter().all(|&[a,b,c]| determinant([a,b,c,p]) <= 0);
    let pivot = points.iter().copied().find(|p|
        rings.iter().all(|r| !r.contains(p) || triangulate(*r).iter().all(|t| t.contains(p) || normal(*t)==[0;3]))
        && visible(*p))
        .or_else(|| {
            // A twisted section may have an interior kernel but no visible
            // corner. Quantize once, then require exact containment in it.
            let center=std::array::from_fn(|j| (points.iter().map(|p|i128::from(p[j])).sum::<i128>() / points.len() as i128) as i64);
            visible(center).then_some(center)
        })
        // Folded centimetre sections still contain nonzero occupied wedges.
        // Keep their independently outward-oriented tetrahedra as well; never
        // drop occupied material because the unsplit strip has no kernel.
        .unwrap_or_else(|| *points.first().unwrap());
    let mut parts = std::collections::BTreeSet::new();
    for face in boundary {
        let mut vertices = [pivot, face[0], face[1], face[2]];
        vertices.sort();
        if determinant(vertices) != 0 { parts.insert(vertices); }
    }
    parts.into_iter().map(|vertices| {
        let mut faces = vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        if determinant(vertices) > 0 {
            for f in &mut faces { f.swap(1, 2); }
        }
        CollisionConvex { vertices: vertices.to_vec(), faces }
    }).collect()
}

fn normal([a,b,c]: [Vertex;3]) -> [i128;3] {
    let u=std::array::from_fn::<_,3,_>(|j|i128::from(b[j])-i128::from(a[j]));
    let v=std::array::from_fn::<_,3,_>(|j|i128::from(c[j])-i128::from(a[j]));
    [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
}

fn triangulate(mut quad: Ring) -> [[Vertex;3];2] {
    let first=(0..4).min_by_key(|&i|quad[i]).unwrap();
    quad.rotate_left(first);
    let [a,b,c,d]=quad;
    let (n0,n1)=(normal([a,b,c]),normal([a,c,d]));
    let n: [i128;3]=std::array::from_fn(|j|n0[j]+n1[j]);
    if [n0,n1].iter().any(|v|(0..3).map(|j|v[j]*n[j]).sum::<i128>()<0) {
        [[a,b,d],[b,c,d]]
    } else { [[a,b,c],[a,c,d]] }
}

fn determinant([a, b, c, d]: [Vertex; 4]) -> i128 {
    let delta = |p: Vertex| std::array::from_fn::<_, 3, _>(|i| i128::from(p[i]) - i128::from(a[i]));
    let (u, v, w) = (delta(b), delta(c), delta(d));
    (u[1]*v[2]-u[2]*v[1])*w[0] + (u[2]*v[0]-u[0]*v[2])*w[1] + (u[0]*v[1]-u[1]*v[0])*w[2]
}

pub(super) fn emit(volumes: &[Volume], b: &mut impl TrackGeometry) -> Result<()> {
    // Derive the exterior from exactly the occupancy parts. Cancelling opposing
    // faces removes tetrahedron interiors and shared section/piece caps alike.
    let mut triangles = std::collections::BTreeMap::<[Vertex; 3], (i32, &str)>::new();
    for v in volumes {
        cancellation::checkpoint()?;
        for shape in tetrahedra(v.rings) {
            for indices in &shape.faces {
                let mut face = indices.map(|i| shape.vertices[i as usize]);
                let first = (0..3).min_by_key(|&i| face[i]).unwrap();
                face.rotate_left(first);
                let orientation = if face[1] < face[2] { 1 } else { -1 };
                face.sort();
                triangles.entry(face).or_insert((0, &v.id)).0 += orientation;
            }
            b.solid(&v.id, SolidShape::Convex(shape))?;
        }
    }
    for (mut face, (orientation, id)) in triangles {
        if orientation == 0 { continue; }
        if orientation < 0 { face.swap(1, 2); }
        b.triangle(face, Surface::Concrete, id, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_preparation_preserves_offsets_budget_and_cancellation() {
        for id in ["straight","gentle90","loop","overpass"] {
            let mut source=variant(id,600,400,600);
            source.rotation_mdeg=[12000,35000,7000];
            let p=materialize(&source);
            for path in [&p.path,&p.alternate_path] {
                let mut prepared=PreparedPath::new(path,p.rotation_mdeg).unwrap();
                assert!(PreparedPath::scratch_bytes(path.len()) >=
                    (prepared.edges.capacity()*std::mem::size_of::<[Vertex;2]>()
                    +prepared.outward.capacity()*std::mem::size_of::<[Option<Vertex>;2]>()) as u64);
                for at in 0..path.len() {
                    assert_eq!(prepared.edges[at],edges(&path[at],p.rotation_mdeg));
                    for side in [-1,1] {
                        let expected=outward_at(path,at,side,p.rotation_mdeg);
                        assert_eq!(prepared.outward_at(path,at,side),expected);
                        assert_eq!(prepared.outward_at(path,at,side),expected);
                    }
                }
            }
            let token=cancellation::CancellationToken::default();
            let result=token.run(|| { token.cancel(); PreparedPath::new(&p.path,p.rotation_mdeg).map(|_|()) });
            assert_eq!(result.unwrap_err().code,"E_CANCELLED");
        }
    }
    #[test]
    fn ordered_geometry_matches_preparation_baseline() {
        use sha2::{Digest, Sha256};
        #[derive(Default)]
        struct Transcript(Vec<serde_json::Value>);
        impl TrackGeometry for Transcript {
            fn triangle(&mut self, v: [Vertex;3], surface: Surface, id: &str, spawnable: bool) -> Result<()> {
                self.0.push(serde_json::json!([v,surface,id,spawnable])); Ok(())
            }
            fn solid(&mut self, id: &str, shape: SolidShape) -> Result<()> {
                self.0.push(match shape {
                    SolidShape::Convex(c) => serde_json::json!([id,"convex",c]),
                    SolidShape::Box { min, max } => serde_json::json!([id,"box",min,max]),
                    _ => panic!("unexpected track solid"),
                }); Ok(())
            }
        }
        // Synthetic ordered geometry, renewed for shared road sections and
        // final-vertex diagonals, canonical ports and curvature-continuous loop
        // feet and analytic overpass ribbons. Includes IDs, materials and spawn
        // eligibility; ribbon corrections also change wall clipping/convexes.
        let mut transcript=Transcript::default();
        for id in ["straight","gentle90","curve_up","curve_left_down","spiral_up","straight_narrow","loop","overpass","finish_plaza","cylinder"] {
            for rotation in [[0,0,0],[12000,35000,7000]] {
                let mut source=variant(id,600,400,600);
                source.rotation_mdeg=rotation;
                let p=materialize(&source);
                let mut other=variant("gentle90",600,400,600);
                other.rotation_mdeg=rotation;
                other.origin_cm=[150,0,300];
                let neighbor=materialize(&other);
                for neighbors in [vec![],vec![&neighbor]] {
                    let mut volumes=vec![];
                    generate_piece(&p,0,&mut transcript,&neighbors,&mut volumes).unwrap();
                    emit(&volumes,&mut transcript).unwrap();
                }
            }
        }
        let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(&transcript.0).unwrap()));
        println!("GEOMETRY_TRANSCRIPT count={} sha256={digest}",transcript.0.len());
        assert_eq!(transcript.0.len(),90620);
        assert_eq!(digest,"704f8e52a95fa305bfc15972a67e1646665fe5b6896ce2f9301b7e56ee841f13");
    }
    #[derive(Default)]
    struct Mesh { triangles: Vec<[Vertex;3]>, solids: Vec<CollisionConvex>, spawnable: bool }
    impl TrackGeometry for Mesh {
        fn triangle(&mut self, v: [Vertex;3], _: Surface, _: &str, spawnable: bool) -> Result<()> {
            self.triangles.push(v); self.spawnable |= spawnable; Ok(())
        }
        fn solid(&mut self, _: &str, shape: SolidShape) -> Result<()> { if let SolidShape::Convex(c)=shape { self.solids.push(c); } Ok(()) }
    }
    #[test]
    fn closed_outward_walls_on_straight_curve_slope_taper_and_plaza() {
        for id in ["straight","gentle90","curve_up","curve_left_down","spiral_up","straight_narrow","loop","finish_plaza"] {
            let p=materialize(&variant(id,600,400,600));
            let mut discarded=Mesh::default(); let mut volumes=vec![];
            generate_piece(&p,0,&mut discarded,&[],&mut volumes).unwrap();
            assert!(!volumes.is_empty(),"{id}");
            let mut out=Mesh::default(); emit(&volumes,&mut out).unwrap();
            assert!(!out.solids.is_empty());
            assert!(out.solids.iter().all(|c| c.valid(100_000_000)), "{id}: valid occupancy");
            let mut exterior=std::collections::BTreeMap::<[Vertex;3],i32>::new();
            for c in &out.solids { for indices in &c.faces {
                let mut face=indices.map(|i|c.vertices[i as usize]);
                let first=(0..3).min_by_key(|&i|face[i]).unwrap();face.rotate_left(first);
                let sign=if face[1]<face[2] {1} else {-1};face.sort();
                *exterior.entry(face).or_default()+=sign;
            }}
            exterior.retain(|_,sign|*sign!=0);
            assert!(exterior.values().all(|sign|sign.abs()==1), "{id}: overlapping boundary faces");
            let rendered: std::collections::BTreeSet<_>=out.triangles.iter().map(|face|{let mut f=*face;f.sort();f}).collect();
            assert_eq!(rendered,exterior.keys().copied().collect(),"{id}: render/collision and occupied exterior");
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
    fn ordinary_curve_costs_and_connected_ports() {
        for id in ["gentle90", "hairpin", "curve_up", "spiral90_left_up", "spiral_up", "spiral_down", "free_curve", "loop", "cylinder_curve"] {
            let mut instance = authoring::instance("cost", id, if id=="loop" {400} else {600});
            if id == "free_curve" {
                instance.control_points = vec![[0,0,0], [0,0,1800], [1800,0,3600], [3600,0,3600]];
            }
            let p = authoring::piece(&instance).unwrap();
            if let Some(old) = match id {
                "gentle90"|"curve_up" => Some(36), "hairpin" => Some(46),
                "spiral90_left_up" => Some(30), "spiral_up"|"spiral_down" => Some(108),
                "free_curve" => Some(45), _ => None,
            } {
                let ceiling=if id=="spiral90_left_up" {0.70} else {0.65};
                assert!((p.path.len()-1) as f64 <= old as f64*ceiling,"{id}: simplification budget");
            }
            let mut road = Mesh::default(); let mut volumes = vec![];
            generate_piece(&p,0,&mut road,&[],&mut volumes).unwrap();
            let mut wall = Mesh::default(); emit(&volumes,&mut wall).unwrap();
            // Actual emitted wall triangles at 25339aa, not longitudinal stations.
            let baseline = match id { "gentle90"|"curve_up" => 360, "hairpin" => 472,
                "free_curve" => 440, "spiral90_left_up" => 328,
                "spiral_up"|"spiral_down" => 1048, _ => 0 };
            if ["gentle90","hairpin","free_curve"].contains(&id) {
                assert!(wall.triangles.len() as f64 <= baseline as f64*0.65,"{id}: >=35% wall reduction");
            }
            if baseline>0 { println!("WALL_REDUCTION {id} before={baseline} after={} percent={:.2}",wall.triangles.len(),100.0*(1.0-wall.triangles.len() as f64/baseline as f64)); }
            assert!(geometry::ordinary_grade_valid(&p));
            assert!(wall.solids.iter().all(|c| c.valid(100_000_000)));
            let next = authoring::snap(&authoring::instance("next", "straight", 600), &instance).unwrap();
            let next = authoring::piece(&next).unwrap();
            if !["loop","cylinder_curve"].contains(&id) {
                assert_eq!(p.path.last().unwrap().position_cm, next.path[0].position_cm, "{id}: shared port");
            }
            println!("CURVE_COST {id} segments={} road_triangles={} wall_triangles={} wall_volumes={} collision_solids={}",
                p.path.len()-1, road.triangles.len(), wall.triangles.len(), volumes.len(), wall.solids.len());
        }
    }
    #[test]
    fn duplicate_vertices_concave_corner_and_exact_zero_volume() {
        let duplicate=[
            [[0,0,0],[0,60,0],[-50,60,0],[-50,0,0]],
            [[0,0,0],[0,60,0],[-49,60,1],[-49,0,1]],
        ];
        let concave=[
            [[-300,0,800],[-300,120,800],[-350,120,800],[-350,0,800]],
            [[-363,0,829],[-363,120,829],[-385,120,785],[-385,0,785]],
        ];
        let small=[[0,0,0],[0,1,0]].map(|base|ring(base,[1,0,0],[0,0,1],false));
        for rings in [duplicate,concave,small] {
            let parts=tetrahedra(rings);
            assert!(!parts.is_empty());
            assert!(parts.len() as u64<=MAX_TETRAHEDRA);
            assert!(parts.iter().all(|p|p.valid(100_000_000)));
            assert_eq!(parts,tetrahedra(rings),"stable ordering");
        }
        assert!(tetrahedra([duplicate[0];2]).is_empty());
        let folded=[
            [[-201,0,2402],[-201,60,2402],[-232,60,2351],[-232,0,2351]],
            [[-203,0,2403],[-203,60,2403],[-222,60,2352],[-222,0,2352]],
        ];
        let parts=tetrahedra(folded);
        assert!(!parts.is_empty(),"nonzero quantized wedges remain occupied");
        assert!(parts.iter().all(|p|p.valid(100_000_000)));
    }
    #[test]
    fn seed7_start_cell_occupancy_is_valid() {
        let d=document(&Settings {seed:7,duration_seconds:60,circuit:false,..Default::default()}).unwrap();
        let a=d.assembled_track.as_ref().unwrap();
        let p=a.pieces[0].path[0].position_cm;
        let cell=d.cell_at([p[0],p[2]]).unwrap();
        let cost=crate::estimate_generation(&d,cell,500_000).unwrap();
        let generated=crate::generate_with_occupancy(crate::GenerationInput {
            document:&d,cell,heightgrid:None,max_triangles:500_000,
        },crate::MAX_OCCUPIED_SOLIDS).unwrap();
        assert!(generated.solids.len() as u64<=cost.occupied_solids);
        assert!(generated.chunk.triangles.len() as u64<=cost.triangles);
        assert!(cost.generation_scratch_bytes>SCRATCH_PER_VOLUME);
        assert_eq!(crate::generate_with_occupancy(crate::GenerationInput {
            document:&d,cell,heightgrid:None,max_triangles:500_000,
        },generated.solids.len()-1).unwrap_err().code,"E_BUDGET");
        let convexes: Vec<_>=generated.solids.iter().filter_map(|s|if let SolidShape::Convex(c)=&s.shape {Some((&s.object_id,c))} else {None}).collect();
        assert!(!convexes.is_empty());
        for (id,c) in &convexes { assert!(c.valid(100_000_000), "{id}: {c:?}"); }
        println!("seed7 start {cell:?}: {} solids, {} convexes",generated.solids.len(),convexes.len());
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
