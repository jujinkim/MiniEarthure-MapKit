//! A generated course is tied to exact current source, assembly and content.
use crate::*;
use mapkit_core::{assembled_track as track, course::*};

pub fn course(document: &MapDocument, world: &str) -> Result<Course> {
    let a = document
        .assembled_track
        .as_ref()
        .ok_or_else(|| error("E_TRACK_REQUIRED", "assembled track required"))?;
    let mut checkpoints = Vec::new();
    let stride = a.pieces.len().div_ceil(48).max(1);
    for (i, p) in a.pieces.iter().enumerate() {
        if i != 0 && (i <= 2 || i % stride != 0) {
            continue;
        }
        let start = &a.pieces[2];
        let position = if i == 0 {
            start.path[start.path.len() / 2].position_cm
        } else {
            p.path[0].position_cm
        };
        checkpoints.push(Checkpoint {
            position_cm: position,
            radius_cm: (track::WIDTH / 2 + 30) as u32,
            shape: CheckpointShape::Sphere,
            placement_mode: PlacementMode::RoadSnap,
            surface_id: format!("assembled-road-{}", if i == 0 { 2 } else { i }),
        });
    }
    if !a.settings.circuit {
        let last = a.pieces.last().unwrap();
        let position = last.path[last.path.len() * 3 / 4].position_cm;
        checkpoints.push(Checkpoint {
            position_cm: position,
            radius_cm: (track::WIDTH / 2 + 30) as u32,
            shape: CheckpointShape::Sphere,
            placement_mode: PlacementMode::RoadSnap,
            surface_id: format!("assembled-road-{}", a.pieces.len() - 1),
        });
    }
    Course::from_definition(
        CourseBody {
            map_id: document.map_id.clone(),
            display_name: format!(
                "Seed {} · {} s{}",
                a.settings.seed,
                a.settings.duration_seconds,
                if a.settings.circuit { "/lap" } else { "" }
            ),
            world_content_hash: world.into(),
            mode: if a.settings.circuit {
                Mode::Circuit
            } else {
                Mode::Sprint
            },
            start_mode: StartMode::Ground,
            start_direction: [0, 1],
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
pub fn verify(document: &MapDocument, world: &str, candidate: &Course) -> Result<()> {
    track::verify_document(document)?;
    candidate.validate(world, &document.bounds)?;
    if *candidate != course(document, world)? {
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
