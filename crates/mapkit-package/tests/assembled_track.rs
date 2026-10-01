use mapkit_core::assembled_track::*;
use mapkit_package::{assembled_track as package, *};
use std::collections::BTreeMap;

#[test]
fn generated_walls_have_no_reverse_coplanar_duplicates() {
    let d = document(&Settings {
        seed: 1,
        circuit: false,
        duration_seconds: 60,
        categories: vec!["driving".into()],
        ..Settings::default()
    })
    .unwrap();
    let assembly = d.assembled_track.as_ref().unwrap();
    let mut checked = 0;
    for piece in [
        assembly.pieces.first().unwrap(),
        assembly.pieces.last().unwrap(),
    ] {
        let p = piece.origin_cm;
        let cell = d.cell_at([p[0], p[2]]).unwrap();
        let chunk = mapkit_core::generate(mapkit_core::GenerationInput {
            document: &d,
            cell,
            heightgrid: None,
            max_triangles: 500_000,
        })
        .unwrap();
        let mut planes = BTreeMap::new();
        for face in chunk
            .triangles
            .iter()
            .filter(|f| f.object_id.starts_with("assembled-wall-"))
        {
            let [a, b, c] = face.vertices;
            let ab = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
            let ac = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
            let normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            for axis in [0, 2] {
                if a[axis] != b[axis] || a[axis] != c[axis] || normal[axis] == 0 {
                    continue;
                }
                let key = (face.object_id.clone(), axis, a[axis]);
                let sign = normal[axis].signum();
                let entry: &mut Vec<(i64,[[i64;3];3])> = planes.entry(key).or_default();
                for (previous,vertices) in entry.iter() {
                    if *previous!=sign {
                        assert!(coplanar_overlap_area(*vertices,face.vertices,axis)<0.001,
                            "opposite wall faces overlap: {} {vertices:?} {:?}",face.object_id,face.vertices);
                    }
                }
                entry.push((sign,face.vertices));
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 8,
        "straight and finish boundary fixtures include wall faces"
    );
}

// Coplanarity alone is insufficient: a notched finish wall can have disjoint
// faces facing opposite directions on the same plane. Reject shared area only.
fn coplanar_overlap_area(a: [[i64;3];3],b: [[i64;3];3],axis: usize) -> f64 {
    let project=|v: [i64;3]| if axis==0 {[v[1] as f64,v[2] as f64]} else {[v[0] as f64,v[1] as f64]};
    let a=a.map(project);let b=b.map(project);
    let side=|p: [f64;2],q: [f64;2],r: [f64;2]| (q[0]-p[0])*(r[1]-p[1])-(q[1]-p[1])*(r[0]-p[0]);
    let orientation=side(b[0],b[1],b[2]).signum();
    let mut polygon=a.to_vec();
    for i in 0..3 {
        if polygon.is_empty() {return 0.0;}
        let mut out=vec![];
        let mut previous=*polygon.last().unwrap();
        let mut before=side(b[i],b[(i+1)%3],previous)*orientation;
        for point in polygon {
            let after=side(b[i],b[(i+1)%3],point)*orientation;
            if (before>=0.0)!=(after>=0.0) {
                let t=before/(before-after);
                out.push([previous[0]+(point[0]-previous[0])*t,previous[1]+(point[1]-previous[1])*t]);
            }
            if after>=0.0 {out.push(point);}
            previous=point;before=after;
        }
        polygon=out;
    }
    (0..polygon.len()).map(|i|{let p=polygon[i];let q=polygon[(i+1)%polygon.len()];p[0]*q[1]-p[1]*q[0]}).sum::<f64>().abs()/2.0
}

#[test]
fn wall_overlap_check_distinguishes_disjoint_faces_and_diagonals() {
    let a=[[0,0,0],[0,10,0],[0,10,10]];
    assert_eq!(coplanar_overlap_area(a,[[0,0,0],[0,10,10],[0,0,10]],0),0.0);
    assert!(coplanar_overlap_area(a,[[0,0,0],[0,0,10],[0,10,0]],0)>0.0);
    assert_eq!(coplanar_overlap_area(a,[[0,20,0],[0,30,0],[0,30,10]],0),0.0);
}

#[test]
fn reproducible_roundtrip_and_modified_source_rejected() {
    let settings = Settings::default();
    let d = package::generate(&settings).unwrap();
    let bytes = pack_bytes(d.clone(), BTreeMap::new()).unwrap();
    assert_eq!(
        bytes,
        pack_bytes(package::generate(&settings).unwrap(), BTreeMap::new()).unwrap()
    );
    let p = read_bytes(&bytes).unwrap();
    let mut support_edit = p.document.to_document();
    let a = support_edit.assembled_track.as_mut().unwrap();
    if let Some(support) = a.supports.first_mut() {
        support.shape.vertices[0][0] += 1;
        assert!(support_edit.validate().is_err());
    }
    let mut floor_edit = p.document.to_document();
    floor_edit.assembled_track.as_mut().unwrap().floor.min_cm[1] -= 1;
    assert!(floor_edit.validate().is_err());
    assert_eq!(p.document.assembled_track, d.assembled_track);
    package::verify(
        &p.document,
        &p.inspection.world_content_hash,
        &p.document.courses[0],
    )
    .unwrap();
    let mut modified = p.document.to_document();
    modified.seed += 1;
    assert!(verify_document(&modified).is_err());
    let mut course = p.document.courses[0].definition.clone();
    course.checkpoints[1].radius_cm += 1;
    let course = mapkit_core::course::Course::from_definition(course, &p.document.bounds).unwrap();
    assert!(package::verify(&p.document, &p.inspection.world_content_hash, &course).is_err());
}
#[test]
fn cancellation_and_invalid_requests() {
    let token = mapkit_core::cancellation::CancellationToken::default();
    token.cancel();
    assert_eq!(
        token
            .run(|| assemble(&Settings::default()))
            .unwrap_err()
            .code,
        "E_CANCELLED"
    );
    let mut s = Settings::default();
    s.duration_seconds = 300;
    assert!(assemble(&s).is_err());
    let mut a = assemble(&Settings::default()).unwrap();
    a.pieces[0].path[1].position_cm[1] += 1;
    assert!(a.validate().is_err());
}

#[test]
fn save_failure_never_replaces_original() {
    let path = std::env::temp_dir().join(format!(
        "mapkit-track-preserve-{}-{}.memap",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"original user artifact").unwrap();
    assert!(package::save(&Settings::default(), &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original user artifact");
    let mut a = assemble(&Settings::default()).unwrap();
    a.pieces.extend(a.pieces.clone());
    assert!(a.validate().is_err());
}

#[test]
fn venue_floor_is_collision_only_and_budgeted() {
    let d = document(&Settings {
        categories: vec!["driving".into()],
        ..Settings::default()
    })
    .unwrap();
    let a = d.assembled_track.as_ref().unwrap();
    let generated = mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell { x: 0, y: 0 },
        heightgrid: None,
        max_triangles: 500_000,
    })
    .unwrap();
    let floor: Vec<_> = generated
        .triangles
        .iter()
        .filter(|f| f.object_id == "assembled-venue-floor")
        .collect();
    assert!(!floor.is_empty());
    assert!(floor
        .iter()
        .all(|f| !f.spawnable && f.vertices.iter().all(|v| v[1] == a.floor.min_cm[1])));
    let mut broken = a.clone();
    broken.pieces = vec![a.pieces[0].clone(); 513];
    assert!(broken.validate().is_err());
    assert!(mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell { x: 0, y: 0 },
        heightgrid: None,
        max_triangles: 1
    })
    .is_err());
}

#[test]
fn finish_plaza_and_editable_free_roam_keep_exact_source_validation() {
    let settings = Settings {
        circuit: false,
        duration_seconds: 60,
        categories: vec!["driving".into()],
        ..Settings::default()
    };
    let mut d = package::generate(&settings).unwrap();
    assert!(!d.free_roam);
    let a = d.assembled_track.as_ref().unwrap();
    let plaza = a.finish_plaza.as_ref().unwrap();
    assert_eq!(
        (plaza.entry_length_cm, plaza.radius_cm, plaza.wall_height_cm),
        (800, 800, 120)
    );
    assert_eq!(a.pieces.last().unwrap().id, "finish_plaza");
    assert_eq!(
        d.courses[0]
            .definition
            .checkpoints
            .last()
            .unwrap()
            .position_cm,
        plaza.checkpoint_cm
    );
    assert!(d.courses[0]
        .definition
        .checkpoints
        .iter()
        .all(|cp| cp.position_cm != plaza.center_cm));
    let p = &a.pieces[plaza.piece_index];
    assert!(
        p.path
            .iter()
            .any(|point| point.position_cm == plaza.checkpoint_cm),
        "finish must be an exact AI route sample"
    );
    assert_eq!(
        a.pieces[plaza.piece_index - 1]
            .path
            .last()
            .unwrap()
            .position_cm,
        p.path[0].position_cm
    );
    let prior_length: u64 = a.pieces[..plaza.piece_index]
        .iter()
        .map(|p| {
            p.path
                .windows(2)
                .map(|w| {
                    ((0..3)
                        .map(|i| ((w[1].position_cm[i] - w[0].position_cm[i]) as f64).powi(2))
                        .sum::<f64>()
                        .sqrt()
                        .round()) as u64
                })
                .sum::<u64>()
        })
        .sum();
    assert_eq!(a.length_cm, prior_length + 400);
    let center = plaza.center_cm;
    let chunk = mapkit_core::generate(mapkit_core::GenerationInput {
        document: &d,
        cell: mapkit_core::Cell {
            x: (center[0] - d.bounds.min[0]).div_euclid(d.cell_size_cm as i64) as i32,
            y: (center[2] - d.bounds.min[1]).div_euclid(d.cell_size_cm as i64) as i32,
        },
        heightgrid: None,
        max_triangles: 500_000,
    })
    .unwrap();
    let road = format!("assembled-road-{}", plaza.piece_index);
    let floor: Vec<_> = chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == road)
        .collect();
    assert!(floor.len() >= 60);
    assert!(floor
        .iter()
        .all(|t| t.spawnable && t.vertices.iter().all(|v| v[1] == center[1])));
    let wall = format!("assembled-wall-{}", plaza.piece_index);
    assert!(chunk
        .triangles
        .iter()
        .any(|t| t.object_id == wall && t.vertices.iter().any(|v| v[1] == center[1] + 120)));
    let old = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    d.free_roam = true;
    package::reseal(&mut d).unwrap();
    let loaded = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    assert!(loaded.inspection.free_roam);
    assert_ne!(
        old.inspection.world_content_hash,
        loaded.inspection.world_content_hash
    );
    package::verify(
        &loaded.document,
        &loaded.inspection.world_content_hash,
        &loaded.document.courses[0],
    )
    .unwrap();
    assert!(package::verify(
        &loaded.document,
        &loaded.inspection.world_content_hash,
        &old.document.courses[0]
    )
    .is_err());
    let bytes = indexed::pack_source(d.clone(), BTreeMap::new(), 1).unwrap();
    let mut reader =
        indexed::IndexedReader::open(std::io::Cursor::new(bytes), 1024 * 1024 * 1024, None)
            .unwrap();
    assert!(reader.index().world.free_roam);
    reader
        .audit_summary(1024 * 1024 * 1024, &indexed::ReadEpoch::default().begin())
        .unwrap();
    d.seed += 1;
    assert!(package::reseal(&mut d).is_err());
    assert!(document(&Settings {
        categories: vec!["driving".into()],
        ..Settings::default()
    })
    .unwrap()
    .assembled_track
    .unwrap()
    .finish_plaza
    .is_none());
}

