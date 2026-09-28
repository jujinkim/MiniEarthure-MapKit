use mapkit_core::assembled_track::{start_surface_at, Settings, START_PIECES};

#[test]
fn each_start_row_uses_its_actual_runway_piece() {
    let settings = Settings { seed: 7, circuit: false, gimmicks: vec![], ..Settings::default() };
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
