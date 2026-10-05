//! Quantized, width-aware ribbons. All consumers use these frames.
use super::*;
use crate::curve_sampling::{self as sampling, norm, cross, Probe};

/// Evaluate unrounded centre and frame, select bounded intervals, then quantize
/// once. All mesh/collision/wall/occupancy consumers receive these same samples.
fn analytical(eval: impl Fn(f64)->([f64;3],[f64;3]), breaks: &[f64], width: u32, entry: u32, exit: u32, mode: &str) -> Vec<Sample> {
    analytical_raw(eval,breaks,width,entry,exit,mode).into_iter().map(|(s,_,_)|s).collect()
}
fn analytical_raw(eval: impl Fn(f64)->([f64;3],[f64;3]), breaks: &[f64], width: u32, entry: u32, exit: u32, mode: &str) -> Vec<(Sample,[f64;3],f64)> {
    analytical_placed(eval,breaks,width,entry,exit,mode,[0;3],[0;3])
}
fn analytical_placed(eval: impl Fn(f64)->([f64;3],[f64;3]), breaks: &[f64], width: u32, entry: u32, exit: u32, mode: &str, rotation: [i32;3], origin: Vertex) -> Vec<(Sample,[f64;3],f64)> {
    let length: f64 = (0..64).map(|i| {
        let a=eval(i as f64/64.0).0; let b=eval((i+1) as f64/64.0).0;
        sampling::length(std::array::from_fn(|j|b[j]-a[j]))
    }).sum();
    let transition=(f64::from(width.abs_diff(entry).max(width.abs_diff(exit)))*2.0).max(300.0).min(length/2.0).max(1.0);
    let half_width=|t:f64| {
        let ease=|x:f64| {let x=x.clamp(0.0,1.0); x*x*(3.0-2.0*x)};
        (width as f64+(entry as f64-width as f64)*(1.0-ease(t*length/transition))
            +(exit as f64-width as f64)*(1.0-ease((1.0-t)*length/transition)))*0.5
    };
    let frame=|t| {
        let (p,f)=eval(t);let f=norm(f);
        let mut side=norm([f[2],0.0,-f[0]]);
        if sampling::length(side)<0.5 {side=[1.0,0.0,0.0];}
        let n=norm(cross(f,side));
        (p,f,n,side)
    };
    let probe=|t| {
        let (p,f,n,side)=frame(t); let w=half_width(t);
        Probe {points:[p,
            std::array::from_fn(|j|p[j]-side[j]*w),std::array::from_fn(|j|p[j]+side[j]*w),
            std::array::from_fn(|j|p[j]-side[j]*(w+WALL_THICKNESS_CM as f64)+n[j]*WALL as f64),
            std::array::from_fn(|j|p[j]+side[j]*(w+WALL_THICKNESS_CM as f64)+n[j]*WALL as f64)],tangent:f,normal:n}
    };
    let tolerance=if mode=="spiral" {0.6} else {0.75};
    let angle=if mode=="spiral" {3.5f64.to_radians()} else {4.0f64.to_radians()};
    let ts=if ["cylinder","halfpipe","loop"].contains(&mode) {
        sampling::parameters(probe,breaks,150.0,tolerance,angle)
    } else {
        sampling::ordinary_parameters(probe,breaks,tolerance,angle)
    };
    let original=ts;
    let mut ts: Vec<_>=original.iter().copied().map(|mut t| {
        if mode=="spiral" && t>0.125 && t<0.875 {
            // Choose stations on integer height contours of the original helix.
            // Rounding arbitrary short spans can turn an analytic 20.8% inner
            // grade into >23%. Move the station, never resample rounded points.
            let target=round(eval(t).0[1]) as f64;
            let increasing=eval(1.0).0[1]>eval(0.0).0[1];
            let (mut lo,mut hi)=(0.0,1.0);
            for _ in 0..40 {
                let mid=(lo+hi)*0.5;
                if (eval(mid).0[1]<target)==increasing {lo=mid;} else {hi=mid;}
            }
            t=(lo+hi)*0.5;
        }
        t
    }).collect();
    // A height contour is optional, the existing approximation limits are not.
    // Reject a move that invalidates either neighbour instead of doubling a
    // nearly-maximal span merely to retain that rounding optimization.
    if mode=="spiral" {
        loop {
            let mut rejected=vec![];
            for (i,w) in ts.windows(2).enumerate() {
                if w[0]>=w[1] || sampling::ordinary_parameters(probe,w,tolerance,angle).len()>2 {
                    for j in [i,i+1] {if ts[j]!=original[j] {rejected.push(j);}}
                }
            }
            if rejected.is_empty() {break;}
            for j in rejected {ts[j]=original[j];}
        }
    }
    ts.into_iter().map(|t| {
        let (p,f,n,side)=frame(t);
        let p=rotate_float(p,rotation);let p=std::array::from_fn(|j|p[j]+origin[j] as f64);
        let f=rotate_float(f,rotation);let n=rotate_float(n,rotation);let side=rotate_float(side,rotation);
        let mut s=sample(p.map(round),f,width,mode);
        s.ribbon_cm=Some([-1.0,1.0].map(|sign|std::array::from_fn(|j|round(p[j]+side[j]*half_width(t)*sign))));
        if mode=="spiral" && (t==0.0 || t==1.0) {
            // A snapped neighbour is positioned at the integer port centre.
            // Canonical port offsets must use that centre on both pieces.
            s.ribbon_cm=Some([-1.0,1.0].map(|sign|std::array::from_fn(|j|s.position_cm[j]+round(side[j]*half_width(t)*sign))));
        }
        s.normal=unit(n);s.lateral_cm=round(half_width(t)) as u32;(s,p,half_width(t))
    }).collect()
}

