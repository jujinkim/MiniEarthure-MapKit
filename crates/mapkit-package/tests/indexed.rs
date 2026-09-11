use mapkit_core::*;
use mapkit_package::{indexed::*, *};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

fn empty() -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();
    d.roads.clear();
    d.zones.clear();
    d.buildings.clear();
    d.bounds = Bounds {
        min: [-100, -200],
        max: [12_701, 6_201],
    };
    d.cell_size_cm = 1600;
    d.recipe_version = 6;
    d
}
fn ticket() -> ReadTicket {
    ReadEpoch::default().begin()
}
const BUDGET: u64 = 4 * 1024 * 1024 * 1024;
#[derive(Clone)]
struct Observed {
    bytes: Cursor<Vec<u8>>,
    reads: Arc<Mutex<Vec<(u64, usize)>>>,
    cancel: Option<(ReadEpoch, u64)>,
}
impl Read for Observed {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let at = self.bytes.position();
        let n = self.bytes.read(buf)?;
        self.reads.lock().unwrap().push((at, n));
        if self
            .cancel
            .as_ref()
            .is_some_and(|(_, threshold)| at >= *threshold)
        {
            self.cancel.take().unwrap().0.cancel();
        }
        Ok(n)
    }
}

fn replace_record(bytes: &[u8], id: usize, replacement: &[u8], trailing: bool) -> Vec<u8> {
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let mut index: Index = serde_json::from_slice(&bytes[48..48 + n]).unwrap();
    let mut payload = vec![];
    for (i, r) in index.records.iter_mut().enumerate() {
        let mut compressed = if i == id {
            let mut encoder =
                flate2::write::DeflateEncoder::new(vec![], flate2::Compression::new(9));
            encoder.write_all(replacement).unwrap();
            encoder.finish().unwrap()
        } else {
            bytes[48 + n + r.offset as usize..48 + n + (r.offset + r.compressed_bytes) as usize]
                .to_vec()
        };
        if i == id {
            r.size = replacement.len() as u64;
            r.sha256 = sha256(replacement);
            if trailing {
                compressed.push(0);
            }
        }
        r.offset = payload.len() as u64;
        r.compressed_bytes = compressed.len() as u64;
        payload.extend_from_slice(&compressed);
    }
    index.payload_bytes = payload.len() as u64;
    let body = canonical(&index).unwrap();
    let mut out = b"MKREGN01".to_vec();
    out.extend_from_slice(&(body.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(&body));
    out.extend_from_slice(&body);
    out.extend_from_slice(&payload);
    out
}

#[test]
fn spatial_pruning_keeps_seam_buildings_manual_proxies_and_vegetation_competitors() {
    let mut d = empty();
    let template: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    for (i, x) in [3100, 9000].into_iter().enumerate() {
        let mut b = template.buildings[0].clone();
        b.id = format!("seam-{i}");
        b.footprint = vec![[x, 2000], [x + 400, 2000], [x + 400, 2400], [x, 2400]];
        d.buildings.push(b);
    }
    d.placements.push(Placement {
        id: "fence".into(),
        asset_id: "builtin:fence".into(),
        position: [9600, 0, 4500],
        quarter_turns: 0,
    });
    let mut zone = template.zones[0].clone();
    zone.polygon = vec![[100, 100], [12000, 100], [12000, 5900], [100, 5900]];
    zone.spacing_cm = 1200;
    zone.density_per_mille = 1000;
    d.zones.push(zone);
    let original = read_bytes(&pack_bytes(d.clone(), BTreeMap::new()).unwrap()).unwrap();
    let bytes = pack_source(d.clone(), BTreeMap::new(), 2).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(bytes), BUDGET, None).unwrap();
    let first = reader.load_region(0, BUDGET, &ticket()).unwrap();
    assert!(first.package.document.buildings.len() < d.buildings.len());
    assert!(first.package.document.placements.is_empty());
    for cell in d.cells() {
        let id = reader.region_for_cell(cell).unwrap();
        let region = reader.load_region(id, BUDGET, &ticket()).unwrap();
        let a = original
            .generate_with_occupancy(cell, 500_000, 200_000)
            .unwrap();
        let b = region
            .package
            .generate_with_occupancy(cell, 500_000, 200_000)
            .unwrap();
        assert_eq!(a.chunk, b.chunk, "cell {cell:?}");
        assert_eq!(a.solids, b.solids);
    }
}

