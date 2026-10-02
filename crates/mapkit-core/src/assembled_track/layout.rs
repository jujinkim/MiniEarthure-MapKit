//! Bounded continuous-space extension and piece-based return routing.
use super::*;
pub(super) fn overlaps(a: &Piece, b: &Piece) -> bool {
    if !(0..3).all(|j| {
        a.reserved_min_cm[j] < b.reserved_max_cm[j] && b.reserved_min_cm[j] < a.reserved_max_cm[j]
    }) {
        return false;
    }
    a.path.iter().chain(&a.alternate_path).any(|s| {
        b.path
            .iter()
            .chain(&b.alternate_path)
            .any(|t| geometry::volume_overlap(s, t))
    })
}

pub(super) fn valid_runs(pieces: &[Piece], circuit: bool) -> bool {
    let seq: Vec<_> = pieces
        .iter()
        .filter(|p| {
            !["tube_entry", "tube_exit", "approach", "finish_plaza"].contains(&p.id.as_str())
        })
        .map(|p| {
            if category(&p.id) == "driving" {
                None
            } else {
                Some(family(&p.id))
            }
        })
        .collect();
    if seq.len() < 3 {
        return true;
    }
    (0..seq.len()).all(|i| {
        !circuit && i + 2 >= seq.len()
            || seq[i].is_none()
            || seq[i] != seq[(i + 1) % seq.len()]
            || seq[i] != seq[(i + 2) % seq.len()]
    })
}
fn end(pieces: &[Piece]) -> Sample {
    pieces.last().unwrap().path.last().unwrap().clone()
}
fn append(pieces: &mut Vec<Piece>, id: &str, w: u32) -> bool {
    let last = end(pieces);
    let yaw = round(
        libm::atan2(last.forward[0] as f64, last.forward[2] as f64) * 180000.0
            / std::f64::consts::PI,
    ) as i32;
    let mut p = variant(id, w, last.lateral_cm * 2, w);
    p.rotation_mdeg = [0, yaw, 0];
    p.quarter_turns = (yaw.rem_euclid(360000) / 90000) as u8;
    p.origin_cm = last.position_cm;
    if let Some(previous) = pieces.last() {
        p.origin_cm[1] -= portal_drop(&previous.id, id, w);
    }
    p = materialize(&p);
    if pieces
        .iter()
        .take(pieces.len().saturating_sub(1))
        .any(|other| authoring::conflict(&p, other))
    {
        return false;
    }
    pieces.push(p);
    if pieces.len() > MAX_PIECES
        || pieces.iter().map(|p| p.path.len()).sum::<usize>() > MAX_SAMPLES
        || !valid_runs(pieces, false)
    {
        pieces.pop();
        return false;
    }
    true
}
fn add_block(pieces: &mut Vec<Piece>, id: &str, w: u32) -> bool {
    let size = pieces.len();
    if ["loop", "banked_chicane"].contains(&id)
        && end(pieces).lateral_cm != 200
        && !append(pieces, "straight", 400)
    {
        return false;
    }
    if (!pipe_piece(id) || append(pieces, "tube_entry", w))
        && append(pieces, id, w)
        && (!pipe_piece(id) || append(pieces, "tube_exit", w))
    {
        true
    } else {
        pieces.truncate(size);
        false
    }
}
const MAX_SEAM_CM: u64 = 800;
const MAX_SEAM_CONTROLS_CM: u64 = 1600;
const CLOSURE_NODES: usize = 4096;

