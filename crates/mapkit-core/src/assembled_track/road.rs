//! Shared quantized cross-sections and local transitions for ordinary ribbons.
use super::*;
use crate::curve_sampling::{cross, norm};

fn normal(v: [Vertex; 3]) -> [f64; 3] {
    norm(cross(
        std::array::from_fn(|j| (v[1][j] - v[0][j]) as f64),
        std::array::from_fn(|j| (v[2][j] - v[0][j]) as f64),
    ))
}
fn splits(v: [Vertex; 4]) -> [[[Vertex; 3]; 2]; 2] {
    [
        [[v[0], v[1], v[2]], [v[0], v[2], v[3]]],
        [[v[0], v[1], v[3]], [v[1], v[2], v[3]]],
    ]
}
fn score(v: [Vertex; 4], faces: [[Vertex; 3]; 2]) -> (f64, f64) {
    let twist = std::array::from_fn::<_, 3, _>(|j| (v[0][j] - v[1][j] + v[2][j] - v[3][j]) as f64);
    let n = faces.map(normal);
    // Maximum distance between the bilinear ribbon and either triangle plane.
    let ridge = n
        .iter()
        .map(|n| (0..3).map(|j| n[j] * twist[j]).sum::<f64>().abs() / 4.0)
        .fold(0.0, f64::max);
    let bend = 1.0 - (0..3).map(|j| n[0][j] * n[1][j]).sum::<f64>();
    (ridge, bend)
}
pub(super) fn quad(v: [Vertex; 4]) -> [[Vertex; 3]; 2] {
    let candidates = splits(v);
    let a = score(v, candidates[0]);
    let b = score(v, candidates[1]);
    if b < a {
        candidates[1]
    } else {
        candidates[0]
    }
}
pub(super) fn strip_count(v: [Vertex; 4]) -> i64 {
    let ridge = score(v, quad(v)).0;
    ridge.ceil().clamp(1.0, 32.0) as i64
}

fn section([a, b]: [Vertex; 2], n: i64) -> Vec<Vertex> {
    (0..=n)
        .map(|k| {
            std::array::from_fn(|j| {
                round((a[j] as f64 * (n - k) as f64 + b[j] as f64 * k as f64) / n as f64)
            })
        })
        .collect()
}

