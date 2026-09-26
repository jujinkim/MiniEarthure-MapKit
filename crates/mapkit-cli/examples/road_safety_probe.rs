//! Focused affected-cell and authored-start check. No driving or full-map gate.
use mapkit_core::*;
use std::{collections::BTreeSet, env, path::Path};
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let directory = env::args().nth(1).ok_or("source directory required")?;
    let mut failures = 0;
    for id in [
        "haeon", "belmont", "nord", "safra", "red-wadi", "kanupi", "bansai",
    ] {
        let root = Path::new(&directory);
        let p = mapkit_package::read(&root.join(format!("{id}.memap")))?;
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(id).join("region.json"))?)?;
        let mut cells = BTreeSet::new();
        for r in &p.document.roads {
            for point in &r.points {
                if let Some(cell) = p.document.cell_at([point[0], point[2]]) {
                    cells.insert(cell);
                }
            }
            if matches!(r.kind, RoadKind::Bridge | RoadKind::Elevated) {
                for s in r.points.windows(2) {
                    let steps = ((s[1][0] - s[0][0]).abs().max((s[1][2] - s[0][2]).abs())
                        / p.document.cell_size_cm as i64
                        + 1)
                    .max(1);
                    for i in 0..=steps {
                        let point = [
                            s[0][0] + (s[1][0] - s[0][0]) * i / steps,
                            s[0][2] + (s[1][2] - s[0][2]) * i / steps,
                        ];
                        if let Some(cell) = p.document.cell_at(point) {
                            cells.insert(cell);
                        }
                    }
                }
            }
        }
        let mut starts = Vec::new();
        for route in meta["routes"].as_array().ok_or("routes")? {
            let s = &route["start"];
            let point = [s["x_cm"].as_i64().unwrap(), s["y_cm"].as_i64().unwrap()];
            let cell = p.document.cell_at(point).ok_or("start outside map")?;
            cells.insert(cell);
            starts.push((cell, point, s["surface_id"].as_str().unwrap()));
        }
        let mut solids = 0;
        let mut maximum = 0;
        for cell in &cells {
            let cost = p.document.estimate(*cell, 500000)?;
            let generated = match p.generate_with_occupancy(*cell, 500000, 200000) {
                Ok(v) => v,
                Err(e) => {
                    println!("ROAD_FAILURE {id} {cell:?} {e}");
                    failures += 1;
                    continue;
                }
            };
            assert!(generated.solids.len() as u64 <= cost.occupied_solids);
            assert!(generated.chunk.asset_convexes.len() as u64 <= cost.asset_convexes);
            let _ = p.document.road_paint(&p.document.cell_bounds(*cell)?)?;
            for (_, point, surface) in starts.iter().filter(|s| s.0 == *cell) {
                generated.chunk.spawn(&SpawnRequest {
                    position_cm: *point,
                    surface_id: surface.to_string(),
                })?;
            }
            solids += generated.chunk.asset_convexes.len();
            maximum = maximum.max(generated.chunk.triangles.len());
        }
        println!(
            "ROAD_SAFETY {id} cells={} starts={} safety_convexes={} max_triangles={maximum}",
            cells.len(),
            starts.len(),
            solids
        );
    }
    if failures > 0 {
        return Err(format!("{failures} affected cells failed").into());
    }
    Ok(())
}