#[test]
fn cancellation_during_io_duplicate_json_and_compression_tails_are_rejected() {
    let bytes = pack_source(empty(), BTreeMap::new(), 2).unwrap();
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let epoch = ReadEpoch::default();
    let mut reader = IndexedReader::open(
        Observed {
            bytes: Cursor::new(bytes.clone()),
            reads: Arc::new(Mutex::new(vec![])),
            cancel: Some((epoch.clone(), 48 + n)),
        },
        BUDGET,
        None,
    )
    .unwrap();
    assert_eq!(
        reader
            .load_region(0, BUDGET, &epoch.begin())
            .err()
            .unwrap()
            .code,
        "E_CANCELLED"
    );
    let region = reader.load_region(0, BUDGET, &epoch.begin()).unwrap();
    let id = reader.index().regions[0].source;
    let mut body = String::from_utf8(region.package.files["document.json"].clone()).unwrap();
    body.insert_str(1, "\"map_id\":\"duplicate\",");
    let malformed = replace_record(&bytes, id, body.as_bytes(), false);
    let mut bad = IndexedReader::open(Cursor::new(malformed), BUDGET, None).unwrap();
    assert_eq!(
        bad.load_region(0, BUDGET, &ticket()).err().unwrap().code,
        "E_JSON"
    );
    let tail = replace_record(&bytes, id, &region.package.files["document.json"], true);
    let mut bad = IndexedReader::open(Cursor::new(tail), BUDGET, None).unwrap();
    assert_eq!(
        bad.load_region(0, BUDGET, &ticket()).err().unwrap().code,
        "E_INDEX"
    );
    // A changed but independently valid snapshot is detectable by the explicit
    // full dependency audit, even when its record inventory was rehashed honestly.
    let mut document = region.package.document.to_document();
    document.terrain_base_cm += 1;
    let modified = replace_record(&bytes, id, &canonical(&document).unwrap(), false);
    let mut bad = IndexedReader::open(Cursor::new(modified), BUDGET, None).unwrap();
    assert!(bad.load_region(0, BUDGET, &ticket()).is_err()); // global metadata mismatch
}

