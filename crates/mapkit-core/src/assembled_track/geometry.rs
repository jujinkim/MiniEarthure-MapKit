//! Quantized, width-aware ribbons. All consumers use these frames.
use super::*;

pub(super) fn rotate3(v: Vertex, rotation: [i32; 3]) -> Vertex {
    let mut p = v.map(|v| v as f64);
    // Euler YXZ, matching the public gimmick placement convention.
    for (axis, a, b) in [(2, 0, 1), (0, 1, 2), (1, 2, 0)] {
        let t = f64::from(rotation[axis]) * std::f64::consts::PI / 180000.0;
        let (s, c) = (libm::sin(t), libm::cos(t));
        let (x, y) = (p[a], p[b]);
        p[a] = c * x - s * y;
        p[b] = s * x + c * y;
    }
    p.map(round)
}
fn sample(position: Vertex, tangent: [f64; 3], width: u32, mode: &str) -> Sample {
    let f = unit(tangent);
    let normal = unit([
        -tangent[0] * tangent[1],
        tangent[0] * tangent[0] + tangent[2] * tangent[2],
        -tangent[2] * tangent[1],
    ]);
    Sample {
        position_cm: position,
        forward: f,
        normal,
        mode: mode.into(),
        safe: mode != "flight",
        min_speed_cmps: 0,
        lateral_cm: width / 2,
        tube_radius_cm: 0,
        below_cm: 150,
        above_cm: 300,
    }
}
pub(super) fn taper(path: &mut [Sample], width: u32, entry: u32, exit: u32) {
    let length: f64 = path
        .windows(2)
        .map(|w| distance(w[0].position_cm, w[1].position_cm) as f64)
        .sum();
    let transition = (f64::from(width.abs_diff(entry).max(width.abs_diff(exit))) * 2.0)
        .max(300.0)
        .min(length / 2.0)
        .max(1.0);
    let mut station = 0.0;
    for i in 0..path.len() {
        if i > 0 {
            station += distance(path[i - 1].position_cm, path[i].position_cm) as f64;
        }
        let ease = |t: f64| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let w = f64::from(width)
            + (f64::from(entry) - f64::from(width)) * (1.0 - ease(station / transition))
            + (f64::from(exit) - f64::from(width)) * (1.0 - ease((length - station) / transition));
        path[i].lateral_cm = round(w / 2.0) as u32;
    }
}
pub(super) fn bezier(points: &[Vertex], width: u32, entry: u32, exit: u32) -> Vec<Sample> {
    let mut path = vec![];
    for cp in points.windows(4).step_by(3) {
        let length: u64 = cp.windows(2).map(|w| distance(w[0], w[1])).sum();
        let steps = (length / 40).clamp(8, 1024);
        for i in 0..=steps {
            if i == 0 && !path.is_empty() {
                continue;
            }
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            let pos = std::array::from_fn(|j| {
                round(
                    u * u * u * cp[0][j] as f64
                        + 3.0 * u * u * t * cp[1][j] as f64
                        + 3.0 * u * t * t * cp[2][j] as f64
                        + t * t * t * cp[3][j] as f64,
                )
            });
            let tangent = std::array::from_fn(|j| {
                3.0 * u * u * (cp[1][j] - cp[0][j]) as f64
                    + 6.0 * u * t * (cp[2][j] - cp[1][j]) as f64
                    + 3.0 * t * t * (cp[3][j] - cp[2][j]) as f64
            });
            path.push(sample(pos, tangent, width, "drive"));
        }
    }
    curve_frames(&mut path);
    taper(&mut path, width, entry, exit);
    path
}
// Parallel transport through vertical tangents; distribute endpoint roll so ports
// agree with the gravity frame without an abrupt twist at the final sample.
fn curve_frames(path: &mut [Sample]) {
    if path.is_empty() {
        return;
    }
    let dot = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let frame = |v: Vertex| v.map(|v| v as f64 / 1e6);
    let mut normal = [0.0, 1.0, 0.0];
    for s in path.iter_mut() {
        let f = frame(s.forward);
        let projection = dot(normal, f);
        let mut n = std::array::from_fn(|j| normal[j] - projection * f[j]);
        if dot(n, n) < 1e-8 {
            n = cross(f, [1.0, 0.0, 0.0]);
        }
        if dot(n, n) < 1e-8 {
            n = cross(f, [0.0, 0.0, 1.0]);
        }
        s.normal = unit(n);
        normal = frame(s.normal);
    }
    let last = path.last().unwrap();
    let f = frame(last.forward);
    let target = unit([-f[0] * f[1], f[0] * f[0] + f[2] * f[2], -f[2] * f[1]]);
    if target == [0; 3] {
        return;
    }
    let angle = libm::atan2(
        dot(cross(normal, frame(target)), f),
        dot(normal, frame(target)),
    );
    let count = (path.len() - 1).max(1) as f64;
    for (i, s) in path.iter_mut().enumerate() {
        let t = i as f64 / count;
        let a = angle * t * t * (3.0 - 2.0 * t);
        let n = frame(s.normal);
        let r = cross(frame(s.forward), n);
        s.normal = unit(std::array::from_fn(|j| {
            n[j] * libm::cos(a) + r[j] * libm::sin(a)
        }));
    }
}
pub(super) fn shape(p: &mut Piece, width: u32) {
    let id = p.id.as_str();
    let left = id.contains("left");
    let sign = if left { -1.0 } else { 1.0 };
    let grade_curve = id.starts_with("curve") && (id.ends_with("up") || id.ends_with("down"));
    let angle = if id.starts_with("gentle45") {
        45.0
    } else if id.starts_with("sharp135") {
        135.0
    } else if id.starts_with("hairpin") {
        180.0
    } else if grade_curve
        || id.starts_with("gentle90")
        || id.starts_with("right90")
        || ["curve", "curve_left", "sharp_curve", "sharp_curve_left"].contains(&id)
    {
        90.0
    } else {
        0.0
    };
    let spiral = id.starts_with("spiral");
    if angle > 0.0 || spiral {
        let degrees = if spiral {
            if id.contains("90_") {
                90.0
            } else if id.contains("180_") {
                180.0
            } else {
                360.0
            }
        } else {
            angle
        };
        let radius = if spiral {
            800.max(width / 2 + 200)
        } else if id.starts_with("gentle") || id == "curve" || id == "curve_left" || grade_curve {
            1600.max(width * 4)
        } else if id.starts_with("right") || id.starts_with("sharp_curve") {
            400.max(width / 2 + 200)
        } else {
            300.max(width / 2 + 100)
        };
        let rise = if spiral {
            degrees / 360.0 * 800.0 * if id.ends_with("down") { -1.0 } else { 1.0 }
        } else if grade_curve {
            if id.ends_with("down") {
                -100.0
            } else {
                100.0
            }
        } else {
            0.0
        };
        let radians = degrees * std::f64::consts::PI / 180.0;
        let steps = ((radius as f64 * radians / 35.0).ceil() as usize).max(32);
        p.path = (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                let a = t * radians;
                let dy = rise * spiral_pitch(t) / (radius as f64 * radians);
                sample(
                    [
                        round(sign * radius as f64 * (1.0 - libm::cos(a))),
                        round(rise * spiral_rise(t)),
                        round(radius as f64 * libm::sin(a)),
                    ],
                    [sign * libm::sin(a), dy, libm::cos(a)],
                    width,
                    if spiral { "spiral" } else { "drift" },
                )
            })
            .collect();
    } else if id == "zigzag" || id == "chicane" || id.ends_with("_narrow") {
        let length = if id.starts_with("straight") {
            1600.0
        } else {
            6400.0
        };
        let amplitude = width as f64 * 1.5;
        p.path = (0..=256)
            .map(|i| {
                let t = i as f64 / 256.0;
                let a = t * std::f64::consts::TAU * 2.0;
                sample(
                    [
                        round(
                            amplitude * libm::sin(a) * libm::sin(std::f64::consts::PI * t).powi(2),
                        ),
                        0,
                        round(length * t),
                    ],
                    [0.0, 0.0, 1.0],
                    width,
                    "drift",
                )
            })
            .collect();
        for i in 1..p.path.len() - 1 {
            p.path[i].forward = unit(std::array::from_fn(|j| {
                (p.path[i + 1].position_cm[j] - p.path[i - 1].position_cm[j]) as f64
            }));
        }
    }
    p.reference_msec = (race_length(p) * 1000 / SPEED as u64) as u32;
}

