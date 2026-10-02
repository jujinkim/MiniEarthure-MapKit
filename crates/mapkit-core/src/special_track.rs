//! Bounded parametric ribbons. All consumers use these quantized tenth-millimetre faces.
use crate::*;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Loop,
    Cylinder,
    SweptCylinder,
    SweptHalfPipe,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TubeFrame {
    pub floor_cm: Vertex,
    pub normal: Vertex,
    pub forward: Vertex,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecialTrack {
    pub kind: TrackKind,
    pub radius_cm: u32,
    pub width_cm: u32,
    pub length_cm: u32,
    /// Current v1 optional geometry; populated only for a swept cylinder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub centerline: Vec<TubeFrame>,
}
#[derive(Debug, Clone, Serialize)]
pub struct TrackMesh {
    pub units_per_metre: u32,
    pub inner: Vec<[Vertex; 3]>,
    pub shell: Vec<[Vertex; 3]>,
    #[serde(skip)]
    pub tiles: Vec<(Vertex, Vertex)>,
}
const STEPS: usize = 256;
pub(crate) const fn loop_offset_cm(width_cm: u32) -> i64 {
    // Preserve lane width while leaving clearance for both 50 cm barriers.
    width_cm as i64 * 65 / 100 + crate::assembled_track::WALL_THICKNESS_CM
}

impl SpecialTrack {
    pub fn valid(&self) -> bool {
        if matches!(
            self.kind,
            TrackKind::SweptCylinder | TrackKind::SweptHalfPipe
        ) {
            return (100..=600).contains(&self.radius_cm)
                && (140..=600).contains(&self.width_cm)
                && (2..=512).contains(&self.centerline.len())
                && self.centerline.iter().all(|f| {
                    f.floor_cm.iter().all(|v| v.unsigned_abs() <= 6400)
                        && [f.normal, f.forward].iter().all(|v| {
                            let norm = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
                            (0.999e12..=1.001e12).contains(&norm)
                        })
                        && (0..3)
                            .map(|j| f.normal[j] as f64 * f.forward[j] as f64)
                            .sum::<f64>()
                            .abs()
                            < 2e9
                })
                && self.centerline.windows(2).all(|w| {
                    let ds = (0..3)
                        .map(|j| (w[1].floor_cm[j] - w[0].floor_cm[j]) as f64)
                        .map(|v| v * v)
                        .sum::<f64>()
                        .sqrt();
                    ds >= 1.0 && ds <= 160.0
                });
        }
        self.centerline.is_empty()
            && (150..=600).contains(&self.radius_cm)
            && (140..=600).contains(&self.width_cm)
            && (600..=3200).contains(&self.length_cm)
    }
    fn bands(&self) -> usize {
        if self.kind == TrackKind::Loop {
            1
        } else if self.radius_cm * 2 > self.length_cm {
            64
        } else if self.radius_cm * 4 > self.length_cm {
            32
        } else {
            16
        }
    }
    pub fn tile_count(&self) -> usize {
        if matches!(
            self.kind,
            TrackKind::SweptCylinder | TrackKind::SweptHalfPipe
        ) {
            128 * self.centerline.len().saturating_sub(1)
        } else {
            STEPS * self.bands()
        }
    }
    pub fn bound_radius(&self) -> i64 {
        match self.kind {
            TrackKind::SweptCylinder | TrackKind::SweptHalfPipe => {
                self.centerline
                    .iter()
                    .map(|f| f.floor_cm.iter().map(|v| v.abs()).sum::<i64>())
                    .max()
                    .unwrap_or(0)
                    + i64::from(self.radius_cm) * 4
                    + 40
            }
            TrackKind::Loop => i64::from(self.radius_cm) * 4 + i64::from(self.width_cm) * 2 + 40,
            TrackKind::Cylinder => {
                i64::from(self.radius_cm) * 4 + i64::from(self.length_cm) / 2 + 40
            }
        }
    }
    pub fn mesh(&self) -> TrackMesh {
        if matches!(
            self.kind,
            TrackKind::SweptCylinder | TrackKind::SweptHalfPipe
        ) {
            return self.swept_mesh();
        }
        let mut mesh = TrackMesh {
            units_per_metre: 10000,
            inner: vec![],
            shell: vec![],
            tiles: vec![],
        };
        let r = f64::from(self.radius_cm);
        let w = f64::from(self.width_cm);
        let l = f64::from(self.length_cm);
        let bands = self.bands();
        let point = |i: usize, j: usize, outer: bool| -> Vertex {
            let t = i as f64 * std::f64::consts::TAU / STEPS as f64;
            let (sn, cs) = (libm::sin(t), libm::cos(t));
            let thickness = if outer { 10.0 } else { 0.0 };
            let p = match self.kind {
                TrackKind::Loop => {
                    // Integrate radius r*(1 + .6*cos(theta)): 1.6r at the floor,
                    // .4r at the crown. Open, separated ends avoid self-overlap.
                    // Smooth lateral separation keeps entrance and exit disjoint.
                    let smooth = |v: f64| {
                        let u = v.clamp(0.0, 1.0);
                        u * u * (3.0 - 2.0 * u)
                    };
                    let shift = -1.0 + smooth((t - 0.85) / 1.60) + smooth((t - 3.83) / 1.60);
                    let x = loop_offset_cm(self.width_cm) as f64 * shift + (j as f64 - 0.5) * w;
                    [
                        x,
                        r * (1.0 - cs) + 0.30 * r * sn * sn - thickness * cs,
                        r * (sn + 0.60 * (t * 0.5 + libm::sin(2.0 * t) * 0.25)) + thickness * sn,
                    ]
                }
                TrackKind::SweptCylinder | TrackKind::SweptHalfPipe => unreachable!(),
                TrackKind::Cylinder => {
                    let u = j as f64 / bands as f64;
                    // Flared entrances leave the bottom tangent to the access road.
                    let flare = 0.12 * r * (1.0 + libm::cos(u * std::f64::consts::TAU)) * 0.5;
                    let radius = r + flare;
                    [
                        (radius + thickness) * sn,
                        radius - (radius + thickness) * cs,
                        (u - 0.5) * l,
                    ]
                }
            };
            p.map(|v| libm::round(v * 100.0) as i64)
        };
        for i in 0..STEPS {
            for j in 0..bands {
                let mut inside = [
                    point(i, j, false),
                    point(i + 1, j, false),
                    point(i + 1, j + 1, false),
                    point(i, j + 1, false),
                ];
                let mut outside = [
                    point(i, j, true),
                    point(i + 1, j, true),
                    point(i + 1, j + 1, true),
                    point(i, j + 1, true),
                ];
                // Cylinder and ribbon parametric axes have opposite handedness.
                if self.kind == TrackKind::Cylinder {
                    inside.reverse();
                    outside.reverse();
                }
                mesh.inner.extend([
                    [inside[0], inside[1], inside[2]],
                    [inside[0], inside[2], inside[3]],
                ]);
                mesh.shell.extend([
                    [outside[2], outside[1], outside[0]],
                    [outside[3], outside[2], outside[0]],
                ]);
                // Only exposed boundaries get sides; no internal contact seams.
                for edge in 0..4 {
                    let exposed = if self.kind == TrackKind::Loop {
                        edge == 0
                            || edge == 2
                            || (edge == 3 && i == 0)
                            || (edge == 1 && i + 1 == STEPS)
                    } else {
                        (edge == 0 && j + 1 == bands) || (edge == 2 && j == 0)
                    };
                    if exposed {
                        let k = (edge + 1) % 4;
                        mesh.shell.extend([
                            [inside[edge], outside[edge], outside[k]],
                            [inside[edge], outside[k], inside[k]],
                        ]);
                    }
                }
                let all = inside.iter().chain(&outside);
                let low = std::array::from_fn(|a| {
                    all.clone().map(|v| v[a].div_euclid(100)).min().unwrap()
                });
                let high = std::array::from_fn(|a| {
                    all.clone()
                        .map(|v| (v[a] + 99).div_euclid(100))
                        .max()
                        .unwrap()
                });
                mesh.tiles.push((low, high));
            }
        }
        mesh
    }
    /// A uniform bore gives directly connected sections identical open rings.
    pub fn swept_radius(&self, _index: usize) -> f64 {
        f64::from(self.radius_cm)
    }
    pub fn contains_swept(&self, point: [f64; 3], margin: f64) -> bool {
        self.centerline.windows(2).enumerate().any(|(i, w)| {
            let center = |f: &TubeFrame, r: f64| {
                std::array::from_fn::<_, 3, _>(|j| {
                    f.floor_cm[j] as f64 + f.normal[j] as f64 / 1e6 * r
                })
            };
            let a = center(&w[0], self.swept_radius(i));
            let b = center(&w[1], self.swept_radius(i + 1));
            let d = std::array::from_fn::<_, 3, _>(|j| b[j] - a[j]);
            let l = d.iter().map(|v| v * v).sum::<f64>();
            let t = ((0..3).map(|j| (point[j] - a[j]) * d[j]).sum::<f64>() / l.max(1.0))
                .clamp(0.0, 1.0);
            (0..3)
                .map(|j| (point[j] - a[j] - d[j] * t).powi(2))
                .sum::<f64>()
                <= (self.swept_radius(i).max(self.swept_radius(i + 1)) + margin).powi(2)
        })
    }
    fn swept_mesh(&self) -> TrackMesh {
        let mut mesh = TrackMesh {
            units_per_metre: 10000,
            inner: vec![],
            shell: vec![],
            tiles: vec![],
        };
        let radii: Vec<_> = (0..self.centerline.len())
            .map(|i| self.swept_radius(i))
            .collect();
        let point = |i: usize, j: usize, outer: bool| {
            let f = &self.centerline[i];
            let n = f.normal.map(|v| v as f64 / 1e6);
            let t = f.forward.map(|v| v as f64 / 1e6);
            let side = [
                n[1] * t[2] - n[2] * t[1],
                n[2] * t[0] - n[0] * t[2],
                n[0] * t[1] - n[1] * t[0],
            ];
            let a = if self.kind == TrackKind::SweptHalfPipe {
                -std::f64::consts::FRAC_PI_2 + j as f64 * std::f64::consts::PI / 128.0
            } else {
                j as f64 * std::f64::consts::TAU / 128.0
            };
            let r = radii[i];
            let shell = r + if outer { 10.0 } else { 0.0 };
            std::array::from_fn::<_, 3, _>(|k| {
                libm::round(
                    (f.floor_cm[k] as f64
                        + n[k] * (r - shell * libm::cos(a))
                        + side[k] * shell * libm::sin(a))
                        * 100.0,
                ) as i64
            })
        };
        for i in 0..self.centerline.len() - 1 {
            for j in 0..128 {
                let inside = [
                    point(i, j, false),
                    point(i + 1, j, false),
                    point(i + 1, j + 1, false),
                    point(i, j + 1, false),
                ];
                let outside = [
                    point(i, j, true),
                    point(i + 1, j, true),
                    point(i + 1, j + 1, true),
                    point(i, j + 1, true),
                ];
                mesh.inner.extend([
                    [inside[0], inside[1], inside[2]],
                    [inside[0], inside[2], inside[3]],
                ]);
                mesh.shell.extend([
                    [outside[2], outside[1], outside[0]],
                    [outside[3], outside[2], outside[0]],
                ]);
                for edge in 0..4 {
                    if (edge == 3 && i == 0)
                        || (edge == 1 && i + 2 == self.centerline.len())
                        || (self.kind == TrackKind::SweptHalfPipe
                            && ((edge == 0 && j == 0) || (edge == 2 && j == 127)))
                    {
                        let k = (edge + 1) % 4;
                        mesh.shell.extend([
                            [inside[edge], outside[edge], outside[k]],
                            [inside[edge], outside[k], inside[k]],
                        ]);
                    }
                }
                let all = inside.iter().chain(&outside);
                mesh.tiles.push((
                    std::array::from_fn(|a| {
                        all.clone().map(|v| v[a].div_euclid(100)).min().unwrap()
                    }),
                    std::array::from_fn(|a| {
                        all.clone()
                            .map(|v| (v[a] + 99).div_euclid(100))
                            .max()
                            .unwrap()
                    }),
                ));
            }
        }
        mesh
    }
}