#[test]
fn free_roam_is_required_boolean() {
    let d = package::generate(&Settings {
        categories: vec!["driving".into()],
        ..Settings::default()
    })
    .unwrap();
    let mut value = serde_json::to_value(&d).unwrap();
    value.as_object_mut().unwrap().remove("free_roam");
    assert!(serde_json::from_value::<mapkit_core::MapDocument>(value.clone()).is_err());
    value["free_roam"] = serde_json::json!(1);
    assert!(serde_json::from_value::<mapkit_core::MapDocument>(value).is_err());
}

#[test]
fn authored_source_roundtrip_draft_export_and_tampering() {
    let source = authoring::shortcut_source();
    let d = package::compile_source(&source).unwrap();
    let a = d.assembled_track.as_ref().unwrap();
    assert!(a.authoring.is_some());
    assert!(a.issues.is_empty());
    let bytes = pack_bytes(d.clone(), BTreeMap::new()).unwrap();
    let p = read_bytes(&bytes).unwrap();
    package::verify(
        &p.document,
        &p.inspection.world_content_hash,
        &p.document.courses[0],
    )
    .unwrap();
    assert_eq!(p.document.assembled_track, d.assembled_track);
    let mut action_tamper = d.clone();
    action_tamper
        .assembled_track
        .as_mut()
        .unwrap()
        .authoring
        .as_mut()
        .unwrap()
        .actions[0]
        .height_cm -= 50;
    assert!(
        action_tamper.validate().is_err(),
        "action source and derived trigger must match"
    );
    let mut corrupt = d.clone();
    corrupt.assembled_track.as_mut().unwrap().pieces[0].path[0].position_cm[0] += 1;
    assert!(pack_bytes(corrupt, BTreeMap::new()).is_err());
    let mut changed = source.clone();
    changed.instances[0].position_cm[0] += 15;
    let draft = package::compile_source(&changed).unwrap();
    assert!(!draft.assembled_track.as_ref().unwrap().issues.is_empty());
    draft.validate().unwrap();
    assert_eq!(
        pack_bytes(draft.clone(), BTreeMap::new()).unwrap_err().code,
        "E_TRACK_DRAFT"
    );
    assert_eq!(
        indexed::pack_source(draft, BTreeMap::new(), 1)
            .unwrap_err()
            .code,
        "E_TRACK_DRAFT"
    );
    let mut geometry_edit = d.clone();
    geometry_edit
        .assembled_track
        .as_mut()
        .unwrap()
        .authoring
        .as_mut()
        .unwrap()
        .instances[0]
        .position_cm[0] += 1;
    assert!(geometry_edit.validate().is_err());
}
#[test]
fn source_mode_and_course_progress_remain_distinct() {
    let seed = package::generate(&Settings::default()).unwrap();
    let source = authoring::from_assembly(seed.assembled_track.as_ref().unwrap());
    let manual = package::compile_source(&source).unwrap();
    assert!(manual.assembled_track.as_ref().unwrap().authoring.is_some());
    assert!(source.grounded_supports);
    assert_eq!(manual.assembled_track.as_ref().unwrap().floor, seed.assembled_track.as_ref().unwrap().floor);
    assert_eq!(manual.assembled_track.as_ref().unwrap().supports, seed.assembled_track.as_ref().unwrap().supports);
    assert_eq!(
        source.original_seed,
        Some(Settings::default().normalized().unwrap())
    );
    for (before, after) in seed
        .assembled_track
        .as_ref()
        .unwrap()
        .pieces
        .iter()
        .zip(&manual.assembled_track.as_ref().unwrap().pieces)
    {
        assert_eq!(before.path, after.path);
    }
    let graph = package::compile_source(&authoring::shortcut_source()).unwrap();
    let a = graph.assembled_track.as_ref().unwrap();
    for cp in &graph.courses[0].definition.checkpoints {
        for route in &a.routes {
            assert!(route.pieces.iter().any(|i| a.pieces[*i]
                .path
                .iter()
                .any(|s| s.position_cm == cp.position_cm)));
        }
    }
}

