//! A generated course is tied to exact current source, assembly and content.
use crate::*;
use mapkit_core::{assembled_track as track, course::*};

pub fn course(document: &MapDocument, world: &str) -> Result<Course> {
    let a = document
        .assembled_track
        .as_ref()
        .ok_or_else(|| error("E_TRACK_REQUIRED", "assembled track required"))?;
    track::authoring::executable(a)?;
    let checkpoints = track::authoring::common_checkpoints(a)
        .into_iter()
        .map(|(i, n)| {
            let sample = &a.pieces[i].path[n];
            Checkpoint {
                position_cm: sample.position_cm,
                radius_cm: sample.lateral_cm + 30,
                shape: CheckpointShape::Sphere,
                placement_mode: PlacementMode::RoadSnap,
                surface_id: format!("assembled-road-{i}"),
            }
        })
        .collect();
    let first = &a.pieces[a.routes[0].pieces[0]].path[0];
    Course::from_definition(
        CourseBody {
            map_id: document.map_id.clone(),
            display_name: if a.authoring.is_some() {
                format!(
                    "Authored track · {:.1} s estimate",
                    a.estimated_msec as f64 / 1000.0
                )
            } else {
                format!(
                    "Seed {} · {} s{}",
                    a.settings.seed,
                    a.settings.duration_seconds,
                    if a.settings.circuit { "/lap" } else { "" }
                )
            },
            world_content_hash: world.into(),
            mode: if a.settings.circuit {
                Mode::Circuit
            } else {
                Mode::Sprint
            },
            start_mode: StartMode::Ground,
            start_direction: [first.forward[0], first.forward[2]],
            checkpoints,
        },
        &document.bounds,
    )
}
pub fn generate(settings: &track::Settings) -> Result<MapDocument> {
    let mut d = track::document(settings)?;
    let world = super::content_hash(&d, &BTreeMap::new())?;
    d.courses = vec![course(&d, &world)?];
    d.validate()?;
    Ok(d)
}
/// Rebind the generated course after an allowed metadata edit. Exact geometry
/// verification is still mandatory; this does not certify edited track sources.
pub fn reseal(document: &mut MapDocument) -> Result<()> {
    track::verify_document(document)?;
    let world = super::content_hash(document, &BTreeMap::new())?;
    if document
        .assembled_track
        .as_ref()
        .is_some_and(|a| !a.issues.is_empty())
    {
        document.courses.clear();
    } else {
        document.courses = vec![course(document, &world)?];
    }
    document.validate()
}
pub fn verify(document: &MapDocument, world: &str, candidate: &Course) -> Result<()> {
    track::verify_document(document)?;
    verify_course(document, world, candidate)
}
/// Association checks for an already deterministically verified immutable document.
/// This alone does not certify the document; `verify` remains the complete public check.
pub fn verify_course(document: &MapDocument, world: &str, candidate: &Course) -> Result<()> {
    candidate.validate(world, &document.bounds)?;
    let mut definition = candidate.clone();
    definition.validation = None;
    if definition != course(document, world)? {
        return Err(error(
            "E_TRACK_COURSE",
            "course does not match the assembled course",
        ));
    }
    Ok(())
}
pub fn save(settings: &track::Settings, path: &Path) -> Result<serde_json::Value> {
    let document = generate(settings)?;
    let bytes = pack_bytes(document.clone(), BTreeMap::new())?;
    mapkit_core::cancellation::checkpoint()?;
    write_new(path, &bytes)?;
    Ok(
        serde_json::json!({"document":document,"path":path,"package_sha256":sha256(&bytes),"package_bytes":bytes.len()}),
    )
}

pub fn compile_source(source: &track::authoring::Source) -> Result<MapDocument> {
    let mut d = track::document_from_assembly(track::authoring::compile(source)?)?;
    if d.assembled_track.as_ref().unwrap().issues.is_empty() {
        let world = super::content_hash(&d, &BTreeMap::new())?;
        d.courses = vec![course(&d, &world)?];
    }
    d.validate()?;
    Ok(d)
}
