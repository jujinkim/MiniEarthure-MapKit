//! Bounded simultaneous integer subdivision for current ground surfaces.
//! Every neighboring face receives the same rounded intersection before emission.
use crate::*;
use i_overlay::{
    core::fill_rule::FillRule,
    i_float::int::point::IntPoint,
    string::{overlay::StringOverlay, rule::StringRule},
};

const MAX_LINES: usize = 512;
const MAX_CROSSING_PAIRS: usize = 8192;
type Rings = Vec<Vec<Point>>;

fn area(ring: &[Point]) -> i128 {
    (0..ring.len())
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
        })
        .sum()
}

fn clip_line(bounds: &Bounds, a: Point, b: Point) -> Option<(Point, Point)> {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    let (mut low, mut high) = ((0i128, 1i128), (1i128, 1i128));
    let less = |x: (i128, i128), y: (i128, i128)| x.0 * y.1 < y.0 * x.1;
    for axis in 0..2 {
        let delta = (b[axis] - a[axis]) as i128;
        if delta == 0 {
            if a[axis] < bounds.min[axis] || a[axis] > bounds.max[axis] {
                return None;
            }
            continue;
        }
        let first = ((bounds.min[axis] - a[axis]) as i128, delta);
        let last = ((bounds.max[axis] - a[axis]) as i128, delta);
        let (lo, hi) = if delta > 0 {
            (first, last)
        } else {
            ((-last.0, -last.1), (-first.0, -first.1))
        };
        if less(low, lo) {
            low = lo;
        }
        if less(hi, high) {
            high = hi;
        }
        if less(high, low) {
            return None;
        }
    }
    let at = |t: (i128, i128)| {
        std::array::from_fn(|i| {
            let numerator = a[i] as i128 * t.1 + (b[i] - a[i]) as i128 * t.0;
            (2 * numerator + t.1).div_euclid(2 * t.1) as i64
        })
    };
    let (a, b) = (at(low), at(high));
    (a != b).then_some((a, b))
}

pub(crate) fn subdivide(
    bounds: &Bounds,
    lines: &[(Point, Point)],
    work: &mut usize,
) -> Result<Vec<Rings>> {
    let mut unique = BTreeSet::new();
    for &(a, b) in lines {
        if a.iter()
            .chain(b.iter())
            .any(|v| v.unsigned_abs() > 1_000_000_000)
        {
            return Err(error(
                "E_GEOMETRY",
                "arrangement coordinate outside integer profile",
            ));
        }
        if let Some((a, b)) = clip_line(bounds, a, b) {
            unique.insert(if a < b { (a, b) } else { (b, a) });
        }
        if unique.len() > MAX_LINES {
            return Err(error("E_BUDGET", "ground tile arrangement edge limit"));
        }
    }
    // Bound potential intersections rather than rejecting many disjoint edges.
    // The x-sorted sweep charges every comparison before the external splitter.
    let ordered: Vec<_> = unique.iter().copied().collect();
    let mut pairs = 0;
    for (i, &(a, b)) in ordered.iter().enumerate() {
        for &(c, d) in &ordered[i + 1..] {
            crate::roads::tick(work, 1)?;
            if c[0] > b[0] + 2 {
                break;
            }
            if a[1].min(b[1]) - 2 <= c[1].max(d[1]) && c[1].min(d[1]) - 2 <= a[1].max(b[1]) {
                pairs += 1;
            }
            if pairs > MAX_CROSSING_PAIRS {
                return Err(error("E_BUDGET", "ground tile intersection limit"));
            }
        }
    }
    crate::roads::tick(work, pairs + unique.len())?;
    let point = |p: Point| IntPoint::<i64>::new(p[0], p[1]);
    let mut overlay = StringOverlay::with_shape_contour(
        &[
            bounds.min,
            [bounds.max[0], bounds.min[1]],
            bounds.max,
            [bounds.min[0], bounds.max[1]],
        ]
        .map(point),
    );
    for (a, b) in unique {
        overlay.add_string_line([point(a), point(b)]);
    }
    let graph = overlay
        .build_graph_view(FillRule::NonZero)
        .ok_or_else(|| error("E_GEOMETRY", "empty ground tile arrangement"))?;
    let mut shapes: Vec<Rings> = graph
        .extract_shapes(StringRule::Slice)
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .enumerate()
                .map(|(i, ring)| {
                    let mut ring: Vec<Point> = ring.into_iter().map(|p| [p.x, p.y]).collect();
                    if (area(&ring) > 0) != (i == 0) {
                        ring.reverse();
                    }
                    if let Some(start) = ring
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, p)| **p)
                        .map(|(i, _)| i)
                    {
                        ring.rotate_left(start);
                    }
                    ring
                })
                .collect()
        })
        .collect();
    let mut total = 0i128;
    for shape in &mut shapes {
        if shape.is_empty() {
            return Err(error("E_GEOMETRY", "empty ground arrangement face"));
        }
        shape[1..].sort();
        for ring in shape.iter() {
            if ring.len() < 3 || ring.iter().any(|p| !bounds.contains(*p)) {
                return Err(error(
                    "E_GEOMETRY",
                    format!(
                        "ground arrangement leaves tile {:?}..{:?}: {:?}",
                        bounds.min, bounds.max, ring
                    ),
                ));
            }
            total += area(ring);
        }
    }
    let expected =
        2 * (bounds.max[0] - bounds.min[0]) as i128 * (bounds.max[1] - bounds.min[1]) as i128;
    if total != expected {
        return Err(error("E_GEOMETRY", "ground arrangement area mismatch"));
    }
    shapes.sort();
    Ok(shapes)
}

