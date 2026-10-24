//! A map-wide budget over actual ordinary road paths, including optional routes.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct NarrowSummary {
    pub sections: usize,
    pub length_cm: u64,
    pub limit_cm: u64,
}
fn ordinary(p: &Piece) -> bool {
    !pipe_piece(&p.id)
        && ![
            "loop",
            "overpass",
            "tube_entry",
            "tube_exit",
            "finish_plaza",
        ]
        .contains(&p.id.as_str())
}
pub(super) fn measure(a: &Assembly) -> NarrowSummary {
    let mut edges = BTreeSet::new();
    let used: BTreeSet<_> = a
        .routes
        .iter()
        .flat_map(|r| r.pieces.iter().copied())
        .collect();
    for i in used {
        let p = &a.pieces[i];
        // Wide bodies tapering to a dedicated 4 m port have no narrow body.
        // Quantized samples next to that endpoint may also round to 200 cm.
        if !ordinary(p) || p.width_cm != 400 {
            continue;
        }
        for path in [&p.path, &p.alternate_path] {
            for w in path.windows(2) {
                // A single 4 m portal endpoint is not a length of narrow road.
                if w.iter().all(|s| s.lateral_cm <= 200 && s.mode != "flight") {
                    let (a, b) = (w[0].position_cm, w[1].position_cm);
                    if a != b {
                        edges.insert(if a < b { (a, b) } else { (b, a) });
                    }
                }
            }
        }
    }
    let mut adjacent: BTreeMap<Vertex, Vec<Vertex>> = BTreeMap::new();
    let length_cm = edges.iter().map(|&(a, b)| distance(a, b)).sum();
    for &(a, b) in &edges {
        adjacent.entry(a).or_default().push(b);
        adjacent.entry(b).or_default().push(a);
    }
    let mut visited = BTreeSet::new();
    let mut sections = 0;
    for &point in adjacent.keys() {
        if visited.contains(&point) {
            continue;
        }
        sections += 1;
        let mut open = vec![point];
        while let Some(p) = open.pop() {
            if visited.insert(p) {
                open.extend(&adjacent[&p]);
            }
        }
    }
    NarrowSummary {
        sections,
        length_cm,
        limit_cm: a.length_cm / 20,
    }
}
pub(super) fn valid(a: &Assembly) -> bool {
    let n = measure(a);
    n.sections <= 1 && n.length_cm <= n.limit_cm
}
/// Generated rail approach/landing roads follow the ordinary width policy.
/// Flight beams and their immutable collision dimensions keep their own widths.
pub(super) fn prepare_fragment(s: &mut authoring::Source) {
    for i in &mut s.instances {
        if i.preset == "free_curve" && i.width_cm == 400 {
            i.width_cm = 600;
        }
    }
}

