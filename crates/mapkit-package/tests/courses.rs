use mapkit_core::{course::*, *};
use mapkit_package::{indexed::*, *};
use std::{collections::BTreeMap, io::Cursor};

#[test]
fn courses_and_evidence_roundtrip_without_changing_driving_content() {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    let empty = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    let world = empty.inspection.world_content_hash;
    let mut files = BTreeMap::new();
    for i in 0..2 {
        let body = CourseBody { map_id:d.map_id.clone(), display_name:format!("Course {i}"),
            world_content_hash:world.clone(), mode:Mode::Circuit, start_mode:StartMode::Air,
            start_direction:[1,0], checkpoints:vec![
                Checkpoint {position_cm:[1000,5000,1000],radius_cm:1200,shape:CheckpointShape::Sphere,placement_mode:PlacementMode::Free,surface_id:String::new()},
                Checkpoint {position_cm:[2000,6000,2000],radius_cm:400,shape:CheckpointShape::Hemisphere,placement_mode:PlacementMode::Free,surface_id:String::new()}] };
        let mut course = Course::from_definition(body, &d.bounds).unwrap();
        if i == 1 {
            // MapKit treats consumer evidence as opaque, bounded, hash-checked bytes.
            let bytes = b"synthetic opaque data, not a player certificate".to_vec();
            let digest = sha256(&bytes);
            let reference = ValidationReference {path:format!("course-validation/{digest}.mevalidation"),
                sha256:digest,bytes:bytes.len() as u32,world_content_hash:world.clone(),geometry_hash:course.definition.geometry_hash().unwrap()};
            files.insert(reference.path.clone(), bytes);
            course.validation = Some(reference);
        }
        d.courses.push(course);
    }
    let bytes = pack_bytes(d.clone(), files.clone()).unwrap();
    let loaded = read_bytes(&bytes).unwrap();
    assert_eq!(loaded.document.courses.len(), 2);
    assert_eq!(loaded.inspection.world_content_hash, world);
    assert_ne!(loaded.inspection.package_sha256, empty.inspection.package_sha256);
    assert_eq!(pack_bytes(loaded.document, loaded.files).unwrap(), bytes);
    let regions = pack_source(d.clone(), files.clone(), 1).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(regions), 1024*1024*1024, None).unwrap();
    assert_eq!(reader.index().world.courses.len(), 2);
    reader.audit_summary(1024*1024*1024, &ReadEpoch::default().begin()).unwrap();
    d.terrain_base_cm += 100;
    assert_ne!(read_bytes(&pack_bytes(d.clone(), files.clone()).unwrap()).unwrap().inspection.world_content_hash, world);
    files.values_mut().next().unwrap().push(0);
    assert!(pack_bytes(d, files).is_err());
}

#[test]
fn geometry_excludes_name_and_reference_but_includes_height_shape_direction() {
    let bounds = Bounds {min:[-10000,-10000],max:[10000,10000]};
    let mut body = CourseBody {map_id:"map".into(),display_name:"Name".into(),world_content_hash:"a".repeat(64),
        mode:Mode::Sprint,start_mode:StartMode::Ground,start_direction:[1,0],checkpoints:vec![
            Checkpoint{position_cm:[0,0,0],radius_cm:1200,shape:CheckpointShape::Sphere,placement_mode:PlacementMode::Free,surface_id:"terrain".into()};2]};
    let first = body.geometry_hash().unwrap();
    body.display_name = "Renamed".into();
    assert_eq!(first, body.geometry_hash().unwrap());
    Course::from_definition(body.clone(), &bounds).unwrap(); // overlaps are authored deliberately
    body.checkpoints[1].position_cm[1] = 100;
    assert_ne!(first, body.geometry_hash().unwrap());
    body.checkpoints[1].position_cm[1] = 0;
    body.checkpoints[1].shape = CheckpointShape::Hemisphere;
    assert_ne!(first, body.geometry_hash().unwrap());
    body.start_direction = [0,0];
    assert!(Course::from_definition(body, &bounds).is_err());
}
