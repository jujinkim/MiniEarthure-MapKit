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

const STREET_SPECIES: [&str; 5] = ["builtin:street_tree_ginkgo", "builtin:street_tree_zelkova", "builtin:street_tree_plane",
    "builtin:street_tree_cherry", "builtin:street_tree_pine"];
const BEDS: [&str; 4] = ["builtin:planter", "builtin:planter_square", "builtin:median_bed", "builtin:median_bed_pine"];

fn yawed(id: &str, asset: &str, x: i64, y: i64, degrees: f64) -> Placement {
    let turns = libm::round(degrees / 90.0);
    let offset = libm::round((degrees - turns * 90.0) * 1000.0) as i32;
    Placement { id: id.into(), asset_id: asset.into(), position: [x, 0, y], quarter_turns: (turns as i64).rem_euclid(4) as u8, yaw_offset_mdeg: offset }
}

/// A 540 cm road with 67 cm sidewalks through (1600, 1600) at `degrees`; returns
/// the document, the road's exact angle and its unit normal.
fn sidewalk_road(degrees: f64) -> (MapDocument, f64, [f64; 2]) {
    let (s, c) = libm::sincos(degrees.to_radians());
    let a = [libm::round(1600.0 - 1500.0 * c) as i64, libm::round(1600.0 - 1500.0 * s) as i64];
    let b = [libm::round(1600.0 + 1500.0 * c) as i64, libm::round(1600.0 + 1500.0 * s) as i64];
    let mut d = doc();
    d.nodes = vec![RoadNode { id: "a".into(), position: [a[0], 0, a[1]], level: 0 }, RoadNode { id: "b".into(), position: [b[0], 0, b[1]], level: 0 }];
    d.roads = vec![Road { design: None, snow_retention_percent: 100, id: "avenue".into(), from: "a".into(), to: "b".into(),
        points: vec![[a[0], 0, a[1]], [b[0], 0, b[1]]], widths_cm: vec![540], surfaces: vec![Surface::Asphalt], kind: RoadKind::Ground,
        clearance_cm: None, sidewalk_cm: Some(67), markings: None }];
    let angle = libm::atan2((b[1] - a[1]) as f64, (b[0] - a[0]) as f64);
    (d, angle.to_degrees(), [-libm::sin(angle), libm::cos(angle)])
}

fn beside(d: &MapDocument, asset: &str, off_edge: f64, degrees: f64, normal: [f64; 2]) -> MapDocument {
    let mut d = d.clone();
    let reach = 270.0 + off_edge;
    d.placements = vec![yawed("pit", asset, libm::round(1600.0 + normal[0] * reach) as i64, libm::round(1600.0 + normal[1] * reach) as i64, degrees)];
    d
}

#[test]
fn street_planting_builtins_have_contract_constants_and_solid_parts() {
    assert_eq!((STREET_TREE_PIT_CM, STREET_CANOPY_MIN_CM), (40, 120));
    assert_eq!(STREET_TREES.map(|(id, _)| id), STREET_SPECIES);
    for (id, reach) in [("builtin:street_tree_ginkgo", 60), ("builtin:street_tree_zelkova", 125), ("builtin:street_tree_plane", 115),
        ("builtin:street_tree_cherry", 120), ("builtin:street_tree_pine", 85)] {
        assert_eq!(street_tree_crown_cm(id), Some(reach), "{id}");
        assert!(distant_tree(id), "{id}");
    }
    assert!(distant_tree("builtin:tree") && distant_tree("builtin:city_tree"));
    for id in BEDS.iter().chain(["builtin:shrub", "builtin:sign_stop"].iter()) {
        assert_eq!(street_tree_crown_cm(id), None, "{id}");
        assert!(!distant_tree(id), "{id}: beds keep their distant proxy faces");
    }
    let mut d = doc();
    d.placements = STREET_SPECIES.iter().chain(BEDS.iter()).enumerate()
        .map(|(i, asset)| place(&format!("piece-{i}"), asset, 400 + (i as i64 % 3) * 1000, 400 + (i as i64 / 3) * 900, 0))
        .collect();
    d.validate().unwrap();
    let chunk = generate(GenerationInput { document: &d, cell: Cell { x: 0, y: 0 }, heightgrid: None, max_triangles: 200_000 }).unwrap();
    assert_eq!(chunk.objects.len(), 9, "every piece is a rendered object");
    // (half x, half z, top) of the solid faces around the placement origin.
    let expected = |asset: &str| match asset {
        "builtin:planter" => (40, 15, 14),
        "builtin:planter_square" => (30, 30, 18),
        "builtin:median_bed" => (150, 40, 8),
        "builtin:median_bed_pine" => (150, 40, 140),
        _ => (7, 7, 140),
    };
    for p in &d.placements {
        let faces: Vec<_> = chunk.triangles.iter().filter(|t| t.object_id == p.id).flat_map(|t| t.vertices).collect();
        assert!(!faces.is_empty(), "{} is solid", p.asset_id);
        let (hx, hz, top) = expected(&p.asset_id);
        assert_eq!(faces.iter().map(|v| (v[0] - p.position[0]).abs()).max(), Some(hx), "{}", p.asset_id);
        assert_eq!(faces.iter().map(|v| (v[2] - p.position[2]).abs()).max(), Some(hz), "{}", p.asset_id);
        assert_eq!(faces.iter().map(|v| v[1]).max(), Some(top), "{} top", p.asset_id);
    }
    let cost = PreparedMap::new(d.clone()).unwrap().estimate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    assert!(cost.triangles >= chunk.triangles.len() as u64 && cost.objects >= chunk.objects.len() as u64);
    // Street trees reserve only their 40 cm pit: pits 41 cm apart fit, 39 cm apart overlap.
    let mut row = doc();
    row.placements = vec![place("a", "builtin:street_tree_zelkova", 800, 800, 0), place("b", "builtin:street_tree_cherry", 841, 800, 0)];
    row.validate().unwrap();
    row.placements[1].position[0] = 839;
    assert!(row.validate().is_err(), "pits overlap");
    // The median pine's trunk and the bed share one record; a bed footprint is its box.
    let mut beds = doc();
    beds.placements = vec![place("bed", "builtin:median_bed", 800, 800, 0), place("next", "builtin:median_bed_pine", 1101, 800, 0)];
    beds.validate().unwrap();
    beds.placements[1].position[0] = 1099;
    assert!(beds.validate().is_err(), "300 cm beds 299 cm apart overlap");
}