pub(super) struct Sections(Vec<Vec<Vertex>>);
impl Sections {
    pub fn new(edges: &[[Vertex; 2]], bounded: bool) -> Result<Self> {
        let counts: Vec<_> = edges
            .windows(2)
            .map(|w| strip_count([w[0][0], w[1][0], w[1][1], w[0][1]]))
            .collect();
        let mut counts: Vec<_> = (0..edges.len())
            .map(|i| {
                if i == 0 || i + 1 == edges.len() {
                    1
                } else {
                    counts[i - 1].max(counts[i])
                }
            })
            .collect();
        loop {
            cancellation::checkpoint()?;
            let mut sections = Self(
                edges
                    .iter()
                    .zip(&counts)
                    .map(|(&e, &n)| section(e, n))
                    .collect(),
            );
            if !bounded {
                return Ok(sections);
            }
            let mut refine = std::collections::BTreeSet::new();
            for i in 0..edges.len() - 1 {
                if sections.error(i, edges) > 1.0 + 1e-9 {
                    for j in [i, i + 1] {
                        if j > 0 && j + 1 < edges.len() && counts[j] < 32 {
                            refine.insert(j);
                        }
                    }
                    if ![i, i + 1].iter().any(|j| refine.contains(j)) {
                        return Err(error(
                            "E_TRACK_SURFACE",
                            "road diagonal exceeds one centimetre at lateral budget",
                        ));
                    }
                }
            }
            if refine.is_empty() {
                // The full-quad estimate is conservative, especially at a width
                // transition. Keep an added section vertex only when removing
                // it would violate the measured bound in one of its neighbours.
                for i in 1..edges.len() - 1 {
                    cancellation::checkpoint()?;
                    while counts[i] > 1 {
                        let previous =
                            std::mem::replace(&mut sections.0[i], section(edges[i], counts[i] - 1));
                        if sections.error(i - 1, edges) > 1.0 + 1e-9
                            || sections.error(i, edges) > 1.0 + 1e-9
                        {
                            sections.0[i] = previous;
                            break;
                        }
                        counts[i] -= 1;
                    }
                }
                return Ok(sections);
            }
            // Only the endpoints of an actually failing span change. Their
            // neighbours are checked again because they share these vertices.
            for i in refine {
                counts[i] += 1;
            }
        }
    }
    pub fn triangles(&self, segment: usize) -> Vec<[Vertex; 3]> {
        let a = &self.0[segment];
        let b = &self.0[segment + 1];
        let (na, nb) = (a.len() - 1, b.len() - 1);
        let (mut i, mut j) = (0, 0);
        let mut out = Vec::with_capacity(na + nb);
        while i < na || j < nb {
            if i < na && j < nb && (2 * i + 1) * nb == (2 * j + 1) * na {
                out.extend(quad([a[i], b[j], b[j + 1], a[i + 1]]));
                i += 1;
                j += 1;
            } else if j == nb || (i < na && (2 * i + 1) * nb < (2 * j + 1) * na) {
                out.push([a[i], b[j], a[i + 1]]);
                i += 1;
            } else {
                out.push([a[i], b[j], b[j + 1]]);
                j += 1;
            }
        }
        out
    }
    fn error(&self, segment: usize, edges: &[[Vertex; 2]]) -> f64 {
        let a = &self.0[segment];
        let b = &self.0[segment + 1];
        let e = [
            edges[segment][0],
            edges[segment + 1][0],
            edges[segment + 1][1],
            edges[segment][1],
        ];
        let mut worst: f64 = 0.0;
        for face in self.triangles(segment) {
            let uv = face.map(|v| {
                if let Some(i) = a.iter().position(|&p| p == v) {
                    [0.0, i as f64 / (a.len() - 1) as f64]
                } else {
                    [
                        1.0,
                        b.iter().position(|&p| p == v).unwrap() as f64 / (b.len() - 1) as f64,
                    ]
                }
            });
            let n = normal(face);
            let value = |t: f64, u: f64| {
                (0..3)
                    .map(|j| {
                        n[j] * (e[0][j] as f64 * (1.0 - t) * (1.0 - u)
                            + e[1][j] as f64 * t * (1.0 - u)
                            + e[2][j] as f64 * t * u
                            + e[3][j] as f64 * (1.0 - t) * u
                            - face[0][j] as f64)
                    })
                    .sum::<f64>()
            };
            // A bilinear patch minus a plane has no strict interior extremum.
            // Its three parameter edges are quadratic: check their vertices too.
            for k in 0..3 {
                let [t, u] = uv[k];
                let [dt, du] = [uv[(k + 1) % 3][0] - t, uv[(k + 1) % 3][1] - u];
                let c = value(t, u);
                let end = value(t + dt, u + du);
                let mid = value(t + dt * 0.5, u + du * 0.5);
                let aa = 2.0 * (end + c - 2.0 * mid);
                let bb = end - c - aa;
                worst = worst.max(c.abs()).max(end.abs());
                if aa.abs() > 1e-12 {
                    let s = -bb / (2.0 * aa);
                    if s > 0.0 && s < 1.0 {
                        worst = worst.max(value(t + dt * s, u + du * s).abs());
                    }
                }
            }
        }
        worst
    }
    pub fn scratch_bytes(samples: usize) -> u64 {
        (samples
            * (33 * std::mem::size_of::<Vertex>()
                + std::mem::size_of::<Vec<Vertex>>()
                + 2 * std::mem::size_of::<i64>()
                + 64) // refinement set nodes, in addition to both count arrays
            + 64 * std::mem::size_of::<[Vertex; 3]>()) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_ribbon_diagonal_error_and_local_triangle_budget() {
        let mut worst: f64 = 0.0;
        for degrees in [90, 180, 360] {
            for side in ["left", "right"] {
                for direction in ["up", "down"] {
                    let id = format!("spiral{degrees}_{side}_{direction}");
                    for &width in WIDTHS {
                        for taper in [false, true] {
                            let mut p = variant(
                                &id,
                                width,
                                if taper { 200 } else { width },
                                if taper { 1200 } else { width },
                            );
                            p.rotation_mdeg = [0, 37000, 0];
                            p = materialize(&p);
                            let edges: Vec<_> = p
                                .path
                                .iter()
                                .map(|s| geometry::ribbon_edges(s, 0))
                                .collect();
                            let sections = Sections::new(&edges, true).unwrap();
                            for segment in 0..edges.len() - 1 {
                                let a = &sections.0[segment];
                                let b = &sections.0[segment + 1];
                                let faces = sections.triangles(segment);
                                assert_eq!(
                                    faces.len(),
                                    a.len() + b.len() - 2,
                                    "no hidden uniform refinement"
                                );
                                assert!(faces.len() <= 64, "existing maximum lateral budget");
                                for face in faces {
                                    let params = face.map(|v| {
                                        if let Some(i) = a.iter().position(|&x| x == v) {
                                            [0.0, i as f64 / (a.len() - 1) as f64]
                                        } else {
                                            [
                                                1.0,
                                                b.iter().position(|&x| x == v).unwrap() as f64
                                                    / (b.len() - 1) as f64,
                                            ]
                                        }
                                    });
                                    let n = normal(face);
                                    for i in 0..=8 {
                                        for j in 0..=8 - i {
                                            let weights = [
                                                i as f64 / 8.0,
                                                j as f64 / 8.0,
                                                (8 - i - j) as f64 / 8.0,
                                            ];
                                            let t = (0..3)
                                                .map(|k| params[k][0] * weights[k])
                                                .sum::<f64>();
                                            let u = (0..3)
                                                .map(|k| params[k][1] * weights[k])
                                                .sum::<f64>();
                                            let at = |e: [Vertex; 2], axis: usize| {
                                                e[0][axis] as f64 * (1.0 - u)
                                                    + e[1][axis] as f64 * u
                                            };
                                            let delta = std::array::from_fn::<_, 3, _>(|axis| {
                                                at(edges[segment], axis) * (1.0 - t)
                                                    + at(edges[segment + 1], axis) * t
                                                    - face[0][axis] as f64
                                            });
                                            let error = (0..3)
                                                .map(|axis| delta[axis] * n[axis])
                                                .sum::<f64>()
                                                .abs();
                                            worst = worst.max(error);
                                            assert!(error<=1.000001,"{id} width={width} taper={taper} segment={segment}: {error}cm diagonal error");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("ROAD_DIAGONAL worst_cm={worst}");
    }
}
