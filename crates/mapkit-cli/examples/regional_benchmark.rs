//! Same-artifact audit/source/generation timing. Not an RSS or gameplay benchmark.
use mapkit_core::*;
use mapkit_package::indexed::*;
use std::{path::Path, time::Instant};

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(error("E_USAGE", "regional_benchmark PACKAGE CELLS_JSON"));
    }
    let cells = serde_json::from_str::<Vec<[i32; 2]>>(&args[2])
        .map_err(|e| error("E_USAGE", e.to_string()))?;
    if cells.len() > 16_384 {
        return Err(error("E_BUDGET", "benchmark cell limit exceeded"));
    }
    const LIMIT: u64 = 1024 * 1024 * 1024;
    let started = Instant::now();
    let mut reader = IndexedReader::open_path(Path::new(&args[1]), LIMIT, None)?;
    let open_ms = milliseconds(started);
    let mut audit_profile = ReadProfile::enabled();
    let started = Instant::now();
    let audit = reader.audit_summary_profiled(LIMIT, &ReadEpoch::default().begin(), &mut audit_profile)?;
    let audit_ms = milliseconds(started);
    println!("{}", serde_json::json!({"stage":"audit", "package":args[1],
        "package_sha256":audit["package_sha256"], "index_sha256":reader.identity(),
        "world_content_hash":audit["world_content_hash"], "regions":reader.index().regions.len(),
        "open_ms":open_ms, "audit_ms":audit_ms, "profile":audit_profile,
        "validation_peak_bytes":audit["validation_peak_bytes"]}));
    let mut current: Option<(usize, RegionSnapshot)> = None;
    for [x, y] in cells {
        let cell = Cell { x, y };
        let region = reader.region_for_cell(cell)?;
        if current.as_ref().map(|(id, _)| *id) != Some(region) {
            // Mirror the serial consumer's single source lifetime. A profile is
            // never an audit receipt or permission to retain a second snapshot.
            drop(current.take());
            let mut profile = ReadProfile::enabled();
            let started = Instant::now();
            let source = reader.load_region_profiled(region, LIMIT, &ReadEpoch::default().begin(), &mut profile)?;
            println!("{}", serde_json::json!({"stage":"source", "region":region,
                "cells":source.cells, "load_ms":milliseconds(started), "profile":profile,
                "bytes_read":source.bytes_read, "region_identity":source.identity,
                "retained_memory_bytes":source.package.inspection.retained_memory_bytes,
                "validation_peak_bytes":source.package.inspection.validation_peak_bytes}));
            current = Some((region, source));
        }
        let source = &current.as_ref().unwrap().1;
        let started = Instant::now();
        let cost = source.package.document.estimate(cell, 500_000)?;
        let estimate_ms = milliseconds(started);
        let started = Instant::now();
        let generated = source.package.generate_with_occupancy(cell, 500_000, MAX_OCCUPIED_SOLIDS)?;
        let generation_ms = milliseconds(started);
        let started = Instant::now();
        let hash = generated.chunk.hash()?;
        println!("{}", serde_json::json!({"stage":"cell", "cell":cell, "region":region,
            "estimate_ms":estimate_ms, "generation_ms":generation_ms,
            "hash_ms":milliseconds(started), "generated_sha256":hash,
            "triangles":generated.chunk.triangles.len(), "objects":generated.chunk.objects.len(),
            "occupied_solids":generated.solids.len(), "estimated_triangles":cost.triangles}));
    }
    Ok(())
}
