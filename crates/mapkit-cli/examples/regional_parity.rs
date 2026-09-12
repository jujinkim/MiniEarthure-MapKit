//! Compare fixed v1/v2 source artifacts without changing their authored worlds.
//! Old regional loads are a migration oracle, never an installation audit receipt.
use mapkit_core::*;
use mapkit_package::indexed::*;
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err(error(
            "E_USAGE",
            "regional_parity OLD NEW CELLS_JSON_OR_all",
        ));
    }
    const LIMIT: u64 = 1024 * 1024 * 1024;
    let mut old = IndexedReader::open_path(Path::new(&args[1]), LIMIT, None)?;
    let mut new = IndexedReader::open_path(Path::new(&args[2]), LIMIT, None)?;
    if old.index().world_content_hash != new.index().world_content_hash {
        return Err(error("E_PARITY", "authored world identity differs"));
    }
    let cells: Vec<Cell> = if args[3] == "all" {
        old.index()
            .regions
            .iter()
            .map(|r| r.cells.cells())
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect()
    } else {
        serde_json::from_str::<Vec<[i32; 2]>>(&args[3])
            .map_err(|e| error("E_USAGE", e.to_string()))?
            .into_iter()
            .map(|[x, y]| Cell { x, y })
            .collect()
    };
    let started = Instant::now();
    let mut results = Vec::new();
    for cell in cells {
        let id = old.region_for_cell(cell)?;
        let expected = {
            let source = old.load_region(id, LIMIT, &ReadEpoch::default().begin())?;
            source
                .package
                .generate_with_occupancy(cell, 500_000, MAX_OCCUPIED_SOLIDS)?
        }; // Release the old source before loading the new one.
        let id = new.region_for_cell(cell)?;
        let actual = {
            let source = new.load_region(id, LIMIT, &ReadEpoch::default().begin())?;
            source
                .package
                .generate_with_occupancy(cell, 500_000, MAX_OCCUPIED_SOLIDS)?
        };
        if expected.chunk != actual.chunk || expected.solids != actual.solids {
            return Err(error(
                "E_PARITY",
                format!("geometry/occupancy differs at {cell:?}"),
            ));
        }
        results.push(
            serde_json::json!({"cell":cell,"generated_sha256":actual.chunk.hash()?,
            "occupied_solids":actual.solids.len()}),
        );
    }
    println!(
        "{}",
        serde_json::json!({"world_content_hash":new.index().world_content_hash,
        "old_index":old.identity(),"new_index":new.identity(),"verified_cells":results.len(),
        "elapsed_seconds":started.elapsed().as_secs_f64(),"cells":results})
    );
    Ok(())
}
