//! Circuit closure: an analytic cubic seam into the start, otherwise a bounded
//! A* over a small return alphabet that retries the seam after each expansion.
use super::*;
use grow::{straight_run, Grow, MAX_STRAIGHT_CM};
use std::collections::{BTreeMap, BinaryHeap};

const SEAM_CM: u64 = 6000;
const SEAM_WIDTH: u32 = 800;
const MAX_GRADE: i64 = 230_000;
const NODES: usize = 1024;
const DEPTH: usize = 16;
const RETURN_WIDTH: u32 = 600;
/// A* only runs this close to the start, and at most every few pieces.
const SEARCH_CM: u64 = 9_000;
const SEARCH_EVERY: usize = 3;
const SEARCHES: usize = 6;

fn dot(a: Vertex, b: [f64; 3]) -> f64 { (0..3).map(|j| a[j] as f64 / 1e6 * b[j]).sum() }
/// Minimum horizontal radius of curvature of one cubic.
fn radius(cp: &[Vertex]) -> f64 {
    let p = |i: usize, j: usize| cp[i][j] as f64;
    (0..=32).map(|n| {
        let (t, u) = (n as f64 / 32.0, 1.0 - n as f64 / 32.0);
        let d1 = |j| 3.0 * u * u * (p(1, j) - p(0, j)) + 6.0 * u * t * (p(2, j) - p(1, j)) + 3.0 * t * t * (p(3, j) - p(2, j));
        let d2 = |j| 6.0 * u * (p(2, j) - 2.0 * p(1, j) + p(0, j)) + 6.0 * t * (p(3, j) - 2.0 * p(2, j) + p(1, j));
        let speed = libm::hypot(d1(0), d1(2));
        let cross = (d1(0) * d2(2) - d1(2) * d2(0)).abs();
        if speed < 1e-6 { 0.0 } else if cross < 1e-9 { f64::INFINITY } else { speed * speed * speed / cross }
    }).fold(f64::INFINITY, f64::min)
}
/// A free curve from `from` into `goal` (≤60 m, radius ≥ w/2+400, grade ≤23%,
/// no self intersection). Clearance against other pieces is the caller's.
pub(super) fn seam(cache: &mut piece_cache::PieceCache, from: &Sample, goal: &Sample) -> Option<Piece> {
    let gap = distance(from.position_cm, goal.position_cm);
    if !(100..=SEAM_CM).contains(&gap) { return None; }
    let toward: [f64; 3] = std::array::from_fn(|j| (goal.position_cm[j] - from.position_cm[j]) as f64 / gap as f64);
    if dot(from.forward, toward) < 0.0 || dot(goal.forward, toward) < 0.0 { return None; }
    let minimum = f64::from(SEAM_WIDTH) / 2.0 + 400.0;
    for k in [0.36, 0.28, 0.45, 0.22, 0.55] {
        let reach = round(gap as f64 * k);
        let cp = vec![from.position_cm, add(from.position_cm, from.forward.map(|v| v * reach / 1_000_000)),
            add(goal.position_cm, goal.forward.map(|v| -v * reach / 1_000_000)), goal.position_cm];
        if radius(&cp) < minimum { continue; }
        let p = cache.instance(&authoring::Instance { entry_width_cm: from.lateral_cm * 2, exit_width_cm: goal.lateral_cm * 2, control_points: cp,
            ..authoring::instance("", "free_curve", SEAM_WIDTH) });
        if race_length(&p) > SEAM_CM || p.path.iter().any(|s| s.forward[1].abs() > MAX_GRADE) || geometry::self_intersects(&p) { continue; }
        return Some(p);
    }
    None
}
struct Node { piece: Piece, parent: usize, depth: usize, cost: u64, tail: u64, samples: usize }
/// Close the circuit from the current end, appending the return pieces on success.
pub(super) fn close(g: &mut Grow) -> Result<bool> {
    let goal = g.pieces[0].path[0].clone();
    let last = g.pieces.len() - 1;
    let end = g.end().clone();
    if let Some(p) = seam(&mut g.cache, &end, &goal) {
        if straight_run(&p, g.c.tail).0 <= MAX_STRAIGHT_CM && g.admissible(&p, g.c.tail, g.c.samples, g.pieces.len()) && g.clear(&p) {
            g.commit(p);
            return Ok(true);
        }
    }
    let toward: [f64; 3] = std::array::from_fn(|j| (goal.position_cm[j] - end.position_cm[j]) as f64);
    let gap = distance(end.position_cm, goal.position_cm);
    // Search only from behind the start, near the approach pose and not facing away.
    let approach = distance(end.position_cm, [goal.position_cm[0], goal.position_cm[1], goal.position_cm[2] - 4000]);
    if g.pieces.len() < g.next_search || g.stats.searches >= SEARCHES || approach > SEARCH_CM || end.position_cm[2] > goal.position_cm[2] - 1000
        || dot(end.forward, toward) < -0.3 * gap as f64 { return Ok(false); }
    // Later searches of one attempt get a quarter of the node budget.
    let limit = if g.stats.searches == 0 { NODES } else { NODES / 4 };
    cancellation::progress("closing", g.pieces.len() as u64, None, "pieces");
    g.next_search = g.pieces.len() + SEARCH_EVERY;
    g.stats.searches += 1;
    // Aim 20 m behind the start, facing it: any such pose leaves a seam.
    let aim = add(goal.position_cm, goal.forward.map(|v| -v * 2000 / 1_000_000));
    let heuristic = |s: &Sample| {
        let ahead = add(s.position_cm, s.forward.map(|v| v * 500 / 1_000_000));
        let facing = (1.0 - dot(s.forward, goal.forward.map(|v| v as f64 / 1e6))) * 1500.0;
        (distance(ahead, aim) + s.position_cm[1].abs_diff(goal.position_cm[1]) * 6 + facing as u64) * 1800 / SPEED as u64
    };
    let key = |s: &Sample| (s.position_cm.map(|v| v.div_euclid(100)), (grow::yaw_mdeg(s.forward) as f64 / 22500.0).round() as i64, s.lateral_cm);
    let mut nodes = vec![Node { piece: g.pieces[last].clone(), parent: usize::MAX, depth: 0, cost: 0, tail: g.c.tail, samples: g.c.samples }];
    let mut open = BinaryHeap::from([std::cmp::Reverse((0u64, 0usize))]);
    let mut visited = BTreeMap::from([(key(&end), 0u64)]);
    let chain = |nodes: &[Node], mut i: usize| { let mut out = vec![]; while i != 0 { out.push(i); i = nodes[i].parent; } out };
    while let Some(std::cmp::Reverse((_, index))) = open.pop() {
        cancellation::checkpoint()?;
        let ancestors = chain(&nodes, index);
        // Clearance is checked when a node is expanded, never for unexpanded children:
        // clear of every placed piece, including the one it joins (as compile checks),
        // and of its branch except its own parent.
        if index > 0 {
            let p = &nodes[index].piece;
            if !g.clear(p)
                || ancestors.iter().skip(2).any(|&i| track_grid::near(p, &nodes[i].piece) && authoring::conflict(p, &nodes[i].piece)) {
                let k = key(p.path.last().unwrap());
                if visited.get(&k) == Some(&nodes[index].cost) { visited.remove(&k); }
                continue;
            }
        }
        let node_end = nodes[index].piece.path.last().unwrap().clone();
        if index > 0 {
            if let Some(p) = seam(&mut g.cache, &node_end, &goal) {
                let n = &nodes[index];
                let chained = ancestors.iter().all(|&i| !track_grid::near(&p, &nodes[i].piece) || !authoring::conflict(&p, &nodes[i].piece));
                if straight_run(&p, n.tail).0 <= MAX_STRAIGHT_CM && g.admissible(&p, n.tail, n.samples, g.pieces.len() + n.depth) && chained && g.clear(&p) {
                    g.stats.nodes += nodes.len();
                    for &i in ancestors.iter().rev() { let piece = nodes[i].piece.clone(); g.commit(piece); }
                    g.commit(p);
                    return Ok(true);
                }
            }
        }
        if nodes[index].depth >= DEPTH || nodes.len() >= limit { continue; }
        let height = node_end.position_cm[1] - goal.position_cm[1];
        let mut alphabet = vec!["straight", "gentle45", "gentle45_left", "right90", "right90_left"];
        if height > 0 { alphabet.extend(["slope_down", "spiral90_right_down", "spiral90_left_down"]); }
        if height < 0 { alphabet.extend(["slope_up", "spiral90_right_up", "spiral90_left_up"]); }
        let previous = nodes[index].piece.id.clone();
        let mut children = vec![];
        for id in alphabet {
            let p = g.piece(id, RETURN_WIDTH, None, &[], &node_end, &previous);
            let n = &nodes[index];
            let tail = straight_run(&p, n.tail).1;
            let cost = n.cost + u64::from(p.reference_msec);
            let k = key(p.path.last().unwrap());
            if !g.admissible(&p, n.tail, n.samples, g.pieces.len() + n.depth) || visited.get(&k).is_some_and(|c| *c <= cost) { continue; }
            let score = cost + heuristic(p.path.last().unwrap());
            children.push((p, k, cost, score, tail));
        }
        for (p, k, cost, score, tail) in children {
            if nodes.len() >= limit { break; }
            visited.insert(k, cost);
            let n = &nodes[index];
            let (depth, samples) = (n.depth + 1, n.samples + p.path.len() + p.alternate_path.len());
            nodes.push(Node { piece: p, parent: index, depth, cost, tail, samples });
            open.push(std::cmp::Reverse((score, nodes.len() - 1)));
        }
    }
    g.stats.nodes += nodes.len();
    Ok(false)
}
