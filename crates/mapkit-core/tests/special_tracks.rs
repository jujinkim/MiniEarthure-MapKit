use mapkit_core::*;
use std::collections::BTreeMap;
fn templates() -> BTreeMap<String, gimmick::Gimmick> {
    serde_json::from_str(include_str!("../../../godot/driving_templates.json")).unwrap()
}
#[test]
fn bounded_templates_effects_meshes_and_spawn_exclusion() {
    let all = templates();
    for g in all.values() {
        assert!(g.valid(), "{}", g.id);
    }
    for id in ["loop", "cylinder"] {
        let g = &all[id];
        let t = g.track.as_ref().unwrap();
        let mesh = t.mesh();
        assert_eq!(mesh.inner.len(), t.tile_count() * 2);
        assert_eq!(mesh.tiles.len() as u64, g.occupied_count());
        assert!(g.memory_bytes() > mesh.inner.len() as u64 * 200);
        assert!(g.parts.is_empty());
        assert!(g.excludes_spawn(g.position));
        assert!(!g.excludes_spawn([g.position[0] + 10000, g.position[1], g.position[2]]));
        for f in mesh.inner.iter().chain(&mesh.shell) {
            let u = std::array::from_fn::<_, 3, _>(|a| f[1][a] - f[0][a]);
            let v = std::array::from_fn::<_, 3, _>(|a| f[2][a] - f[0][a]);
            assert_ne!(
                [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0]
                ],
                [0; 3],
                "nondegenerate {id}"
            );
        }
        let resolved = g.resolved_json();
        assert!(resolved["track_mesh"]["inner"].is_array());
        let mut invalid = g.clone();
        invalid.track.as_mut().unwrap().radius_cm = u32::MAX;
        assert!(!invalid.valid());
        invalid = g.clone();
        invalid.motion.kind = gimmick::MotionKind::Rotate;
        assert!(!invalid.valid());
        invalid = g.clone();
        invalid.parts = all["boost"].parts.clone();
        assert!(!invalid.valid());
    }
    let mut g = all["target_speed"].clone();
    g.effect.as_mut().unwrap().strength_percent = 0;
    assert!(!g.valid());
    g = all["jump_height"].clone();
    g.effect.as_mut().unwrap().jump_height_cm = 0;
    assert!(!g.valid());
    g = all["boost"].clone();
    g.parts = vec![g.parts[0].clone(); 33];
    assert!(!g.valid(), "ordinary convex cap unchanged");
}
#[test]
fn special_track_cost_archive_order_and_hollow_occupancy() {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    d.bounds = Bounds {
        min: [0, 0],
        max: [25600, 25600],
    };
    d.cell_size_cm = 3200;
    d.gimmicks = templates()
        .into_values()
        .filter(|g| g.track.is_some() || g.effect.is_some())
        .collect();
    d.normalize();
    gimmick::validate(&d).unwrap();
    let cell = Cell { x: 2, y: 2 };
    let cost = estimate_generation(&d, cell, 500_000).unwrap();
    assert!(cost.occupied_solids >= 2304);
    let generated = generate(GenerationInput {
        document: &d,
        cell,
        heightgrid: None,
        max_triangles: 500_000,
    })
    .unwrap();
    let key = archive_key("tracks", cell);
    let archive = encode_archive(&generated, &key, archive_limit(&cost)).unwrap();
    assert_eq!(
        decode_archive(&archive, &key, cell, &cost).unwrap(),
        generated
    );
    let mut small = cost.clone();
    small.gimmick_bytes = 0;
    assert!(decode_archive(&archive, &key, cell, &small).is_err());
    let hash = generated.hash().unwrap();
    d.gimmicks.reverse();
    d.normalize();
    assert_eq!(
        generate(GenerationInput {
            document: &d,
            cell,
            heightgrid: None,
            max_triangles: 500_000
        })
        .unwrap()
        .hash()
        .unwrap(),
        hash
    );
    let tube = d.gimmicks.iter().find(|g| g.id == "cylinder").unwrap();
    let center = [tube.position[0], tube.position[1] + i64::from(tube.track.as_ref().unwrap().radius_cm), tube.position[2]];
    assert!(
        !tube
            .occupancy_bounds()
            .iter()
            .any(|(lo, hi)| (0..3).all(|a| center[a] >= lo[a] && center[a] <= hi[a])),
        "hollow center"
    );
}

