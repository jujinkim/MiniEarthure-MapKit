//! Bounded presentation paths from the shared road plan; no authored fields or
//! serialized generated-format additions. All distances are centimetres.
use crate::{
    road_plan::{corner_radius, fillet, hit, length, plan},
    *,
};

#[derive(Clone, Debug)]
pub struct RoadPaintSegment {
    pub a: Vertex,
    pub b: Vertex,
    pub width_cm: f64,
    pub station_cm: f64,
    pub period_cm: f64,
    pub total_cm: f64,
}
#[derive(Default, Debug)]
pub struct RoadPaint {
    pub paths: BTreeMap<String, Vec<RoadPaintSegment>>,
    pub edges: BTreeMap<String, Vec<[Vertex; 2]>>,
}

impl MapDocument {
    pub fn road_paint(&self, bounds: &Bounds) -> Result<RoadPaint> {
        if self.roads.iter().any(|r| {
            r.points.len() < 2
                || r.widths_cm.len() + 1 != r.points.len()
                || r.surfaces.len() != r.widths_cm.len()
                || r.points.windows(2).any(|s| length(s[0], s[1]) == 0.0)
        }) {
            return Err(error(
                "E_GEOMETRY",
                "road paint requires valid segment arrays and nonzero lengths",
            ));
        }
        let mut result = RoadPaint::default();
        let mut nodes: BTreeMap<&str, Vec<(&Road, bool)>> = BTreeMap::new();
        for r in &self.roads {
            nodes.entry(&r.from).or_default().push((r, true));
            nodes.entry(&r.to).or_default().push((r, false));
        }
        let (_, edges) = plan(self, bounds)?;
        for edge in edges {
            if edge.road.markings.as_ref().is_some_and(|m| m.edge_lines)
                && hit(&[edge.a, edge.b], bounds, 20)
            {
                result
                    .edges
                    .entry(edge.road.id.clone())
                    .or_default()
                    .push([edge.a, edge.b]);
            }
        }
        for r in self.roads.iter().filter(|r| r.markings.is_some()) {
            crate::cancellation::checkpoint()?;
            let mut points = r.points.clone();
            let mut widths = r.widths_cm.clone();
            let mut connected = [false; 2];
            for (end, node) in [&r.from, &r.to].into_iter().enumerate() {
                let arms = &nodes[node.as_str()];
                if arms.len() == 2 {
                    if let Some(&(other, from)) = arms.iter().find(|(other, _)| other.id != r.id) {
                        let p = if from {
                            other.points[1]
                        } else {
                            other.points[other.points.len() - 2]
                        };
                        let w = if from {
                            other.widths_cm[0]
                        } else {
                            *other.widths_cm.last().unwrap()
                        };
                        if end == 0 {
                            points.insert(0, p);
                            widths.insert(0, w);
                        } else {
                            points.push(p);
                            widths.push(w);
                        }
                        connected[end] = true;
                    }
                } else if arms.len() > 2 {
                    let radius = arms
                        .iter()
                        .map(|(a, from)| {
                            if *from {
                                a.widths_cm[0]
                            } else {
                                *a.widths_cm.last().unwrap()
                            }
                        })
                        .max()
                        .unwrap() as f64
                        / 2.0;
                    let i = if end == 0 { 0 } else { points.len() - 1 };
                    let j = if end == 0 { 1 } else { i - 1 };
                    let t = (radius + 200.0).min(length(points[i], points[j]) * 0.45)
                        / length(points[i], points[j]);
                    points[i] = std::array::from_fn(|k| {
                        points[i][k] + libm::round((points[j][k] - points[i][k]) as f64 * t) as i64
                    });
                }
            }
            let mut path = Vec::<(Vertex, f64)>::new();
            for i in 0..points.len() {
                if i == 0 || i + 1 == points.len() {
                    if (i == 0 && connected[0]) || (i + 1 == points.len() && connected[1]) {
                        continue;
                    }
                    path.push((points[i], widths[i.min(widths.len() - 1)] as f64));
                    continue;
                }
                let w = widths[i - 1].max(widths[i]);
                // Radius includes half the carriageway so parallel lane offsets
                // never turn inside out. Exterior/curb radii remain 15% of width.
                let arc = fillet(
                    points[i - 1],
                    points[i],
                    points[i + 1],
                    w as f64 / 2.0 + corner_radius(w),
                );
                let half = (arc.len() - 1) / 2;
                for (j, p) in arc.iter().enumerate() {
                    if connected[0] && i == 1 && j < half {
                        continue;
                    }
                    if connected[1] && i + 2 == points.len() && j > half {
                        continue;
                    }
                    let t = if arc.len() > 1 {
                        j as f64 / (arc.len() - 1) as f64
                    } else {
                        0.5
                    };
                    path.push((*p, widths[i - 1] as f64 * (1.0 - t) + widths[i] as f64 * t));
                }
            }
            let total: f64 = path.windows(2).map(|s| length(s[0].0, s[1].0)).sum();
            // Fit complete dash periods between graph nodes. Both halves of a
            // degree-two join have phase zero; cells never restart the pattern.
            let period = if total > 0.0 {
                total / libm::round(total / 240.0).max(1.0)
            } else {
                240.0
            };
            let mut station = 0.0;
            for s in path.windows(2) {
                let len = length(s[0].0, s[1].0);
                if len > 0.0 && hit(&[s[0].0, s[1].0], bounds, widest(r) / 2 + 100) {
                    result
                        .paths
                        .entry(r.id.clone())
                        .or_default()
                        .push(RoadPaintSegment {
                            a: s[0].0,
                            b: s[1].0,
                            width_cm: (s[0].1 + s[1].1) / 2.0,
                            station_cm: station,
                            period_cm: period,
                            total_cm: total,
                        });
                }
                station += len;
            }
        }
        if result.paths.values().any(|v| v.len() > 128)
            || result.edges.values().any(|v| v.len() > 128)
        {
            return Err(error(
                "E_BUDGET",
                "road paint exceeds 128 local path/exterior segments",
            ));
        }
        Ok(result)
    }
}
fn widest(r: &Road) -> i64 {
    r.widths_cm.iter().copied().max().unwrap_or(0) as i64
}
