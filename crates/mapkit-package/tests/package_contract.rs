use mapkit_core::*;
use mapkit_package::*;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};
fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
fn package(d: MapDocument) -> Vec<u8> {
    pack_bytes(d, BTreeMap::new()).unwrap()
}
fn rewrite(bytes: &[u8], edit: impl FnOnce(&mut Vec<(String, Vec<u8>)>)) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = vec![];
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).unwrap();
        let mut b = vec![];
        f.read_to_end(&mut b).unwrap();
        entries.push((f.name().into(), b));
    }
    edit(&mut entries);
    let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
    for (p, b) in entries {
        writer
            .start_file(p, zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&b).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn rejected(bytes: &[u8], code: &str) {
    assert_eq!(read_bytes(bytes).err().unwrap().code, code);
}
#[test]
fn reproducible_third_party_roundtrip() {
    let first = package(document());
    let p = read_bytes(&first).unwrap();
    assert_eq!(p.document.provenance.fingerprint, "untrusted-and-accepted");
    assert_eq!(first, pack_bytes(p.document, p.files).unwrap());
}
#[test]
fn provenance_does_not_change_world_or_generated_hash() {
    let a = read_bytes(&package(document())).unwrap();
    let mut d = document();
    d.provenance.tool_id = "unknown-tool".into();
    d.provenance.last_edited = "2026-09-08T00:00:00Z".into();
    let b = read_bytes(&package(d)).unwrap();
    assert_ne!(a.inspection.package_sha256, b.inspection.package_sha256);
    assert_eq!(
        a.inspection.world_content_hash,
        b.inspection.world_content_hash
    );
    assert_eq!(
        a.generate(Cell { x: 0, y: 0 }, 500_000).unwrap(),
        b.generate(Cell { x: 0, y: 0 }, 500_000).unwrap()
    );
}
#[test]
fn object_and_cell_order_independent() {
    let mut d = document();
    let a = read_bytes(&package(d.clone())).unwrap();
    d.nodes.reverse();
    d.roads.reverse();
    d.buildings.reverse();
    d.zones.reverse();
    let b = read_bytes(&package(d)).unwrap();
    assert_eq!(a.inspection.package_sha256, b.inspection.package_sha256);
    for c in a.document.cells().into_iter().rev() {
        assert_eq!(
            a.generate(c, 500_000).unwrap().hash().unwrap(),
            b.generate(c, 500_000).unwrap().hash().unwrap()
        );
    }
}
#[test]
fn spawn_uses_explicit_surface_and_never_rooftop() {
    let p = read_bytes(&package(document())).unwrap();
    let c = p.generate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    let ground = c
        .spawn(&SpawnRequest {
            position_cm: [25600, 25600],
            surface_id: "ground-road".into(),
        })
        .unwrap();
    let bridge = c
        .spawn(&SpawnRequest {
            position_cm: [25600, 25600],
            surface_id: "bridge-road".into(),
        })
        .unwrap();
    assert_eq!(ground[1], 20);
    assert_eq!(bridge[1], 700);
    assert!(c
        .spawn(&SpawnRequest {
            position_cm: [31000, 31000],
            surface_id: "building-1".into()
        })
        .is_err());
}
#[test]
fn geometry_clipped_and_vegetation_owned_once() {
    let p = read_bytes(&package(document())).unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for cell in p.document.cells() {
        let c = p.generate(cell, 500_000).unwrap();
        let bounds = p.document.cell_bounds(cell).unwrap();
        for t in c.triangles {
            assert!(t.vertices.iter().all(|p| bounds.contains([p[0], p[2]])));
        }
        for o in c.objects {
            assert!(ids.insert(o.id));
        }
    }
    assert!(!ids.is_empty());
}
#[test]
fn rejects_duplicate_traversal_and_case_collisions() {
    let b = package(document());
    for name in [
        "../escape",
        "/absolute",
        "a\\b",
        "a/../b",
        "C:/x",
        "NUL.png",
        "assets/a.",
    ] {
        rejected(&rewrite(&b, |e| e.push((name.into(), vec![]))), "E_PATH");
    }
    rejected(&rewrite(&b, |e| e.push(e[1].clone())), "E_PATH");
    rejected(
        &rewrite(&b, |e| e.push(("DOCUMENT.JSON".into(), vec![]))),
        "E_PATH",
    );
}
#[test]
fn rejects_corruption_unlisted_files_and_manifest_order() {
    let b = package(document());
    rejected(&rewrite(&b, |e| e[1].1.push(b' ')), "E_HASH");
    rejected(
        &rewrite(&b, |e| e.push(("extra.json".into(), b"{}".to_vec()))),
        "E_REFERENCE",
    );
    rejected(&rewrite(&b, |e| e.swap(0, 1)), "E_MANIFEST");
    rejected(
        &rewrite(&b, |e| {
            e.pop();
        }),
        "E_REFERENCE",
    );
}
#[test]
fn rejects_duplicate_json_keys() {
    let b = package(document());
    rejected(
        &rewrite(&b, |e| {
            let s = String::from_utf8(e[0].1.clone()).unwrap();
            e[0].1 = s
                .replacen(
                    "\"format\":\"memap\"",
                    "\"format\":\"memap\",\"format\":\"memap\"",
                    1,
                )
                .into_bytes();
        }),
        "E_JSON",
    );
}
#[test]
fn rejects_invalid_graph_polygon_and_extreme_values_without_panic() {
    let mut d = document();
    d.roads[0].from = "missing".into();
    assert!(d.validate().is_err());
    let mut d = document();
    d.buildings[0].footprint = vec![[0, 0], [100, 100], [0, 100], [100, 0]];
    assert!(d.validate().is_err());
    let mut d = document();
    d.bounds.min[0] = i64::MIN;
    assert!(d.validate().is_err());
    let mut d = document();
    d.nodes[0].id = d.nodes[1].id.clone();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
}
#[test]
fn clipping_preserves_shared_road_edge_heights() {
    let mut d = document();
    d.roads[0].points[1][1] = 231;
    d.roads[0].points[2][1] = 500;
    d.nodes
        .iter_mut()
        .find(|n| n.id == "east")
        .unwrap()
        .position[1] = 500;
    let p = read_bytes(&package(d)).unwrap();
    let edge = |c: Cell| {
        p.generate(c, 500_000)
            .unwrap()
            .triangles
            .into_iter()
            .filter(|t| t.object_id == "ground-road")
            .flat_map(|t| t.vertices)
            .filter(|v| v[0] == 51200)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(edge(Cell { x: 0, y: 0 }), edge(Cell { x: 1, y: 0 }));
}
#[test]
fn budget_and_map_edge_window() {
    let p = read_bytes(&package(document())).unwrap();
    assert_eq!(
        p.generate(Cell { x: 0, y: 0 }, 1).unwrap_err().code,
        "E_BUDGET"
    );
    assert_eq!(p.document.window([0, 0]).len(), 4);
    assert!(p.generate(Cell { x: 2, y: 0 }, 500_000).is_err());
}
fn height_png(value: u16) -> Vec<u8> {
    let mut bytes = vec![];
    {
        let mut enc = png::Encoder::new(&mut bytes, 257, 257);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Sixteen);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&value.to_be_bytes().repeat(257 * 257))
            .unwrap();
    }
    bytes
}
#[test]
fn heightmap_boundary_checks_and_decode() {
    let mut d = document();
    d.heightmaps.push(Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "terrain/0-0.png".into(),
        spacing_cm: 200,
        offset_cm: 0,
        step_cm: 1,
        source_accuracy_cm: Some(3000),
    });
    let files = BTreeMap::from([("terrain/0-0.png".into(), height_png(1))]);
    assert_eq!(pack_bytes(d.clone(), files).unwrap_err().code, "E_SEAM");
    let b = pack_bytes(
        d,
        BTreeMap::from([("terrain/0-0.png".into(), height_png(0))]),
    )
    .unwrap();
    let p = read_bytes(&b).unwrap();
    let occupied = p.generate_with_occupancy(Cell { x: 0, y: 0 }, 500_000, 2).unwrap();
    assert_eq!(occupied.chunk, p.generate(Cell { x: 0, y: 0 }, 500_000).unwrap());
    assert_eq!(occupied.solids.len(), 2);
    assert_eq!(p.generate_with_occupancy(Cell { x: 0, y: 0 }, 500_000, 1).unwrap_err().code, "E_BUDGET");
    assert_eq!(p.generate_with_occupancy(Cell { x: 0, y: 0 }, 500_000, MAX_OCCUPIED_SOLIDS + 1).unwrap_err().code, "E_BUDGET");
    assert!(
        p.generate(Cell { x: 0, y: 0 }, 500_000)
            .unwrap()
            .triangles
            .len()
            > 131000
    );
}
#[test]
fn release_gate_precedes_transfer() {
    let a = GameReleaseIdentity {
        game_release_id: "game-1".into(),
        execution_contract_hash: "a".repeat(64),
    };
    let mut b = a.clone();
    assert!(a.admits(&b).is_ok());
    b.game_release_id = "game-2".into();
    assert_eq!(a.admits(&b).unwrap_err().code, "E_RELEASE");
}
#[test]
fn external_asset_uri_is_rejected() {
    let json = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"https://example.com/x"}]}"#;
    let mut data = json.to_vec();
    while !data.len().is_multiple_of(4) {
        data.push(b' ');
    }
    let mut bytes = b"glTF".to_vec();
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(((20 + data.len()) as u32).to_le_bytes());
    bytes.extend((data.len() as u32).to_le_bytes());
    bytes.extend(b"JSON");
    bytes.extend(data);
    let mut d = document();
    d.assets.push(Asset { convex_collision: vec![], material: None,
        id: "custom".into(),
        path: "assets/custom.glb".into(),
        attribution: Attribution {
            source: "test".into(),
            license: "MIT".into(),
            notice: "".into(),
        },
        collision: vec![],
    });
    assert_eq!(
        pack_bytes(d, BTreeMap::from([("assets/custom.glb".into(), bytes)]))
            .unwrap_err()
            .code,
        "E_ASSET"
    );
}

