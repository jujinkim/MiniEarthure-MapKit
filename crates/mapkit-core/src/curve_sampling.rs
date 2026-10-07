//! Deterministic, error-driven sampling before any coordinate quantization.
pub(crate) type V3 = [f64; 3];
pub(crate) fn norm(v: V3) -> V3 {
    let length = libm::sqrt(v.iter().map(|v| v*v).sum::<f64>()).max(1e-12);
    v.map(|v| v/length)
}
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
}
pub(crate) fn length(v: V3) -> f64 { libm::sqrt(v.iter().map(|v| v*v).sum()) }
pub(crate) struct Probe { pub points: [V3; 5], pub tangent: V3, pub normal: V3 }
/// Bounds centre, edges and shell as well as tangent/frame changes. Quarter
/// probes detect closed curves and inflections that a midpoint alone misses.
/// Subdivision count follows measured error, angle and distance (not bisection).
pub(crate) fn parameters(eval: impl Fn(f64) -> Probe, breaks: &[f64], spacing: f64, tolerance: f64, angle: f64) -> Vec<f64> {
    fn ratio(eval: &impl Fn(f64) -> Probe, a: f64, b: f64, spacing: f64, tolerance: f64, angle: f64) -> f64 {
        let samples: Vec<_> = (0..=4).map(|i|eval(a+(b-a)*i as f64/4.0)).collect();
        let mut error: f64 = 0.0;
        let mut distance: f64 = 0.0;
        let mut turn: f64 = 0.0;
        for k in 0..5 {
            let mut span = 0.0;
            for i in 1..=4 {
                span += length(std::array::from_fn(|j|samples[i].points[k][j]-samples[i-1].points[k][j]));
                let t = i as f64/4.0;
                error = error.max(length(std::array::from_fn(|j|samples[i].points[k][j]-(samples[0].points[k][j]*(1.0-t)+samples[4].points[k][j]*t))));
            }
            distance = distance.max(span);
        }
        if distance < 1e-9 { return 1.0; } // Caller rejects a stationary tangent.
        for field in [false,true] {
            let mut sum = 0.0;
            for pair in samples.windows(2) {
                let [a,b] = [0,1].map(|i|norm(if field {pair[i].normal} else {pair[i].tangent}));
                sum += libm::acos((0..3).map(|j|a[j]*b[j]).sum::<f64>().clamp(-1.0,1.0));
            }
            turn = turn.max(sum);
        }
        (distance/spacing).max(turn/angle).max(libm::sqrt(error/tolerance))
    }
    refine(breaks, |a,b| ratio(&eval,a,b,spacing,tolerance,angle))
}

/// Ordinary ribbons relax only longitudinal/horizontal approximation. Height,
/// pitch and the complete surface normal retain their previous limits.
/// Units are centimetres: horizontal error <=8cm, yaw <=12deg, span <=400cm.
pub(crate) fn ordinary_parameters(eval: impl Fn(f64) -> Probe, breaks: &[f64], tolerance: f64, angle: f64) -> Vec<f64> {
    refine(breaks, |a,b| {
        let samples: Vec<_> = (0..=4).map(|i| eval(a+(b-a)*i as f64/4.0)).collect();
        let mut ratio: f64 = 1.0;
        for k in 0..5 {
            let mut distance = 0.0;
            for i in 1..=4 {
                distance += length(std::array::from_fn(|j| samples[i].points[k][j]-samples[i-1].points[k][j]));
                let t = i as f64/4.0;
                let error: V3 = std::array::from_fn(|j| samples[i].points[k][j]-(samples[0].points[k][j]*(1.0-t)+samples[4].points[k][j]*t));
                ratio = ratio.max(libm::sqrt(libm::hypot(error[0],error[2])/8.0))
                    .max(libm::sqrt(error[1].abs()/tolerance));
            }
            ratio = ratio.max(distance/400.0);
        }
        let mut yaw = 0.0; let mut pitch = 0.0; let mut normal = 0.0;
        for pair in samples.windows(2) {
            let [a,b] = [0,1].map(|i| norm(pair[i].tangent));
            let heading = |v: V3| libm::atan2(v[0],v[2]);
            let elevation = |v: V3| libm::atan2(v[1],libm::hypot(v[0],v[2]));
            let delta = heading(b)-heading(a);
            yaw += libm::atan2(libm::sin(delta),libm::cos(delta)).abs();
            pitch += (elevation(b)-elevation(a)).abs();
            let [a,b] = [0,1].map(|i| norm(pair[i].normal));
            normal += libm::acos((0..3).map(|j| a[j]*b[j]).sum::<f64>().clamp(-1.0,1.0));
        }
        ratio.max(yaw/12.0f64.to_radians()).max(pitch/angle).max(normal/angle)
    })
}

