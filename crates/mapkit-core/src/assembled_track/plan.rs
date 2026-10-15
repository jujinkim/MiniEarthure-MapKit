//! Independent seed streams for environment and an immutable course/section plan.
//! These are geometric preferences, never a difficulty tier or rating.
use super::*;

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

// Rejection sampling also keeps the six clock choices unbiased.
fn draw(rng: &mut u64, count: u64) -> u64 {
    let limit = u64::MAX - u64::MAX % count;
    loop {
        let value = next(rng);
        if value < limit {
            return value % count;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Section {
    pub families: [u32; 13],
    pub categories: [u32; 3],
    pub widths: [u32; 5],
    pub left: bool,
    pub up: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub sections: Vec<Section>,
    pub obstacle_spacing: u64,
    pub shortcut: bool,
}
impl Plan {
    pub fn new(seed: u64) -> Self {
        let mut rng = seed ^ 0x636f_7572_7365_3031;
        // The seed varies shape and order within the shared density contract,
        // without a combined difficulty grade or a disabled special category.
        let straight = 8 + draw(&mut rng, 21) as u32;
        let turn_draw = draw(&mut rng, 35) as u32;
        let turn = turn_draw * turn_draw / 35;
        let elevation = (draw(&mut rng, 32) as u32).saturating_sub(16);
        let gimmick = 20 + draw(&mut rng, 21) as u32;
        let action = 20 + draw(&mut rng, 21) as u32;
        let width = draw(&mut rng, 4) as usize + 1;
        let obstacle_spacing = 1600 + draw(&mut rng, 113) * 100;
        let shortcut = draw(&mut rng, 4) == 0;
        let count = 4 + draw(&mut rng, 3) as usize;
        let mut sections = Vec::new();
        for _ in 0..count {
            // Pick the flow for a whole section before selecting any pieces.
            // A recovery straight, corner sequence or action sequence persists
            // across every layout attempt for this seed.
            let flow = draw(&mut rng, 3);
            let mut widths = [1, 5, 18, 18, 8];
            widths[width] += 50;
            sections.push(Section {
                families: [
                    straight,
                    elevation,
                    turn,
                    turn / 2,
                    turn / 3,
                    turn / 4,
                    turn / 4,
                    elevation,
                    elevation / 2,
                    elevation / 3,
                    turn / 3,
                    elevation,
                    3 + turn / 3,
                ],
                categories: [
                    30,
                    gimmick * if flow == 2 { 3 } else { 1 },
                    action * if flow == 2 { 3 } else { 1 },
                ],
                widths,
                left: draw(&mut rng, 2) == 0,
                up: draw(&mut rng, 2) == 0,
            });
        }
        Self {
            sections,
            obstacle_spacing,
            shortcut,
        }
    }
    pub fn section(&self, elapsed: u32, target: u32) -> &Section {
        &self.sections[(elapsed as usize * self.sections.len() / target as usize)
            .min(self.sections.len() - 1)]
    }
    /// Candidate fit, not a track rating. Compare the actual sequence with its
    /// own section plan; time feasibility sorts ahead of this residual.
    pub fn fit(&self, a: &Assembly) -> u64 {
        let target = u32::from(a.settings.duration_seconds) * 1000;
        let mut elapsed = 0;
        let mut actual = vec![[0u64; 3]; self.sections.len()];
        let mut straight = vec![0u64; self.sections.len()];
        let mut driving = vec![0u64; self.sections.len()];
        for &index in &a.routes[0].pieces {
            let p = &a.pieces[index];
            let i = (elapsed as usize * self.sections.len() / target as usize)
                .min(self.sections.len() - 1);
            elapsed += p.reference_msec;
            let c = match category(&p.id) {
                "gimmick" => 1,
                "action" => 2,
                _ => 0,
            };
            actual[i][c] += race_length(p);
            if c == 0 {
                driving[i] += race_length(p);
                if base_preset(&p.id) == "straight" || p.id == "sprint_lane" {
                    straight[i] += race_length(p);
                }
            }
        }
        let mut residual = 0;
        for (i, section) in self.sections.iter().enumerate() {
            let weights: Vec<u64> = section
                .categories
                .iter()
                .enumerate()
                .map(|(c, w)| {
                    if a.settings
                        .categories
                        .iter()
                        .any(|v| v == selection_ids()[c])
                    {
                        u64::from(*w)
                    } else {
                        0
                    }
                })
                .collect();
            let sum = weights.iter().sum::<u64>().max(1);
            let n = actual[i].iter().sum::<u64>().max(1);
            for (c, w) in weights.iter().enumerate() {
                residual += (actual[i][c] * 100 / n).abs_diff(w * 100 / sum);
            }
            if driving[i] > 0 {
                residual += (straight[i] * 100 / driving[i]).abs_diff(
                    u64::from(section.families[0]) * 100
                        / u64::from(section.families.iter().sum::<u32>()),
                );
            }
        }
        residual / self.sections.len() as u64
    }
    pub fn rank(&self, a: &Assembly, required: &composition::Required) -> (bool, u64) {
        let target = u32::from(a.settings.duration_seconds) * 1000;
        let delta = a.estimated_msec.abs_diff(target);
        let quality = composition::Composition::assembly(a, required);
        (
            delta > target / 10 || !quality.valid(&a.settings),
            quality.penalty(&a.settings) * 1000 / quality.total()
                + u64::from(delta) * 1000 / u64::from(target) + self.fit(a),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_seed_environment_and_plans() {
        let mut clocks = [0; 6];
        let mut weather = [0; 4];
        for seed in 0..2400 {
            let defaults = track_seed_defaults(seed).unwrap();
            clocks[[360, 540, 720, 1020, 1080, 1260]
                .iter()
                .position(|v| Some(*v) == defaults["time_minutes"].as_u64())
                .unwrap()] += 1;
            weather[["clear", "cloudy", "rain", "snow"]
                .iter()
                .position(|v| *v == defaults["weather"])
                .unwrap()] += 1;
            let plan = Plan::new(seed);
            for _ in 0..24 {
                assert_eq!(track_seed_defaults(seed).unwrap(), defaults);
                assert_eq!(Plan::new(seed), plan);
            }
        }
        assert!(clocks.iter().all(|n| (330..470).contains(n)), "{clocks:?}");
        assert!(
            weather.iter().all(|n| (520..680).contains(n)),
            "{weather:?}"
        );
        assert!(track_seed_defaults(9_007_199_254_740_992).is_err());
    }
}