#[test]
fn street_tree_pits_turn_with_the_road_beside_the_curb() {
    for degrees in [0.0, 30.0, 45.0] {
        let (road, angle, normal) = sidewalk_road(degrees);
        road.validate().unwrap();
        for asset in STREET_SPECIES {
            beside(&road, asset, 28.0, angle, normal).validate().unwrap_or_else(|e| panic!("{asset} at {degrees}°: {e:?}"));
        }
        let error = beside(&road, "builtin:street_tree_ginkgo", 15.0, angle, normal).validate().unwrap_err();
        assert!(error.message.contains("intersects road"), "{degrees}°: {error:?}");
    }
    // An axis-aligned 40 cm square reaches 28.3 cm toward a 45° curb: only the
    // pit turned with the road fits 28 cm off the edge.
    let (road, angle, normal) = sidewalk_road(45.0);
    assert!(beside(&road, "builtin:street_tree_plane", 28.0, 0.0, normal).validate().unwrap_err().message.contains("intersects road"));
    let tree = beside(&road, "builtin:street_tree_plane", 28.0, angle, normal);
    // The yawed trunk is a convex proxy; distant sources omit it in the anchor cell.
    let prepared = PreparedMap::new(tree.clone()).unwrap();
    let cell = Cell { x: 0, y: 0 };
    let chunk = prepared.generate(cell, None, 500_000).unwrap();
    assert!(chunk.objects.iter().any(|o| o.id == "pit" && o.asset_id == "builtin:street_tree_plane"));
    assert!(chunk.asset_convexes.iter().any(|c| c.object_id == "pit"), "a yawed trunk convex");
    assert!(chunk.triangles.iter().any(|t| t.object_id == "pit"));
    let distant = prepared.generate_distant_source(cell, None, 500_000).unwrap();
    assert!(!distant.triangles.iter().any(|t| t.object_id == "pit"), "the far tree replaces the trunk faces");
    let mut bed = tree.clone();
    bed.placements[0].asset_id = "builtin:median_bed_pine".into();
    bed.placements[0].position = [1600 - 400, 0, 1600 + 400];
    bed.validate().unwrap();
    let distant = PreparedMap::new(bed).unwrap().generate_distant_source(cell, None, 500_000).unwrap();
    assert!(distant.triangles.iter().any(|t| t.object_id == "pit"), "median beds keep their distant proxy faces");
}

#[test]
fn street_species_are_not_zone_trees() {
    for asset in STREET_SPECIES {
        let mut d = doc();
        d.zones = vec![Zone { id: "wood".into(), polygon: vec![[400, 400], [2800, 400], [2800, 2800], [400, 2800]], kind: ZoneKind::Forest,
            spacing_cm: 300, density_per_mille: 1000, exclusions: vec![],
            tree: Some(ZoneTree { asset_id: asset.into(), radius_cm: 120, clearance_cm: 30 }) }];
        assert!(d.validate().is_err(), "{asset} as a zone tree");
    }
}
