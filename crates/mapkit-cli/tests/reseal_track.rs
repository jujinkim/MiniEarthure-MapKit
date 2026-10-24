//! `mapkit reseal-track` on a stale seed package read from disk.
use mapkit_core::assembled_track::{Length, Settings};
use mapkit_core::{canonical, sha256, MapDocument};
use mapkit_package::{assembled_track as package, pack_bytes, PackageManifest};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::process::Command;

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
    manifest.world_content_hash = sha256(&canonical(&(identity, BTreeMap::<String, String>::new())).unwrap());
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

fn mapkit(args: &[&std::path::Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mapkit")).arg("reseal-track").args(args).output().unwrap()
}
fn error_code(out: &std::process::Output) -> String {
    assert!(!out.status.success());
    let e: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    e["code"].as_str().unwrap().into()
}

#[test]
fn reseal_track_rewrites_a_stale_seed_package_from_disk() {
    let dir = std::env::temp_dir().join(format!("mapkit-reseal-track-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).unwrap();
    let settings = Settings { seed: 1, circuit: false, length: Length::Short, ..Settings::default() };
    let mut current = package::generate(&settings).unwrap();
    current.free_roam = true;
    current.backdrop = Some("snow_mountain".into());
    package::reseal(&mut current).unwrap();
    let current_bytes = pack_bytes(current, BTreeMap::new()).unwrap();
    let stale = dir.join("stale.memap");
    std::fs::write(&stale, stale_package(&current_bytes, |d| {
        d.assembled_track.as_mut().unwrap().generator_fingerprint = "0".repeat(64);
    })).unwrap();

    let output = dir.join("resealed.memap");
    let out = mapkit(&[&stale, &output]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(bytes, current_bytes, "free_roam, backdrop and the course survive");
    let summary: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(summary["package_sha256"], sha256(&bytes));
    assert_eq!(summary["package_bytes"], bytes.len());
    assert!(!mapkit(&[&stale, &output]).status.success(), "never overwrites");
    assert_eq!(std::fs::read(&output).unwrap(), current_bytes);

    let authored = package::compile_source(&mapkit_core::assembled_track::authoring::shortcut_source()).unwrap();
    let authored_path = dir.join("authored.memap");
    std::fs::write(&authored_path, pack_bytes(authored, BTreeMap::new()).unwrap()).unwrap();
    assert_eq!(error_code(&mapkit(&[&authored_path, &dir.join("authored-out.memap")])), "E_TRACK_REQUIRED");
    assert!(!dir.join("authored-out.memap").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