pub(super) fn rotate3(v: Vertex, rotation: [i32; 3]) -> Vertex {
    rotate_float(v.map(|v|v as f64),rotation).map(round)
}
fn rotate_float(mut p: [f64;3], rotation: [i32;3]) -> [f64;3] {
    // Euler YXZ, matching the public gimmick placement convention.
    for (axis, a, b) in [(2, 0, 1), (0, 1, 2), (1, 2, 0)] {
        let t = f64::from(rotation[axis]) * std::f64::consts::PI / 180000.0;
        let (s, c) = (libm::sin(t), libm::cos(t));
        let (x, y) = (p[a], p[b]);
        p[a] = c * x - s * y;
        p[b] = s * x + c * y;
    }
    p
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
        ribbon_cm: None,
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
        path[i].ribbon_cm = None;
    }
}
pub(super) fn bezier(points: &[Vertex], width: u32, entry: u32, exit: u32) -> Vec<Sample> {
    let mut path = vec![];
    for (index,cp) in points.windows(4).step_by(3).enumerate() {
        let part=analytical_raw(|t| {
            let u=1.0-t;
            let pos=std::array::from_fn(|j|u*u*u*cp[0][j] as f64+3.0*u*u*t*cp[1][j] as f64+3.0*u*t*t*cp[2][j] as f64+t*t*t*cp[3][j] as f64);
            let tangent=std::array::from_fn(|j|3.0*u*u*(cp[1][j]-cp[0][j]) as f64+6.0*u*t*(cp[2][j]-cp[1][j]) as f64+3.0*t*t*(cp[3][j]-cp[2][j]) as f64);
            (pos,tangent)
        }, &[0.0,1.0],width,if index==0 {entry} else {width},if index*3+4==points.len() {exit} else {width},"drive");
        path.extend(part.into_iter().skip(if index==0 {0} else {1}));
    }
    let mut samples: Vec<_>=path.iter().map(|(s,_,_)|s.clone()).collect();
    curve_frames(&mut samples);
    for (s,(_,position,width)) in samples.iter_mut().zip(path) {
        let side=norm(cross(s.normal.map(|v|v as f64/1e6),s.forward.map(|v|v as f64/1e6)));
        s.ribbon_cm=Some([-1.0,1.0].map(|sign|std::array::from_fn(|j|round(position[j]+side[j]*width*sign))));
    }
    samples
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

/// Subdivide twisted quads across their width until diagonal ridge error is
/// below one centimetre. Flat ribbons need no extra lateral triangles.
#[cfg(test)]
pub(super) fn strips(a: &Sample, b: &Sample) -> i64 {
    let [al,ar]=ribbon_edges(a,0);let [bl,br]=ribbon_edges(b,0);
    road::strip_count([al,bl,br,ar])
}
pub(super) fn ramp(length:f64,rise:f64,width:u32,entry:u32,exit:u32)->Vec<Sample> {
    analytical(|t|([0.0,rise*spiral_rise(t),length*t],[0.0,rise*spiral_pitch(t),length]),
        &[0.0,0.125,0.875,1.0],width,entry,exit,"drive")
}
fn pipe_path(id: &str, width: u32) -> Vec<Sample> {
    let mode=if id=="banked_chicane" {"halfpipe"} else {"cylinder"};
    let mut origin=[0.0;3];let mut yaw:f64=0.0;let mut path=vec![];
    let mut segment=|distance:f64,turn:f64| {
        let (sn,cs)=(libm::sin(yaw),libm::cos(yaw));
        let at=|t:f64| {
            let (x,z,dx,dz)=if turn==0.0 {(0.0,distance*t,0.0,distance)} else {
                let a=turn.abs()*t;let sign=turn.signum();
                (sign*400.0*(1.0-libm::cos(a)),400.0*libm::sin(a),sign*400.0*libm::sin(a)*turn.abs(),400.0*libm::cos(a)*turn.abs())
            };
            let pos=[origin[0]+cs*x+sn*z,0.0,origin[2]-sn*x+cs*z];
            let mut f=[cs*dx+sn*dz,0.0,-sn*dx+cs*dz];let mut p=pos;
            if id.ends_with("_s_rise") {
                let a=std::f64::consts::PI*p[2]/SLOT as f64;
                p[1]=80.0*libm::sin(a).powi(2);
                f[1]=80.0*std::f64::consts::PI/SLOT as f64*libm::sin(2.0*a)*f[2];
            }
            (p,f)
        };
        let end=at(1.0).0;
        let mut part=analytical(at,&[0.0,1.0],width,width,width,mode);
        for s in &mut part {s.tube_radius_cm=width/2;s.safe=false;s.above_cm=250;s.below_cm=50;}
        let skip=usize::from(!path.is_empty());path.extend(part.into_iter().skip(skip));
        origin=end;origin[1]=0.0;yaw+=turn;
    };
    let quarter=std::f64::consts::FRAC_PI_2;
    if id.contains("curve") || id.contains("uturn") {
        segment(400.0,0.0);
        segment(0.0,quarter*if id.contains("uturn") {2.0} else {1.0}*if id.ends_with("left") {-1.0} else {1.0});
        segment(400.0,0.0);
    } else {
        segment(800.0,0.0);
        for sign in [-1.0,1.0,1.0,-1.0] {segment(0.0,sign*quarter);}
        segment(800.0,0.0);
    }
    path
}
fn spiral_curve(p: &Piece) -> impl Fn(f64)->([f64;3],[f64;3]) {
    let degrees=if p.id.contains("90_") {90.0} else if p.id.contains("180_") {180.0} else {360.0};
    let width=p.width_cm.max(p.entry_width_cm).max(p.exit_width_cm);
    let radius=800.max(width/2+700) as f64;
    let rise=degrees/360.0*800.0*if p.id.ends_with("down") {-1.0} else {1.0};
    let sign=if p.id.contains("left") {-1.0} else {1.0};
    let radians=degrees*std::f64::consts::PI/180.0;
    move |t| {
        let a=t*radians;
        ([sign*radius*(1.0-libm::cos(a)),rise*spiral_rise(t),radius*libm::sin(a)],
         [sign*libm::sin(a),rise*spiral_pitch(t)/(radius*radians),libm::cos(a)])
    }
}
pub(super) fn spiral_path(p: &Piece, rotation: [i32;3], origin: Vertex) -> Vec<Sample> {
    analytical_placed(spiral_curve(p), &[0.0,0.125,0.875,1.0],p.width_cm,p.entry_width_cm,p.exit_width_cm,"spiral",rotation,origin)
        .into_iter().map(|(s,_,_)|s).collect()
}
pub(super) fn shape(p: &mut Piece, width: u32) -> bool {
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
    if id.starts_with("spiral") {
        p.path=spiral_path(p,[0;3],[0;3]);
    } else if angle > 0.0 {
        let radius = if id.starts_with("gentle") || id == "curve" || id == "curve_left" || grade_curve {
            1600.max(width * 4)
        } else if id.starts_with("right") || id.starts_with("sharp_curve") {
            400.max(width / 2 + 200)
        } else {300.max(width / 2 + 100)};
        let rise = if grade_curve {if id.ends_with("down") {-100.0} else {100.0}} else {0.0};
        let radians = angle * std::f64::consts::PI / 180.0;
        p.path = analytical(|t| {
            let a = t * radians;
            let dy = rise * spiral_pitch(t) / (radius as f64 * radians);
            ([sign * radius as f64 * (1.0-libm::cos(a)), rise*spiral_rise(t), radius as f64*libm::sin(a)],
             [sign*libm::sin(a),dy,libm::cos(a)])
        }, &[0.0,0.125,0.875,1.0], p.width_cm,p.entry_width_cm,p.exit_width_cm,"drift");
    } else if id == "zigzag" || id == "chicane" || id.ends_with("_narrow") {
        let length = if id.starts_with("straight") {
            1600.0
        } else {
            6400.0
        };
        // Bound the second derivative of sin(4πt)·sin²(πt). Its magnitude
        // is below 300; keeping radius beyond the outer edge prevents a wide
        // ribbon from folding back over itself at a wave crest.
        let amplitude = if id == "straight_narrow" { 0.0 } else {
            (width as f64 * 1.5).min(length*length/(300.0*(width as f64/2.0+100.0)))
        };
        p.path = analytical(|t| {
            let pi = std::f64::consts::PI;
            let a = t*pi*4.0;
            let sn = libm::sin(pi*t);
            let dx = amplitude*(4.0*pi*libm::cos(a)*sn*sn+pi*libm::sin(a)*libm::sin(2.0*pi*t));
            ([amplitude*libm::sin(a)*sn*sn,0.0,length*t], [dx,0.0,length])
        }, &[0.0,0.25,0.5,0.75,1.0],p.width_cm,p.entry_width_cm,p.exit_width_cm,"drift");
    } else if ["slope_up","slope_down"].contains(&id) {
        let rise = if id.ends_with("down") {-100.0} else {100.0};
        p.path = analytical(|t| ([0.0,rise*spiral_rise(t),800.0*t],[0.0,rise*spiral_pitch(t),800.0]),
            &[0.0,0.125,0.875,1.0],p.width_cm,p.entry_width_cm,p.exit_width_cm,"drive");
    } else if id == "slope" {
        p.path = analytical(|t| {
            let (u,sign) = if t<0.5 {(t*2.0,1.0)} else {(2.0-t*2.0,-1.0)};
            ([0.0,62.5*spiral_rise(u),800.0*t],[0.0,125.0*spiral_pitch(u)*sign,800.0])
        }, &[0.0,0.0625,0.4375,0.5,0.5625,0.9375,1.0],p.width_cm,p.entry_width_cm,p.exit_width_cm,"drive");
    } else if id.starts_with("cylinder") || id=="banked_chicane" {
        p.path=pipe_path(id,p.width_cm);
    } else {
        return false;
    }
    p.reference_msec = (race_length(p) * 1000 / SPEED as u64) as u32;
    true
}

// Conservative sample volume, including tilted lane edges and the supported
// vehicle envelope. Tube samples reserve the complete bore and shell.
const ROAD_CLEARANCE_CM: i64 = 30;
pub(super) fn volume(s: &Sample) -> (Vertex, i64, i64, i64) {
    let radius = i64::from(s.tube_radius_cm);
    if radius > 0 {
        let c = add(s.position_cm, s.normal.map(|n| n * radius / 1_000_000));
        return (c, radius + 15, c[1] - radius - 15, c[1] + radius + 15);
    }
    let edge_y = ((s.normal[2] * s.forward[0] - s.normal[0] * s.forward[2]).abs()
        * (i64::from(s.lateral_cm) + WALL_THICKNESS_CM)
        + 999_999_999_999)
        / 1_000_000_000_000;
    let height = if s.mode == "flight" { 120 } else { 235 };
    let up_y = s.normal[1] * height / 1_000_000;
    let side = ((s.normal[0].abs() + s.normal[2].abs()) * height + 999_999) / 1_000_000;
    (
        s.position_cm,
        i64::from(s.lateral_cm) + WALL_THICKNESS_CM + ROAD_CLEARANCE_CM + side,
        s.position_cm[1] - edge_y + up_y.min(0) - 15,
        s.position_cm[1] + edge_y + up_y.max(0),
    )
}
pub(super) fn volume_overlap(a: &Sample, b: &Sample) -> bool {
    let (ac, ar, alo, ahi) = volume(a);
    let (bc, br, blo, bhi) = volume(b);
    alo < bhi && blo < ahi && (ac[0] - bc[0]).pow(2) + (ac[2] - bc[2]).pow(2) < (ar + br).pow(2)
}

/// A conservative finite footprint, only for level, constant-width straight
/// ribbons. All quantized edges are enclosed; the existing wall and vehicle
/// margins extend both the sides and the ends. Other geometry keeps the sample
/// volume check, including banks, tubes, flights, tapers and alternate paths.
fn straight_footprint(p: &Piece) -> Option<[[f64; 2]; 4]> {
    let first = p.path.first()?;
    if p.path.len() < 2 || !p.alternate_path.is_empty()
        || p.width_cm != p.entry_width_cm || p.width_cm != p.exit_width_cm
        || first.forward[1] != 0
    {
        return None;
    }
    let length = libm::hypot(first.forward[0] as f64, first.forward[2] as f64);
    if length < 1.0 { return None; }
    let forward = [first.forward[0] as f64 / length, first.forward[2] as f64 / length];
    let side = [forward[1], -forward[0]];
    let origin = [first.position_cm[0] as f64, first.position_cm[2] as f64];
    let project = |v: Vertex, axis: [f64; 2]| {
        (v[0] as f64 - origin[0]) * axis[0] + (v[2] as f64 - origin[1]) * axis[1]
    };
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut previous = 0.0;
    for s in &p.path {
        let station = project(s.position_cm, forward);
        if !s.safe || s.mode != "drive" || s.tube_radius_cm != 0
            || s.normal != [0, 1_000_000, 0] || s.forward != first.forward
            || s.lateral_cm != first.lateral_cm || s.position_cm[1] != first.position_cm[1]
            // Allow only the quantization error of a straight centreline.
            || project(s.position_cm, side).abs() > 1.0 || station < previous
        {
            return None;
        }
        previous = station;
        for edge in ribbon_edges(s, 0) {
            if edge[1] != first.position_cm[1] { return None; }
            for (i, axis) in [forward, side].into_iter().enumerate() {
                let v = project(edge, axis);
                lo[i] = lo[i].min(v);
                hi[i] = hi[i].max(v);
            }
        }
    }
    if previous <= 0.0 { return None; }
    let margin = (WALL_THICKNESS_CM + ROAD_CLEARANCE_CM) as f64;
    lo = lo.map(|v| v - margin);
    hi = hi.map(|v| v + margin);
    Some([[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]]
        .map(|v| std::array::from_fn(|j| origin[j] + forward[j] * v[0] + side[j] * v[1])))
}

/// Continuous clearance for eligible straight ribbons. The caller retains the
/// shared-port exclusions. Horizontal touching is conservative overlap; vertical
/// clearance keeps the same open intervals as the sample-volume predicate.
/// None leaves unsupported geometry on the existing sampled path.
pub(super) fn straight_overlap(a: &Piece, b: &Piece) -> Option<bool> {
    let (af, bf) = (straight_footprint(a)?, straight_footprint(b)?);
    let (_, _, alo, ahi) = volume(&a.path[0]);
    let (_, _, blo, bhi) = volume(&b.path[0]);
    Some(alo < bhi && blo < ahi && !footprints_separated(af, bf))
}

fn footprints_separated(a: [[f64; 2]; 4], b: [[f64; 2]; 4]) -> bool {
    [a, b].iter().any(|corners| {
        corners.windows(2).take(2).any(|edge| {
            let delta = [edge[1][0] - edge[0][0], edge[1][1] - edge[0][1]];
            let length = libm::hypot(delta[0], delta[1]);
            if length < 1.0 { return false; }
            let axis = [-delta[1] / length, delta[0] / length];
            let range = |points: &[[f64; 2]; 4]| points.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    let v = (p[0] - a[0][0]) * axis[0] + (p[1] - a[0][1]) * axis[1];
                    (lo.min(v), hi.max(v))
                });
            let (alo, ahi) = range(&a);
            let (blo, bhi) = range(&b);
            ahi + 1e-6 < blo || bhi + 1e-6 < alo
        })
    })
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
                && distance(a.position_cm, b.position_cm) < width + 2 * WALL_THICKNESS_CM as u64 + 30
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

