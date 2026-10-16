use mapkit_core::assembled_track::{self as track,authoring,Settings};
#[test]
fn seeded_corner_shortcuts_are_reproducible_and_bounded() {
    for (seed,count) in [(0,0),(11,1),(21,2)] {
        let settings=Settings{seed,circuit:false,duration_seconds:120,..Settings::default()};
        let a=track::assemble(&settings).unwrap();
        let s=a.seed_source.as_ref().unwrap();
        assert_eq!(s.grind_lines.len(),count,"seed {seed}");
        assert_eq!(s.paths.len(),1<<count);
        assert_eq!(track::assemble(&settings).unwrap(),a);
        assert_eq!(authoring::compile(s).unwrap().pieces,a.pieces);
        assert_eq!(authoring::compile(s).unwrap().supports,a.supports);
        assert!(a.routes[0].estimated_msec.abs_diff(120_000)<=12_000);
    }
    let a=track::assemble(&Settings{seed:21,circuit:false,duration_seconds:120,
        categories:vec!["driving".into(),"action".into()],..Settings::default()}).unwrap();
    assert!(a.seed_source.as_ref().unwrap().grind_lines.is_empty());
}
