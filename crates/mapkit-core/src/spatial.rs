//! Integer-centimetre topology; safe to query even before document validation.
use super::*;

impl MapDocument {
    pub(crate) fn cell_dimensions(&self) -> Result<[i64; 2]> {
        let dimensions = self.source_dimensions()?;
        if !self.indexed_topology && dimensions[0] * dimensions[1] > 16_384 {
            return Err(error("E_LIMIT", "too many map cells"));
        }
        Ok(dimensions)
    }
    fn source_dimensions(&self) -> Result<[i64; 2]> {
        let size = i64::from(self.cell_size_cm);
        if !(200..=102_400).contains(&size)
            || size % 200 != 0
            || (0..2).any(|a| {
                self.bounds.min[a].unsigned_abs() > 10_000_000
                    || self.bounds.max[a].unsigned_abs() > 10_000_000
                    || self.bounds.min[a] >= self.bounds.max[a]
            })
        {
            return Err(error("E_DOCUMENT", "invalid cell topology"));
        }
        let dimensions = std::array::from_fn::<_, 2, _>(|a| {
            (self.bounds.max[a] - self.bounds.min[a] + size - 1) / size
        });
        Ok(dimensions)
    }
    /// Constant-space topology query; this never allocates one record per cell.
    pub fn cell_count(&self) -> Result<u64> {
        let [x, y] = self.source_dimensions()?;
        Ok((x * y) as u64)
    }
    pub fn cells(&self) -> Vec<Cell> {
        let Ok([nx, ny]) = self.cell_dimensions() else {
            return vec![];
        };
        if nx * ny > 16_384 {
            return vec![];
        }
        (0..ny as i32)
            .flat_map(|y| (0..nx as i32).map(move |x| Cell { x, y }))
            .collect()
    }
    pub fn has_cell(&self, c: Cell) -> bool {
        let Ok([nx, ny]) = self.cell_dimensions() else {
            return false;
        };
        c.x >= 0 && c.y >= 0 && i64::from(c.x) < nx && i64::from(c.y) < ny
    }
    pub fn cell_at(&self, p: Point) -> Option<Cell> {
        self.cell_dimensions().ok()?;
        if !self.bounds.contains(p) {
            return None;
        }
        let s = self.cell_size_cm as i64;
        Some(Cell {
            x: ((p[0].min(self.bounds.max[0] - 1) - self.bounds.min[0]) / s) as i32,
            y: ((p[1].min(self.bounds.max[1] - 1) - self.bounds.min[1]) / s) as i32,
        })
    }
    pub fn window(&self, p: Point) -> Vec<Cell> {
        let Some(c) = self.cell_at(p) else {
            return vec![];
        };
        (-1..=1)
            .flat_map(|dy| {
                (-1..=1).map(move |dx| Cell {
                    x: c.x + dx,
                    y: c.y + dy,
                })
            })
            .filter(|c| self.has_cell(*c))
            .collect()
    }
    pub fn cell_bounds(&self, c: Cell) -> Result<Bounds> {
        if !self.has_cell(c) {
            return Err(error("E_CELL", "cell outside map"));
        }
        let s = self.cell_size_cm as i64;
        let min = [
            self.bounds.min[0] + c.x as i64 * s,
            self.bounds.min[1] + c.y as i64 * s,
        ];
        Ok(Bounds {
            min,
            max: [
                (min[0] + s).min(self.bounds.max[0]),
                (min[1] + s).min(self.bounds.max[1]),
            ],
        })
    }

    /// Collision preparation includes the complete declared sweep and landing
    /// envelope of structures touching the ordinary driving window. Consumers
    /// still apply their unchanged cell/memory caps before materialization.
    pub fn driving_window(&self, p: Point) -> Vec<Cell> {
        let mut cells = self.window(p);
        let base = cells.clone();
        for g in &self.gimmicks {
            if !base.iter().any(|c| self.cell_bounds(*c).is_ok_and(|b| g.intersects(&b))) { continue; }
            let Some(lo) = self.cell_at([g.safety_min_cm[0], g.safety_min_cm[2]]) else { continue; };
            let Some(hi) = self.cell_at([g.safety_max_cm[0], g.safety_max_cm[2]]) else { continue; };
            for y in lo.y..=hi.y { for x in lo.x..=hi.x {
                let cell = Cell { x, y };
                if !cells.contains(&cell) { cells.push(cell); }
            }}
        }
        cells.sort_by_key(|c| (c.y, c.x));
        cells
    }
}
