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
fn all_pipe_variants_share_bores_and_keep_inside_bend_clearance() {
    let catalogue = track::catalogue();
    for entry in catalogue["entries"].as_array().unwrap().iter().filter(|e| e["id"].as_str().unwrap().starts_with("cylinder")) {
        let id=entry["id"].as_str().unwrap();
        assert_eq!(entry["widths_cm"], serde_json::json!([400,600]));
        let reference=piece(&instance("pipe",id,600)).unwrap();
        for width in [400,600] {
            let p=piece(&instance("pipe",id,width)).unwrap();
            assert_eq!(p.path.first().unwrap().position_cm,reference.path.first().unwrap().position_cm);
            assert!(length(&p) <= length(&reference) + 3.0);
            if id.contains("curve") || id.contains("uturn") {
                let radius = 400.max(width / 2 + 200) as f64;
                assert!(radius - width as f64 / 2.0 >= 200.0);
                let end = p.path.last().unwrap().position_cm;
                if id.contains("uturn") { assert_eq!(end[0].unsigned_abs(), (2.0 * radius) as u64); }
                else { assert_eq!(end[0].unsigned_abs(), (radius + 400.0) as u64); }
            }
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
    assert_eq!(track::supported_widths("straight"), &[400,600,800,1200]);
    assert_eq!(track::supported_widths("loop"), &[400]);
    assert_eq!(track::supported_widths("banked_chicane"), &[400,600]);
}

#[test]
fn small_pipe_portals_and_internal_connections_preserve_grade_and_section() {
    for width in [400,600] {
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
    assert_eq!((t.radius_cm,t.length_cm),(200,1600));
    let mut t=t.clone();t.radius_cm=100;assert!(t.valid());t.radius_cm=99;assert!(!t.valid());
    let mut l=templates["loop"].track.clone().unwrap();l.radius_cm=149;assert!(!l.valid());
    let p=piece(&instance("half","banked_chicane",400)).unwrap();
    let mut h=tube(&p);h.kind=TrackKind::SweptHalfPipe;h.radius_cm=99;assert!(!h.valid());
}

#[test]
fn undersize_authoring_reports_dimensions_without_conversion() {
    let old=instance("old","cylinder",100);
    let snapshot=serde_json::to_value(&old).unwrap();
    let failure=piece(&old).unwrap_err();
    assert_eq!(failure.code,"E_PIPE_DIMENSIONS");
    assert!(failure.message.contains("400 cm"));
    assert_eq!(serde_json::to_value(&old).unwrap(),snapshot);
    let templates: std::collections::BTreeMap<String,gimmick::Gimmick>=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    let mut g=templates["cylinder"].clone();g.track.as_mut().unwrap().radius_cm=99;
    assert!(g.track.as_ref().unwrap().pipe_dimension_error());
}

#[test]
fn catalogue_and_all_ribbons_obey_new_authoring_minimum() {
    let catalogue=track::catalogue();
    assert_eq!(catalogue["min_width_cm"],400);
    assert_eq!(catalogue["pipe_radii_cm"],serde_json::json!([200,300]));
    for entry in catalogue["entries"].as_array().unwrap() {
        let id=entry["id"].as_str().unwrap();
        assert_eq!(entry["min_port_width_cm"],400);
        assert!(entry["default_width_cm"].as_u64().unwrap()>=400);
        for width in entry["widths_cm"].as_array().unwrap() {
            let width=width.as_u64().unwrap() as u32;
            let p=piece(&instance("ribbon",id,width)).unwrap();
            assert!(p.path.iter().chain(&p.alternate_path).all(|s|s.lateral_cm>=200), "{id}/{width}: internal bottleneck");
        }
    }
    for (preset,width) in [("straight",250),("cylinder",200),("cylinder",300),("loop",200)] {
        let item=instance("narrow-source",preset,width);
        let failure=piece(&item).unwrap_err();
        assert!(failure.message.contains("narrow-source") && failure.message.contains("width_cm"));
    }
    // Region widths compile only as overlays on an existing map (§44.367).
    for width in [150,200,300] {
        assert!(piece(&instance("region","straight",width)).is_ok());
        let mut standalone=Source::empty();standalone.instances.push(instance("region","straight",width));
        assert_eq!(compile(&standalone).unwrap_err().code,"E_TRACK_SOURCE");
    }
}

#[test]
fn stored_special_dimensions_and_new_authoring_are_separate() {
    let templates: std::collections::BTreeMap<String,gimmick::Gimmick>=serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap();
    for radius in [100,125,150,200,300] {
        let mut t=templates["cylinder"].track.clone().unwrap();t.radius_cm=radius;
        assert!(t.valid(),"stored v1 radius {radius}");
        assert_eq!(t.validate_authoring("fixture").is_ok(),[200,300].contains(&radius));
    }
    let mut t=templates["loop"].track.clone().unwrap();t.width_cm=220;
    assert!(t.valid());assert!(t.validate_authoring("narrow-loop").unwrap_err().message.contains("narrow-loop"));
    t.width_cm=400;assert!(t.validate_authoring("loop").is_ok());
}

#[test]
fn ramp_footprint_needs_a_continuous_bypass_and_preserves_source_on_failure() {
    for width in [400,600,800] {
        for kind in ["ramp_low","ramp_standard","ramp_triple"] {
            let mut source=Source::empty();source.instances.push(instance("ramp-road","sprint_lane",width));
            source.attachments.push(Attachment {kind:kind.into(),piece:"ramp-road".into(),path:"main".into(),station_cm:800,side:1});
            let before=source.clone();let result=compile(&source);
            assert_eq!(source,before);
            if width==400 {
                let err=result.unwrap_err();assert_eq!(err.code,"E_TRACK_CLEARANCE");
                assert!(err.message.contains("ramp-road") && err.message.contains("220 cm"));
            } else {
                let a=result.unwrap();let o=&a.obstacles[0];
                assert_eq!(o.half_width_cm,160);
                assert!(width as i64/2+o.lateral_cm-o.half_width_cm as i64>=220);
            }
        }
    }
}

fn shaped(preset: &str, width: u32, cp: Vec<[i64; 3]>) -> Instance {
    let mut i = instance("shaped", preset, width);
    i.control_points = cp;
    i
}
/// A lead, a right-hand quarter arc of radius r and a lead, as cubic points.
fn quarter(r: i64) -> Vec<[i64; 3]> {
    let k = (4.0 / 3.0 * (std::f64::consts::PI / 8.0).tan() * r as f64).round() as i64;
    vec![[0, 0, 0], [0, 0, 200], [0, 0, 400], [0, 0, 600], [0, 0, 600 + k], [r - k, 0, 600 + r], [r, 0, 600 + r],
        [r + 200, 0, 600 + r], [r + 400, 0, 600 + r], [r + 600, 0, 600 + r]]
}
#[test]
fn control_point_pipes_are_pure_bounded_and_valid_tubes() {
    for (preset, width) in [("cylinder", 400), ("cylinder", 600), ("banked_chicane", 400), ("banked_chicane", 600)] {
        let cp = quarter(1000);
        let p = piece(&shaped(preset, width, cp.clone())).unwrap();
        assert_eq!(p, piece(&shaped(preset, width, cp.clone())).unwrap(), "{preset}/{width}: pure");
        let mut moved = shaped(preset, width, cp.clone());
        moved.position_cm = [1234, 56, -789];
        let q = piece(&moved).unwrap();
        assert!(p.path.iter().zip(&q.path).all(|(a, b)| (0..3).all(|j| b.position_cm[j] - a.position_cm[j] == moved.position_cm[j]) && a.forward == b.forward));
        assert_eq!(p.path[0].position_cm, [0, 0, 0]);
        assert_eq!(p.path[0].forward, [0, 0, 1_000_000]);
        assert_eq!(p.path.last().unwrap().position_cm, *cp.last().unwrap());
        assert_eq!(p.path.last().unwrap().forward, [1_000_000, 0, 0]);
        assert!(p.path.len() <= 512 && p.path.windows(2).all(|w| (0..3).map(|j| ((w[1].position_cm[j] - w[0].position_cm[j]) as f64).powi(2)).sum::<f64>().sqrt() <= 51.0));
        let mode = if preset == "cylinder" { "cylinder" } else { "halfpipe" };
        assert!(p.path.iter().all(|s| s.mode == mode && s.tube_radius_cm == width / 2 && s.lateral_cm == width / 2 && !s.safe));
        let mut t = tube(&p);
        if preset == "banked_chicane" { (t.kind, t.width_cm) = (TrackKind::SweptHalfPipe, width); }
        assert!(t.valid(), "{preset}/{width}");
        assert_ne!(p.path, piece(&instance("plain", preset, width)).unwrap().path);
    }
    assert_eq!(track::supported_widths("banked_chicane"), &[400, 600]);
    let rules = &track::catalogue()["pipe_control_points"];
    assert_eq!(rules["presets"], serde_json::json!(["cylinder", "banked_chicane"]));
    assert_eq!(rules["min_radius_cm"]["600"], 500);
}
#[test]
fn pipe_control_points_reject_invalid_shapes() {
    let ok = quarter(1000);
    let mut cases: Vec<(&str, Vec<[i64; 3]>)> = vec![
        ("count", ok[..8].to_vec()),
        ("origin", ok.iter().map(|p| [p[0] + 10, p[1], p[2]]).collect()),
        ("heading", { let mut v = ok.clone(); v[1] = [50, 0, 200]; v }),
        ("bounds", { let mut v = ok.clone(); for p in &mut v[7..] { p[0] += 6000; } v }),
        ("radius", quarter(300)),
        ("grade", { let mut v = ok.clone(); for p in &mut v[2..] { p[1] += 300; } v }),
        ("level end", { let mut v = ok.clone(); v[9][1] = 40; v }),
    ];
    let mut long = vec![[0, 0, 0]];
    for n in 0..8 { long.extend([[0, 0, 300 * n + 100], [0, 0, 300 * n + 200], [0, 0, 300 * n + 300]]); }
    cases.push(("segments", long));
    for (label, cp) in cases {
        for preset in ["cylinder", "banked_chicane"] {
            let item = shaped(preset, 400, cp.clone());
            let before = item.clone();
            let e = piece(&item).err().unwrap_or_else(|| panic!("{label}/{preset} accepted"));
            assert_eq!(e.code, "E_PIPE_DIMENSIONS", "{label}/{preset}: {}", e.message);
            assert_eq!(item, before);
        }
    }
    assert!(piece(&shaped("cylinder", 600, quarter(500))).is_ok());
    assert_eq!(piece(&shaped("cylinder", 600, quarter(450))).unwrap_err().code, "E_PIPE_DIMENSIONS");
    assert_eq!(piece(&shaped("cylinder_curve", 400, ok.clone())).unwrap_err().code, "E_TRACK_SOURCE");
    assert_eq!(piece(&shaped("cylinder", 800, ok)).unwrap_err().code, "E_PIPE_DIMENSIONS");
}