// Conservative sample volume, including tilted lane edges and the supported
// vehicle envelope. Tube samples reserve the complete bore and shell.
pub(super) fn volume(s: &Sample) -> (Vertex, i64, i64, i64) {
    let radius = i64::from(s.tube_radius_cm);
    if radius > 0 {
        let c = add(s.position_cm, s.normal.map(|n| n * radius / 1_000_000));
        return (c, radius + 15, c[1] - radius - 15, c[1] + radius + 15);
    }
    let edge_y = ((s.normal[2] * s.forward[0] - s.normal[0] * s.forward[2]).abs()
        * i64::from(s.lateral_cm)
        + 999_999_999_999)
        / 1_000_000_000_000;
    let height = if s.mode == "flight" { 120 } else { 235 };
    let up_y = s.normal[1] * height / 1_000_000;
    let side = ((s.normal[0].abs() + s.normal[2].abs()) * height + 999_999) / 1_000_000;
    (
        s.position_cm,
        i64::from(s.lateral_cm) + 30 + side,
        s.position_cm[1] - edge_y + up_y.min(0) - 15,
        s.position_cm[1] + edge_y + up_y.max(0),
    )
}
pub(super) fn volume_overlap(a: &Sample, b: &Sample) -> bool {
    let (ac, ar, alo, ahi) = volume(a);
    let (bc, br, blo, bhi) = volume(b);
    alo < bhi && blo < ahi && (ac[0] - bc[0]).pow(2) + (ac[2] - bc[2]).pow(2) < (ar + br).pow(2)
}

