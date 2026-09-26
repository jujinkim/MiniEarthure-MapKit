use mapkit_core::*;
fn templates() -> std::collections::BTreeMap<String, gimmick::Gimmick> {
    serde_json::from_str(include_str!("../../../examples/driving-library/library.json")).unwrap()
}
#[test]
fn declarative_motion_bounds_proxies_cost_and_archive() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    d.bounds = Bounds { min: [0,0], max: [25600,25600] };
    d.gimmicks = templates().into_values().collect();
    for g in &d.gimmicks { assert!(g.valid(), "{}",g.id); }
    let g = d.gimmicks.iter_mut().find(|g| g.id=="launch").unwrap();
    g.safety_min_cm[0]=g.position[0]; assert!(!g.valid());
    d.gimmicks=templates().into_values().collect();
    gimmick::validate(&d).unwrap();
    let mut duplicate=d.clone();duplicate.gimmicks.push(d.gimmicks[0].clone());assert!(gimmick::validate(&duplicate).is_err());
    let cell=Cell{x:0,y:0};
    let all=d.gimmicks.clone();
    let chunk=GeneratedChunk{water_bodies: vec![], gimmicks:all,asset_convexes:vec![],building_prisms:vec![],format_version:1,cell,triangles:vec![],objects:vec![]};
    assert_eq!(chunk.hash().unwrap(),sha256(&canonical(&chunk).unwrap()));
    let mut cost=estimate_generation(&d,cell,500_000).unwrap();
    cost.gimmick_bytes=chunk.gimmicks.iter().map(|g|g.memory_bytes()).sum();
    let key=archive_key("synthetic",cell);
    let encoded=encode_archive(&chunk,&key,archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&encoded,&key,cell,&cost).unwrap(),chunk);
    let mut corrupt=encoded.clone();*corrupt.last_mut().unwrap()=0;assert!(decode_archive(&corrupt,&key,cell,&cost).is_err());
    cost.gimmick_bytes=0;assert!(decode_archive(&encoded,&key,cell,&cost).is_err());
}
#[test]
fn hollow_structures_have_an_open_center_and_compound_parts() {
    for id in ["pipe","log","halfpipe"] {
        let g=templates().remove(id).unwrap();
        assert!(g.parts.len()>=6);
        for p in g.parts {
            let probe=[0,150,0];
            assert!(p.planes().any(|(n,a)| (0..3).map(|i|n[i]*i128::from(probe[i]-a[i])).sum::<i128>()>0));
        }
    }
}

#[test]
fn driving_window_covers_sweep_and_landing_with_unchanged_topology() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    d.bounds = Bounds { min: [0,0], max: [25600,25600] };
    d.cell_size_cm = 1600;
    assert_eq!(d.driving_window([3200,3200]),d.window([3200,3200]));
    let mut g = templates().remove("launch").unwrap();
    g.safety_max_cm[0] = 9600;
    d.gimmicks = vec![g.clone()];
    let cells = d.driving_window([3200,3200]);
    assert!(cells.len() > 9);
    assert!(cells.contains(&d.cell_at([g.safety_max_cm[0],g.safety_max_cm[2]]).unwrap()));
    assert_eq!(cells.iter().map(|c|(c.x,c.y)).collect::<std::collections::HashSet<_>>().len(),cells.len());
}