#[test]
fn validation_budget_rejects_before_inflating_payloads() {
    let bytes = package(document());
    let cost = inspect_read_cost(&bytes).unwrap();
    let accepted = read_bytes_with_budget(&bytes, cost.validation_peak_bytes).unwrap();
    assert_eq!(
        accepted.inspection.retained_memory_bytes,
        cost.retained_memory_bytes
    );
    assert_eq!(
        accepted.inspection.world_content_hash,
        read_bytes(&bytes).unwrap().inspection.world_content_hash
    );
    assert_eq!(
        read_bytes_with_budget(&bytes, cost.validation_peak_bytes - 1)
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET"
    );
    let mut corrupt = bytes.clone();
    let position = {
        let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let entry = zip.by_index(1).unwrap();
        entry.data_start() as usize
    };
    corrupt[position] ^= 0xff;
    assert_eq!(inspect_read_cost(&corrupt).unwrap(), cost);
    assert_eq!(
        read_bytes_with_budget(&corrupt, cost.validation_peak_bytes - 1)
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET"
    );
    assert_ne!(
        read_bytes_with_budget(&corrupt, cost.validation_peak_bytes)
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET"
    );
}

fn slope_png(cell_x: usize, cell_y: usize, break_west: bool) -> Vec<u8> {
    let mut bytes = vec![];
    let mut samples = Vec::new();
    for y in 0..257 {
        for x in 0..257 {
            let height = (cell_x * 256
                + x
                + cell_y * 256
                + y
                + usize::from(break_west && x == 0 && y == 127)) as u16;
            samples.extend_from_slice(&height.to_be_bytes());
        }
    }
    {
        let mut encoder = png::Encoder::new(&mut bytes, 257, 257);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&samples)
            .unwrap();
    }
    bytes
}
#[test]
fn boundary_only_validation_preserves_both_seam_directions() {
    let mut d = document();
    let mut files = BTreeMap::new();
    for y in 0..2 {
        for x in 0..2 {
            let path = format!("terrain/{x}-{y}.png");
            d.heightmaps.push(Heightmap {
                cell: Cell { x, y },
                path: path.clone(),
                spacing_cm: 200,
                offset_cm: 0,
                step_cm: 1,
                source_accuracy_cm: None,
            });
            files.insert(path, slope_png(x as usize, y as usize, false));
        }
    }
    d.heightmaps.reverse();
    let bytes = pack_bytes(d.clone(), files.clone()).unwrap();
    let cost = inspect_read_cost(&bytes).unwrap();
    assert!(read_bytes_with_budget(&bytes, cost.validation_peak_bytes).is_ok());
    files.insert("terrain/1-0.png".into(), slope_png(1, 0, true));
    assert_eq!(pack_bytes(d, files).err().unwrap().code, "E_SEAM");
}

