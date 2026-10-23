use super::*;
use crate::occupancy::Occupancy;

pub(super) struct Builder {
    pub(super) chunk: GeneratedChunk,
    pub(super) bounds: Bounds,
    pub(super) distant: bool,
    max: usize,
    occupancy: Option<Occupancy>,
}
impl Builder {
    /// Track tessellation uses outward mathematical normals. Generated package
    /// faces use the opposite winding, like roads/terrain and box shells. The
    /// shared Godot adapter reflects map-y to -z and reverses indices once.
    /// Convert at publication, keeping occupancy and analytic track frames intact.
    fn assembled(&mut self, track: &crate::assembled_track::Assembly) -> Result<()> {
        let first = self.chunk.triangles.len();
        crate::assembled_track::generate(track, self)?;
        for face in &mut self.chunk.triangles[first..] {
            crate::cancellation::checkpoint()?;
            face.vertices.swap(1, 2);
        }
        Ok(())
    }

    pub(super) fn solid(&mut self, id: &str, shape: SolidShape) -> Result<()> {
        if let Some(occupancy) = &mut self.occupancy {
            occupancy.push(id, shape, &self.bounds)?;
        }
        Ok(())
    }
    /// Clip collision and display geometry together. Integer interpolation fixes byte identity.
    pub(super) fn triangle(
        &mut self,
        v: [Vertex; 3],
        surface: Surface,
        id: &str,
        spawnable: bool,
    ) -> Result<()> {
        crate::cancellation::checkpoint()?;
        // No clipping pass changes a triangle that lies within the bounds.
        if v.iter().all(|p| (0..2).all(|a| p[a * 2] >= self.bounds.min[a] && p[a * 2] <= self.bounds.max[a])) {
            return self.face(v, surface, id, spawnable);
        }
        let mut poly = v.to_vec();
        for (axis, limit, lower) in [
            (0, self.bounds.min[0], true),
            (0, self.bounds.max[0], false),
            (2, self.bounds.min[1], true),
            (2, self.bounds.max[1], false),
        ] {
            let previous = std::mem::take(&mut poly);
            if previous.is_empty() {
                return Ok(());
            }
            for i in 0..previous.len() {
                let (a, b) = (previous[i], previous[(i + 1) % previous.len()]);
                let inside = |p: Vertex| {
                    if lower {
                        p[axis] >= limit
                    } else {
                        p[axis] <= limit
                    }
                };
                if inside(a) {
                    poly.push(a);
                }
                if inside(a) != inside(b) {
                    // Always interpolate from lexicographically smaller endpoint: rounding is orientation independent.
                    let (a, b) = if a < b { (a, b) } else { (b, a) };
                    let mut p = a;
                    for j in 0..3 {
                        p[j] = a[j]
                            + (((b[j] - a[j]) as i128 * (limit - a[axis]) as i128)
                                / (b[axis] - a[axis]) as i128) as i64;
                    }
                    p[axis] = limit;
                    poly.push(p);
                }
            }
        }
        for i in 1..poly.len().saturating_sub(1) {
            self.face([poly[0], poly[i], poly[i + 1]], surface, id, spawnable)?;
        }
        Ok(())
    }
    /// One clipped fan face: zero-area faces are dropped, then the budget applies.
    fn face(&mut self, vertices: [Vertex; 3], surface: Surface, id: &str, spawnable: bool) -> Result<()> {
        let u = [
            vertices[1][0] - vertices[0][0],
            vertices[1][1] - vertices[0][1],
            vertices[1][2] - vertices[0][2],
        ];
        let w = [
            vertices[2][0] - vertices[0][0],
            vertices[2][1] - vertices[0][1],
            vertices[2][2] - vertices[0][2],
        ];
        if [
            u[1] as i128 * w[2] as i128 - u[2] as i128 * w[1] as i128,
            u[2] as i128 * w[0] as i128 - u[0] as i128 * w[2] as i128,
            u[0] as i128 * w[1] as i128 - u[1] as i128 * w[0] as i128,
        ] == [0; 3]
        {
            return Ok(());
        }
        if self.chunk.triangles.len() >= self.max {
            return Err(error("E_BUDGET", "chunk triangle budget exceeded"));
        }
        self.chunk.triangles.push(Triangle {
            contact_class: 0, snow_retention_percent: 100,
            vertices,
            surface,
            object_id: id.into(),
            spawnable,
        });
        Ok(())
    }
    pub(super) fn quad(
        &mut self,
        v: [Vertex; 4],
        surface: Surface,
        id: &str,
        spawnable: bool,
    ) -> Result<()> {
        self.triangle([v[0], v[1], v[2]], surface, id, spawnable)?;
        self.triangle([v[0], v[2], v[3]], surface, id, spawnable)
    }
    pub(super) fn convex_shape(&mut self, shape: CollisionConvex, id: &str) -> Result<()> {
        let area = shape.bounds();
        if (0..2).any(|a| area.max[a] < self.bounds.min[a] || area.min[a] > self.bounds.max[a]) {
            return Ok(());
        }
        self.solid(id, SolidShape::Convex(shape.clone()))?;
        for face in &shape.faces {
            self.triangle(
                face.map(|i| shape.vertices[i as usize]),
                Surface::Concrete,
                id,
                false,
            )?;
        }
        if !self.distant {
            self.chunk.asset_convexes.push(GeneratedConvex { object_id: id.into(), shape });
        }
        Ok(())
    }
    pub(super) fn box_shape(&mut self, center: Vertex, size: [u32; 3], id: &str) -> Result<()> {
        let min = [
            center[0] - size[0] as i64 / 2,
            center[1] - size[1] as i64 / 2,
            center[2] - size[2] as i64 / 2,
        ];
        let max = [
            min[0] + size[0] as i64,
            min[1] + size[1] as i64,
            min[2] + size[2] as i64,
        ];
        self.solid(id, SolidShape::Box { min, max })?;
        let v = [
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], min[1], max[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ];
        for f in [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ] {
            self.quad(f.map(|i| v[i]), Surface::Concrete, id, false)?;
        }
        Ok(())
    }
}
pub(super) fn polygon_triangles(poly: &[Point]) -> Result<Vec<[usize; 3]>> {
    let area: i128 = (0..poly.len())
        .map(|i| {
            poly[i][0] as i128 * poly[(i + 1) % poly.len()][1] as i128
                - poly[i][1] as i128 * poly[(i + 1) % poly.len()][0] as i128
        })
        .sum();
    let mut ring: Vec<usize> = if area > 0 {
        (0..poly.len()).collect()
    } else {
        (0..poly.len()).rev().collect()
    };
    // Larger rings test only the remaining indices near each candidate ear.
    let mut grid = if ring.len() >= IndexGrid::MIN_POINTS { IndexGrid::new(poly, &ring) } else { None };
    let mut out = vec![];
    while ring.len() > 3 {
        let mut ear = None;
        for i in 0..ring.len() {
            let (a, b, c) = (
                ring[(i + ring.len() - 1) % ring.len()],
                ring[i],
                ring[(i + 1) % ring.len()],
            );
            if cross(poly[a], poly[b], poly[c]) <= 0 {
                continue;
            }
            let triangle = [poly[a], poly[b], poly[c]];
            let blocks = |j: &usize| ![a, b, c].contains(j) && point_in_polygon(poly[*j], &triangle);
            if match &grid { Some(grid) => grid.any(&triangle, blocks), None => ring.iter().any(blocks) } {
                continue;
            }
            ear = Some((i, [a, b, c]));
            break;
        }
        let Some((i, t)) = ear else {
            return Err(error("E_GEOMETRY", "polygon cannot be triangulated"));
        };
        out.push(t);
        if let Some(grid) = &mut grid {
            grid.remove(poly, ring[i]);
        }
        ring.remove(i);
    }
    out.push([ring[0], ring[1], ring[2]]);
    Ok(out)
}

