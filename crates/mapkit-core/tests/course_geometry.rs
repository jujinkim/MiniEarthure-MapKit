use mapkit_core::{course::*, *};
fn gate(p: Vertex, r:u32, shape:CheckpointShape)->Checkpoint {
    Checkpoint {position_cm:p,radius_cm:r,shape,placement_mode:PlacementMode::Free,surface_id:String::new()}
}
#[test]
fn stable_overlap_list_preserves_source_and_allows_disabled_courses() {
    let bounds=Bounds{min:[-10000,-10000],max:[10000,10000]};
    let body=CourseBody{map_id:"map".into(),display_name:"course".into(),world_content_hash:"a".repeat(64),mode:Mode::Sprint,start_mode:StartMode::Ground,start_direction:[1,0],checkpoints:vec![gate([0,0,0],100,CheckpointShape::Sphere),gate([50,0,0],100,CheckpointShape::Sphere),gate([400,0,0],100,CheckpointShape::Sphere)]};
    assert_eq!(body.effective_indices(),[0,2]);
    // Simulate a stored valid source identity before deriving its driving view.
    let raw=Course{format:"miniearthure-course".into(),format_version:1,course_id:sha256(&canonical(&body).unwrap()),definition:body,validation:None};
    raw.validate_document(&bounds).unwrap();
    let view=raw.effective();
    assert_eq!(raw.definition.checkpoints.len(),3);assert_eq!(view.definition.checkpoints.len(),2);
    assert_ne!(view.course_id,raw.course_id);view.validate_document(&bounds).unwrap();
    let mut one=raw.definition.clone();one.checkpoints.truncate(1);
    Course::from_definition(one,&bounds).unwrap();
}
#[test]
fn hemisphere_overlap_uses_actual_half_volume() {
    let upper=gate([0,0,0],100,CheckpointShape::Hemisphere);
    let below=gate([0,-100,0],30,CheckpointShape::Sphere);
    assert!(!upper.overlaps(&below));
    assert!(upper.overlaps(&gate([0,0,0],30,CheckpointShape::Sphere)));
    assert!(upper.overlaps(&gate([200,0,0],100,CheckpointShape::Sphere)));
    assert!(!upper.overlaps(&gate([201,0,0],100,CheckpointShape::Sphere)));
}
#[test]
fn continuous_capsule_grazes_without_stationary_or_initial_inside_progress() {
    let cp=gate([0,0,0],100,CheckpointShape::Sphere);
    let a=Capsule{a:[-500,124,-70],b:[-500,124,70],radius_cm:25};
    let b=Capsule{a:[500,124,-70],b:[500,124,70],radius_cm:25};
    let t=capsule_entry(a,b,&cp).unwrap();assert!(t>0.45&&t<0.5);
    assert!(capsule_entry(a,a,&cp).is_none());
    assert!(capsule_entry(Capsule::point([0,0,0]),b,&cp).is_none());
    let far=Capsule{a:[-500,126,-70],b:[-500,126,70],radius_cm:25};
    assert!(capsule_entry(far,Capsule{a:[500,126,-70],b:[500,126,70],radius_cm:25},&cp).is_none());
    let hemi=gate([0,0,0],100,CheckpointShape::Hemisphere);
    assert!(capsule_entry(Capsule{a:[-500,-10,0],b:[-500,-10,0],radius_cm:11},Capsule{a:[500,-10,0],b:[500,-10,0],radius_cm:11},&hemi).is_some());
    assert!(capsule_entry(Capsule::point([-500,-1,0]),Capsule::point([500,-1,0]),&hemi).is_none());
}
