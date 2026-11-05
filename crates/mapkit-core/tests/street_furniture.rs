use mapkit_core::*;

fn doc() -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.bounds.max = [3200, 3200];
    d.cell_size_cm = 3200;
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    d.placements.clear();
    d.repetitions.clear();
    d
}

fn place(id: &str, asset: &str, x: i64, y: i64, turns: u8) -> Placement {
    Placement { id: id.into(), asset_id: asset.into(), position: [x, 0, y], quarter_turns: turns, yaw_offset_mdeg: 0 }
}

#[test]
fn city_street_furniture_is_builtin_with_solid_and_display_only_parts() {
    let mut d = doc();
    d.placements = vec![
        place("signal-h", "builtin:signal_h", 400, 400, 0),
        place("signal-v", "builtin:signal_v", 900, 400, 1),
        place("shelter", "builtin:bus_shelter", 1600, 400, 0),
        place("entrance", "builtin:subway_entrance", 2400, 600, 0),
        place("sign", "builtin:station_sign", 2900, 400, 0),
        place("bench", "builtin:bench", 400, 1600, 0),
        place("shrub", "builtin:shrub", 900, 1600, 0),
    ];
    d.traffic = Some(Traffic { driving_side: DrivingSide::Left, signal_style: SignalStyle::Vertical });
    d.validate().unwrap();
    let chunk = generate(GenerationInput { document: &d, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    let ids: std::collections::BTreeSet<_> = chunk.objects.iter().map(|o| o.asset_id.as_str()).collect();
    assert_eq!(ids.len(), 7, "every placement is a rendered object");
    for solid in ["signal-h", "signal-v", "shelter", "entrance", "sign"] {
        assert!(chunk.triangles.iter().any(|t| t.object_id == solid), "{solid} has collision faces");
    }
    for detail in ["bench", "shrub"] {
        assert!(!chunk.triangles.iter().any(|t| t.object_id == detail), "{detail} is display only");
    }
    let cost = PreparedMap::new(d.clone()).unwrap().estimate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    assert!(cost.triangles >= chunk.triangles.len() as u64 && cost.objects >= chunk.objects.len() as u64);
    // Display-only details still reserve a footprint: overlapping ones refuse.
    let mut crowded = d.clone();
    crowded.placements.push(place("bench-2", "builtin:bench", 420, 1600, 0));
    assert!(crowded.validate().is_err(), "benches 20 cm apart overlap");
    let text = String::from_utf8(canonical(&d).unwrap()).unwrap();
    assert!(text.contains("\"traffic\":{\"driving_side\":\"left\",\"signal_style\":\"vertical\"}"));
    let mut plain = d.clone();
    plain.traffic = None;
    assert!(!String::from_utf8(canonical(&plain).unwrap()).unwrap().contains("traffic"), "absent convention keeps bytes");
}

#[test]
fn city_trees_have_a_proportionate_trunk_footprint_and_zone_use() {
    let mut d = doc();
    d.placements = vec![place("tree", "builtin:city_tree", 800, 800, 0)];
    d.validate().unwrap();
    let chunk = generate(GenerationInput { document: &d, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    let trunk: Vec<_> = chunk.triangles.iter().filter(|t| t.object_id == "tree").collect();
    assert!(!trunk.is_empty(), "a solid trunk");
    assert_eq!(trunk.iter().flat_map(|t| t.vertices).map(|v| v[1]).max(), Some(140), "140 cm trunk");
    assert_eq!(trunk.iter().flat_map(|t| t.vertices).map(|v| v[0]).max(), Some(807), "14 cm wide");
    let mut crowded = d.clone();
    crowded.placements.push(place("tree-2", "builtin:city_tree", 1030, 800, 0));
    assert!(crowded.validate().is_err(), "120 cm footprints 230 cm apart overlap");
    crowded.placements[1].position[0] = 1050;
    crowded.validate().unwrap();
    // A forest zone may grow city trees within a footprint of at least 120 cm.
    let mut forest = doc();
    forest.zones = vec![Zone { id: "wood".into(), polygon: vec![[400, 400], [2800, 400], [2800, 2800], [400, 2800]], kind: ZoneKind::Forest,
        spacing_cm: 300, density_per_mille: 1000, exclusions: vec![],
        tree: Some(ZoneTree { asset_id: "builtin:city_tree".into(), radius_cm: 120, clearance_cm: 30 }) }];
    forest.validate().unwrap();
    let chunk = generate(GenerationInput { document: &forest, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    assert!(!chunk.objects.is_empty() && chunk.objects.iter().all(|o| o.asset_id == "builtin:city_tree"));
    let cost = PreparedMap::new(forest.clone()).unwrap().estimate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    assert!(cost.triangles >= chunk.triangles.len() as u64 && cost.objects >= chunk.objects.len() as u64);
    forest.zones[0].tree.as_mut().unwrap().radius_cm = 100;
    assert!(forest.validate().is_err(), "a smaller footprint than the city tree is refused");
}

#[test]
fn city_tree_radius_follows_the_zone_that_grew_it() {
    let mut d = doc();
    d.placements = vec![place("tree", "builtin:city_tree", 800, 800, 0)];
    d.zones = vec![Zone { id: "park:west".into(), polygon: vec![[1200, 200], [3000, 200], [3000, 3000], [1200, 3000]], kind: ZoneKind::Forest,
        spacing_cm: 500, density_per_mille: 1000, exclusions: vec![],
        tree: Some(ZoneTree { asset_id: "builtin:city_tree".into(), radius_cm: 200, clearance_cm: 30 }) }];
    d.placements[0].position = [500, 0, 500];
    d.validate().unwrap();
    let radii = mapkit_core::city_tree_zone_radii(&d);
    assert_eq!(radii.len(), 1);
    assert_eq!(mapkit_core::generated_zone_id("park:west:3:-4"), Some("park:west"), "zone ids may hold colons");
    assert_eq!(mapkit_core::city_tree_radius(&radii, "park:west:3:-4"), 200);
    for street in ["tree", "park:west:03:4", "park:west:x:4", "park:east:3:4", "3:4"] {
        assert_eq!(mapkit_core::city_tree_radius(&radii, street), mapkit_core::CITY_TREE_RADIUS, "{street}");
    }
    let chunk = generate(GenerationInput { document: &d, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    let trees: Vec<_> = chunk.objects.iter().filter(|o| o.asset_id == "builtin:city_tree").collect();
    assert!(trees.len() > 2, "the zone grows trees: {}", trees.len());
    for tree in trees {
        let expected = if tree.id == "tree" { 120 } else { 200 };
        assert_eq!(mapkit_core::city_tree_radius(&radii, &tree.id), expected, "{}", tree.id);
    }
    // An authored identity of the generated form is refused, so it never
    // borrows a zone radius.
    d.placements[0].id = "park:west:9:9".into();
    assert!(d.validate().is_err());
}

#[test]
fn road_signs_are_solid_posts_with_display_only_plates_and_boards() {
    let poles = ["builtin:sign_speed_30", "builtin:sign_speed_40", "builtin:sign_speed_50", "builtin:sign_speed_60",
        "builtin:sign_speed_80", "builtin:sign_speed_100", "builtin:sign_crossing", "builtin:sign_stop", "builtin:sign_yield",
        "builtin:sign_children"];
    let mut d = doc();
    d.placements = poles.iter().enumerate()
        .map(|(i, asset)| place(&format!("sign-{i}"), asset, 300 + i as i64 * 250, 400, (i % 4) as u8))
        .collect();
    d.placements.push(place("guide", "builtin:guide_sign", 1600, 1600, 1));
    d.validate().unwrap();
    let chunk = generate(GenerationInput { document: &d, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    assert_eq!(chunk.objects.len(), 11, "every sign is a rendered object");
    for (i, p) in d.placements.iter().enumerate() {
        let faces: Vec<_> = chunk.triangles.iter().filter(|t| t.object_id == p.id).flat_map(|t| t.vertices).collect();
        assert!(!faces.is_empty(), "{} has collision faces", p.asset_id);
        let (half, top) = if i < poles.len() { (3, 104) } else { (5, 250) };
        assert!(faces.iter().all(|v| (v[0] - p.position[0]).abs() <= half && (v[2] - p.position[2]).abs() <= half),
            "{}: only the post collides", p.asset_id);
        assert_eq!(faces.iter().map(|v| v[1]).max(), Some(top), "{} top", p.asset_id);
    }
    let cost = PreparedMap::new(d.clone()).unwrap().estimate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    assert!(cost.triangles >= chunk.triangles.len() as u64 && cost.objects >= chunk.objects.len() as u64);
    let mut crowded = d.clone();
    crowded.placements.push(place("sign-x", "builtin:sign_stop", 304, 400, 0));
    assert!(crowded.validate().is_err(), "two posts 4 cm apart overlap");

    // Only the guide pole reserves ground: it may stand just beside a road
    // corridor while the display-only arm overhangs the carriageway.
    let mut road = doc();
    road.nodes = vec![RoadNode { id: "w".into(), position: [0, 0, 1600], level: 0 }, RoadNode { id: "e".into(), position: [3200, 0, 1600], level: 0 }];
    road.roads = vec![Road { design: None, snow_retention_percent: 100, id: "street".into(), from: "w".into(), to: "e".into(),
        points: vec![[0, 0, 1600], [3200, 0, 1600]], widths_cm: vec![400], surfaces: vec![Surface::Asphalt], kind: RoadKind::Ground,
        clearance_cm: None, sidewalk_cm: Some(0), markings: None }];
    road.placements = vec![place("guide", "builtin:guide_sign", 1600, 1820, 1)];
    road.validate().unwrap();
    road.placements[0].position[2] = 1700;
    assert!(road.validate().is_err(), "a pole on the carriageway is refused");
}
