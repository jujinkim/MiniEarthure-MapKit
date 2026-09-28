//! Bounded random extension followed by a collision-aware ordinary-road closure.
use super::*;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

const ORDINARY: &[&str] = &[
    "straight",
    "straight",
    "curve",
    "curve_left",
    "slope_up",
    "slope_down",
    "curve_up",
    "curve_down",
    "curve_left_up",
    "curve_left_down",
    "chicane",
    "hairpin",
    "hairpin_left",
];
const CLOSING: &[&str] = &[
    "straight",
    "curve",
    "curve_left",
    "slope_up",
    "slope_down",
    "curve_up",
    "curve_down",
    "curve_left_up",
    "curve_left_down",
];
const TUBES: &[&str] = &[
    "cylinder",
    "cylinder_curve",
    "cylinder_curve_left",
    "cylinder_uturn",
    "cylinder_uturn_left",
    "cylinder_s_rise",
];
const MAX_BACKTRACKS: usize = 192;
const MAX_CLOSURE_NODES: usize = 4096;

/// Mandatory approach/exit roads cannot conceal a run of three equal gimmicks.
/// An independently chosen ordinary road does break the run. Up/down helices differ.
pub(super) fn valid_runs(pieces: &[Piece], circuit: bool) -> bool {
    let sequence: Vec<Option<&str>> = pieces
        .iter()
        .filter_map(|p| {
            if !basic_ids().contains(&p.id.as_str()) {
                Some(Some(family(&p.id)))
            } else if p.ordinary
                && !["tube_entry", "tube_exit", "approach", "finish_plaza"].contains(&p.id.as_str())
            {
                Some(None)
            } else {
                None
            }
        })
        .collect();
    if sequence.is_empty() {
        return true;
    }
    for i in 0..sequence.len() {
        if !circuit && i + 2 >= sequence.len() {
            break;
        }
        if sequence[i].is_some()
            && (1..3).all(|n| sequence[(i + n) % sequence.len()] == sequence[i])
        {
            return false;
        }
    }
    true
}

pub(super) fn overlaps(a: &Piece, b: &Piece) -> bool {
    if !(0..3).all(|j| {
        a.reserved_min_cm[j] < b.reserved_max_cm[j] && b.reserved_min_cm[j] < a.reserved_max_cm[j]
    }) {
        return false;
    }
    // The cylinder reference is its actual inner floor. Reserve its full bore
    // and shell, not the former lowered floor or an arbitrary route offset.
    let volume = |s: &Sample| {
        let r = i64::from(s.tube_radius_cm);
        let center = if r > 0 {
            add(s.position_cm, s.normal.map(|v| v * r / 1_000_000))
        } else {
            s.position_cm
        };
        (
            center,
            if r > 0 {
                r + 15
            } else {
                i64::from(s.lateral_cm) + 30
            },
            if r > 0 {
                center[1] - r - 15
            } else {
                center[1] - 15
            },
            if r > 0 {
                center[1] + r + 15
            } else {
                center[1] + 235
            },
        )
    };
    a.path.iter().chain(&a.alternate_path).any(|s| {
        let (ac, ar, alo, ahi) = volume(s);
        b.path.iter().chain(&b.alternate_path).any(|t| {
            let (bc, br, blo, bhi) = volume(t);
            alo < bhi
                && blo < ahi
                && (ac[0] - bc[0]).pow(2) + (ac[2] - bc[2]).pow(2) < (ar + br).pow(2)
        })
    })
}
fn end(pieces: &[Piece]) -> (Vertex, u8) {
    let p = pieces.last().unwrap().path.last().unwrap();
    let q = match p.forward {
        [0, 0, 1_000_000] => 0,
        [1_000_000, 0, 0] => 1,
        [0, 0, -1_000_000] => 2,
        [-1_000_000, 0, 0] => 3,
        _ => unreachable!("catalogue ports are cardinal"),
    };
    (p.position_cm, q)
}
fn fits(pieces: &[Piece], p: &Piece, closing: bool) -> bool {
    pieces
        .iter()
        .enumerate()
        .take(pieces.len().saturating_sub(1))
        .all(|(i, other)| closing && i == 0 || !overlaps(p, other))
}
fn within_budget(pieces: &[Piece]) -> bool {
    pieces.len() <= MAX_PIECES
        && pieces
            .iter()
            .map(|p| p.path.len() + p.alternate_path.len())
            .sum::<usize>()
            <= MAX_SAMPLES
}
fn append(pieces: &mut Vec<Piece>, id: &str, w: u32, ordinary: bool, chain: [u32; 3]) -> bool {
    let (mut origin, q) = end(pieces);
    let size = pieces.len();
    push_piece(pieces, &mut origin, q, id, w, ordinary, chain);
    if !fits(&pieces[..size], &pieces[size], false)
        || !within_budget(pieces)
        || !valid_runs(pieces, false)
    {
        pieces.pop();
        false
    } else {
        true
    }
}
fn dedicated(id: &str) -> bool {
    ["loop", "banked_chicane", "overpass"].contains(&id)
}
fn block(pieces: &mut Vec<Piece>, id: &str, rng: &mut u64) -> bool {
    let size = pieces.len();
    let w = if dedicated(id) { 400 } else { width(rng) };
    let tube = pipe_piece(id);
    let chosen = if family(id) == "cylinder" {
        TUBES[next(rng) as usize % TUBES.len()]
    } else {
        id
    };
    let chain = size as u32 + 1;
    // Presets contain their actual run-up/landing areas. Only pipes need
    // separate floor-height ramps; ordinary spacing is searched on demand.
    if (!tube || append(pieces, "tube_entry", w, false, [0; 3]))
        && append(pieces, chosen, w, false, [chain, 0, 1])
        && (!tube || append(pieces, "tube_exit", w, false, [0; 3]))
    {
        true
    } else {
        pieces.truncate(size);
        false
    }
}

