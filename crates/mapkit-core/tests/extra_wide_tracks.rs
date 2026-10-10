use mapkit_core::assembled_track::{self as track, authoring::*};

const PRESETS: [(&str, &str, i64); 5] = [
    ("straight_extra_wide", "straight", 0),
    ("gentle90_extra_wide", "gentle90", 4800),
    ("gentle90_extra_wide_left", "gentle90_left", -4800),
    ("right90_extra_wide", "right90", 800),
    ("right90_extra_wide_left", "right90_left", -800),
];

#[test]
fn extra_wide_catalogue_dimensions_and_fixed_body_contract() {
    let catalogue = track::catalogue();
    for (preset, _, radius) in PRESETS {
        assert_eq!(track::category(preset), "driving");
        assert_eq!(track::supported_widths(preset), &[1200]);
        let entry = catalogue["entries"].as_array().unwrap().iter().find(|e| e["id"] == preset).unwrap();
        assert_eq!(entry["default_width_cm"], 1200);
        let p: track::Piece = serde_json::from_value(catalogue["pieces"].as_array().unwrap().iter().find(|p| p["id"] == preset).unwrap().clone()).unwrap();
        assert_eq!((p.width_cm, p.entry_width_cm, p.exit_width_cm), (1200, 1200, 1200));
        assert!(p.path.iter().all(|s| s.lateral_cm == 600 && s.normal == [0, 1_000_000, 0]));
        assert_eq!(p.path.last().unwrap().position_cm, if radius == 0 { [0, 0, 800] } else { [radius, 0, radius.abs()] });
        assert_eq!(p.path.last().unwrap().forward, if radius == 0 { [0, 0, 1_000_000] } else { [radius.signum()*1_000_000, 0, 0] });
        for width in [200, 400, 600, 800, 1199, 1201] {
            assert_eq!(piece(&instance("a", preset, width)).unwrap_err().code, "E_TRACK_SOURCE");
        }
        for port in [199, 1201] {
            for entry in [false, true] {
                let mut road = instance("a", preset, 1200);
                if entry { road.entry_width_cm = port; } else { road.exit_width_cm = port; }
                assert!(piece(&road).is_err());
            }
        }
    }
}

#[test]
fn extra_wide_preserves_base_geometry_rotations_tapers_and_terrain_policy() {
    for (preset, base, _) in PRESETS {
        for rotation in [[0, 0, 0], [0, 37000, 0], [27000, 17000, 11000]] {
            for [entry, exit] in [[1200, 1200], [200, 1200], [1200, 200], [400, 600]] {
                let mut road = instance("a", preset, 1200);
                road.rotation_mdeg = rotation;
                road.position_cm = [137, 211, 389];
                road.entry_width_cm = entry;
                road.exit_width_cm = exit;
                let mut p = piece(&road).unwrap();
                road.preset = base.into();
                p.id = base.into();
                assert_eq!(p, piece(&road).unwrap(), "{preset} {rotation:?} {entry}/{exit}");
                let mut source = Source::empty();
                source.instances.push(road);
                let original = compile(&source).unwrap();
                source.instances[0].preset = preset.into();
                let wide = compile(&source).unwrap();
                assert_eq!(wide.terrain_policy(0), original.terrain_policy(0));
                assert!(wide.geometry_issues.is_empty(), "{preset}: {:?}", wide.geometry_issues);
            }
        }
    }
    for (right, left) in [(PRESETS[1].0, PRESETS[2].0), (PRESETS[3].0, PRESETS[4].0)] {
        let a = piece(&instance("a", right, 1200)).unwrap();
        let b = piece(&instance("b", left, 1200)).unwrap();
        assert_eq!(a.path.len(), b.path.len());
        for (a, b) in a.path.iter().zip(&b.path) {
            assert_eq!(a.position_cm, [-b.position_cm[0], b.position_cm[1], b.position_cm[2]]);
            assert_eq!(a.forward, [-b.forward[0], b.forward[1], b.forward[2]]);
        }
    }
}

#[test]
fn extra_wide_is_excluded_without_driving_category() {
    for circuit in [false, true] {
        let settings = track::Settings { seed: 42, circuit, categories: vec!["gimmick".into()], ..Default::default() };
        let a = track::assemble(&settings).unwrap();
        assert!(a.pieces.iter().all(|p| !PRESETS.iter().any(|(id, _, _)| *id == p.id)));
        assert!(a.estimated_msec.abs_diff(60000) <= 6000);
        assert!(a.pieces.len() <= 512 && a.pieces.iter().map(|p| p.path.len()+p.alternate_path.len()).sum::<usize>() <= 32000);
    }
}