/// Pieces a narrow section may use: ordinary level roads whose geometry does
/// not depend on their own body width once the ports keep theirs.
const NARROWABLE: [&str; 5] = ["sprint_lane", "straight", "gentle45", "gentle45_left", "chicane"];
/// Pure selection of the seed's one optional narrow section: about one seed in
/// eight (`next % 8 == 0` on the narrow stream) narrows 1–2 consecutive
/// ordinary pieces, ≤40 m and ≤5% of the route, starting and ending between
/// 20% and 80% of its length, away from the start, jumps, drops, pipes,
/// shortcuts and obstacles. Empty when the seed has none or nothing fits.
pub(super) fn select(a: &Assembly, source: &authoring::Source, reserved: &[std::ops::Range<usize>]) -> Vec<usize> {
    let mut rng = a.settings.seed ^ 0x6e61_7272_6f77_3031;
    if next(&mut rng) % 8 != 0 { return vec![]; }
    let route = &a.routes[0].pieces;
    let n = route.len();
    let calm = |i: usize| {
        let p = &a.pieces[route[i]];
        i >= START_PIECES && category(&p.id) == "driving" && !pipe_piece(&p.id)
            && !["tube_entry", "tube_exit", "finish_plaza", "flight_curve", "loop", "overpass"].contains(&p.id.as_str())
            && !reserved.iter().any(|r| r.contains(&route[i])) && !a.obstacles.iter().any(|o| o.piece_index == route[i])
    };
    let eligible = |i: usize| {
        let p = &a.pieces[route[i]];
        calm(i) && NARROWABLE.contains(&p.id.as_str()) && p.width_cm >= 600 && p.entry_width_cm >= 600 && p.exit_width_cm >= 600
    };
    let mut stations = vec![0];
    for &i in route { stations.push(stations.last().unwrap() + race_length(&a.pieces[i])); }
    let total = *stations.last().unwrap();
    let limit = (total / 20).min(4000);
    let mut options = vec![];
    for i in START_PIECES + 1..n.saturating_sub(1) {
        for k in 1..=2 {
            if i + k >= n || !(i..i + k).all(eligible) || !calm(i - 1) || !calm(i + k) { continue; }
            let (from, to) = (stations[i], stations[i + k]);
            if from * 5 >= total && to * 5 <= total * 4 && to - from <= limit { options.push((i..i + k).map(|j| route[j]).collect::<Vec<_>>()); }
        }
    }
    if options.is_empty() { return vec![]; }
    let first = next(&mut rng) as usize % options.len();
    options.rotate_left(first);
    // The tapers must leave a real narrow run inside the chosen pieces, and the
    // narrowed ports must still clear their neighbours exactly as compile checks.
    options.into_iter().take(8).find(|sel| {
        let mut trial = source.clone();
        apply(&mut trial, sel);
        let Ok(narrowed) = sel.iter().map(|&i| authoring::piece(&trial.instances[i])).collect::<Result<Vec<_>>>() else { return false };
        let (first, last) = (sel[0], *sel.last().unwrap());
        let around = [first - 1, last + 1].map(|i| &a.pieces[i]);
        narrowed.iter().map(|p| p.path.windows(2).filter(|w| w.iter().all(|s| s.lateral_cm <= 200))
            .map(|w| distance(w[0].position_cm, w[1].position_cm)).sum::<u64>()).sum::<u64>() >= 400
            && !narrowed.iter().any(|p| around.iter().any(|q| authoring::conflict(p, q)))
            && !(narrowed.len() == 2 && authoring::conflict(&narrowed[0], &narrowed[1]))
    }).unwrap_or_default()
}
/// Narrow the selected bodies to 4 m; a two-piece section also narrows its inner joint.
pub(super) fn apply(source: &mut authoring::Source, selected: &[usize]) {
    for (k, &i) in selected.iter().enumerate() {
        let item = &mut source.instances[i];
        item.width_cm = 400;
        if k > 0 { item.entry_width_cm = 400; }
        if k + 1 < selected.len() { item.exit_width_cm = 400; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(widths: &[u32]) -> Assembly {
        let mut pieces = vec![];
        let mut at = [0; 3];
        for &w in widths {
            push_piece(&mut pieces, &mut at, 0, "straight", w, true, [0; 3]);
        }
        finish::assembly(pieces, &Settings::default())
    }
    #[test]
    fn actual_narrow_runs_span_pieces_branches_and_the_circuit_seam() {
        let mut widths = vec![800; 80];
        widths[8] = 400;
        widths[9] = 400;
        let mut a = fixture(&widths);
        let n = measure(&a);
        assert_eq!((n.sections, n.length_cm), (1, 1600));
        assert!(valid(&a));
        a.pieces[30] = materialize(&variant("straight", 400, 400, 400));
        assert_eq!(measure(&a).sections, 2);
        assert!(!valid(&a));
        // Shared routes and repeated route combinations cannot multiply length.
        let mut a = fixture(&widths);
        a.routes.push(a.routes[0].clone());
        assert_eq!(measure(&a), n);
        // A distinct optional path consumes the same global budget.
        let i = a.pieces.len();
        let mut p = materialize(&variant("straight", 400, 400, 400));
        for s in &mut p.path {
            s.position_cm[0] += 10000;
        }
        a.pieces.push(p);
        let mut r = a.routes[0].clone();
        r.pieces.push(i);
        a.routes.push(r);
        assert_eq!(measure(&a).sections, 2);
        let mut seam = fixture(&[400, 800, 400]);
        let end = seam.pieces[0].path[0].position_cm;
        seam.pieces[2].path.last_mut().unwrap().position_cm = end;
        assert_eq!(measure(&seam).sections, 1);
        assert!(!valid(&fixture(&[400, 800, 800])));
    }
    #[test]
    fn eased_ports_and_dedicated_structures_do_not_add_narrow_road_runs() {
        let mut fragment=corner_shortcuts::source("grind_s_right",800,true).unwrap();
        prepare_fragment(&mut fragment);
        authoring::executable(&authoring::compile(&fragment).unwrap()).unwrap();
        let p = materialize(&variant("sprint_lane", 400, 600, 600));
        assert_eq!(p.path.first().unwrap().lateral_cm, 300);
        assert_eq!(p.path.last().unwrap().lateral_cm, 300);
        let length = race_length(&p);
        let a = finish::assembly(vec![p], &Settings::default());
        let n = measure(&a);
        assert_eq!(n.sections, 1);
        assert!(n.length_cm > 0 && n.length_cm < length);
        for id in [
            "loop",
            "cylinder",
            "banked_chicane",
            "overpass",
            "tube_entry",
            "tube_exit",
        ] {
            let a = finish::assembly(
                vec![materialize(&variant(id, 400, 400, 400))],
                &Settings::default(),
            );
            assert_eq!(measure(&a).length_cm, 0, "{id}");
        }
        let a = finish::assembly(
            vec![materialize(&variant("straight", 600, 800, 400))],
            &Settings::default(),
        );
        assert_eq!(
            measure(&a).length_cm,
            0,
            "one 4 m endpoint is a port, not a road run"
        );
    }
}
