use mapkit_core::{assembled_track::{self as track,authoring::*},grind::*,*};
fn line() -> GrindLine {
    GrindLine{id:"air".into(),control_points:vec![[0,100,0],[0,100,1200]],up:[0,1_000_000,0],capture_width_cm:30,start_connections:vec![],end_connections:vec![]}
}
#[test] fn independent_air_line_and_cubic_frames_are_validated() {
    let mut l=line();validate(&[l.clone()]).unwrap();
    l.control_points=vec![[0,100,0],[0,100,2000],[2000,500,2000],[2000,500,0]];
    validate(&[l.clone()]).unwrap();
    let p=l.samples();
    assert!(p.len()>100);
    assert!(p.iter().all(|s|s.mode=="grind" && !s.safe));
    assert!(p.windows(2).all(|w| (0..3).map(|j|((w[0].position_cm[j]-w[1].position_cm[j]) as f64).powi(2)).sum::<f64>()<38.0*38.0));
    l.up=[0;3];assert!(validate(&[l]).is_err());
    let mut l=line();l.control_points[1]=l.control_points[0];assert!(validate(&[l]).is_err());
    let mut l=line();l.end_connections.push(Endpoint{line:"missing".into(),end:false});assert!(validate(&[l]).is_err());
    let mut l=line();l.end_connections.push(Endpoint{line:"x".repeat(33),end:false});assert!(validate_geometry(&[l]).is_err());
}
#[test] fn source_and_generated_line_tampering_and_budget() {
    let mut source=Source::empty();source.grind_lines.push(line());
    let d=track::document_from_assembly(compile(&source).unwrap()).unwrap();
    d.validate().unwrap();
    assert!(d.gimmicks.is_empty(),"air line needs no supporting object");
    let cell=d.cell_at([0,600]).unwrap();
    let chunk=generate(GenerationInput{document:&d,cell,heightgrid:None,max_triangles:500_000}).unwrap();
    assert_eq!(chunk.grind_lines,d.grind_lines);
    assert_eq!(chunk.hash().unwrap(),sha256(&canonical(&chunk).unwrap()));
    let cost=estimate_generation(&d,cell,500_000).unwrap();
    let key=archive_key(&"a".repeat(64),cell);
    let archive=encode_archive(&chunk,&key,archive_limit(&cost)).unwrap();
    assert_eq!(decode_archive(&archive,&key,cell,&cost).unwrap(),chunk);
    let mut corrupt=d.clone();corrupt.grind_lines[0].capture_width_cm+=1;assert!(corrupt.validate().is_err());
    source.grind_lines[0].control_points[1]=[0,100,10_000_000];assert!(compile(&source).is_err());
}
#[test] fn all_ordinary_curves_share_width_and_twist_refinement() {
    for preset in ["free_curve","gentle90","curve_left_up","spiral180_left_up"] {
        let mut i=instance("p",preset,1200);
        if preset=="free_curve" {i.control_points=vec![[0,0,0],[0,0,4000],[4000,1200,4000],[8000,1200,4000]];}
        let p=piece(&i).unwrap();
        for w in p.path.windows(2) {
            let dot=(0..3).map(|j|w[0].forward[j] as f64*w[1].forward[j] as f64/1e12).sum::<f64>();
            assert!(dot>0.9997,"{preset}: tangent step={dot}");
        }
    }
}

#[test] fn rail_preset_places_an_independent_editable_line() {
    let i=instance("road","sprint_lane",400);
    let attachment=Attachment{kind:"grind_rail".into(),piece:"road".into(),path:"main".into(),station_cm:800,side:1};
    let lines=attachment_lines(&i,&attachment).unwrap();
    assert_eq!(lines.len(),1);
    let mut source=Source::empty();source.instances.push(i);source.attachments.push(attachment);source.grind_lines=lines;
    let d=track::document_from_assembly(compile(&source).unwrap()).unwrap();
    assert_eq!(d.grind_lines.len(),1); assert!(!d.gimmicks.is_empty());
    source.grind_lines.clear();
    let removed=track::document_from_assembly(compile(&source).unwrap()).unwrap();
    assert!(removed.grind_lines.is_empty());assert_eq!(d.gimmicks,removed.gimmicks,"deleting the interaction leaves the supporting rail collider intact");
}