#[test]
fn recipes_are_explicit_preserved_and_manifest_bound() {
    let mut d: MapDocument=serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
    let bytes=package(d.clone()); let p=read_bytes(&bytes).unwrap();
    assert_eq!(p.manifest.recipe_version,2);
    assert_eq!(bytes,pack_bytes(p.document.clone(),p.files.clone()).unwrap());
    let hash=p.generate(Cell{x:0,y:0},500_000).unwrap().hash().unwrap();
    d.recipe_version=1; let old=read_bytes(&package(d)).unwrap();
    assert_eq!(old.manifest.recipe_version,1);
    assert_ne!(old.inspection.world_content_hash,p.inspection.world_content_hash);
    assert_ne!(old.generate(Cell{x:0,y:0},500_000).unwrap().hash().unwrap(),hash);
    let bad=rewrite(&bytes,|entries| {
        let (_,b)=entries.iter_mut().find(|(name,_)|name=="manifest.json").unwrap();
        let mut m:serde_json::Value=serde_json::from_slice(b).unwrap();m["recipe_version"]=1.into();*b=canonical(&m).unwrap();
    });
    rejected(&bad,"E_MANIFEST");
    let bad=rewrite(&bytes,|entries| {
        let (_,b)=entries.iter_mut().find(|(name,_)|name=="manifest.json").unwrap();
        let mut m:serde_json::Value=serde_json::from_slice(b).unwrap();m["recipe_version"]=(mapkit_core::RECIPE_VERSION+1).into();*b=canonical(&m).unwrap();
    });
    rejected(&bad,"E_VERSION");
}

