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

fn cover(ground: &str, environment: Option<serde_json::Value>) -> distant::DistantMesh {
    let mut d=document();d.buildings.clear();d.nodes.clear();d.roads.clear();d.zones.clear();d.free_roam=true;
    d.ground_style_id=format!("ground/{ground}");
    d.environment=environment.map(|e|serde_json::from_value(e).unwrap());
    let p=read_bytes(&pack_bytes(d,BTreeMap::new()).unwrap()).unwrap();
    p.generate_distant(Cell{x:0,y:0}).unwrap()
}
fn environment(concept: &str, climate: &str, terrain: &str) -> Option<serde_json::Value> {
    Some(serde_json::json!({"version":1,"concept":concept,"climate":climate,"terrain":terrain,"latitude_mdeg":36000,"longitude_mdeg":-112000,
        "utc_offset_minutes":-420,"sunrise_minutes":360,"sunset_minutes":1080,"regions":[],"lights":[]}))
}
#[test]
fn explicit_cover_keeps_its_palette_at_local_zero_near_and_far() {
    // Classified cover owns its colour (role -2); vegetated cover keeps it too
    // but carries role -3, where a map terrain may bind steep-face bare rock.
    for ground in ["snow","ice","rock","sand","managed_lawn","grassland","scrub","forest_floor"] {
        let far=cover(ground,None);
        let c=appearance::catalog().ground[ground].color;
        let role=if appearance::vegetated_ground(ground) {-3.} else {-2.};
        assert!(far.colors.iter().zip(&far.light_data).any(|(color,tag)|*color==[c[0],c[1],c[2],255] && tag[1]==role),"explicit cover role {ground}");
    }
}
#[test]
fn arid_and_canyon_palettes_recolour_natural_ground_only_where_authored() {
    let c=appearance::catalog();
    let rgb=|v:[u8;3]|[v[0],v[1],v[2],255];
    // Grand Canyon: arid climate, canyon terrain. Cover reads tan, bare rock red-buff.
    let canyon=environment("desert","arid","canyon");
    for (ground,expected) in [("grassland",c.climate_ground["arid"]["grassland"]),("scrub",c.climate_ground["arid"]["scrub"]),
        ("forest_floor",c.climate_ground["arid"]["forest_floor"]),("rock",c.terrain_ground["canyon"]["rock"])] {
        let far=cover(ground,canyon.clone());
        assert!(far.colors.iter().all(|v|*v==rgb(expected)),"canyon {ground}");
        assert_eq!(appearance::ground_color_for(&format!("ground/{ground}"),"arid","canyon"),Some(rgb(expected)));
        assert_ne!(expected,c.ground[ground].color,"{ground} palette differs from the temperate catalogue");
    }
    // An empty climate resolves through the concept default, as near.
    assert!(cover("grassland",environment("desert","","")).colors.iter().all(|v|*v==rgb(c.climate_ground["arid"]["grassland"])));
    // Other climates and terrains keep the catalogue colour; unlisted kinds keep it under arid too.
    for (concept,climate,terrain) in [("metropolis","temperate","plain"),("countryside","","mountain"),("polar","polar","coast"),("jungle","","river")] {
        for ground in ["grassland","rock","forest_floor"] {
            assert!(cover(ground,environment(concept,climate,terrain)).colors.iter().all(|v|*v==rgb(c.ground[ground].color)),"{concept} {climate} {terrain} {ground}");
        }
    }
    assert!(cover("asphalt",canyon).colors.iter().all(|v|*v==rgb(c.ground["asphalt"].color)));
    assert_eq!(appearance::ground_color_for("road/street/scrub","arid","plain"),Some(rgb(c.climate_ground["arid"]["scrub"])));
    assert_eq!(appearance::ground_color_for("building/modern/hotel/concrete","arid","canyon"),appearance::color("building/modern/hotel/concrete",false));
}
#[test]
fn korean_and_japanese_houses_take_their_use_roof_without_shop_modules() {
    for (house,shop) in [("building/korean_modern/detached/plaster","building/korean_modern/shop/concrete"),("building/japanese_modern/detached/plaster","building/japanese_modern/shop/plaster")] {
        // A one-storey envelope: the tile roof sits above a full storey inside the source height.
        let mut d=document();let b=&mut d.buildings[0];b.height_cm=500;b.style_id=house.into();
        d.normalize();d.validate().unwrap();
        let mesh=display::generate(&d,Cell{x:0,y:0}).unwrap();
        let roof:Vec<f64>=mesh.faces.iter().filter(|f|f.key=="roof/dark_tile").flat_map(|f|f.vertices).map(|p|p[1]).collect();
        assert!(!roof.is_empty(),"{house}");
        let (low,high)=roof.iter().fold((f64::MAX,f64::MIN),|(l,h),y|(l.min(*y),h.max(*y)));
        assert!((high-600.).abs()<1e-6 && (low-400.).abs()<1e-6,"{house} eaves {low} ridge {high}");
        // Shop canopies, balconies, rails and parapets of the architecture default are metal/stone/concrete; the house has none.
        assert!(mesh.faces.iter().all(|f|!matches!(f.key.as_str(),"material/metal"|"material/stone"|"material/concrete")),"{house}");
        let mut d=document();d.buildings[0].style_id=shop.into();
        d.normalize();d.validate().unwrap();
        assert!(display::generate(&d,Cell{x:0,y:0}).unwrap().faces.iter().any(|f|f.key=="material/metal"),"{shop} keeps the canopy");
    }
}
