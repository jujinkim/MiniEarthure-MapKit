use mapkit_core::*;
use mapkit_package::*;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}

fn package() -> Vec<u8> {
    pack_bytes(document(), BTreeMap::new()).unwrap()
}

fn rewrite(bytes: &[u8], edit: impl FnOnce(&mut Vec<(String, Vec<u8>)>)) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = vec![];
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let mut data = vec![];
        entry.read_to_end(&mut data).unwrap();
        entries.push((entry.name().to_string(), data));
    }
    edit(&mut entries);
    let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, data) in entries {
        writer
            .start_file(name, zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn rejects_disagreeing_local_and_central_headers_before_inflation() {
    let bytes = package();
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        let start = entry.header_start() as usize;
        // Name, flags, method, CRC, compressed size and expanded size.
        for offset in [30, 6, 8, 14, 18, 22] {
            let mut bad = bytes.clone();
            bad[start + offset] ^= 1;
            assert_eq!(
                inspect_read_cost(&bad).unwrap_err().code,
                "E_ZIP",
                "entry={index} offset={offset}"
            );
            assert_eq!(read_bytes(&bad).err().unwrap().code, "E_ZIP");
        }
    }
}

#[test]
fn exporter_applies_manifest_limit_before_returning_bytes() {
    let mut doc = document();
    doc.attributions[0].notice = "x".repeat(MAX_MANIFEST_BYTES as usize);
    assert_eq!(
        pack_bytes(doc, BTreeMap::new()).unwrap_err().code,
        "E_LIMIT"
    );
}

#[test]
fn inventory_sizes_digests_and_exact_reference_set_are_binding() {
    let bytes = package();
    for (label, code) in [
        ("size", "E_HASH"),
        ("digest", "E_HASH"),
        ("world", "E_HASH"),
        ("duplicate", "E_REFERENCE"),
        ("missing", "E_REFERENCE"),
        ("extra", "E_REFERENCE"),
        ("self", "E_REFERENCE"),
        ("case", "E_REFERENCE"),
    ] {
        let bad = rewrite(&bytes, |entries| {
            let mut m: serde_json::Value = serde_json::from_slice(&entries[0].1).unwrap();
            match label {
                "size" => m["files"][0]["size"] = 1.into(),
                "digest" => m["files"][0]["sha256"] = "0".repeat(64).into(),
                "world" => m["world_content_hash"] = "0".repeat(64).into(),
                "duplicate" => {
                    let record = m["files"][0].clone();
                    m["files"].as_array_mut().unwrap().push(record);
                }
                "missing" => {
                    m["files"].as_array_mut().unwrap().clear();
                }
                "extra" => {
                    m["files"].as_array_mut().unwrap().push(
                        serde_json::json!({"path":"extra.json","size":2,"sha256":sha256(b"{}")}),
                    );
                    entries.push(("extra.json".into(), b"{}".to_vec()));
                }
                "self" => m["files"][0]["path"] = "manifest.json".into(),
                "case" => m["files"][0]["path"] = "DOCUMENT.JSON".into(),
                _ => unreachable!(),
            }
            entries[0].1 = canonical(&m).unwrap();
        });
        assert_eq!(read_bytes(&bad).err().unwrap().code, code, "{label}");
    }
}

#[test]
fn noncanonical_source_and_container_repack_to_the_same_export() {
    let bytes = package();
    let different = rewrite(&bytes, |entries| {
        let mut doc = document();
        doc.nodes.reverse();
        doc.roads.reverse();
        entries[1].1 = serde_json::to_vec_pretty(&doc).unwrap();
        let mut m: PackageManifest = serde_json::from_slice(&entries[0].1).unwrap();
        m.files[0].size = entries[1].1.len() as u64;
        m.files[0].sha256 = sha256(&entries[1].1);
        entries[0].1 = serde_json::to_vec_pretty(&m).unwrap();
    });
    assert_ne!(bytes, different);
    let original = read_bytes(&bytes).unwrap();
    let read = read_bytes(&different).unwrap();
    assert_eq!(
        original.inspection.world_content_hash,
        read.inspection.world_content_hash
    );
    for cell in original.document.cells() {
        assert_eq!(
            original.generate(cell, 500_000).unwrap(),
            read.generate(cell, 500_000).unwrap()
        );
    }
    assert_eq!(pack_bytes(read.document, read.files).unwrap(), bytes);
}

#[test]
fn normalized_export_keeps_the_existing_v1_bytes_and_hashes() {
    let bytes = package();
    assert_eq!(
        sha256(&bytes),
        "3e72644cafa75a526cc5bbc82d71e2d872087d585d1e0a16bf52c275b1045244"
    );
    assert_eq!(
        read_bytes(&bytes).unwrap().inspection.world_content_hash,
        "cbdd26b1dec24f6ec71beee23adc528bce8105bc31912581cb2853d6e2aef776"
    );
}