#[test]
fn recipe_three_roundtrips_extensions_and_solid_content() {
    let d:MapDocument=serde_json::from_str(include_str!("../../../examples/placement/document.json")).unwrap();
    let bytes=package(d);let p=read_bytes(&bytes).unwrap();
    assert_eq!(p.manifest.recipe_version,3);
    assert_eq!(bytes,pack_bytes(p.document.clone(),p.files.clone()).unwrap());
    assert!(!p.document.repetitions.is_empty());
    let generated=p.generate(Cell{x:0,y:0},500_000).unwrap();
    assert!(!generated.building_prisms.is_empty());
    let mut changed=p.document.clone();changed.provenance.tool_id="another-tool".into();
    let other=read_bytes(&package(changed)).unwrap();
    assert_eq!(p.inspection.world_content_hash,other.inspection.world_content_hash);
    assert_eq!(generated.hash().unwrap(),other.generate(Cell{x:0,y:0},500_000).unwrap().hash().unwrap());
}

#[test]
fn recipe_five_courtyard_roundtrip_preserves_holes_and_content_identity() {
    let d: MapDocument=serde_json::from_str(include_str!("../../../examples/courtyard/document.json")).unwrap();
    let bytes=package(d.clone());let p=read_bytes(&bytes).unwrap();
    assert_eq!(p.document.buildings[0].holes,d.buildings[0].holes);
    assert_eq!(p.manifest.recipe_version,5);
    assert_eq!(bytes,pack_bytes(p.document.clone(),p.files.clone()).unwrap());
    let mut filled=d.clone();filled.buildings[0].holes.clear();let solid=read_bytes(&package(filled)).unwrap();
    assert_ne!(p.inspection.world_content_hash,solid.inspection.world_content_hash);
    assert_ne!(p.generate(Cell{x:0,y:0},500_000).unwrap().hash().unwrap(),solid.generate(Cell{x:0,y:0},500_000).unwrap().hash().unwrap());
}
