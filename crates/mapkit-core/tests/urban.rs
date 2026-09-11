use mapkit_core::*;
fn doc(height: i64) -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.recipe_version = 6;
    d.bounds.max = [3200, 3200];
    d.cell_size_cm = 1600;
    d.nodes = vec![
        RoadNode {
            id: "a".into(),
            position: [0, height, 800],
            level: 0,
        },
        RoadNode {
            id: "b".into(),
            position: [3200, height, 800],
            level: 0,
        },
    ];
    d.roads = vec![Road {
        id: "road".into(),
        from: "a".into(),
        to: "b".into(),
        points: vec![[0, height, 800], [3200, height, 800]],
        widths_cm: vec![400],
        surfaces: vec![Surface::Asphalt],
        kind: RoadKind::Elevated,
        clearance_cm: None,
        sidewalk_cm: Some(0),
        markings: None,
    }];
    d.buildings.clear();
    d.zones.clear();
    d
}
fn generate_cell(d: &MapDocument, x: i32) -> GeneratedChunk {
    generate(GenerationInput {
        document: d,
        cell: Cell { x, y: 0 },
        heightgrid: None,
        max_triangles: 200_000,
    })
    .unwrap()
}
fn sample(c: &GeneratedChunk, id: &str, p: Point) -> Result<Vertex> {
    c.spawn(&SpawnRequest {
        position_cm: p,
        surface_id: id.into(),
    })
}
#[test]
fn exact_overlap_removed_but_one_centimetre_cover_and_clearance_survive() {
    for height in [-100, -1, 0, 1, 100] {
        let d = doc(height);
        for x in [0, 1] {
            let c = generate_cell(&d, x);
            let p = [x as i64 * 1600 + 711, 803];
            assert_eq!(sample(&c, "road", p).unwrap()[1], height);
            assert_eq!(sample(&c, "terrain", p).is_ok(), height != 0);
            assert_eq!(sample(&c, "terrain", [p[0], 1101]).unwrap()[1], 0);
        }
        let mut old = d;
        old.recipe_version = 5;
        assert!(sample(&generate_cell(&old, 0), "terrain", [711, 803]).is_ok());
    }
}
#[test]
fn paving_partitions_ground_preserves_road_material_and_rejects_overlap() {
    let mut d = doc(-1);
    d.surface_areas.push(SurfaceArea {
        id: "soil".into(),
        polygon: vec![[0, 0], [1600, 0], [1600, 1600], [0, 1600]],
        surface: Surface::Dirt,
    });
    let c = generate_cell(&d, 0);
    assert_eq!(sample(&c, "soil", [711, 803]).unwrap()[1], 0);
    assert_eq!(sample(&c, "road", [711, 803]).unwrap()[1], -1);
    assert!(c
        .triangles
        .iter()
        .filter(|t| t.object_id == "soil")
        .all(|t| t.surface == Surface::Dirt));
    d.roads[0].kind = RoadKind::Ground;
    let c = generate_cell(&d, 0);
    assert!(sample(&c, "soil", [711, 803]).is_err());
    assert!(sample(&c, "road", [711, 803]).is_ok());
    let mut other = d.surface_areas[0].clone();
    other.id = "other".into();
    d.surface_areas.push(other);
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    d.surface_areas.pop();
    d.recipe_version = 5;
    assert_eq!(d.validate().unwrap_err().code, "E_VERSION");
}
#[test]
fn continuous_sidewalk_has_support_at_bend_and_around_prop() {
    let mut d = doc(0);
    d.roads[0].kind = RoadKind::Ground;
    d.roads[0].sidewalk_cm = Some(100);
    d.roads[0].points = vec![[0, 0, 800], [800, 0, 800], [800, 0, 3200]];
    d.roads[0].widths_cm = vec![400, 400];
    d.roads[0].surfaces = vec![Surface::Asphalt; 2];
    d.nodes[1].position = [800, 0, 3200];
    d.placements.push(Placement {
        id: "lamp".into(),
        asset_id: "builtin:streetlight".into(),
        position: [400, 12, 550],
        quarter_turns: 0,
    });
    let c = generate_cell(&d, 0);
    assert!(c.objects.iter().any(|o| o.id == "lamp"));
    // Exterior bend corner used to be absent because entire endpoint pieces were suppressed.
    for p in [[401, 551], [1051, 1201], [850, 750]] {
        assert_eq!(
            sample(&c, "road:sidewalk", p).unwrap_or_else(|e| panic!("{p:?}: {e}"))[1],
            12,
            "{p:?}"
        );
    }
    let cost = estimate_generation(&d, Cell { x: 0, y: 0 }, 200_000).unwrap();
    assert!(c.triangles.len() as u64 <= cost.triangles);
    d.roads[0].markings = Some(RoadMarkings {
        lanes: 2,
        center_line: true,
        edge_lines: true,
        crosswalk_start: true,
        crosswalk_end: true,
    });
    assert_eq!(
        generate_cell(&d, 0).hash().unwrap(),
        c.hash().unwrap(),
        "markings never change collision"
    );
    d.roads[0].markings.as_mut().unwrap().lanes = 0;
    assert!(d.validate().is_err());
}

#[test]
fn indexed_validation_retains_manual_overlap_rejection() {
    let source = include_str!("../../../examples/placement/document.json");
    let original: MapDocument = serde_json::from_str(source).unwrap();
    for case in 0..4 {
        let mut d = original.clone();
        d.recipe_version = 6;
        match case {
            0 => d.placements[0].position = [1900, 0, 2000],
            1 => {
                let mut p = d.placements[0].clone();
                p.id = "duplicate-proxy".into();
                d.placements.push(p);
            }
            2 => {
                let mut b = d.buildings[0].clone();
                b.id = "duplicate-building".into();
                d.buildings.push(b);
            }
            _ => d.placements[0].position = [8000, 0, 6000],
        }
        assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    }
    let mut d = original;
    d.recipe_version = 6;
    d.validate().unwrap();
}
#[test]
fn paving_and_sidewalks_share_the_sloping_terrain_plane() {
    let mut d = doc(0);
    d.roads[0].kind = RoadKind::Ground;
    d.roads[0].sidewalk_cm = Some(100);
    d.surface_areas.push(SurfaceArea {
        id: "paving".into(),
        polygon: vec![[3, 2], [3200, 15], [3200, 3200], [3, 3200]],
        surface: Surface::Concrete,
    });
    d.heightmaps.push(Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "slope.png".into(),
        spacing_cm: 400,
        offset_cm: 0,
        step_cm: 1,
        source_accuracy_cm: None,
    });
    let grid = HeightGrid {
        side: 5,
        heights_cm: (0..5)
            .flat_map(|y| (0..5).map(move |x| x * 40 + y * 20))
            .collect(),
    };
    let c = generate(GenerationInput {
        document: &d,
        cell: Cell { x: 0, y: 0 },
        heightgrid: Some(&grid),
        max_triangles: 200_000,
    })
    .unwrap();
    for (id, p, offset) in [
        ("paving", [1200, 1400], 0),
        ("road", [1200, 800], 0),
        ("road:sidewalk", [1200, 550], 12),
    ] {
        assert_eq!(
            sample(&c, id, p).unwrap()[1],
            p[0] / 10 + p[1] / 20 + offset
        );
    }
}
