use mapkit_core::*;
#[test]
fn snow_and_contact_identity_survive_generation_archive_and_hash() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    assert!(d.roads.iter().all(|r| r.snow_retention_percent == 100));
    let original = canonical(&d).unwrap();
    for r in &mut d.roads { r.snow_retention_percent = 15; }
    assert_ne!(original, canonical(&d).unwrap());
    let restored: MapDocument = serde_json::from_slice(&canonical(&d).unwrap()).unwrap();
    assert_eq!(d, restored);
    let cell = Cell {x: 0, y: 0};
    let c = generate(GenerationInput {document: &d, cell, heightgrid: None, max_triangles: 200_000}).unwrap();
    for face in &c.triangles {
        if face.object_id == "terrain" { assert_eq!((face.contact_class, face.snow_retention_percent), (1,100)); }
        if face.spawnable {
            if let Some(r) = d.roads.iter().find(|r| r.id == face.object_id) {
                assert_eq!(face.snow_retention_percent, 15);
                assert_eq!(face.contact_class, if r.kind == RoadKind::Ground {2} else {3});
            }
        } else { assert_eq!((face.contact_class, face.snow_retention_percent), (0,100)); }
    }
    let cost = estimate_generation(&d, cell, 200_000).unwrap();
    let key = archive_key("contact-test", cell);
    let bytes = encode_archive(&c, &key, archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&bytes, &key, cell, &cost).unwrap(), c);
    let mut changed = c.clone(); changed.triangles[0].snow_retention_percent = 33;
    assert_ne!(c.hash().unwrap(), changed.hash().unwrap());
    changed = c.clone(); changed.triangles[0].contact_class = 3;
    assert_ne!(c.hash().unwrap(), changed.hash().unwrap());
    d.roads[0].snow_retention_percent = 101;
    assert!(d.validate().is_err());
}
