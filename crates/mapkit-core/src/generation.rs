use super::*;
use crate::occupancy::Occupancy;

pub(super) struct Builder {
    pub(super) chunk: GeneratedChunk,
    pub(super) bounds: Bounds,
    max: usize,
    occupancy: Option<Occupancy>,
}
impl Builder {
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
            let vertices = [poly[0], poly[i], poly[i + 1]];
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
                continue;
            }
            if self.chunk.triangles.len() >= self.max {
                return Err(error("E_BUDGET", "chunk triangle budget exceeded"));
            }
            self.chunk.triangles.push(Triangle {
                vertices,
                surface,
                object_id: id.into(),
                spawnable,
            });
        }
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
        self.chunk.asset_convexes.push(GeneratedConvex {
            object_id: id.into(),
            shape,
        });
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
            if ring.iter().any(|j| {
                ![a, b, c].contains(j) && point_in_polygon(poly[*j], &[poly[a], poly[b], poly[c]])
            }) {
                continue;
            }
            ear = Some((i, [a, b, c]));
            break;
        }
        let Some((i, t)) = ear else {
            return Err(error("E_GEOMETRY", "polygon cannot be triangulated"));
        };
        out.push(t);
        ring.remove(i);
    }
    out.push([ring[0], ring[1], ring[2]]);
    Ok(out)
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
    let cost = crate::cost::estimate_validated(&document, input.cell, input.max_triangles)?;
    generate_validated(
        GenerationInput {
            document: &document,
            ..input
        },
        occupancy,
        &cost,
        None,
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
    )
}

fn generate_validated(
    input: GenerationInput<'_>,
    occupancy: Option<Occupancy>,
    cost: &GenerationCost,
    placements: Option<&crate::placement::PreparedPlacements>,
) -> Result<GeneratedOccupancy> {
    let d = input.document;
    let bounds = d.cell_bounds(input.cell)?;
    let mut b = Builder {
        chunk: GeneratedChunk {
            water_bodies: crate::water::generate(d, &bounds)?,
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
    crate::roads::generate(d, &bounds, input.heightgrid, spacing, side, &mut b)?;

    crate::placement::generate(d, input.cell, &mut b, placements)?;
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
    let mut builder = Builder { chunk: GeneratedChunk { water_bodies: vec![], gimmicks: vec![], asset_convexes: vec![], building_prisms: vec![], format_version: GENERATED_VERSION, cell: Cell{x:0,y:0}, triangles: vec![], objects: vec![] }, bounds: bounds.clone(), max: 4096, occupancy: None };
    for face in faces { builder.triangle(face.map(|p| [p[0],height,p[1]]), Surface::Concrete, "water", false)?; }
    Ok(builder.chunk.triangles.into_iter().map(|t|t.vertices).collect())
}
