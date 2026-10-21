//! In-memory authoring snapshot. Reading, brushing and generating never write files.
//! The editor owns these arrays and sparse history; only explicit publication encodes PNGs.
use crate::*;
use mapkit_core::profile::{self, Budget, Profile};
use std::sync::Arc;

pub const WORK_BYTES: usize = 64 * 1024 * 1024;
pub const CHANGED_SAMPLES: usize = 1_000_000;
const MIB: u64 = 1024 * 1024;
// Editor-only working budgets: Runtime values are the historic refusals; an
// Authoring scope warns above them up to the raised memory guards.
pub const RESOURCE_FILE: Budget = Budget { code: "working_resource_file", limit: WORK_BYTES as u64, guard: MAX_ENTRY_BYTES,
    message: "A project resource file is very large and needs a lot of memory", runtime_rejects: false };
pub const HEIGHT_ARRAYS: Budget = Budget { code: "working_heights", limit: WORK_BYTES as u64, guard: 1024 * MIB,
    message: "Working terrain height arrays need a lot of memory", runtime_rejects: false };
pub const SNAPSHOT_PAYLOADS: Budget = Budget { code: "snapshot_payloads", limit: WORK_BYTES as u64, guard: 1024 * MIB,
    message: "Saved snapshot files need a lot of memory", runtime_rejects: false };
pub const WORKING_RESOURCES: Budget = Budget { code: "working_resources", limit: WORK_BYTES as u64, guard: 1024 * MIB,
    message: "Working asset and course files need a lot of memory", runtime_rejects: false };
pub const CHANGED: Budget = Budget { code: "brush_samples", limit: CHANGED_SAMPLES as u64, guard: 8 * CHANGED_SAMPLES as u64,
    message: "Terrain change touches very many height samples", runtime_rejects: false };
pub const PREVIEW_WORK: Budget = Budget { code: "preview_work", limit: 256 * MIB, guard: 2048 * MIB,
    message: "Cell preview needs more work memory than the preview allowance", runtime_rejects: false };
pub const PREVIEW_PAYLOAD: Budget = Budget { code: "preview_payload", limit: 16 * MIB, guard: 128 * MIB,
    message: "Cell preview asset files are very large", runtime_rejects: false };
/// Generation cap for the active profile.
pub fn cell_triangles() -> usize {
    if Profile::current() == Profile::Authoring { profile::AUTHORING_TRIANGLES } else { profile::RUNTIME_TRIANGLES }
}