fn short_seam(last: &Sample, goal: &Sample) -> Option<Piece> {
    let gap=distance(last.position_cm,goal.position_cm);
    if !(50..=MAX_SEAM_CM).contains(&gap) {return None;}
    let direction=unit(std::array::from_fn(|j|(goal.position_cm[j]-last.position_cm[j]) as f64));
    let dot=|a: Vertex,b: Vertex| (0..3).map(|j|a[j] as f64*b[j] as f64/1e12).sum::<f64>();
    if dot(direction,last.forward)<0.5 || dot(direction,goal.forward)<0.5 {return None;}
    let reach=(gap/3).max(50) as i64;
    let mut p=variant("free_curve",400,last.lateral_cm*2,goal.lateral_cm*2);
    p.control_points=vec![last.position_cm,
        add(last.position_cm,last.forward.map(|v|v*reach/1_000_000)),
        add(goal.position_cm,goal.forward.map(|v|-v*reach/1_000_000)),goal.position_cm];
    if p.control_points.windows(2).map(|w|distance(w[0],w[1])).sum::<u64>()>MAX_SEAM_CONTROLS_CM {return None;}
    p=materialize(&p);
    if geometry::self_intersects(&p) || p.path.iter().any(|s|s.forward[1].abs()>230_000) {return None;}
    Some(p)
}

fn return_piece(previous: &Piece, id: &str, templates: &mut std::collections::BTreeMap<(String,u32),Piece>) -> Piece {
    let last=previous.path.last().unwrap();
    let yaw=round(libm::atan2(last.forward[0] as f64,last.forward[2] as f64)*180000.0/std::f64::consts::PI) as i32;
    let template=templates.entry((id.into(),last.lateral_cm*2)).or_insert_with(||materialize(&variant(id,400,last.lateral_cm*2,400)));
    let mut p=template.clone();
    p.origin_cm=last.position_cm;
    p.rotation_mdeg=[0,yaw,0];
    p.quarter_turns=(yaw.rem_euclid(360000)/90000) as u8;
    position_piece(template.clone(),&p)
}

