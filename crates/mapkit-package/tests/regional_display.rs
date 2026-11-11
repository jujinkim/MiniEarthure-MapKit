use mapkit_core::{appearance, regional_display as display, *};
use mapkit_package::*;
use std::collections::BTreeMap;
fn document() -> MapDocument {
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.buildings.truncate(1);let b=&mut d.buildings[0];
    b.footprint=vec![[30000,30000],[31200,30000],[31200,31000],[30000,31000]];
    b.height_cm=900;b.base_cm=100;b.roof="flat".into();b.holes.clear();b.parts.clear();d
}
fn digest(v:&display::Display)->String { sha256(format!("{:?}",v.faces).as_bytes()) }
#[test]
fn every_architecture_is_deterministic_bounded_and_shared_with_distant() {
    for (name,a) in &appearance::catalog().architectures {
        let mut d=document();d.buildings[0].style_id=format!("building/{name}/hotel/{}",a.material);d.normalize();d.validate().unwrap();
        let cell=Cell{x:0,y:0};let mesh=display::generate(&d,cell).unwrap();
        assert!(!mesh.faces.is_empty(),"{name}");assert_eq!(mesh.omitted_modules,0);
        assert_eq!(digest(&mesh),digest(&display::generate(&d,cell).unwrap()));
        assert!(mesh.faces.len() as u64<=display::face_bound(&d,cell).unwrap());
        assert!(mesh.faces.iter().flat_map(|f|f.vertices).all(|p|p[1]>=100. && p[1]<=1000.),"source height envelope {name}");
        let p=read_bytes(&pack_bytes(d,BTreeMap::new()).unwrap()).unwrap();let far=p.generate_distant(cell).unwrap();
        assert!(far.vertices.len()/3<=p.distant_triangle_bound(cell).unwrap() as usize);
        // The complete native regional product, including silhouette modules, is present at distance.
        for f in &mesh.faces {
            let v=f.vertices[0];let point=[v[0] as f32*0.01,v[1] as f32*0.01,-v[2] as f32*0.01];
            assert!(far.vertices.iter().zip(&far.colors).any(|(p,c)|*p==point && *c==f.color),"shared vertex/color {name}");
        }
    }
}
#[test]
fn roofs_preserve_courtyards_and_agree_at_cell_seams() {
    let mut d=document();let b=&mut d.buildings[0];
    b.footprint=vec![[50600,30000],[51800,30000],[51800,31000],[50600,31000]];
    b.style_id="building/cycladic/religious/whitewash/dome/blue_tile".into();
    let l=display::generate(&d,Cell{x:0,y:0}).unwrap();let r=display::generate(&d,Cell{x:1,y:0}).unwrap();
    let seam=|v:&display::Display| {let mut s:Vec<_>=v.faces.iter().filter(|f|f.key.starts_with("roof/")).flat_map(|f|f.vertices).filter(|p|(p[0]-51200.).abs()<1e-6).map(|p|[(p[1]*100.).round() as i64,(p[2]*100.).round() as i64]).collect();s.sort();s.dedup();s};
    assert!(!seam(&l).is_empty());assert_eq!(seam(&l),seam(&r));
    let b=&mut d.buildings[0];b.holes=vec![vec![[51000,30300],[51400,30300],[51400,30700],[51000,30700]]];
    let mesh=display::generate(&d,Cell{x:0,y:0}).unwrap();
    for f in mesh.faces.iter().filter(|f|f.key.starts_with("roof/")) {
        let p=[f.vertices.iter().map(|p|p[0]).sum::<f64>()/3.,f.vertices.iter().map(|p|p[2]).sum::<f64>()/3.];
        assert!(!(p[0]>51000. && p[0]<51400. && p[1]>30300. && p[1]<30700.));
    }
}
#[test]
fn clipped_corner_and_short_modules_keep_nonzero_faces() {
    let mut d=document();d.nodes.clear();d.roads.clear();d.zones.clear();d.free_roam=true;
    d.bounds=Bounds{min:[0,0],max:[6400,6400]};d.cell_size_cm=3200;
    let b=&mut d.buildings[0];
    b.footprint=vec![[3000,3000],[3200,3000],[3200,3200],[3000,3200]];
    b.base_cm=0;b.height_cm=20;
    b.style_id="building/modern/shop/concrete".into();
    d.normalize();d.validate().unwrap();
    for cell in [Cell{x:0,y:0},Cell{x:1,y:0},Cell{x:0,y:1},Cell{x:1,y:1}] {
        let mesh=display::generate(&d,cell).unwrap();
        for f in mesh.faces {
            let u: [f64;3]=std::array::from_fn(|i|f.vertices[1][i]-f.vertices[0][i]);
            let v: [f64;3]=std::array::from_fn(|i|f.vertices[2][i]-f.vertices[0][i]);
            let cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
            assert!(cross.iter().any(|x|*x!=0.),"zero-area face in {cell:?}: {:?}",f.vertices);
        }
    }
}
#[test]
fn face_normals_keep_source_precision_before_scene_conversion() {
    let mut face=display::Face{vertices:[[20_000_000.,10.,20_000_000.],[20_000_001.,10.,20_000_000.],[20_000_000.,10.,20_000_001.]],key:"roof/concrete".into(),color:[255;4],seed:0.,base_cm:0.,eaves_cm:10.};
    assert_eq!(face.normal(),Some([0.,-1.,0.]));
    // The first two corners coincide if absolute coordinates are cast first.
    assert_eq!(face.vertices[0][0] as f32,face.vertices[1][0] as f32);
    face.vertices[1]=face.vertices[0];
    assert_eq!(face.normal(),None);
}
#[test]
fn explicit_roof_contract_and_cancel_are_strict() {
    use appearance::Domain;
    assert!(appearance::valid("building/cycladic/religious/whitewash/dome/blue_tile",Domain::Building));
    for id in ["building/cycladic/detached/whitewash/dome/blue_tile","building/modern/religious/stone/dome/blue_tile","building/cycladic/religious/whitewash/unknown/stone"] {assert!(!appearance::valid(id,Domain::Building));}
    let token=cancellation::CancellationToken::default();token.cancel();
    let result=token.run(||display::generate(&document(),Cell{x:0,y:0}));
    assert!(matches!(result,Err(Error{code,..}) if code=="E_CANCELLED"));
}