pub(crate) fn triangulate(rings: &[Vec<Point>], work: &mut usize) -> Result<Vec<[Point; 3]>> {
    if rings.is_empty() || rings.len() > 17 || rings.iter().map(Vec::len).sum::<usize>() > 512 {
        return Err(error("E_BUDGET", "ground face triangulation limit"));
    }
    let mut triangles = crate::courtyard::triangulate_rings(&rings[0], &rings[1..], work)?;
    // Hole bridging may simplify exactly collinear vertices. Restore those exact
    // graph vertices on both adjacent triangles; never stitch near an edge.
    let points: BTreeSet<Point> = rings.iter().flatten().copied().collect();
    for p in points {
        let mut next = Vec::new();
        for t in triangles {
            crate::roads::tick(work, 1)?;
            if t.contains(&p) {
                next.push(t);
                continue;
            }
            if let Some(i) = (0..3).find(|&i| on_segment(t[i], t[(i + 1) % 3], p)) {
                next.push([t[i], p, t[(i + 2) % 3]]);
                next.push([p, t[(i + 1) % 3], t[(i + 2) % 3]]);
            } else {
                next.push(t);
            }
        }
        triangles = next;
    }
    Ok(triangles)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arrangement_limits_fail_before_unbounded_graph_work() {
        let bounds = Bounds {
            min: [-2000, -2000],
            max: [2000, 2000],
        };
        let mut work = 0;
        let many: Vec<_> = (0..513)
            .map(|i| ([-1000, i * 3 - 1000], [1000, i * 3 - 1000]))
            .collect();
        assert_eq!(
            subdivide(&bounds, &many, &mut work).unwrap_err().code,
            "E_BUDGET"
        );
        let dense: Vec<_> = (0..200)
            .map(|i| ([-1000, i * 5 - 500], [1000, 500 - i * 5]))
            .collect();
        assert_eq!(
            subdivide(&bounds, &dense, &mut work).unwrap_err().code,
            "E_BUDGET"
        );
        let mut exhausted = 8_000_000;
        assert_eq!(
            subdivide(&bounds, &[([-1000, 0], [1000, 0])], &mut exhausted)
                .unwrap_err()
                .code,
            "E_BUDGET"
        );
        assert_eq!(
            subdivide(&bounds, &[([1_000_000_001, 0], [0, 0])], &mut work)
                .unwrap_err()
                .code,
            "E_GEOMETRY"
        );
    }

    #[test]
    fn enclosed_ring_preserves_hole_edges_without_filling_twice() {
        let bounds = Bounds {
            min: [0, 0],
            max: [500, 500],
        };
        let ring = [[101, 103], [307, 103], [307, 309], [101, 309]];
        let lines: Vec<_> = (0..4).map(|i| (ring[i], ring[(i + 1) % 4])).collect();
        let mut work = 0;
        let faces = subdivide(&bounds, &lines, &mut work).unwrap();
        assert_eq!(faces.len(), 2);
        assert!(faces.iter().any(|rings| rings.len() == 2));
        let triangles: Vec<_> = faces
            .iter()
            .flat_map(|rings| triangulate(rings, &mut work).unwrap())
            .collect();
        assert_eq!(
            triangles
                .iter()
                .map(|t| cross(t[0], t[1], t[2]).abs())
                .sum::<i128>(),
            500_000
        );
        let mut edges = BTreeMap::new();
        for t in triangles {
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                *edges
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_insert(0) += 1;
            }
        }
        for ((a, b), n) in edges {
            let outer = (0..2).any(|i| a[i] == b[i] && [0, 500].contains(&a[i]));
            assert_eq!(n, if outer { 1 } else { 2 });
        }
    }
}
