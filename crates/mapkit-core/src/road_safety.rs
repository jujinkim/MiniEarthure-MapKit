//! Automatic, non-spawnable deck furniture. Display, contact and occupancy
//! receive the very same convex pieces; metal is only a presentation identity.
use crate::{
    generation::Builder,
    road_plan::{hit, length, Edge},
    *,
};

pub(crate) const OUTSET_CM: i64 = 20;

pub(crate) fn generate(d: &MapDocument, edges: &[Edge], b: &mut Builder) -> Result<()> {
    let mut posts = BTreeSet::new();
    for edge in edges {
        if !matches!(edge.road.kind, RoadKind::Bridge | RoadKind::Elevated)
            || !hit(&[edge.a, edge.b], &b.bounds, OUTSET_CM)
        {
            continue;
        }
        crate::cancellation::checkpoint()?;
        let len = length(edge.a, edge.b);
        if len < 1.0 {
            continue;
        }
        let bridge = edge.road.kind == RoadKind::Bridge;
        if bridge {
            piece(d, b, edge, edge.a, edge.b, 0, 20, 0, 20, "base")?;
        }
        for (bottom, top) in if bridge {
            [(40, 48), (72, 80)]
        } else {
            [(42, 56), (66, 80)]
        } {
            piece(d, b, edge, edge.a, edge.b, bottom, top, 4, 12, "metal")?;
        }
        // Stationing is fixed on the complete source edge/junction before cell
        // clipping. A neighboring cell therefore receives identical posts.
        let Some((first, last)) = post_range(edge, &b.bounds) else {
            continue;
        };
        let mut station = first;
        while station <= last {
            let t = (station - edge.station_cm) / len;
            let at: Vertex = std::array::from_fn(|i| {
                edge.a[i] + libm::round((edge.b[i] - edge.a[i]) as f64 * t) as i64
            });
            if posts.insert((at, bridge)) {
                let endpoint = |offset: f64| {
                    std::array::from_fn(|i| {
                        at[i] + libm::round((edge.b[i] - edge.a[i]) as f64 * offset / len) as i64
                    })
                };
                piece(
                    d,
                    b,
                    edge,
                    endpoint(-4.0),
                    endpoint(4.0),
                    if bridge { 20 } else { 0 },
                    80,
                    4,
                    12,
                    "metal",
                )?;
            }
            station += 200.0;
        }
    }
    Ok(())
}

fn piece(
    d: &MapDocument,
    b: &mut Builder,
    edge: &Edge,
    a: Vertex,
    c: Vertex,
    bottom: i64,
    top: i64,
    inside: i64,
    outside: i64,
    material: &str,
) -> Result<()> {
    let len = length(a, c);
    if len < 1.0 {
        return Ok(());
    }
    let offset = |p: Vertex, n: i64, h: i64| {
        [
            p[0] + libm::round((c[2] - a[2]) as f64 * n as f64 / len) as i64,
            p[1] + h,
            p[2] - libm::round((c[0] - a[0]) as f64 * n as f64 / len) as i64,
        ]
    };
    let vertices = vec![
        offset(a, inside, bottom),
        offset(c, inside, bottom),
        offset(c, outside, bottom),
        offset(a, outside, bottom),
        offset(a, inside, top),
        offset(c, inside, top),
        offset(c, outside, top),
        offset(a, outside, top),
    ];
    let mut faces = Vec::new();
    for q in [
        [0, 1, 2, 3],
        [4, 7, 6, 5],
        [0, 4, 5, 1],
        [1, 5, 6, 2],
        [2, 6, 7, 3],
        [3, 7, 4, 0],
    ] {
        faces.push([q[0], q[1], q[2]]);
        faces.push([q[0], q[2], q[3]]);
    }
    let mut shape = CollisionConvex { vertices, faces };
    // Centroid-side orientation is integer and independent of source direction.
    let center: Vertex = std::array::from_fn(|i| shape.vertices.iter().map(|p| p[i]).sum::<i64>());
    let planes: Vec<_> = shape.planes().collect();
    for (face, (n, p)) in shape.faces.iter_mut().zip(planes) {
        if (0..3)
            .map(|i| n[i] * (center[i] - 8 * p[i]) as i128)
            .sum::<i128>()
            > 0
        {
            face.swap(1, 2);
        }
    }
    if !shape.valid(1_000_000_000) {
        return Err(error(
            "E_GEOMETRY",
            format!("{} at {:?}: invalid safety solid", edge.road.id, a),
        ));
    }
    let area = shape.bounds();
    let min_y = shape.vertices.iter().map(|v| v[1]).min().unwrap();
    let max_y = shape.vertices.iter().map(|v| v[1]).max().unwrap();
    for g in &d.gimmicks {
        for (min, max) in g.occupancy_bounds() {
            if min[1] < max_y
                && max[1] > min_y
                && min[0] < area.max[0]
                && max[0] > area.min[0]
                && min[2] < area.max[1]
                && max[2] > area.min[1]
                && intersects_box(&shape, min, max)
            {
                return Err(error(
                    "E_PLACEMENT",
                    format!(
                        "{} at {:?}: automatic safety facility overlaps authored {}",
                        edge.road.id, a, g.id
                    ),
                ));
            }
        }
    }
    for p in &d.placements {
        for proxy in crate::placement::proxies(d, p) {
            let min: Vertex =
                std::array::from_fn(|i| proxy.center[i] - proxy.size_cm[i] as i64 / 2);
            let max: Vertex = std::array::from_fn(|i| min[i] + proxy.size_cm[i] as i64);
            if min[1] < max_y
                && max[1] > min_y
                && min[0] < area.max[0]
                && max[0] > area.min[0]
                && min[2] < area.max[1]
                && max[2] > area.min[1]
                && intersects_box(&shape, min, max)
            {
                return Err(error(
                    "E_PLACEMENT",
                    format!(
                        "{} at {:?}: automatic safety facility overlaps authored {}",
                        edge.road.id, a, p.id
                    ),
                ));
            }
        }
    }
    b.convex_shape(shape, &format!("{}:safety:{material}", edge.road.id))
}

