//! Uniform xz hash over padded reserved boxes. It only prunes pairs that
//! `authoring::conflict` can never report; the clearance predicate is unchanged.
use super::*;
use std::collections::HashMap;

const CELL_CM: i64 = 1600;
/// `conflict` is true only when the reserved boxes overlap or, for two level
/// straight ribbons without shared joints, when their `straight_footprint`s
/// overlap. A footprint extends at most lateral + 2·(wall + clearance) + 4 cm
/// beyond its centreline box, while reserved boxes already add the half width
/// plus wall + 60 cm. 400 cm covers the remaining 54 cm with a wide margin.
pub(super) const PAD_CM: i64 = 400;
/// Larger boxes are kept in one list instead of thousands of cells.
const MAX_CELLS: i64 = 4096;

pub(super) fn bounds(p: &Piece) -> [i64; 4] {
    [p.reserved_min_cm[0] - PAD_CM, p.reserved_min_cm[2] - PAD_CM, p.reserved_max_cm[0] + PAD_CM, p.reserved_max_cm[2] + PAD_CM]
}
fn touching(a: [i64; 4], b: [i64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}
/// Necessary condition for `authoring::conflict(a, b)` (and for equal pieces).
pub(super) fn near(a: &Piece, b: &Piece) -> bool {
    touching(bounds(a), bounds(b))
}
fn span(b: [i64; 4]) -> ([i64; 2], [i64; 2], i64) {
    let lo = [b[0].div_euclid(CELL_CM), b[1].div_euclid(CELL_CM)];
    let hi = [b[2].div_euclid(CELL_CM), b[3].div_euclid(CELL_CM)];
    (lo, hi, (hi[0] - lo[0] + 1).saturating_mul(hi[1] - lo[1] + 1))
}

/// Pieces are indexed in insertion order; `truncate` removes the newest ones.
#[derive(Default)]
pub(super) struct PieceGrid {
    cells: HashMap<(i64, i64), Vec<usize>>,
    wide: Vec<usize>,
    boxes: Vec<[i64; 4]>,
}
impl PieceGrid {
    pub(super) fn new(pieces: &[Piece]) -> Self {
        let mut grid = Self::default();
        for p in pieces { grid.push(p); }
        grid
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn len(&self) -> usize { self.boxes.len() }
    pub(super) fn push(&mut self, p: &Piece) {
        let (index, b) = (self.boxes.len(), bounds(p));
        let (lo, hi, count) = span(b);
        if count > MAX_CELLS { self.wide.push(index); } else {
            for x in lo[0]..=hi[0] { for z in lo[1]..=hi[1] { self.cells.entry((x, z)).or_default().push(index); } }
        }
        self.boxes.push(b);
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn truncate(&mut self, len: usize) {
        while self.boxes.len() > len {
            let b = self.boxes.pop().unwrap();
            let index = self.boxes.len();
            let (lo, hi, count) = span(b);
            if count > MAX_CELLS { self.wide.pop(); continue; }
            for x in lo[0]..=hi[0] { for z in lo[1]..=hi[1] {
                let cell = self.cells.get_mut(&(x, z)).unwrap();
                debug_assert_eq!(cell.last(), Some(&index));
                cell.pop();
                if cell.is_empty() { self.cells.remove(&(x, z)); }
            } }
        }
    }
    /// Sorted, deduplicated indices whose padded boxes touch `p`'s padded box.
    pub(super) fn candidates(&self, p: &Piece) -> Vec<usize> {
        let b = bounds(p);
        let (lo, hi, count) = span(b);
        let mut out: Vec<usize> = if count > self.cells.len() as i64 {
            (0..self.boxes.len()).collect()
        } else {
            let mut out = self.wide.clone();
            for x in lo[0]..=hi[0] { for z in lo[1]..=hi[1] {
                if let Some(cell) = self.cells.get(&(x, z)) { out.extend(cell); }
            } }
            out
        };
        out.sort_unstable();
        out.dedup();
        out.retain(|i| touching(self.boxes[*i], b));
        out
    }
    /// Equal to `!pieces[..self.len()].iter().any(|o| authoring::conflict(p, o))`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn clear(&self, p: &Piece, pieces: &[Piece]) -> bool {
        !self.candidates(p).into_iter().any(|i| authoring::conflict(p, &pieces[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn brute(source: Option<&authoring::Source>, pieces: &[Piece]) -> Vec<(usize, usize)> {
        let mut out = vec![];
        for i in 0..pieces.len() { for j in 0..i {
            if !source.is_some_and(|s| jump_shortcuts::shared_approach(s, i, j))
                && (pieces[i] == pieces[j] || authoring::conflict(&pieces[i], &pieces[j])) { out.push((i, j)); }
        } }
        out
    }
    fn gridded(source: Option<&authoring::Source>, pieces: &[Piece]) -> Vec<(usize, usize)> {
        let mut out = vec![];
        let mut grid = PieceGrid::default();
        for (i, p) in pieces.iter().enumerate() {
            for j in grid.candidates(p) {
                if !source.is_some_and(|s| jump_shortcuts::shared_approach(s, i, j))
                    && (pieces[i] == pieces[j] || authoring::conflict(p, &pieces[j])) { out.push((i, j)); }
            }
            grid.push(p);
        }
        out
    }
    /// Every straight footprint and every conflicting pair lies inside the padded boxes.
    fn check(source: Option<&authoring::Source>, pieces: &[Piece], label: &str) -> usize {
        for p in pieces {
            if let Some(f) = geometry::straight_footprint(p, false) {
                let b = bounds(p);
                assert!(f.iter().all(|v| v[0] > (b[0] + PAD_CM / 2) as f64 && v[0] < (b[2] - PAD_CM / 2) as f64
                    && v[1] > (b[1] + PAD_CM / 2) as f64 && v[1] < (b[3] - PAD_CM / 2) as f64), "{label}: footprint {} beyond padding", p.id);
            }
        }
        let expected = brute(source, pieces);
        assert_eq!(gridded(source, pieces), expected, "{label}");
        let grid = PieceGrid::new(pieces);
        for (i, p) in pieces.iter().enumerate() {
            let near: Vec<_> = (0..pieces.len()).filter(|j| near(p, &pieces[*j])).collect();
            assert_eq!(grid.candidates(p), near, "{label}: {i}");
        }
        expected.len()
    }
    #[test]
    fn catalogue_footprints_stay_inside_padding() {
        for id in catalogue_ids() {
            for &w in supported_widths(id) {
                for yaw in [0, 30000, 90000, 135000, 251000] {
                    let mut p = variant(id, w, w, w);
                    p.rotation_mdeg = [0, yaw, 0];
                    p.origin_cm = [12345, 0, -6789];
                    check(None, &[materialize(&p)], id);
                }
            }
        }
    }
    #[test]
    fn grid_truncate_matches_rebuild() {
        let s = Settings { seed: 3, circuit: false, length: Length::Short, ..Settings::default() };
        let a = assemble(&s).unwrap();
        let mut grid = PieceGrid::new(&a.pieces);
        for len in [a.pieces.len() / 2, 7, 0] {
            grid.truncate(len);
            assert_eq!(grid.len(), len);
            let fresh = PieceGrid::new(&a.pieces[..len]);
            for p in &a.pieces { assert_eq!(grid.candidates(p), fresh.candidates(p)); }
        }
        for p in &a.pieces { grid.push(p); }
        for (i, p) in a.pieces.iter().enumerate() {
            assert_eq!(grid.clear(p, &a.pieces), !a.pieces.iter().any(|o| authoring::conflict(p, o)), "{i}");
        }
    }
    #[test]
    fn grid_equals_brute_force_on_seed_tracks() {
        let mut collisions = 0;
        for (seed, circuit) in [(1, true), (2, false), (7, true), (42, false)] {
            let s = Settings { seed, circuit, length: Length::Short, ..Settings::default() };
            let a = assemble(&s).unwrap();
            let source = a.seed_source.as_ref().unwrap();
            collisions += check(Some(source), &a.pieces, &format!("seed {seed} circuit {circuit}"));
            let mut tight = a.pieces.clone();
            // Shift a copy onto the course so the comparison also covers real collisions.
            for p in tight.iter_mut().skip(a.pieces.len() / 2) {
                let q = Piece { origin_cm: add(p.origin_cm, [137, 0, -211]), ..p.clone() };
                *p = materialize(&q);
            }
            collisions += check(Some(source), &tight, &format!("shifted seed {seed}"));
        }
        assert!(collisions > 0);
    }
    #[test]
    fn grid_equals_brute_force_on_editor_examples() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../map-editor/examples");
        let Ok(_) = std::fs::metadata(&root) else { return; };
        let mut stack = vec![root];
        let mut files = vec![];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() { stack.push(path); } else if path.extension().is_some_and(|e| e == "json") { files.push(path); }
            }
        }
        files.sort();
        let mut checked = 0;
        for path in files {
            let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()) else { continue; };
            legacy_length(&mut value);
            let label = path.display().to_string();
            let source = serde_json::from_value::<authoring::Source>(value.clone()).ok()
                .or_else(|| value.get("assembled_track").and_then(|a| a.get("authoring")).and_then(|v| serde_json::from_value(v.clone()).ok()));
            if let Some(source) = source {
                if let Ok(pieces) = source.instances.iter().map(authoring::piece).collect::<Result<Vec<_>>>() {
                    check(Some(&source), &pieces, &label);
                    checked += 1;
                }
            }
            if let Some(a) = value.get("assembled_track").and_then(|v| serde_json::from_value::<Assembly>(v.clone()).ok()) {
                check(a.authoring.as_ref().or(a.seed_source.as_ref()), &a.pieces, &label);
                checked += 1;
            }
        }
        assert!(checked > 0, "no loadable example tracks");
    }
    /// Test-only reading of editor examples that still store `duration_seconds`
    /// (rewritten to `length` in M8).
    fn legacy_length(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(seconds) = m.remove("duration_seconds").and_then(|s| s.as_u64()) {
                    m.insert("length".into(), [Length::Short, Length::Medium, Length::Long][usize::from(seconds > 75) + usize::from(seconds > 105)].id().into());
                }
                m.values_mut().for_each(legacy_length);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(legacy_length),
            _ => {}
        }
    }
}
