use mapkit_core::{assembled_preview, assembled_track::{self as track, authoring::*}, Vertex};

fn cross([a, b, c]: [Vertex; 3]) -> [i128; 3] {
    let u = std::array::from_fn::<_, 3, _>(|i| i128::from(b[i] - a[i]));
    let v = std::array::from_fn::<_, 3, _>(|i| i128::from(c[i] - a[i]));
    [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
}

#[test]
fn published_track_faces_match_package_winding_and_shared_join() {
    let mut source = Source::empty();
    source.instances.push(instance("a", "straight", 400));
    let mut second = instance("b", "straight", 400);
    let end = piece(&source.instances[0]).unwrap().path.last().unwrap().position_cm;
    second.position_cm = end;
    source.instances.push(second);
    source.connections.push(Connection { from: "a".into(), to: "b".into() });
    let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
    let geometry = assembled_preview(&document).unwrap();
    for face in &geometry.triangles {
        if face.object_id.starts_with("assembled-road-") || face.object_id=="assembled-venue-floor" {
            // In package coordinates a top points inward; the Godot reflection
            // and index conversion make its clockwise front face point upward.
            assert!(cross(face.vertices)[1] < 0, "{} top must face traffic", face.object_id);
        }
        if face.object_id.starts_with("assembled-shell-") && face.vertices.iter().all(|v|v[1]==-10) {
            assert!(cross(face.vertices)[1] > 0, "underside must face below");
        }
    }
    let edge = |id: &str| -> std::collections::BTreeSet<Vertex> {
        geometry.triangles.iter().filter(|t|t.object_id==id).flat_map(|t|t.vertices)
            .filter(|v|v[2]==end[2]).collect()
    };
    let a = edge("assembled-road-0");
    assert_eq!(a.len(), 2);
    assert_eq!(a, edge("assembled-road-1"), "shared exact seam vertices");
}

#[test]
fn published_wall_shells_keep_outward_godot_faces() {
    for preset in ["straight", "gentle90", "slope_up", "curve_up", "finish_plaza"] {
        let mut source = Source::empty();
        source.instances.push(instance("road", preset, 400));
        let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
        let geometry = assembled_preview(&document).unwrap();
        let walls: Vec<_> = geometry.triangles.iter().filter(|t|t.object_id.starts_with("assembled-wall-")).collect();
        assert!(!walls.is_empty(), "{preset}");
        let volume: i128 = walls.iter().map(|t| {
            let n=cross(t.vertices);
            (0..3).map(|i|n[i]*i128::from(t.vertices[0][i])).sum::<i128>()
        }).sum();
        assert!(volume<0, "{preset}: package shell winding encloses negative signed volume");
    }
}

#[test]
fn published_rotated_curve_joins_have_one_shared_cross_section() {
    for preset in ["gentle90","curve_up","slope_up","spiral90_left_up"] {
        for rotation in [[0,37000,0],[27000,17000,11000]] {
            let mut source=Source::empty();
            let mut a=instance("a",preset,600);
            a.position_cm=[137,211,389];a.rotation_mdeg=rotation;
            let b=snap(&instance("b","straight",600),&a).unwrap();
            let end=piece(&a).unwrap().path.pop().unwrap();
            source.instances=vec![a,b];
            source.connections.push(Connection{from:"a".into(),to:"b".into()});
            let document=track::document_from_assembly(compile(&source).unwrap()).unwrap();
            let geometry=assembled_preview(&document).unwrap();
            let section=|id:&str| -> std::collections::BTreeSet<Vertex> {
                geometry.triangles.iter().filter(|f|f.object_id==id).flat_map(|f|f.vertices)
                    .filter(|v|(0..3).map(|j|(v[j]-end.position_cm[j])*end.forward[j]).sum::<i64>().abs()<1_000_000).collect()
            };
            let left=section("assembled-road-0");
            assert_eq!(left.len(),2,"{preset} {rotation:?}");
            assert_eq!(left,section("assembled-road-1"),"no gap/overlap in published faces {preset} {rotation:?}");
        }
    }
}

const PLACEMENTS: [[i32; 3]; 3] = [[0, 0, 0], [0, 37000, 0], [27000, 17000, 11000]];

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|j| a[j] * b[j]).sum()
}

