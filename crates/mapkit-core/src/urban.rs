//! Ground paint. Integer subdivisions share the road clipping rule.
use crate::generation::polygon_triangles;
use crate::roads::{partition, tick, valid};
use crate::*;

fn overlaps(a: &[Vertex], b: &[Vertex]) -> bool {
    [0, 2].iter().all(|&i| {
        a.iter().map(|p| p[i]).min().unwrap() <= b.iter().map(|p| p[i]).max().unwrap()
            && b.iter().map(|p| p[i]).min().unwrap() <= a.iter().map(|p| p[i]).max().unwrap()
    })
}
pub(crate) fn validate(d: &MapDocument) -> Result<()> {
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
