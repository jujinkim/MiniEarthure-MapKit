use mapkit_core::*;
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
#[test]
fn overview_preserves_selection_geometry_without_editor_metadata() {
    let mut d = document();
    let original = overview(&d).unwrap().to_json(u64::MAX).unwrap();
    d.provenance.fingerprint = "changed".into();
    d.attributions[0].notice = "large preserved license notice".repeat(10000);
    let view = overview(&d).unwrap();
    assert_eq!(original, view.to_json(u64::MAX).unwrap());
    assert_eq!(
        view.roads
            .iter()
            .find(|r| r.id == "ground-road")
            .unwrap()
            .points,
        d.roads[0].points
    );
    assert_eq!(view.buildings[0].footprint, d.buildings[0].footprint);
    assert_eq!(view.attributions[0].license, d.attributions[0].license);
    assert!(original.len() * 10 < serde_json::to_vec(&d).unwrap().len());
    assert!(
        !original.contains("fingerprint")
            && !original.contains("notice")
            && !original.contains("heightmaps")
    );
    d.roads.reverse();
    d.buildings.reverse();
    d.attributions.reverse();
    assert_eq!(original, overview(&d).unwrap().to_json(u64::MAX).unwrap());
}
#[test]
fn overview_cost_counts_escaped_bytes_and_enforces_boundary() {
    let mut d = document();
    d.map_id = "지도 \"\n".into();
    let view = overview(&d).unwrap();
    let cost = view.cost().unwrap();
    let json = view.to_json(cost.json_bytes).unwrap();
    assert_eq!(json.len() as u64, cost.json_bytes);
    assert_eq!(
        view.to_json(cost.json_bytes - 1).unwrap_err().code,
        "E_MEMORY_BUDGET"
    );
    assert_eq!(
        cost.points_3d,
        d.roads.iter().map(|r| r.points.len() as u64).sum::<u64>()
    );
    assert_eq!(
        cost.points_2d,
        d.buildings
            .iter()
            .map(|b| b.footprint.len() as u64)
            .sum::<u64>()
    );
}

#[test]
fn overview_reports_custom_assets_without_copying_asset_metadata() {
    let mut d = document();
    assert!(!overview(&d).unwrap().has_custom_assets);
    d.assets.push(Asset {
        id: "custom".into(),
        path: "custom.glb".into(),
        attribution: Attribution {
            source: "Synthetic".into(),
            license: "MIT".into(),
            notice: "preserved in package".into(),
        },
        collision: vec![],
    });
    let json = overview(&d).unwrap().to_json(u64::MAX).unwrap();
    assert!(overview(&d).unwrap().has_custom_assets);
    assert!(!json.contains("custom.glb") && !json.contains("preserved in package"));
}
