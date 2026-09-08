use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap()
}
fn chunk(d: &MapDocument, cell: Cell, grid: Option<&HeightGrid>) -> GeneratedChunk {
    generate(GenerationInput {
        document: d,
        cell,
        heightgrid: grid,
        max_triangles: 200_000,
    })
    .unwrap()
}
fn sample(c: &GeneratedChunk, p: Point, id: &str) -> Result<Vertex> {
    c.spawn(&SpawnRequest {
        position_cm: p,
        surface_id: id.into(),
    })
}
fn surface_count(c: &GeneratedChunk, p: Point, height: i64) -> usize {
    c.triangles
        .iter()
        .filter(|t| {
            if !t.spawnable {
                return false;
            }
            let one = GeneratedChunk {
                format_version: 6,
                cell: c.cell,
                triangles: vec![(*t).clone()],
                objects: vec![],
            };
            sample(&one, p, &t.object_id).is_ok_and(|v| v[1] == height)
        })
        .count()
}
#[test]
fn explicit_graph_segment_material_width_and_layered_profiles() {
    let d = document();
    d.validate().unwrap();
    assert_eq!(
        d.connected_roads("junction").unwrap(),
        ["branch", "ground-east", "ground-west"]
    );
    assert_eq!(d.connected_roads("bridge-from").unwrap(), ["bridge"]);
    assert!(d.connected_roads("absent").is_err());
    let c = chunk(&d, Cell { x: 0, y: 0 }, None);
    assert_eq!(sample(&c, [2500, 1000], "bridge").unwrap()[1], 600);
    assert_eq!(sample(&c, [2500, 1000], "ground-west").unwrap()[1], 0);
    assert!(sample(&c, [2500, 1000], "terrain").is_err());
    assert!(sample(&c, [2500, 1301], "ground-west").is_err());
    let c = chunk(&d, Cell { x: 1, y: 0 }, None);
    assert_eq!(sample(&c, [7500, 1400], "elevated").unwrap()[1], 700);
    assert!(c
        .triangles
        .iter()
        .any(|t| t.object_id == "ground-east" && t.surface == Surface::Gravel));
    for p in [[5051, 1301], [4991, 1251], [4891, 1491], [5011, 2501]] {
        assert_eq!(
            surface_count(&c, p, 0),
            usize::from(p[0] >= 5000),
            "junction {p:?}"
        );
    }
}
#[test]
fn terrain_conformance_has_one_surface_and_keeps_crossfall() {
    let mut d = document();
    d.bounds.max = [5000, 5000];
    d.roads.retain(|r| r.id == "ground-west");
    d.nodes
        .retain(|n| ["ground-west-from", "junction"].contains(&n.id.as_str()));
    d.heightmaps.push(Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "terrain.png".into(),
        spacing_cm: 200,
        offset_cm: -1000,
        step_cm: 1,
        source_accuracy_cm: None,
    });
    let grid = HeightGrid {
        side: 26,
        heights_cm: (0..26 * 26)
            .map(|i| -1000 + (i % 26) as i64 * 20 + (i / 26) as i64 * 40)
            .collect(),
    };
    let c = chunk(&d, Cell { x: 0, y: 0 }, Some(&grid));
    for p in [[1001, 901], [2001, 1203], [4991, 1091]] {
        let height = (-10000 + p[0] + 2 * p[1]) / 10;
        assert_eq!(sample(&c, p, "ground-west").unwrap()[1], height);
        assert_eq!(
            surface_count(&c, p, height),
            1,
            "no overlapping grass or road {p:?}"
        );
    }
    for t in c.triangles.iter().filter(|t| t.object_id == "ground-west") {
        for v in t.vertices {
            assert!((v[1] - (-10000 + v[0] + 2 * v[2]) / 10).abs() <= 1);
        }
    }
}
#[test]
fn open_cut_tunnel_portals_ceiling_and_bend_have_continuous_floors() {
    let d = document();
    for x in [0, 1] {
        for y in [0, 1] {
            let c = chunk(&d, Cell { x, y }, None);
            let center = if x == 0 { 3001 } else { 7001 };
            if y == 1 {
                assert_eq!(sample(&c, [center, 8001], "tunnel").unwrap()[1], -600);
                assert_eq!(sample(&c, [center, 8001], "terrain").unwrap()[1], 0);
                assert!(c.triangles.iter().any(|t| t.object_id == "tunnel"
                    && !t.spawnable
                    && t.vertices.iter().all(|p| p[1] == -300)));
                assert!(
                    sample(&c, [if x == 0 { 501 } else { 9501 }, 8001], "terrain").is_err(),
                    "open tunnel entrance"
                );
            }
            assert!(
                sample(&c, [center, 5000], "terrain").is_err(),
                "underpass is open above"
            );
            assert_eq!(sample(&c, [center, 5000], "underpass").unwrap()[1], -500);
        }
    }
    let mut d = d;
    let r = d.roads.iter_mut().find(|r| r.id == "tunnel").unwrap();
    r.points[2][2] = 7500;
    r.points[3][2] = 7500;
    for x in [0, 1] {
        let c = chunk(&d, Cell { x, y: 1 }, None);
        for t in c
            .triangles
            .iter()
            .filter(|t| t.spawnable && t.object_id == "tunnel")
        {
            let p = [
                (t.vertices[0][0] + t.vertices[1][0] + t.vertices[2][0]) / 3,
                (t.vertices[0][2] + t.vertices[1][2] + t.vertices[2][2]) / 3,
            ];
            assert!(sample(&c, p, "tunnel").is_ok());
        }
    }
}
#[test]
fn order_seams_budget_and_archive_are_stable() {
    let mut d = document();
    let a = chunk(&d, Cell { x: 0, y: 1 }, None);
    let c = chunk(&d, Cell { x: 1, y: 1 }, None);
    for (y, id) in [(5000, "underpass"), (8000, "tunnel")] {
        for delta in (if y == 5000 { 0 } else { -299 })..300 {
            assert_eq!(
                sample(&a, [5000, y + delta], id).unwrap(),
                sample(&c, [5000, y + delta], id).unwrap()
            );
        }
    }
    let hash = a.hash().unwrap();
    d.roads.reverse();
    d.nodes.reverse();
    assert_eq!(chunk(&d, a.cell, None).hash().unwrap(), hash);
    let cost = estimate_generation(&d, a.cell, 200_000).unwrap();
    assert!(cost.triangles >= a.triangles.len() as u64);
    let key = archive_key(&"a".repeat(64), a.cell);
    let bytes = encode_archive(&a, &key, archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&bytes, &key, a.cell, &cost).unwrap(), a);
    assert_eq!(
        generate(GenerationInput {
            document: &d,
            cell: a.cell,
            heightgrid: None,
            max_triangles: 1
        })
        .unwrap_err()
        .code,
        "E_BUDGET"
    );
    assert_eq!(
        chunk(&d, a.cell, None).hash().unwrap(),
        hash,
        "failure does not poison retry"
    );
}

