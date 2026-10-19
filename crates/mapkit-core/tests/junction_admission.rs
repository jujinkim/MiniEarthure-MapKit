use mapkit_core::{unbuildable_junctions, MapDocument};

/// Two ground roads at `n0`. With `hairpin`, the second leaves in the direction
/// the first arrived from (a source way doubling back on itself), which no single
/// junction passage can serve; otherwise it continues straight on.
fn junction(hairpin: bool) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.bounds.min = [-10_000, -10_000];
    d.bounds.max = [10_000, 10_000];
    let out: [i64; 3] = if hairpin { [1139, 0, -286] } else { [-1139, 0, 286] };
    let nodes = serde_json::json!([{"id":"n0","position":[0,0,0],"level":0}, {"id":"a","position":[600,0,-150],"level":0}, {"id":"b","position":out,"level":0}]);
    let road = |id: &str, from: &str, to: &str, points: Vec<[i64; 3]>, width: u32| serde_json::json!({"id":id,"from":from,"to":to,"points":points,
        "widths_cm":vec![width; points.len()-1],"surfaces":vec!["asphalt"; points.len()-1],"kind":"ground","clearance_cm":null,"sidewalk_cm":null});
    d.nodes = serde_json::from_value(nodes).unwrap();
    d.roads = serde_json::from_value(serde_json::json!([road("in","a","n0",vec![[600,0,-150],[11,0,-3],[0,0,0]],20), road("out","n0","b",vec![[0,0,0],out],38)])).unwrap();
    d
}

#[test]
fn reports_only_junctions_the_planner_cannot_build() {
    assert!(unbuildable_junctions(&junction(false)).unwrap().is_empty());
    assert_eq!(unbuildable_junctions(&junction(true)).unwrap(), vec!["n0".to_string()], "a way doubling back");
    // Separating the node into independent road ends leaves nothing to report.
    let mut separated = junction(true);
    separated.roads[0].to = "n0-arm-0".into();
    separated.roads[1].from = "n0-arm-1".into();
    for i in 0..2 { separated.nodes.push(serde_json::from_value(serde_json::json!({"id":format!("n0-arm-{i}"),"position":[0,0,0],"level":0})).unwrap()); }
    separated.nodes.retain(|n| n.id != "n0");
    assert!(unbuildable_junctions(&separated).unwrap().is_empty());
}
