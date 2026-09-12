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
    derive_source(d, region, false)
}

/// Version-2 closure. Version-1 derivation remains byte-for-byte reproducible.
pub fn local_region_source(d: &MapDocument, region: CellRegion) -> Result<MapDocument> {
    derive_source(d, region, true)
}

/// Clone metadata without allocating a transient copy of world geometry.
pub fn source_metadata(d: &MapDocument) -> MapDocument {
    MapDocument {
        indexed_topology: d.indexed_topology,
        map_id: d.map_id.clone(),
        revision: d.revision,
        bounds: d.bounds.clone(),
        cell_size_cm: d.cell_size_cm,
        seed: d.seed,
        recipe_version: d.recipe_version,
        theme: d.theme.clone(),
        terrain_base_cm: d.terrain_base_cm,
        attributions: d.attributions.clone(),
        provenance: d.provenance.clone(),
        heightmaps: vec![],
        nodes: vec![],
        roads: vec![],
        surface_areas: vec![],
        buildings: vec![],
        zones: vec![],
        assets: vec![],
        placements: vec![],
        repetitions: vec![],
    }
}

fn derive_source(d: &MapDocument, region: CellRegion, local: bool) -> Result<MapDocument> {
    region.validate(d)?;
    let mut out = source_metadata(d);
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
    let prune = d.recipe_version >= 3
        && d.repetitions.is_empty()
        && d.roads
            .iter()
            .all(|r| r.kind != RoadKind::Ground || r.sidewalk_cm.is_some());
    out.buildings = d
        .buildings
        .iter()
        .filter(|b| !prune || hit(&b.footprint) || b.entrances.iter().any(|p| hit(p)))
        .cloned()
        .collect();
    out.placements = d
        .placements
        .iter()
        .filter(|p| {
            !prune
                || hit(&crate::placement::footprint(d, p))
                || hit(&[[p.position[0], p.position[2]]])
        })
        .cloned()
        .collect();
    out.surface_areas = d
        .surface_areas
        .iter()
        .filter(|a| hit(&a.polygon))
        .cloned()
        .collect();
    out.heightmaps = d
        .heightmaps
        .iter()
        .filter(|h| {
            h.cell.x >= region.min.x - 1
                && h.cell.x <= region.end.x
                && h.cell.y >= region.min.y - 1
                && h.cell.y <= region.end.y
        })
        .cloned()
        .collect();
    out.repetitions = d.repetitions.clone();
    if local && prune && d.recipe_version == 6 {
        let road_margin = crate::roads::influence_margin(d) + 1000;
        let relevant: BTreeSet<_> = d
            .roads
            .iter()
            .filter(|r| {
                r.points.windows(2).any(|s| {
                    (0..2).all(|a| {
                        s.iter().any(|p| p[a * 2] <= bounds.max[a] + road_margin)
                            && s.iter().any(|p| p[a * 2] >= bounds.min[a] - road_margin)
                    })
                })
            })
            .map(|r| r.id.as_str())
            .collect();
        // Complete authored endpoint stars, not a geometric proximity join.
        let endpoints: BTreeSet<_> = d
            .roads
            .iter()
            .filter(|r| relevant.contains(r.id.as_str()))
            .flat_map(|r| [r.from.as_str(), r.to.as_str()])
            .collect();
        let widest = d
            .roads
            .iter()
            .max_by_key(|r| r.widths_cm.iter().max().copied().unwrap_or(0));
        out.roads = d
            .roads
            .iter()
            .filter(|r| {
                relevant.contains(r.id.as_str())
                    || endpoints.contains(r.from.as_str())
                    || endpoints.contains(r.to.as_str())
                    || widest.is_some_and(|w| w.id == r.id)
            })
            .cloned()
            .collect();
        let nodes: BTreeSet<_> = out.roads.iter().flat_map(|r| [&r.from, &r.to]).collect();
        out.nodes = d
            .nodes
            .iter()
            .filter(|n| nodes.contains(&n.id))
            .cloned()
            .collect();
        out.zones = d
            .zones
            .iter()
            .filter(|z| hit(&z.polygon))
            .cloned()
            .collect();
    } else {
        out.roads = d.roads.clone();
        out.nodes = d.nodes.clone();
        out.zones = d.zones.clone();
    }
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
    out.assets = d
        .assets
        .iter()
        .filter(|a| needed.contains(&a.id))
        .cloned()
        .collect();
    out.normalize();
    Ok(out)
}