#[test]
fn quantized_surface_direction_changes_stay_below_five_degrees() {
    fn normal(f: &[Vertex; 3]) -> [f64; 3] {
        let u = std::array::from_fn::<_, 3, _>(|a| (f[1][a] - f[0][a]) as f64);
        let v = std::array::from_fn::<_, 3, _>(|a| (f[2][a] - f[0][a]) as f64);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = libm::sqrt(n.iter().map(|v| v * v).sum());
        n.map(|v| v / length)
    }
    for id in ["loop", "cylinder"] {
        for radius in if id == "cylinder" { vec![50, 100, 125, 150, 250, 600] } else { vec![150, 250, 600] } {
            for length in [600, 1600, 3200] {
                let mut t = templates()[id].track.clone().unwrap();
                t.radius_cm = radius;
                t.length_cm = length;
                let mesh = t.mesh();
                let steps = t.longitudinal_parameters().len()-1;
                let bands = t.tile_count() / steps;
                assert_eq!(mesh.units_per_metre, 10000);
                let ns: Vec<_> = mesh.inner.iter().map(normal).collect();
                let mut largest = 0.0f64;
                for i in 0..steps {
                    for j in 0..bands {
                        for k in 0..2 {
                            let index = (i * bands + j) * 2 + k;
                            for other in [
                                index ^ 1,
                                ((i + 1).min(steps-1) * bands + j) * 2 + k,
                                (i * bands + (j + 1).min(bands - 1)) * 2 + k,
                            ] {
                                let dot = (0..3)
                                    .map(|a| ns[index][a] * ns[other][a])
                                    .sum::<f64>()
                                    .clamp(-1.0, 1.0);
                                largest = largest.max(libm::acos(dot).to_degrees());
                            }
                        }
                    }
                }
                assert!(largest <= 5.0, "{id} r={radius} l={length}: {largest}");
                // Analytic maximum sag plus integer quantization, in centimetres.
                let largest_step=t.longitudinal_parameters().windows(2).map(|p|p[1]-p[0]).fold(0.0,f64::max);
                let angular = 1.6 * radius as f64 * (1.0 - libm::cos(std::f64::consts::PI * largest_step));
                let axial = if id == "cylinder" {
                    0.06 * radius as f64 * (1.0 - libm::cos(std::f64::consts::PI / bands as f64))
                } else {
                    0.0
                };
                assert!(
                    angular + axial + 0.009 < 1.0,
                    "one centimetre approximation bound"
                );
            }
        }
    }
}

