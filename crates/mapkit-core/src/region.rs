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
    derive_source(d, region, false, None)
}

/// Version-2 closure. Version-1 derivation remains byte-for-byte reproducible.
pub fn local_region_source(d: &MapDocument, region: CellRegion) -> Result<MapDocument> {
    derive_source(d, region, true, None)
}

/// Bounded, immutable selection scratch for repeated regional derivation.
/// Like `region_source`, this transforms an already valid document into an
/// ordinary MapDocument; it is NOT a validation receipt or a PreparedMap.
/// Callers must validate returned source before execution. No source is cloned
/// or retained beyond the input borrow, and no caller-supplied bounds are trusted.
pub struct RegionSourcePlan<'a> {
    document: &'a MapDocument,
    local: bool,
    placements: Vec<Bounds>,
    roads: Vec<Bounds>,
    margin: i64,
    prune: bool,
    road_margin: i64,
    widest: Option<usize>,
}

impl<'a> RegionSourcePlan<'a> {
    // Shared authored-object limit: these two arrays never exceed its total.
    const MAX_ENTRIES: usize = 200_000;
    const OVERHEAD: u64 = 256;

    /// Pre-read reservation. Actual entries are checked against this allowance
    /// before allocation, so even a forged serialized size cannot undercharge.
    pub fn allocation_bound(source_bytes: u64) -> u64 {
        source_bytes.min((Self::MAX_ENTRIES * std::mem::size_of::<Bounds>()) as u64)
            + Self::OVERHEAD
    }

    pub fn new(
        document: &'a MapDocument,
        local: bool,
        memory_limit: u64,
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let count = document
            .placements
            .len()
            .checked_add(document.roads.len())
            .filter(|count| *count <= Self::MAX_ENTRIES)
            .ok_or_else(|| error("E_LIMIT", "regional plan object limit exceeded"))?;
        let bytes = (count * std::mem::size_of::<Bounds>()) as u64 + Self::OVERHEAD;
        if bytes > memory_limit {
            return Err(error("E_MEMORY_BUDGET", "regional plan exceeds allowance"));
        }
        let mut prune = document.recipe_version >= 3 && document.repetitions.is_empty();
        let mut widest = None;
        let mut maximum_width = 0;
        for (i, road) in document.roads.iter().enumerate() {
            if i % 64 == 0 {
                check()?;
            }
            prune &= road.kind != RoadKind::Ground || road.sidewalk_cm.is_some();
            let width = road.widths_cm.iter().copied().max().unwrap_or(0);
            // max_by_key keeps the last item on a tie, including zero width.
            if widest.is_none() || width >= maximum_width {
                maximum_width = width;
                widest = Some(i);
            }
        }
        let mut maximum_spacing = 0;
        for (i, zone) in document.zones.iter().enumerate() {
            if i % 64 == 0 {
                check()?;
            }
            maximum_spacing = maximum_spacing.max(i64::from(zone.spacing_cm));
        }
        let mut placements = Vec::with_capacity(if prune { document.placements.len() } else { 0 });
        if prune {
            for (i, placement) in document.placements.iter().enumerate() {
                if i % 64 == 0 {
                    check()?;
                }
                placements.push(point_bounds(crate::placement::footprint(
                    document, placement,
                )));
            }
        }
        let use_local = local && prune && document.recipe_version >= 6;
        let mut roads = Vec::with_capacity(if use_local { document.roads.len() } else { 0 });
        if use_local {
            for (i, road) in document.roads.iter().enumerate() {
                if i % 64 == 0 {
                    check()?;
                }
                roads.push(point_bounds(road.points.iter().map(|p| [p[0], p[2]])));
            }
        }
        let plan = Self {
            document,
            local,
            placements,
            roads,
            margin: maximum_spacing.max(501) + 1000,
            prune,
            road_margin: if use_local {
                crate::roads::width_influence_margin(maximum_width) + 1000
            } else {
                0
            },
            widest: if use_local { widest } else { None },
        };
        check()?;
        Ok(plan)
    }

    pub fn allocated_bytes(&self) -> u64 {
        ((self.placements.capacity() + self.roads.capacity()) * std::mem::size_of::<Bounds>())
            as u64
            + Self::OVERHEAD
    }

