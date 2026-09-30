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

fn interval(a: Vertex, b: Vertex, w: &[Sample]) -> Option<(f64,f64)> {
    if w.iter().any(|s| ["flight","loop","cylinder","halfpipe"].contains(&s.mode.as_str())) { return None; }
    let delta = std::array::from_fn::<_,3,_>(|j| (w[1].position_cm[j]-w[0].position_cm[j]) as f64);
    let length = libm::sqrt(delta.iter().map(|v|v*v).sum());
    if length < 1.0 { return None; }
    let f = delta.map(|v|v/length);
    let n = w[0].normal.map(|v|v as f64/1e6);
    let r = [n[1]*f[2]-n[2]*f[1],n[2]*f[0]-n[0]*f[2],n[0]*f[1]-n[1]*f[0]];
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for (axis,min,max) in [(f,0.0,length),(n,-5.0,5.0),(r,-(w[0].lateral_cm.min(w[1].lateral_cm) as f64)+2.0,w[0].lateral_cm.min(w[1].lateral_cm) as f64-2.0)] {
        let p: f64 = (0..3).map(|j|(a[j]-w[0].position_cm[j]) as f64*axis[j]).sum();
        let d: f64 = (0..3).map(|j|(b[j]-a[j]) as f64*axis[j]).sum();
        if d.abs()<1e-9 { if p<min || p>max { return None; } }
        else { let x=(min-p)/d; let y=(max-p)/d; lo=lo.max(x.min(y)); hi=hi.min(x.max(y)); }
        if hi<=lo { return None; }
    }
    Some((lo,hi))
}

pub(super) fn visible(a: Vertex, b: Vertex, neighbors: &[&Piece], alternate: &[Sample]) -> Vec<(Vertex,Vertex)> {
    let mut hidden = vec![];
    for path in neighbors.iter().flat_map(|p| [&p.path[..],&p.alternate_path[..]]).chain(std::iter::once(alternate)) {
        for w in path.windows(2) { if let Some(range)=interval(a,b,w) { hidden.push(range); } }
    }
    hidden.sort_by(|a,b|a.0.total_cmp(&b.0));
    let at = |t: f64| std::array::from_fn(|j|round(a[j] as f64+(b[j]-a[j]) as f64*t));
    let mut out=vec![];
    let mut cursor: f64=0.0;
    for (lo,hi) in hidden {
        if lo>cursor { out.push((at(cursor),at(lo))); }
        cursor=cursor.max(hi);
    }
    if cursor<1.0 { out.push((at(cursor),b)); }
    out.retain(|(a,b)|distance(*a,*b)>0);
    out
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn trims_interior_preserves_outer_and_grade_separation() {
        let p=variant("straight",400,400,400);
        assert!(visible([0,0,100],[0,0,600],&[&p],&[]).is_empty());
        assert_eq!(visible([200,0,100],[200,0,600],&[&p],&[]).len(),1);
        assert_eq!(visible([0,100,100],[0,100,600],&[&p],&[]).len(),1);
        let kept=visible([-300,0,400],[300,0,400],&[&p],&[]);
        assert_eq!(kept.len(),2);
        assert!(kept[0].1[0]< -190 && kept[1].0[0]>190);
    }
}