#[test]
fn swept_tubes_share_hollow_geometry_frames_and_bounds() {
    let c = assembled_track::catalogue();
    let pieces: Vec<assembled_track::Piece> = serde_json::from_value(c["pieces"].clone()).unwrap();
    for p in pieces.iter().filter(|p| p.id.starts_with("cylinder")) {
        let samples: Vec<_> = p.path.iter().filter(|s| s.mode == "cylinder").collect();
        let t = special_track::SpecialTrack {
            kind: special_track::TrackKind::SweptCylinder,
            radius_cm: if p.id.starts_with("cylinder_wide") {
                200
            } else {
                100
            },
            width_cm: 400,
            length_cm: 1600,
            centerline: samples
                .iter()
                .map(|s| special_track::TubeFrame {
                    floor_cm: s.position_cm,
                    normal: s.normal,
                    forward: s.forward,
                })
                .collect(),
        };
        assert!(t.valid(), "{}", p.id);
        assert!(
            samples.iter().any(|s| s.forward[0].abs() > 500_000),
            "real bend, not axial roll"
        );
        // Measure actual circular coordinates; a preset name cannot certify radius.
        let sign = if p.id.ends_with("left") { -1.0 } else { 1.0 };
        let centers = if p.id.contains("curve") || p.id.contains("uturn") {
            vec![[sign * 400.0, 400.0]]
        } else {
            vec![[-400.0, 800.0], [-400.0, 1600.0], [-400.0, 2400.0]]
        };
        for sample in &samples {
            let z = sample.position_cm[2];
            if !p.id.contains("curve") && !p.id.contains("uturn") && !(800..=2400).contains(&z) {
                continue;
            }
            if p.id.contains("curve") && (z < 400 || sample.position_cm[0].abs() > 400) {
                continue;
            }
            if p.id.contains("uturn") && z < 400 {
                continue;
            }
            let radius = centers
                .iter()
                .map(|c| {
                    ((sample.position_cm[0] as f64 - c[0]).powi(2) + (z as f64 - c[1]).powi(2))
                        .sqrt()
                })
                .fold(f64::INFINITY, f64::min);
            assert!(
                (radius - 400.0).abs() < 1.0,
                "{} measured radius {radius}",
                p.id
            );
        }
        let m = t.mesh();
        assert_eq!(m.inner.len(), t.tile_count() * 2);
        for (i, s) in samples.iter().enumerate() {
            assert!((s.tube_radius_cm as f64 - t.swept_radius(i)).abs() <= 0.5);
            let center = std::array::from_fn(|j| {
                s.position_cm[j] as f64 + s.normal[j] as f64 / 1e6 * t.swept_radius(i)
            });
            assert!(t.contains_swept(center, 0.0));
            assert!(
                !m.tiles.iter().any(|(lo, hi)| (0..3)
                    .all(|j| center[j] >= lo[j] as f64 && center[j] <= hi[j] as f64)),
                "hollow {}",
                p.id
            );
        }
        for f in m.inner.iter().chain(&m.shell) {
            let a = std::array::from_fn::<_, 3, _>(|j| f[1][j] - f[0][j]);
            let b = std::array::from_fn::<_, 3, _>(|j| f[2][j] - f[0][j]);
            assert_ne!(
                [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0]
                ],
                [0; 3]
            );
            assert!(f
                .iter()
                .all(|p| p.iter().map(|v| v.abs() / 100).sum::<i64>() <= t.bound_radius()));
        }
        let mut bad = t.clone();
        bad.centerline[0].floor_cm = [i64::MIN; 3];
        assert!(!bad.valid());
        let mut bad = t.clone();
        bad.centerline[1].floor_cm = bad.centerline[0].floor_cm;
        assert!(!bad.valid());
        let mut bad = t.clone();
        bad.centerline[0].normal = [0; 3];
        assert!(!bad.valid());
        let mut bad = t.clone();
        bad.centerline.resize(513, bad.centerline[0].clone());
        assert!(!bad.valid());
    }
}

#[test]
fn curved_halfpipe_has_open_crown_and_real_solid_sidewalls() {
    let mut source=assembled_track::authoring::Source::empty();
    source.instances.push(assembled_track::authoring::instance("halfpipe","banked_chicane",400));
    let d=assembled_track::document_from_assembly(assembled_track::authoring::compile(&source).unwrap()).unwrap();
    let t = d
        .gimmicks
        .iter()
        .find_map(|g| {
            g.track
                .as_ref()
                .filter(|t| t.kind == special_track::TrackKind::SweptHalfPipe)
        })
        .unwrap();
    assert!(t.valid());
    let mesh = t.mesh();
    assert_eq!(mesh.inner.len(), t.tile_count() * 2);
    assert!(
        mesh.inner.iter().flatten().all(|v| v[1] <= 22500),
        "no ceiling above the 2m banks"
    );
    assert!(
        mesh.inner.iter().flatten().any(|v| v[1] > 19000),
        "real side bank, not flat ribbon"
    );
    let f = &t.centerline[t.centerline.len() / 2];
    let crown = [f.floor_cm[0], f.floor_cm[1] + 400, f.floor_cm[2]];
    assert!(
        !mesh
            .tiles
            .iter()
            .any(|(a, b)| (0..3).all(|j| crown[j] >= a[j] && crown[j] <= b[j])),
        "open crown occupancy"
    );
}
