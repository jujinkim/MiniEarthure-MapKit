use mapkit_core::*;
use mapkit_package::*;
use serde_json::json;
use std::collections::BTreeMap;

fn portal(mirrored: bool) -> Vec<u8> {
    scaled_portal(mirrored, 1.0)
}
fn scaled_portal(mirrored: bool, scale: f32) -> Vec<u8> {
    // Three separate quads, with a real opening. A material AABB would fill it.
    let mut points = Vec::<[f32;3]>::new();
    for (x,y,w,h) in [(0.,0.,1.,4.),(3.,0.,1.,4.),(1.,3.,2.,1.)] {
        let v = [[x,y,0.],[x+w,y,0.],[x+w,y+h,0.],[x,y+h,0.]];
        for i in [0,1,2,0,2,3] { points.push(v[i]); }
    }
    let bin: Vec<_> = points.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
    let mut doc = serde_json::to_vec(&json!({
        "asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"translation":[1,2,3],"children":[1]},
                 {"mesh":0,"translation":[0,1,0],"scale":[if mirrored {-2.0*scale} else {2.0*scale},3.0*scale,4.0*scale]}],
        "buffers":[{"byteLength":bin.len()}],"bufferViews":[{"buffer":0,"byteLength":bin.len()}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":18,"type":"VEC3","min":[0,0,0],"max":[4,4,0]}],
        "materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.25,0.5,0.75,1]}}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0}]}]
    })).unwrap();
    while doc.len()%4 != 0 {doc.push(b' ');}
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());out.extend(((28+doc.len()+bin.len()) as u32).to_le_bytes());
    out.extend((doc.len() as u32).to_le_bytes());out.extend(b"JSON");out.extend(doc);
    out.extend((bin.len() as u32).to_le_bytes());out.extend(b"BIN\0");out.extend(bin);out
}

#[test]
fn water_and_small_props_have_bounded_display_only_geometry() {
    let (mut d, mut files) = fixture();
    files.insert("near.glb".into(),scaled_portal(false,0.2));
    files.insert("far.glb".into(),scaled_portal(false,0.2));
    d.water_bodies.push(mapkit_core::water::WaterBody {
        id:"pond".into(),polygon:vec![[1000,1000],[4000,1000],[4000,4000],[1000,4000]],
        islands:vec![vec![[2000,2000],[2000,3000],[3000,3000],[3000,2000]]],
        surface_cm:100,bottom_cm:-200,flow_cm_s:[0,0],
    });
    let p=read_bytes(&pack_bytes(d,files).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let chunk=p.generate(cell,500_000).unwrap();
    let far=p.distant_from_generated(&chunk).unwrap();
    assert_eq!(far.decoration.len(),far.vertices.len());
    let first_small=far.decoration.iter().position(|v| *v==1).unwrap();
    assert!(far.decoration[first_small..].iter().all(|v| *v==1),"one contiguous small-prop tail");
    assert_eq!(far.decoration.len()-first_small,18);
    let water:Vec<_>=far.vertices.iter().zip(&far.colors).filter(|(_,c)| **c==[56,103,112,255]).collect();
    assert_eq!(water.len(),chunk.water_bodies[0].surface.len()*3);
    assert!(water.iter().all(|(v,_)| v[1]==1.0));
    assert!(chunk.triangles.iter().all(|t| t.object_id!="pond"));
    assert!(p.distant_triangle_bound(cell).unwrap()>=far.vertices.len() as u64/3);
}
fn fixture() -> (MapDocument,BTreeMap<String,Vec<u8>>) {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();d.roads.clear();d.zones.clear();d.buildings.clear();
    d.assets.push(Asset {id:"portal".into(),path:"near.glb".into(),distant_path:Some("far.glb".into()),
        attribution:Attribution{source:"synthetic portal".into(),license:"MIT".into(),notice:"".into()},
        collision:vec![CollisionBox{center:[50,200,0],size_cm:[100,400,50]}],convex_collision:vec![],material:None});
    d.placements.push(Placement{id:"gate".into(),asset_id:"portal".into(),position:[10000,100,10000],quarter_turns:1,yaw_offset_mdeg:30000});
    (d,BTreeMap::from([("near.glb".into(),portal(false)),("far.glb".into(),portal(false))]))
}

#[test]
fn authored_silhouette_preserves_opening_transforms_collision_and_bounds() {
    let (d,files) = fixture();
    let p=read_bytes(&pack_bytes(d.clone(),files.clone()).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let geometry=p.generate(cell,500_000).unwrap();
    let far=p.distant_from_generated(&geometry).unwrap();
    let vertices:Vec<_>=far.vertices.iter().zip(&far.colors).filter(|(_,c)| **c==[64,128,191,255]).map(|(v,_)| *v).collect();
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
    let mut changed=files;changed.insert("far.glb".into(),portal(true));
    let q=read_bytes(&pack_bytes(d,changed).unwrap()).unwrap();
    let cell=Cell{x:0,y:0};let mesh=q.generate_distant(cell).unwrap();
    assert!(q.distant_triangle_bound(cell).unwrap()>=mesh.vertices.len() as u64/3);
    assert_ne!(q.inspection.world_content_hash,read_bytes(&bytes).unwrap().inspection.world_content_hash);
    let cost=inspect_read_cost(&bytes).unwrap();
    assert!(read_bytes_with_budget(&bytes,cost.validation_peak_bytes).is_ok());
    assert_eq!(read_bytes_with_budget(&bytes,cost.validation_peak_bytes-1).err().unwrap().code,"E_MEMORY_BUDGET");
}