/// Remaining ring indices of a polygon in square buckets by point. Exclusion
/// stays by index: a duplicate point at another index still blocks an ear.
/// A point outside a triangle's bounds is never inside it, so testing only the
/// buckets under those bounds finds the same first unblocked ear.
struct IndexGrid {
    min: Point,
    size: i64,
    columns: i64,
    cells: Vec<Vec<usize>>,
}
impl IndexGrid {
    const MIN_POINTS: usize = 24;
    /// None when the ring's extent does not fit in `i64` (the full scan then
    /// keeps the old wrapping arithmetic and its errors).
    fn new(poly: &[Point], ring: &[usize]) -> Option<Self> {
        let min: Point = std::array::from_fn(|a| ring.iter().map(|&i| poly[i][a]).min().unwrap());
        let max: Point = std::array::from_fn(|a| ring.iter().map(|&i| poly[i][a]).max().unwrap());
        let span = [max[0].checked_sub(min[0])?, max[1].checked_sub(min[1])?];
        let side = (ring.len() as f64).sqrt().ceil() as i64;
        let size = (span[0].max(span[1]).max(1) / side).max(1);
        let columns = span[0] / size + 1;
        let rows = span[1] / size + 1;
        let mut grid = Self { min, size, columns, cells: vec![vec![]; (columns * rows) as usize] };
        for &i in ring {
            let cell = grid.cell(poly[i]);
            grid.cells[cell].push(i);
        }
        Some(grid)
    }
    fn key(&self, p: Point) -> [i64; 2] {
        std::array::from_fn(|a| (p[a] - self.min[a]).div_euclid(self.size))
    }
    fn cell(&self, p: Point) -> usize {
        let [x, y] = self.key(p);
        (y * self.columns + x) as usize
    }
    fn remove(&mut self, poly: &[Point], index: usize) {
        let cell = self.cell(poly[index]);
        let bucket = &mut self.cells[cell];
        if let Some(at) = bucket.iter().position(|&j| j == index) {
            bucket.swap_remove(at);
        }
    }
    /// Whether `test` holds for a remaining index whose point lies within the
    /// bounds of `triangle` (whose vertices are ring points).
    fn any(&self, triangle: &[Point; 3], test: impl Fn(&usize) -> bool) -> bool {
        let lo = self.key(std::array::from_fn(|a| triangle.iter().map(|p| p[a]).min().unwrap()));
        let hi = self.key(std::array::from_fn(|a| triangle.iter().map(|p| p[a]).max().unwrap()));
        (lo[1]..=hi[1]).any(|y| (lo[0]..=hi[0])
            .any(|x| self.cells[(y * self.columns + x) as usize].iter().any(&test)))
    }
}
pub fn generate(input: GenerationInput<'_>) -> Result<GeneratedChunk> {
    generate_internal(input, None).map(|result| result.chunk)
}

