use mapkit_core::{assembled_track as track, special_track::{SpecialTrack, TrackKind, TubeFrame}, *};
use track::authoring::*;

fn length(p: &track::Piece) -> f64 {
    p.path.windows(2).map(|s| (0..3).map(|j| (s[1].position_cm[j]-s[0].position_cm[j]) as f64).map(|v| v*v).sum::<f64>().sqrt()).sum()
}
fn tube(p: &track::Piece) -> SpecialTrack {
    SpecialTrack { kind: TrackKind::SweptCylinder, radius_cm: p.width_cm/2, width_cm:400, length_cm:1600,
        centerline: p.path.iter().map(|s| TubeFrame { floor_cm:s.position_cm,normal:s.normal,forward:s.forward }).collect() }
}
#[test]
fn all_pipe_variants_keep_body_paths_and_share_small_bores() {
    let catalogue = track::catalogue();
    for entry in catalogue["entries"].as_array().unwrap().iter().filter(|e| e["id"].as_str().unwrap().starts_with("cylinder")) {
        let id=entry["id"].as_str().unwrap();
        assert_eq!(entry["widths_cm"], serde_json::json!([100,200,300,400,600]));
        let reference=piece(&instance("pipe",id,600)).unwrap();
        for width in [100,200,300,400,600] {
            let p=piece(&instance("pipe",id,width)).unwrap();
            assert_eq!(p.path.first().unwrap().position_cm,reference.path.first().unwrap().position_cm);
            assert_eq!(p.path.last().unwrap().position_cm,reference.path.last().unwrap().position_cm);
            assert!((length(&p)-length(&reference)).abs()<3.0,"{id}/{width}: {} vs {}",length(&p),length(&reference));
            assert!(p.path.iter().all(|s| s.tube_radius_cm==width/2 && s.lateral_cm==width/2 && !s.safe));
            let t=tube(&p); assert!(t.valid());
            let mesh=t.mesh();
            assert_eq!(mesh.inner.len(), t.tile_count()*2);
            for f in &t.centerline {
                let center=std::array::from_fn::<_,3,_>(|j| f.floor_cm[j] as f64+f.normal[j] as f64/1e6*width as f64/2.0);
                assert!(t.contains_swept(center,0.0));
                assert!(!mesh.tiles.iter().any(|(lo,hi)|(0..3).all(|j|center[j]>=lo[j] as f64 && center[j]<=hi[j] as f64)),"hollow occupancy {id}/{width}");
            }
            // The entry/exit annuli must never become caps over the bore.
            let first=&t.centerline[0];
            let center=[first.floor_cm[0] as f64,first.floor_cm[1] as f64+width as f64/2.0,first.floor_cm[2] as f64];
            for triangle in mesh.shell.iter().filter(|f|f.iter().all(|v|v[2]==first.floor_cm[2]*100)) {
                for v in triangle { assert!(((v[0] as f64/100.0-center[0]).powi(2)+(v[1] as f64/100.0-center[1]).powi(2)).sqrt()>=width as f64/2.0-0.02); }
            }
        }
    }
    assert_eq!(track::supported_widths("straight"), &[200,400,600,800,1200]);
    for id in ["loop","banked_chicane"] { assert_eq!(track::supported_widths(id), &[400]); }
}

#[test]
fn small_pipe_portals_and_internal_connections_preserve_grade_and_section() {
    for width in [100,200,300,400,600] {
        let mut previous=instance("road","straight",400);
        for (id,preset) in [("entry","tube_entry"),("pipe-a","cylinder_curve"),("pipe-b","cylinder_curve"),("exit","tube_exit"),("road-end","straight")] {
            let mut next=instance(id,preset,if preset=="straight" {400} else {width});
            if preset=="tube_exit" { next.exit_width_cm=400; }
            next=snap(&next,&previous).unwrap();
            let a=piece(&previous).unwrap(); let b=piece(&next).unwrap();
            let end=a.path.last().unwrap(); let start=&b.path[0];
            let portal=previous.preset=="tube_entry" || preset=="tube_exit";
            let drop=if portal {(width as f64/3.0).round() as i64} else {0};
            assert_eq!(end.position_cm[1]-start.position_cm[1],drop);
            assert_eq!(end.lateral_cm,start.lateral_cm);
            assert_eq!(end.forward,start.forward); assert_eq!(end.normal,start.normal);
            if preset.starts_with("tube_") {
                for s in b.path.windows(2) {
                    let horizontal=((s[1].position_cm[0]-s[0].position_cm[0]) as f64).hypot((s[1].position_cm[2]-s[0].position_cm[2]) as f64);
                    assert!((s[1].position_cm[1]-s[0].position_cm[1]).abs() as f64<=horizontal*0.12+1.0,"12% grade plus centimetre quantization");
                }
            }
            previous=next;
        }
    }
    let mut invalid=instance("pipe","cylinder",100);invalid.entry_width_cm=99;
    assert!(piece(&invalid).is_err());
    let mut road=instance("road","straight",400);road.entry_width_cm=100;
    assert!(piece(&road).is_err(),"road input range remains unchanged");
    let mut huge=Source::empty();huge.instances=vec![instance("p","cylinder",100);513];
    assert_eq!(compile(&huge).unwrap_err().code,"E_TRACK_BUDGET");
}

#[test]
fn standalone_radius_limits_do_not_relax_loop_or_halfpipe() {
    let templates: std::collections::BTreeMap<String,gimmick::Gimmick>=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    let t=templates["cylinder"].track.as_ref().unwrap();
    assert_eq!((t.radius_cm,t.length_cm),(125,1600));
    let mut t=t.clone();t.radius_cm=50;assert!(t.valid());t.radius_cm=49;assert!(!t.valid());
    let mut l=templates["loop"].track.clone().unwrap();l.radius_cm=149;assert!(!l.valid());
    let p=piece(&instance("half","banked_chicane",400)).unwrap();
    let mut h=tube(&p);h.kind=TrackKind::SweptHalfPipe;h.radius_cm=99;assert!(!h.valid());
}
