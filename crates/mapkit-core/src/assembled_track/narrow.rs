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

pub(super) fn try_section(
    source: &authoring::Source,
    a: &Assembly,
    reserved: &composition::Required,
) -> Result<Assembly> {
    let mut rng = a.settings.seed ^ 0x6e61_7272_6f77_3031;
    if next(&mut rng) % 2 == 0 || measure(a).sections > 0 {
        return Ok(a.clone());
    }
    let mut sites: Vec<_> = a.routes[0]
        .pieces
        .iter()
        .copied()
        .filter(|&i| {
            i >= START_PIECES
                && !reserved.iter().any(|r| r.contains(&i))
                && a.pieces[i].id == "sprint_lane"
                && a.pieces[i].entry_width_cm >= 600
                && a.pieces[i].exit_width_cm >= 600
                && !a.obstacles.iter().any(|o| o.piece_index == i)
        })
        .collect();
    if !sites.is_empty() {
        let n = next(&mut rng) as usize % sites.len();
        sites.rotate_left(n);
    }
    for i in sites.into_iter().take(4) {
        cancellation::checkpoint()?;
        let mut trial = source.clone();
        trial.instances[i].width_cm = 400;
        let mut compiled = match authoring::compile(&trial) {
            Ok(a) => a,
            Err(e) if e.code == "E_CANCELLED" => return Err(e),
            Err(_) => continue,
        };
        if valid(&compiled) && authoring::executable(&compiled).is_ok() {
            compiled.authoring = None;
            compiled.seed_source = Some(trial);
            return Ok(compiled);
        }
    }
    Ok(a.clone())
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
        layout::finish(pieces, &Settings::default())
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
        let a = layout::finish(vec![p], &Settings::default());
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
            let a = layout::finish(
                vec![materialize(&variant(id, 400, 400, 400))],
                &Settings::default(),
            );
            assert_eq!(measure(&a).length_cm, 0, "{id}");
        }
        let a = layout::finish(
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
