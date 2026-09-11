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
}
#[godot_api]
impl IRefCounted for MapKitRegionReader {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            reader: None,
            epoch: ReadEpoch::default(),
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
            let index = reader.index();
            let result = serde_json::json!({"index_sha256": reader.identity(),
                "world_content_hash": index.world_content_hash, "bounds": index.world.bounds,
                "cell_size_cm": index.world.cell_size_cm, "side_cells": index.side_cells,
                "region_count": index.regions.len(), "execution_cells": index.world.cell_count()?,
                "retained_memory_bytes": reader.index_retained_bytes(), "verification": "index-only"});
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
            let reader = self
                .reader
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open index first"))?
                .lock()
                .map_err(|_| mapkit_core::error("E_STATE", "reader lock poisoned"))?;
            let id = reader.region_for_cell(Cell { x, y })?;
            Ok(
                serde_json::json!({"region": id, "cells": reader.index().regions[id].cells,
                "cost": reader.region_cost(id)?}),
            )
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
