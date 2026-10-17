use mapkit_core::assembled_track::{self as track,authoring,jump_shortcuts};
#[test]
fn jump_templates_are_disconnected_v1_routes() {
    for count in 5..=8 {
        let source=jump_shortcuts::source(count,true).unwrap();
        let assembly=authoring::compile(&source).unwrap();
        assert!(assembly.issues.is_empty(), "{count}: {:?}", assembly.issues);
        let doc=track::document_from_assembly(assembly.clone()).unwrap();
        assert_eq!(source.jump_shortcuts[0].platforms.len(),count);
        assert_eq!(doc.gimmicks.iter().filter(|g|g.id.contains("-solid-")).count(),count);
        assert_eq!(assembly.routes.len(),2);
        let encoded=serde_json::to_vec(&assembly).unwrap();
        let decoded:track::Assembly=serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded,assembly);
        assert_eq!(authoring::compile(decoded.authoring.as_ref().unwrap()).unwrap(),assembly);
    }
}

#[test]
fn seeded_jump_routes_reproduce_and_obey_category_gate() {
    // Seeds re-chosen after the §44.363 diversity generator; 93 places one jump detour.
    for (seed,count) in [(0,0),(93,1)] {
        let settings=track::Settings{seed,circuit:false,duration_seconds:120,..Default::default()};
        let a=track::assemble(&settings).unwrap();
        assert_eq!(a.seed_source.as_ref().unwrap().jump_shortcuts.len(),count);
        assert_eq!(track::assemble(&settings).unwrap(),a);
        let mut disabled=settings;
        disabled.categories=vec!["driving".into(),"action".into()];
        assert!(track::assemble(&disabled).unwrap().seed_source.unwrap().jump_shortcuts.is_empty());
    }
}
