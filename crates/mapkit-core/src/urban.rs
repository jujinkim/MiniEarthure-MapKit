//! Recipe 6 ground paint. Integer subdivisions share the road clipping rule.
use crate::generation::{polygon_triangles, Builder};
use crate::roads::{emit, partition, tick, valid};
use crate::*;

pub(crate) struct Paint<'a> {
    triangle: [Vertex; 3],
    area: &'a SurfaceArea,
}
fn overlaps(a: &[Vertex], b: &[Vertex]) -> bool {
    [0, 2].iter().all(|&i| {
        a.iter().map(|p| p[i]).min().unwrap() <= b.iter().map(|p| p[i]).max().unwrap()
            && b.iter().map(|p| p[i]).min().unwrap() <= a.iter().map(|p| p[i]).max().unwrap()
    })
}
pub(crate) fn validate(d: &MapDocument) -> Result<()> {
    if d.recipe_version < 6
        && (!d.surface_areas.is_empty() || d.roads.iter().any(|r| r.markings.is_some()))
    {
        return Err(error(
            "E_VERSION",
            "surface areas and road markings require recipe 6",
        ));
    }
    for road in &d.roads {
        if road
            .markings
            .as_ref()
            .is_some_and(|m| !(1..=8).contains(&m.lanes))
        {
            return Err(error("E_GEOMETRY", "road markings require 1..8 lanes"));
        }
    }
    let mut triangles = Vec::new();
    for area in &d.surface_areas {
        if !polygon_valid(&area.polygon, &d.bounds) {
            return Err(error("E_GEOMETRY", "invalid surface area polygon"));
        }
        triangles.push(
            polygon_triangles(&area.polygon)?
                .into_iter()
                .map(|t| t.map(|i| [area.polygon[i][0], 0, area.polygon[i][1]]))
                .collect::<Vec<_>>(),
        );
    }
    let mut work = 0;
    for (i, a) in triangles.iter().enumerate() {
        for b in &triangles[..i] {
            for ta in a {
                for tb in b {
                    tick(&mut work, 1)?;
                    if overlaps(ta, tb) && valid(&partition(ta, tb, &mut work)?.0) {
                        return Err(error("E_GEOMETRY", "surface area interiors overlap"));
                    }
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn areas<'a>(d: &'a MapDocument, bounds: &Bounds) -> Result<Vec<Paint<'a>>> {
    let mut out = vec![];
    for area in &d.surface_areas {
        if (0..2).any(|i| {
            area.polygon.iter().map(|p| p[i]).max().unwrap() < bounds.min[i]
                || area.polygon.iter().map(|p| p[i]).min().unwrap() > bounds.max[i]
        }) {
            continue;
        }
        for t in polygon_triangles(&area.polygon)? {
            out.push(Paint {
                triangle: t.map(|i| [area.polygon[i][0], 0, area.polygon[i][1]]),
                area,
            });
        }
    }
    Ok(out)
}
pub(crate) fn paint(
    b: &mut Builder,
    poly: &[Vertex],
    areas: &[Paint],
    terrain: &[Vertex; 3],
    work: &mut usize,
) -> Result<()> {
    let mut remaining = vec![poly.to_vec()];
    for paint in areas {
        let mut next = vec![];
        for p in remaining {
            tick(work, 1)?;
            if !overlaps(&p, &paint.triangle) {
                next.push(p);
                continue;
            }
            let (inside, outside) = partition(&p, &paint.triangle, work)?;
            let inside: Vec<_> = inside
                .into_iter()
                .map(|p| crate::roads::on_plane(terrain, p))
                .collect();
            emit(b, &inside, paint.area.surface, &paint.area.id, true)?;
            next.extend(outside);
        }
        remaining = next;
        if remaining.len() > 16384 {
            return Err(error("E_BUDGET", "surface fragments exceeded"));
        }
    }
    for mut p in remaining {
        if !areas.is_empty() {
            p = p
                .into_iter()
                .map(|p| crate::roads::on_plane(terrain, p))
                .collect();
        }
        emit(b, &p, Surface::Grass, "terrain", true)?;
    }
    Ok(())
}
pub(crate) fn subtract(
    mut polys: Vec<Vec<Vertex>>,
    ring: &[Point],
    work: &mut usize,
) -> Result<Vec<Vec<Vertex>>> {
    if polys.is_empty() {
        return Ok(polys);
    }
    let v: Vec<_> = ring.iter().map(|p| [p[0], 0, p[1]]).collect();
    tick(work, 1)?;
    if !polys.iter().any(|p| overlaps(p, &v)) {
        return Ok(polys);
    }
    for t in polygon_triangles(ring)? {
        let clip = t.map(|i| v[i]);
        let mut next = vec![];
        for p in polys {
            next.extend(partition(&p, &clip, work)?.1);
        }
        if next.len() > 16384 {
            return Err(error("E_BUDGET", "sidewalk exclusions exceeded"));
        }
        polys = next;
    }
    Ok(polys)
}