pub(super) fn self_intersects(p: &Piece) -> bool {
    let mut stations = vec![0u64];
    for w in p.path.windows(2) {
        stations.push(stations.last().unwrap() + distance(w[0].position_cm, w[1].position_cm));
    }
    for i in (0..p.path.len()).step_by(3) {
        for j in (0..i).step_by(3) {
            let a = &p.path[i];
            let b = &p.path[j];
            let width = u64::from(a.lateral_cm + b.lateral_cm);
            if stations[i] - stations[j] < width * 2 + 100 {
                continue;
            }
            if (a.position_cm[1] - b.position_cm[1]).abs() < 235
                && distance(a.position_cm, b.position_cm) < width + 30
            {
                return true;
            }
        }
    }
    false
}

pub(super) fn basis(s: &Sample) -> [[f64; 3]; 3] {
    let n = s.normal.map(|v| v as f64 / 1e6);
    let f = s.forward.map(|v| v as f64 / 1e6);
    let r = unit([
        n[1] * f[2] - n[2] * f[1],
        n[2] * f[0] - n[0] * f[2],
        n[0] * f[1] - n[1] * f[0],
    ])
    .map(|v| v as f64 / 1e6);
    std::array::from_fn(|i| [r[i], n[i], f[i]])
}
pub(super) fn euler(m: [[f64; 3]; 3]) -> [i32; 3] {
    let pitch = -libm::asin(m[1][2].clamp(-1.0, 1.0));
    let (yaw, roll) = if libm::cos(pitch).abs() < 1e-6 {
        (libm::atan2(-m[2][0], m[0][0]), 0.0)
    } else {
        (libm::atan2(m[0][2], m[2][2]), libm::atan2(m[1][0], m[1][1]))
    };
    [pitch, yaw, roll].map(|a| round(a * 180000.0 / std::f64::consts::PI) as i32)
}
