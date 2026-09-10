//! Allocation planning for successful generation; never materializes geometry.
use super::*;

/// Conservative counts, not process/GPU byte measurements. Runtime adapters own
/// representation-specific byte allowances. Estimates do not affect world hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationCost {
    pub triangles: u64,
    /// Bounded recipe-2 road planning/subdivision workspace, separate from output.
    pub generation_scratch_bytes: u64,
    pub objects: u64,
    /// Upper bound for optional occupied-volume records, independent of face clipping.
    pub occupied_solids: u64,
    pub building_prisms: u64,
    pub asset_convexes: u64,
    pub height_samples: u64,
    pub max_object_id_bytes: u64,
}

fn bounds(points: impl Iterator<Item = Point>, margin: i64) -> Bounds {
    let mut out = Bounds {
        min: [i64::MAX; 2],
        max: [i64::MIN; 2],
    };
    for p in points {
        for axis in 0..2 {
            out.min[axis] = out.min[axis].min(p[axis] - margin);
            out.max[axis] = out.max[axis].max(p[axis] + margin);
        }
    }
    out
}

// A triangle clipped by four half-planes has at most seven vertices (five
// triangles). Entirely contained triangles retain at most their original count.
fn clip_factor(shape: &Bounds, cell: &Bounds) -> u64 {
    if (0..2).any(|a| shape.max[a] < cell.min[a] || shape.min[a] > cell.max[a]) {
        0
    } else if (0..2).all(|a| shape.min[a] >= cell.min[a] && shape.max[a] <= cell.max[a]) {
        1
    } else {
        5
    }
}

pub fn estimate_generation(
    d: &MapDocument,
    cell: Cell,
    max_triangles: usize,
) -> Result<GenerationCost> {
    d.validate()?;
    estimate_validated(d, cell, max_triangles)
}

