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
    let mut out = vec![breaks[0]];
    for pair in breaks.windows(2) {
        let (a,b)=(pair[0],pair[1]);
        let mut count=ratio(&eval,a,b,spacing,tolerance,angle).ceil().clamp(1.0,32768.0) as usize;
        loop {
            let worst=(0..count).map(|i|ratio(&eval,a+(b-a)*i as f64/count as f64,a+(b-a)*(i+1) as f64/count as f64,spacing,tolerance,angle)).fold(1.0,f64::max);
            if worst <= 1.000001 || count>=32768 {break;}
            count=((count as f64*worst).ceil() as usize).max(count+1).min(32768);
        }
        out.extend((1..=count).map(|i|a+(b-a)*i as f64/count as f64));
    }
    out
}