fn delta(a: Vertex, b: Vertex) -> [f64; 3] {
    std::array::from_fn(|j| (a[j] - b[j]) as f64)
}

// Find the authored surface frame at a published face. Segment distance also
// works for wide curves, where the closest sample alone can belong to a bend
// farther along the route. Bridge and main-road faces use their own path.
fn nearest_normal(vertices: [Vertex; 3], path: &[track::Sample]) -> [f64; 3] {
    let center = std::array::from_fn::<_, 3, _>(|j| {
        vertices.iter().map(|v| v[j] as f64).sum::<f64>() / 3.0
    });
    path.windows(2)
        .map(|w| {
            let direction = delta(w[1].position_cm, w[0].position_cm);
            let offset = std::array::from_fn(|j| center[j] - w[0].position_cm[j] as f64);
            let t = (dot(offset, direction) / dot(direction, direction).max(1.0)).clamp(0.0, 1.0);
            let distance = (0..3).map(|j| (offset[j] - direction[j] * t).powi(2)).sum::<f64>();
            let normal = std::array::from_fn(|j| (w[0].normal[j] + w[1].normal[j]) as f64);
            (distance, normal)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}

#[test]
fn all_catalogue_road_tops_face_the_authored_surface_at_supported_widths_and_placements() {
    let mut cases = 0;
    let mut faces = 0;
    for &preset in track::catalogue_ids() {
        for &width in track::supported_widths(preset) {
            for rotation in PLACEMENTS {
                let mut road = instance("road", preset, width);
                road.position_cm = [137, 211, 389];
                road.rotation_mdeg = rotation;
                let mut source = Source::empty();
                source.instances.push(road);
                let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
                let geometry = assembled_preview(&document).unwrap();
                let p = &document.assembled_track.as_ref().unwrap().pieces[0];
                let before = faces;
                for face in geometry.triangles.iter().filter(|f| f.object_id.starts_with("assembled-road-")) {
                    let path = if face.object_id.ends_with("-bridge") { &p.alternate_path } else { &p.path };
                    let normal = nearest_normal(face.vertices, path);
                    assert!(dot(cross(face.vertices).map(|v| v as f64), normal) < 0.0,
                        "{preset} width={width} rotation={rotation:?}: folded or degenerate top {:?}", face.vertices);
                    faces += 1;
                }
                if p.path.windows(2).any(|w| w.iter().all(|s| !["flight", "loop", "cylinder", "halfpipe"].contains(&s.mode.as_str()))) {
                    assert!(faces > before, "{preset}: ordinary road must publish surfaces");
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 972, "every catalogue width and placement is audited");
    println!("CATALOGUE_ROAD_WINDING cases={cases} faces={faces}");
}

fn ribbon(sample: &track::Sample) -> [Vertex; 2] {
    sample.ribbon_cm.unwrap_or_else(|| {
        let n = sample.normal.map(|v| v as f64);
        let f = sample.forward.map(|v| v as f64);
        let right = [n[1] * f[2] - n[2] * f[1], n[2] * f[0] - n[0] * f[2], n[0] * f[1] - n[1] * f[0]];
        let length = dot(right, right).sqrt();
        [-1.0, 1.0].map(|side| std::array::from_fn(|j| {
            sample.position_cm[j] + (right[j] / length * sample.lateral_cm as f64 * side).round() as i64
        }))
    })
}

fn overlap_area(a: [Vertex; 3], b: [Vertex; 3]) -> f64 {
    let project = |v: Vertex| [v[0] as f64, v[2] as f64];
    let a = a.map(project);
    let b = b.map(project);
    if (0..2).any(|axis| {
        a.iter().map(|v| v[axis]).fold(f64::NEG_INFINITY, f64::max)
            <= b.iter().map(|v| v[axis]).fold(f64::INFINITY, f64::min)
        || b.iter().map(|v| v[axis]).fold(f64::NEG_INFINITY, f64::max)
            <= a.iter().map(|v| v[axis]).fold(f64::INFINITY, f64::min)
    }) { return 0.0; }
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let orientation = side(b[0], b[1], b[2]).signum();
    let mut polygon = a.to_vec();
    for i in 0..3 {
        if polygon.is_empty() { return 0.0; }
        let mut clipped = vec![];
        let mut previous = *polygon.last().unwrap();
        let mut before = side(b[i], b[(i + 1) % 3], previous) * orientation;
        for point in polygon {
            let after = side(b[i], b[(i + 1) % 3], point) * orientation;
            if (before >= 0.0) != (after >= 0.0) {
                let t = before / (before - after);
                clipped.push(std::array::from_fn(|j| previous[j] + (point[j] - previous[j]) * t));
            }
            if after >= 0.0 { clipped.push(point); }
            previous = point;
            before = after;
        }
        polygon = clipped;
    }
    (0..polygon.len()).map(|i| {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        a[0] * b[1] - a[1] * b[0]
    }).sum::<f64>().abs() / 2.0
}

#[test]
fn overpass_lower_ribbon_has_no_folded_edges_or_overlapping_faces() {
    for rotation in PLACEMENTS {
        let mut road = instance("road", "overpass", 400);
        road.rotation_mdeg = rotation;
        let p = piece(&road).unwrap();
        for (index, w) in p.path.windows(2).enumerate() {
            let [a, b] = [ribbon(&w[0]), ribbon(&w[1])];
            let forward = std::array::from_fn(|j| (w[0].forward[j] + w[1].forward[j]) as f64);
            for side in 0..2 {
                assert!(dot(delta(b[side], a[side]), forward) > 0.0,
                    "rotation={rotation:?} segment={index} side={side}: ribbon doubles back");
            }
        }
    }
    let mut source = Source::empty();
    source.instances.push(instance("road", "overpass", 400));
    let document = track::document_from_assembly(compile(&source).unwrap()).unwrap();
    let geometry = assembled_preview(&document).unwrap();
    let faces: Vec<_> = geometry.triangles.iter().filter(|f| f.object_id == "assembled-road-0").collect();
    assert_eq!(faces.len(), 524, "one unfurled pair of faces per original segment");
    for (i, face) in faces.iter().enumerate() {
        for other in &faces[..i] {
            assert!(overlap_area(face.vertices, other.vertices) < 1e-6,
                "lower-road faces overlap: {:?} / {:?}", face.vertices, other.vertices);
        }
    }
}

#[test]
fn overpass_keeps_sample_indices_branch_dimensions_and_shared_ports() {
    let road = instance("road", "overpass", 400);
    let p = piece(&road).unwrap();
    assert_eq!((p.path.len(), p.alternate_path.len()), (263, 65));
    assert!(p.path.iter().all(|s| s.position_cm[1] == 0 && s.lateral_cm == 200));
    assert!(p.alternate_path.iter().all(|s| s.lateral_cm == 100));
    assert_eq!(p.alternate_path[32].position_cm, [0, 200, 1600]);
    for (index, position) in [
        (3, [0, 0, 400]), (35, [-300, 0, 700]), (67, [-600, 0, 1000]),
        (99, [-300, 0, 1300]), (131, [0, 0, 1600]), (163, [300, 0, 1900]),
        (195, [600, 0, 2200]), (227, [300, 0, 2500]), (259, [0, 0, 2800]),
    ] {
        assert_eq!(p.path[index].position_cm, position, "original corner station {index}");
    }
    for (index, position) in [(0, [0, 0, 0]), (262, [0, 0, 3200])] {
        assert_eq!(p.path[index].position_cm, position);
        assert_eq!(p.path[index].forward, [0, 0, 1_000_000]);
        assert_eq!(p.path[index].normal, [0, 1_000_000, 0]);
    }
    assert_eq!(p.alternate_path[0].position_cm, p.path[0].position_cm);
    assert_eq!(p.alternate_path[64].position_cm, p.path[262].position_cm);
    for rotation in PLACEMENTS {
        let mut road = road.clone();
        road.position_cm = [137, 211, 389];
        road.rotation_mdeg = rotation;
        let next = snap(&instance("next", "straight", 400), &road).unwrap();
        let a = piece(&road).unwrap();
        let b = piece(&next).unwrap();
        assert_eq!(a.path.last().unwrap().ribbon_cm, b.path[0].ribbon_cm,
            "shared exit section rotation={rotation:?}");
    }
}

#[test]
fn free_and_flight_curve_end_frames_follow_empty_or_editor_cubic_controls() {
    for preset in ["free_curve", "flight_curve"] {
        for controls in [vec![], vec![[0, 0, 0], [0, 0, 600], [600, 0, 1200], [1200, 0, 1200]]] {
            for rotation in PLACEMENTS {
                let mut road = instance("road", preset, 400);
                road.control_points = controls.clone();
                road.rotation_mdeg = rotation;
                let p = piece(&road).unwrap();
                let end = p.path.last().unwrap();
                let chord = delta(end.position_cm, p.path[p.path.len() - 2].position_cm);
                let forward = end.forward.map(|v| v as f64);
                let alignment = dot(chord, forward) / (dot(chord, chord) * dot(forward, forward)).sqrt();
                // Cubics use the ordinary sampler's twelve-degree turn bound.
                // The exact endpoint checks below separately catch a spurious
                // ninety-degree end frame on an otherwise straight fallback.
                assert!(alignment > 12.0_f64.to_radians().cos(),
                    "{preset} controls={} rotation={rotation:?}: endpoint rotates away from its route ({alignment})", controls.len());
                if rotation == [0; 3] {
                    assert_eq!(end.forward, if controls.is_empty() { [0, 0, 1_000_000] } else { [1_000_000, 0, 0] });
                }
                let next = piece(&snap(&instance("next", "straight", 400), &road).unwrap()).unwrap();
                assert_eq!(end.ribbon_cm, next.path[0].ribbon_cm, "{preset}: snapped port keeps the actual tangent");
            }
        }
    }
}

fn coplanar_overlap(a: [Vertex; 3], b: [Vertex; 3]) -> f64 {
    let normal = cross(a);
    if normal == [0; 3] || b.iter().any(|v| {
        (0..3).map(|j| normal[j] * i128::from(v[j] - a[0][j])).sum::<i128>() != 0
    }) { return 0.0; }
    let axis = (0..3).max_by_key(|&j| normal[j].abs()).unwrap();
    let axes: Vec<_> = (0..3).filter(|&j| j != axis).collect();
    let project = |v: Vertex| [v[axes[0]], 0, v[axes[1]]];
    overlap_area(a.map(project), b.map(project))
}

fn ordinary_segment(w: &[track::Sample]) -> bool {
    w.iter().all(|s| !["flight", "loop", "cylinder", "halfpipe"].contains(&s.mode.as_str()))
}

fn audit_road_geometry(document: &mapkit_core::MapDocument) -> (usize, Vec<String>) {
    let geometry = assembled_preview(document).unwrap();
    let piece = &document.assembled_track.as_ref().unwrap().pieces[0];
    let mut failures = vec![];
    let mut faces = 0;
    for (object, path) in [("assembled-road-0", &piece.path), ("assembled-road-0-bridge", &piece.alternate_path)] {
        if piece.id != "finish_plaza" {
            for (i, w) in path.windows(2).enumerate().filter(|(_, w)| ordinary_segment(w)) {
                let [a, b] = [ribbon(&w[0]), ribbon(&w[1])];
                let forward = std::array::from_fn(|j| (w[0].forward[j] + w[1].forward[j]) as f64);
                for side in 0..2 {
                    if dot(delta(b[side], a[side]), forward) <= 0.0 {
                        failures.push(format!("{object}: folded edge at segment {i}, side {side}"));
                    }
                }
            }
        }
        let mut road: Vec<_> = geometry.triangles.iter().filter(|f| f.object_id == object).collect();
        faces += road.len();
        for (i, face) in road.iter().enumerate() {
            if dot(cross(face.vertices).map(|v| v as f64), nearest_normal(face.vertices, path)) >= 0.0 {
                failures.push(format!("{object}: reversed/degenerate face {i}: {:?}", face.vertices));
            }
        }
        // Sweep the first axis before exact coplanarity/projection. Shared
        // seams have zero intersection area; elevated crossings are distinct
        // planes. Branch mouths intentionally overlap, so audit each road ID.
        road.sort_by_key(|face| face.vertices.iter().map(|v| v[0]).min().unwrap());
        for (i, a) in road.iter().enumerate() {
            let max_x = a.vertices.iter().map(|v| v[0]).max().unwrap();
            for b in &road[i + 1..] {
                if b.vertices.iter().map(|v| v[0]).min().unwrap() > max_x { break; }
                if (1..3).any(|axis| {
                    a.vertices.iter().map(|v| v[axis]).max().unwrap() < b.vertices.iter().map(|v| v[axis]).min().unwrap()
                        || b.vertices.iter().map(|v| v[axis]).max().unwrap() < a.vertices.iter().map(|v| v[axis]).min().unwrap()
                }) { continue; }
                if coplanar_overlap(a.vertices, b.vertices) > 1e-6 {
                    failures.push(format!("{object}: coplanar overlap {:?} / {:?}", a.vertices, b.vertices));
                }
            }
        }
    }
    (faces, failures)
}

fn audit_special_geometry(special: &mapkit_core::special_track::SpecialTrack) -> (usize, Vec<String>) {
    use std::collections::{BTreeMap, BTreeSet};
    let mesh = special.mesh();
    let mut failures = vec![];
    let mut edges = BTreeMap::<[Vertex; 2], (usize, i32)>::new();
    let inner: BTreeSet<_> = mesh.inner.iter().flatten().copied().collect();
    let outer: BTreeSet<_> = mesh.shell.iter().flatten().filter(|v| !inner.contains(*v)).copied().collect();
    // A shell is ten centimetres thick; spatial bins avoid an all-pairs vertex
    // search while locating the independent outside of every contact vertex.
    const BIN: i64 = 1100; // Special meshes use 100 units per centimetre.
    let cell = |v: Vertex| v.map(|n| n.div_euclid(BIN));
    let mut bins = BTreeMap::<Vertex, Vec<Vertex>>::new();
    for vertex in outer { bins.entry(cell(vertex)).or_default().push(vertex); }
    let mut outward = BTreeMap::new();
    for &vertex in &inner {
        let at = cell(vertex);
        let mut nearest: Option<(i128, Vertex)> = None;
        for x in -1..=1 { for y in -1..=1 { for z in -1..=1 {
            if let Some(candidates) = bins.get(&[at[0] + x, at[1] + y, at[2] + z]) {
                for &candidate in candidates {
                    let distance: i128 = (0..3).map(|j| i128::from(candidate[j] - vertex[j]).pow(2)).sum();
                    if nearest.is_none_or(|n| distance < n.0) { nearest = Some((distance, candidate)); }
                }
            }
        } } }
        if let Some((_, candidate)) = nearest { outward.insert(vertex, delta(candidate, vertex)); }
        else { failures.push(format!("special contact vertex has no outer shell: {vertex:?}")); }
    }
    for face in mesh.inner.iter().chain(&mesh.shell) {
        if cross(*face) == [0; 3] { failures.push(format!("degenerate special face {face:?}")); }
        for i in 0..3 {
            let [a, b] = [face[i], face[(i + 1) % 3]];
            let (key, sign) = if a < b { ([a, b], 1) } else { ([b, a], -1) };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += sign;
        }
    }
    for (edge, (count, direction)) in edges {
        if count != 2 || direction != 0 {
            failures.push(format!("special shell boundary {edge:?}: count={count} direction={direction}"));
        }
    }
    for face in &mesh.inner {
        if face.iter().all(|v| outward.contains_key(v)) {
            let shell_direction = std::array::from_fn(|j| face.iter().map(|v| outward[v][j]).sum::<f64>());
            if dot(cross(*face).map(|v| v as f64), shell_direction) >= 0.0 {
                failures.push(format!("special contact face points into its shell: {face:?}"));
            }
        }
    }
    (mesh.inner.len() + mesh.shell.len(), failures)
}

/// Explicit broad audit: all catalogue widths/placements, both extreme taper
/// directions where supported, and the Editor's real default cubic controls.
/// It is opt-in because it also audits every unique dedicated contact mesh.
#[test]
#[ignore = "explicit catalogue audit; set TRACK_GEOMETRY_AUDIT_OUT to retain diagnostic JSON"]
fn catalogue_geometry_audit_includes_tapers_editor_curves_and_special_surfaces() {
    use std::collections::BTreeSet;
    let mut reports = vec![];
    let mut special_reports = vec![];
    let mut special_seen = BTreeSet::new();
    for &preset in track::catalogue_ids() {
        for &width in track::supported_widths(preset) {
            let taper_supported = !preset.starts_with("cylinder")
                && !["loop", "banked_chicane", "overpass", "finish_plaza"].contains(&preset);
            let mut ports = vec![[width, width]];
            if taper_supported { ports.extend([[200, 1200], [1200, 200]]); }
            let mut controls = vec![vec![]];
            if ["free_curve", "flight_curve"].contains(&preset) {
                controls.push(vec![[0, 0, 0], [0, 0, 600], [600, 0, 1200], [1200, 0, 1200]]);
            }
            for control_points in &controls { for &[entry, exit] in &ports { for rotation in PLACEMENTS {
                let mut road = instance("road", preset, width);
                road.entry_width_cm = entry;
                road.exit_width_cm = exit;
                road.control_points = control_points.clone();
                road.position_cm = [137, 211, 389];
                road.rotation_mdeg = rotation;
                let mut source = Source::empty();
                source.instances.push(road);
                let result = compile(&source).and_then(track::document_from_assembly);
                let (faces, failures, geometry_issues) = match result {
                    Err(error) => (0, vec![format!("generation: {error:?}")], vec![]),
                    Ok(document) => {
                        let (faces, failures) = audit_road_geometry(&document);
                        for special in document.gimmicks.iter().filter_map(|g| g.track.as_ref()) {
                            if special_seen.insert(serde_json::to_string(special).unwrap()) {
                                let (faces, failures) = audit_special_geometry(special);
                                special_reports.push(serde_json::json!({"preset":preset,"width":width,"faces":faces,"failures":failures}));
                            }
                        }
                        (faces, failures, document.assembled_track.as_ref().unwrap().geometry_issues.clone())
                    }
                };
                reports.push(serde_json::json!({"preset":preset,"width":width,"entry":entry,"exit":exit,
                    "rotation":rotation,"editor_controls":!control_points.is_empty(),"faces":faces,
                    "geometry_issues":geometry_issues,"failures":failures}));
            } } }
        }
    }
    let failed: Vec<_> = reports.iter().chain(&special_reports).filter(|r| !r["failures"].as_array().unwrap().is_empty()).collect();
    let face_count: u64 = reports.iter().chain(&special_reports).map(|r| r["faces"].as_u64().unwrap()).sum();
    println!("CATALOGUE_GEOMETRY_AUDIT cases={} special_meshes={} faces={face_count} failed_cases={}", reports.len(), special_reports.len(), failed.len());
    if let Ok(path) = std::env::var("TRACK_GEOMETRY_AUDIT_OUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({"cases":reports,"special_meshes":special_reports})).unwrap()).unwrap();
    }
    assert!(failed.is_empty(), "geometry failures (first eight): {:?}", &failed[..failed.len().min(8)]);
}