pub(crate) fn estimate_validated(
    d: &MapDocument,
    cell: Cell,
    max_triangles: usize,
) -> Result<GenerationCost> {
    let area = d.cell_bounds(cell)?;
    let descriptor = d.heightmaps.iter().find(|h| h.cell == cell);
    let spacing = descriptor.map_or(d.cell_size_cm, |h| h.spacing_cm) as i64;
    let side = d.cell_size_cm as u64 / spacing as u64 + 1;
    let mut cost = GenerationCost {
        triangles: 0,
        generation_scratch_bytes: (if d.recipe_version >= 2 && !d.roads.is_empty() {
            crate::roads::SCRATCH_BYTES
        } else {
            0
        }) + if d.recipe_version >= 3 {
            crate::placement::SCRATCH_BYTES
        } else {
            0
        },
        objects: 0,
        occupied_solids: 0,
        building_prisms: 0,
        asset_convexes: 0,
        height_samples: if descriptor.is_some() { side * side } else { 0 },
        max_object_id_bytes: 7,
    };
    let mut add = |count: u64, id_bytes: usize| {
        cost.triangles = cost.triangles.saturating_add(count);
        cost.max_object_id_bytes = cost.max_object_id_bytes.max(id_bytes as u64);
    };
    // Include an extra boundary row when the map ends exactly on a sample; its
    // degenerate clipped triangles may disappear, but must never be undercounted.
    let nx = ((area.max[0] - area.min[0]) / spacing + 1).min(side as i64 - 1) as u64;
    let ny = ((area.max[1] - area.min[1]) / spacing + 1).min(side as i64 - 1) as u64;
    let partial = (area.max[0] - area.min[0]) < d.cell_size_cm as i64
        || (area.max[1] - area.min[1]) < d.cell_size_cm as i64;
    add(nx * ny * 2 * if partial { 5 } else { 1 }, 7);
    let road_margin = crate::roads::influence_margin(d);
    for r in &d.roads {
        for (index, points) in r.points.windows(2).enumerate() {
            let shape = bounds(
                points.iter().map(|p| [p[0], p[2]]),
                r.widths_cm[index] as i64 / 2 + 1,
            );
            let count = match r.kind {
                RoadKind::Tunnel => 8,
                RoadKind::Underpass => 6,
                _ => 2,
            };
            if d.recipe_version == 1 {
                add(count * clip_factor(&shape, &area), r.id.len());
            } else {
                // Corridors plus their two aprons. This is a source-derived
                // output allowance, also enforced by recipe-2 generation.
                let local = bounds(points.iter().map(|p| [p[0], p[2]]), road_margin);
                if clip_factor(&local, &area) != 0 {
                    let span = |axis: usize| {
                        ((local.max[axis].min(area.max[axis])
                            - local.min[axis].max(area.min[axis]))
                        .max(0)
                            / spacing
                            + 2) as u64
                    };
                    let touched = span(0).saturating_mul(span(1));
                    add(touched.saturating_mul(128).saturating_add(256), r.id.len());
                    if d.recipe_version >= 3 {
                        let width = crate::placement::sidewalk_width(d, r) as i64;
                        if width > 0 {
                            let local_span = ((local.max[0].min(area.max[0])
                                - local.min[0].max(area.min[0]))
                            .max(0)
                                + (local.max[1].min(area.max[1]) - local.min[1].max(area.min[1]))
                                    .max(0))
                                / 200
                                + 4;
                            add(
                                local_span as u64 * 96 * (width / spacing + 2) as u64,
                                r.id.len() + 9,
                            );
                        }
                    }
                }
            }
        }
    }
    for building in &d.buildings {
        let shape = bounds(building.footprint.iter().copied(), 0);
        let n = (building.footprint.len()
            + building.holes.iter().map(Vec::len).sum::<usize>()
            + 2 * building.holes.len()) as u64;
        if d.recipe_version >= 3 {
            let parts = if building.roof == "gable" { 4 } else { n - 2 };
            let clipped = parts * clip_factor(&shape, &area);
            cost.building_prisms = cost.building_prisms.saturating_add(clipped);
            if clipped > 0 {
                cost.occupied_solids = cost.occupied_solids.saturating_add(parts);
            }
            add(clipped * 8, building.id.len());
            continue;
        }
        if clip_factor(&shape, &area) != 0 {
            cost.occupied_solids = cost.occupied_solids.saturating_add(n - 2);
        }
        add(
            (n - 2 + 2 * n) * clip_factor(&shape, &area),
            building.id.len(),
        );
    }
    for zone in &d.zones {
        if zone.density_per_mille == 0 {
            continue;
        }
        let shape = bounds(zone.polygon.iter().copied(), 0);
        if clip_factor(&shape, &area) == 0 {
            continue;
        }
        let spacing = zone.spacing_cm as i64;
        let jitter = if zone.kind == ZoneKind::Forest {
            spacing / 3
        } else {
            0
        };
        let mut candidates = 1u64;
        for axis in 0..2 {
            let min = shape.min[axis].max(area.min[axis]) - jitter;
            let max = shape.max[axis].min(area.max[axis]) + jitter;
            candidates = candidates
                .saturating_mul((max.div_euclid(spacing) - min.div_euclid(spacing) + 2) as u64);
        }
        // Same generator per-zone candidate ceiling; successful output cannot
        // exceed this even when the domain bounds span many cells.
        candidates = candidates.min(300_000);
        cost.objects = cost.objects.saturating_add(candidates);
        cost.occupied_solids = cost.occupied_solids.saturating_add(candidates);
        add(candidates.saturating_mul(12 * 5), zone.id.len() + 42);
    }
    let repeated = if d.recipe_version >= 3 {
        crate::placement::repeated(d)?
    } else {
        vec![]
    };
    if d.recipe_version >= 3 {
        cost.generation_scratch_bytes = cost
            .generation_scratch_bytes
            .saturating_add((d.placements.len() + repeated.len()) as u64 * 512);
    }
    for placement in d.placements.iter().chain(&repeated) {
        let builtin = if d.recipe_version >= 3 {
            crate::placement::builtin(&placement.asset_id)
        } else {
            None
        };
        let collision = builtin.as_ref().unwrap_or_else(|| {
            &d.assets
                .iter()
                .find(|a| a.id == placement.asset_id)
                .unwrap()
                .collision
        });
        if let Some(asset) = d.assets.iter().find(|a| a.id == placement.asset_id) {
            for c in &asset.convex_collision {
                let shape = c.placed(placement);
                let factor = clip_factor(&shape.bounds(), &area);
                if factor > 0 {
                    cost.asset_convexes += 1;
                    cost.occupied_solids += 1;
                    add(shape.faces.len() as u64 * factor, placement.id.len());
                }
            }
        }
        for proxy in collision {
            let mut center = proxy.center;
            let mut size = proxy.size_cm;
            for _ in 0..placement.quarter_turns {
                center = [-center[2], center[1], center[0]];
                size.swap(0, 2);
            }
            let center = [
                placement.position[0] + center[0],
                placement.position[2] + center[2],
            ];
            let half = [size[0] as i64 / 2, size[2] as i64 / 2];
            let shape = Bounds {
                min: [center[0] - half[0], center[1] - half[1]],
                max: [
                    center[0] - half[0] + size[0] as i64,
                    center[1] - half[1] + size[2] as i64,
                ],
            };
            if clip_factor(&shape, &area) != 0 {
                cost.occupied_solids = cost.occupied_solids.saturating_add(1);
            }
            add(12 * clip_factor(&shape, &area), placement.id.len());
        }
        if d.cell_at([placement.position[0], placement.position[2]]) == Some(cell) {
            cost.objects = cost.objects.saturating_add(1);
        }
    }
    for placement in d.placements.iter().chain(&repeated) {
        cost.max_object_id_bytes = cost
            .max_object_id_bytes
            .max(placement.id.len() as u64)
            .max(placement.asset_id.len() as u64);
    }
    // A generation exceeding its triangle limit fails rather than returning a
    // larger chunk. The reservation bounds successful output, not failed work.
    cost.triangles = cost.triangles.min(max_triangles.min(2_000_000) as u64);
    Ok(cost)
}
