use mapkit_core::*;
use mapkit_package::{indexed::*, *};
use std::{collections::BTreeMap, io::Cursor};

#[test]
fn water_and_authored_lighting_bind_both_containers_and_decoder_allowances() {
    let mut d: MapDocument=serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.water_bodies.push(water::WaterBody {id:"lake".into(),polygon:vec![[1000,1000],[7000,1000],[7000,7000],[1000,7000]],islands:vec![vec![[2000,2000],[3000,2000],[3000,3000],[2000,3000]]],surface_cm:100,bottom_cm:-300,flow_cm_s:[25,0]});
    d.environment=Some(serde_json::from_value(serde_json::json!({"version":1,"concept":"metropolis","start_minutes":1200,"ground_color":[170,76,42],"latitude_mdeg":37000,"longitude_mdeg":127000,"utc_offset_minutes":540,"sunrise_minutes":360,"sunset_minutes":1080,"regions":[],"lights":[]})).unwrap());
    let templates:serde_json::Value=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    d.gimmicks.push(serde_json::from_value(templates["ramp"].clone()).unwrap());
    let files=BTreeMap::new();
    let original=read_bytes(&pack_bytes(d.clone(),files.clone()).unwrap()).unwrap();
    let index=pack_source(d.clone(),files.clone(),1).unwrap();
    let mut reader=IndexedReader::open(Cursor::new(index),1024*1024*1024,None).unwrap();
    reader.audit(1024*1024*1024,&ReadEpoch::default().begin()).unwrap();
    assert_eq!(reader.index().world_content_hash,original.manifest.world_content_hash);
    d.water_bodies[0].surface_cm+=1;
    let changed=read_bytes(&pack_bytes(d,files).unwrap()).unwrap();
    assert_ne!(changed.manifest.world_content_hash,original.manifest.world_content_hash);
}
