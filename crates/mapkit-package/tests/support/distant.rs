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

/// Two by two 32 m heightmap cells (2 m samples) of rough terrain, a
/// block-aligned grassland paint in cell (0, 0) and one road in cell (1, 1):
/// coarse distant ground blocks next to full-detail tiles and cell borders.
#[allow(dead_code)]
pub fn heightmap_terrain() -> (MapDocument,BTreeMap<String,Vec<u8>>) {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.free_roam = true;
    d.bounds = Bounds { min: [0, 0], max: [6400, 6400] };
    d.cell_size_cm = 3200;
    let mut files = BTreeMap::new();
    for cell in d.cells() {
        let samples: Vec<u16> = (0..17 * 17).map(|i| {
            let (x, y) = (cell.x as i64 * 16 + i % 17, cell.y as i64 * 16 + i / 17);
            (2000 + x * 20 + (x * 37 + y * 53 + x * y * 11) % 41 * 9) as u16
        }).collect();
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, 17, 17);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::Sixteen);
            e.write_header().unwrap().write_image_data(&samples.iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<_>>()).unwrap();
        }
        let path = format!("terrain/{}-{}.png", cell.x, cell.y);
        files.insert(path.clone(), bytes);
        d.heightmaps.push(Heightmap { cell, path, spacing_cm: 200, offset_cm: 0, step_cm: 1, source_accuracy_cm: None });
    }
    d.surface_areas.push(SurfaceArea { id: "meadow".into(), polygon: vec![[800, 800], [2400, 800], [2400, 2400], [800, 2400]],
        surface: Surface::Grass, style_id: "ground/grassland".into() });
    d.nodes = serde_json::from_value(json!([{"id":"a","position":[3600,2200,6000],"level":0},{"id":"b","position":[6000,2200,6000],"level":0}])).unwrap();
    d.roads = serde_json::from_value(json!([{"id":"lane","from":"a","to":"b","points":[[3600,2200,6000],[6000,2200,6000]],
        "widths_cm":[400],"surfaces":["asphalt"],"kind":"ground","clearance_cm":null,"sidewalk_cm":0}])).unwrap();
    (d, files)
}
