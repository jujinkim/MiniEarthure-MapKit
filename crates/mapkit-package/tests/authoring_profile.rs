//! Editor (Authoring) profile: in-memory project open and PNG16 step choice.
use mapkit_core::*;
use mapkit_package::{working::*, *};
use std::{collections::BTreeMap, path::{Path, PathBuf}};

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
}

#[test]
fn in_memory_project_open_matches_the_stored_preview_zip_route() {
    for name in ["minimal", "assets", "placement", "roads"] {
        let (d, files) = read_project(&example(name)).unwrap();
        let zipped = read_bytes(&pack_stored_preview(d.clone(), files.clone()).unwrap()).unwrap();
        let (opened, warnings) = Profile::Authoring.run(|| Package::from_project(d, files));
        let opened = opened.unwrap();
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
        assert_eq!(canonical(&opened.document.to_document()).unwrap(), canonical(&zipped.document.to_document()).unwrap());
        assert_eq!(opened.files, zipped.files, "{name}");
        assert_eq!(canonical(&opened.manifest).unwrap(), canonical(&zipped.manifest).unwrap(), "{name}");
        let (a, b) = (&opened.inspection, &zipped.inspection);
        assert_eq!(a.world_content_hash, b.world_content_hash);
        assert_eq!((a.expanded_bytes, a.user_asset_bytes, a.base_data_bytes, a.cell_count, a.free_roam),
            (b.expanded_bytes, b.user_asset_bytes, b.base_data_bytes, b.cell_count, b.free_roam));
        assert_eq!((a.package_sha256.as_str(), a.package_bytes), ("", 0));
    }
}