/// Optional occupied-volume sidecar; does not change generated geometry or hashes.
/// See `GeneratedOccupancy` for cell ownership and completeness requirements.
pub fn generate_with_occupancy(
    input: GenerationInput<'_>,
    max_solids: usize,
) -> Result<GeneratedOccupancy> {
    if max_solids > MAX_OCCUPIED_SOLIDS {
        return Err(error("E_BUDGET", "occupancy limit exceeds 200000 solids"));
    }
    generate_internal(
        input,
        Some(Occupancy {
            solids: Vec::new(),
            max: max_solids,
        }),
    )
}

fn generate_internal(
    input: GenerationInput<'_>,
    occupancy: Option<Occupancy>,
) -> Result<GeneratedOccupancy> {
    let mut document = input.document.clone();
    document.normalize();
    document.validate()?;
    // Barrier documents prepare once for estimate and generation; others keep
    // the original placement preparation order and error wrapping.
    let prepared = if document.barriers.is_empty() { None } else {
        Some(crate::placement::PreparedPlacements::new(&document).map_err(|mut e| {
            e.message = format!("placements: {}", e.message); e
        })?)
    };
    let cost = crate::cost::estimate_validated(&document, input.cell, input.max_triangles, prepared.as_ref())?;
    generate_validated(
        GenerationInput {
            document: &document,
            ..input
        },
        occupancy,
        &cost,
        prepared.as_ref(),
        false,
    )
}

pub(crate) fn generate_prepared(
    input: GenerationInput<'_>,
    solids: Option<usize>,
    cost: &GenerationCost,
    placements: Option<&crate::placement::PreparedPlacements>,
) -> Result<GeneratedOccupancy> {
    generate_validated(
        input,
        solids.map(|max| Occupancy {
            solids: Vec::new(),
            max,
        }),
        cost,
        placements,
        false,
    )
}

