use mapkit_core::*;
use std::collections::BTreeMap;
fn templates()->BTreeMap<String,gimmick::Gimmick> {serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap()}
#[test]
fn bounded_templates_effects_meshes_and_spawn_exclusion() {
    let all=templates();
    for g in all.values(){assert!(g.valid(),"{}",g.id);}
    for id in ["loop","cylinder"] {
        let g=&all[id];let t=g.track.as_ref().unwrap();let mesh=t.mesh();
        assert_eq!(mesh.inner.len(),t.tile_count()*2);
        assert_eq!(mesh.tiles.len() as u64,g.occupied_count());
        assert!(g.memory_bytes()>mesh.inner.len() as u64*200);
        assert!(g.parts.is_empty());
        assert!(g.excludes_spawn(g.position));
        assert!(!g.excludes_spawn([g.position[0]+10000,g.position[1],g.position[2]]));
        for f in mesh.inner.iter().chain(&mesh.shell) {
            let u=std::array::from_fn::<_,3,_>(|a|f[1][a]-f[0][a]);
            let v=std::array::from_fn::<_,3,_>(|a|f[2][a]-f[0][a]);
            assert_ne!([u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]],[0;3],"nondegenerate {id}");
        }
        let resolved=g.resolved_json();assert!(resolved["track_mesh"]["inner"].is_array());
        let mut invalid=g.clone();invalid.track.as_mut().unwrap().radius_cm=u32::MAX;assert!(!invalid.valid());
        invalid=g.clone();invalid.motion.kind=gimmick::MotionKind::Rotate;assert!(!invalid.valid());
        invalid=g.clone();invalid.parts=all["boost"].parts.clone();assert!(!invalid.valid());
    }
    let mut g=all["target_speed"].clone();g.effect.as_mut().unwrap().strength_percent=0;assert!(!g.valid());
    g=all["jump_height"].clone();g.effect.as_mut().unwrap().jump_height_cm=0;assert!(!g.valid());
    g=all["boost"].clone();g.parts=vec![g.parts[0].clone();33];assert!(!g.valid(),"ordinary convex cap unchanged");
}
#[test]
fn special_track_cost_archive_order_and_hollow_occupancy() {
    let mut d:MapDocument=serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    d.bounds=Bounds{min:[0,0],max:[25600,25600]};d.cell_size_cm=3200;
    d.gimmicks=templates().into_values().filter(|g|g.track.is_some()||g.effect.is_some()).collect();
    d.normalize();gimmick::validate(&d).unwrap();
    let cell=Cell{x:2,y:2};
    let cost=estimate_generation(&d,cell,500_000).unwrap();
    assert!(cost.occupied_solids>=2304);
    let generated=generate(GenerationInput{document:&d,cell,heightgrid:None,max_triangles:500_000}).unwrap();
    let key=archive_key("tracks",cell);
    let archive=encode_archive(&generated,&key,archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&archive,&key,cell,&cost).unwrap(),generated);
    let mut small=cost.clone();small.gimmick_bytes=0;assert!(decode_archive(&archive,&key,cell,&small).is_err());
    let hash=generated.hash().unwrap();d.gimmicks.reverse();d.normalize();
    assert_eq!(generate(GenerationInput{document:&d,cell,heightgrid:None,max_triangles:500_000}).unwrap().hash().unwrap(),hash);
    let tube=d.gimmicks.iter().find(|g|g.id=="cylinder").unwrap();
    let center=[tube.position[0],tube.position[1]+250,tube.position[2]];
    assert!(!tube.occupancy_bounds().iter().any(|(lo,hi)|(0..3).all(|a|center[a]>=lo[a]&&center[a]<=hi[a])),"hollow center");
}

#[test]
fn quantized_surface_direction_changes_stay_below_five_degrees() {
    fn normal(f:&[Vertex;3])->[f64;3] {
        let u=std::array::from_fn::<_,3,_>(|a|(f[1][a]-f[0][a]) as f64);
        let v=std::array::from_fn::<_,3,_>(|a|(f[2][a]-f[0][a]) as f64);
        let n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        let length=libm::sqrt(n.iter().map(|v|v*v).sum());n.map(|v|v/length)
    }
    for id in ["loop","cylinder"] { for radius in [150,250,600] {for length in [600,1600,3200] {
        let mut t=templates()[id].track.clone().unwrap();t.radius_cm=radius;t.length_cm=length;
        let mesh=t.mesh();let bands=t.tile_count()/256;
        assert_eq!(mesh.units_per_metre,10000);
        let ns:Vec<_>=mesh.inner.iter().map(normal).collect();
        let mut largest=0.0f64;
        for i in 0..256 {for j in 0..bands {for k in 0..2 {
            let index=(i*bands+j)*2+k;
            for other in [index^1,((i+1).min(255)*bands+j)*2+k,(i*bands+(j+1).min(bands-1))*2+k] {
                let dot=(0..3).map(|a|ns[index][a]*ns[other][a]).sum::<f64>().clamp(-1.0,1.0);
                largest=largest.max(libm::acos(dot).to_degrees());
            }
        }}}
        assert!(largest<=5.0,"{id} r={radius} l={length}: {largest}");
        // Analytic maximum sag plus integer quantization, in centimetres.
        let angular=1.6*radius as f64*(1.0-libm::cos(std::f64::consts::PI/256.0));
        let axial=if id=="cylinder" {0.06*radius as f64*(1.0-libm::cos(std::f64::consts::PI/bands as f64))} else {0.0};
        assert!(angular+axial+0.009<1.0,"one centimetre approximation bound");
    }}}
}