/// Integer ribbon edges shared by generation and connected-road wall clipping.
/// A small inward offset preserves coincident outer walls at ordinary joins.
pub(super) fn ribbon_edges(s: &Sample, inset: u32) -> [Vertex;2] {
    if inset==0 {if let Some(edges)=s.ribbon_cm {return edges;}}
    let basis=basis(s);
    let right=std::array::from_fn::<_,3,_>(|j|round(basis[j][0]*1e6));
    [-1,1].map(|side| std::array::from_fn(|j|
        s.position_cm[j]+round(right[j] as f64*f64::from(s.lateral_cm.saturating_sub(inset))*side as f64/1e6)))
}

pub(super) fn ordinary_grade_valid(p: &Piece) -> bool {
    if !(p.id.starts_with("spiral") || ["slope","slope_up","slope_down","curve_up","curve_down","curve_left_up","curve_left_down"].contains(&p.id.as_str())) {return true;}
    p.path.windows(2).all(|w| {
        let a=ribbon_edges(&w[0],0);let b=ribbon_edges(&w[1],0);
        [(a[0],b[0]),(w[0].position_cm,w[1].position_cm),(a[1],b[1])].into_iter().all(|(a,b)| {
            let dx=(b[0]-a[0]) as f64;let dz=(b[2]-a[2]) as f64;
            (b[1]-a[1]).abs() as f64 <= 0.23*libm::sqrt(dx*dx+dz*dz)+1e-9
        })
    })
}

