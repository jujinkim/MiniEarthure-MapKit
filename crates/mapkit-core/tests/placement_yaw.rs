use mapkit_core::*;

#[test]
fn yaw_shares_exact_collision_occupancy_bounds_and_archive() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.heightmaps.clear();
    d.placements = serde_json::from_value(serde_json::json!([
        {"id":"rotated","asset_id":"builtin:fence","position":[3200,0,3200],"quarter_turns":1,"yaw_offset_mdeg":37000}
    ])).unwrap();
    d.validate().unwrap();
    let cell=d.cell_at([3200,3200]).unwrap();
    let generated=generate_with_occupancy(GenerationInput {document:&d,cell,heightgrid:None,max_triangles:500_000},20_000).unwrap();
    let shape=&generated.chunk.asset_convexes[0].shape;
    assert!(shape.valid(100_000_000));
    assert_eq!(generated.chunk.objects[0].yaw_offset_mdeg,37000);
    assert!(generated.solids.iter().any(|s| matches!(&s.shape,SolidShape::Convex(c) if c == shape)));
    let cost=estimate_generation(&d,cell,500_000).unwrap();
    assert!(cost.asset_convexes>=generated.chunk.asset_convexes.len() as u64);
    let key=archive_key(&"b".repeat(64),cell);
    let bytes=encode_archive(&generated.chunk,&key,archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&bytes,&key,cell,&cost).unwrap(),generated.chunk);
    let original=sha256(&canonical(&d).unwrap());
    d.placements[0].yaw_offset_mdeg=0;
    assert_ne!(sha256(&canonical(&d).unwrap()),original);
    d.placements[0].yaw_offset_mdeg=360001;
    assert!(d.validate().is_err());
}

#[test]
fn additive_yaw_uses_source_axes_and_preserves_cardinal_defaults() {
    let p: Placement=serde_json::from_value(serde_json::json!({"id":"a","asset_id":"builtin:fence","position":[100,20,300],"quarter_turns":1})).unwrap();
    assert_eq!(p.transform_point([1000,0,0]),[100,20,1300]);
    assert!(!String::from_utf8(canonical(&p).unwrap()).unwrap().contains("yaw_offset"));
    let mut p=p; p.yaw_offset_mdeg=90000;
    assert_eq!(p.transform_point([1000,0,0]),[-900,20,300]);
}
