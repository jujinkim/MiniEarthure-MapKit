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