fn ordinary(pieces: &mut Vec<Piece>, rng: &mut u64) -> bool {
    for _ in 0..12 {
        if append(
            pieces,
            ORDINARY[next(rng) as usize % ORDINARY.len()],
            width(rng),
            true,
            [0; 3],
        ) {
            return true;
        }
    }
    false
}
fn mandatory(
    pieces: &mut Vec<Piece>,
    ids: &[String],
    rng: &mut u64,
    tries: &mut usize,
) -> Result<bool> {
    if ids.is_empty() {
        return Ok(true);
    }
    let size = pieces.len();
    for attempt in 0..12 {
        cancellation::checkpoint()?;
        if *tries >= MAX_BACKTRACKS {
            break;
        }
        *tries += 1;
        pieces.truncate(size);
        let roads = attempt / 3; // direct connection first, then minimal connectors
        if !(0..roads).all(|_| ordinary(pieces, rng)) {
            continue;
        }
        if block(pieces, &ids[0], rng) && mandatory(pieces, &ids[1..], rng, tries)? {
            return Ok(true);
        }
    }
    pieces.truncate(size);
    Ok(false)
}
fn connection_reserve(pieces: &[Piece], circuit: bool) -> u64 {
    if !circuit {
        return 4000;
    }
    let (p, _) = end(pieces);
    // A conservative length allowance for turns, height restoration and detours.
    (p[0].abs() + p[2].abs() + p[1].abs() * 8 + 10_000) as u64
}
fn elapsed(pieces: &[Piece]) -> u32 {
    pieces.iter().map(|p| p.reference_msec).sum()
}
fn close(pieces: &mut Vec<Piece>) -> Result<bool> {
    #[derive(Clone)]
    struct Node {
        port: (Vertex, u8),
        parent: usize,
        piece: Option<Piece>,
        cost: u64,
    }
    let goal = ([0; 3], 0);
    let initial = end(pieces);
    let mut nodes = vec![Node {
        port: initial,
        parent: 0,
        piece: None,
        cost: 0,
    }];
    let heuristic = |(p, q): (Vertex, u8)| {
        ((p[0].abs() + p[2].abs()) as u64).max(p[1].unsigned_abs() * 8)
            + if q == 0 { 0 } else { 800 }
    };
    let mut queue = BinaryHeap::from([Reverse((heuristic(initial), 0usize))]);
    let mut costs = BTreeMap::from([(initial, 0u64)]);
    let ymin = initial.0[1].min(0);
    let ymax = initial.0[1].max(0);
    while let Some(Reverse((_, index))) = queue.pop() {
        cancellation::checkpoint()?;
        let node = nodes[index].clone();
        if node.port == goal && index > 0 {
            let mut path = vec![];
            let mut at = index;
            while at > 0 {
                path.push(nodes[at].piece.clone().unwrap());
                at = nodes[at].parent;
            }
            path.reverse();
            pieces.extend(path);
            return Ok(within_budget(pieces));
        }
        if nodes.len() + CLOSING.len() > MAX_CLOSURE_NODES {
            break;
        }
        for id in CLOSING {
            let mut origin = node.port.0;
            let mut trial = vec![];
            push_piece(&mut trial, &mut origin, node.port.1, id, 400, true, [0; 3]);
            let p = trial.pop().unwrap();
            let port = end(std::slice::from_ref(&p));
            if port.0[1] < ymin
                || port.0[1] > ymax
                || port.0[0].abs() > initial.0[0].abs() + 8000
                || port.0[2].abs() > initial.0[2].abs() + 8000
            {
                continue;
            }
            let cost = node.cost + race_length(&p);
            if costs.get(&port).is_some_and(|old| *old <= cost) {
                continue;
            }
            // Only the initial closure segment neighbours the growth path.
            let count = if index == 0 {
                pieces.len().saturating_sub(1)
            } else {
                pieces.len()
            };
            if pieces
                .iter()
                .take(count)
                .enumerate()
                .any(|(i, other)| !(port == goal && i == 0) && overlaps(&p, other))
            {
                continue;
            }
            let mut at = node.parent;
            let mut collision = false;
            while at > 0 {
                if overlaps(&p, nodes[at].piece.as_ref().unwrap()) {
                    collision = true;
                    break;
                }
                at = nodes[at].parent;
            }
            if collision {
                continue;
            }
            costs.insert(port, cost);
            let n = nodes.len();
            nodes.push(Node {
                port,
                parent: index,
                piece: Some(p),
                cost,
            });
            queue.push(Reverse((cost + heuristic(port) * 2, n)));
        }
    }
    Ok(false)
}
pub(super) fn finish(mut pieces: Vec<Piece>, s: &Settings) -> Assembly {
    // Ports are shared exactly; width changes occur smoothly within the road.
    for i in 0..pieces.len() {
        let previous = if i > 0 {
            i - 1
        } else if s.circuit {
            pieces.len() - 1
        } else {
            0
        };
        let following = if i + 1 < pieces.len() {
            i + 1
        } else if s.circuit {
            0
        } else {
            i
        };
        let joint = |a: &Piece, b: &Piece| {
            if a.id.starts_with("cylinder") || b.id.starts_with("cylinder") {
                a.width_cm.min(b.width_cm)
            } else if dedicated(&a.id) || dedicated(&b.id) {
                400
            } else {
                (a.width_cm + b.width_cm) / 2
            }
        };
        pieces[i].entry_width_cm = joint(&pieces[previous], &pieces[i]);
        pieces[i].exit_width_cm = joint(&pieces[i], &pieces[following]);
    }
    pieces = pieces.iter().map(materialize).collect();
    let (length_cm, ordinary_length_cm, ordinary_straight_cm, estimated_msec, floor) =
        statistics(&pieces);
    Assembly {
        settings: s.clone(),
        generator_fingerprint: fingerprint(),
        catalogue_fingerprint: catalogue_fingerprint(),
        finish_plaza: finish_plaza(&pieces),
        pieces,
        obstacles: vec![],
        obstacle_eligible_length_cm: 0,
        obstacle_target_count: 0,
        length_cm,
        ordinary_length_cm,
        ordinary_straight_cm,
        estimated_msec,
        floor,
    }
}
fn candidate(s: &Settings, attempt: u64) -> Result<Option<Assembly>> {
    let mut rng = s.seed ^ attempt.wrapping_mul(0xa0761d6478bd642f);
    let mut ids: Vec<String> = s
        .gimmicks
        .iter()
        .filter(|id| id.as_str() != "obstacles")
        .map(|id| family(id).to_string())
        .collect();
    ids.sort();
    ids.dedup();
    for i in (1..ids.len()).rev() {
        let j = next(&mut rng) as usize % (i + 1);
        ids.swap(i, j);
    }
    let mut pieces = vec![];
    let mut origin = [0; 3];
    for _ in 0..START_PIECES {
        push_piece(&mut pieces, &mut origin, 0, "straight", 400, false, [0; 3]);
    }
    if !mandatory(&mut pieces, &ids, &mut rng, &mut 0)? {
        return Ok(None);
    }
    // Required quantities are exactly one. Additional choices only use time left
    // after all mandatory pieces and a safe connection allowance are reserved.
    let target = u32::from(s.duration_seconds) * 1000;
    let required_size = pieces.len();
    let mut extensions = vec![];
    let mut growth_backtracks = 0;
    for _ in 0..MAX_PIECES {
        cancellation::checkpoint()?;
        let size = pieces.len();
        let added = if !ids.is_empty() && next(&mut rng) % 3 == 0 {
            block(
                &mut pieces,
                &ids[next(&mut rng) as usize % ids.len()],
                &mut rng,
            )
        } else {
            ordinary(&mut pieces, &mut rng)
        };
        if !added {
            // A trapped random walk is not a duration decision. Undo a bounded
            // number of complete extensions and draw another continuation.
            if growth_backtracks >= 32 {
                break;
            }
            let Some(previous) = extensions.pop() else {
                break;
            };
            pieces.truncate(previous);
            growth_backtracks += 1;
            continue;
        }
        if elapsed(&pieces) as u64 + connection_reserve(&pieces, s.circuit) * 1000 / SPEED as u64
            > u64::from(target)
        {
            pieces.truncate(size);
            break;
        }
        extensions.push(size);
    }
    if s.circuit {
        let mut connected = false;
        for _ in 0..16 {
            let size = pieces.len();
            if close(&mut pieces)? {
                if elapsed(&pieces) <= target || size == required_size {
                    connected = true;
                    break;
                }
            }
            pieces.truncate(size);
            let Some(previous) = extensions.pop() else {
                break;
            };
            pieces.truncate(previous);
        }
        if !connected {
            return Ok(None);
        }
    } else {
        let mut connected = false;
        for _ in 0..16 {
            let size = pieces.len();
            for _ in 0..8 {
                if append(&mut pieces, "finish_plaza", 400, false, [0; 3]) {
                    connected = elapsed(&pieces) <= target || size == required_size;
                    break;
                }
                if !ordinary(&mut pieces, &mut rng) {
                    break;
                }
            }
            if connected {
                break;
            }
            pieces.truncate(size);
            let Some(previous) = extensions.pop() else {
                break;
            };
            pieces.truncate(previous);
        }
        if !connected {
            return Ok(None);
        }
    }
    let mut a = finish(pieces, s);
    let (obstacles, eligible, target) = obstacles::place(&a)?;
    a.obstacles = obstacles;
    a.obstacle_eligible_length_cm = eligible;
    a.obstacle_target_count = target;
    match a.validate() {
        Ok(()) => Ok(Some(a)),
        Err(e) if e.code == "E_CANCELLED" => Err(e),
        Err(_) => Ok(None),
    }
}
pub(super) fn assemble(settings: &Settings) -> Result<Assembly> {
    let s = settings.normalized()?;
    let target = u32::from(s.duration_seconds) * 1000;
    let mut best: Option<Assembly> = None;
    for attempt in 0..24 {
        cancellation::checkpoint()?;
        let candidate = match candidate(&s, attempt) {
            Err(e) if e.code == "E_TRACK_OBSTACLES" => None,
            other => other?,
        };
        if let Some(a) = candidate {
            if best.as_ref().is_none_or(|b| {
                a.estimated_msec.abs_diff(target) < b.estimated_msec.abs_diff(target)
            }) {
                best = Some(a);
            }
        }
    }
    best.ok_or_else(||error(if s.gimmicks.iter().any(|id| id == "obstacles") { "E_TRACK_OBSTACLES" } else { "E_TRACK_BUDGET" },"no connected collision-free layout fits the bounded search, piece, sample, cell and memory budgets with every required gimmick"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn family_limit_includes_transitions_and_circuit_boundary() {
        let item = |id: &str, ordinary| {
            let mut p = variant(id, 400, 400, 400);
            p.ordinary = ordinary;
            p
        };
        let a = item("cylinder", false);
        let b = item("cylinder_wide_curve_left", false);
        let ramp = item("tube_exit", false);
        let approach = item("straight", false);
        assert!(!valid_runs(
            &[
                a.clone(),
                ramp.clone(),
                b.clone(),
                approach.clone(),
                a.clone()
            ],
            false
        ));
        let road = item("curve", true);
        assert!(valid_runs(
            &[a.clone(), b.clone(), road.clone(), a.clone()],
            false
        ));
        assert!(!valid_runs(
            &[a.clone(), road.clone(), a.clone(), b.clone()],
            true
        ));
        assert!(valid_runs(
            &[
                item("spiral_up", false),
                item("spiral_down", false),
                item("spiral_up", false)
            ],
            false
        ));
        assert!(valid_runs(&[a, b], false));
    }
    #[test]
    fn ordinary_slopes_have_one_metre_flat_ports_and_safe_inner_grade() {
        for id in CLOSING
            .iter()
            .filter(|id| id.ends_with("up") || id.ends_with("down"))
        {
            for width in [200, 400, 600] {
                let p = variant(id, width, 600, 600);
                let first = &p.path[0];
                let last = p.path.last().unwrap();
                assert_eq!(
                    last.position_cm[1],
                    if id.ends_with("down") { -100 } else { 100 }
                );
                assert_eq!(first.normal, [0, 1_000_000, 0]);
                assert_eq!(last.normal, first.normal);
                assert_eq!(first.forward, [0, 0, 1_000_000]);
                assert_eq!(last.forward[1], 0);
                assert_eq!(p.path[1].position_cm[1], first.position_cm[1]);
                assert_eq!(p.path[p.path.len() - 2].position_cm[1], last.position_cm[1]);
                for side in [-1.0, 1.0] {
                    let edge = |s: &Sample| {
                        let n = s.normal.map(|v| v as f64 / 1e6);
                        let f = s.forward.map(|v| v as f64 / 1e6);
                        let r = unit([
                            n[1] * f[2] - n[2] * f[1],
                            n[2] * f[0] - n[0] * f[2],
                            n[0] * f[1] - n[1] * f[0],
                        ]);
                        std::array::from_fn::<_, 3, _>(|j| {
                            s.position_cm[j] as f64 + side * r[j] as f64 / 1e6 * s.lateral_cm as f64
                        })
                    };
                    for pair in p.path.windows(2) {
                        let a = edge(&pair[0]);
                        let b = edge(&pair[1]);
                        let grade = (a[1] - b[1]).abs()
                            / ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                        assert!(
                            grade <= 0.23,
                            "{id} width={width} side={side} grade={grade}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn closest_time_among_bounded_valid_candidates_and_no_forced_helix() {
        for circuit in [true, false] {
            let s = Settings {
                seed: 7,
                circuit,
                duration_seconds: 60,
                gimmicks: vec!["spiral_up".into()],
                ..Settings::default()
            }
            .normalized()
            .unwrap();
            let selected = assemble(&s).unwrap();
            assert!(selected.pieces.iter().any(|p| p.id == "spiral_up"));
            assert!(selected.pieces.iter().all(|p| p.id != "spiral_down"));
            for attempt in 0..24 {
                if let Some(a) = candidate(&s, attempt).unwrap() {
                    assert!(
                        selected.estimated_msec.abs_diff(60_000)
                            <= a.estimated_msec.abs_diff(60_000)
                    );
                }
            }
        }
        let s = Settings {
            duration_seconds: 60,
            ..Settings::default()
        };
        let a = assemble(&s).unwrap();
        assert!(a.estimated_msec > 60_000);
        for id in s.gimmicks.iter().filter(|id| id.as_str() != "obstacles") {
            assert_eq!(
                a.pieces
                    .iter()
                    .filter(|p| family(&p.id) == family(id))
                    .count(),
                1,
                "over-budget mandatory quantity {id}"
            );
        }
    }
}