fn refine(breaks: &[f64], ratio: impl Fn(f64,f64) -> f64) -> Vec<f64> {
    let mut out = vec![breaks[0]];
    for pair in breaks.windows(2) {
        let (a,b)=(pair[0],pair[1]);
        let mut count=ratio(a,b).ceil().clamp(1.0,32768.0) as usize;
        loop {
            let worst=(0..count).map(|i|ratio(a+(b-a)*i as f64/count as f64,a+(b-a)*(i+1) as f64/count as f64)).fold(1.0,f64::max);
            if worst <= 1.000001 || count>=32768 {break;}
            count=((count as f64*worst).ceil() as usize).max(count+1).min(32768);
        }
        out.extend((1..=count).map(|i|a+(b-a)*i as f64/count as f64));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn probe(t: f64, rise: f64) -> Probe {
        let a=t*std::f64::consts::FRAC_PI_2;
        let p=[1600.0*(1.0-libm::cos(a)),rise*t*t,1600.0*libm::sin(a)];
        let tangent=norm([1600.0*libm::sin(a)*std::f64::consts::FRAC_PI_2,2.0*rise*t,1600.0*libm::cos(a)*std::f64::consts::FRAC_PI_2]);
        let side=norm([tangent[2],0.0,-tangent[0]]);
        Probe { points: [-600.0,-300.0,0.0,300.0,600.0].map(|w|std::array::from_fn(|j|p[j]+w*side[j])), tangent, normal:norm(cross(tangent,side)) }
    }
    #[test]
    fn horizontal_relaxation_keeps_height_pitch_and_normal_bounds() {
        let angle=4.0f64.to_radians();
        for rise in [0.0,100.0,1200.0] {
            let eval=|t|probe(t,rise);
            let old=parameters(eval,&[0.0,1.0],150.0,0.75,angle);
            let new=ordinary_parameters(eval,&[0.0,0.37,1.0],0.75,angle);
            assert!(new.contains(&0.37), "authored branch boundary is fixed");
            if rise==0.0 { assert!((new.len()-1) as f64 <= (old.len()-1) as f64*0.65); }
            for w in new.windows(2) {
                let a=eval(w[0]);let b=eval(w[1]);
                let pitch=|v: V3|libm::atan2(v[1],libm::hypot(v[0],v[2]));
                assert!((pitch(a.tangent)-pitch(b.tangent)).abs()<=angle+1e-8);
                let dot=(0..3).map(|j|a.normal[j]*b.normal[j]).sum::<f64>().clamp(-1.0,1.0);
                assert!(libm::acos(dot)<=angle+1e-8);
                for i in 1..4 {
                    let u=i as f64/4.0;let p=eval(w[0]+(w[1]-w[0])*u);
                    for k in 0..5 {
                        let e: V3=std::array::from_fn(|j|p.points[k][j]-a.points[k][j]*(1.0-u)-b.points[k][j]*u);
                        assert!(e[1].abs()<=0.750001);
                        assert!(libm::hypot(e[0],e[2])<=8.0+1e-6);
                    }
                }
            }
            assert_eq!(new.first(),Some(&0.0));assert_eq!(new.last(),Some(&1.0));
            assert_eq!(new,ordinary_parameters(eval,&[0.0,0.37,1.0],0.75,angle));
        }
    }
}
