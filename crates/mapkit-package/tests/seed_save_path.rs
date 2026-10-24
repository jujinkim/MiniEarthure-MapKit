//! Process-wide generation counters: this binary holds a single test on purpose.
use mapkit_core::assembled_track::{self as track, Length, Settings};
use mapkit_core::{canonical, sha256, MapDocument};
use mapkit_package::{assembled_track as package, *};
use std::io::{Cursor, Read, Write};

/// A previous build's package: the stored document's assembly is edited and the
/// manifest is rebound so `read` accepts it, while export still refuses it.
fn stale_package(current: &[u8], edit: impl FnOnce(&mut MapDocument)) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(current)).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = (0..archive.len()).map(|i| {
        let mut f = archive.by_index(i).unwrap();
        let mut b = vec![];
        f.read_to_end(&mut b).unwrap();
        (f.name().into(), b)
    }).collect();
    let i = entries.iter().position(|e| e.0 == "document.json").unwrap();
    let mut d: MapDocument = serde_json::from_slice(&entries[i].1).unwrap();
    edit(&mut d);
    d.courses.clear();
    let document = canonical(&d).unwrap();
    let mut identity = serde_json::to_value(&d).unwrap();
    for key in ["provenance", "attributions", "courses", "backdrop"] { identity.as_object_mut().unwrap().remove(key); }
    let mut manifest: PackageManifest = serde_json::from_slice(&entries[0].1).unwrap();
    manifest.world_content_hash = sha256(&canonical(&(identity, std::collections::BTreeMap::<String, String>::new())).unwrap());
    let record = manifest.files.iter_mut().find(|f| f.path == "document.json").unwrap();
    record.size = document.len() as u64;
    record.sha256 = sha256(&document);
    entries[i].1 = document;
    entries[0].1 = canonical(&manifest).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
    for (p, b) in entries {
        writer.start_file(p, zip::write::FileOptions::default()).unwrap();
        writer.write_all(&b).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn seed_save_assembles_once_and_reseal_recompiles_stale_packages() {
    let settings = Settings { seed: 1, circuit: false, length: Length::Short, ..Settings::default() };
    let path = std::env::temp_dir().join(format!("mapkit-seed-save-{}.memap", std::process::id()));
    let _ = std::fs::remove_file(&path);
    track::reset_generation_counters();
    let saved = package::save(&settings, &path).unwrap();
    let counters = track::generation_counters();
    assert_eq!(counters.assemblies, 1, "{counters:?}");
    assert!(counters.compiles >= 1 && counters.conflicts > 0 && counters.materializations > 0, "{counters:?}");
    let p = read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let d: MapDocument = p.document.into();
    assert_eq!(serde_json::to_value(&d).unwrap(), saved["document"]);

    track::reset_generation_counters();
    track::verify_document(&d).unwrap();
    pack_bytes(d.clone(), Default::default()).unwrap();
    assert_eq!(track::generation_counters().assemblies, 0);
    track::verify_seed_reproduction(&d).unwrap();
    assert_eq!(track::generation_counters().assemblies, 1);

    let mut stale = d.clone();
    stale.assembled_track.as_mut().unwrap().generator_fingerprint = "0".repeat(64);
    assert_eq!(track::verify_document(&stale).unwrap_err().code, "E_TRACK_MODIFIED");
    assert!(pack_bytes(stale.clone(), Default::default()).is_err());
    track::reset_generation_counters();
    let resealed = package::reseal_seed(&stale).unwrap();
    assert_eq!(track::generation_counters().assemblies, 0);
    assert_eq!(resealed, d);
    assert_eq!(pack_bytes(resealed, Default::default()).unwrap(), pack_bytes(d.clone(), Default::default()).unwrap());

    // A stale package read from disk keeps the user's free_roam and backdrop.
    let mut themed = d.clone();
    themed.free_roam = true;
    themed.backdrop = Some("snow_mountain".into());
    package::reseal(&mut themed).unwrap();
    let themed_bytes = pack_bytes(themed.clone(), Default::default()).unwrap();
    let stale_path = path.with_extension("stale.memap");
    std::fs::write(&stale_path, stale_package(&themed_bytes, |s| {
        s.assembled_track.as_mut().unwrap().generator_fingerprint = "0".repeat(64);
    })).unwrap();
    let loaded = read(&stale_path);
    std::fs::remove_file(&stale_path).unwrap();
    let loaded: MapDocument = loaded.unwrap().document.into();
    assert_eq!(loaded.assembled_track.as_ref().unwrap().generator_fingerprint, "0".repeat(64));
    assert!(loaded.courses.is_empty());
    assert_eq!(pack_bytes(loaded.clone(), Default::default()).unwrap_err().code, "E_TRACK_MODIFIED");
    track::reset_generation_counters();
    let resealed = package::reseal_seed(&loaded).unwrap();
    assert_eq!(track::generation_counters().assemblies, 0);
    assert!(resealed.free_roam);
    assert_eq!(resealed.backdrop.as_deref(), Some("snow_mountain"));
    assert_eq!(resealed, themed);
    assert_eq!(pack_bytes(resealed, Default::default()).unwrap(), themed_bytes);

    // Seed tracks have no ordinary roads or authoring instances, so a stored
    // attachment cannot resolve: reseal refuses it instead of dropping it.
    let mut attached = stale.clone();
    attached.surface_attachments.push(track::surface::Attachment {
        id: "jump".into(),
        surface: track::surface::Reference { surface_id: "track:main".into(), station_cm: 400 },
        kind: "jump_panel".into(), height_cm: 200, panel_width_percent: 50,
        panel_alignment: track::authoring::PanelAlignment::Center, side: 1,
    });
    assert_eq!(package::reseal_seed(&attached).unwrap_err().code, "E_SURFACE_REFERENCE");
    attached.surface_attachments[0].surface.surface_id = "designed".into();
    assert_eq!(package::reseal_seed(&attached).unwrap_err().code, "E_SURFACE_REFERENCE");

    let mut edited = d.clone();
    edited.assembled_track.as_mut().unwrap().pieces[3].path[1].position_cm[1] += 1;
    assert_eq!(track::verify_document(&edited).unwrap_err().code, "E_TRACK_MODIFIED");
    let mut source = track::authoring::Source::empty();
    source.instances.push(track::authoring::instance("road", "sprint_lane", 400));
    let authored = track::document_from_assembly(track::authoring::compile(&source).unwrap()).unwrap();
    assert_eq!(track::reseal_seed_document(&authored).unwrap_err().code, "E_TRACK_REQUIRED");
    assert_eq!(track::verify_seed_reproduction(&authored).unwrap_err().code, "E_TRACK_REQUIRED");
}