#[test]
fn manual_flight_and_static_shapes_survive_both_containers() {
    let mut source=authoring::shortcut_source();
    let automatic=package::compile_source(&source).unwrap();
    let mut beam=automatic.gimmicks.iter().find(|g|g.id.starts_with("action-")).unwrap().clone();
    beam.id="authored-test-beam".into();
    beam.motion.kind=mapkit_core::gimmick::MotionKind::Static;
    beam.effect=None;
    source.structures.push(beam.clone());
    source.actions[0].kind="manual_flight".into();
    let document=package::compile_source(&source).unwrap();
    let bytes=pack_bytes(document.clone(),BTreeMap::new()).unwrap();
    let read=read_bytes(&bytes).unwrap();
    assert!(read.document.gimmicks.contains(&beam));
    assert!(!read.document.gimmicks.iter().any(|g|g.id.starts_with("action-")));
    assert!(read.document.courses[0].validation.is_none());
    let indexed=indexed::pack_source(document.clone(),BTreeMap::new(),1).unwrap();
    let mut reader=indexed::IndexedReader::open(std::io::Cursor::new(&indexed),u64::MAX,None).unwrap();
    reader.audit(u64::MAX,&indexed::ReadEpoch::default().begin()).unwrap();
    assert_eq!(reader.index().world_content_hash,read.inspection.world_content_hash);
}
