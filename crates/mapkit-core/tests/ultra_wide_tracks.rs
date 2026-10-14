use mapkit_core::assembled_track::{self as track, authoring::*};

const PRESETS: [(&str, i64); 5] = [
    ("straight_ultra_wide", 0),
    ("gentle90_ultra_wide", 7200), ("gentle90_ultra_wide_left", -7200),
    ("right90_ultra_wide", 1100), ("right90_ultra_wide_left", -1100),
];

#[test]
fn ultra_wide_dimensions_ports_tapers_and_mirrors() {
    let catalogue = track::catalogue();
    for (id, radius) in PRESETS {
        let entry = catalogue["entries"].as_array().unwrap().iter().find(|e|e["id"]==id).unwrap();
        assert_eq!(entry["default_width_cm"],1800);
        assert_eq!(entry["max_port_width_cm"],1800);
        assert_eq!(track::supported_widths(id), &[1800]);
        let p = piece(&instance("road",id,1800)).unwrap();
        assert_eq!(p.path.last().unwrap().position_cm, if radius==0 {[0,0,800]} else {[radius,0,radius.abs()]});
        assert!(p.path.iter().all(|s|s.lateral_cm==900));
        for ports in [[400,1800],[1800,400],[600,1200],[1800,1800]] {
            for rotation in [[0,0,0],[17000,37000,11000]] {
                let mut road=instance("road",id,1800);
                road.entry_width_cm=ports[0]; road.exit_width_cm=ports[1]; road.rotation_mdeg=rotation;
                let mut source=Source::empty(); source.instances.push(road.clone());
                let a=compile(&source).unwrap();
                assert!(a.geometry_issues.is_empty(),"{id} {ports:?}: {:?}",a.geometry_issues);
                a.validate_stored().unwrap();
                let p=piece(&road).unwrap();
                assert_eq!(p.path[0].lateral_cm*2,ports[0]);
                assert_eq!(p.path.last().unwrap().lateral_cm*2,ports[1]);
                let next=snap(&instance("next","straight",800),&road).unwrap();
                assert_eq!(next.entry_width_cm,ports[1]);
                assert_eq!(p.path.last().unwrap().position_cm, piece(&next).unwrap().path[0].position_cm);
            }
        }
        for width in [400,1200,1799,1801] {assert!(piece(&instance("bad",id,width)).is_err());}
        let mut bad=instance("bad",id,1800); bad.exit_width_cm=1801;
        assert!(piece(&bad).is_err());
    }
    for pair in [[1,2],[3,4]] {
        let a=piece(&instance("r",PRESETS[pair[0]].0,1800)).unwrap();
        let b=piece(&instance("l",PRESETS[pair[1]].0,1800)).unwrap();
        assert_eq!(a.path.len(),b.path.len());
        for (a,b) in a.path.iter().zip(&b.path) {
            assert_eq!(a.position_cm,[-b.position_cm[0],b.position_cm[1],b.position_cm[2]]);
            assert_eq!(a.forward,[-b.forward[0],b.forward[1],b.forward[2]]);
        }
    }
    for id in ["straight","gentle90","tube_entry","tube_exit","free_curve","jump_panel"] {
        let mut road=instance("transition",id,800); road.entry_width_cm=1800;
        assert_eq!(piece(&road).unwrap().path[0].lateral_cm,900);
    }
}

#[test]
fn enlarged_loop_keeps_four_metre_ribbon_and_flat_connections() {
    let p=piece(&instance("loop","loop",400)).unwrap();
    assert_eq!(p.path.iter().map(|s|s.position_cm[1]).max(),Some(1050));
    assert!(p.path.iter().all(|s|s.lateral_cm==200));
    for end in [&p.path[0],p.path.last().unwrap()] {
        assert_eq!(end.position_cm[1],0);
        assert_eq!(end.normal,[0,1_000_000,0]);
        assert_eq!(end.forward,[0,0,1_000_000]);
    }
    let next=snap(&instance("exit","straight",800),&instance("loop","loop",400)).unwrap();
    assert_eq!(p.path.last().unwrap().position_cm,piece(&next).unwrap().path[0].position_cm);
}
