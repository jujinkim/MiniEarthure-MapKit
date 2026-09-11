//! Standalone source-storage evidence. Not a gameplay/RSS/GPU benchmark.
use mapkit_core::*;
use mapkit_package::{indexed::*, *};
use std::{collections::BTreeMap, io::Cursor, path::Path, time::Instant};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err(error(
            "E_USAGE",
            "regional_compare OLD.memap NEW.mkregions SIDE_CELLS",
        ));
    }
    let original = read(Path::new(&args[1]))?;
    let side = args[3]
        .parse()
        .map_err(|_| error("E_USAGE", "integer side"))?;
    let start = Instant::now();
    let bytes = pack_source(
        original.document.to_document(),
        original.files.clone(),
        side,
    )?;
    let export_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let mut reader = IndexedReader::open(Cursor::new(&bytes), 512 * 1024 * 1024, None)?;
    let index_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut results = Vec::new();
    let mut combined = BTreeMap::new();
    for id in 0..reader.index().regions.len() {
        let start = Instant::now();
        let source = reader.load_region(id, 512 * 1024 * 1024, &ReadEpoch::default().begin())?;
        let load_ms = start.elapsed().as_secs_f64() * 1000.0;
        for cell in source.cells.cells()? {
            let expected = original.generate_with_occupancy(cell, 500_000, MAX_OCCUPIED_SOLIDS)?;
            let actual =
                source
                    .package
                    .generate_with_occupancy(cell, 500_000, MAX_OCCUPIED_SOLIDS)?;
            if expected.chunk != actual.chunk || expected.solids != actual.solids {
                return Err(error("E_PARITY", format!("region {id}, cell {cell:?}")));
            }
            combined.insert(format!("{},{}", cell.x, cell.y), actual.chunk.hash()?);
        }
        results.push(serde_json::json!({"region": id, "cells": source.cells,
            "source_bytes_read": source.bytes_read, "load_ms": load_ms,
            "retained_bytes": source.package.inspection.retained_memory_bytes,
            "validation_peak_bytes": source.package.inspection.validation_peak_bytes,
            "source_document_bytes": source.package.files["document.json"].len(),
            "source_objects": {"buildings": source.package.document.buildings.len(),
                "placements": source.package.document.placements.len(), "roads": source.package.document.roads.len()}}));
    }
    reader.audit(4 * 1024 * 1024 * 1024, &ReadEpoch::default().begin())?;
    let report = serde_json::json!({"verified_cells": combined.len(), "generated_set_sha256": sha256(&canonical(&combined)?),
        "original_package_bytes": original.inspection.package_bytes, "indexed_package_bytes": bytes.len(),
        "original_source_document_bytes": original.files["document.json"].len(), "original_cost": original.inspection,
        "index_sha256": reader.identity(), "index_retained_bytes": reader.index_retained_bytes(),
        "payload_count": reader.index().payloads.len(), "payload_expanded_bytes": reader.index().payloads.values().map(|id|reader.index().records[*id].size).sum::<u64>(),
        "export_ms": export_ms, "index_open_ms": index_ms, "regions": results});
    write_new(Path::new(&args[2]), &bytes)?;
    println!("{report}");
    Ok(())
}
