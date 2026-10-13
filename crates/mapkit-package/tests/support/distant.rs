use mapkit_core::*;
use serde_json::json;
use std::collections::BTreeMap;

fn portal(mirrored: bool) -> Vec<u8> {
    scaled_portal(mirrored, 1.0)
}
pub fn scaled_portal(mirrored: bool, scale: f32) -> Vec<u8> {
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

pub fn fixture() -> (MapDocument,BTreeMap<String,Vec<u8>>) {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();d.roads.clear();d.zones.clear();d.buildings.clear();
    d.assets.push(Asset {id:"portal".into(),path:"near.glb".into(),distant_path:Some("far.glb".into()),
        attribution:Attribution{source:"synthetic portal".into(),license:"MIT".into(),notice:"".into()},
        collision:vec![CollisionBox{center:[50,200,0],size_cm:[100,400,50]}],convex_collision:vec![],material:None});
    d.placements.push(Placement{id:"gate".into(),asset_id:"portal".into(),position:[10000,100,10000],quarter_turns:1,yaw_offset_mdeg:30000});
    (d,BTreeMap::from([("near.glb".into(),portal(false)),("far.glb".into(),portal(false))]))
}
