//! Engine-owned handles to editor working arrays. Forks are immutable worker inputs.
use super::{engine_document, packed, presentation, response};
use godot::prelude::*;
use mapkit_core::*;
use mapkit_package::working::{Brush, BrushOptions, Resources, WorkingSnapshot};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct MapKitWorkingSnapshot {
    base: Base<RefCounted>,
    snapshot: Option<WorkingSnapshot>,
    brush: Option<Brush>,
}
#[godot_api]
impl IRefCounted for MapKitWorkingSnapshot {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            snapshot: None,
            brush: None,
        }
    }
}
impl MapKitWorkingSnapshot {
    fn retarget_document(&mut self, text: GString, root: GString, history: bool) -> GString {
        response((|| {
            let mut d = engine_document(&text.to_string())?;
            d.normalize();
            d.validate()?;
            let mut next = self.state()?.clone();
            let s = &mut next;
            if s.document.bounds != d.bounds
                || s.document.cell_size_cm != d.cell_size_cm
                || s.document.terrain_base_cm != d.terrain_base_cm
            {
                if !s.modified.is_empty() {
                    return Err(error(
                        "E_STATE",
                        "save or undo terrain changes before changing its grid or base",
                    ));
                }
                s.tiles.clear();
                s.saved_tiles.clear();
                s.changed_indices.clear();
            }
            let descriptors: std::collections::BTreeMap<_, _> = s
                .document
                .heightmaps
                .iter()
                .chain(&d.heightmaps)
                .map(|h| (h.cell, h.clone()))
                .collect();
            for (_, h) in descriptors {
                if s.document.heightmaps.iter().find(|p| p.cell == h.cell)
                    != d.heightmaps.iter().find(|p| p.cell == h.cell)
                {
                    if !history && s.modified.contains(&h.cell) {
                        return Err(error(
                            "E_STATE",
                            "save or undo terrain changes before replacing its source grid",
                        ));
                    }
                    if s.document.has_cell(h.cell) {
                        s.grid(h.cell)?;
                    }
                    s.tiles.remove(&h.cell);
                    s.authored_tiles.remove(&h.cell);
                    s.modified.remove(&h.cell);
                    s.changed_indices.remove(&h.cell);
                }
            }
            s.fitters.clear();
            s.document = d;
            s.resources.root = root.to_string().into();
            self.snapshot = Some(next);
            Ok(serde_json::json!({}))
        })())
    }