#[derive(Clone, Default)]
pub struct Resources {
    pub root: std::path::PathBuf,
    pub memory: BTreeMap<String, Arc<Vec<u8>>>,
    pub observed: BTreeMap<String, String>,
}
impl Resources {
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        if !safe_path(path) {
            return Err(error("E_PATH", "unsafe working resource path"));
        }
        if let Some(bytes) = self.memory.get(path) {
            return Ok(bytes.as_ref().clone());
        }
        self.read_disk(path)
    }
    fn read_disk(&self, path: &str) -> Result<Vec<u8>> {
        let root = self.root.canonicalize().map_err(io)?;
        let actual = root.join(path).canonicalize().map_err(io)?;
        if !actual.starts_with(root) {
            return Err(error("E_PATH", "resource escapes project"));
        }
        let bytes = bounded_read(&actual, RESOURCE_FILE.active_limit())?;
        RESOURCE_FILE.check(bytes.len() as u64, || error("E_LIMIT", "file exceeds supported profile"))?;
        Ok(bytes)
    }
    pub fn verify(&self) -> Result<()> {
        for (path, expected) in &self.observed {
            cancellation::checkpoint()?;
            let actual = self.read_disk(path)?;
            if sha256(&actual) != *expected {
                return Err(error(
                    "E_CONFLICT",
                    format!("source changed on disk: {path}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct WorkingSnapshot {
    pub document: MapDocument,
    pub resources: Resources,
    pub tiles: BTreeMap<Cell, Arc<HeightGrid>>,
    pub saved_tiles: BTreeMap<Cell, Arc<HeightGrid>>,
    pub saved_descriptors: BTreeMap<Cell, Heightmap>,
    pub authored_tiles: BTreeSet<Cell>,
    pub changed_indices: BTreeMap<Cell, BTreeSet<usize>>,
    pub modified: BTreeSet<Cell>,
    pub fitters: BTreeMap<Cell, Arc<mapkit_core::road_design::Fitter>>,
    /// The last validated, prepared composition; reused while it is unchanged.
    prepared: Option<Arc<PreparedMap>>,
}
impl WorkingSnapshot {
    pub fn new(mut document: MapDocument, resources: Resources) -> Result<Self> {
        document.normalize();
        document.validate()?;
        Ok(Self {
            saved_descriptors: document
                .heightmaps
                .iter()
                .map(|h| (h.cell, h.clone()))
                .collect(),
            document,
            resources,
            tiles: BTreeMap::new(),
            saved_tiles: BTreeMap::new(),
            authored_tiles: BTreeSet::new(),
            changed_indices: BTreeMap::new(),
            modified: BTreeSet::new(),
            fitters: BTreeMap::new(),
            prepared: None,
        })
    }
    pub fn spacing(&self) -> i64 {
        self.document
            .heightmaps
            .first()
            .map_or(200, |h| h.spacing_cm as i64)
    }
    pub fn dimensions(&self) -> [i32; 2] {
        [0, 1].map(|a| {
            ((self.document.bounds.max[a] - self.document.bounds.min[a] + self.spacing() - 1)
                / self.spacing()) as i32
        })
    }
    pub fn grid(&mut self, cell: Cell) -> Result<Arc<HeightGrid>> {
        if let Some(grid) = self.tiles.get(&cell) {
            return Ok(grid.clone());
        }
        let h = self
            .document
            .heightmaps
            .iter()
            .find(|h| h.cell == cell)
            .or_else(|| {
                self.authored_tiles
                    .contains(&cell)
                    .then(|| self.saved_descriptors.get(&cell))
                    .flatten()
            });
        let grid = if let Some(h) = h {
            let bytes = self.resources.read(&h.path)?;
            if !self.resources.root.as_os_str().is_empty()
                && !self.resources.memory.contains_key(&h.path)
            {
                self.resources
                    .observed
                    .insert(h.path.clone(), sha256(&bytes));
            }
            decode_heightmap(h, self.document.cell_size_cm, &bytes)?
        } else {
            let side = (self.document.cell_size_cm as i64 / self.spacing() + 1) as usize;
            HeightGrid {
                side,
                heights_cm: vec![self.document.terrain_base_cm; side * side],
            }
        };
        let height_bytes = self
            .tiles
            .values()
            .map(|g| g.heights_cm.len() * 8)
            .sum::<usize>()
            + grid.heights_cm.len() * 8;
        HEIGHT_ARRAYS.check(height_bytes as u64, || error("E_BUDGET", "working height arrays exceed 64 MiB"))?;
        let grid = Arc::new(grid);
        self.tiles.insert(cell, grid.clone());
        if let Some(saved) = self.saved_tiles.get(&cell) {
            let changed: BTreeSet<_> = grid
                .heights_cm
                .iter()
                .enumerate()
                .filter_map(|(i, value)| {
                    (saved.side != grid.side || saved.heights_cm.get(i) != Some(value)).then_some(i)
                })
                .collect();
            if !changed.is_empty() {
                self.modified.insert(cell);
            }
            self.changed_indices.insert(cell, changed);
        } else {
            self.saved_tiles.insert(cell, grid.clone());
        }
        Ok(grid)
    }
    pub fn height(&mut self, p: Point) -> Result<i64> {
        let d = &self.document;
        let p = [0, 1].map(|a| p[a].clamp(d.bounds.min[a], d.bounds.max[a]));
        let cell = d
            .cell_at([0, 1].map(|a| p[a].min(d.bounds.max[a] - 1)))
            .ok_or_else(|| error("E_CELL", "point outside terrain"))?;
        let bounds = d.cell_bounds(cell)?;
        let spacing = d
            .heightmaps
            .iter()
            .find(|h| h.cell == cell)
            .map_or(self.spacing(), |h| h.spacing_cm as i64);
        let grid = self.grid(cell)?;
        Ok(terrain_height(
            &bounds,
            spacing,
            grid.side,
            Some(&grid),
            self.document.terrain_base_cm,
            p,
        ))
    }
    pub fn point(&self, sample: [i32; 2]) -> Point {
        // PNG samples keep their regular spacing even in a clipped edge cell.
        [0, 1].map(|a| self.document.bounds.min[a] + sample[a] as i64 * self.spacing())
    }
    pub fn sample(&mut self, p: [i32; 2]) -> Result<i64> {
        let dim = self.dimensions();
        let p = [0, 1].map(|a| p[a].clamp(0, dim[a]));
        let n = self.document.cell_size_cm as i32 / self.spacing() as i32;
        let cell = Cell {
            x: (p[0] / n).min((dim[0] - 1) / n),
            y: (p[1] / n).min((dim[1] - 1) / n),
        };
        let grid = self.grid(cell)?;
        Ok(
            grid.heights_cm
                [(p[1] - cell.y * n) as usize * grid.side + (p[0] - cell.x * n) as usize],
        )
    }
    pub fn surface_sample(&mut self, p: [i32; 2]) -> Result<i64> {
        let raw = self.sample(p)?;
        let point = self.point(p);
        let cell = self
            .document
            .cell_at([0, 1].map(|a| point[a].min(self.document.bounds.max[a] - 1)))
            .ok_or_else(|| error("E_CELL", "surface point outside map"))?;
        if !self.fitters.contains_key(&cell) {
            self.fitters.insert(
                cell,
                Arc::new(mapkit_core::road_design::Fitter::new(
                    &self.composed_document(),
                    &self.document.cell_bounds(cell)?,
                )),
            );
        }
        Ok(self.fitters[&cell].height(point, raw))
    }
    pub fn surface_height(&mut self, p: Point) -> Result<i64> {
        if self.document.roads.iter().all(|r| r.design.is_none())
            && self.document.assembled_track.is_none()
        {
            return self.height(p);
        }
        let dim = self.dimensions();
        let sp = self.spacing();
        let p =
            [0, 1].map(|a| p[a].clamp(self.document.bounds.min[a], self.document.bounds.max[a]));
        let at = [0, 1]
            .map(|a| ((p[a] - self.document.bounds.min[a]) / sp).min(dim[a] as i64 - 1) as i32);
        let origin = self.point(at);
        let grid = HeightGrid {
            side: 2,
            heights_cm: vec![
                self.surface_sample(at)?,
                self.surface_sample([at[0] + 1, at[1]])?,
                self.surface_sample([at[0], at[1] + 1])?,
                self.surface_sample([at[0] + 1, at[1] + 1])?,
            ],
        };
        Ok(terrain_height(
            &Bounds {
                min: origin,
                max: [origin[0] + sp, origin[1] + sp],
            },
            sp,
            2,
            Some(&grid),
            self.document.terrain_base_cm,
            p,
        ))
    }
    pub fn set_sample(&mut self, p: [i32; 2], value: i64) -> Result<()> {
        let dimensions = self.dimensions();
        if (0..2).any(|a| p[a] < 0 || p[a] > dimensions[a])
            || !(-1_000_000..=1_000_000).contains(&value)
        {
            return Err(error(
                "E_HEIGHT",
                "height sample outside working grid or range",
            ));
        }
        let was_empty = self.modified.is_empty();
        let n = self.document.cell_size_cm as i32 / self.spacing() as i32;
        for y in ((p[1] - 1).div_euclid(n)).max(0)..=p[1] / n {
            for x in ((p[0] - 1).div_euclid(n)).max(0)..=p[0] / n {
                let cell = Cell { x, y };
                if !self.document.has_cell(cell) {
                    continue;
                }
                self.grid(cell)?;
                let grid = Arc::make_mut(self.tiles.get_mut(&cell).unwrap());
                let index = (p[1] - y * n) as usize * grid.side + (p[0] - x * n) as usize;
                grid.heights_cm[index] = value;
                self.authored_tiles.insert(cell);
                let changed = self.changed_indices.entry(cell).or_default();
                if self.saved_tiles[&cell].side == grid.side
                    && self.saved_tiles[&cell].heights_cm.get(index) == Some(&value)
                {
                    changed.remove(&index);
                } else {
                    changed.insert(index);
                }
                if changed.is_empty() {
                    self.modified.remove(&cell);
                    if !self.saved_descriptors.contains_key(&cell)
                        && !self.document.heightmaps.iter().any(|h| h.cell == cell)
                    {
                        self.authored_tiles.remove(&cell);
                    }
                } else {
                    self.modified.insert(cell);
                }
            }
        }
        if was_empty != self.modified.is_empty() {
            self.fitters.clear();
        }
        Ok(())
    }
    pub fn composed_document(&self) -> MapDocument {
        let mut d = self.document.clone();
        if !self.modified.is_empty() {
            if let Some(authoring) = d
                .assembled_track
                .as_mut()
                .and_then(|a| a.authoring.as_mut())
            {
                authoring.terrain_integration = true;
            }
        }
        for cell in self.modified.union(&self.authored_tiles) {
            if !d.heightmaps.iter().any(|h| h.cell == *cell) {
                d.heightmaps.push(Heightmap {
                    cell: *cell,
                    path: format!("editor/working-{}-{}.png", cell.x, cell.y),
                    spacing_cm: self.spacing() as u32,
                    offset_cm: 0,
                    step_cm: 1,
                    source_accuracy_cm: None,
                });
            }
        }
        d.normalize();
        d
    }
    pub fn generate(&mut self, cell: Cell, display: bool) -> Result<(MapDocument, GeneratedChunk)> {
        self.generate_with(cell, display, profile::RUNTIME_TRIANGLES)
    }
    /// Editor preview with a caller cap (`profile::authoring_triangles`).
    pub fn generate_with(&mut self, cell: Cell, display: bool, max_triangles: usize) -> Result<(MapDocument, GeneratedChunk)> {
        cancellation::checkpoint()?;
        let mut d = self.composed_document();
        let grid = if display {
            d.cell_size_cm = 3200;
            d.heightmaps.clear();
            let bounds = d.cell_bounds(cell)?;
            let mut values = Vec::with_capacity(289);
            for y in 0..=16 {
                for x in 0..=16 {
                    values.push(self.height([bounds.min[0] + x * 200, bounds.min[1] + y * 200])?);
                }
            }
            d.heightmaps.push(Heightmap {
                cell,
                path: "editor/display.png".into(),
                spacing_cm: 200,
                offset_cm: 0,
                step_cm: 1,
                source_accuracy_cm: None,
            });
            Some(Arc::new(HeightGrid {
                side: 17,
                heights_cm: values,
            }))
        } else if d.heightmaps.iter().any(|h| h.cell == cell) {
            Some(self.grid(cell)?)
        } else {
            None
        };
        let p = PreparedMap::new(d.clone())?;
        let cost = p.estimate(cell, max_triangles)?;
        PREVIEW_WORK.check(cost.generation_scratch_bytes + cost.triangles * 256, || error("E_BUDGET", "preview work allowance"))?;
        let chunk = p.generate(cell, grid.as_deref(), max_triangles)?;
        profile::note_triangles(chunk.triangles.len());
        Ok((d, chunk))
    }
    /// Admission generates every requested cell against one immutable prepared
    /// document. No presentation buffers or persistent validation cache are kept.
    pub fn validate_cells(&mut self, cells: &[Cell]) -> Result<()> {
        self.validate_cells_with(cells, profile::RUNTIME_TRIANGLES)
    }
    pub fn validate_cells_with(&mut self, cells: &[Cell], max_triangles: usize) -> Result<()> {
        if cells.is_empty() || cells.len() > 256 {
            return Err(error("E_BUDGET", "cell validation requires 1..256 cells"));
        }
        let unique: BTreeSet<_> = cells.iter().copied().collect();
        if unique.len() != cells.len() {
            return Err(error("E_CELL", "duplicate validation cell"));
        }
        // Batched callers check one unchanged composition many times. Its full
        // validation and preparation happen once; resources are still verified.
        let mut composed = self.composed_document();
        composed.normalize();
        let current = self.prepared.as_ref().filter(|p| std::ops::Deref::deref(p.as_ref()) == &composed).cloned();
        let prepared = match current {
            Some(prepared) => prepared,
            None => {
                self.prepared = None;
                let prepared = Arc::new(PreparedMap::new(composed)?);
                self.prepared = Some(prepared.clone());
                prepared
            }
        };
        self.verify_memory(&prepared)?;
        for &cell in cells { prepared.cell_bounds(cell)?; }
        for &cell in cells {
            cancellation::checkpoint()?;
            let result = (|| {
                let grid = if prepared.heightmap(cell).is_some() { Some(self.grid(cell)?) } else { None };
                let cost = prepared.estimate(cell, max_triangles)?;
                PREVIEW_WORK.check(cost.generation_scratch_bytes + cost.triangles * 256, || error("E_BUDGET", "preview work allowance"))?;
                let chunk = prepared.generate(cell, grid.as_deref(), max_triangles)?;
                profile::note_triangles(chunk.triangles.len());
                Ok(())
            })();
            result.map_err(|mut e: Error| { e.message = format!("Cell ({}, {}): {}",cell.x,cell.y,e.message); e })?;
        }
        self.resources.verify()?;
        Ok(())
    }
    /// Explicit save/export only. Unchanged input PNGs are reused byte for byte.
    pub fn materialize(&self) -> Result<(MapDocument, BTreeMap<String, Vec<u8>>)> {
        let mut snapshot = self.clone();
        snapshot.validate_memory()?;
        snapshot.materialize_checked()
    }
    fn materialize_checked(&mut self) -> Result<(MapDocument, BTreeMap<String, Vec<u8>>)> {
        self.resources.verify()?;
        self.clone().validate_changed_geometry()?;
        let mut d = self.composed_document();
        let reused = |s: &Self, cell: Cell| !s.modified.contains(&cell) && s.saved_descriptors.contains_key(&cell);
        let encoded: BTreeSet<Cell> = d.heightmaps.iter().map(|h| h.cell).filter(|&cell| !reused(self, cell)
            && (self.authored_tiles.contains(&cell) || !self.document.heightmaps.iter().any(|source| source.cell == cell))).collect();
        let steps = self.png16_steps(&d, encoded)?;
        let mut files = BTreeMap::new();
        for h in &mut d.heightmaps {
            cancellation::checkpoint()?;
            if let Some((step, grid)) = steps.get(&h.cell) {
                let (bytes, offset) = encode_heightmap_step(grid, *step)?;
                h.offset_cm = offset;
                h.step_cm = *step;
                h.path = format!("editor/{}.png", sha256(&bytes));
                files.insert(h.path.clone(), bytes);
            } else if reused(self, h.cell) {
                let saved = &self.saved_descriptors[&h.cell];
                h.path = saved.path.clone();
                h.offset_cm = saved.offset_cm;
                h.step_cm = saved.step_cm;
            }
        }
        // Keep source identities needed by earlier file commands after Save As.
        // Publishing derived PNG identities must never rewrite those commands.
        for path in references(&d)?
            .into_iter()
            .chain(references(&self.document)?)
        {
            if path == "document.json" || files.contains_key(&path) {
                continue;
            }
            files.insert(path.clone(), self.resources.read(&path)?);
        }
        SNAPSHOT_PAYLOADS.check(files.values().map(Vec::len).sum::<usize>() as u64, || error("E_BUDGET", "snapshot payloads exceed 64 MiB"))?;
        validate_heightmaps(&d, &files)?;
        validate_assets(&d, &files)?;
        validate_course_files(&d, &files)?;
        Ok((d, files))
    }
    /// PNG16 step per encoded (modified or new) cell; kept PNGs are never
    /// re-encoded. A cell that fits 655.35 m at 1 cm stays exact (byte-identical
    /// when its stored neighbours agree with memory). Otherwise it uses the
    /// smallest power-of-two step that fits, flooring heights onto a lattice
    /// through `terrain_base_cm`. A sample shared with a kept cell (or flat
    /// terrain) keeps that cell's stored value exactly; a sample shared only by
    /// encoded cells takes the coarsest of their steps, so nested lattices agree.
    /// When a stored shared sample is not on the required lattice, the save is
    /// refused (`E_HEIGHTMAP`) instead of changing the kept cell.
    fn png16_steps(&mut self, d: &MapDocument, encoded: BTreeSet<Cell>) -> Result<BTreeMap<Cell, (u32, HeightGrid)>> {
        let base = d.terrain_base_cm;
        let floor = |v: i64, step: u32| base + (v - base).div_euclid(i64::from(step)) * i64::from(step);
        let around = |c: Cell| (-1..=1).flat_map(move |y| (-1..=1).map(move |x| Cell { x: c.x + x, y: c.y + y }));
        // Stored grids of kept neighbours: their PNG, not working memory, is what
        // a reader decodes (a stepped save leaves memory exact).
        let mut kept: BTreeMap<Cell, HeightGrid> = BTreeMap::new();
        for &cell in &encoded {
            let pending: Vec<Cell> = around(cell).filter(|n| !encoded.contains(n) && !kept.contains_key(n)).collect();
            for n in pending {
                let Some(h) = d.heightmaps.iter().find(|h| h.cell == n) else { continue };
                let stored = if !self.modified.contains(&n) { self.saved_descriptors.get(&n).unwrap_or(h) } else { h };
                let bytes = self.resources.read(&stored.path)?;
                kept.insert(n, decode_heightmap(stored, d.cell_size_cm, &bytes)?);
            }
        }
        let mut grids = BTreeMap::new();
        for &cell in &encoded {
            grids.insert(cell, self.grid(cell)?);
        }
        let mut steps: BTreeMap<Cell, u32> = encoded.iter().map(|c| (*c, 1)).collect();
        loop {
            let mut changed = false;
            let mut planned = BTreeMap::new();
            for &cell in &encoded {
                cancellation::checkpoint()?;
                let grid = &grids[&cell];
                let (side, step) = (grid.side, steps[&cell]);
                let n = side as i32 - 1;
                let mut values: Vec<i64> = grid.heights_cm.iter().map(|v| floor(*v, step)).collect();
                let mut fixed = Vec::new();
                for k in (0..side).chain((side - 1) * side..side * side)
                    .chain((0..side).map(|j| j * side)).chain((0..side).map(|j| j * side + side - 1)) {
                    let (i, j) = ((k % side) as i32, (k / side) as i32);
                    let (mut shared, mut exact) = (step, None);
                    for dy in [-1, 0, 1].into_iter().filter(|dy| (*dy < 0 && j == 0) || *dy == 0 || (*dy > 0 && j == n)) {
                        for dx in [-1, 0, 1].into_iter().filter(|dx| (*dx < 0 && i == 0) || *dx == 0 || (*dx > 0 && i == n)) {
                            let other = Cell { x: cell.x + dx, y: cell.y + dy };
                            if (dx, dy) == (0, 0) || !d.has_cell(other) { continue; }
                            if let Some(s) = steps.get(&other) {
                                shared = shared.max(*s);
                            } else if exact.is_none() {
                                exact = Some(match kept.get(&other) {
                                    Some(g) if g.side != side => return Err(error("E_SEAM", "adjacent heightmap spacing must match")),
                                    Some(g) => g.heights_cm[(j - dy * n) as usize * side + (i - dx * n) as usize],
                                    None => base,
                                });
                            }
                        }
                    }
                    values[k] = match exact {
                        Some(v) => { fixed.push(v); v }
                        None => floor(grid.heights_cm[k], shared),
                    };
                }
                let (lo, hi) = (values.iter().min().unwrap(), values.iter().max().unwrap());
                if (hi - lo) / i64::from(step) > 65535 {
                    if step >= 64 {
                        return Err(error("E_HEIGHTMAP", "terrain relief exceeds the PNG16 step range"));
                    }
                    steps.insert(cell, step * 2);
                    changed = true;
                    continue;
                }
                if fixed.iter().any(|v| (v - base) % i64::from(step) != 0) {
                    return Err(error("E_HEIGHTMAP", format!(
                        "cell ({}, {}): relief above 655.35 m needs {step} cm steps, but its unchanged neighbour's shared edge is not on that step",
                        cell.x, cell.y)));
                }
                if values.iter().any(|v| v.abs() > 1_000_000) {
                    return Err(error("E_HEIGHTMAP", "stepped terrain height leaves the v1 range"));
                }
                planned.insert(cell, (step, HeightGrid { side, heights_cm: values }));
            }
            if !changed {
                let coarsest = steps.values().copied().max().unwrap_or(1);
                if coarsest > 1 {
                    profile::note(mapkit_core::Warning {
                        code: "heightmap_step".into(), actual: u64::from(coarsest), limit: 1,
                        message: "Terrain relief above 655.35 m in one cell is saved in coarser height steps (cm)".into(),
                        runtime_rejects: false,
                    });
                }
                return Ok(planned);
            }
        }
    }
    pub fn validate_memory(&mut self) -> Result<()> {
        let d = self.composed_document();
        d.validate()?;
        self.verify_memory(&d)
    }
    /// Resource, asset, course and terrain checks for an already valid document.
    fn verify_memory(&mut self, d: &MapDocument) -> Result<()> {
        self.resources.verify()?;
        let mut files = BTreeMap::new();
        let mut total = 0;
        for path in d.assets.iter().flat_map(|a| a.paths()).chain(
            d.courses
                .iter()
                .filter_map(|c| c.validation.as_ref().map(|v| &v.path)),
        ) {
            if files.contains_key(path) {
                continue;
            }
            let bytes = self.resources.read(path)?;
            total += bytes.len();
            WORKING_RESOURCES.check(total as u64, || error("E_BUDGET", "working resources exceed 64 MiB"))?;
            files.insert(path.clone(), bytes);
        }
        validate_assets(&d, &files)?;
        validate_course_files(&d, &files)?;
        let cells: BTreeSet<_> = d
            .heightmaps
            .iter()
            .flat_map(|h| {
                [
                    h.cell,
                    Cell {
                        x: h.cell.x - 1,
                        y: h.cell.y,
                    },
                    Cell {
                        x: h.cell.x,
                        y: h.cell.y - 1,
                    },
                ]
            })
            .filter(|c| d.has_cell(*c))
            .collect();
        for c in cells {
            for n in [Cell { x: c.x + 1, y: c.y }, Cell { x: c.x, y: c.y + 1 }] {
                cancellation::checkpoint()?;
                if !d.has_cell(n) {
                    continue;
                }
                let a = self.grid(c)?;
                let b = self.grid(n)?;
                if a.side != b.side {
                    return Err(error("E_SEAM", "adjacent heightmap spacing must match"));
                }
                for i in 0..a.side {
                    let (av, bv) = if c.x != n.x {
                        (
                            a.heights_cm[i * a.side + a.side - 1],
                            b.heights_cm[i * b.side],
                        )
                    } else {
                        (a.heights_cm[(a.side - 1) * a.side + i], b.heights_cm[i])
                    };
                    if av != bv {
                        return Err(error("E_SEAM", "adjacent memory height samples differ"));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn validate_changed_geometry(&mut self) -> Result<()> {
        if self.document.roads.is_empty() && self.document.assembled_track.is_none() {
            return Ok(());
        }
        let cells: Vec<_> = self.modified.iter().copied().collect();
        for batch in cells.chunks(256) {
            self.validate_cells_with(batch, cell_triangles())?;
        }
        Ok(())
    }
}

/// Lossless PNG16 (`step_cm` 1). Returns the PNG and its `offset_cm`.
pub fn encode_heightmap(grid: &HeightGrid) -> Result<(Vec<u8>, i64)> {
    encode_heightmap_step(grid, 1)
}
/// PNG16 with `height = offset_cm + sample * step_cm`. Every height must lie
/// on the step lattice through the minimum (always true for step 1).
pub fn encode_heightmap_step(grid: &HeightGrid, step: u32) -> Result<(Vec<u8>, i64)> {
    let low = *grid
        .heights_cm
        .iter()
        .min()
        .ok_or_else(|| error("E_HEIGHTMAP", "empty grid"))?;
    let high = *grid.heights_cm.iter().max().unwrap();
    let step_cm = i64::from(step);
    if !(1..=100).contains(&step_cm)
        || (high - low) / step_cm > 65535
        || low.abs() > 1_000_000
        || grid.heights_cm.iter().any(|v| (v - low) % step_cm != 0)
    {
        return Err(error(
            "E_HEIGHTMAP",
            "lossless PNG16 requires a height range within 655.35 m",
        ));
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, grid.side as u32, grid.side as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder
            .write_header()
            .map_err(|e| error("E_HEIGHTMAP", e.to_string()))?;
        let raw: Vec<u8> = grid
            .heights_cm
            .iter()
            .flat_map(|v| (((*v - low) / step_cm) as u16).to_be_bytes())
            .collect();
        writer
            .write_image_data(&raw)
            .map_err(|e| error("E_HEIGHTMAP", e.to_string()))?;
    }
    Ok((bytes, low))
}

#[derive(Clone, serde::Deserialize)]
pub struct BrushOptions {
    pub mode: String,
    pub radius_cm: f64,
    pub rate_cm_s: f64,
    pub steepness: f64,
    pub strength: f64,
    pub target_cm: Option<f64>,
}
pub struct Brush {
    pub options: BrushOptions,
    pub original: WorkingSnapshot,
    pub previous: [f64; 2],
    pub changes: BTreeMap<[i32; 2], (i64, f64)>,
}
impl Brush {
    pub fn begin(
        snapshot: &mut WorkingSnapshot,
        point: [f64; 2],
        mut options: BrushOptions,
    ) -> Result<Self> {
        if snapshot
            .document
            .heightmaps
            .iter()
            .any(|h| h.spacing_cm as i64 != snapshot.spacing())
        {
            return Err(error("E_SEAM", "terrain grids must use the same spacing"));
        }
        if point.iter().any(|p| !p.is_finite())
            || options
                .target_cm
                .is_some_and(|h| !h.is_finite() || !(-1_000_000.0..=1_000_000.0).contains(&h))
            || !options.radius_cm.is_finite()
            || options.radius_cm < snapshot.spacing() as f64
            || options.radius_cm > snapshot.document.cell_size_cm as f64 * 2.0
            || !options.rate_cm_s.is_finite()
            || !(0.0..=10000.0).contains(&options.rate_cm_s)
            || !(0.0..=1.0).contains(&options.steepness)
            || !(0.0..=1.0).contains(&options.strength)
            || !["raise", "lower", "flatten", "smooth"].contains(&options.mode.as_str())
        {
            return Err(error("E_BRUSH", "invalid brush options"));
        }
        if options.target_cm.is_none() {
            options.target_cm =
                Some(snapshot.surface_height(point.map(|v| v.round() as i64))? as f64);
        }
        Ok(Self {
            options,
            original: snapshot.clone(),
            previous: point,
            changes: BTreeMap::new(),
        })
    }
    pub fn step(
        &mut self,
        snapshot: &mut WorkingSnapshot,
        point: [f64; 2],
        seconds: f64,
    ) -> Result<BTreeSet<Cell>> {
        if !seconds.is_finite() || seconds < 0.0 || point.iter().any(|p| !p.is_finite()) {
            return Err(error("E_BRUSH", "invalid timed input"));
        }
        let spacing = snapshot.spacing() as f64;
        let distance = (point[0] - self.previous[0]).hypot(point[1] - self.previous[1]);
        let steps = (distance / (spacing.min(self.options.radius_cm / 4.0)))
            .ceil()
            .max(1.0) as usize;
        CHANGED.check(steps as u64, || error("E_BUDGET", "brush path exceeds sample allowance"))?;
        let dt = seconds / steps as f64;
        let mut cells = BTreeSet::new();
        for step in 1..=steps {
            cancellation::checkpoint()?;
            let p = [0, 1].map(|a| {
                self.previous[a] + (point[a] - self.previous[a]) * step as f64 / steps as f64
            });
            let origin = snapshot.document.bounds.min;
            let dim = snapshot.dimensions();
            let r = self.options.radius_cm;
            let lo =
                [0, 1].map(|a| (((p[a] - r - origin[a] as f64) / spacing).ceil() as i32).max(0));
            let hi = [0, 1]
                .map(|a| (((p[a] + r - origin[a] as f64) / spacing).floor() as i32).min(dim[a]));
            let mut updates = Vec::new();
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let sample = [x, y];
                    let position = snapshot.point(sample);
                    let t = ((position[0] as f64 - p[0]).hypot(position[1] as f64 - p[1])) / r;
                    if t >= 1.0 {
                        continue;
                    }
                    let weight = (1.0 - t).powf(4.0 * (1.0 - self.options.steepness) + 0.25);
                    let before = snapshot.sample(sample)?;
                    let current = self.changes.get(&sample).map_or(before as f64, |v| v.1);
                    let target = if self.options.mode == "smooth" {
                        let mut sum = 0;
                        for o in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                            sum += snapshot.sample([x + o[0], y + o[1]])?;
                        }
                        sum as f64 / 4.0
                    } else {
                        self.options.target_cm.unwrap()
                    };
                    let after = match self.options.mode.as_str() {
                        "raise" => current + self.options.rate_cm_s * dt * weight,
                        "lower" => current - self.options.rate_cm_s * dt * weight,
                        _ => {
                            current
                                + (target - current)
                                    * (1.0 - (-self.options.strength * 4.0 * dt * weight).exp())
                        }
                    }
                    .clamp(-1_000_000.0, 1_000_000.0);
                    if after != current {
                        updates.push((sample, before, after));
                    }
                }
            }
            let changed = self.changes.len()
                + updates
                    .iter()
                    .filter(|(p, _, _)| !self.changes.contains_key(p))
                    .count();
            CHANGED.check(changed as u64, || error(
                "E_BUDGET",
                "brush exceeds one million changed samples",
            ))?;
            for (sample, before, after) in updates {
                self.changes.entry(sample).or_insert((before, after)).1 = after;
                if after.round() as i64 != before {
                    snapshot.set_sample(sample, after.round() as i64)?;
                    let p = snapshot.point(sample);
                    let origin = snapshot.document.bounds.min;
                    for y in ((p[1] - origin[1] - 200).div_euclid(3200)).max(0)
                        ..=(p[1] - origin[1] + 200) / 3200
                    {
                        for x in ((p[0] - origin[0] - 200).div_euclid(3200)).max(0)
                            ..=(p[0] - origin[0] + 200) / 3200
                        {
                            cells.insert(Cell {
                                x: x as i32,
                                y: y as i32,
                            });
                        }
                    }
                }
            }
        }
        self.previous = point;
        Ok(cells)
    }
    pub fn delta(&self) -> Vec<i32> {
        self.changes
            .iter()
            .filter(|(_, v)| v.0 != v.1.round() as i64)
            .flat_map(|(p, v)| [p[0], p[1], v.0 as i32, v.1.round() as i32])
            .collect()
    }
}
