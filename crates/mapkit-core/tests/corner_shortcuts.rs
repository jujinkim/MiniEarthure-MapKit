use mapkit_core::assembled_track::{self as track,authoring,Settings};
#[test]
fn seeded_corner_shortcuts_are_reproducible_and_bounded() {
    // Seeds chosen for the motif generator; 4 places one corner rail from its ticket.
    for (seed,count) in [(0,0),(4,1)] {
        let settings=Settings{seed,circuit:false,length:track::Length::Long,..Settings::default()};
        let a=track::assemble(&settings).unwrap();
        let s=a.seed_source.as_ref().unwrap();
        assert_eq!(s.paths.len(),1<<(count+s.jump_shortcuts.len()));
        assert_eq!(track::assemble(&settings).unwrap(),a);
        assert_eq!(authoring::compile(s).unwrap().pieces,a.pieces);
        assert_eq!(authoring::compile(s).unwrap().supports,a.supports);
        assert_eq!(s.grind_lines.iter().filter(|l|l.id.ends_with("-rail")).count(),count);
    }
    let a=track::assemble(&Settings{seed:21,circuit:false,length:track::Length::Long,
        categories:vec!["driving".into(),"action".into()],..Settings::default()}).unwrap();
    assert!(a.seed_source.as_ref().unwrap().grind_lines.is_empty());
}