#[test]
fn explicit_ground_portal_connects_and_rejects_an_incompatible_apron() {
    let mut d = document();
    d.nodes.push(RoadNode {
        id: "approach-end".into(),
        position: [0, 0, 4000],
        level: 0,
    });
    d.roads.push(Road {
        id: "approach".into(),
        from: "underpass-from".into(),
        to: "approach-end".into(),
        points: vec![[0, 0, 5000], [0, 0, 4000]],
        widths_cm: vec![600],
        surfaces: vec![Surface::Asphalt],
        kind: RoadKind::Ground,
        clearance_cm: None,
        sidewalk_cm: None,
    });
    assert_eq!(
        d.connected_roads("underpass-from").unwrap(),
        ["approach", "underpass"]
    );
    let c = chunk(&d, Cell { x: 0, y: 0 }, None);
    assert_eq!(sample(&c, [400, 4900], "underpass").unwrap()[1], 0);
    d.roads
        .iter_mut()
        .find(|r| r.id == "underpass")
        .unwrap()
        .points[1][1] = -100;
    assert_eq!(
        generate(GenerationInput {
            document: &d,
            cell: c.cell,
            heightgrid: None,
            max_triangles: 200_000
        })
        .unwrap_err()
        .code,
        "E_GEOMETRY"
    );
}
#[test]
fn junction_degree_and_negative_partial_bounds_fail_closed() {
    let mut d = document();
    let r = d.roads.iter().find(|r| r.id == "branch").unwrap().clone();
    for i in 0..33 {
        let mut r = r.clone();
        r.id = format!("duplicate-arm-{i}");
        d.roads.push(r);
    }
    assert_eq!(d.validate().unwrap_err().code, "E_LIMIT");
    let mut d = document();
    d.bounds.min = [-837, -851];
    d.bounds.max = [9163, 9149];
    for n in &mut d.nodes {
        n.position[0] -= 837;
        n.position[2] -= 851;
    }
    for r in &mut d.roads {
        for p in &mut r.points {
            p[0] -= 837;
            p[2] -= 851;
        }
    }
    let c = chunk(&d, Cell { x: 0, y: 1 }, None);
    let cost = estimate_generation(&d, c.cell, 200_000).unwrap();
    assert_eq!(cost.generation_scratch_bytes, 16 * 1024 * 1024);
    assert!(c.triangles.len() as u64 <= cost.triangles);
    for t in &c.triangles {
        for p in t.vertices {
            assert!(d.cell_bounds(c.cell).unwrap().contains([p[0], p[2]]));
        }
    }
}
