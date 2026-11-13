//! Shared display geometry for regional buildings. Source solids/collision are untouched.
//! Roofs, wall edges and every module are clipped by the same cell planes near and far.
use crate::{appearance, cancellation::checkpoint, Bounds, Building, Cell, MapDocument, Result};
use std::collections::BTreeSet;
type V = [f64; 3];
pub const MODULE_LIMIT: usize = 8192;
pub const FACE_LIMIT: usize = 131072;
#[derive(Clone, Debug)]
pub struct Face { pub vertices: [V; 3], pub key: String, pub color: [u8; 4], pub seed: f32, pub base_cm: f32, pub eaves_cm: f32 }
impl Face {
    /// Source-space unit normal before the renderer converts positions to f32.
    pub fn normal(&self) -> Option<V> {
        let u: V=std::array::from_fn(|i|self.vertices[1][i]-self.vertices[0][i]);
        let v: V=std::array::from_fn(|i|self.vertices[2][i]-self.vertices[0][i]);
        let cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        let length=libm::hypot(libm::hypot(cross[0],cross[1]),cross[2]);
        (length>0. && length.is_finite()).then(||cross.map(|x|x/length))
    }
}
#[derive(Default)]
pub struct Display { pub faces: Vec<Face>, pub objects: BTreeSet<String>, pub omitted_modules: usize }
struct Builder<'a> { out: Display, bounds: &'a Bounds, key: String, color: [u8; 4], seed: f32, eaves: f32, base: f32, ceiling: f64, module: bool, modules: usize }
impl Builder<'_> {
    fn material(&mut self, material: &str) {
        self.key = format!("material/{material}");
        let c = appearance::catalog().materials[material].color;
        self.color = [c[0],c[1],c[2],255];
    }
    fn triangle(&mut self, points: [V;3]) -> Result<()> {
        checkpoint()?;
        let mut poly = points.map(|mut p| {if self.module {p[1]=p[1].clamp(self.base as f64,self.ceiling);} p}).to_vec();
        for (axis, value, sign) in [(0,self.bounds.min[0] as f64,1.),(0,self.bounds.max[0] as f64,-1.),(2,self.bounds.min[1] as f64,1.),(2,self.bounds.max[1] as f64,-1.)] {
            let mut next = Vec::new();
            for i in 0..poly.len() {
                let (a,b) = (poly[i],poly[(i+1)%poly.len()]);
                let (da,db) = ((a[axis]-value)*sign,(b[axis]-value)*sign);
                if da >= 0. { next.push(a); }
                if (da >= 0.) != (db >= 0.) { let t=da/(da-db); next.push(std::array::from_fn(|k|a[k]+t*(b[k]-a[k]))); }
            }
            poly=next;
        }
        for k in 1..poly.len().saturating_sub(1) {
            let face=Face { vertices:[poly[0],poly[k],poly[k+1]],key:self.key.clone(),color:self.color,seed:self.seed,base_cm:self.base,eaves_cm:self.eaves };
            // A clipping-plane corner or a clamped decorative face can have no
            // area. It contributes neither geometry nor decoration allowance.
            if face.normal().is_none() { continue; }
            if self.module && self.modules >= MODULE_LIMIT { self.out.omitted_modules += 1; continue; }
            if self.out.faces.len() >= FACE_LIMIT { return Err(crate::error("E_BUDGET","Regional display exceeds cell face allowance")); }
            self.out.faces.push(face);
            if self.module { self.modules += 1; }
        }
        Ok(())
    }
    fn quad(&mut self, p:[V;4]) -> Result<()> { self.triangle([p[0],p[1],p[2]])?; self.triangle([p[0],p[2],p[3]]) }
    // Shallow facade box in an edge's own frame. x is along, z is outward.
    fn panel(&mut self, a:V, u:V, n:V, c:V, size:V) -> Result<()> {
        let v: [V;8]=std::array::from_fn(|i| {
            let x=c[0]+if i&1==0 {-size[0]/2.}else{size[0]/2.};
            let y=c[1]+if i&2==0 {-size[1]/2.}else{size[1]/2.};
            let z=c[2]+if i&4==0 {-size[2]/2.}else{size[2]/2.};
            [a[0]+u[0]*x+n[0]*z,y,a[2]+u[2]*x+n[2]*z]
        });
        for q in [[0,1,3,2],[4,6,7,5],[0,4,5,1],[2,3,7,6],[0,2,6,4],[1,5,7,3]] { self.quad(q.map(|i|v[i]))?; }
        Ok(())
    }
}
fn aabb(p:&[crate::Point])->Bounds { Bounds {min:std::array::from_fn(|i|p.iter().map(|v|v[i]).min().unwrap()),max:std::array::from_fn(|i|p.iter().map(|v|v[i]).max().unwrap())} }
fn overlaps(b:&Building, r:&Bounds) -> bool { let a=aabb(&b.footprint); (0..2).all(|i| a.max[i]+100>=r.min[i] && a.min[i]-100<=r.max[i]) }
/// Source-sized upper bound, plus the fixed decoration allowance. Includes clipping fan growth.
pub fn face_bound(d:&MapDocument, cell:Cell) -> Result<u64> {
    let r=d.cell_bounds(cell)?;
    let mut n=0u64; let mut solids=0u64;
    for b in d.buildings.iter().filter(|b| !b.style_id.is_empty()) { for s in b.solids().filter(|s|overlaps(s,&r)) {
        let vertices=s.footprint.len()+s.holes.iter().map(Vec::len).sum::<usize>();
        n=n.saturating_add((vertices as u64+2*s.holes.len() as u64)*96);
        solids+=1;
    }}
    Ok(if n==0 {0}else{n.saturating_add((solids*2048).min(MODULE_LIMIT as u64)).min(FACE_LIMIT as u64)})
}
/// Renderer batches share material and 32 m buckets. Account for occupied bucket
/// ranges and partial 256-triangle batches, rather than one draw node per face.
pub fn presentation_bound(d:&MapDocument,cell:Cell)->Result<u64> {
    let faces=face_bound(d,cell)?; if faces==0 {return Ok(0);}
    let r=d.cell_bounds(cell)?;let mut groups=0u64;
    for b in d.buildings.iter().filter(|b|!b.style_id.is_empty()) {for s in b.solids().filter(|s|overlaps(s,&r)) {
        let a=aabb(&s.footprint);
        let nx=((a.max[0].min(r.max[0])+100).div_euclid(3200)-(a.min[0].max(r.min[0])-100).div_euclid(3200)+1) as u64;
        let nz=((a.max[1].min(r.max[1])+100).div_euclid(3200)-(a.min[1].max(r.min[1])-100).div_euclid(3200)+1) as u64;
        let ny=(s.height_cm as u64+1000)/3200+2;
        groups=groups.saturating_add(nx*nz*ny*12);
    }}
    Ok(faces*1280+(groups+faces/256).min(faces)*12288)
}
pub fn generate(d:&MapDocument, cell:Cell) -> Result<Display> {
    let bounds=d.cell_bounds(cell)?;
    let mut out=Builder{out:Display::default(),bounds:&bounds,key:String::new(),color:[255;4],seed:0.,eaves:0.,base:0.,ceiling:0.,module:false,modules:0};
    let scale=1. / crate::osm_source_denominator(d).unwrap_or(1) as f64;
    for record in &d.buildings {
        checkpoint()?;
        let Some(style)=appearance::building(&record.style_id) else {continue;};
        for (index,b) in record.solids().enumerate().filter(|(_,b)|overlaps(b,&bounds)) {
            out.out.objects.insert(record.id.clone());
            let hash=crate::sha256(record.id.as_bytes());
            out.seed=u32::from_str_radix(&hash[..6],16).unwrap() as f32 / 16777215.;
            // Grouped records hold many houses; each solid keeps its own palette roof in every cell.
            let style=appearance::BuildingStyle{roof_material:style.roof_material_of(&format!("{}#{index}",record.id)),..style};
            solid(&mut out,&b,&style,scale)?;
        }
    }
    Ok(out.out)
}
fn solid(out:&mut Builder, b:&Building, style:&appearance::BuildingStyle, scale:f64) -> Result<()> {
    let area=aabb(&b.footprint);
    let size=[(area.max[0]-area.min[0]) as f64,(area.max[1]-area.min[1]) as f64];
    let axis=usize::from(size[0]>size[1]);
    let width=size[axis].max(1.);
    let centre=[(area.min[0]+area.max[0]) as f64/2.,(area.min[1]+area.max[1]) as f64/2.];
    // A source gable already owns its rise; other roof modules stay inside the source height.
    let native_rise=crate::placement::roof_rise(b) as f64;
    let top=b.base_cm as f64+b.height_cm as f64+native_rise;
    let shape=if b.holes.is_empty(){style.roof}else{"flat"};
    // At most 26.6° across the narrow side, 2.5 source m and 40 % of a low envelope
    // (a one-storey house keeps a visible roof above its storey).
    let rise=if shape=="flat"{0.}else if native_rise>0.{native_rise}else{(width*0.25).min(250.*scale).min(b.height_cm as f64*0.4)};
    let eaves=top-rise;
    out.eaves=eaves as f32; out.base=b.base_cm as f32; out.ceiling=top;
    let height=|p:[f64;2]| {
        let x=((p[axis]-centre[axis])/(width/2.)).clamp(-1.,1.);
        let z=((p[1-axis]-centre[1-axis])/(size[1-axis].max(1.)/2.)).clamp(-1.,1.);
        let weight=match shape {
            "gabled"=>1.-x.abs(), "hipped"|"pyramidal"=>1.-x.abs().max(z.abs()),
            "dome"=>libm::sqrt((1.-x*x-z*z).max(0.)), "vault"=>libm::sqrt((1.-x*x).max(0.)),
            "skillion"=>(x+1.)/2., _=>0.,
        };
        eaves+rise*weight
    };
    out.module=false;
    out.key=b.style_id.clone(); out.color=appearance::color(&b.style_id,false).unwrap();
    for ring in std::iter::once(&b.footprint).chain(b.holes.iter()) {
        for i in 0..ring.len() {
            let (a,z)=(ring[i].map(|v|v as f64),ring[(i+1)%ring.len()].map(|v|v as f64));
            for k in 0..4 {
                let p=std::array::from_fn(|j| a[j]+(z[j]-a[j])*k as f64/4.);
                let q=std::array::from_fn(|j| a[j]+(z[j]-a[j])*(k+1) as f64/4.);
                out.quad([[p[0],b.base_cm as f64,p[1]],[q[0],b.base_cm as f64,q[1]],[q[0],height(q),q[1]],[p[0],height(p),p[1]]])?;
            }
        }
    }
    out.material(style.roof_material);
    out.key=format!("roof/{}",style.roof_material);
    let rgb=appearance::catalog().materials[style.roof_material].roof_color;
    out.color=[rgb[0],rgb[1],rgb[2],255];
    for t in crate::courtyard::triangulate(b,&mut 0)? {
        roof(out,t.map(|p|p.map(|v|v as f64)),if rise>0.{2}else{0},&height)?;
    }
    out.module=true;
    let architecture=&appearance::catalog().architectures[style.architecture];
    let kind=&appearance::catalog().building_types[style.kind];
    let modules:BTreeSet<&str>=architecture.modules_for(style.kind).iter().chain(kind.modules.iter()).map(String::as_str).collect();
    let mut edges:Vec<_>=(0..b.footprint.len()).map(|i| {
        let a=b.footprint[i];let z=b.footprint[(i+1)%b.footprint.len()];
        let length=libm::hypot((z[0]-a[0]) as f64,(z[1]-a[1]) as f64);(i,length)
    }).collect();
    edges.sort_by(|a,b|b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for (rank,(i,length)) in edges.into_iter().take(4).enumerate() {
        if length<100.*scale {continue;}
        let p=b.footprint[i];let q=b.footprint[(i+1)%b.footprint.len()];
        let a=[p[0] as f64,0.,p[1] as f64];let u=[(q[0]-p[0]) as f64/length,0.,(q[1]-p[1]) as f64/length];
        let mut n=[-u[2],0.,u[0]];
        let test=[((p[0]+q[0]) as f64/2.+n[0]*2.).round() as i64,((p[1]+q[1]) as f64/2.+n[2]*2.).round() as i64];
        if b.contains(test){n=n.map(|v|-v);}
        edge_modules(out,a,u,n,length,b.base_cm as f64,eaves,scale,style,&modules,rank)?;
    }
    Ok(())
}
fn roof(out:&mut Builder,t:[[f64;2];3],depth:u8,height:&impl Fn([f64;2])->f64) -> Result<()> {
    if depth==0 {return out.triangle(t.map(|p|[p[0],height(p),p[1]]));}
    let ab=std::array::from_fn(|i|(t[0][i]+t[1][i])/2.);let bc=std::array::from_fn(|i|(t[1][i]+t[2][i])/2.);let ca=std::array::from_fn(|i|(t[2][i]+t[0][i])/2.);
    for v in [[t[0],ab,ca],[ab,t[1],bc],[ca,bc,t[2]],[ab,bc,ca]] {roof(out,v,depth-1,height)?;} Ok(())
}
#[allow(clippy::too_many_arguments)]
fn edge_modules(out:&mut Builder,a:V,u:V,n:V,length:f64,base:f64,eaves:f64,s:f64,style:&appearance::BuildingStyle,modules:&BTreeSet<&str>,rank:usize)->Result<()> {
    let mid=length/2.;let pitch=300.*s;let h=eaves-base;let top=(eaves-8.*s).max(base);
    let has=|names:&[&str]|names.iter().any(|n|modules.contains(n));
    if has(&["deep_eaves","tiled_eaves","ridge","cornice","parapet","rounded_parapet"]) {
        out.material(if has(&["deep_eaves","tiled_eaves","ridge"]){style.roof_material}else{style.material});
        out.panel(a,u,n,[mid,top,6.*s],[length,12.*s,if has(&["deep_eaves","tiled_eaves"]){50.*s}else{20.*s}])?;
    }
    if modules.contains("stone_plinth") {out.material("stone");out.panel(a,u,n,[mid,base+25.*s,3.*s],[length,50.*s,10.*s])?;}
    if has(&["timber_frame","stilts"]) {
        out.material(if style.architecture=="polar_station"{"metal"}else{"wood"});
        for x in [20.*s,length-20.*s] {out.panel(a,u,n,[x,(base+eaves)/2.,3.*s],[12.*s,h,12.*s])?;}
    }
    if rank>1{return Ok(());}
    if modules.contains("blue_shutter") {
        out.material("blue_tile");
        for x in [mid-65.*s,mid+65.*s] {out.panel(a,u,n,[x,base+(pitch*0.55).min(h*0.55),4.*s],[32.*s,(100.*s).min(h*0.5),8.*s])?;}
    }
    if has(&["balcony","veranda","porch","rail"]) && h>pitch*1.2 {
        let y=base+pitch;let width=(length*0.7).min(350.*s);
        out.material(if modules.contains("veranda"){"wood"}else{"stone"});
        out.panel(a,u,n,[mid,y,20.*s],[width,12.*s,65.*s])?;
        out.material("metal");
        out.panel(a,u,n,[mid,y+80.*s,50.*s],[width,5.*s,6.*s])?;
        for x in [mid-width/2.,mid,mid+width/2.] {out.panel(a,u,n,[x,y+40.*s,50.*s],[5.*s,80.*s,6.*s])?;}
    }
    if rank>0{return Ok(());}
    let wide=matches!(style.kind,"warehouse"|"barn"|"shopping"|"factory");
    let door=(if wide {300.*s}else{120.*s}).min(length*0.55);
    let dh=(if wide {260.*s}else{220.*s}).min(h*0.85);
    out.material(if wide{"metal"}else if matches!(style.kind,"office"|"hotel"|"shopping"|"station"){"glass"}else{"wood"});
    out.panel(a,u,n,[mid,base+dh/2.,2.*s],[door,dh,5.*s])?;
    if has(&["arch","shade_arcade","courtyard_gate","portal"]) {
        out.material(if style.architecture=="venetian"{"stone"}else{style.material});
        let radius=(door/2.+18.*s).min(length/2.);
        for k in 0..8 {
            let angle=(k as f64+0.5)*std::f64::consts::PI/8.;
            let x=mid+radius*libm::cos(angle);let y=base+dh*0.7+radius*libm::sin(angle);
            if y<eaves-8.*s {out.panel(a,u,n,[x,y,6.*s],[radius*0.40,18.*s,16.*s])?;}
        }
    }
    if has(&["shop_canopy","canopy","ambulance_canopy","platform_canopy","veranda","porch","colonnade"]) {
        out.material(if style.kind=="hospital"{"whitewash"}else if style.architecture=="venetian"{"stone"}else{"metal"});
        let width=if style.kind=="station"{length*0.85}else{(door*1.8).min(length*0.8)};
        let depth=if style.kind=="station"{120.*s}else{75.*s};let y=base+(dh+20.*s).min(h*0.9);
        out.panel(a,u,n,[mid,y,depth/2.],[width,12.*s,depth])?;
        if has(&["colonnade","veranda","porch"]) {for x in [mid-width/2.+10.*s,mid+width/2.-10.*s] {out.panel(a,u,n,[x,(base+y)/2.,depth-10.*s],[15.*s,y-base,15.*s])?;}}
    }
    if has(&["vent","service_door","loading_bay","barn_door"]) {
        out.material("metal");for k in 0..3 {out.panel(a,u,n,[length*0.2,base+h*0.75+k as f64*8.*s,4.*s],[(60.*s).min(length*0.2),4.*s,10.*s])?;}
    }
    if modules.contains("steps") {out.material("stone");for k in 0..3 {out.panel(a,u,n,[mid,base+(3-k) as f64*5.*s,(k as f64+0.5)*20.*s],[door*1.5,10.*s,20.*s])?;}}
    if modules.contains("chimney") {out.material("brick");out.panel(a,u,n,[length*0.25,eaves-30.*s,-25.*s],[40.*s,60.*s,40.*s])?;}
    Ok(())
}
