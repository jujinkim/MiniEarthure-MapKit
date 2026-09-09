//! Recipe 5: bounded integer rings and deterministic visible bridges. No inserted
//! rounded vertices: triangulation, solids and roofs share original centimetres.
use crate::*;
const MAX_WORK: usize = 4_000_000;
fn tick(work: &mut usize) -> Result<()> {
    *work += 1;
    if *work > MAX_WORK {
        Err(error(
            "E_BUDGET",
            "courtyard topology/triangulation work limit",
        ))
    } else {
        Ok(())
    }
}
fn area(p: &[Point]) -> i128 {
    (0..p.len())
        .map(|i| {
            p[i][0] as i128 * p[(i + 1) % p.len()][1] as i128
                - p[i][1] as i128 * p[(i + 1) % p.len()][0] as i128
        })
        .sum()
}
fn boundary(p: Point, ring: &[Point]) -> bool {
    (0..ring.len()).any(|i| on_segment(ring[i], ring[(i + 1) % ring.len()], p))
}
impl Building {
    /// Boundary belongs to the solid, including courtyard walls.
    pub fn contains(&self, p: Point) -> bool {
        point_in_polygon(p, &self.footprint)
            && !self
                .holes
                .iter()
                .any(|h| point_in_polygon(p, h) && !boundary(p, h))
    }
}
pub(crate) fn validate(b: &Building, bounds: &Bounds, work: &mut usize) -> Result<()> {
    if b.holes.is_empty() {
        return Ok(());
    }
    if b.roof != "flat"
        || b.holes.len() > 16
        || b.footprint.len() + b.holes.iter().map(Vec::len).sum::<usize>() > 512
    {
        return Err(error(
            "E_GEOMETRY",
            "courtyard requires flat roof, at most 16 holes and 512 total vertices",
        ));
    }
    if !polygon_valid(&b.footprint, bounds) {
        return Err(error("E_GEOMETRY", "invalid courtyard outer ring"));
    }
    for (i, h) in b.holes.iter().enumerate() {
        if !polygon_valid(h, bounds) || !point_in_polygon(h[0], &b.footprint) {
            return Err(error("E_GEOMETRY", "invalid/outside courtyard ring"));
        }
        for other in std::iter::once(&b.footprint).chain(b.holes[..i].iter()) {
            for j in 0..h.len() {
                for k in 0..other.len() {
                    tick(work)?;
                    if intersects(
                        h[j],
                        h[(j + 1) % h.len()],
                        other[k],
                        other[(k + 1) % other.len()],
                    ) {
                        return Err(error("E_GEOMETRY", "courtyard rings touch or intersect"));
                    }
                }
            }
        }
        for other in &b.holes[..i] {
            if point_in_polygon(h[0], other) || point_in_polygon(other[0], h) {
                return Err(error("E_GEOMETRY", "courtyard holes overlap or nest"));
            }
        }
    }
    triangulate(b, work)?;
    Ok(())
}
fn canonical(p: &[Point], positive: bool) -> Vec<Point> {
    let mut p = p.to_vec();
    // Remove redundant collinear vertices only in the new courtyard recipe path.
    loop {
        let n = p.len();
        let Some(i) = (0..n).find(|&i| cross(p[(i + n - 1) % n], p[i], p[(i + 1) % n]) == 0) else {
            break;
        };
        p.remove(i);
    }
    if (area(&p) > 0) != positive {
        p.reverse();
    }
    let start = p.iter().enumerate().min_by_key(|(_, p)| **p).unwrap().0;
    p.rotate_left(start);
    p
}
pub(crate) fn triangulate(b: &Building, work: &mut usize) -> Result<Vec<[Point; 3]>> {
    if b.holes.is_empty() {
        return Ok(crate::generation::polygon_triangles(&b.footprint)?
            .into_iter()
            .map(|t| t.map(|i| b.footprint[i]))
            .collect());
    }
    let mut ring = canonical(&b.footprint, true);
    let mut holes: Vec<_> = b.holes.iter().map(|h| canonical(h, false)).collect();
    holes.sort();
    let target = area(&ring) + holes.iter().map(|h| area(h)).sum::<i128>();
    for hole in &holes {
        let mut best = None;
        for (i, &a) in ring.iter().enumerate() {
            for (j, &c) in hole.iter().enumerate() {
                tick(work)?;
                // A duplicated bridge endpoint has multiple local sectors. Choose
                // the occurrence whose interior cone contains the new bridge.
                let prev = ring[(i + ring.len() - 1) % ring.len()];
                let next = ring[(i + 1) % ring.len()];
                let local = if cross(prev, a, next) > 0 {
                    cross(a, c, next) <= 0 && cross(a, prev, c) <= 0
                } else {
                    cross(a, c, prev) > 0 || cross(a, next, c) > 0
                };
                if !local {
                    continue;
                }
                let mut clear = true;
                for edge in std::iter::once(&ring).chain(holes.iter()) {
                    for k in 0..edge.len() {
                        tick(work)?;
                        let (u, v) = (edge[k], edge[(k + 1) % edge.len()]);
                        if !intersects(a, c, u, v) {
                            continue;
                        }
                        if (u == a || u == c || v == a || v == c)
                            && cross(a, c, if u == a || u == c { v } else { u }) != 0
                        {
                            continue;
                        }
                        clear = false;
                        break;
                    }
                    if !clear {
                        break;
                    }
                }
                if !clear {
                    continue;
                }
                // Exact half-centimetre midpoint classification, without truncation.
                let p = [a[0] + c[0], a[1] + c[1]];
                let doubled =
                    |r: &[Point]| r.iter().map(|p| [p[0] * 2, p[1] * 2]).collect::<Vec<_>>();
                if !point_in_polygon(p, &doubled(&b.footprint))
                    || holes.iter().any(|h| point_in_polygon(p, &doubled(h)))
                {
                    continue;
                }
                let length = (a[0] - c[0]) as i128 * (a[0] - c[0]) as i128
                    + (a[1] - c[1]) as i128 * (a[1] - c[1]) as i128;
                let key = (length, a, c, i, j);
                if best.is_none_or(|v| key < v) {
                    best = Some(key);
                }
            }
        }
        let Some((_, _, _, i, j)) = best else {
            return Err(error("E_GEOMETRY", "courtyard has no visible bridge"));
        };
        let mut merged = ring[..=i].to_vec();
        merged.extend((0..=hole.len()).map(|k| hole[(j + k) % hole.len()]));
        merged.extend_from_slice(&ring[i..]);
        ring = merged;
    }
    let mut out = vec![];
    while ring.len() > 3 {
        let n = ring.len();
        let mut ear = None;
        for i in 0..n {
            tick(work)?;
            let t = [ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]];
            if cross(t[0], t[1], t[2]) <= 0 {
                continue;
            }
            let mut blocked = false;
            for p in &ring {
                tick(work)?;
                if !t.contains(p) && point_in_polygon(*p, &t) {
                    blocked = true;
                    break;
                }
            }
            if !blocked {
                ear = Some((i, t));
                break;
            }
        }
        let Some((i, t)) = ear else {
            // Ear removal can expose a redundant collinear bridge vertex.
            if let Some(i) =
                (0..n).find(|&i| cross(ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]) == 0)
            {
                ring.remove(i);
                continue;
            }
            return Err(error(
                "E_GEOMETRY",
                "courtyard cannot be triangulated within integer profile",
            ));
        };
        out.push(t);
        ring.remove(i);
    }
    if cross(ring[0], ring[1], ring[2]) != 0 {
        out.push([ring[0], ring[1], ring[2]]);
    }
    if out.iter().any(|t| cross(t[0], t[1], t[2]) <= 0)
        || out.iter().map(|t| cross(t[0], t[1], t[2])).sum::<i128>() != target
    {
        return Err(error("E_GEOMETRY", "courtyard triangulation area mismatch"));
    }
    Ok(out)
}