    pub fn derive(&self, region: CellRegion) -> Result<MapDocument> {
        derive_source(self.document, region, self.local, Some(self))
    }
}

fn point_bounds(points: impl IntoIterator<Item = Point>) -> Bounds {
    let mut bounds = Bounds {
        min: [i64::MAX; 2],
        max: [i64::MIN; 2],
    };
    for p in points {
        for a in 0..2 {
            bounds.min[a] = bounds.min[a].min(p[a]);
            bounds.max[a] = bounds.max[a].max(p[a]);
        }
    }
    bounds
}

fn bounds_hit(candidate: &Bounds, bounds: &Bounds) -> bool {
    (0..2).all(|a| candidate.min[a] <= bounds.max[a] && candidate.max[a] >= bounds.min[a])
}

fn competition_margin(d: &MapDocument) -> i64 {
    d.zones
        .iter()
        .map(|z| i64::from(z.spacing_cm).max(
            z.tree.as_ref().map_or(0, |t| i64::from(t.radius_cm) * 2 + i64::from(t.clearance_cm) + 1)))
        .max()
        .unwrap_or(0)
        .max(501)
        + 1000
}

fn can_prune(d: &MapDocument) -> bool {
    d.recipe_version >= 3
        && d.repetitions.is_empty()
        && d.roads
            .iter()
            .all(|r| r.kind != RoadKind::Ground || r.sidewalk_cm.is_some())
}

fn widest_road(d: &MapDocument) -> Option<usize> {
    d.roads
        .iter()
        .enumerate()
        .max_by_key(|(_, r)| r.widths_cm.iter().max().copied().unwrap_or(0))
        .map(|(i, _)| i)
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

fn derive_source(
    d: &MapDocument,
    region: CellRegion,
    local: bool,
    plan: Option<&RegionSourcePlan<'_>>,
) -> Result<MapDocument> {
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
    let margin = plan.map_or_else(|| competition_margin(d), |p| p.margin);
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
    let prune = plan.map_or_else(|| can_prune(d), |p| p.prune);
    out.buildings = d
        .buildings
        .iter()
        .filter(|b| !prune || hit(&b.footprint) || b.entrances.iter().any(|p| hit(p)))
        .cloned()
        .collect();
    out.placements = d
        .placements
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            !prune
                || plan.map_or_else(
                    || hit(&crate::placement::footprint(d, p)),
                    |plan| bounds_hit(&plan.placements[*i], &bounds),
                )
                || hit(&[[p.position[0], p.position[2]]])
        })
        .map(|(_, p)| p.clone())
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
    if local && prune && d.recipe_version >= 6 {
        let road_margin = plan.map_or_else(
            || crate::roads::influence_margin(d) + 1000,
            |p| p.road_margin,
        );
        let road_bounds = Bounds {
            min: bounds.min.map(|v| v - road_margin),
            max: bounds.max.map(|v| v + road_margin),
        };
        let relevant: BTreeSet<_> = d
            .roads
            .iter()
            .enumerate()
            .filter(|(i, r)| {
                plan.is_none_or(|p| bounds_hit(&p.roads[*i], &road_bounds))
                    && r.points.windows(2).any(|s| {
                        (0..2).all(|a| {
                            s.iter().any(|p| p[a * 2] <= bounds.max[a] + road_margin)
                                && s.iter().any(|p| p[a * 2] >= bounds.min[a] - road_margin)
                        })
                    })
            })
            .map(|(_, r)| r.id.as_str())
            .collect();
        // Complete authored endpoint stars, not a geometric proximity join.
        let endpoints: BTreeSet<_> = d
            .roads
            .iter()
            .filter(|r| relevant.contains(r.id.as_str()))
            .flat_map(|r| [r.from.as_str(), r.to.as_str()])
            .collect();
        let widest = plan
            .map_or_else(|| widest_road(d), |p| p.widest)
            .map(|i| &d.roads[i]);
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
        .chain(out.zones.iter().filter_map(|z| z.tree.as_ref().map(|t| t.asset_id.clone())))
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