pub(crate) fn generate_distant_prepared(
    input: GenerationInput<'_>,
    cost: &GenerationCost,
    placements: &crate::placement::PreparedPlacements,
) -> Result<DistantSource> {
    let generated = generate_validated(input, None, cost, Some(placements), true)?.chunk;
    Ok(DistantSource {
        triangles: generated.triangles,
        objects: generated.objects,
        water_bodies: generated.water_bodies,
    })
}

fn generate_validated(
    input: GenerationInput<'_>,
    occupancy: Option<Occupancy>,
    cost: &GenerationCost,
    placements: Option<&crate::placement::PreparedPlacements>,
    distant: bool,
) -> Result<GeneratedOccupancy> {
    crate::cancellation::checkpoint()?;
    let d = input.document;
    let bounds = d.cell_bounds(input.cell)?;
    let mut b = Builder {
        chunk: GeneratedChunk {
            water_bodies: crate::water::generate(d, &bounds)?,
            grind_lines: if distant { vec![] } else { d.grind_lines.iter().filter(|g| g.intersects(&bounds)).cloned().collect() },
            gimmicks: d.gimmicks.iter().filter(|g| g.intersects(&bounds)).cloned().collect(),
            asset_convexes: vec![],
            building_prisms: vec![],
            format_version: GENERATED_VERSION,
            cell: input.cell,
            triangles: vec![],
            objects: vec![],
        },
        bounds: bounds.clone(),
        max: { cost.triangles as usize },
        occupancy,
        distant,
    };
    let descriptor = d.heightmaps.iter().find(|h| h.cell == input.cell);
    if descriptor.is_some() != input.heightgrid.is_some() {
        return Err(error(
            "E_HEIGHTMAP",
            "heightmap input missing or unexpected",
        ));
    }
    let spacing = descriptor.map_or(d.cell_size_cm, |h| h.spacing_cm) as i64;
    let side = (d.cell_size_cm as i64 / spacing + 1) as usize;
    if let Some(g) = input.heightgrid {
        if g.side != side
            || g.heights_cm.len() != side * side
            || g.heights_cm.iter().any(|v| v.unsigned_abs() > 8_000_000)
        {
            return Err(error(
                "E_HEIGHTMAP",
                "height grid dimensions or values invalid",
            ));
        }
    }
    if let Some(track) = &d.assembled_track {
        b.assembled(track)?;
        if !distant {
            for face in &mut b.chunk.triangles {
                if face.object_id.starts_with("assembled-road-") { face.contact_class = 3; }
            }
        }
    }
    if d.assembled_track.as_ref().is_none_or(|a| a.terrain_integration()) {
        crate::roads::generate(d, &bounds, input.heightgrid, spacing, side, &mut b, placements.map(|p| p.paint(d))).map_err(|mut e| {
            e.message = format!("ground surfaces: {}", e.message); e
        })?;
    }

    if let Some(track)=d.assembled_track.as_ref().filter(|a|a.terrain_integration()) {
        let first=b.chunk.triangles.len();
        crate::assembled_track::terrain_supports(track,&mut b,|p| {
            crate::terrain_height(&bounds,spacing,side,input.heightgrid,d.terrain_base_cm,p)
        })?;
        for face in &mut b.chunk.triangles[first..] {face.vertices.swap(1,2);}
    }

    // Classify generated faces while only terrain/road geometry exists. Authored
    // placements are appended afterwards and always retain obstacle classification.
    if !distant {
        // Generation documents are normalized and validated: roads and surface
        // areas are sorted by unique ID, so a search finds what an ID map would.
        debug_assert!(d.roads.is_sorted_by(|a, b| a.id < b.id) && d.surface_areas.is_sorted_by(|a, b| a.id < b.id));
        let road = |id: &str| d.roads.binary_search_by(|r| r.id.as_str().cmp(id)).ok().map(|i| &d.roads[i]);
        let area = |id: &str| d.surface_areas.binary_search_by(|a| a.id.as_str().cmp(id)).is_ok();
        for face in &mut b.chunk.triangles {
            if !face.spawnable { continue; }
            if let Some(road) = road(face.object_id.as_str()) {
                face.contact_class = if road.kind == RoadKind::Ground && road.design.is_none() { 2 } else { 3 };
                face.snow_retention_percent = road.snow_retention_percent;
            } else if face.object_id == "terrain" || area(face.object_id.as_str()) {
                face.contact_class = 1;
            }
        }
    }
    crate::placement::generate(d, input.cell, &mut b, placements).map_err(|mut e| {
        e.message = format!("placements: {}", e.message); e
    })?;
    if !d.barriers.is_empty() {
        let owned;
        let spans = match placements {
            Some(prepared) => &prepared.barriers,
            None => { owned = crate::barrier::Spans::new(d)?; &owned }
        };
        crate::barrier::generate(d, input.cell, input.heightgrid, spans, &mut b).map_err(|mut e| {
            e.message = format!("barriers: {}", e.message); e
        })?;
    }
    if let Some(occupied) = &mut b.occupancy {
        for g in &b.chunk.gimmicks {
            for (min,max) in g.occupancy_bounds() {
                occupied.push(&g.id, SolidShape::Box {min,max}, &bounds)?;
            }
        }
    }
    return Ok(GeneratedOccupancy {
        chunk: b.chunk,
        solids: b.occupancy.map_or_else(Vec::new, |v| v.solids),
    });
}

