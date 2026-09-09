use mapkit_core::*;
fn doc() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/courtyard/document.json")).unwrap()
}
fn gen(d: &MapDocument, cell: Cell) -> GeneratedOccupancy {
    generate_with_occupancy(
        GenerationInput {
            document: d,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        },
        20_000,
    )
    .unwrap()
}
fn cross(t: [Point; 3]) -> i128 {
    (t[1][0] - t[0][0]) as i128 * (t[2][1] - t[0][1]) as i128
        - (t[1][1] - t[0][1]) as i128 * (t[2][0] - t[0][0]) as i128
}
#[test]
fn courtyard_roofs_solids_queries_cost_archive_and_seams() {
    let d = doc();
    d.validate().unwrap();
    let mut area = 0;
    for y in 0..2 {
        for x in 0..2 {
            let cell = Cell { x, y };
            let g = gen(&d, cell);
            let cost = estimate_generation(&d, cell, 500_000).unwrap();
            assert!(cost.triangles >= g.chunk.triangles.len() as u64);
            assert!(cost.building_prisms >= g.chunk.building_prisms.len() as u64);
            assert!(cost.occupied_solids >= g.solids.len() as u64);
            for p in &g.chunk.building_prisms {
                area += cross(p.footprint).abs();
                for hole in [[12000, 12000], [5000, 5000]] {
                    assert!(!point_in_polygon(hole, &p.footprint));
                }
            }
            for s in &g.solids {
                if let SolidShape::SlopedPrism { footprint, .. } = &s.shape {
                    assert!(!point_in_polygon([12000, 12000], footprint));
                }
            }
            let point = [
                if x == 0 { 12000 } else { 14000 },
                if y == 0 { 12000 } else { 14000 },
            ];
            assert!(g
                .chunk
                .spawn(&SpawnRequest {
                    position_cm: point,
                    surface_id: "terrain".into()
                })
                .is_ok());
            let key = archive_key(&"a".repeat(64), cell);
            let bytes = encode_archive(&g.chunk, &key, archive_limit(&cost)).unwrap();
            assert_eq!(decode_archive(&bytes, &key, cell, &cost).unwrap(), g.chunk);
        }
    }
    assert_eq!(area, 2 * (21000_i128 * 21000 - 10000 * 10000 - 2000 * 2000));
    assert!(!d.buildings[0].contains([12000, 12000]));
    assert!(d.buildings[0].contains([8000, 12000]));
    assert!(d.buildings[0].contains([7000, 12000]));
    let view = overview(&d).unwrap();
    assert_eq!(view.cost().unwrap().points_2d, 12);
    assert_eq!(
        view.cost().unwrap().records,
        3 + d.attributions.len() as u64
    );
    assert_eq!(view.buildings[0].holes.len(), 2);
}
#[test]
fn ring_rotation_winding_hole_order_and_collinearity_are_deterministic() {
    let d = doc();
    let mut other = d.clone();
    other.buildings[0].footprint.reverse();
    other.buildings[0].footprint.rotate_left(2);
    other.buildings[0].holes.reverse();
    for h in &mut other.buildings[0].holes {
        h.reverse();
        h.rotate_left(1);
    }
    let a = other.buildings[0].footprint[0];
    let b = other.buildings[0].footprint[1];
    other.buildings[0]
        .footprint
        .insert(1, [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]);
    for y in 0..2 {
        for x in 0..2 {
            let c = Cell { x, y };
            assert_eq!(gen(&d, c).chunk, gen(&other, c).chunk);
        }
    }
}
#[test]
fn invalid_topology_version_roof_and_limits_fail_closed() {
    for version in 1..5 {
        let mut d = doc();
        d.recipe_version = version;
        assert_eq!(d.validate().unwrap_err().code, "E_VERSION");
    }
    let cases = vec![
        vec![[0, 0], [100, 0], [100, 100], [0, 100]],
        vec![[2000, 3000], [3000, 3000], [3000, 4000], [2000, 4000]],
        vec![[4000, 4000], [6000, 6000], [4000, 6000], [6000, 4000]],
        vec![[9000, 9000], [10000, 9000], [10000, 10000], [9000, 10000]],
    ];
    for hole in cases {
        let mut d = doc();
        d.buildings[0].holes.push(hole);
        assert!(d.validate().is_err());
    }
    let mut d = doc();
    d.buildings[0].roof = "gable".into();
    assert!(d.validate().is_err());
    let mut d = doc();
    d.buildings[0].holes = vec![d.buildings[0].holes[0].clone(); 17];
    assert!(d.validate().is_err());
}
#[test]
fn courtyard_can_hold_an_island_building_and_manual_asset_but_walls_reject() {
    let mut d = doc();
    let mut island = d.buildings[0].clone();
    island.id = "island".into();
    island.holes.clear();
    island.footprint = vec![
        [10000, 10000],
        [11000, 10000],
        [11000, 11000],
        [10000, 11000],
    ];
    d.buildings.push(island);
    d.validate().unwrap();
    d.placements.push(Placement {
        id: "tree".into(),
        asset_id: "builtin:tree".into(),
        position: [14000, 0, 14000],
        quarter_turns: 0,
    });
    d.validate().unwrap();
    d.placements[0].position[0] = 8100;
    assert!(d.validate().is_err());
    d.placements.clear();
    d.buildings[1].footprint[0] = [7000, 10000];
    assert!(d.validate().is_err());
}
#[test]
fn concave_outer_and_concave_holes_keep_their_exact_area() {
    let mut d = doc();
    d.buildings[0].footprint = vec![
        [2000, 2000],
        [23000, 2000],
        [23000, 23000],
        [15000, 23000],
        [15000, 20000],
        [2000, 20000],
    ];
    d.buildings[0].holes = vec![vec![
        [8000, 8000],
        [18000, 8000],
        [18000, 18000],
        [14000, 18000],
        [14000, 14000],
        [8000, 14000],
    ]];
    d.validate().unwrap();
    let mut total = 0;
    for y in 0..2 {
        for x in 0..2 {
            total += gen(&d, Cell { x, y })
                .chunk
                .building_prisms
                .iter()
                .map(|p| cross(p.footprint).abs())
                .sum::<i128>();
        }
    }
    assert_eq!(
        total,
        2 * (21000_i128 * 21000 - 13000 * 3000 - 10000 * 10000 + 6000 * 4000)
    );
}
#[test]
fn courtyard_road_and_vegetation_are_allowed_only_with_full_clearance() {
    let mut d = doc();
    let source: MapDocument =
        serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    let mut r = source.roads[0].clone();
    r.id = "inside-road".into();
    r.from = "a".into();
    r.to = "b".into();
    r.points = vec![[10000, 20, 12000], [16000, 20, 12000]];
    r.widths_cm = vec![400];
    r.surfaces = vec![Surface::Asphalt];
    r.sidewalk_cm = Some(100);
    let mut a = source.nodes[0].clone();
    a.id = "a".into();
    a.position = r.points[0];
    let mut b = a.clone();
    b.id = "b".into();
    b.position = r.points[1];
    d.nodes = vec![a, b];
    d.roads = vec![r];
    d.validate().unwrap();
    let g = gen(&d, Cell { x: 0, y: 0 });
    assert!(g
        .chunk
        .spawn(&SpawnRequest {
            position_cm: [11000, 12000],
            surface_id: "inside-road".into()
        })
        .is_ok());
    d.roads[0].widths_cm[0] = 9000;
    assert!(d.validate().is_err());
    d.roads.clear();
    d.nodes.clear();
    d.zones.push(Zone {
        id: "court-trees".into(),
        polygon: vec![[9000, 9000], [17000, 9000], [17000, 17000], [9000, 17000]],
        kind: ZoneKind::Orchard,
        spacing_cm: 1000,
        density_per_mille: 1000,
        exclusions: vec![],
    });
    let g = gen(&d, Cell { x: 0, y: 0 });
    assert!(!g.chunk.objects.is_empty());
    assert!(g
        .chunk
        .objects
        .iter()
        .all(|p| !d.buildings[0].contains([p.position[0], p.position[2]])));
}
#[test]
fn signed_coordinates_and_many_disjoint_courtyards_are_stable() {
    let mut d = doc();
    d.buildings[0].holes.clear();
    for y in 0..4 {
        for x in 0..4 {
            let a = 4000 + x * 4000;
            let b = 4000 + y * 4000;
            d.buildings[0].holes.push(vec![
                [a, b],
                [a + 1000, b],
                [a + 1000, b + 1000],
                [a, b + 1000],
            ]);
        }
    }
    d.validate().unwrap();
    for a in 0..2 {
        d.bounds.min[a] -= 25600;
        d.bounds.max[a] -= 25600;
        for p in d.buildings[0].footprint.iter_mut() {
            p[a] -= 25600;
        }
        for h in &mut d.buildings[0].holes {
            for p in h {
                p[a] -= 25600;
            }
        }
    }
    d.validate().unwrap();
    let mut all = vec![];
    for y in 0..2 {
        for x in 0..2 {
            all.extend(gen(&d, Cell { x, y }).chunk.building_prisms);
        }
    }
    for y in (-23000..-3000).step_by(701) {
        for x in (-23000..-3000).step_by(673) {
            let point = [x, y];
            assert_eq!(
                all.iter().any(|p| point_in_polygon(point, &p.footprint)),
                d.buildings[0].contains(point),
                "coverage {point:?}"
            );
        }
    }
}

#[test]
fn cumulative_courtyard_work_is_bounded_before_generation() {
    let mut d = doc();
    let b = d.buildings[0].clone();
    d.buildings = (0..10000)
        .map(|i| {
            let mut b = b.clone();
            b.id = format!("b{i}");
            b
        })
        .collect();
    assert_eq!(d.validate().unwrap_err().code, "E_BUDGET");
}