#[cfg(test)]
mod straight_clearance_tests {
    use super::*;
    #[test]
    fn uncertain_shapes_keep_broad_phase_and_actual_edges_bound_footprint() {
        let road = |id, start, points| {
            let mut i = authoring::instance(id, "free_curve", 800);
            i.position_cm = start;
            i.control_points = points;
            authoring::piece(&i).unwrap()
        };
        let a = road("a", [0, 0, -3000], vec![[0, 0, 0], [0, 0, 1000], [0, 0, 2000], [0, 0, 3000]]);
        let b = road("b", [600, 0, 600], vec![[0, 0, 0], [800, 0, 0], [1600, 0, 0], [2400, 0, 0]]);
        assert!(volume_overlap(a.path.last().unwrap(), &b.path[0]));
        assert_eq!(straight_overlap(&a, &b), Some(false));
        assert_eq!(straight_overlap(&b, &a), Some(false));
        for case in 0..11 {
            let mut uncertain = a.clone();
            match case {
                0 => uncertain.path[1].normal = [0, 999999, 1000],
                1 => uncertain.path[1].forward = [1000, 0, 999999],
                2 => uncertain.path[1].lateral_cm += 1,
                3 => uncertain.path[1].position_cm[1] += 1,
                4 => uncertain.path[1].tube_radius_cm = 400,
                5 => uncertain.path[1].mode = "flight".into(),
                6 => uncertain.alternate_path = uncertain.path.clone(),
                7 => uncertain.path[1].position_cm[0] += 10,
                8 => uncertain.entry_width_cm = 400,
                9 => uncertain.path[1].safe = false,
                _ => uncertain.path[1].position_cm[2] = -3100,
            }
            assert_eq!(straight_overlap(&uncertain, &b), None, "fallback case {case}");
            assert_eq!(straight_overlap(&b, &uncertain), None, "reverse fallback case {case}");
            assert!(authoring::conflict(&uncertain, &b), "sample collision retained: {case}");
        }
        let mut edges = a;
        // Bounds must enclose the delivered ribbon, not just width metadata.
        edges.path.last_mut().unwrap().ribbon_cm = Some([[-600, 0, 200], [600, 0, 200]]);
        assert_eq!(straight_overlap(&edges, &b), Some(true));
    }
}