/// Reuse exactly the integer cell clipping used for terrain, without collision.
pub(crate) fn water_surface(bounds: &Bounds, height: i64, faces: &[[Point; 3]]) -> Result<Vec<[Vertex; 3]>> {
    let mut builder = Builder { chunk: GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], asset_convexes: vec![], building_prisms: vec![], format_version: GENERATED_VERSION, cell: Cell{x:0,y:0}, triangles: vec![], objects: vec![] }, bounds: bounds.clone(), max: 4096, occupancy: None, distant: false };
    for face in faces { builder.triangle(face.map(|p| [p[0],height,p[1]]), Surface::Concrete, "water", false)?; }
    Ok(builder.chunk.triangles.into_iter().map(|t|t.vertices).collect())
}

/// Bounded draft display uses the exact executable track tessellator.
pub fn assembled_preview(d:&MapDocument)->Result<GeneratedChunk> {
    let mut b=Builder{chunk:GeneratedChunk{water_bodies:vec![],grind_lines:d.grind_lines.clone(),gimmicks:d.gimmicks.clone(),asset_convexes:vec![],building_prisms:vec![],format_version:1,cell:Cell{x:0,y:0},triangles:vec![],objects:vec![]},bounds:d.bounds.clone(),max:500_000,occupancy:None,distant:false};
    if let Some(a)=&d.assembled_track {
        b.assembled(a)?;
        if a.authoring.as_ref().is_some_and(|s| !s.grounded_supports) {
            b.chunk.triangles.retain(|t|t.object_id!="assembled-venue-floor");
        }
    }
    Ok(b.chunk)
}

#[cfg(test)]
mod speedup_tests {
    use super::*;

