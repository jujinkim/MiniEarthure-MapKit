use mapkit_core::{assembled_track::{self as track, authoring::*}, gimmick::{Gimmick, MotionKind}};
use std::collections::BTreeMap;

fn square(g: &Gimmick) {
    assert_eq!(g.effect.as_ref().unwrap().ring_radius_cm, 150);
    assert_eq!(g.effect.as_ref().unwrap().strength_percent, 100);
    assert_eq!(g.motion.cooldown_ms, 1500);
    assert_eq!(g.parts.len(), 4);
    for p in &g.parts {
        let lo: [i64;3] = std::array::from_fn(|a| p.vertices.iter().map(|v|v[a]).min().unwrap());
        let hi: [i64;3] = std::array::from_fn(|a| p.vertices.iter().map(|v|v[a]).max().unwrap());
        assert_eq!((lo[2],hi[2]),(-15,15));
        assert!((0..2).any(|a| (lo[a]==150 && hi[a]==200) || (lo[a]==-200 && hi[a]==-150)));
    }
}

#[test]
fn generated_and_manual_ring_defaults_keep_pose_and_rim() {
    let mut s=Source::empty();s.instances.push(instance("road","air_ring",400));
    let d=track::document_from_assembly(compile(&s).unwrap()).unwrap();
    let g=d.gimmicks.iter().find(|g|g.motion.kind==MotionKind::AirRing).unwrap();
    square(g);assert_eq!(g.position,[0,200,1600]);
    s.instances[0]=instance("road","straight",400);
    s.actions.push(Action{id:"ring".into(),kind:"air_ring".into(),piece:"road".into(),sample:4,height_cm:370,panel_width_percent:50,panel_alignment:PanelAlignment::Center,landing:None});
    let a=compile(&s).unwrap();let mut expected=a.pieces[0].path[4].position_cm;expected[1]+=370;
    let d=track::document_from_assembly(a).unwrap();
    let g=d.gimmicks.iter().find(|g|g.id=="action-ring").unwrap();square(g);assert_eq!(g.position,expected);
    assert_eq!(s,serde_json::from_str::<Source>(&serde_json::to_string(&s).unwrap()).unwrap());
    assert_eq!(d,track::document_from_assembly(compile(&s).unwrap()).unwrap());
}

#[test]
fn standalone_ring_has_three_metre_aperture_and_original_twenty_cm_rim() {
    let all:BTreeMap<String,Gimmick>=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    let g=&all["air_ring"];assert!(g.valid());assert_eq!(g.parts.len(),24);
    assert_eq!(g.effect.as_ref().unwrap().ring_radius_cm,150);
    assert_eq!(g.effect.as_ref().unwrap().strength_percent,100);
    assert_eq!(g.position,[6400,400,6400]);assert_eq!(g.motion.cooldown_ms,1500);
    for p in &g.parts {for (i,v) in p.vertices.iter().enumerate() {
        let r=((v[0]*v[0]+v[1]*v[1]) as f64).sqrt();
        assert!((r-if [0,3].contains(&(i%4)){150.0}else{170.0}).abs()<0.71);
        assert_eq!(v[2].abs(),10);
    }}
    let mut explicit=g.clone();explicit.effect.as_mut().unwrap().strength_percent=37;
    explicit.effect.as_mut().unwrap().ring_radius_cm=300;
    for p in &mut explicit.parts {for v in &mut p.vertices {v[0]*=2;v[1]*=2;}}
    assert!(explicit.valid());
    assert_eq!(explicit,serde_json::from_str::<Gimmick>(&serde_json::to_string(&explicit).unwrap()).unwrap());
}