fn terrain_png(samples: &[u16]) -> Vec<u8> {
    let mut bytes = vec![];
    {
        let mut e = png::Encoder::new(&mut bytes, 3, 3);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Sixteen);
        e.write_header()
            .unwrap()
            .write_image_data(
                &samples
                    .iter()
                    .flat_map(|n| n.to_be_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    bytes
}
#[test]
fn terrain_neighbors_validate_locally_and_shared_payloads_are_stored_once() {
    let mut d = empty();
    d.cell_size_cm = 400;
    d.bounds = Bounds {
        min: [-13, -17],
        max: [1500, 1490],
    };
    let mut files = BTreeMap::new();
    for cell in d.cells() {
        let h = Heightmap {
            cell,
            path: format!("terrain/{}-{}.png", cell.x, cell.y),
            spacing_cm: 200,
            offset_cm: -1000,
            step_cm: 1,
            source_accuracy_cm: None,
        };
        let samples = (0..3)
            .flat_map(|y| (0..3).map(move |x| (100 + cell.x * 2 + x + cell.y * 2 + y) as u16))
            .collect::<Vec<_>>();
        files.insert(h.path.clone(), terrain_png(&samples));
        d.heightmaps.push(h);
    }
    let old = read_bytes(&pack_bytes(d.clone(), files.clone()).unwrap()).unwrap();
    let bytes = pack_source(d.clone(), files.clone(), 1).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(bytes.clone()), BUDGET, None).unwrap();
    assert_eq!(reader.index().payloads.len(), 16);
    assert_eq!(reader.index().records.len(), 1 + 16 + 16);
    let first = reader.load_region(0, BUDGET, &ticket()).unwrap();
    assert_eq!(first.package.document.heightmaps.len(), 4); // current, two neighbors, shared corner
    for cell in d.cells() {
        let id = reader.region_for_cell(cell).unwrap();
        let r = reader.load_region(id, BUDGET, &ticket()).unwrap();
        assert_eq!(
            old.generate(cell, 500_000).unwrap(),
            r.package.generate(cell, 500_000).unwrap()
        );
    }
    let id = reader.index().payloads["terrain/1-0.png"];
    let bad = replace_record(&bytes, id, &terrain_png(&[999; 9]), false);
    let mut bad = IndexedReader::open(Cursor::new(bad), BUDGET, None).unwrap();
    assert_eq!(
        bad.load_region(0, BUDGET, &ticket()).err().unwrap().code,
        "E_SEAM"
    );
    // A remote intact source still loads without reading this damaged neighbor.
    assert!(bad.load_region(15, BUDGET, &ticket()).is_ok());
    assert_eq!(
        old.generate(Cell { x: 0, y: 0 }, 500_000).unwrap(),
        first
            .package
            .generate(Cell { x: 0, y: 0 }, 500_000)
            .unwrap()
    );
}
impl Seek for Observed {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(p)
    }
}
fn mutate_index(bytes: &[u8], f: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let mut index: serde_json::Value = serde_json::from_slice(&bytes[48..48 + n]).unwrap();
    f(&mut index);
    let body = canonical(&index).unwrap();
    let mut out = b"MKREGN01".to_vec();
    out.extend_from_slice(&(body.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(&body));
    out.extend_from_slice(&body);
    out.extend_from_slice(&bytes[48 + n..]);
    out
}

#[test]
fn index_only_open_and_local_reads_have_exact_bounded_spans() {
    let bytes = pack_source(empty(), BTreeMap::new(), 2).unwrap();
    let reads = Arc::new(Mutex::new(vec![]));
    let mut reader = IndexedReader::open(
        Observed {
            bytes: Cursor::new(bytes.clone()),
            reads: reads.clone(),
            cancel: None,
        },
        BUDGET,
        None,
    )
    .unwrap();
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    assert_eq!(*reads.lock().unwrap(), vec![(0, 48), (48, n as usize)]);
    let source = reader.index().regions[0].source;
    let record = reader.index().records[source].clone();
    reads.lock().unwrap().clear();
    let cost = reader.region_cost(0).unwrap();
    assert_eq!(
        reader
            .load_region(0, cost.validation_peak_bytes - 1, &ticket())
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET"
    );
    assert!(reads.lock().unwrap().is_empty());
    let region = reader
        .load_region(0, cost.validation_peak_bytes, &ticket())
        .unwrap();
    assert_eq!(region.bytes_read, record.compressed_bytes);
    assert_eq!(
        reads
            .lock()
            .unwrap()
            .iter()
            .map(|(_, n)| *n as u64)
            .sum::<u64>(),
        record.compressed_bytes
    );
    assert_eq!(reads.lock().unwrap()[0].0, 48 + n + record.offset);
    assert_eq!(
        region
            .package
            .generate(Cell { x: 2, y: 0 }, 500_000)
            .unwrap_err()
            .code,
        "E_CELL"
    );
}

#[test]
fn larger_world_uses_bounded_regions_without_relaxing_legacy_validation() {
    let mut d = empty();
    d.bounds.max = [1_000_000, 1_000_000];
    d.bounds.min = [0, 0];
    assert_eq!(d.cell_count().unwrap(), 390_625);
    assert_eq!(d.validate().unwrap_err().code, "E_LIMIT");
    assert!(d.cells().is_empty());
    assert!(!d.has_cell(Cell { x: 0, y: 0 }));
    assert_eq!(
        pack_bytes(d.clone(), BTreeMap::new()).unwrap_err().code,
        "E_LIMIT"
    );
    // Actual 128m storage / 16m execution topology. This empty source is a
    // topology/IO test, never representative density or platform acceptance.
    let bytes = pack_source(d, BTreeMap::new(), 8).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(bytes), BUDGET, None).unwrap();
    assert_eq!(reader.index().regions.len(), 6241);
    assert_eq!(
        reader.region_for_cell(Cell { x: 624, y: 624 }).unwrap(),
        6240
    );
    let region = reader.load_region(6240, BUDGET, &ticket()).unwrap();
    assert_eq!(
        region
            .package
            .generate(Cell { x: 624, y: 624 }, 500_000)
            .unwrap()
            .cell,
        Cell { x: 624, y: 624 }
    );
    assert!(region.package.document.cells().is_empty());
    assert_eq!(
        region.package.document.validate().unwrap_err().code,
        "E_LIMIT"
    );
    // A serialized capability cannot bypass the old document profile.
    let raw: MapDocument =
        serde_json::from_slice(&canonical(&region.package.document).unwrap()).unwrap();
    assert!(!raw.has_cell(Cell { x: 0, y: 0 }));
}