#[test]
fn project_with_manifest_above_four_mib_opens_with_a_runtime_warning() {
    let root = std::env::temp_dir().join(format!("mapkit-authoring-manifest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.attributions[0].notice = "x".repeat(5 * 1024 * 1024);
    std::fs::write(root.join("document.json"), canonical(&d).unwrap()).unwrap();

    let (d, files) = read_project(&root).unwrap();
    assert_eq!(pack_stored_preview(d.clone(), files.clone()).unwrap_err(),
        error("E_LIMIT", "manifest or expanded package exceeds limits"));
    let (opened, warnings) = Profile::Authoring.run(|| {
        let (d, files) = read_project(&root)?;
        Package::from_project(d, files)
    });
    let opened = opened.unwrap();
    assert_eq!(opened.document.attributions[0].notice.len(), 5 * 1024 * 1024);
    let w = warnings.iter().find(|w| w.code == "manifest_bytes").unwrap();
    assert!(w.actual > MAX_MANIFEST_BYTES && w.limit == MAX_MANIFEST_BYTES && w.runtime_rejects);
    std::fs::remove_dir_all(&root).unwrap();
}

fn reference_png16(grid: &HeightGrid) -> (Vec<u8>, i64) {
    // The previous fixed 1 cm encoder, kept here as the byte-identity oracle.
    let low = *grid.heights_cm.iter().min().unwrap();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, grid.side as u32, grid.side as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let raw: Vec<u8> = grid.heights_cm.iter().flat_map(|v| ((*v - low) as u16).to_be_bytes()).collect();
        encoder.write_header().unwrap().write_image_data(&raw).unwrap();
    }
    (bytes, low)
}

fn snapshot() -> WorkingSnapshot {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.bounds = Bounds { min: [0, 0], max: [6400, 6400] };
    d.cell_size_cm = 3200;
    d.free_roam = true;
    d.roads.clear();
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.attributions.clear();
    WorkingSnapshot::new(d, Resources::default()).unwrap()
}

#[test]
fn png16_keeps_one_centimetre_bytes_when_the_range_fits() {
    let grid = HeightGrid { side: 3, heights_cm: vec![-7, 0, 65528, 3, 4, 5, 6, 7, 8] };
    assert_eq!(encode_heightmap(&grid).unwrap(), reference_png16(&grid));
    assert_eq!(encode_heightmap_step(&grid, 1).unwrap(), reference_png16(&grid));
    let mut s = snapshot();
    s.set_sample([5, 5], 65_535).unwrap();
    s.set_sample([16, 5], 101).unwrap();
    let (d, files) = s.materialize().unwrap();
    for h in &d.heightmaps {
        assert_eq!(h.step_cm, 1);
        assert_eq!(files[&h.path], reference_png16(&s.tiles[&h.cell]).0);
    }
}

#[test]
fn png16_steps_relief_above_655_metres_and_keeps_seams() {
    let mut s = snapshot();
    s.set_sample([5, 5], 70_000).unwrap();
    s.set_sample([16, 5], 101).unwrap(); // shared by cells (0,0) and (1,0)
    s.set_sample([3, 3], 69_999).unwrap();
    let (result, warnings) = Profile::Authoring.run(|| s.materialize());
    let (d, files) = result.unwrap();
    assert_eq!(warnings.iter().find(|w| w.code == "heightmap_step").map(|w| (w.actual, w.runtime_rejects)), Some((2, false)));
    let decoded = |cell: Cell| {
        let h = d.heightmaps.iter().find(|h| h.cell == cell).unwrap();
        (h.step_cm, decode_heightmap(h, d.cell_size_cm, &files[&h.path]).unwrap())
    };
    let (step, a) = decoded(Cell { x: 0, y: 0 });
    assert_eq!(step, 2);
    assert_eq!(a.heights_cm[5 * 17 + 5], 70_000);
    assert_eq!(a.heights_cm[3 * 17 + 3], 69_998); // floored onto the 2 cm lattice
    let (step, b) = decoded(Cell { x: 1, y: 0 });
    assert_eq!(step, 1);
    // The shared edge takes the coarser step on both sides.
    assert_eq!((a.heights_cm[5 * 17 + 16], b.heights_cm[5 * 17]), (100, 100));
    // Runtime reader accepts the result.
    let package = read_bytes(&pack_bytes(d.clone(), files.clone()).unwrap()).unwrap();
    assert_eq!(package.document.heightmaps.len(), d.heightmaps.len());
}

/// What the Editor does after writing a save: files on disk, saved baselines.
fn accept_saved(s: &mut WorkingSnapshot, d: &MapDocument, files: BTreeMap<String, Vec<u8>>) {
    for (p, b) in files { s.resources.memory.insert(p, std::sync::Arc::new(b)); }
    s.saved_descriptors = d.heightmaps.iter().cloned().map(|h| (h.cell, h)).collect();
    s.modified.clear();
    s.saved_tiles = s.tiles.clone();
    s.changed_indices.clear();
}

#[test]
fn second_save_after_a_stepped_save_keeps_seams_and_kept_pngs() {
    let mut s = snapshot();
    s.set_sample([5, 5], 70_000).unwrap();
    s.set_sample([16, 5], 101).unwrap();
    let (r, _) = Profile::Authoring.run(|| s.materialize());
    let (d, files) = r.unwrap();
    let kept = d.heightmaps.iter().find(|h| h.cell == Cell { x: 1, y: 0 }).unwrap().clone();
    accept_saved(&mut s, &d, files);
    // Remove the spike in (0,0) only: the stored, floored edge of (1,0) is kept.
    s.set_sample([5, 5], 0).unwrap();
    let (r, warnings) = Profile::Authoring.run(|| s.materialize());
    let (d, files) = r.unwrap();
    assert!(warnings.iter().all(|w| w.code != "heightmap_step"), "{warnings:?}");
    assert_eq!(d.heightmaps.iter().find(|h| h.cell == kept.cell).unwrap(), &kept);
    let a = d.heightmaps.iter().find(|h| h.cell == Cell { x: 0, y: 0 }).unwrap();
    assert_eq!(a.step_cm, 1);
    assert_eq!(decode_heightmap(a, d.cell_size_cm, &files[&a.path]).unwrap().heights_cm[5 * 17 + 16], 100);
}

#[test]
fn stepped_cell_never_requantizes_an_unchanged_neighbour() {
    for (shared, accepted) in [(101, false), (100, true)] {
        let mut s = snapshot();
        s.set_sample([16, 5], shared).unwrap(); // exact shared sample at 1 cm
        let (d, files) = s.materialize().unwrap();
        let kept = d.heightmaps.iter().find(|h| h.cell == Cell { x: 1, y: 0 }).unwrap().clone();
        accept_saved(&mut s, &d, files);
        s.set_sample([5, 5], 70_000).unwrap(); // only (0,0) changes; it needs 2 cm steps
        let result = Profile::Authoring.run(|| s.materialize()).0;
        if !accepted {
            // An odd stored edge is not on the 2 cm lattice: refuse, never re-encode (1,0).
            assert_eq!(result.unwrap_err().code, "E_HEIGHTMAP");
            continue;
        }
        let (d, _) = result.unwrap();
        assert_eq!(d.heightmaps.iter().find(|h| h.cell == kept.cell).unwrap(), &kept);
        assert_eq!(d.heightmaps.iter().find(|h| h.cell == Cell { x: 0, y: 0 }).unwrap().step_cm, 2);
    }
}
