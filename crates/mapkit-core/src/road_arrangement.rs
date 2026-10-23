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

/// Whether `triangulate` accepts this face's ring count and size.
pub(crate) fn triangulable(rings: &[Vec<Point>]) -> bool {
    !(rings.is_empty() || rings.len() > 17 || rings.iter().map(Vec::len).sum::<usize>() > 512)
}

pub(crate) fn triangulate(rings: &[Vec<Point>], work: &mut usize) -> Result<Vec<[Point; 3]>> {
    if !triangulable(rings) {
        return Err(error("E_BUDGET", "ground face triangulation limit"));
    }
    // The courtyard limit bounds this face; the cell's road budget bounds the sum.
    let mut face_work = 0;
    let mut triangles = crate::courtyard::triangulate_rings(&rings[0], &rings[1..], &mut face_work)
        .map_err(|mut e|{e.message=format!("{}: {rings:?}",e.message);e})?;
    crate::roads::tick(work, face_work)?;
    // Hole bridging may simplify exactly collinear vertices. Restore those exact
    // graph vertices on both adjacent triangles; never stitch near an edge.
    let points: BTreeSet<Point> = rings.iter().flatten().copied().collect();
    // Without such a point the loop below keeps every triangle as is; charge
    // its point × triangle scan without running it.
    if !restores_any(&triangles, &points) {
        crate::roads::tick_many(work, points.len().saturating_mul(triangles.len()))?;
        return Ok(triangles);
    }
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

/// Whether a ring point lies strictly inside an edge of a triangle that does not
/// have it as a vertex: exactly when the restoration loop splits a triangle.
fn restores_any(triangles: &[[Point; 3]], points: &BTreeSet<Point>) -> bool {
    let mut by_y: Option<BTreeSet<(i64, i64)>> = None;
    triangles.iter().any(|t| (0..3).any(|i| {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        let outside = |p: &Point| !t.contains(p);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let (lo, hi) = (a.min(b), a.max(b));
        if dx == 0 && dy == 0 {
            false
        } else if dx == 0 {
            hi[1] - lo[1] >= 2 && points.range([lo[0], lo[1] + 1]..=[lo[0], hi[1] - 1]).any(outside)
        } else if dy == 0 {
            let (y, x0, x1) = (a[1], dx.min(0) + a[0], dx.max(0) + a[0]);
            x1 - x0 >= 2 && by_y.get_or_insert_with(|| points.iter().map(|p| (p[1], p[0])).collect())
                .range((y, x0 + 1)..=(y, x1 - 1)).any(|&(y, x)| outside(&[x, y]))
        } else {
            // Integer points inside the segment are `a + k * step`, 0 < k < g.
            let g = gcd(dx.unsigned_abs(), dy.unsigned_abs()) as i64;
            if g <= 64 {
                (1..g).any(|k| {
                    let p = [a[0] + k * (dx / g), a[1] + k * (dy / g)];
                    points.contains(&p) && outside(&p)
                })
            } else {
                points.range([lo[0] + 1, i64::MIN]..=[hi[0] - 1, i64::MAX])
                    .any(|p| cross(a, b, *p) == 0 && outside(p))
            }
        }
    }))
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: i64) -> i64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) % n as u64) as i64
        }
    }

    /// `triangulate` before the restoration skip: every point scans every triangle.
    fn triangulate_reference(rings: &[Vec<Point>], work: &mut usize) -> Result<Vec<[Point; 3]>> {
        if !triangulable(rings) {
            return Err(error("E_BUDGET", "ground face triangulation limit"));
        }
        let mut face_work = 0;
        let mut triangles = crate::courtyard::triangulate_rings(&rings[0], &rings[1..], &mut face_work)
            .map_err(|mut e|{e.message=format!("{}: {rings:?}",e.message);e})?;
        crate::roads::tick(work, face_work)?;
        let points: BTreeSet<Point> = rings.iter().flatten().copied().collect();
        for p in points {
            let mut next = Vec::new();
            for t in triangles {
                crate::roads::tick(work, 1)?;
                if t.contains(&p) { next.push(t); continue; }
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

    /// Random tile arrangements: road-like lines and paint rectangles (holes and
    /// collinear tile-boundary vertices), on a coarse grid for many collinear cuts.
    fn arrangements(rng: &mut Lcg, count: usize) -> Vec<Vec<Rings>> {
        let bounds = Bounds { min: [0, 0], max: [12800, 12800] };
        let mut out = vec![];
        for case in 0..count {
            let step = [1, 7, 100][case % 3];
            let mut point = |rng: &mut Lcg| [(rng.next(15_000) - 1_100) / step * step, (rng.next(15_000) - 1_100) / step * step];
            let mut lines = vec![];
            for _ in 0..rng.next(25) { lines.push((point(rng), point(rng))); }
            for _ in 0..rng.next(12) {
                let (a, w, h) = (point(rng), 50 + rng.next(3000) / step * step, 50 + rng.next(3000) / step * step);
                let r = [a, [a[0] + w, a[1]], [a[0] + w, a[1] + h], [a[0], a[1] + h]];
                for i in 0..4 { lines.push((r[i], r[(i + 1) % 4])); }
            }
            if let Ok(faces) = subdivide(&bounds, &lines, &mut 0) { out.push(faces); }
        }
        out
    }

    #[test]
    fn restoration_skip_matches_the_full_scan_and_its_work() {
        let mut rng = Lcg(5);
        let (mut faces, mut restored, mut holed) = (0, 0, 0);
        for arrangement in arrangements(&mut rng, 400) {
            for rings in arrangement.iter().filter(|r| triangulable(r)) {
                for start in [0, 7_999_000, 7_999_990, 8_000_000] {
                    let (mut old_work, mut new_work) = (start, start);
                    let old = triangulate_reference(rings, &mut old_work);
                    let new = triangulate(rings, &mut new_work);
                    assert_eq!(old, new, "{rings:?} at {start}");
                    if old.is_ok() { assert_eq!(old_work, new_work, "{rings:?} at {start}"); }
                }
                faces += 1;
                holed += usize::from(rings.len() > 1);
                let mut w = 0;
                let base = crate::courtyard::triangulate_rings(&rings[0], &rings[1..], &mut w).unwrap();
                restored += usize::from(triangulate(rings, &mut 0).unwrap().len() != base.len());
            }
        }
        assert!(faces > 2000 && holed > 50 && restored > 10, "faces {faces} holed {holed} restored {restored}");
    }

    #[test]
    fn restoration_test_matches_brute_force() {
        let mut rng = Lcg(9);
        let mut hits = 0;
        for case in 0..4000 {
            // Small grids hit edges often; scaled grids give long lattice steps.
            let (n, scale) = ([6, 12, 30][case % 3], [1, 1, 97, 1000][case % 4]);
            let p = |rng: &mut Lcg| [rng.next(n) * scale, rng.next(n) * scale];
            let mut triangles: Vec<[Point; 3]> = (0..1 + rng.next(6)).map(|_| [p(&mut rng), p(&mut rng), p(&mut rng)]).collect();
            if case % 5 == 0 { let t = triangles[0]; triangles.push([t[0], t[0], t[1]]); }
            if case % 7 == 0 { triangles.push([[0, 0], [n * scale, 0], [0, n * scale]]); }
            let points: BTreeSet<Point> = (0..1 + rng.next(10)).map(|_| p(&mut rng)).collect();
            let brute = triangles.iter().any(|t| points.iter().any(|p| !t.contains(p)
                && (0..3).any(|i| on_segment(t[i], t[(i + 1) % 3], *p))));
            assert_eq!(restores_any(&triangles, &points), brute, "{triangles:?} {points:?}");
            hits += usize::from(brute);
        }
        assert!(hits > 400 && hits < 3600, "{hits}");
    }
    #[test]
    fn quantized_junction_hole_touching_boundary_preserves_area() {
        let rings=vec![vec![[18093,98200],[18200,98200],[18200,98400],[18150,98350],[18149,98346],[18142,98330],[18135,98310],[18133,98306],[18127,98288],[18097,98212]],vec![[18134,98306],[18135,98310],[18136,98311]]];
        let tris=triangulate(&rings,&mut 0).unwrap();
        assert_eq!(tris.iter().map(|t|area(t)).sum::<i128>(),rings.iter().map(|r|area(r)).sum::<i128>());
        assert!(tris.iter().all(|t|area(t)>0));
        let rings=vec![vec![[67600,90400],[67800,90600],[67794,90600],[67781,90597],[67770,90593],[67765,90592],[67758,90590],[67754,90589],[67727,90581],[67681,90568],[67631,90555],[67619,90552],[67610,90550],[67600,90547]],vec![[67612,90550],[67616,90551],[67613,90550]],vec![[67616,90551],[67619,90552],[67626,90553]]];
        let tris=triangulate(&rings,&mut 0).unwrap();
        assert_eq!(tris.iter().map(|t|area(t)).sum::<i128>(),rings.iter().map(|r|area(r)).sum::<i128>());
    }
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
