//! Bounded source snapshots retain world coordinates and complete dependencies.
use crate::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CellRegion {
    pub min: Cell,
    /// Exclusive upper cell, unlike inclusive point bounds.
    pub end: Cell,
}
impl CellRegion {
    pub fn contains(self, c: Cell) -> bool {
        c.x >= self.min.x && c.y >= self.min.y && c.x < self.end.x && c.y < self.end.y
    }
    pub fn validate(self, d: &MapDocument) -> Result<()> {
        let [nx, ny] = d.cell_dimensions()?;
        if self.min.x < 0
            || self.min.y < 0
            || self.end.x <= self.min.x
            || self.end.y <= self.min.y
            || i64::from(self.end.x) > nx
            || i64::from(self.end.y) > ny
            || i64::from(self.end.x - self.min.x) * i64::from(self.end.y - self.min.y) > 16_384
        {
            return Err(error("E_LIMIT", "invalid or oversized execution region"));
        }
        Ok(())
    }
    pub fn cells(self) -> Result<Vec<Cell>> {
        let width = i64::from(self.end.x) - i64::from(self.min.x);
        let height = i64::from(self.end.y) - i64::from(self.min.y);
        if self.min.x < 0
            || self.min.y < 0
            || width <= 0
            || height <= 0
            || width.saturating_mul(height) > 16_384
        {
            return Err(error("E_LIMIT", "invalid or oversized execution region"));
        }
        Ok((self.min.y..self.end.y)
            .flat_map(|y| (self.min.x..self.end.x).map(move |x| Cell { x, y }))
            .collect())
    }
}

/// Extract only source with a proven local dependency boundary. Roads, graph
/// nodes, zones and repetition rules remain complete. Implicit sidewalk widths
/// and repeated-placement eligibility can depend on distant buildings; retain
/// their full placement/building context instead of silently changing generation.
/// This conservative fallback is intentional and is reported in source byte counts.
pub fn region_source(d: &MapDocument, region: CellRegion) -> Result<MapDocument> {
    region.validate(d)?;
    let mut out = d.clone();
    let mut bounds = d.cell_bounds(region.min)?;
    bounds.max = d
        .cell_bounds(Cell {
            x: region.end.x - 1,
            y: region.end.y - 1,
        })?
        .max;
    // A vegetation competitor up to max spacing away can suppress a local tree.
    // Its footprint and clearance need the same source, across storage seams.
    let margin = d
        .zones
        .iter()
        .map(|z| i64::from(z.spacing_cm))
        .max()
        .unwrap_or(0)
        .max(501)
        + 1000;
    for a in 0..2 {
        bounds.min[a] -= margin;
        bounds.max[a] += margin;
    }
    let hit = |points: &[Point]| {
        (0..2).all(|a| {
            points.iter().any(|p| p[a] <= bounds.max[a])
                && points.iter().any(|p| p[a] >= bounds.min[a])
        })
    };
    if d.recipe_version >= 3
        && d.repetitions.is_empty()
        && d.roads
            .iter()
            .all(|r| r.kind != RoadKind::Ground || r.sidewalk_cm.is_some())
    {
        out.buildings
            .retain(|b| hit(&b.footprint) || b.entrances.iter().any(|p| hit(p)));
        out.placements.retain(|p| {
            hit(&crate::placement::footprint(d, p)) || hit(&[[p.position[0], p.position[2]]])
        });
    }
    out.surface_areas.retain(|a| hit(&a.polygon));
    // Keep immediate height neighbors for exact restored edge validation.
    out.heightmaps.retain(|h| {
        h.cell.x >= region.min.x - 1
            && h.cell.x <= region.end.x
            && h.cell.y >= region.min.y - 1
            && h.cell.y <= region.end.y
    });
    let mut needed: BTreeSet<String> = out
        .placements
        .iter()
        .map(|p| p.asset_id.clone())
        .chain(out.repetitions.iter().map(|p| p.asset_id.clone()))
        .collect();
    loop {
        let before = needed.len();
        for a in &d.assets {
            if needed.contains(&a.id) {
                if let Some(id) = a.material.as_ref().and_then(|m| m.albedo_texture.as_ref()) {
                    needed.insert(id.clone());
                }
            }
        }
        if before == needed.len() {
            break;
        }
    }
    out.assets.retain(|a| needed.contains(&a.id));
    out.normalize();
    Ok(out)
}