#[test]
fn regional_plants_preserve_footprints_physics_and_shared_faces() {
    for kind in ["conifer","palm","orchard","scrub","polar"] {
        let tree=matches!(kind,"conifer"|"palm"|"orchard");
        let style=if kind=="polar"{"polar_scrub"}else{kind};
        let mut d=document();d.nodes.clear();d.roads.clear();d.buildings.clear();d.zones.clear();d.free_roam=true;
        let zone=serde_json::json!({"id":"regional","polygon":[[1000,1000],[3000,1000],[3000,3000],[1000,3000]],"kind":"forest","spacing_cm":600,"density_per_mille":1000,"exclusions":[],"tree":{"asset_id":if tree{"builtin:city_tree"}else{"builtin:shrub"},"radius_cm":if tree{120}else{42},"clearance_cm":0},"style_id":format!("vegetation/{style}")});
        // Styling leaves existing generated collision and placement identities unchanged.
        d.zones.push(serde_json::from_value(zone).unwrap());d.normalize();
        let p=read_bytes(&pack_bytes(d.clone(),BTreeMap::new()).unwrap()).unwrap();let cell=Cell{x:0,y:0};
        let chunk=p.generate(cell,500000).unwrap();let far=p.generate_distant(cell).unwrap();
        assert!(!chunk.objects.is_empty());assert!(far.vertices.len()/3<=p.distant_triangle_bound(cell).unwrap() as usize);
        d.zones[0].style_id.clear();let old=read_bytes(&pack_bytes(d,BTreeMap::new()).unwrap()).unwrap();
        assert_eq!(chunk.hash().unwrap(),old.generate(cell,500000).unwrap().hash().unwrap(),"appearance preserves physics {kind}");
        for variant in &regional_plants::catalog()[kind].variants {for face in variant {for point in face.vertices {
            let reach=libm::hypotf(point[0],point[2])*if tree{1.15}else{1.1}+if tree{0.06}else{0.025};
            assert!(reach<=if tree{1.2}else{0.42},"{kind} footprint {reach}");
        }}}
        if tree {let color=regional_plants::catalog()[kind].color;assert!(far.colors.contains(&[color[0],color[1],color[2],255]));}
    }
}

#[test]
fn explicit_cover_keeps_its_palette_at_local_zero_near_and_far() {
    for ground in ["snow","ice","rock","sand","managed_lawn"] {
        let mut d=document();d.buildings.clear();d.nodes.clear();d.roads.clear();d.zones.clear();d.free_roam=true;
        d.ground_style_id=format!("ground/{ground}");
        let p=read_bytes(&pack_bytes(d,BTreeMap::new()).unwrap()).unwrap();
        let far=p.generate_distant(Cell{x:0,y:0}).unwrap();
        let c=appearance::catalog().ground[ground].color;
        assert!(far.colors.iter().zip(&far.light_data).any(|(color,tag)|*color==[c[0],c[1],c[2],255] && tag[1]==-2.),"explicit cover role {ground}");
    }
}