// Full convex-vs-box SAT, after the cheap AABB phase. A sloping or diagonal
// rail must not diagnose an unrelated prop in an empty corner of its bounds.
fn intersects_box(shape: &CollisionConvex, min: Vertex, max: Vertex) -> bool {
    let vertices: Vec<Vertex> = (0..8)
        .map(|i| std::array::from_fn(|j| if i & (1 << j) == 0 { min[j] } else { max[j] }))
        .collect();
    let mut axes: Vec<[i128; 3]> = shape.planes().map(|(n, _)| n).collect();
    axes.extend([[1, 0, 0], [0, 1, 0], [0, 0, 1]]);
    for f in &shape.faces {
        for i in 0..3 {
            let (a, b) = (
                shape.vertices[f[i] as usize],
                shape.vertices[f[(i + 1) % 3] as usize],
            );
            let v: [i128; 3] = std::array::from_fn(|j| (b[j] - a[j]) as i128);
            axes.extend([[0, v[2], -v[1]], [-v[2], 0, v[0]], [v[1], -v[0], 0]]);
        }
    }
    axes.into_iter().filter(|n| *n != [0; 3]).all(|n| {
        let range = |points: &[Vertex]| {
            let mut lo = i128::MAX;
            let mut hi = i128::MIN;
            for p in points {
                let dot = (0..3).map(|i| n[i] * p[i] as i128).sum::<i128>();
                lo = lo.min(dot);
                hi = hi.max(dot);
            }
            (lo, hi)
        };
        let (a, b) = range(&shape.vertices);
        let (c, d) = range(&vertices);
        a < d && c < b
    })
}

pub(crate) fn post_range(edge: &Edge, bounds: &Bounds) -> Option<(f64, f64)> {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for i in [0, 2] {
        let a = edge.a[i] as f64;
        let delta = (edge.b[i] - edge.a[i]) as f64;
        let min = (bounds.min[i / 2] - 24) as f64;
        let max = (bounds.max[i / 2] + 24) as f64;
        if delta == 0.0 {
            if a < min || a > max {
                return None;
            }
            continue;
        }
        let (p, q) = ((min - a) / delta, (max - a) / delta);
        lo = lo.max(p.min(q));
        hi = hi.min(p.max(q));
    }
    if lo > hi {
        return None;
    }
    let len = length(edge.a, edge.b);
    Some((
        libm::ceil((edge.station_cm + len * lo) / 200.0) * 200.0,
        edge.station_cm + len * hi,
    ))
}
