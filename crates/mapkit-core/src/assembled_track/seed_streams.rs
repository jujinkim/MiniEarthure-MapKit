//! Independent seed streams. Environment, style, layout attempts and obstacles
//! never share draws; `derive` gives every (attempt, step, retry) its own stream.
use super::*;

pub(super) const STYLE: u64 = 0x7374_796c_6576_3031;
pub(super) const MOTIF: u64 = 0x6d6f_7469_6676_3031;
pub(super) const PICK: u64 = 0x7069_636b_7376_3031;
pub(super) const OBSTACLE: u64 = 0x636f_7572_7365_3031;

pub fn track_seed_defaults(seed: u64) -> Result<serde_json::Value> {
    if seed > 9_007_199_254_740_991 {
        return Err(error(
            "E_TRACK_SETTINGS",
            "seed exceeds the exact integer range",
        ));
    }
    let mut rng = seed ^ 0x656e_7669_726f_6e31;
    let minutes = [360, 540, 720, 1020, 1080, 1260][draw(&mut rng, 6) as usize];
    let weather = ["clear", "cloudy", "rain", "snow"][draw(&mut rng, 4) as usize];
    Ok(serde_json::json!({"time_minutes":minutes,"weather":weather}))
}

pub(super) fn next(rng: &mut u64) -> u64 {
    *rng = rng.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *rng;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

// Rejection sampling also keeps the six clock choices unbiased.
pub(super) fn draw(rng: &mut u64, count: u64) -> u64 {
    let limit = u64::MAX - u64::MAX % count;
    loop {
        let value = next(rng);
        if value < limit {
            return value % count;
        }
    }
}
/// Uniform in [0, 1) from the top 53 bits.
pub(super) fn uniform(rng: &mut u64) -> f64 {
    (next(rng) >> 11) as f64 / (1u64 << 53) as f64
}
/// A stream that depends only on its coordinates, never on how many draws or
/// retries happened before it.
pub(super) fn derive(seed: u64, salt: u64, attempt: u64, step: u64, retry: u64) -> u64 {
    [attempt, step, retry].iter().fold(seed ^ salt, |r, v| {
        let mut r = r ^ v.wrapping_mul(0xd1b5_4a32_d192_ed03);
        next(&mut r)
    })
}
/// Obstacle spacing and the final recovery share, fixed per seed for every attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ObstaclePlan {
    pub spacing: u64,
    /// Sites in the last 1/`recovery` of the route reference time stay clear.
    pub recovery: u64,
}
pub(super) fn obstacle_plan(seed: u64) -> ObstaclePlan {
    let mut rng = seed ^ OBSTACLE;
    ObstaclePlan { spacing: 1600 + draw(&mut rng, 113) * 100, recovery: 4 + draw(&mut rng, 3) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_defaults_are_frozen_and_independent() {
        for (seed, minutes, weather) in [(0, 540, "snow"), (1, 720, "snow"), (2, 1080, "clear"), (3, 1260, "rain"),
            (7, 360, "rain"), (42, 1020, "cloudy"), (1000, 360, "clear"), (9_007_199_254_740_991, 1020, "cloudy")] {
            assert_eq!(track_seed_defaults(seed).unwrap(), serde_json::json!({"time_minutes":minutes,"weather":weather}), "{seed}");
        }
        let mut clocks = [0; 6];
        let mut weather = [0; 4];
        for seed in 0..2400 {
            let defaults = track_seed_defaults(seed).unwrap();
            clocks[[360, 540, 720, 1020, 1080, 1260].iter().position(|v| Some(*v) == defaults["time_minutes"].as_u64()).unwrap()] += 1;
            weather[["clear", "cloudy", "rain", "snow"].iter().position(|v| *v == defaults["weather"]).unwrap()] += 1;
            assert_eq!(track_seed_defaults(seed).unwrap(), defaults);
            assert_eq!(obstacle_plan(seed), obstacle_plan(seed));
        }
        assert!(clocks.iter().all(|n| (330..470).contains(n)), "{clocks:?}");
        assert!(weather.iter().all(|n| (520..680).contains(n)), "{weather:?}");
        assert!(track_seed_defaults(9_007_199_254_740_992).is_err());
    }
    #[test]
    fn derived_streams_depend_only_on_their_coordinates() {
        let a = derive(5, MOTIF, 1, 2, 3);
        assert_eq!(a, derive(5, MOTIF, 1, 2, 3));
        let others = [derive(5, MOTIF, 1, 2, 4), derive(5, MOTIF, 1, 3, 3), derive(5, MOTIF, 2, 2, 3), derive(6, MOTIF, 1, 2, 3), derive(5, PICK, 1, 2, 3)];
        assert!(others.iter().all(|v| *v != a));
        let plans: std::collections::BTreeSet<_> = (0..64).map(|s| obstacle_plan(s).spacing).collect();
        assert!(plans.len() > 20 && plans.iter().all(|v| (1600..=12800).contains(v)));
    }
}