#[cfg(test)]
mod playtest_surface_tests {
    use super::*;
    #[test]
    fn export_unquantized_spiral_reference_when_requested() {
        let Ok(directory)=std::env::var("SPIRAL_REFERENCE_DIR") else {return};
        std::fs::create_dir_all(&directory).unwrap();
        for degrees in [90,180,360] {for side in ["left","right"] {for direction in ["up","down"] {
            let id=format!("spiral{degrees}_{side}_{direction}");let p=variant(&id,400,400,400);
            let eval=spiral_curve(&p);let mut vertices=vec![];
            let at=|i:usize,j:usize| {let (p,f)=eval(i as f64/1024.0);let side=norm([f[2],0.0,-f[0]]);std::array::from_fn::<_,3,_>(|k|p[k]+side[k]*(-200.0+j as f64*25.0))};
            for i in 0..1024 {for j in 0..16 {
                let [a,b,c,d]=[at(i,j),at(i+1,j),at(i+1,j+1),at(i,j+1)];vertices.extend([a,b,c,a,c,d]);
            }}
            std::fs::write(std::path::Path::new(&directory).join(format!("{id}.json")),serde_json::to_vec(&vertices).unwrap()).unwrap();
        }}}
    }
    #[test]
    fn spiral_moved_stations_preserve_bounds_and_mirrored_paths() {
        for degrees in [90,180,360] {for direction in ["up","down"] {for &width in WIDTHS {for taper in [false,true] {
            let shape=|side:&str|variant(&format!("spiral{degrees}_{side}_{direction}"),width,if taper {200} else {width},if taper {1200} else {width});
            let p=shape("right");let left=shape("left");
            assert!(ordinary_grade_valid(&p),"{} {width}/{taper}: grade",p.id);
            assert_eq!(p.path.len(),left.path.len());
            for (r,l) in p.path.iter().zip(&left.path) {assert_eq!(r.position_cm,[-l.position_cm[0],l.position_cm[1],l.position_cm[2]]);}
            let angle=degrees as f64*std::f64::consts::PI/180.0;
            let parameter=|i:usize|if i+1==p.path.len() {1.0} else {
                let f=p.path[i].forward;let mut a=libm::atan2(f[0] as f64,f[2] as f64);if a<0.0 {a+=std::f64::consts::TAU;}a/angle
            };
            let rise=degrees as f64/360.0*800.0*if direction=="down" {-1.0} else {1.0};
            for (i,w) in p.path.windows(2).enumerate() {
                let (ta,tb)=(parameter(i),parameter(i+1));assert!(tb>ta,"strict station order");
                assert!(distance(w[0].position_cm,w[1].position_cm)<=252);
                for (a,b) in [(w[0].normal,w[1].normal)] {
                    let dot=(0..3).map(|j|a[j] as f64*b[j] as f64/1e12).sum::<f64>();
                    assert!(libm::acos(dot.clamp(-1.0,1.0)).to_degrees()<3.51,"sample frame bound");
                }
                assert!((tb-ta)*degrees as f64<=3.5*5.0/3.0+0.001,"heading bound");
                for quarter in 0..=4 {
                    let t=quarter as f64/4.0;
                    let expected=rise*spiral_rise(ta+(tb-ta)*t);
                    let chord=w[0].position_cm[1] as f64*(1.0-t)+w[1].position_cm[1] as f64*t;
                    assert!((expected-chord).abs()<=1.1001,"{} width={width} taper={taper}: analytic .6cm plus .5cm quantization",p.id);
                }
            }
        }}}}
    }
    #[test]
    fn final_edges_grade_frames_spacing_and_planar_triangles() {
        for id in ["slope","slope_up","slope_down","curve_up","curve_left_down","spiral90_left_up","spiral180_right_down","spiral_up","spiral_down","gentle45","hairpin"] {
            for width in WIDTHS {
                let p=materialize(&variant(id,*width,*width,*width));
                assert!(ordinary_grade_valid(&p),"{id} w={width}: inner/centre/outer grade exceeds 23%: {:?}",p.path.windows(2).filter(|w| {
                    let a=ribbon_edges(&w[0],0);let b=ribbon_edges(&w[1],0);
                    (0..2).any(|i| {let dx=(b[i][0]-a[i][0]) as f64;let dz=(b[i][2]-a[i][2]) as f64;(b[i][1]-a[i][1]).abs() as f64>0.23*libm::sqrt(dx*dx+dz*dz)})
                }).map(|w|(ribbon_edges(&w[0],0),ribbon_edges(&w[1],0))).collect::<Vec<_>>());
                for pair in p.path.windows(2) {
                    assert!(distance(pair[0].position_cm,pair[1].position_cm)<=252,"{id}: spacing");
                    for vectors in [[pair[0].normal,pair[1].normal]] {
                        let dot=(0..3).map(|j|vectors[0][j] as f64*vectors[1][j] as f64/1e12).sum::<f64>().clamp(-1.0,1.0);
                        assert!(libm::acos(dot).to_degrees()<=4.01,"{id}: frame turn");
                    }
                    let heading=|v: Vertex| libm::atan2(v[0] as f64,v[2] as f64);
                    let delta=heading(pair[1].forward)-heading(pair[0].forward);
                    assert!(libm::atan2(libm::sin(delta),libm::cos(delta)).abs().to_degrees()<=6.68,"{id}: horizontal turn");
                    if ["slope","slope_up","slope_down","gentle45","hairpin"].contains(&id) {
                        assert_eq!(strips(&pair[0],&pair[1]),1,"planar trapezoids need two triangles");
                    }
                }
                assert_eq!(p,materialize(&p),"deterministic final quantization");
            }
        }
    }
    #[test]
    fn ordinary_grade_quantization() {
        for id in ["curve_up","curve_down","curve_left_up","curve_left_down","spiral_up","spiral_down"] {
            let p=materialize(&variant(id,600,600,600));
            let mut error: f64=0.0;
            let mut shortest: f64=f64::MAX;
            for w in p.path.windows(2) {
                let d=std::array::from_fn::<_,3,_>(|j|(w[1].position_cm[j]-w[0].position_cm[j]) as f64);
                let horizontal=libm::sqrt(d[0]*d[0]+d[2]*d[2]);
                if horizontal<1.0 {continue;}
                shortest=shortest.min(horizontal);
                let f=std::array::from_fn::<_,3,_>(|j|(w[0].forward[j]+w[1].forward[j]) as f64/2e6);
                let intended=f[1]/libm::sqrt(f[0]*f[0]+f[2]*f[2]);
                error=error.max((d[1]/horizontal-intended).abs());
            }
            println!("SURFACE {id} samples={} shortest_cm={shortest:.3} max_grade_error={error:.5}",p.path.len());
            assert!(error<0.04,"integer road chord must stay within four percentage points of its authored grade");
            assert_eq!(p.path.first().unwrap().position_cm,[0,0,0]);
            assert_eq!(p.path.first().unwrap().normal,[0,1_000_000,0]);
            assert_eq!(p.path.last().unwrap().normal,[0,1_000_000,0]);
        }
    }
    #[test]
    fn wave_ribbon_edges_never_fold_backwards() {
        for id in ["zigzag","chicane","zigzag_narrow","chicane_narrow","straight_narrow"] {
            for width in [200,400,600,800,1200] {
                let p=materialize(&variant(id,width,width,width));
                for w in p.path.windows(2) {
                    let a=ribbon_edges(&w[0],0);let b=ribbon_edges(&w[1],0);
                    for side in 0..2 {
                        let progress: i128=(0..3).map(|j|i128::from(b[side][j]-a[side][j])*i128::from(w[0].forward[j]+w[1].forward[j])).sum();
                        assert!(progress>=0,"{id} width={width}: folded edge {:?} -> {:?}",a[side],b[side]);
                    }
                }
                if id=="straight_narrow" {assert!(p.path.iter().all(|s|s.position_cm[0]==0));}
            }
        }
    }

}
