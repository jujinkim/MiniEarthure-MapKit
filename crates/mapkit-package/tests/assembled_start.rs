use mapkit_core::assembled_track::{start_surface_at, Settings, START_PIECES};

#[test]
fn each_start_row_uses_its_actual_runway_piece() {
    let settings = Settings { seed: 7, circuit: false, categories: vec!["driving".into()], ..Settings::default() };
    let document = mapkit_package::assembled_track::generate(&settings).unwrap();
    let assembly = document.assembled_track.as_ref().unwrap();
    for (i, piece) in assembly.pieces.iter().take(START_PIECES).enumerate() {
        let middle = piece.path[piece.path.len() / 2].position_cm;
        assert_eq!(start_surface_at(assembly, [middle[0],middle[2]]), Some(format!("assembled-road-{i}")));
        assert_eq!(start_surface_at(assembly, [middle[0]+i64::from(piece.width_cm)/2+1,middle[2]]), None);
    }
    assert_eq!(start_surface_at(assembly, [0,-1]), None);
    let end = assembly.pieces[START_PIECES-1].path.last().unwrap().position_cm;
    assert_eq!(start_surface_at(assembly, [end[0],end[2]+1]), None);
    assert_eq!(start_surface_at(assembly, [i64::MAX,i64::MIN]), None);
}

#[test]
fn new_seed_start_keeps_eight_full_size_rows_on_the_runway() {
    let d = mapkit_package::assembled_track::generate(&Settings {seed:7,circuit:false,..Default::default()}).unwrap();
    let a = d.assembled_track.as_ref().unwrap();
    let source = a.seed_source.as_ref().unwrap();
    assert_eq!(source.checkpoints[0].sample,a.pieces[2].path.len()-1);
    assert_eq!(d.courses[0].definition.checkpoints[0].position_cm,a.pieces[2].path.last().unwrap().position_cm);
    for row in 0..4 {
        let z=1830-500*row-132;
        for x in [-243,243] {for dz in [-90,90] {
            assert!(start_surface_at(a,[x,z+dz]).is_some());
        }}
    }
}
