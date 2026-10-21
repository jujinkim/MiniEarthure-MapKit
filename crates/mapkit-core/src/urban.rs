//! Ground paint. Integer subdivisions share the road clipping rule.
use crate::generation::polygon_triangles;
use crate::roads::tick;
use crate::*;

/// Open boxes meet with positive area. Every triangle lies in its own closed
/// box, so boxes that are disjoint or only touch (zero-area intersection)
/// cannot hold triangles whose interiors overlap: [`interiors_overlap`] is an
/// exact separating-axis test and refuses only a shared interior.
pub(crate) fn open_boxes_meet(a: &Bounds, b: &Bounds) -> bool {
    (0..2).all(|i| a.min[i] < b.max[i] && b.min[i] < a.max[i])
}
fn triangle_bounds(t: &[Vertex; 3]) -> Bounds {
    Bounds {
        min: [0, 2].map(|i| t.iter().map(|p| p[i]).min().unwrap()),
        max: [0, 2].map(|i| t.iter().map(|p| p[i]).max().unwrap()),
    }
}
/// Positive-area triangle intersection without creating rounded clip vertices.
/// Quantized clipping is appropriate for emitted geometry, but a sequence of
/// truncations can invent a thin intersection between disjoint concave paints.
pub(crate) fn interiors_overlap(a: &[Vertex; 3], b: &[Vertex; 3]) -> bool {
    let area = |t: &[Vertex; 3]| {
        (t[1][0] - t[0][0]) as i128 * (t[2][2] - t[0][2]) as i128
            - (t[1][2] - t[0][2]) as i128 * (t[2][0] - t[0][0]) as i128
    };
    if area(a) == 0 || area(b) == 0 {
        return false;
    }
    for triangle in [a, b] {
        for i in 0..3 {
            let p = triangle[i];
            let q = triangle[(i + 1) % 3];
            let project = |v: &Vertex| {
                (q[0] - p[0]) as i128 * (v[2] - p[2]) as i128
                    - (q[2] - p[2]) as i128 * (v[0] - p[0]) as i128
            };
            let aa = a.map(|v| project(&v));
            let bb = b.map(|v| project(&v));
            if aa.iter().max().unwrap() <= bb.iter().min().unwrap()
                || bb.iter().max().unwrap() <= aa.iter().min().unwrap()
            {
                return false;
            }
        }
    }
    true
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
    paint_disjoint(d, &mut 0)
}
/// Paint pieces may touch, never share interior. `work` receives the charge.
fn paint_disjoint(d: &MapDocument, work: &mut usize) -> Result<()> {
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
    let bounds: Vec<_> = d.surface_areas.iter()
        .map(|a| crate::bounds_index::bounds(&a.polygon)).collect();
    let boxes: Vec<Vec<_>> = triangles.iter()
        .map(|t| t.iter().map(triangle_bounds).collect()).collect();
    tick(work, bounds.len().saturating_mul(bounds.len().max(1).ilog2() as usize + 1))?;
    let index = crate::bounds_index::BoundsIndex::new(&bounds);
    let (mut near_a, mut near_b) = (Vec::new(), Vec::new());
    for i in 0..triangles.len() {
        for j in index.query(&bounds[i], work)?.into_iter().filter(|&j| j < i) {
            // Pieces whose boxes only touch (adjacent paint cells) share no
            // interior and are never charged.
            if !open_boxes_meet(&bounds[i], &bounds[j]) { continue; }
            let (a, b) = (&boxes[i], &boxes[j]);
            // Only triangles meeting the shared box can meet each other.
            let shared = Bounds {
                min: [0, 1].map(|k| bounds[i].min[k].max(bounds[j].min[k])),
                max: [0, 1].map(|k| bounds[i].max[k].min(bounds[j].max[k])),
            };
            near_a.clear();
            near_a.extend((0..a.len()).filter(|&k| open_boxes_meet(&a[k], &shared)));
            near_b.clear();
            near_b.extend((0..b.len()).filter(|&k| open_boxes_meet(&b[k], &shared)));
            for &ta in &near_a {
                for &tb in &near_b {
                    tick(work, 1)?;
                    if open_boxes_meet(&a[ta], &b[tb]) {
                        // Retain the prior per-triangle clipping work charge.
                        tick(work, 9)?;
                        if interiors_overlap(&triangles[i][ta], &triangles[j][tb]) {
                            return Err(error("E_GEOMETRY", "surface area interiors overlap"));
                        }
                    }
                }
            }
            // Charge the narrowing after the scan, capped by the full pair
            // scan: the running total never exceeds the unnarrowed charge.
            let pairs = near_a.len() * near_b.len();
            tick(work, (a.len() * b.len()).min(a.len() + b.len() + pairs) - pairs)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> i64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 33) as i64
    }

    /// The touching-box skip relies on this: triangles whose boxes meet with
    /// zero area never share interior, whatever their shape or orientation.
    #[test]
    fn triangles_with_touching_boxes_never_overlap() {
        let mut seed = 7;
        let mut tested = 0;
        for _ in 0..20_000 {
            let mut a: [Vertex; 3] = std::array::from_fn(|_| [lcg(&mut seed) % 41, 0, lcg(&mut seed) % 41]);
            let mut b: [Vertex; 3] = std::array::from_fn(|_| [lcg(&mut seed) % 41, 0, lcg(&mut seed) % 41]);
            // Shift b so its box touches a's box along x, along z, or at a corner.
            let (ba, bb) = (triangle_bounds(&a), triangle_bounds(&b));
            let mode = lcg(&mut seed) % 3;
            let dx = if mode != 1 { ba.max[0] - bb.min[0] } else { 0 };
            let dz = if mode != 0 { ba.max[1] - bb.min[1] } else { 0 };
            for p in &mut b { p[0] += dx; p[2] += dz; }
            if lcg(&mut seed) % 2 == 0 { std::mem::swap(&mut a, &mut b); }
            if open_boxes_meet(&triangle_bounds(&a), &triangle_bounds(&b)) { continue; }
            tested += 1;
            assert!(!interiors_overlap(&a, &b), "{a:?} {b:?}");
        }
        assert!(tested > 15_000);
    }

    fn square(id: &str, x: i64, y: i64, size: i64) -> SurfaceArea {
        SurfaceArea { id: id.into(), polygon: vec![[x, y], [x + size, y], [x + size, y + size], [x, y + size]], surface: Surface::Concrete }
    }
    /// Index build plus traversal charges: the work left when no pair is charged.
    fn index_only(d: &MapDocument) -> usize {
        let bounds: Vec<_> = d.surface_areas.iter().map(|a| crate::bounds_index::bounds(&a.polygon)).collect();
        let mut work = bounds.len() * (bounds.len().max(1).ilog2() as usize + 1);
        let index = crate::bounds_index::BoundsIndex::new(&bounds);
        for b in &bounds { index.query(b, &mut work).unwrap(); }
        work
    }

    #[test]
    fn touching_paint_cells_are_never_charged() {
        let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        d.bounds.max = [102_400, 102_400];
        // A 20 x 20 lattice of 32 m paint cells: every neighbour, including the
        // diagonal ones, only touches.
        d.surface_areas = (0..400).map(|i| square(&format!("p{i}"), (i % 20) * 3200, (i / 20) * 3200, 3200)).collect();
        let mut work = 0;
        paint_disjoint(&d, &mut work).unwrap();
        assert_eq!(work, index_only(&d));
        // Box-overlapping pieces that only touch along a diagonal are still
        // tested exactly and charged; a shared interior is still refused.
        d.surface_areas = vec![
            SurfaceArea { id: "a".into(), polygon: vec![[0, 0], [3200, 0], [0, 3200]], surface: Surface::Grass },
            SurfaceArea { id: "b".into(), polygon: vec![[3200, 0], [3200, 3200], [0, 3200]], surface: Surface::Dirt },
        ];
        let mut work = 0;
        paint_disjoint(&d, &mut work).unwrap();
        assert_eq!(work, index_only(&d) + 10);
        d.surface_areas.push(square("c", 1000, 1000, 100));
        let e = paint_disjoint(&d, &mut 0).unwrap_err();
        assert_eq!((e.code.as_str(), e.message.as_str()), ("E_GEOMETRY", "surface area interiors overlap"));
    }
}
