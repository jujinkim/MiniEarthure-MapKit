use mapkit_core::{*, assembled_track::{self as track, authoring::*}};
use mapkit_package::*;
use std::collections::BTreeMap;

fn base() -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    d.free_roam = true; d.bounds = Bounds { min: [-32000, -32000], max: [32000, 32000] }; d.cell_size_cm = 3200;
    d.roads.clear(); d.nodes.clear(); d.repetitions.clear(); d.buildings.clear(); d.zones.clear(); d.placements.clear(); d.assets.clear();
    d
}
fn chain(presets: &[&str], width: u32) -> Source {
    let mut source = Source::empty();
    for (i, preset) in presets.iter().enumerate() {
        let mut item = instance(&format!("p{i}"), preset, width);
        if let Some(last) = source.instances.last() {
            item = snap(&item, last).unwrap();
            source.connections.push(Connection { from: last.id.clone(), to: item.id.clone() });
        } else { item.position_cm = [0, 0, -8000]; }
        source.instances.push(item);
    }
    source.paths = vec![Path { id: "base".into(), pieces: source.instances.iter().map(|i| i.id.clone()).collect()}];
    source
}

#[test]
fn every_region_preset_packs_and_generates_at_every_region_width() {
    let presets = track::catalogue_ids().iter().filter(|id| !track::region_widths(id).is_empty()).count();
    assert_eq!(presets, 19, "every listed region preset exists in the catalogue");
    for &width in track::REGION_WIDTHS {
        for id in track::catalogue_ids().iter().filter(|id| !track::region_widths(id).is_empty()) {
            let mut source = chain(&["straight", id, "straight"], width);
            if *id == "free_curve" { source.instances[1].control_points = vec![[0,0,0],[0,0,600],[600,0,1200],[1200,0,1200]]; source = chain_fix(source); }
            let d = track::composite::apply_source(&base(), &source).unwrap_or_else(|e| panic!("{id} {width}: {e}"));
            track::verify_document(&d).unwrap_or_else(|e| panic!("{id} {width}: {e}"));
            let a = d.assembled_track.as_ref().unwrap();
            assert!(a.pieces.iter().all(|p| p.width_cm == width), "{id} {width}");
            let package = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap_or_else(|e| panic!("{id} {width}: {e}"))).unwrap();
            let p = &a.pieces[1];
            let cell = package.document.cell_at([p.path[0].position_cm[0], p.path[0].position_cm[2]]).unwrap();
            let chunk = package.generate(cell, 400_000).unwrap();
            assert!(chunk.triangles.iter().any(|t| t.object_id == "assembled-road-1"), "{id} {width}");
        }
    }
}
fn chain_fix(mut s: Source) -> Source {
    let (a, b) = (s.instances[0].clone(), s.instances[1].clone());
    s.instances[1] = snap(&b, &a).unwrap();
    s.instances[2] = snap(&s.instances[2], &s.instances[1]).unwrap();
    s
}
#[test]
fn region_widths_are_overlay_only_and_ordinary_presets_only() {
    let narrow = chain(&["straight", "straight"], 150);
    assert_eq!(compile(&narrow).unwrap_err().code, "E_TRACK_SOURCE");
    assert!(track::region_widths("loop").is_empty() && track::region_widths("cylinder").is_empty());
    let mut special = Source::empty(); special.instances.push(instance("l", "loop", 150));
    assert!(track::composite::apply_source(&base(), &special).is_err());
    assert_eq!(track::supported_widths("straight"), &[400, 600, 800, 1200]);
}
