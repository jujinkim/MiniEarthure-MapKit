//! Explicit current-v1 grind interaction. Supporting colliders are independent.
use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub line: String,
    pub end: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GrindLine {
    pub id: String,
    /// Two points form a straight line; 3n+1 points form cubic Bezier spans.
    pub control_points: Vec<Vertex>,
    pub up: Vertex,
    pub capture_width_cm: u32,
    pub start_connections: Vec<Endpoint>,
    pub end_connections: Vec<Endpoint>,
}
impl GrindLine {
    pub fn samples(&self) -> Vec<assembled_track::Sample> {
        let cp = if self.control_points.len()==2 {
            let a=self.control_points[0]; let b=self.control_points[1];
            vec![a,std::array::from_fn(|j|a[j]+(b[j]-a[j])/3),std::array::from_fn(|j|a[j]+(b[j]-a[j])*2/3),b]
        } else { self.control_points.clone() };
        let mut path=assembled_track::grind_path(&cp,self.capture_width_cm);
        // Parallel transport the author-specified up frame; reverse travel never
        // changes this frame or flips the balance gauge.
        let mut up=self.up.map(|v|v as f64/1e6);
        for s in &mut path {
            let f=s.forward.map(|v|v as f64/1e6);
            let dot: f64=(0..3).map(|j|up[j]*f[j]).sum();
            let n=std::array::from_fn::<_,3,_>(|j|up[j]-dot*f[j]);
            let length=libm::sqrt(n.iter().map(|v|v*v).sum()).max(1e-9);
            up=n.map(|v|v/length);
            s.normal=up.map(|v|libm::round(v*1e6) as i64);
            s.safe=false; s.mode="grind".into();
        }
        path
    }
    pub fn intersects(&self,b: &Bounds) -> bool {
        (0..2).all(|j| self.control_points.iter().map(|v|v[j*2]).min().unwrap_or(0)-self.capture_width_cm as i64<=b.max[j]
            && self.control_points.iter().map(|v|v[j*2]).max().unwrap_or(0)+self.capture_width_cm as i64>=b.min[j])
    }
    pub fn resolved_json(&self) -> serde_json::Value {
        let mut v=serde_json::to_value(self).unwrap();
        v["samples"]=serde_json::to_value(self.samples()).unwrap();
        v["capture_height_cm"]=serde_json::json!(4);
        v
    }
    pub fn memory_bytes(&self)->u64 { 1024+self.control_points.len() as u64*512+self.samples().len() as u64*512 }
}
pub fn validate_geometry(lines: &[GrindLine]) -> Result<()> {
    if lines.len()>256 { return Err(error("E_GRIND_BUDGET","too many grind lines")); }
    let mut ids=std::collections::BTreeSet::new();
    let mut samples=0;
    for l in lines {
        cancellation::checkpoint()?;
        let n=l.control_points.len();
        if l.id.is_empty() || l.id.len()>32 || !ids.insert(&l.id) || !(5..=100).contains(&l.capture_width_cm)
            || !(n==2 || (n>=4 && n<=49 && (n-1)%3==0))
            || l.control_points.iter().flatten().any(|v|v.unsigned_abs()>10_000_000)
            || l.up.iter().any(|v|v.unsigned_abs()>1_000_000)
            || (l.up.iter().map(|v|(*v as f64/1e6).powi(2)).sum::<f64>()-1.0).abs()>0.001
            || l.start_connections.len()>16 || l.end_connections.len()>16 {
            return Err(error("E_GRIND_SOURCE","invalid grind identifier, points, frame or width"));
        }
        if l.start_connections.iter().chain(&l.end_connections).any(|e|e.line.is_empty() || e.line.len()>32) {
            return Err(error("E_GRIND_CONNECTION","invalid endpoint line identifier"));
        }
        let path=l.samples(); samples+=path.len();
        if path.len()<2 || path.windows(2).any(|w|w[0].position_cm==w[1].position_cm)
            || path.iter().any(|s| s.forward.iter().map(|v|(*v as f64/1e6).powi(2)).sum::<f64>()<0.99
                || s.normal.iter().map(|v|(*v as f64/1e6).powi(2)).sum::<f64>()<0.99) {
            return Err(error("E_GRIND_SOURCE","degenerate grind path or frame"));
        }
        if samples>32_000 { return Err(error("E_GRIND_BUDGET","grind sample budget exceeded")); }
    }
    Ok(())
}
pub fn validate(lines: &[GrindLine]) -> Result<()> {
    validate_geometry(lines)?;
    for l in lines {
        for (end,links) in [(false,&l.start_connections),(true,&l.end_connections)] {
            let p=if end {l.control_points.last().unwrap()} else {&l.control_points[0]};
            let mut seen=std::collections::BTreeSet::new();
            for link in links {
                let Some(other)=lines.iter().find(|v|v.id==link.line) else { return Err(error("E_GRIND_CONNECTION","missing line")); };
                let q=if link.end {other.control_points.last().unwrap()} else {&other.control_points[0]};
                if !seen.insert((&link.line,link.end)) || (0..3).any(|j|(p[j]-q[j]).abs()>2) {
                    return Err(error("E_GRIND_CONNECTION","duplicate or disconnected endpoint"));
                }
            }
        }
    }
    Ok(())
}
