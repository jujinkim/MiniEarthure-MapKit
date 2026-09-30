use mapkit_core::{assembled_track::{self as track,authoring::*},grind::*};
use mapkit_package::*;
use std::collections::BTreeMap;
#[test] fn current_v1_source_save_reopen_regenerate() {
    let mut source=shortcut_source();
    source.grind_lines.push(GrindLine{id:"air".into(),control_points:vec![[0,150,0],[0,150,1200]],up:[0,1_000_000,0],capture_width_cm:30,start_connections:vec![],end_connections:vec![]});
    let d=track::document_from_assembly(compile(&source).unwrap()).unwrap();
    let bytes=pack_bytes(d.clone(),BTreeMap::new()).unwrap();
    let restored=read_bytes(&bytes).unwrap();
    assert_eq!(restored.document.grind_lines,d.grind_lines);
    assert_eq!(pack_bytes(restored.document,BTreeMap::new()).unwrap(),bytes);
    assert_eq!(track::document_from_assembly(compile(&source).unwrap()).unwrap(),d);
}
