use mapkit_core::{
    assembled_preview,
    assembled_track::{self as track, authoring::*},
};
use mapkit_core::{Triangle, Vertex};
use mapkit_package::{assembled_track::compile_source, pack_bytes, read_bytes};
use std::collections::{BTreeMap, BTreeSet};

fn sub(a: Vertex, b: Vertex) -> [f64; 3] {
    std::array::from_fn(|j| (a[j] - b[j]) as f64)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|j| a[j] * b[j]).sum()
}
fn normal(v: [Vertex; 3]) -> [f64; 3] {
    let a = sub(v[1], v[0]);
    let b = sub(v[2], v[0]);
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn on_segment(v: Vertex, a: Vertex, b: Vertex, tolerance: f64) -> bool {
    let d = sub(b, a);
    let p = sub(v, a);
    let t = dot(p, d) / dot(d, d).max(1e-10);
    (-1e-8..=1.00000001).contains(&t)
        && dot(
            std::array::from_fn(|j| p[j] - d[j] * t),
            std::array::from_fn(|j| p[j] - d[j] * t),
        ) <= tolerance * tolerance
}
fn assert_surface(triangles: &[Triangle], path: &[track::Sample], label: &str, clip: bool) {
    let mut boundary = vec![];
    for w in path.windows(2) {
        for side in 0..2 {
            boundary.push((w[0].ribbon_cm.unwrap()[side], w[1].ribbon_cm.unwrap()[side]));
        }
    }
    for i in [0, path.len() - 1] {
        let [a, b] = path[i].ribbon_cm.unwrap();
        boundary.push((a, b));
    }
    let mut edges = BTreeMap::<[Vertex; 2], usize>::new();
    for f in triangles
        .iter()
        .filter(|t| t.object_id == "assembled-road-3")
    {
        let n = normal(f.vertices);
        assert!(
            n[1] > 0.0,
            "{label}: degenerate/inverted face {:?}",
            f.vertices
        );
        for i in 0..3 {
            let mut e = [f.vertices[i], f.vertices[(i + 1) % 3]];
            e.sort();
            *edges.entry(e).or_default() += 1;
        }
    }
    assert!(!edges.is_empty());
    for (e, count) in edges {
        assert!(count <= 2, "{label}: overlapping edge {e:?}");
        if count == 1 {
            assert!(
                boundary.iter().any(|&(a, b)| e.iter().all(|&v| on_segment(
                    v,
                    a,
                    b,
                    if clip { 1.5 } else { 1e-7 }
                ))),
                "{label}: unmatched interior edge {e:?}"
            );
        }
    }
}

#[test]
fn spiral_package_surface() {
    let mut rows = vec![];
    for degrees in [90, 180, 360] {
        for side in ["left", "right"] {
            for direction in ["up", "down"] {
                let preset = format!("spiral{degrees}_{side}_{direction}");
                for &width in track::supported_widths(&preset) {
                    for placement in 0..2 {
                        let started = std::time::Instant::now();
                        let mut source = Source::empty();
                        source.settings.circuit = false;
                        let mut road = instance("spiral", &preset, width);
                        if placement == 1 {
                            road.entry_width_cm = 200;
                            road.exit_width_cm = 1200;
                            road.rotation_mdeg = [0, 37000, 0];
                            road.position_cm = [3190, 200, -3210];
                        }
                        for i in 0..7 {
                            let mut item = match i {
                                3 => road.clone(),
                                6 => instance("finish", "finish_plaza", 400),
                                _ => {
                                    let mut straight = instance(
                                        &format!("straight-{i}"),
                                        "free_curve",
                                        if i < 3 {
                                            road.entry_width_cm
                                        } else {
                                            road.exit_width_cm
                                        },
                                    );
                                    straight.control_points =
                                        vec![[0, 0, 0], [0, 0, 1600], [0, 0, 3200], [0, 0, 4800]];
                                    straight
                                }
                            };
                            if i == 5 {
                                item.exit_width_cm = 400;
                            }
                            if i == 0 {
                                item.position_cm = road.position_cm;
                                item.rotation_mdeg = road.rotation_mdeg;
                            }
                            if let Some(previous) = source.instances.last() {
                                item = snap(&item, previous).unwrap();
                                source.connections.push(Connection {
                                    from: previous.id.clone(),
                                    to: item.id.clone(),
                                });
                            }
                            source.instances.push(item);
                        }
                        source.paths.push(Path {
                            id: "base".into(),
                            pieces: source.instances.iter().map(|i| i.id.clone()).collect(),
                        });
                        source.checkpoints = vec![
                            Checkpoint {
                                piece: source.instances[2].id.clone(),
                                sample: 0,
                            },
                            Checkpoint {
                                piece: "finish".into(),
                                sample: 2,
                            },
                        ];
                        let document = compile_source(&source).unwrap();
                        let bytes = pack_bytes(document, BTreeMap::new()).unwrap();
                        let reopened = read_bytes(&bytes).unwrap();
                        let d = reopened.document.to_document();
                        let geometry = assembled_preview(&d).unwrap();
                        let a = d.assembled_track.as_ref().unwrap();
                        let label = format!("{preset}/{width}/{placement}");
                        assert_surface(&geometry.triangles, &a.pieces[3].path, &label, false);
                        // The underside is a translation of exactly the same triangles,
                        // including transitions and diagonal choice, with opposite winding.
                        let shells: BTreeSet<_> = geometry
                            .triangles
                            .iter()
                            .filter(|t| t.object_id == "assembled-shell-3")
                            .map(|t| {
                                let mut v = t.vertices;
                                v.sort();
                                v
                            })
                            .collect();
                        for t in geometry
                            .triangles
                            .iter()
                            .filter(|t| t.object_id == "assembled-road-3")
                        {
                            let mut v = t.vertices.map(|mut v| {
                                v[1] -= 10;
                                v
                            });
                            v.sort();
                            assert!(shells.contains(&v), "{label}: different underside");
                        }
                        assert_eq!(
                            a.pieces[3].path.last().unwrap().ribbon_cm,
                            a.pieces[4].path.first().unwrap().ribbon_cm,
                            "{label}: port mismatch"
                        );
                        // A rotated narrow fixture crosses cell boundaries in both axes.
                        if width == 400 && placement == 1 && direction == "up" && side == "left" {
                            let prepared = mapkit_core::PreparedMap::new(d.clone()).unwrap();
                            let cells: BTreeSet<_> = geometry
                                .triangles
                                .iter()
                                .filter(|t| t.object_id == "assembled-road-3")
                                .flat_map(|t| t.vertices)
                                .map(|v| {
                                    let c = d.cell_at([v[0], v[2]]).unwrap();
                                    (c.x, c.y)
                                })
                                .collect();
                            let mut clipped = vec![];
                            for (x, y) in cells {
                                let cell = mapkit_core::Cell { x, y };
                                let cost = prepared.estimate(cell, 500_000).unwrap();
                                let occupied = prepared
                                    .generate_with_occupancy(
                                        cell,
                                        None,
                                        500_000,
                                        Some(mapkit_core::MAX_OCCUPIED_SOLIDS),
                                    )
                                    .unwrap();
                                assert!(occupied.chunk.triangles.len() as u64 <= cost.triangles);
                                assert!(occupied.solids.len() as u64 <= cost.occupied_solids);
                                assert_eq!(
                                    occupied.chunk,
                                    prepared.generate(cell, None, 500_000).unwrap()
                                );
                                clipped.extend(occupied.chunk.triangles);
                            }
                            assert_surface(&clipped, &a.pieces[3].path, &label, true);
                        }
                        rows.push(serde_json::json!({"preset":preset,"width":width,"placement":placement,
                "path":a.pieces[3].path,"after":a.pieces[4].path,
                "triangles":geometry.triangles.iter().filter(|t|t.object_id=="assembled-road-3" || t.object_id=="assembled-road-4" || t.object_id=="assembled-shell-3").map(|t| {let mut t=t.clone();t.object_id=t.object_id.replace("road-3","road-0").replace("road-4","road-1").replace("shell-3","shell-0");t}).collect::<Vec<_>>(),
                "all_triangles":geometry.triangles.len(),
                "package_bytes":bytes.len(),"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
                    }
                }
            }
        }
    }
    if let Ok(path) = std::env::var("SPIRAL_GEOMETRY_OUT") {
        std::fs::write(path, serde_json::to_vec(&rows).unwrap()).unwrap();
    }
    println!("SPIRAL_GEOMETRY cases={}", rows.len());
}