#[test]
fn source_region_partition_preserves_all_cells_collision_occupancy_and_hashes() {
    for name in ["minimal", "roads", "placement", "courtyard", "assets"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name);
        let (d, files) = read_project(&path).unwrap();
        let original = read_bytes(&pack_bytes(d.clone(), files.clone()).unwrap()).unwrap();
        let bytes = pack_source(d.clone(), files.clone(), 1).unwrap();
        assert_eq!(
            bytes,
            pack_source(d, files, 1).unwrap(),
            "reproducible {name}"
        );
        let mut reader = IndexedReader::open(Cursor::new(bytes), BUDGET, None).unwrap();
        reader.audit(BUDGET, &ticket()).unwrap();
        for c in original.document.cells().into_iter().rev() {
            let id = reader.region_for_cell(c).unwrap();
            let region = reader.load_region(id, BUDGET, &ticket()).unwrap();
            let a = original
                .generate_with_occupancy(c, 500_000, 200_000)
                .unwrap();
            let b = region
                .package
                .generate_with_occupancy(c, 500_000, 200_000)
                .unwrap();
            assert_eq!(
                a.chunk.hash().unwrap(),
                b.chunk.hash().unwrap(),
                "{name} {c:?}"
            );
            assert_eq!(a.solids, b.solids, "occupancy {name} {c:?}");
        }
    }
}

#[test]
fn independent_damage_cancellation_and_late_results_never_mutate_live_snapshot() {
    let bytes = pack_source(empty(), BTreeMap::new(), 2).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(bytes.clone()), BUDGET, None).unwrap();
    let epoch = ReadEpoch::default();
    let old = reader.load_region(0, BUDGET, &epoch.begin()).unwrap();
    let cell = Cell { x: 0, y: 0 };
    let hash = old.package.generate(cell, 500_000).unwrap().hash().unwrap();
    let late = reader.load_region(1, BUDGET, &epoch.begin()).unwrap();
    epoch.cancel();
    assert_eq!(late.check_candidate().unwrap_err().code, "E_CANCELLED");
    assert_eq!(
        old.package.generate(cell, 500_000).unwrap().hash().unwrap(),
        hash
    );
    let cancelled = epoch.begin();
    epoch.cancel();
    assert_eq!(
        reader
            .load_region(0, BUDGET, &cancelled)
            .err()
            .unwrap()
            .code,
        "E_CANCELLED"
    );
    let other = ReadEpoch::default();
    assert!(reader.load_region(0, BUDGET, &other.begin()).is_ok());
    let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let r = reader.index().records[reader.index().regions[1].source].clone();
    let mut damaged = bytes.clone();
    damaged[(48 + n + r.offset) as usize] ^= 0xff;
    let mut damaged =
        IndexedReader::open(Cursor::new(damaged), BUDGET, Some(reader.identity())).unwrap();
    assert!(damaged.load_region(0, BUDGET, &ticket()).is_ok());
    assert!(damaged.load_region(1, BUDGET, &ticket()).is_err());
    assert!(damaged.audit(BUDGET, &ticket()).is_err());
    assert_eq!(
        old.package.generate(cell, 500_000).unwrap().hash().unwrap(),
        hash
    );
}