    fn state(&mut self) -> Result<&mut WorkingSnapshot> {
        self.snapshot
            .as_mut()
            .ok_or_else(|| error("E_STATE", "configure working snapshot first"))
    }
}
#[godot_api]
impl MapKitWorkingSnapshot {
    #[func]
    fn configure(&mut self, text: GString, root: GString) -> GString {
        response((|| {
            self.snapshot = Some(WorkingSnapshot::new(
                engine_document(&text.to_string())?,
                Resources {
                    root: root.to_string().into(),
                    ..Default::default()
                },
            )?);
            self.brush = None;
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn set_document(&mut self, text: GString, root: GString) -> GString {
        self.retarget_document(text, root, false)
    }
    #[func]
    fn replay_document(&mut self, text: GString, root: GString) -> GString {
        self.retarget_document(text, root, true)
    }
    #[func]
    fn accept_saved(&mut self, text: GString, root: GString) -> GString {
        response((|| {
            let d = engine_document(&text.to_string())?;
            let s = self.state()?;
            let mut observed = std::collections::BTreeMap::new();
            for h in s.document.heightmaps.iter().chain(&d.heightmaps) {
                if let Some(hash) = h
                    .path
                    .strip_prefix("editor/")
                    .and_then(|p| p.strip_suffix(".png"))
                    .filter(|p| p.len() == 64 && p.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    observed.insert(h.path.clone(), hash.to_owned());
                } else if let Some(hash) = s.resources.observed.get(&h.path) {
                    observed.insert(h.path.clone(), hash.clone());
                }
            }
            s.fitters.clear();
            s.saved_descriptors = d.heightmaps.into_iter().map(|h| (h.cell, h)).collect();
            s.resources.root = root.to_string().into();
            s.resources.observed = observed;
            s.modified.clear();
            s.saved_tiles = s.tiles.clone();
            s.changed_indices.clear();
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn has_changes(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|s| !s.modified.is_empty())
    }
    #[func]
    fn provide_resources(&mut self, resources: VarDictionary) -> GString {
        response((|| {
            let s = self.state()?;
            let mut next = s.resources.memory.clone();
            for (key, value) in resources.iter_shared() {
                let path = key
                    .try_to::<GString>()
                    .map_err(|_| error("E_PATH", "resource key must be a path"))?
                    .to_string();
                if !safe_path(&path) {
                    return Err(error("E_PATH", "unsafe resource path"));
                }
                let bytes = value
                    .try_to::<PackedByteArray>()
                    .map_err(|_| error("E_RESOURCE", "resource must contain bytes"))?;
                next.insert(path, std::sync::Arc::new(bytes.as_slice().to_vec()));
            }
            if next.values().map(|b| b.len()).sum::<usize>() > mapkit_package::working::WORK_BYTES {
                return Err(error("E_BUDGET", "working resources exceed 64 MiB"));
            }
            s.resources.memory = next;
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn fork(&self) -> Gd<Self> {
        let mut next = Self::new_gd();
        next.bind_mut().snapshot = self.snapshot.clone();
        next
    }
    #[func]
    fn document_json(&mut self) -> GString {
        response(
            self.state()
                .and_then(|s| Ok(serde_json::to_value(s.composed_document()).unwrap())),
        )
    }
    #[func]
    fn spacing_cm(&mut self) -> i64 {
        self.snapshot.as_ref().map_or(200, WorkingSnapshot::spacing)
    }
    #[func]
    fn height(&mut self, point: Vector2) -> GString {
        response(
            self.state()
                .and_then(|s| s.surface_height([point.x.round() as i64, point.y.round() as i64]))
                .map(|h| serde_json::json!({"height":h})),
        )
    }
    #[func]
    fn raycast(&mut self, origin: Vector3, direction: Vector3) -> GString {
        response((|| {
            let s = self.state()?;
            let o = [
                origin.x as f64 * 100.0,
                origin.y as f64 * 100.0,
                -origin.z as f64 * 100.0,
            ];
            let v = [direction.x as f64, direction.y as f64, -direction.z as f64];
            let mut near: f64 = 0.0;
            let mut far: f64 = 4_000_000.0;
            for (a, k) in [(0, 0), (1, 2)] {
                let lo = s.document.bounds.min[a] as f64;
                let hi = s.document.bounds.max[a] as f64;
                if v[k].abs() < 1e-9 {
                    if o[k] < lo || o[k] > hi {
                        return Ok(serde_json::json!({}));
                    }
                } else {
                    let t0 = (lo - o[k]) / v[k];
                    let t1 = (hi - o[k]) / v[k];
                    near = near.max(t0.min(t1));
                    far = far.min(t0.max(t1));
                }
            }
            if far < near {
                return Ok(serde_json::json!({}));
            }
            let step = (s.spacing() as f64 * 0.5).max(100.0);
            let mut t = near;
            let mut previous = near;
            for _ in 0..100_000 {
                let p = [0, 1, 2].map(|k| o[k] + v[k] * t);
                if p[1] <= s.surface_height([p[0].round() as i64, p[2].round() as i64])? as f64 {
                    let mut low = previous;
                    let mut high = t;
                    for _ in 0..12 {
                        let mid = (low + high) * 0.5;
                        let p = [0, 1, 2].map(|k| o[k] + v[k] * mid);
                        if p[1]
                            > s.surface_height([p[0].round() as i64, p[2].round() as i64])? as f64
                        {
                            low = mid;
                        } else {
                            high = mid;
                        }
                    }
                    return Ok(
                        serde_json::json!({"point":([0,1,2].map(|k|(o[k]+v[k]*high).round() as i64))}),
                    );
                }
                if t >= far {
                    break;
                }
                previous = t;
                t = (t + step).min(far);
            }
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn begin_brush(&mut self, point: Vector2, options: GString) -> GString {
        response((|| {
            if self.brush.is_some() {
                return Err(error("E_STATE", "brush already active"));
            }
            let options: BrushOptions = serde_json::from_str(&options.to_string())
                .map_err(|e| error("E_BRUSH", e.to_string()))?;
            self.brush = Some(Brush::begin(
                self.state()?,
                [point.x as f64, point.y as f64],
                options,
            )?);
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn step_brush(&mut self, point: Vector2, seconds: f64) -> VarDictionary {
        packed::respond((|| {
            let b = self
                .brush
                .as_mut()
                .ok_or_else(|| error("E_STATE", "begin brush first"))?;
            let cells = b.step(
                self.snapshot.as_mut().unwrap(),
                [point.x as f64, point.y as f64],
                seconds,
            )?;
            let mut data = VarDictionary::new();
            let cells: Array<Vector2i> =
                cells.into_iter().map(|c| Vector2i::new(c.x, c.y)).collect();
            data.set("cells", &cells);
            data.set("samples", b.changes.len() as i64);
            Ok(data)
        })())
    }
    #[func]
    fn finish_brush(&mut self) -> PackedInt32Array {
        self.brush.take().map_or_else(PackedInt32Array::new, |b| {
            PackedInt32Array::from(b.delta().as_slice())
        })
    }
    #[func]
    fn cancel_brush(&mut self) {
        if let Some(b) = self.brush.take() {
            self.snapshot = Some(b.original);
        }
    }
    #[func]
    fn apply_delta(&mut self, delta: PackedInt32Array, reverse: bool) -> GString {
        response((|| {
            if delta.len() % 4 != 0 || delta.len() / 4 > mapkit_package::working::CHANGED_SAMPLES {
                return Err(error("E_BUDGET", "invalid height history"));
            }
            let mut s = self.state()?.clone();
            for row in delta.as_slice().chunks_exact(4) {
                let (before, after) = if reverse {
                    (row[3], row[2])
                } else {
                    (row[2], row[3])
                };
                if s.sample([row[0], row[1]])? != before as i64 {
                    return Err(error("E_STALE", "height history changed"));
                }
                s.set_sample([row[0], row[1]], after as i64)?;
            }
            self.snapshot = Some(s);
            Ok(serde_json::json!({}))
        })())
    }
    #[func]
    fn water_edit(&mut self, point: Vector2, remove: bool, refresh: bool) -> GString {
        response(
            self.state()
                .and_then(|s| {
                    s.validate_memory()?;
                    s.validate_changed_geometry()?;
                    mapkit_package::water_edit::edit(
                        s,
                        (!refresh).then_some([point.x.round() as i64, point.y.round() as i64]),
                        remove,
                    )
                })
                .map(|b| serde_json::json!(b)),
        )
    }
    #[func]
    fn tile_cells(&mut self) -> Array<Vector2i> {
        self.snapshot.as_ref().map_or_else(Array::new, |s| {
            s.composed_document()
                .heightmaps
                .iter()
                .map(|h| Vector2i::new(h.cell.x, h.cell.y))
                .collect()
        })
    }
    #[func]
    fn tile(&mut self, cell: Vector2i) -> VarDictionary {
        packed::respond((|| {
            let g = self.state()?.grid(Cell {
                x: cell.x,
                y: cell.y,
            })?;
            let mut data = VarDictionary::new();
            data.set("heights", &PackedInt64Array::from(g.heights_cm.as_slice()));
            data.set("side", g.side as i64);
            Ok(data)
        })())
    }
    #[func]
    fn generate_packed(&mut self, x: i32, y: i32, display: bool) -> VarDictionary {
        packed::respond((|| {
            let s = self.state()?;
            let (d, mut c) = s.generate(Cell { x, y }, display)?;
            if display {
                c.triangles
                    .retain(|t| !t.object_id.starts_with("assembled-"));
                c.gimmicks.clear();
                c.grind_lines.clear();
            }
            let (files, cost) = super::source_preview::assets_from(&d, &c, |p, limit| {
                let bytes = s.resources.read(p)?;
                if bytes.len() as u64 > limit {
                    return Err(error("E_BUDGET", "preview payload allowance"));
                }
                Ok(bytes)
            })?;
            let hash =
                sha256(&serde_json::to_vec(&(c.hash()?, &d.environment, &d.assets)).unwrap());
            let mut data = presentation::decorate_document(&d, &files, packed::pack(c)?)?;
            data.set("preview_bytes", cost as i64);
            data.set("preview_signature", hash.as_str());
            Ok(data)
        })())
    }
    /// Full generation admission without packing render arrays for discarded cells.
    #[func]
    fn validate_cells(&mut self, cells: Array<Vector2i>) -> GString {
        response((|| {
            if cells.is_empty() || cells.len() > 256 {
                return Err(error("E_BUDGET", "cell validation requires 1..256 cells"));
            }
            let selected: Vec<_> = cells.iter_shared().map(|c| Cell { x:c.x, y:c.y }).collect();
            self.state()?.validate_cells(&selected)?;
            Ok(serde_json::json!({"cells": selected.len()}))
        })())
    }
    #[func]
    fn materialize(&mut self) -> VarDictionary {
        packed::respond((|| {
            let (d, files) = self.state()?.materialize()?;
            let mut data = VarDictionary::new();
            let mut blobs = VarDictionary::new();
            for (p, b) in files {
                blobs.set(p.as_str(), &PackedByteArray::from(b.as_slice()));
            }
            data.set(
                "canonical",
                String::from_utf8(canonical(&d)?).unwrap().as_str(),
            );
            data.set("blobs", &blobs);
            Ok(data)
        })())
    }
    #[func]
    fn estimate(&mut self, x: i32, y: i32) -> GString {
        response((|| {
            let s = self.state()?;
            let d = s.composed_document();
            let cell = Cell { x, y };
            let prepared = PreparedMap::new(d.clone())?;
            let estimate = prepared.estimate(cell, 500_000)?;
            let placements = prepared.authored_placement_candidates(cell)?;
            let mut needed: std::collections::BTreeSet<_> =
                placements.iter().map(|p| p.asset_id.clone()).collect();
            for a in &d.assets {
                if needed.contains(&a.id) {
                    if let Some(id) = a.material.as_ref().and_then(|m| m.albedo_texture.as_ref()) {
                        needed.insert(id.clone());
                    }
                }
            }
            let mut subset = d.clone();
            subset.assets.retain(|a| needed.contains(&a.id));
            let mut files = std::collections::BTreeMap::new();
            let mut bytes = 0;
            for a in &mut subset.assets {
                a.distant_path = None;
                let data = s.resources.read(&a.path)?;
                bytes += data.len();
                if bytes > 16 * 1024 * 1024 {
                    return Err(error("E_BUDGET", "preview payload allowance"));
                }
                files.insert(a.path.clone(), data);
            }
            let assets = mapkit_package::preview_asset_cost(&subset, &files, placements.len())?;
            let presentation_bytes =
                assets + presentation::environment_cost(&d) + estimate.triangles * 256;
            let mut value = serde_json::to_value(estimate).unwrap();
            value["presentation_bytes"] = serde_json::json!(presentation_bytes);
            Ok(value)
        })())
    }
    #[func]
    fn validate_memory(&mut self) -> GString {
        response((|| {
            let s = self.state()?;
            s.validate_memory()?;
            let d = s.composed_document();
            Ok(
                serde_json::json!({"cell_count":d.cells().len(),"free_roam":d.free_roam,"memory_snapshot":true}),
            )
        })())
    }
}
