//! Reuses local piece geometry across placements. `place` equals `materialize`
//! exactly: both build `local_shape` and finish it with `place_shape`.
use super::*;
use std::collections::HashMap;

type Key = (String, u32, u32, u32, Vec<Vertex>);
#[derive(Default)]
pub(super) struct PieceCache {
    shapes: HashMap<Key, Piece>,
}
impl PieceCache {
    fn shape(&mut self, id: &str, width: u32, entry: u32, exit: u32, control_points: &[Vertex]) -> Piece {
        self.shapes.entry((id.into(), width, entry, exit, control_points.to_vec()))
            .or_insert_with(|| local_shape(id, width, entry, exit, control_points)).clone()
    }
    /// The placed piece for `p`'s preset, widths, control points, chain and pose.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn place(&mut self, p: &Piece) -> Piece {
        place_shape(self.shape(&p.id, p.width_cm, p.entry_width_cm, p.exit_width_cm, &p.control_points), p)
    }
    /// `materialize` of an authored instance's preset, widths, pose and control points.
    pub(super) fn instance(&mut self, i: &authoring::Instance) -> Piece {
        let mut pose = self.shape(&i.preset, i.width_cm, i.entry_width_cm, i.exit_width_cm, &i.control_points);
        pose.origin_cm = i.position_cm;
        pose.rotation_mdeg = i.rotation_mdeg;
        pose.quarter_turns = (i.rotation_mdeg[1].rem_euclid(360000) / 90000) as u8;
        let (path, alternate_path) = (std::mem::take(&mut pose.path), std::mem::take(&mut pose.alternate_path));
        place_shape(Piece { path, alternate_path, ..pose.clone() }, &pose)
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize { self.shapes.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_placement_equals_materialize() {
        let mut cache = PieceCache::default();
        for id in catalogue_ids() {
            for &w in supported_widths(id) {
                for (yaw, origin) in [(0, [0, 0, 0]), (45000, [1234, -56, 789]), (270000, [-99999, 400, 5]), (123456, [7, 8, 9])] {
                    let mut p = variant(id, w, w, w);
                    (p.rotation_mdeg, p.origin_cm, p.quarter_turns) = ([0, yaw, 0], origin, (yaw / 90000) as u8);
                    (p.chain_id, p.chain_index, p.chain_count) = (3, 1, 2);
                    assert_eq!(cache.place(&p), materialize(&p), "{id} {w} {yaw}");
                    assert_eq!(cache.place(&p), materialize(&p), "cached {id} {w} {yaw}");
                    let mut instance = authoring::instance(id, id, w);
                    (instance.position_cm, instance.rotation_mdeg) = (origin, [0, yaw, 0]);
                    let mut expected = variant(id, w, w, w);
                    (expected.origin_cm, expected.rotation_mdeg, expected.quarter_turns) = (origin, [0, yaw, 0], (yaw / 90000) as u8);
                    assert_eq!(cache.instance(&instance), materialize(&expected), "instance {id} {w} {yaw}");
                }
            }
        }
        let mut curve = variant("free_curve", 800, 600, 800);
        curve.control_points = vec![[0, 0, 0], [0, 0, 900], [600, 100, 1800], [1500, 100, 2400]];
        curve.origin_cm = [300, 20, -40];
        curve.rotation_mdeg = [0, 15000, 0];
        assert_eq!(cache.place(&curve), materialize(&curve));
        curve.control_points[3][0] += 50;
        assert_eq!(cache.place(&curve), materialize(&curve));
        assert!(cache.len() > catalogue_ids().len());
    }
    #[test]
    fn cached_placement_reproduces_seed_tracks() {
        let mut cache = PieceCache::default();
        for (seed, circuit) in [(5, true), (6, false)] {
            let a = layout::assemble(&Settings { seed, circuit, length: Length::Short, ..Settings::default() }).unwrap();
            for p in &a.pieces { assert_eq!(&cache.place(p), p, "seed {seed} {}", p.id); }
        }
    }
}
