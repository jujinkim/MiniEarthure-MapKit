//! The caller owns reservations, worker join and serialized collision commit.
use super::*;
use mapkit_package::indexed::{IndexedReader, ReadEpoch};
use std::{fs::File, sync::Mutex};

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct MapKitRegionReader {
    base: Base<RefCounted>,
    reader: Option<Mutex<IndexedReader<File>>>,
    epoch: ReadEpoch,
    world: Option<mapkit_core::MapDocument>,
    regions: Vec<serde_json::Value>,
}
#[godot_api]
impl IRefCounted for MapKitRegionReader {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            reader: None,
            epoch: ReadEpoch::default(),
            world: None,
            regions: Vec::new(),
        }
    }
}
#[godot_api]
impl MapKitRegionReader {
    /// Initialize before sharing with a worker. Reopening requires caller join.
    #[func]
    fn open_index(&mut self, path: GString, memory_limit: i64, expected_index: GString) -> GString {
        self.epoch.cancel();
        self.reader = None;
        self.world = None;
        self.regions.clear();
        response((|| {
            let expected = expected_index.to_string();
            let reader = IndexedReader::open_path(
                Path::new(&path.to_string()),
                memory_limit.max(0) as u64,
                if expected.is_empty() {
                    None
                } else {
                    Some(&expected)
                },
            )?;
            if reader.index_retained_bytes() * 2 > memory_limit.max(0) as u64 {
                return Err(mapkit_core::error(
                    "E_MEMORY_BUDGET",
                    "index query metadata exceeds allowance",
                ));
            }
            let index = reader.index();
            let result = serde_json::json!({"index_sha256": reader.identity(),
                "world_content_hash": index.world_content_hash, "bounds": index.world.bounds,
                "cell_size_cm": index.world.cell_size_cm, "side_cells": index.side_cells,
                "region_count": index.regions.len(), "execution_cells": index.world.cell_count()?,
                "retained_memory_bytes": reader.index_retained_bytes() * 2, "verification": "index-only"});
            self.world = Some(index.world.clone());
            self.regions = (0..index.regions.len()).map(|id| Ok(serde_json::json!({
                "region": id, "cells": index.regions[id].cells, "cost": reader.region_cost(id)?
            }))).collect::<mapkit_core::Result<Vec<_>>>()?;
            self.reader = Some(Mutex::new(reader));
            Ok(result)
        })())
    }
    #[func]
    fn begin_request(&self) -> i64 {
        self.epoch.begin().generation() as i64
    }
    #[func]
    fn cancel_request(&self) {
        self.epoch.cancel();
    }
    #[func]
    fn request_is_current(&self, generation: i64) -> bool {
        generation > 0 && self.epoch.ticket(generation as u64).check().is_ok()
    }
    #[func]
    fn region_for_cell(&self, x: i32, y: i32) -> GString {
        response((|| {
            let world = self
                .world
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?;
            if !world.has_cell(Cell { x, y }) {
                return Err(mapkit_core::error("E_CELL", "cell outside indexed world"));
            }
            self.regions
                .iter()
                .find(|r| {
                    let min = &r["cells"]["min"];
                    let end = &r["cells"]["end"];
                    i64::from(x) >= min["x"].as_i64().unwrap()
                        && i64::from(x) < end["x"].as_i64().unwrap()
                        && i64::from(y) >= min["y"].as_i64().unwrap()
                        && i64::from(y) < end["y"].as_i64().unwrap()
                })
                .cloned()
                .ok_or_else(|| mapkit_core::error("E_CELL", "unknown region"))
        })())
    }
    /// Full dependency audit is an admission boundary, not an index-only open.
    #[func]
    fn audit(&self, memory_limit: i64, generation: i64) -> GString {
        response((|| {
            let ticket = self.epoch.ticket(generation as u64);
            ticket.check()?;
            let mut reader = self
                .reader
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?
                .lock()
                .map_err(|_| mapkit_core::error("E_STATE", "reader lock poisoned"))?;
            // Explicit allowance for immutable metadata duplicated outside the I/O lock.
            let metadata_bytes = reader.index_retained_bytes();
            let mut result = reader.audit_summary(
                (memory_limit.max(0) as u64).saturating_sub(metadata_bytes),
                &ticket,
            )?;
            result["retained_memory_bytes"] = serde_json::json!(
                result["retained_memory_bytes"].as_u64().unwrap() + metadata_bytes
            );
            result["validation_peak_bytes"] = serde_json::json!(
                result["validation_peak_bytes"].as_u64().unwrap() + metadata_bytes
            );
            result["source_peak_bytes"] = serde_json::json!(self
                .regions
                .iter()
                .map(|r| r["cost"]["validation_peak_bytes"].as_u64().unwrap())
                .max()
                .unwrap_or(0));
            Ok(result)
        })())
    }
    // These immutable metadata queries never wait on an in-flight I/O lock.
    #[func]
    fn cell_window(&self, x_cm: i64, y_cm: i64) -> GString {
        response((|| {
            let d = self
                .world
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?;
            let cell = d
                .cell_at([x_cm, y_cm])
                .ok_or_else(|| mapkit_core::error("E_CELL", "position outside map"))?;
            Ok(
                serde_json::json!({"cell":cell,"cells":d.window([x_cm,y_cm]),"bounds":d.bounds,
                "cell_bounds":d.cell_bounds(cell)?,"cell_size_cm":d.cell_size_cm,"world_scale":mapkit_core::WORLD_SCALE,
                "scene_units_version":mapkit_core::SCENE_UNITS_VERSION,"package_format_version":mapkit_core::PACKAGE_VERSION,
                "recipe_version":d.recipe_version,"generated_format_version":mapkit_core::GENERATED_VERSION}),
            )
        })())
    }
    #[func]
    fn map_bounds(&self) -> PackedInt64Array {
        self.world
            .as_ref()
            .map(|d| {
                PackedInt64Array::from(
                    &[
                        d.bounds.min[0],
                        d.bounds.min[1],
                        d.bounds.max[0],
                        d.bounds.max[1],
                    ][..],
                )
            })
            .unwrap_or_default()
    }
    #[func]
    fn query_cells(
        &self,
        min_x: i64,
        min_y: i64,
        max_x: i64,
        max_y: i64,
        max_cells: i64,
    ) -> GString {
        response((|| {
            if !(1..=16_384).contains(&max_cells) {
                return Err(mapkit_core::error("E_BUDGET", "invalid query allowance"));
            }
            let d = self
                .world
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?;
            Ok(serde_json::json!(d.query_cells(
                &mapkit_core::Bounds {
                    min: [min_x, min_y],
                    max: [max_x, max_y]
                },
                max_cells as usize
            )?))
        })())
    }
    #[func]
    fn export_project(&self, path: GString, destination: GString, side_cells: i64) -> GString {
        response((|| {
            if !(1..=128).contains(&side_cells) {
                return Err(mapkit_core::error("E_BUDGET", "invalid storage grouping"));
            }
            let (d, f) =
                mapkit_package::indexed::read_source_project(Path::new(&path.to_string()))?;
            let bytes = mapkit_package::indexed::pack_source(d, f, side_cells as u32)?;
            write_new(Path::new(&destination.to_string()), &bytes)?;
            Ok(serde_json::json!({"package_bytes":bytes.len(),"format":"mkregions"}))
        })())
    }
    #[func]
    fn unpack_source(&self, destination: GString, memory_limit: i64, generation: i64) -> GString {
        response((|| {
            let mut reader = self
                .reader
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?
                .lock()
                .map_err(|_| mapkit_core::error("E_STATE", "reader lock poisoned"))?;
            reader.unpack_source(
                Path::new(&destination.to_string()),
                memory_limit.max(0) as u64,
                &self.epoch.ticket(generation as u64),
            )?;
            Ok(serde_json::json!({"path":destination.to_string()}))
        })())
    }
    /// Returns a separately owned existing generation/render/occupancy bridge.
    /// Never replaces an earlier bridge, even after a rejected or cancelled read.
    #[func]
    fn load_region(&self, region: i64, memory_limit: i64, generation: i64) -> VarDictionary {
        let result = (|| {
            if region < 0 || generation <= 0 {
                return Err(mapkit_core::error("E_CELL", "invalid region/request"));
            }
            let ticket = self.epoch.ticket(generation as u64);
            ticket.check()?;
            let mut reader = self
                .reader
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?
                .lock()
                .map_err(|_| mapkit_core::error("E_STATE", "reader lock poisoned"))?;
            let snapshot =
                reader.load_region(region as usize, memory_limit.max(0) as u64, &ticket)?;
            snapshot.check_candidate()?;
            let mut out = VarDictionary::new();
            out.set("ok", true);
            out.set("generation", generation);
            out.set("region_identity", snapshot.identity.as_str());
            out.set("bytes_read", snapshot.bytes_read as i64);
            out.set(
                "retained_memory_bytes",
                snapshot.package.inspection.retained_memory_bytes as i64,
            );
            let bridge = Gd::from_init_fn(|base| MapKitBridge {
                base,
                package: Some(snapshot.package),
            });
            out.set("bridge", &bridge);
            ticket.check()?;
            Ok(out)
        })();
        match result {
            Ok(out) => out,
            Err(e) => {
                let mut out = VarDictionary::new();
                let mut error = VarDictionary::new();
                error.set("code", e.code.as_str());
                error.set("message", e.message.as_str());
                out.set("ok", false);
                out.set("error", &error);
                out
            }
        }
    }
}
