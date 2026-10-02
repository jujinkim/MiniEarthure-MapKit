//! Remove only the portions of a wall inside a connected road's driving area.
use super::*;

pub(super) fn neighbors(a: &Assembly, index: usize) -> Vec<&Piece> {
    let Some(source) = a.authoring.as_ref().or(a.seed_source.as_ref()) else { return vec![] };
    let id = &source.instances[index].id;
    let mut ids = std::collections::BTreeSet::new();
    for c in &source.connections {
        if &c.from == id { ids.insert(c.to.as_str()); }
        if &c.to == id { ids.insert(c.from.as_str()); }
        for other in &source.connections {
            if c.from == other.from && &c.to == id { ids.insert(other.to.as_str()); }
            if c.to == other.to && &c.from == id { ids.insert(other.from.as_str()); }
        }
    }
    source.instances.iter().enumerate().filter(|(i,p)| *i != index && ids.contains(p.id.as_str()))
        .map(|(i,_)| &a.pieces[i]).collect()
}

// Clip against the actual rendered/colliding road triangles. Chord-aligned
// rectangles leave a wedge at every curved/tapered join (the barcode walls).
fn triangle_interval(rings: [[Vertex; 4]; 2], triangle: [Vertex; 3]) -> Option<(f64,f64)> {
    let sub = |a: Vertex,b: Vertex| std::array::from_fn::<_,3,_>(|j| (a[j]-b[j]) as f64);
    let cross = |a: [f64;3],b: [f64;3]| [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
    let dot = |a: [f64;3],b: [f64;3]| (0..3).map(|j| a[j]*b[j]).sum::<f64>();
    let raw = cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]));
    let length = libm::sqrt(dot(raw,raw));
    if length<1e-9 { return None; }
    let normal = raw.map(|v|v/length);
    let mut planes = vec![(normal,triangle[0],-5.0),(normal.map(|v|-v),triangle[0],-5.0)];
    for i in 0..3 {
        let edge = sub(triangle[(i+1)%3],triangle[i]);
        planes.push((cross(normal,edge),triangle[i],0.0));
    }
    let mut lo: f64=0.0;
    let mut hi: f64=1.0;
    for (axis,origin,min) in planes {
        // The interpolated support bounds every corner of the entire section.
        // This is conservative for twisted sections; no outer/top corner can
        // remain inside a connected lane when the inner edge misses it.
        let support = |ring: [Vertex;4]| ring.into_iter().map(|v|dot(sub(v,origin),axis)-min).fold(f64::NEG_INFINITY,f64::max);
        let start=support(rings[0]);
        let delta=support(rings[1])-start;
        if delta.abs()<1e-9 { if start< -1e-7 { return None; } }
        else if delta>0.0 { lo=lo.max(-start/delta); }
        else { hi=hi.min(-start/delta); }
        if hi<=lo { return None; }
    }
    Some((lo,hi))
}

pub(super) fn visible_volume(rings: [[Vertex;4];2], neighbors: &[&Piece], alternate: &[Sample]) -> Vec<(f64,f64)> {
    let mut hidden = vec![];
    for path in neighbors.iter().flat_map(|p| [&p.path[..],&p.alternate_path[..]]).chain(std::iter::once(alternate)) {
        for w in path.windows(2) {
            if w.iter().any(|s| ["flight","loop","cylinder","halfpipe"].contains(&s.mode.as_str())) { continue; }
            let [al,ar]=geometry::ribbon_edges(&w[0],2);
            let [bl,br]=geometry::ribbon_edges(&w[1],2);
            for triangle in [[al,bl,br],[al,br,ar]] {
                if let Some(range)=triangle_interval(rings,triangle) { hidden.push(range); }
            }
        }
    }
    hidden.sort_by(|a,b|a.0.total_cmp(&b.0));
    let mut out=vec![];
    let mut cursor: f64=0.0;
    for (lo,hi) in hidden {
        if lo>cursor { out.push((cursor,lo)); }
        cursor=cursor.max(hi);
    }
    if cursor<1.0 { out.push((cursor,1.0)); }

    out
}

#[cfg(test)]
fn visible(a: Vertex, b: Vertex, neighbors: &[&Piece], alternate: &[Sample]) -> Vec<(Vertex,Vertex)> {
    let at = |t: f64| std::array::from_fn(|j|round(a[j] as f64+(b[j]-a[j]) as f64*t));
    visible_volume([[a;4],[b;4]],neighbors,alternate).into_iter().map(|(a,b)|(at(a),at(b))).filter(|(a,b)|distance(*a,*b)>0).collect()
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn full_volume_cuts_outer_and_top_intrusions() {
        let p=variant("straight",400,400,400);
        let rings=[[240,0,100],[240,0,600]].map(|v| walls::ring(v,[-50,0,0],[0,120,0],false));
        assert!(visible([240,0,100],[240,0,600],&[&p],&[]).len()==1);
        assert!(visible_volume(rings,&[&p],&[]).is_empty());
        let rings=[[0,-80,100],[0,-80,600]].map(|v| walls::ring(v,[50,0,0],[0,120,0],false));
        assert!(visible_volume(rings,&[&p],&[]).is_empty());
        let rings=[[250,0,100],[250,0,600]].map(|v| walls::ring(v,[50,0,0],[0,120,0],false));
        assert_eq!(visible_volume(rings,&[&p],&[]),vec![(0.0,1.0)]);
    }
    #[test] fn trims_interior_preserves_outer_and_grade_separation() {
        let p=variant("straight",400,400,400);
        assert!(visible([0,0,100],[0,0,600],&[&p],&[]).is_empty());
        assert_eq!(visible([200,0,100],[200,0,600],&[&p],&[]).len(),1);
        assert_eq!(visible([0,100,100],[0,100,600],&[&p],&[]).len(),1);
        let kept=visible([-300,0,400],[300,0,400],&[&p],&[]);
        assert_eq!(kept.len(),2);
        assert!(kept[0].1[0]< -190 && kept[1].0[0]>190);
    }
    #[test]
    fn curved_and_tapered_road_has_no_barcode_wall_fragments() {
        for id in ["gentle90", "curve_up", "curve_left_down", "straight_narrow"] {
            let p=variant(id,600,400,600);
            for w in p.path.windows(2) {
                let inside = |s: &Sample| {
                    let basis=geometry::basis(s);
                    std::array::from_fn(|j| s.position_cm[j]+round(basis[j][0]*s.lateral_cm as f64*0.8))
                };
                let a=inside(&w[0]);
                let b=inside(&w[1]);
                assert!(visible(a,b,&[&p],&[]).is_empty(), "{id}: interior wall {a:?} -> {b:?}");
            }
        }
    }

}