    /// Deterministic generator for the differential tests.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: i64) -> i64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) % n as u64) as i64
        }
    }

    /// The ear clipper before the index grid: every remaining index is tested.
    fn polygon_triangles_reference(poly: &[Point]) -> Result<Vec<[usize; 3]>> {
        let area: i128 = (0..poly.len()).map(|i| poly[i][0] as i128 * poly[(i + 1) % poly.len()][1] as i128
            - poly[i][1] as i128 * poly[(i + 1) % poly.len()][0] as i128).sum();
        let mut ring: Vec<usize> = if area > 0 { (0..poly.len()).collect() } else { (0..poly.len()).rev().collect() };
        let mut out = vec![];
        while ring.len() > 3 {
            let mut ear = None;
            for i in 0..ring.len() {
                let (a, b, c) = (ring[(i + ring.len() - 1) % ring.len()], ring[i], ring[(i + 1) % ring.len()]);
                if cross(poly[a], poly[b], poly[c]) <= 0 { continue; }
                if ring.iter().any(|j| ![a, b, c].contains(j) && point_in_polygon(poly[*j], &[poly[a], poly[b], poly[c]])) { continue; }
                ear = Some((i, [a, b, c]));
                break;
            }
            let Some((i, t)) = ear else { return Err(error("E_GEOMETRY", "polygon cannot be triangulated")); };
            out.push(t);
            ring.remove(i);
        }
        out.push([ring[0], ring[1], ring[2]]);
        Ok(out)
    }

    fn rings(rng: &mut Lcg) -> Vec<Vec<Point>> {
        let mut out = vec![];
        for case in 0..600 {
            let n = 3 + rng.next(140) as usize;
            let ring: Vec<Point> = match case % 5 {
                // Star rings, on a coarse grid (collinear runs) or fine.
                0 | 1 => {
                    let step = if case % 5 == 0 { 50 } else { 1 };
                    let mut angles: Vec<i64> = (0..n).map(|_| rng.next(36_000)).collect();
                    angles.sort();
                    angles.into_iter().map(|a| {
                        let (r, t) = ((200 + rng.next(800)) as f64, a as f64 / 36_000.0 * std::f64::consts::TAU);
                        [((r * t.cos()) as i64 / step) * step, ((r * t.sin()) as i64 / step) * step]
                    }).collect()
                }
                // Staircase with collinear points on long runs.
                2 => {
                    let k = n.max(4) / 2;
                    let mut ring: Vec<Point> = (0..k as i64).flat_map(|i| [[i * 30, i * 20], [i * 30 + 30, i * 20]]).collect();
                    let last = *ring.last().unwrap();
                    ring.extend([[last[0], last[1] + 400], [last[0] / 2, last[1] + 400], [0, last[1] + 400]]);
                    ring
                }
                // Pinched: squares touching at duplicated vertices.
                3 => {
                    let m = 1 + n as i64 / 8;
                    let mut ring = vec![];
                    for i in 0..m { ring.extend([[i * 20, 0], [i * 20 + 10, 0], [i * 20 + 10, 10], [i * 20 + 20, 10]]); }
                    let end = m * 20;
                    ring.extend([[end, 20]]);
                    for i in (0..m).rev() { ring.extend([[i * 20 + 10, 20], [i * 20 + 10, 10], [i * 20, 10]]); }
                    ring.dedup();
                    ring
                }
                // Random (usually self-intersecting) rings.
                _ => (0..n).map(|_| [rng.next(300), rng.next(300)]).collect(),
            };
            if ring.len() >= 3 { out.push(ring); }
        }
        out
    }

    #[test]
    fn indexed_ear_clipping_matches_full_scan() {
        let mut rng = Lcg(7);
        let (mut total, mut grid, mut failed) = (0, 0, 0);
        for mut ring in rings(&mut rng) {
            for _ in 0..2 {
                let (old, new) = (polygon_triangles_reference(&ring), polygon_triangles(&ring));
                assert_eq!(old, new, "ring {ring:?}");
                total += 1;
                grid += usize::from(ring.len() >= IndexGrid::MIN_POINTS);
                failed += usize::from(old.is_err());
                ring.reverse();
            }
        }
        // A ring whose extent overflows i64 keeps the full scan (and whatever its
        // wrapping arithmetic yields in release builds).
        let mut wide: Vec<Point> = (0..23i128).map(|i| [(-5_000_000_000_000_000_000 + i * 450_000_000_000_000_000) as i64, 0]).collect();
        wide.extend([[wide[22][0], 1000], [-5_000_000_000_000_000_000, 1000]]);
        assert!(IndexGrid::new(&wide, &(0..wide.len()).collect::<Vec<_>>()).is_none());
        // Both successful and failing rings, most of them through the grid.
        assert!(grid * 2 > total && failed > 10 && failed + 100 < total, "total {total} grid {grid} failed {failed}");
    }

    /// A point outside a polygon's closed bounds is never inside it (ground paint
    /// classification skips those tests).
    #[test]
    fn points_outside_bounds_are_outside_polygons() {
        let mut rng = Lcg(11);
        for ring in rings(&mut rng) {
            let b = crate::bounds_index::bounds(&ring);
            for p in [[b.min[0] - 1, b.min[1]], [b.max[0] + 1, b.max[1]], [b.min[0], b.min[1] - 1], [b.max[0], b.max[1] + 1],
                [b.min[0] - 7, (b.min[1] + b.max[1]) / 2], [b.max[0] + 7, (b.min[1] + b.max[1]) / 2]] {
                assert!(!point_in_polygon(p, &ring), "{p:?} {ring:?}");
            }
        }
    }

    fn builder(bounds: Bounds, max: usize) -> Builder {
        Builder {
            chunk: GeneratedChunk { water_bodies: vec![], gimmicks: vec![], grind_lines: vec![], asset_convexes: vec![],
                building_prisms: vec![], format_version: GENERATED_VERSION, cell: Cell { x: 0, y: 0 }, triangles: vec![], objects: vec![] },
            bounds, distant: false, max, occupancy: None,
        }
    }

    /// The clip before the inside fast path.
    fn clip_reference(bounds: &Bounds, v: [Vertex; 3]) -> Vec<[Vertex; 3]> {
        let mut poly = v.to_vec();
        for (axis, limit, lower) in [(0, bounds.min[0], true), (0, bounds.max[0], false), (2, bounds.min[1], true), (2, bounds.max[1], false)] {
            let previous = std::mem::take(&mut poly);
            if previous.is_empty() { return vec![]; }
            for i in 0..previous.len() {
                let (a, b) = (previous[i], previous[(i + 1) % previous.len()]);
                let inside = |p: Vertex| if lower { p[axis] >= limit } else { p[axis] <= limit };
                if inside(a) { poly.push(a); }
                if inside(a) != inside(b) {
                    let (a, b) = if a < b { (a, b) } else { (b, a) };
                    let mut p = a;
                    for j in 0..3 { p[j] = a[j] + (((b[j] - a[j]) as i128 * (limit - a[axis]) as i128) / (b[axis] - a[axis]) as i128) as i64; }
                    p[axis] = limit;
                    poly.push(p);
                }
            }
        }
        (1..poly.len().saturating_sub(1)).map(|i| [poly[0], poly[i], poly[i + 1]]).filter(|t| {
            let u: [i64; 3] = std::array::from_fn(|k| t[1][k] - t[0][k]);
            let w: [i64; 3] = std::array::from_fn(|k| t[2][k] - t[0][k]);
            [u[1] as i128 * w[2] as i128 - u[2] as i128 * w[1] as i128, u[2] as i128 * w[0] as i128 - u[0] as i128 * w[2] as i128,
                u[0] as i128 * w[1] as i128 - u[1] as i128 * w[0] as i128] != [0; 3]
        }).collect()
    }

    #[test]
    fn inside_fast_path_matches_clipping() {
        let mut rng = Lcg(3);
        let bounds = Bounds { min: [0, 0], max: [1000, 1000] };
        let mut inside = 0;
        for case in 0..20_000 {
            // Inside, on the boundary, crossing edges and corners, or degenerate.
            let span = if case % 3 == 0 { 1000 } else { 1400 };
            let mut v: [Vertex; 3] = std::array::from_fn(|_| [rng.next(span + 1) - (span - 1000) / 2, rng.next(200), rng.next(span + 1) - (span - 1000) / 2]);
            if case % 7 == 0 { v[2] = v[1]; }
            if case % 11 == 0 { v[0][0] = 0; v[1][0] = 1000; }
            let expected = clip_reference(&bounds, v);
            let mut b = builder(bounds.clone(), 1_000_000);
            b.triangle(v, Surface::Grass, "x", true).unwrap();
            assert_eq!(b.chunk.triangles.iter().map(|t| t.vertices).collect::<Vec<_>>(), expected, "{v:?}");
            inside += usize::from(v.iter().all(|p| bounds.contains([p[0], p[2]])));
            // The triangle budget applies after the same zero-area filter.
            let mut full = builder(bounds.clone(), 0);
            assert_eq!(full.triangle(v, Surface::Grass, "x", true).is_err(), !expected.is_empty());
        }
        assert!(inside > 5_000, "{inside}");
    }
}
