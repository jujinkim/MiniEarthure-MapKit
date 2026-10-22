//! Allocation planning for successful generation; never materializes geometry.
//! Every count is an upper bound of what generation emits, because generation
//! fails closed above `triangles`. Cells without a heightmap or assembled track
//! pre-run the ground arrangement (no triangulation) and bound sidewalks from
//! that ground count and the nearby sidewalk segments; other cells keep the
//! sampled-terrain allowances.
use super::*;

/// Conservative counts, not process/GPU byte measurements. Runtime adapters own
/// representation-specific byte allowances. Estimates do not affect world hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationCost {
    pub water_bytes: u64,
    pub gimmick_bytes: u64,
    pub triangles: u64,
    /// Bounded road road planning/subdivision workspace, separate from output.
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
    estimate_validated(d, cell, max_triangles, None)
}

pub(crate) fn estimate_validated(
    d: &MapDocument,
    cell: Cell,
    max_triangles: usize,
    barriers: Option<&crate::barrier::Spans>,
) -> Result<GenerationCost> {
    let area = d.cell_bounds(cell)?;
    let descriptor = d.heightmaps.iter().find(|h| h.cell == cell);
    let source_spacing = descriptor.map_or(d.cell_size_cm, |h| h.spacing_cm) as i64;
    let spacing=if d.assembled_track.as_ref().is_some_and(|a|a.terrain_integration()) {source_spacing.min(200)} else {source_spacing};
    let side = d.cell_size_cm as u64 / spacing as u64 + 1;
    let mut cost = GenerationCost {
        water_bytes: crate::water::candidates(d, &area)?.iter().map(|&i| d.water_bodies[i].memory_bytes()).sum(),
        gimmick_bytes: d.grind_lines.iter().filter(|g|g.intersects(&area)).map(|g|g.memory_bytes()).sum::<u64>() + d.gimmicks.iter().filter(|g| g.intersects(&area)).map(|g| g.memory_bytes()).sum::<u64>(),
        triangles: 0,
        generation_scratch_bytes: (if !d.roads.is_empty() || !d.surface_areas.is_empty() {
            crate::roads::SCRATCH_BYTES * { 2 }
        } else {
            0
        }) + { crate::placement::SCRATCH_BYTES },
        objects: 0,
        occupied_solids: d.gimmicks.iter().filter(|g| g.intersects(&area)).map(|g| g.occupied_count()).sum(),
        building_prisms: 0,
        asset_convexes: 0,
        height_samples: if descriptor.is_some() { (d.cell_size_cm as u64/source_spacing as u64+1).pow(2) } else { 0 },
        max_object_id_bytes: d.gimmicks.iter().map(|g| g.id.len() as u64).max().unwrap_or(7).max(7),
    };
    if let Some(track) = &d.assembled_track {
        let (triangles, solids, scratch) = crate::assembled_track::cost(track, &area)?;
        cost.generation_scratch_bytes += scratch;
        cost.triangles += triangles;
        if track.terrain_integration() {
            // Every terrain tile may be subdivided by local track edges. Bound
            // those arrangements as well as the track's ordinary clipped faces.
            for piece in &track.pieces { for path in [&piece.path,&piece.alternate_path] {for pair in path.windows(2) {
                let radius=pair[0].lateral_cm.max(pair[1].lateral_cm) as i64+200;
                let shape=bounds(pair.iter().map(|s|[s.position_cm[0],s.position_cm[2]]),radius);
                if clip_factor(&shape,&area)==0 {continue;}
                let span=|axis:usize| ((shape.max[axis].min(area.max[axis])-shape.min[axis].max(area.min[axis])).max(0)/spacing+2) as u64;
                cost.triangles=cost.triangles.saturating_add(span(0)*span(1)*96);
            }}}
            let local=track.pieces.iter().filter(|p| crate::road_plan::hit(&[p.reserved_min_cm,p.reserved_max_cm],&area,20)).count() as u64;
            cost.triangles+=local*60;
            cost.occupied_solids+=local;
            cost.generation_scratch_bytes+=crate::roads::SCRATCH_BYTES;
        }
        cost.occupied_solids += solids;
        cost.max_object_id_bytes = cost.max_object_id_bytes.max(32);
    }
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
    // Flat cells (one terrain tile, no decoded heights) count the real ground
    // arrangement; sampled or track-integrated terrain keeps the tile allowances.
    let flat = descriptor.is_none() && d.assembled_track.is_none();
    let plan = if flat || d.roads.iter().any(|r| matches!(r.kind, RoadKind::Elevated | RoadKind::Bridge)) {
        Some(crate::road_plan::surface_plan(d, &area)?)
    } else { None };
    let mut ground = 0u64;
    if flat {
        let (patches, walls) = plan.as_ref().unwrap();
        ground = crate::roads::estimate_ground(d, &area, patches)?;
        add(ground, 7);
        // Structural decks (plus a tunnel ceiling) and portal walls are emitted
        // whole and clipped to the cell afterwards.
        for patch in patches {
            if patch.road.kind == RoadKind::Ground { continue; }
            let f = clip_factor(&bounds(patch.v.iter().map(|p| [p[0], p[2]]), 0), &area);
            add(f * if patch.road.kind == RoadKind::Tunnel { 2 } else { 1 }, patch.road.id.len());
        }
        // A tunnel wall is one quad; an underpass wall is cut by the tile's two
        // terrain triangles and the roof plane: up to four quads.
        for wall in walls {
            let quads = match wall.road.kind { RoadKind::Tunnel => 1, RoadKind::Underpass => 4, _ => continue };
            if crate::road_plan::hit(&[wall.a, wall.b], &area, 0) {
                add(2 * quads * clip_factor(&bounds([wall.a, wall.b].iter().map(|p| [p[0], p[2]]), 0), &area), wall.road.id.len());
            }
        }
    } else if d.assembled_track.as_ref().is_none_or(|a| a.terrain_integration()) { add(nx * ny * 2 * if partial { 5 } else { 1 }, 7); }
    for paint in &d.surface_areas {
        let shape = bounds(paint.polygon.iter().copied(), 0);
        if clip_factor(&shape, &area) != 0 {
            add(if flat { 0 } else { nx * ny * (paint.polygon.len() as u64) * 24 }, paint.id.len());
        }
    }
    let road_margin = crate::roads::influence_margin(d);
    // Sidewalk hulls come from the plan of the cell widened by 1,000 cm.
    let mut sidewalk_segments = 0u64;
    for r in &d.roads {
        let width = crate::placement::sidewalk_width(d, r) as i64;
        for points in r.points.windows(2) {
            // Corridors plus their two aprons. This is a source-derived
            // output allowance, also enforced by road generation.
            let local = bounds(points.iter().map(|p| [p[0], p[2]]), road_margin);
            if clip_factor(&local, &area) != 0 {
                if flat {
                    add(0, r.id.len());
                } else {
                    let span = |axis: usize| {
                        ((local.max[axis].min(area.max[axis]) - local.min[axis].max(area.min[axis]))
                            .max(0)
                            / spacing
                            + 2) as u64
                    };
                    let touched = span(0).saturating_mul(span(1));
                    add(touched.saturating_mul(192).saturating_add(512), r.id.len());
                }
                if width > 0 && !flat {
                    let local_span = ((local.max[0].min(area.max[0])
                        - local.min[0].max(area.min[0]))
                    .max(0)
                        + (local.max[1].min(area.max[1]) - local.min[1].max(area.min[1])).max(0))
                        / 200
                        + 4;
                    add(
                        local_span as u64 * 96 * (width / spacing + 2) as u64,
                        r.id.len() + 9,
                    );
                }
            }
            if flat && width > 0
                && clip_factor(&bounds(points.iter().map(|p| [p[0], p[2]]), road_margin + 1000), &area) != 0 {
                sidewalk_segments += 1;
                add(0, r.id.len() + 9);
            }
        }
    }
    if flat {
        // Sidewalk tops re-cut the ground faces inside the unioned hulls (16 ring
        // points per boundary edge) minus the building footprints and entrances
        // near the cell (their vertices become hole vertices), then add one wall
        // quad per exposed ring edge. Measured city cells stay below 2.1 times
        // this base; the factor 4 is the allowance, and generation fails closed
        // above it.
        let exclusion_vertices: u64 = d.buildings.iter()
            .flat_map(|b| b.footprints().chain(&b.entrances))
            .filter(|poly| clip_factor(&bounds(poly.iter().copied(), 0), &area) != 0)
            .map(|poly| poly.len() as u64)
            .sum();
        add(4 * (16 * sidewalk_segments + ground + exclusion_vertices), 7);
    }
    // The shared exterior plan gives bounded safety counts, including arcs and
    // posts whose centers lie outside this cell. No memory/cell caps change.
    let edges: &[crate::road_plan::Edge] = plan.as_ref().map_or(&[], |p| p.1.as_slice());
    for edge in edges {
        if matches!(edge.road.kind, RoadKind::Elevated | RoadKind::Bridge)
            && crate::road_plan::hit(&[edge.a, edge.b], &area, crate::road_safety::OUTSET_CM) {
            let posts=crate::road_safety::post_range(edge,&area).map_or(0,|(a,b)|
                if a>b {0} else {libm::floor((b-a)/200.0) as u64+1});
            let n = 3 + posts;
            cost.asset_convexes = cost.asset_convexes.saturating_add(n);
            cost.occupied_solids = cost.occupied_solids.saturating_add(n);
            add(n.saturating_mul(60), edge.road.id.len() + 13);
        }
    }
    for building in &d.buildings {
        for (footprint, holes) in building.rings() {
        let shape = bounds(footprint.iter().copied(), 0);
        let n = (footprint.len()
            + holes.iter().map(Vec::len).sum::<usize>()
            + 2 * holes.len()) as u64;

        let parts = if building.roof == "gable" { 4 } else { n - 2 };
        let clipped = parts * clip_factor(&shape, &area);
        cost.building_prisms = cost.building_prisms.saturating_add(clipped);
        if clipped > 0 {
            cost.occupied_solids = cost.occupied_solids.saturating_add(parts);
        }
        add(clipped * 8, building.id.len());
        }
    }
    if !d.barriers.is_empty() {
        let owned;
        let spans = match barriers {
            Some(spans) => spans,
            None => { owned = crate::barrier::Spans::new(d)?; &owned }
        };
        // Owned pieces: twelve faces each (sixty when map-edge clipped).
        // Counting stops past what the cell's triangle/solid limits admit.
        let cap = (max_triangles.min(2_000_000) as u64 / 12 + 1).min(crate::MAX_OCCUPIED_SOLIDS as u64 + 1);
        let (triangles, pieces, id_bytes) = spans.cost(d, cell, cap)?;
        add(triangles, id_bytes as usize);
        cost.asset_convexes = cost.asset_convexes.saturating_add(pieces);
        cost.occupied_solids = cost.occupied_solids.saturating_add(pieces);
        cost.generation_scratch_bytes = cost.generation_scratch_bytes.saturating_add(pieces.saturating_mul(256));
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
        // exceed this even when the domain bounds span many cells. Trees are
        // emitted whole by their owner cell, never clipped into pieces; the one
        // extra lattice index per axis above keeps the bound when an odd-sized
        // rotated proxy box straddles the map edge by a centimetre.
        candidates = candidates.min(300_000);
        cost.objects = cost.objects.saturating_add(candidates);
        if let Some(tree) = &zone.tree {
            let asset = d.assets.iter().find(|a| a.id == tree.asset_id).unwrap();
            cost.occupied_solids = cost.occupied_solids.saturating_add(
                candidates
                    .saturating_mul((asset.collision.len() + asset.convex_collision.len()) as u64),
            );
            cost.asset_convexes = cost
                .asset_convexes
                .saturating_add(candidates.saturating_mul(asset.convex_collision.len() as u64));
            let triangles = asset.collision.len() as u64 * 12
                + asset
                    .convex_collision
                    .iter()
                    .map(|c| c.faces.len() as u64)
                    .sum::<u64>();
            add(
                candidates.saturating_mul(triangles),
                (zone.id.len() + 42).max(tree.asset_id.len()),
            );
        } else {
            cost.occupied_solids = cost.occupied_solids.saturating_add(candidates);
            add(candidates.saturating_mul(12), zone.id.len() + 42);
        }
    }
    let repeated = { crate::placement::repeated(d)? };

    cost.generation_scratch_bytes = cost
        .generation_scratch_bytes
        .saturating_add((d.placements.len() + repeated.len()) as u64 * 512);

    for placement in d.placements.iter().chain(&repeated) {
        let builtin = { crate::placement::builtin(&placement.asset_id) };
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
            let shape = proxy.placed(placement).bounds();
            if placement.yaw_offset_mdeg != 0 && clip_factor(&shape, &area) != 0 {
                cost.asset_convexes += 1;
            }
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

/// Upper bound of the road display styles a cell can create: one per road and
/// surface with a spawnable face in the cell, so every road with a corridor or
/// junction face near the cell (within the plan's influence margin) counts its
/// distinct surfaces, plus one shared safety style. A road sharing an assembled
/// track face ID is styled wherever that face is. A display-byte allowance
/// input, not a hash input.
pub fn road_style_bound(d: &MapDocument, cell: Cell) -> Result<u64> {
    let area = d.cell_bounds(cell)?;
    let margin = crate::roads::influence_margin(d);
    let mut styles = 0u64;
    let mut safety = false;
    for r in &d.roads {
        let near = r.points.windows(2).any(|p| clip_factor(&bounds(p.iter().map(|v| [v[0], v[2]]), margin), &area) != 0)
            || (d.assembled_track.is_some() && r.id.starts_with("assembled-road-"));
        if !near { continue; }
        let mut surfaces: Vec<Surface> = r.surfaces.clone();
        surfaces.sort_by_key(|s| *s as u8);
        surfaces.dedup();
        styles += surfaces.len() as u64;
        safety |= matches!(r.kind, RoadKind::Elevated | RoadKind::Bridge);
    }
    Ok(styles + u64::from(safety))
}
