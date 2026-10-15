//! Generation policy over the completed base route, never stored-v1 validation.
use super::*;
use std::{collections::BTreeSet, ops::Range};

pub(super) type Required = Vec<Range<usize>>;
pub(super) const MAX_STRAIGHT_CM: u64 = 3200;

#[derive(Default, Debug, Clone)]
pub(super) struct Composition {
    pub lengths: [u64; 3],
    pub families: BTreeSet<String>,
    pub longest_straight: u64,
    pub tail: u64,
}

pub(super) fn category_index(id: &str) -> usize {
    match category(id) {
        "gimmick" => 1,
        "action" => 2,
        _ => 0,
    }
}

// Measure actual horizontal tangent changes, including slopes, wide presets and
// cubic return seams. Required template recovery resets the optional-straight run.
pub(super) fn straight_run(p: &Piece, required: bool, mut tail: u64) -> (u64, u64) {
    if required
        || category_index(&p.id) != 0
        || ["tube_entry", "tube_exit", "approach", "finish_plaza"].contains(&p.id.as_str())
    {
        return (0, 0);
    }
    let mut longest = tail;
    for w in p.path.windows(2) {
        let (a, b) = (w[0].forward, w[1].forward);
        if (a[0] as i128 * b[2] as i128 - a[2] as i128 * b[0] as i128).abs() < 5_000_000_000 {
            tail += distance(w[0].position_cm, w[1].position_cm);
            longest = longest.max(tail);
        } else {
            tail = 0;
        }
    }
    (longest, tail)
}

impl Composition {
    pub fn measure<'a>(
        pieces: impl IntoIterator<Item = (usize, &'a Piece)>,
        required: &Required,
    ) -> Self {
        let mut out = Self::default();
        for (i, p) in pieces {
            let c = category_index(&p.id);
            out.lengths[c] += race_length(p);
            if c != 0 {
                out.families.insert(family(&p.id).into());
            }
            let (longest, tail) = straight_run(
                p,
                i < START_PIECES || required.iter().any(|r| r.contains(&i)),
                out.tail,
            );
            out.longest_straight = out.longest_straight.max(longest);
            out.tail = tail;
        }
        out
    }
    pub fn assembly(a: &Assembly, required: &Required) -> Self {
        Self::measure(
            a.routes[0].pieces.iter().map(|&i| (i, &a.pieces[i])),
            required,
        )
    }
    pub fn total(&self) -> u64 {
        self.lengths.iter().sum::<u64>().max(1)
    }
    pub fn special(&self) -> u64 {
        self.lengths[1] + self.lengths[2]
    }
    pub fn penalty(&self, s: &Settings) -> u64 {
        let has = |id| s.categories.iter().any(|c| c == id);
        let mut penalty = self.longest_straight.saturating_sub(MAX_STRAIGHT_CM);
        if has("gimmick") || has("action") {
            penalty += (self.total() * 40).saturating_sub(self.special() * 100) / 100;
            if has("driving") {
                penalty += (self.special() * 100).saturating_sub(self.total() * 60) / 100;
            }
        }
        if s.categories.len() == 3 {
            penalty += (3usize.saturating_sub(self.families.len()) as u64) * 3200;
            penalty += self.lengths[1..].iter().filter(|&&n| n == 0).count() as u64 * 3200;
        }
        penalty
    }
    pub fn valid(&self, s: &Settings) -> bool {
        // Keep exact integer boundary checks (a sub-centimetre ratio miss is a miss).
        let special = s.categories.iter().any(|c| c != "driving");
        self.penalty(s) == 0
            && (!special || self.special() * 100 >= self.total() * 40)
            && (!special
                || !s.categories.iter().any(|c| c == "driving")
                || self.special() * 100 <= self.total() * 60)
    }
    pub fn reason(&self) -> String {
        format!("base-route special distance {:.1}%, {} families, gimmick/action {} / {} cm, longest optional straight {} cm (required: 40–60% with driving, >=40% special-only, all-category >=3 families and both categories, straight <=3200 cm)",
            self.special() as f64 * 100.0 / self.total() as f64, self.families.len(), self.lengths[1], self.lengths[2], self.longest_straight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_count_only_main_special_distance_and_preserve_recovery() {
        for id in ["double_jump", "air_ring_chain", "grind_shortcut"] {
            let source = templates::source(id).unwrap();
            let a = authoring::compile(&source).unwrap();
            let required = vec![0..a.routes[0].pieces.len()];
            let quality = Composition::assembly(&a, &required);
            let expected: u64 = a.routes[0]
                .pieces
                .iter()
                .map(|&i| &a.pieces[i])
                .filter(|p| p.id == "jump")
                .map(race_length)
                .sum();
            assert_eq!(quality.special(), expected, "{id}");
            assert_eq!(quality.total(), a.length_cm);
            assert_eq!(quality.longest_straight, 0);
            assert!(
                quality.lengths[0] > 0,
                "preparation/recovery counts as ordinary road"
            );
        }
    }
    #[test]
    fn straight_limit_spans_piece_widths_and_includes_return_seams() {
        let pieces: Vec<_> = [
            "straight",
            "straight_extra_wide",
            "straight_ultra_wide",
            "sprint_lane",
        ]
        .iter()
        .map(|id| materialize(&catalogue_piece(id)))
        .collect();
        // These are after the runway, so their 8+8+8+16 m form one optional run.
        let q = Composition::measure(pieces.iter().enumerate().map(|(i, p)| (i + 3, p)), &vec![]);
        assert_eq!(q.longest_straight, 4000);
        assert!(!q.valid(&Settings {
            categories: vec!["driving".into()],
            ..Settings::default()
        }));
        let mut seam = authoring::instance("seam", "free_curve", 800);
        seam.control_points = vec![[0, 0, 0], [0, 0, 200], [0, 0, 400], [0, 0, 600]];
        let seam = authoring::piece(&seam).unwrap();
        assert_eq!(straight_run(&seam, false, 3000).0, 3600);
        assert_eq!(straight_run(&seam, true, 3000), (0, 0));
    }
    #[test]
    fn mixed_and_special_only_masks_use_exact_distance_bounds() {
        let mixed = Settings::default();
        let mut q = Composition {
            lengths: [6000, 2000, 2000],
            families: ["loop", "cylinder", "jump"].map(String::from).into(),
            ..Composition::default()
        };
        assert!(q.valid(&mixed));
        q.lengths = [6001, 2000, 2000];
        assert!(!q.valid(&mixed));
        q.lengths = [4000, 3000, 3000];
        assert!(q.valid(&mixed));
        q.lengths = [3999, 3000, 3000];
        assert!(!q.valid(&mixed));
        assert!(q.valid(&Settings {
            categories: vec!["gimmick".into(), "action".into()],
            ..mixed.clone()
        }));
        q.lengths = [6000, 4000, 0];
        assert!(!q.valid(&mixed));
        q.lengths = [10000, 0, 0];
        q.families.clear();
        assert!(q.valid(&Settings {
            categories: vec!["driving".into()],
            ..mixed
        }));
    }
}
