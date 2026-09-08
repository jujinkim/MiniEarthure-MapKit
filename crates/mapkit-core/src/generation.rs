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
    pub(super) fn quad(&mut self, v: [Vertex; 4], surface: Surface, id: &str, spawnable: bool) -> Result<()> {
        self.triangle([v[0], v[1], v[2]], surface, id, spawnable)?;
        self.triangle([v[0], v[2], v[3]], surface, id, spawnable)
    }
    pub(super) fn convex_shape(&mut self, shape: CollisionConvex, id: &str) -> Result<()> {
        let area=shape.bounds();
        if (0..2).any(|a| area.max[a]<self.bounds.min[a] || area.min[a]>self.bounds.max[a]) {return Ok(());}
        self.solid(id, SolidShape::Convex(shape.clone()))?;
        for face in &shape.faces {self.triangle(face.map(|i|shape.vertices[i as usize]),Surface::Concrete,id,false)?;}
        self.chunk.asset_convexes.push(GeneratedConvex {object_id:id.into(),shape});
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
pub(super) fn road_contains(p: Point, r: &Road, extra: i64) -> bool {
    r.points.windows(2).enumerate().any(|(i, s)| {
        let a = [s[0][0], s[0][2]];
        let b = [s[1][0], s[1][2]];
        let dx = (b[0] - a[0]) as i128;
        let dy = (b[1] - a[1]) as i128;
        let len = dx * dx + dy * dy;
        let dot = ((p[0] - a[0]) as i128 * dx + (p[1] - a[1]) as i128 * dy).clamp(0, len);
        let q = [
            a[0] + (dx * dot / len) as i64,
            a[1] + (dy * dot / len) as i64,
        ];
        let d = (p[0] - q[0]) as i128 * (p[0] - q[0]) as i128
            + (p[1] - q[1]) as i128 * (p[1] - q[1]) as i128;
        let radius = r.widths_cm[i] as i64 / 2 + extra;
        d <= radius as i128 * radius as i128
    })
}
pub fn generate(input: GenerationInput<'_>) -> Result<GeneratedChunk> {
    generate_internal(input, None).map(|result| result.chunk)
}

/// Optional occupied-volume sidecar; does not change generated v6 bytes or hashes.
/// See `GeneratedOccupancy` for cell ownership and completeness requirements.
pub fn generate_with_occupancy(
    input: GenerationInput<'_>,
    max_solids: usize,
) -> Result<GeneratedOccupancy> {
    if max_solids > MAX_OCCUPIED_SOLIDS {
        return Err(error("E_BUDGET", "occupancy limit exceeds 200000 solids"));
    }
    generate_internal(input, Some(Occupancy { solids: Vec::new(), max: max_solids }))
}

fn generate_internal(
    input: GenerationInput<'_>,
    occupancy: Option<Occupancy>,
) -> Result<GeneratedOccupancy> {
    let mut document = input.document.clone();
    document.normalize();
    document.validate()?;
    let d = &document;
    let bounds = d.cell_bounds(input.cell)?;
    let mut b = Builder {
        chunk: GeneratedChunk { asset_convexes: vec![],
            building_prisms: vec![],
            format_version: GENERATED_VERSION,
            cell: input.cell,
            triangles: vec![],
            objects: vec![],
        },
        bounds: bounds.clone(),
        max: if d.recipe_version >= 2 {
            estimate_generation(d, input.cell, input.max_triangles)?.triangles as usize
        } else { input.max_triangles.min(2_000_000) },
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
    let height = |x: usize, y: usize| {
        input
            .heightgrid
            .map_or(d.terrain_base_cm, |g| g.heights_cm[y * side + x])
    };
    if d.recipe_version >= 2 {
        crate::roads::generate(d, &bounds, input.heightgrid, spacing, side, &mut b)?;
    } else {
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let px = bounds.min[0] + x as i64 * spacing;
            let py = bounds.min[1] + y as i64 * spacing;
            b.quad(
                [
                    [px, height(x, y), py],
                    [px + spacing, height(x + 1, y), py],
                    [px + spacing, height(x + 1, y + 1), py + spacing],
                    [px, height(x, y + 1), py + spacing],
                ],
                Surface::Grass,
                "terrain",
                true,
            )?;
        }
    }
    for r in &d.roads {
        for (i, s) in r.points.windows(2).enumerate() {
            let dx = (s[1][0] - s[0][0]) as f64;
            let dy = (s[1][2] - s[0][2]) as f64;
            let len = libm::sqrt(dx * dx + dy * dy);
            let half = r.widths_cm[i] as f64 / 2.0;
            let nx = libm::round(-dy / len * half) as i64;
            let ny = libm::round(dx / len * half) as i64;
            let v = [
                [s[0][0] + nx, s[0][1], s[0][2] + ny],
                [s[1][0] + nx, s[1][1], s[1][2] + ny],
                [s[1][0] - nx, s[1][1], s[1][2] - ny],
                [s[0][0] - nx, s[0][1], s[0][2] - ny],
            ];
            b.quad(v, r.surfaces[i], &r.id, true)?;
            if matches!(r.kind, RoadKind::Tunnel | RoadKind::Underpass) {
                let h = r.clearance_cm.unwrap() as i64;
                let top = v.map(|p| [p[0], p[1] + h, p[2]]);
                for (a, c) in [(0, 1), (2, 3)] {
                    b.quad(
                        [v[a], v[c], top[c], top[a]],
                        Surface::Concrete,
                        &r.id,
                        false,
                    )?;
                }
                if r.kind == RoadKind::Tunnel {
                    b.quad(top, Surface::Concrete, &r.id, false)?;
                }
            }
        }
    }
    } // Frozen recipe-v1 terrain/road strategy.
    if d.recipe_version >= 3 {
        crate::placement::generate(d, input.cell, &mut b)?;
        return Ok(GeneratedOccupancy { chunk: b.chunk, solids: b.occupancy.map_or_else(Vec::new, |v| v.solids) });
    }
    for building in &d.buildings {
        let top = building.base_cm + building.height_cm as i64;
        for t in polygon_triangles(&building.footprint)? {
            if b.occupancy.is_some() {
                b.solid(&building.id, SolidShape::TriangularPrism {
                    footprint: t.map(|i| building.footprint[i]),
                    bottom_cm: building.base_cm,
                    top_cm: top,
                })?;
            }
            b.triangle(
                t.map(|i| [building.footprint[i][0], top, building.footprint[i][1]]),
                Surface::Concrete,
                &building.id,
                false,
            )?;
        }
        for i in 0..building.footprint.len() {
            let (a, c) = (
                building.footprint[i],
                building.footprint[(i + 1) % building.footprint.len()],
            );
            b.quad(
                [
                    [a[0], building.base_cm, a[1]],
                    [c[0], building.base_cm, c[1]],
                    [c[0], top, c[1]],
                    [a[0], top, a[1]],
                ],
                Surface::Concrete,
                &building.id,
                false,
            )?;
        }
    }
    // Global lattice and per-object seed make placement independent of spawn/chunk order.
    for zone in &d.zones {
        let s = zone.spacing_cm as i64;
        let mut candidates = 0usize;
        for y in bounds.min[1].div_euclid(s) - 1..=bounds.max[1].div_euclid(s) + 1 {
            for x in bounds.min[0].div_euclid(s) - 1..=bounds.max[0].div_euclid(s) + 1 {
                candidates += 1;
                if candidates > 300_000 {
                    return Err(error("E_BUDGET", "zone candidate budget exceeded"));
                }
                let seed = sha256(&canonical(&(d.seed, &zone.id, "vegetation-v1", x, y))?);
                let random = u64::from_str_radix(&seed[..16], 16).unwrap();
                if random % 1000 >= zone.density_per_mille as u64 {
                    continue;
                }
                let jitter = if zone.kind == ZoneKind::Forest {
                    s / 3
                } else {
                    0
                };
                let jx = if jitter > 0 {
                    ((random >> 10) % (jitter as u64 * 2 + 1)) as i64 - jitter
                } else {
                    0
                };
                let jy = if jitter > 0 {
                    ((random >> 32) % (jitter as u64 * 2 + 1)) as i64 - jitter
                } else {
                    0
                };
                let p = [x * s + jx, y * s + jy];
                if d.cell_at(p) != Some(input.cell)
                    || !point_in_polygon(p, &zone.polygon)
                    || zone.exclusions.iter().any(|v| point_in_polygon(p, v))
                    || d.buildings
                        .iter()
                        .any(|v| point_in_polygon(p, &v.footprint))
                    || d.roads.iter().any(|r| road_contains(p, r, 100))
                {
                    continue;
                }
                let id = format!("{}:{x}:{y}", zone.id);
                let request = SpawnRequest {
                    position_cm: p,
                    surface_id: "terrain".into(),
                };
                let position = if d.recipe_version == 1 {
                    b.chunk.recipe_v1_spawn(&request)?
                } else {
                    // A cut is empty space, never an invented tree support.
                    let Ok(position) = b.chunk.spawn(&request) else { continue; };
                    position
                };
                b.box_shape(
                    [position[0], position[1] + i64::from(crate::query::TREE_PROXY_SIZE_CM[1] / 2), position[2]],
                    crate::query::TREE_PROXY_SIZE_CM,
                    &id,
                )?;
                b.chunk.objects.push(GeneratedObject {
                    id,
                    asset_id: "builtin:tree".into(),
                    position,
                    quarter_turns: (random % 4) as u8,
                });
            }
        }
    }
    for p in &d.placements {
        let asset = d.assets.iter().find(|a| a.id == p.asset_id).unwrap();
        for proxy in &asset.collision {
            let mut c = proxy.center;
            let mut size = proxy.size_cm;
            for _ in 0..p.quarter_turns {
                c = [-c[2], c[1], c[0]];
                size.swap(0, 2);
            }
            b.box_shape(
                [
                    p.position[0] + c[0],
                    p.position[1] + c[1],
                    p.position[2] + c[2],
                ],
                size,
                &p.id,
            )?;
        }
        if d.cell_at([p.position[0], p.position[2]]) == Some(input.cell) {
            b.chunk.objects.push(GeneratedObject {
                id: p.id.clone(),
                asset_id: p.asset_id.clone(),
                position: p.position,
                quarter_turns: p.quarter_turns,
            });
        }
    }
    Ok(GeneratedOccupancy {
        chunk: b.chunk,
        solids: b.occupancy.map_or_else(Vec::new, |value| value.solids),
    })
}
