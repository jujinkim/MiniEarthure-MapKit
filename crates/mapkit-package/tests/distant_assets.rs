use mapkit_core::*;
use mapkit_package::*;
#[path = "support/distant.rs"]
mod support;
use support::{fixture, scaled_portal};

#[test]
fn water_and_small_props_have_bounded_display_only_geometry() {
    let (mut d, mut files) = fixture();
    files.insert("near.glb".into(),scaled_portal(false,0.2));
    files.insert("far.glb".into(),scaled_portal(false,0.2));
    d.assets[0].collision[0].center[1] = -1000; // buried soft root-bed
    d.water_bodies.push(mapkit_core::water::WaterBody {
        id:"pond".into(),polygon:vec![[1000,1000],[4000,1000],[4000,4000],[1000,4000]],
        islands:vec![vec![[2000,2000],[2000,3000],[3000,3000],[3000,2000]]],
        surface_cm:100,bottom_cm:-200,flow_cm_s:[45,-20],
    });
    let p=read_bytes(&pack_bytes(d,files).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let chunk=p.generate(cell,500_000).unwrap();
    let far=p.distant_from_generated(&chunk).unwrap();
    assert_same(&far, &p.generate_distant(cell).unwrap());
    assert_eq!(far.decoration.len(),far.vertices.len());
    let first_small=far.decoration.iter().position(|v| *v==1).unwrap();
    assert!(far.decoration[first_small..].iter().all(|v| *v==1),"one contiguous small-prop tail");
    assert_eq!(far.decoration.len()-first_small,18);
    let water:Vec<_>=far.vertices.iter().zip(&far.colors).filter(|(_,c)| **c==[56,103,112,255]).collect();
    assert_eq!(water.len(),chunk.water_bodies[0].surface.len()*3);
    assert!(water.iter().all(|(v,_)| v[1]==1.0));
    let water_data:Vec<_>=far.decoration.iter().zip(&far.light_data).filter(|(kind,_)| **kind==2).collect();
    assert_eq!(water_data.len(),water.len());
    assert!(water_data.iter().all(|(_,uv)| (uv[0]-0.45).abs()<0.0001 && (uv[1]-0.20).abs()<0.0001));
    assert!(chunk.triangles.iter().all(|t| t.object_id!="pond"));
    assert!(p.distant_triangle_bound(cell).unwrap()>=far.vertices.len() as u64/3);
}

#[test]
fn authored_silhouette_preserves_opening_transforms_collision_and_bounds() {
    let (d,files) = fixture();
    let p=read_bytes(&pack_bytes(d.clone(),files.clone()).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let geometry=p.generate(cell,500_000).unwrap();
    let far=p.distant_from_generated(&geometry).unwrap();
    assert_same(&far, &p.generate_distant(cell).unwrap());
    // GLTF's linear [.25,.5,.75] reaches the same sRGB colour properties as
    // the primary GLTF importer; terrain colours already use display sRGB.
    let vertices:Vec<_>=far.vertices.iter().zip(&far.colors).filter(|(_,c)| **c==[137,188,225,255]).map(|(v,_)| *v).collect();
    assert_eq!(vertices.len(),18,"authored faces, not filled material boxes");
    let angle=120_f32.to_radians();let (s,c)=angle.sin_cos();
    // Recover GLB local points: parent translation and child translation/scale
    // must survive placement quarter-turn + continuous yaw exactly once.
    for v in &vertices {
        let x=v[0]-100.;let z=v[2]+100.;
        let local_x=(x*c-z*s-1.)/2.;let local_y=(v[1]-4.)/3.;
        assert!(local_x<=1.001 || local_x>=2.999 || local_y>=2.999);
        assert!((x*s+z*c-3.).abs()<0.0001);
    }
    assert!(p.distant_triangle_bound(cell).unwrap() >= far.vertices.len() as u64/3);
    assert!(p.distant_workspace_bytes(cell).unwrap() > files["far.glb"].len() as u64);
    let mut tinted=d.clone();
    tinted.assets[0].material=Some(AssetMaterial{albedo_rgba:[64,128,192,255],metallic_per_mille:0,
        roughness_per_mille:900,double_sided:false,albedo_texture:None});
    let override_package=read_bytes(&pack_bytes(tinted,files.clone()).unwrap()).unwrap();
    let override_mesh=override_package.generate_distant(cell).unwrap();
    assert_eq!(override_mesh.colors.iter().filter(|c| **c==[64,128,192,255]).count(),18,
        "sRGB overrides must not be transformed a second time");
    let mut without=d;without.assets[0].distant_path=None;
    let mut near_files=files;near_files.remove("far.glb");
    let other=read_bytes(&pack_bytes(without,near_files).unwrap()).unwrap();
    let unchanged=other.generate(cell,500_000).unwrap();
    assert_eq!(geometry.triangles,unchanged.triangles);
    assert_eq!(geometry.objects,unchanged.objects);
    assert_eq!(geometry.asset_convexes,unchanged.asset_convexes);
    assert_ne!(p.inspection.world_content_hash,other.inspection.world_content_hash);
}

#[test]
fn distant_references_hashes_cost_and_validation_are_complete() {
    let (d,files)=fixture();let bytes=pack_bytes(d.clone(),files.clone()).unwrap();
    let p=read_bytes(&bytes).unwrap();
    assert_eq!(p.inspection.user_asset_bytes,files.values().map(|v| v.len() as u64).sum::<u64>());
    assert_eq!(bytes,pack_bytes(p.document,p.files).unwrap());
    for path in ["../far.glb","far.png","course-validation/far.glb"] {
        let mut bad=d.clone();bad.assets[0].distant_path=Some(path.into());
        assert!(pack_bytes(bad,files.clone()).is_err());
    }
    let mut missing=files.clone();missing.remove("far.glb");
    assert!(pack_bytes(d.clone(),missing).is_err());
    let mut bad=files.clone();bad.insert("far.glb".into(),b"not a glb".to_vec());
    assert_eq!(pack_bytes(d.clone(),bad).unwrap_err().code,"E_ASSET");
    let mut changed=files;changed.insert("far.glb".into(),scaled_portal(true, 1.0));
    let q=read_bytes(&pack_bytes(d,changed).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let mesh=q.generate_distant(cell).unwrap();
    assert_same(&mesh, &q.distant_from_generated(&q.generate(cell,500_000).unwrap()).unwrap());
    assert!(q.distant_triangle_bound(cell).unwrap()>=mesh.vertices.len() as u64/3);
    assert_ne!(q.inspection.world_content_hash,read_bytes(&bytes).unwrap().inspection.world_content_hash);
    let cost=inspect_read_cost(&bytes).unwrap();
    assert!(read_bytes_with_budget(&bytes,cost.validation_peak_bytes).is_ok());
    assert_eq!(read_bytes_with_budget(&bytes,cost.validation_peak_bytes-1).err().unwrap().code,"E_MEMORY_BUDGET");
}

#[test]
fn small_collision_structures_never_enter_the_decoration_tail() {
    let (mut d, mut files) = fixture();
    files.insert("near.glb".into(),scaled_portal(false,0.2));
    files.insert("far.glb".into(),scaled_portal(false,0.2));
    let p=read_bytes(&pack_bytes(d.clone(),files.clone()).unwrap()).unwrap();
    let mesh=p.generate_distant(Cell{x:0,y:0}).unwrap();
    assert!(mesh.decoration.iter().all(|&kind| kind == 0));
    assert!(d.assets[0].has_visible_collision());
    d.assets[0].collision[0].center[1] = -1000;
    assert!(!d.assets[0].has_visible_collision());
    d.assets[0].convex_collision.push(CollisionConvex {
        vertices:vec![[0,-10,0],[10,-10,0],[0,0,10]], faces:vec![[0,1,2]],
    });
    assert!(d.assets[0].has_visible_collision());
}

fn assert_same(a: &distant::DistantMesh, b: &distant::DistantMesh) {
    assert_eq!(a.vertices,b.vertices); assert_eq!(a.colors,b.colors);
    assert_eq!(a.light_data,b.light_data); assert_eq!(a.decoration,b.decoration);
}

#[test]
fn reused_small_silhouettes_keep_light_roles_seeds_and_cancel_scope() {
    let (mut d,mut files)=fixture();
    files.insert("far.glb".into(),scaled_portal(true,0.2));
    d.assets[0].collision[0].center[1]=-1000;
    let mut other=d.placements[0].clone();other.id="other".into();other.position[0]+=2000;
    d.placements.push(other);
    d.environment=Some(serde_json::from_value(serde_json::json!({
        "version":1,"concept":"metropolis","latitude_mdeg":0,"longitude_mdeg":0,
        "utc_offset_minutes":0,"sunrise_minutes":360,"sunset_minutes":1080,"regions":[],
        "lights":[{"asset_id":"portal","window_materials":[0],"bulb_materials":[],
        "position_cm":[0,0,0],"range_cm":1000,"color":[255,200,100]}]
    })).unwrap());
    let p=read_bytes(&pack_bytes(d,files).unwrap()).unwrap();let cell=Cell{x:0,y:0};
    let chunk=p.generate(cell,500_000).unwrap();let far=p.generate_distant(cell).unwrap();
    assert_same(&far,&p.distant_from_generated(&chunk).unwrap());
    let small:Vec<_>=far.light_data.iter().zip(&far.decoration).filter(|(_,kind)|**kind==1).map(|(v,_)|v).collect();
    assert_eq!(small.len(),36);assert!(small.iter().all(|v|v[1]==1.));
    assert_ne!(small[0][0],small[18][0]);
    let token=cancellation::CancellationToken::default();
    assert_eq!(token.run(|| {token.cancel();p.generate_distant(cell)}).unwrap_err().code,"E_CANCELLED");
    assert_same(&far,&cancellation::CancellationToken::default().run(||p.generate_distant(cell)).unwrap());
}