fn closure(pieces: &mut Vec<Piece>, s: &Settings) -> Result<bool> {
    if !s.circuit {return Ok(append(pieces,"finish_plaza",400));}
    struct Node {piece: Piece,parent: Option<usize>,cost: u64,samples: usize,depth: usize}
    let goal=pieces[0].path[0].clone();
    let budget=(u32::from(s.duration_seconds)*1100).saturating_sub(pieces.iter().map(|p|p.reference_msec).sum::<u32>());
    let heuristic=|sample: &Sample| {
        let ahead=add(sample.position_cm,sample.forward.map(|v|v*500/1_000_000));
        distance(ahead,goal.position_cm)+sample.position_cm[1].abs_diff(goal.position_cm[1])*6
    };
    let key=|sample: &Sample| {
        let yaw=round(libm::atan2(sample.forward[0] as f64,sample.forward[2] as f64)*4.0/std::f64::consts::PI);
        (sample.position_cm[0].div_euclid(50),sample.position_cm[1].div_euclid(50),sample.position_cm[2].div_euclid(50),yaw)
    };
    let mut templates=std::collections::BTreeMap::new();
    let mut nodes=vec![Node{piece:pieces.last().unwrap().clone(),parent:None,cost:0,samples:pieces.iter().map(|p|p.path.len()+p.alternate_path.len()).sum(),depth:0}];
    let mut open=std::collections::BinaryHeap::new();
    open.push(std::cmp::Reverse((0u64,0usize)));
    let mut visited=std::collections::BTreeMap::new();
    visited.insert(key(nodes[0].piece.path.last().unwrap()),0u64);
    while let Some(std::cmp::Reverse((_,index)))=open.pop() {
        cancellation::checkpoint()?;
        let node=&nodes[index];
        let last=node.piece.path.last().unwrap();
        if visited.get(&key(last)).is_some_and(|cost|*cost<node.cost) {continue;}
        let mut chain=vec![];
        let mut parent=Some(index);
        while let Some(i)=parent {if i>0 {chain.push(i);} parent=nodes[i].parent;}
        let clear=|p: &Piece| {
            !pieces.iter().any(|other|authoring::conflict(p,other))
                && !chain.iter().any(|i|authoring::conflict(p,&nodes[*i].piece))
        };
        let seam=short_seam(last,&goal);
        let joined=authoring::joined(last,&goal);
        if node.depth>0 && (joined || seam.as_ref().is_some_and(|p|clear(p)
            && node.cost+u64::from(p.reference_msec)<=u64::from(budget)
            && node.samples+p.path.len()<=MAX_SAMPLES)) {
            chain.reverse();
            for i in chain {pieces.push(nodes[i].piece.clone());}
            if !joined {pieces.push(seam.unwrap());}
            return Ok(true);
        }
        if nodes.len()>=CLOSURE_NODES || node.depth>=64 || pieces.len()+node.depth+2>=MAX_PIECES {continue;}
        let mut choices=vec!["straight","right90","right90_left","gentle45","gentle45_left"];
        let height=last.position_cm[1]-goal.position_cm[1];
        if height>0 {choices.push("slope_down");}
        if height<0 {choices.push("slope_up");}
        if height>=800 {choices.push("spiral360_right_down");choices.push("spiral360_left_down");}
        if height<= -800 {choices.push("spiral360_right_up");choices.push("spiral360_left_up");}
        let mut next=vec![];
        for id in choices {
            cancellation::checkpoint()?;
            let p=return_piece(&node.piece,id,&mut templates);
            let cost=node.cost+u64::from(p.reference_msec);
            let endpoint=p.path.last().unwrap();
            let k=key(endpoint);
            let minimum=distance(endpoint.position_cm,goal.position_cm).max(endpoint.position_cm[1].abs_diff(goal.position_cm[1])*6)*1000/SPEED as u64;
            if cost+minimum>u64::from(budget)+64 || node.samples+p.path.len()>MAX_SAMPLES
                || visited.get(&k).is_some_and(|old|*old<=cost) || !clear(&p) {continue;}
            let score=cost+heuristic(endpoint)*1800/SPEED as u64;
            next.push((p,k,cost,score,node.samples,node.depth));
        }
        for (p,k,cost,score,samples,depth) in next {
            if nodes.len()>=CLOSURE_NODES {break;}
            visited.insert(k,cost);
            let next_index=nodes.len();
            nodes.push(Node{samples:samples+p.path.len(),piece:p,parent:Some(index),cost,depth:depth+1});
            open.push(std::cmp::Reverse((score,next_index)));
        }
    }
    Ok(false)
}
pub(super) fn finish(pieces: Vec<Piece>, s: &Settings) -> Assembly {
    let stats = statistics(&pieces);
    let route = authoring::Route {
        id: "base".into(),
        pieces: (0..pieces.len()).collect(),
        estimated_msec: stats.3,
    };
    Assembly {
        authoring: None,
        seed_source: None,
        routes: vec![route],
        issues: vec![],
        settings: s.clone(),
        generator_fingerprint: fingerprint(),
        catalogue_fingerprint: catalogue_fingerprint(),
        finish_plaza: finish_plaza(&pieces),
        pieces,
        supports: vec![],
        obstacles: vec![],
        obstacle_eligible_length_cm: 0,
        obstacle_target_count: 0,
        length_cm: stats.0,
        ordinary_length_cm: stats.1,
        ordinary_straight_cm: stats.2,
        estimated_msec: stats.3,
        floor: stats.4,
    }
}
fn candidate(s: &Settings, attempt: u64) -> Result<Option<Assembly>> {
    let mut rng = s.seed ^ attempt.wrapping_mul(0xa0761d6478bd642f);
    let driving = [
        "straight",
        "slope_up",
        "slope_down",
        "zigzag",
        "gentle45",
        "gentle45_left",
        "gentle90",
        "gentle90_left",
        "right90",
        "right90_left",
        "sharp135",
        "sharp135_left",
        "hairpin",
        "hairpin_left",
        "spiral90_right_down",
        "spiral90_left_up",
        "spiral90_right_up",
        "spiral90_left_down",
        "spiral180_right_up",
        "spiral180_left_down",
        "spiral180_right_down",
        "spiral180_left_up",
        "spiral360_right_down",
        "spiral360_left_up",
        "spiral360_right_up",
        "spiral360_left_down",
    ];
    let gimmicks = [
        "cylinder",
        "cylinder_curve",
        "cylinder_curve_left",
        "cylinder_uturn",
        "cylinder_uturn_left",
        "cylinder_s_rise",
        "overpass",
        "banked_chicane",
        "loop",
    ];
    let actions = [
        "jump_panel",
        "jump",
        "acceleration_panel",
        "boost_chain",
        "air_ring",
    ];
    let mut choices = vec![];
    for (category, ids) in [
        ("driving", driving.as_slice()),
        ("gimmick", gimmicks.as_slice()),
        ("action", actions.as_slice()),
    ] {
        if s.categories.iter().any(|id| id == category) {
            choices.extend_from_slice(ids);
        }
    }
    let mut pieces = vec![];
    let mut origin = [0; 3];
    for _ in 0..START_PIECES {
        push_piece(&mut pieces, &mut origin, 0, "straight", 400, false, [0; 3]);
    }
    let shortcut = s.categories.len() == 3 && attempt % 3 == 0;
    if shortcut {
        let example = authoring::shortcut_source();
        for i in &example.instances[3..5] {
            pieces.push(authoring::piece(i)?);
        }
    }
    let target = u32::from(s.duration_seconds) * 1000;
    let mut best: Option<Assembly> = None;
    let mut backtracks = 0;
    let mut closure_after=target/3;
    for _ in 0..MAX_PIECES {
        cancellation::checkpoint()?;
        let elapsed: u32 = pieces.iter().map(|p| p.reference_msec).sum();
        if elapsed > target * 11 / 10 {
            pieces.pop();
            break;
        }
        if elapsed > closure_after {
            closure_after=elapsed+4000;
            let mut connected = pieces.clone();
            if closure(&mut connected, s)? {
                let a = finish(connected, s);
                if best.as_ref().is_none_or(|b| {
                    a.estimated_msec.abs_diff(target) < b.estimated_msec.abs_diff(target)
                }) {
                    best = Some(a);
                }
                if best
                    .as_ref()
                    .is_some_and(|a| a.estimated_msec.abs_diff(target) < 300)
                {
                    break;
                }
            }
        }
        let mut added = false;
        for _ in 0..12 {
            let id = choices[next(&mut rng) as usize % choices.len()];
            let widths = supported_widths(id);
            let w = widths[next(&mut rng) as usize % widths.len()];
            if add_block(&mut pieces, id, w) {
                added = true;
                break;
            }
        }
        if !added {
            if pieces.len() <= START_PIECES || backtracks >= 32 {
                break;
            }
            pieces.pop();
            backtracks += 1;
        }
    }
    if let Some(mut a) = best {
        let (obstacles, eligible, target) = obstacles::place(&a)?;
        a.obstacles = obstacles;
        a.obstacle_eligible_length_cm = eligible;
        a.obstacle_target_count = target;
        grounding::apply(&mut a)?;
        authoring::checkpoint_budget(&a)?;
        if a.validate().is_ok() {
            if shortcut {
                return match authoring::seed_shortcut(&a) {
                    Ok(graph) => Ok(Some(graph)),
                    Err(e) if e.code == "E_CANCELLED" || e.code == "E_TRACK_SUPPORT" || e.code == "E_TRACK_CHECKPOINT_LIMIT" => Err(e),
                    Err(_) => Ok(None),
                };
            }
            return Ok(Some(a));
        }
    }
    Ok(None)
}
pub(super) fn assemble(settings: &Settings) -> Result<Assembly> {
    let s = settings.normalized()?;
    let target = u32::from(s.duration_seconds) * 1000;
    let mut best: Option<Assembly> = None;
    let mut support_failure = None;
    let mut checkpoint_failure = None;
    for attempt in 0..24 {
        cancellation::checkpoint()?;
        let candidate = match candidate(&s, attempt) {
            Err(e) if e.code == "E_TRACK_CHECKPOINT_LIMIT" => { checkpoint_failure=Some(e); continue; }
            Err(e) if e.code == "E_TRACK_SUPPORT" => { support_failure = Some(e); continue; }
            other => other?,
        };
        if let Some(a) = candidate {
            if best.as_ref().is_none_or(|b| {
                a.estimated_msec.abs_diff(target) < b.estimated_msec.abs_diff(target)
            }) {
                best = Some(a);
            }
            // A deterministic one-percent result needs no more candidate search.
            if best.as_ref().is_some_and(|a|a.estimated_msec.abs_diff(target)<=target/100) {break;}
        }
    }
    if best.is_none() { if let Some(e)=checkpoint_failure {return Err(e);} }
    if best.is_none() { if let Some(e) = support_failure { return Err(error("E_TRACK_SUPPORT", format!("Requested {} s; no valid result in 24 layout candidates: {}", s.duration_seconds, e.message))); } }
    let a=best.ok_or_else(||error("E_TRACK_DURATION",format!("Requested {} s; closest unavailable: no connected layout within search/resource budget",s.duration_seconds)))?;
    if a.estimated_msec.abs_diff(target) > target / 10 {
        return Err(error(
            "E_TRACK_DURATION",
            format!(
                "Requested {} s; closest {:.1} s: no result within ±10% in 24 candidates",
                s.duration_seconds,
                a.estimated_msec as f64 / 1000.0
            ),
        ));
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn return_route_uses_modules_and_only_short_seams() {
        let mut pieces=vec![];
        let mut origin=[0;3];
        for _ in 0..START_PIECES {push_piece(&mut pieces,&mut origin,0,"straight",400,false,[0;3]);}
        let start=pieces.len();
        assert!(closure(&mut pieces,&Settings::default()).unwrap());
        assert!(pieces[start..].iter().filter(|p|p.id!="free_curve").count()>=3);
        for p in &pieces[start..] {
            assert_eq!(*p,materialize(p));
            if p.id=="free_curve" {assert!(p.control_points.windows(2).map(|w|distance(w[0],w[1])).sum::<u64>()<=MAX_SEAM_CONTROLS_CM);}
        }
        assert!(authoring::joined(pieces.last().unwrap().path.last().unwrap(),&pieces[0].path[0]));
        assert!(short_seam(&pieces[START_PIECES-1].path[0],&pieces[0].path[0]).is_none());
    }
    #[test]
    fn return_search_failure_and_cancellation_preserve_input() {
        let mut pieces=vec![];
        let mut origin=[0;3];
        for _ in 0..START_PIECES {push_piece(&mut pieces,&mut origin,0,"straight",400,false,[0;3]);}
        let before=pieces.clone();
        let mut settings=Settings::default();
        settings.duration_seconds=0;
        assert!(!closure(&mut pieces,&settings).unwrap());
        assert_eq!(pieces,before);
        let token=cancellation::CancellationToken::default();
        token.cancel();
        assert_eq!(token.run(||closure(&mut pieces,&Settings::default())).unwrap_err().code,"E_CANCELLED");
        assert_eq!(pieces,before);
    }
    #[test]
    fn automatic_composition_is_a_graph_candidate() {
        let mut found = false;
        for seed in 1..=8 {
            let settings = Settings {
                seed,
                circuit: false,
                duration_seconds: 90,
                ..Default::default()
            }
            .normalized()
            .unwrap();
            if let Some(a) = candidate(&settings, 0).unwrap() {
                assert!(a.seed_source.is_some());
                assert_eq!(a.routes.len(), 2);
                found = true;
                break;
            }
        }
        assert!(found, "bounded seed search includes the composed shortcut");
    }
}
