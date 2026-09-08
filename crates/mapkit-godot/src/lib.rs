mod packed;
mod occupied;
use godot::prelude::*;
use mapkit_core::{canonical, Cell, GenerationInput, SpawnRequest};
use mapkit_package::{pack_bytes, read, read_bytes, read_project, write_new, Package};
use std::path::Path;
struct MapKitExtension;
#[gdextension]
unsafe impl ExtensionLibrary for MapKitExtension {}
#[derive(GodotClass)]
#[class(base=RefCounted)]
struct MapKitBridge {
    base: Base<RefCounted>,
    package: Option<Package>,
}
#[godot_api]
impl IRefCounted for MapKitBridge {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            package: None,
        }
    }
}
fn response(result: mapkit_core::Result<serde_json::Value>) -> GString {
    let value = match result {
        Ok(data) => serde_json::json!({"ok":true,"data":data}),
        Err(e) => serde_json::json!({"ok":false,"error":e}),
    };
    GString::from(serde_json::to_string(&value).unwrap().as_str())
}
fn engine_document(text: &str) -> mapkit_core::Result<mapkit_core::MapDocument> {
    let mut value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| mapkit_core::error("E_JSON", e.to_string()))?;
    fn integers(v: &mut serde_json::Value) -> mapkit_core::Result<()> {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() != 0.0 || f.abs() > 9_007_199_254_740_991.0 {
                    return Err(mapkit_core::error(
                        "E_JSON",
                        "engine map numbers must be exactly representable integers",
                    ));
                }
                *v = serde_json::Value::from(f as i64);
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    integers(v)?;
                }
            }
            serde_json::Value::Object(o) => {
                for v in o.values_mut() {
                    integers(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    integers(&mut value)?;
    serde_json::from_value(value).map_err(|e| mapkit_core::error("E_JSON", e.to_string()))
}
#[godot_api]
impl MapKitBridge {
    #[func]
    fn open_package(&mut self, path: GString) -> GString {
        self.package = None;
        response(read(Path::new(&path.to_string())).map(|p| {
            let info = serde_json::to_value(&p.inspection).unwrap();
            self.package = Some(p);
            info
        }))
    }
    /// Caller-supplied validation allowance, checked before payload inflation.
    #[func]
    fn open_package_bytes_budgeted(&mut self, bytes: PackedByteArray, memory_limit: i64) -> GString {
        self.package = None;
        if memory_limit <= 0 {
            return response(Err(mapkit_core::error("E_MEMORY_BUDGET", "positive memory allowance required")));
        }
        response(mapkit_package::read_bytes_with_budget(bytes.as_slice(), memory_limit as u64).map(|p| {
            let info = serde_json::to_value(&p.inspection).unwrap();
            self.package = Some(p);
            info
        }))
    }
    /// The caller owns transfer/cache/session snapshot policy. MapKit only validates bytes.
    #[func]
    fn open_package_bytes(&mut self, bytes: PackedByteArray) -> GString {
        self.package = None;
        response(read_bytes(bytes.as_slice()).map(|p| {
            let info = serde_json::to_value(&p.inspection).unwrap();
            self.package = Some(p);
            info
        }))
    }
    /// Map topology is owned by MapKit; callers never reimplement cell-index constants.
    #[func]
    fn cell_window(&self, x_cm: i64, y_cm: i64) -> GString {
        response(
            self.package
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .and_then(|p| {
                    let cell = p
                        .document
                        .cell_at([x_cm, y_cm])
                        .ok_or_else(|| mapkit_core::error("E_CELL", "position outside map"))?;
                    Ok(serde_json::json!({
                        "cell": cell, "cells": p.document.window([x_cm, y_cm]),
                        "bounds": p.document.bounds, "cell_bounds": p.document.cell_bounds(cell)?,
                        "cell_size_cm": p.document.cell_size_cm,
                        "world_scale": mapkit_core::WORLD_SCALE,
                        "package_format_version": mapkit_core::PACKAGE_VERSION,
                        "recipe_version": mapkit_core::RECIPE_VERSION,
                        "generated_format_version": mapkit_core::GENERATED_VERSION,
                    }))
                }),
        )
    }
    #[func]
    fn open_project(&mut self, path: GString) -> GString {
        self.package = None;
        response(
            read_project(Path::new(&path.to_string()))
                .and_then(|(d, f)| pack_bytes(d, f))
                .and_then(|b| read_bytes(&b))
                .map(|p| {
                    let info = serde_json::to_value(&p.inspection).unwrap();
                    self.package = Some(p);
                    info
                }),
        )
    }
    #[func]
    fn overview_cost(&self) -> GString {
        response(self.package.as_ref()
            .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
            .and_then(|p| mapkit_core::overview(&p.document)?.cost())
            .map(|cost| serde_json::json!(cost)))
    }
    #[func]
    fn overview_json(&self, max_json_bytes: i64) -> GString {
        let result = self.package.as_ref()
            .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
            .and_then(|p| mapkit_core::overview(&p.document)?.to_json(max_json_bytes.max(0) as u64));
        match result {
            Ok(body) => GString::from(format!("{{\"ok\":true,\"data\":{body}}}").as_str()),
            Err(error) => response(Err(error)),
        }
    }
    #[func]
    fn document_json(&self) -> GString {
        response(
            self.package
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .map(|p| serde_json::to_value(&p.document).unwrap()),
        )
    }
    #[func]
    fn estimate_chunk(&self, x: i32, y: i32) -> GString {
        response(
            self.package.as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .and_then(|p| mapkit_core::estimate_generation(&p.document, Cell { x, y }, 500_000))
                .map(|cost| serde_json::json!(cost))
        )
    }
    /// Bounded broad-phase plan; callers estimate and reserve each required cell.
    #[func]
    fn query_cells(&self, min_x: i64, min_y: i64, max_x: i64, max_y: i64, max_cells: i64) -> GString {
        response(self.package.as_ref()
            .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
            .and_then(|p| {
                if !(1..=16_384).contains(&max_cells) {
                    return Err(mapkit_core::error("E_BUDGET", "invalid query cell allowance"));
                }
                p.document.query_cells(&mapkit_core::Bounds {
                    min: [min_x, min_y], max: [max_x, max_y],
                }, max_cells as usize)
            }).map(|plan| serde_json::json!(plan)))
    }
    /// Exact integer centimetres and indexed triangle metadata; no JSON geometry copy.
    #[func]
    fn generate_chunk_packed(&self, x: i32, y: i32) -> VarDictionary {
        packed::response(self.package.as_ref()
            .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
            .and_then(|p| p.generate(Cell { x, y }, 500_000)))
    }
    /// Archive identity and pre-allocation size bound; storage/leases belong to callers.
    #[func]
    fn chunk_archive_info(&self, x: i32, y: i32) -> GString {
        response((|| {
            let p = self.package.as_ref().ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))?;
            let cell = Cell { x, y };
            let cost = mapkit_core::estimate_generation(&p.document, cell, 500_000)?;
            Ok(serde_json::json!({"key": mapkit_core::archive_key(&p.inspection.world_content_hash, cell),
                "max_bytes": mapkit_core::archive_limit(&cost)}))
        })())
    }
    #[func]
    fn generate_chunk_archived(&self, x: i32, y: i32, max_bytes: i64) -> VarDictionary {
        packed::respond((|| {
            let p = self.package.as_ref().ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))?;
            let cell = Cell { x, y };
            let chunk = p.generate(cell, 500_000)?;
            let archive = mapkit_core::encode_archive(&chunk,
                &mapkit_core::archive_key(&p.inspection.world_content_hash, cell), max_bytes.max(0) as u64).ok();
            let mut data = packed::pack(chunk)?;
            if let Some(bytes) = archive { data.set("archive", &PackedByteArray::from(bytes.as_slice())); }
            Ok(data)
        })())
    }
    #[func]
    fn restore_chunk_archive(&self, x: i32, y: i32, bytes: PackedByteArray) -> VarDictionary {
        packed::response((|| {
            let p = self.package.as_ref().ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))?;
            let cell = Cell { x, y };
            let cost = mapkit_core::estimate_generation(&p.document, cell, 500_000)?;
            mapkit_core::decode_archive(bytes.as_slice(),
                &mapkit_core::archive_key(&p.inspection.world_content_hash, cell), cell, &cost)
        })())
    }
    #[func]
    fn generate_chunk_occupied_packed(&self, x: i32, y: i32, max_solids: i64) -> VarDictionary {
        occupied::response(if !(0..=mapkit_core::MAX_OCCUPIED_SOLIDS as i64).contains(&max_solids) {
            Err(mapkit_core::error("E_BUDGET", "invalid occupied solid allowance"))
        } else {
            self.package.as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .and_then(|p| p.generate_with_occupancy(Cell { x, y }, 500_000, max_solids as usize))
        })
    }
    #[func]
    fn generate_chunk(&self, x: i32, y: i32) -> GString {
        response(
            self.package
                .as_ref()
                .ok_or_else(|| mapkit_core::error("E_STATE", "open package first"))
                .and_then(|p| p.generate(Cell { x, y }, 500_000))
                .map(|chunk| {
                    let hash = chunk.hash().unwrap();
                    serde_json::json!({"chunk":chunk,"generated_sha256":hash})
                }),
        )
    }
    #[func]
    fn preview_document(&self, document: GString, x: i32, y: i32) -> GString {
        response(engine_document(&document.to_string()).map_err(|e|mapkit_core::error("E_JSON",e.to_string())).and_then(|d|mapkit_core::generate(GenerationInput{document:&d,cell:Cell{x,y},heightgrid:None,max_triangles:500_000})).map(|chunk|serde_json::json!({"generated_sha256":chunk.hash().unwrap(),"chunk":chunk})))
    }
    #[func]
    fn validate_document(&self, document: GString) -> GString {
        response(
            engine_document(&document.to_string())
                .map_err(|e| mapkit_core::error("E_JSON", e.to_string()))
                .and_then(|mut d| {
                    d.normalize();
                    d.validate()?;
                    Ok(serde_json::json!({"canonical": String::from_utf8(canonical(&d)?).unwrap(), "document":d}))
                }),
        )
    }
    #[func]
    fn export_project(&self, path: GString, destination: GString) -> GString {
        response(
            read_project(Path::new(&path.to_string()))
                .and_then(|(d, f)| pack_bytes(d, f))
                .and_then(|bytes| {
                    let p = read_bytes(&bytes)?;
                    write_new(Path::new(&destination.to_string()), &bytes)?;
                    Ok(serde_json::to_value(p.inspection).unwrap())
                }),
        )
    }
    #[func]
    fn spawn_options(&self, x_cm: i64, y_cm: i64) -> GString {
        response(self.package.as_ref().ok_or_else(|| mapkit_core::error("E_STATE", "open package first")).and_then(|p| {
            let cell = p.document.cell_at([x_cm, y_cm]).ok_or_else(|| mapkit_core::error("E_SPAWN", "outside map"))?;
            let chunk = p.generate(cell, 500_000)?;
            let ids: std::collections::BTreeSet<_> = chunk.triangles.iter().filter(|t| t.spawnable).map(|t| t.object_id.clone()).collect();
            let options: Vec<_> = ids.into_iter().filter_map(|id| {
                chunk.spawn(&SpawnRequest { position_cm: [x_cm, y_cm], surface_id: id.clone() }).ok().map(|position| serde_json::json!({"surface_id": id, "position_cm": position}))
            }).collect();
            Ok(serde_json::json!({"surfaces": options}))
        }))
    }
    #[func]
    fn spawn(&self, x_cm: i64, y_cm: i64, surface: GString) -> GString {
        response(self.package.as_ref().ok_or_else(||mapkit_core::error("E_STATE","open package first")).and_then(|p| {
            let cell=p.document.cell_at([x_cm,y_cm]).ok_or_else(||mapkit_core::error("E_SPAWN","outside map"))?;
            let position=p.generate(cell,500_000)?.spawn(&SpawnRequest{position_cm:[x_cm,y_cm],surface_id:surface.to_string()})?;
            Ok(serde_json::json!({"position_cm":position,"window":p.document.window([x_cm,y_cm])}))
        }))
    }
    /// Exact local bounds without serializing the full editing document.
    #[func]
    fn map_bounds(&self) -> PackedInt64Array {
        self.package.as_ref().map(|p| PackedInt64Array::from(&[
            p.document.bounds.min[0], p.document.bounds.min[1],
            p.document.bounds.max[0], p.document.bounds.max[1]][..])).unwrap_or_default()
    }
    /// Caller must reserve generation cost and invoke on its bounded worker.
    #[func]
    fn surface_probe(&self, x_cm: i64, y_cm: i64, surface: GString) -> GString {
        response(self.package.as_ref().ok_or_else(||mapkit_core::error("E_STATE","open package first")).and_then(|p| {
            let point = [x_cm, y_cm];
            let cell = p.document.cell_at(point).ok_or_else(||mapkit_core::error("E_SPAWN","outside map"))?;
            let sample = p.generate(cell, 500_000)?.surface_probe(&SpawnRequest { position_cm: point, surface_id: surface.to_string() })?;
            let is_road = p.document.roads.iter().any(|r| r.id == sample.surface_id);
            let blocked_by_building = p.document.buildings.iter().any(|b|
                mapkit_core::point_in_polygon(point, &b.footprint) && sample.position_cm[1] >= b.base_cm
                && sample.position_cm[1] <= b.base_cm + b.height_cm as i64);
            Ok(serde_json::json!({"position_cm":sample.position_cm,"normal_q":sample.normal_q,
                "surface_id":sample.surface_id,"is_road":is_road,"blocked_by_building":blocked_by_building}))
        }))
    }
    #[func]
    fn canonical_document(&self) -> GString {
        self.package
            .as_ref()
            .and_then(|p| canonical(&p.document).ok())
            .map(|b| GString::from(String::from_utf8(b).unwrap().as_str()))
            .unwrap_or_default()
    }
}