#[test]
fn malformed_indexes_reject_before_any_payload_read() {
    let bytes = pack_source(empty(), BTreeMap::new(), 2).unwrap();
    let edits: Vec<Box<dyn Fn(&mut serde_json::Value)>> = vec![
        Box::new(|v| v["version"] = 2.into()),
        Box::new(|v| v["records"][0]["offset"] = 1.into()),
        Box::new(|v| v["records"][1]["offset"] = 0.into()),
        Box::new(|v| v["records"][0]["size"] = (MAX_DOCUMENT_BYTES + 1).into()),
        Box::new(|v| v["regions"][1]["cells"] = v["regions"][0]["cells"].clone()),
        Box::new(|v| v["regions"][0]["source"] = v["authoring_source"].clone()),
        Box::new(|v| v["side_cells"] = 0.into()),
        Box::new(|v| v["payloads"]["../bad"] = 0.into()),
        Box::new(|v| v["regions"][0]["payloads"]["assets/missing.png"] = 1.into()),
        Box::new(|v| v["extra"] = 1.into()),
        Box::new(|v| v["records"][0]["sha256"] = "A".repeat(64).into()),
    ];
    for edit in edits {
        let malformed = mutate_index(&bytes, edit);
        let n = u64::from_le_bytes(malformed[8..16].try_into().unwrap());
        let reads = Arc::new(Mutex::new(vec![]));
        assert!(IndexedReader::open(
            Observed {
                bytes: Cursor::new(malformed),
                reads: reads.clone(),
                cancel: None
            },
            BUDGET,
            None
        )
        .is_err());
        assert!(reads
            .lock()
            .unwrap()
            .iter()
            .all(|(p, size)| p + *size as u64 <= 48 + n));
    }
    let mut tail = bytes.clone();
    tail.push(0);
    assert!(IndexedReader::open(Cursor::new(tail), BUDGET, None).is_err());
    assert!(
        IndexedReader::open(Cursor::new(bytes.clone()), BUDGET, Some(&"0".repeat(64))).is_err()
    );
    assert!(IndexedReader::open(Cursor::new(bytes), 1, None).is_err());
}

#[test]
fn authoring_roundtrip_preserves_every_payload_and_refuses_existing_destination() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets");
    let (d, files) = read_project(&path).unwrap();
    let bytes = pack_source(d, files.clone(), 1).unwrap();
    let mut reader = IndexedReader::open(Cursor::new(bytes.clone()), BUDGET, None).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "mapkit-indexed-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let destination = directory.join("restored");
    reader
        .unpack_source(&destination, BUDGET, &ticket())
        .unwrap();
    let (restored, restored_files) = read_source_project(&destination).unwrap();
    assert_eq!(files, restored_files);
    assert_eq!(bytes, pack_source(restored, restored_files, 1).unwrap());
    assert!(reader
        .unpack_source(&destination, BUDGET, &ticket())
        .is_err());
    assert_eq!(
        std::fs::read(destination.join("document.json")).unwrap(),
        files["document.json"]
    );
    let artifact = directory.join("source.mkregions");
    std::fs::write(&artifact, &bytes).unwrap();
    let mut live = IndexedReader::open_path(&artifact, BUDGET, None).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&artifact)
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert_eq!(
        live.load_region(0, BUDGET, &ticket()).err().unwrap().code,
        "E_INDEX"
    );
    std::fs::remove_dir_all(directory).unwrap(); // exclusively created by this test
}
