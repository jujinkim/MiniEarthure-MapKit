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
    // This is a generation constraint, not a restriction on authored actions or
    // stored packages. A normal driving piece separates acceleration and jump.
    for i in 0..pieces.len() {
        if !circuit && i + 1 == pieces.len() { break; }
        if ["acceleration_panel", "boost_chain"].contains(&pieces[i].id.as_str())
            && ["jump_panel", "jump", "offset_jump", "air_ring"]
                .contains(&pieces[(i + 1) % pieces.len()].id.as_str()) {
            return false;
        }
    }
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
    // The exit transition widens back into an ordinary road.
    let exit = if id == "tube_exit" && pieces.last().is_some_and(|p| p.id.starts_with("cylinder")) { w * 2 } else { w };
    let mut p = variant(id, w, last.lateral_cm * 2, exit);
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
    if ["loop", "banked_chicane", "overpass"].contains(&id)
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
    let mut p=variant("free_curve",800,last.lateral_cm*2,goal.lateral_cm*2);
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
    let template=templates.entry((id.into(),last.lateral_cm*2)).or_insert_with(||materialize(&variant(id,800,last.lateral_cm*2,800)));
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
        geometry_issues: vec![],
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
// The immutable section plan supplies independent family/category/width weights.
const DRIVING_FAMILIES: [&[&str]; 13] = [
    &["straight", "sprint_lane"],
    &["slope_up", "slope_down", "slope", "roller_waves", "curve_up", "curve_down", "curve_left_up", "curve_left_down"],
    &["gentle45", "gentle45_left", "gentle90", "gentle90_left"],
    &["right90", "right90_left"],
    &["zigzag", "chicane", "chicane_narrow", "zigzag_narrow", "straight_narrow"],
    &["sharp135", "sharp135_left"],
    &["hairpin", "hairpin_left"],
    &["spiral90_right_down", "spiral90_left_up", "spiral90_right_up", "spiral90_left_down"],
    &["spiral180_right_up", "spiral180_left_down", "spiral180_right_down", "spiral180_left_up"],
    &["spiral360_right_down", "spiral360_left_up", "spiral360_right_up", "spiral360_left_down"],
    &["banked_hairpin", "banked_hairpin_left"],
    &["spatial_s", "spatial_s_left"],
    &["curve", "curve_left", "sharp_curve", "sharp_curve_left"],
];
fn weighted_index(rng: &mut u64, weights: &[u32]) -> usize {
    let mut ticket = (next(rng) % u64::from(weights.iter().sum::<u32>())) as u32;
    for (index, weight) in weights.iter().enumerate() {
        if ticket < *weight { return index; }
        ticket -= weight;
    }
    unreachable!("nonempty positive weight distribution")
}
fn driving_choice(rng: &mut u64, section: &plan::Section) -> &'static str {
    let family = DRIVING_FAMILIES[weighted_index(rng, &section.families)];
    let variants:Vec<_>=family.iter().copied().filter(|id| {
        (!family.iter().any(|v|v.contains("left")) || id.contains("left")==section.left)
            && (!id.ends_with("up") && !id.ends_with("down") || id.ends_with("up")==section.up)
    }).collect();
    variants[next(rng) as usize % variants.len()]
}
fn road_width(rng: &mut u64, id: &str, section: &plan::Section) -> u32 {
    let widths = if ULTRA_WIDE_PRESETS.iter().any(|(_, base)| *base == id) { WIDTHS } else { supported_widths(id) };
    let weights: Vec<_> = widths.iter().map(|w| section.widths[WIDTHS.iter().position(|v| v == w).unwrap()]).collect();
    widths[weighted_index(rng, &weights)]
}
fn driving_piece(rng: &mut u64, section: &plan::Section) -> (&'static str, u32) {
    let id = driving_choice(rng, section);
    let width = road_width(rng, id, section);
    let preset = if width == 1800 {
        ULTRA_WIDE_PRESETS.iter().find(|(_, base)| *base == id).map_or(id, |(wide, _)| *wide)
    } else if width == 1200 {
        EXTRA_WIDE_PRESETS.iter().find(|(_, base)| *base == id).map_or(id, |(wide, _)| *wide)
    } else { id };
    (preset, width)
}
fn special_width(rng: &mut u64, id: &str) -> u32 {
    // Same bore choices as manual authoring.
    let widths = supported_widths(id);
    widths[next(rng) as usize % widths.len()]
}
fn candidate(s: &Settings, plan: &plan::Plan, attempt: u64) -> Result<Option<Assembly>> {
    let mut rng = s.seed ^ attempt.wrapping_mul(0xa0761d6478bd642f);

    let gimmicks = [
        "cylinder",
        "cylinder_curve",
        "cylinder_curve_left",
        "cylinder_uturn",
        "cylinder_uturn_left",
        "cylinder_s_rise",
        "cylinder_spiral_right_up",
        "cylinder_spiral_right_down",
        "cylinder_spiral_left_up",
        "cylinder_spiral_left_down",
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
        "offset_jump",
        "double_jump",
        "air_ring_chain",
    ];
    let mut pieces = vec![];
    let mut origin = [0; 3];
    for _ in 0..START_PIECES {
        push_piece(&mut pieces, &mut origin, 0, "straight", 800, false, [0; 3]);
    }
    let mut fragments:Vec<(usize,authoring::Source)>=vec![];
    let mut blocks=vec![];
    if plan.shortcut && s.categories.len()==3 {
        let fragment=templates::placed("grind_shortcut",&end(&pieces))?;
        let start=pieces.len();
        for id in &fragment.paths[0].pieces {
            pieces.push(authoring::piece(fragment.instances.iter().find(|i|&i.id==id).unwrap())?);
        }
        blocks.push(start);fragments.push((start,fragment));
    }
    let target = u32::from(s.duration_seconds) * 1000;
    let mut best: Option<(Assembly,Vec<(usize,authoring::Source)>)> = None;
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
                // Keep duration feasibility and the section-plan residual together.
                if authoring::checkpoint_budget(&a).is_ok() && best.as_ref().is_none_or(|b| {
                    plan.rank(&a) < plan.rank(&b.0)
                }) {
                    best = Some((a,fragments.clone()));
                }
                if best
                    .as_ref()
                    .is_some_and(|(a,_)| a.estimated_msec.abs_diff(target) < 300 && plan.fit(a)<65)
                {
                    break;
                }
            }
        }
        let mut added = false;
        for _ in 0..12 {
            let section=plan.section(elapsed,target);
            let mut category_weights=section.categories;
            for (index,id) in selection_ids().iter().enumerate() {
                if !s.categories.iter().any(|v|v==id) {category_weights[index]=0;}
            }
            // A category-only request remains usable even when its seed weight is zero.
            if category_weights.iter().sum::<u32>()==0 {
                for (i,id) in selection_ids().iter().enumerate() {category_weights[i]=u32::from(s.categories.iter().any(|v|v==id));}
            }
            let choice=match weighted_index(&mut rng,&category_weights) {
                0=>"driving",1=>gimmicks[next(&mut rng) as usize%gimmicks.len()],
                _=>actions[next(&mut rng) as usize%actions.len()],
            };
            let start=pieces.len();
            if ["double_jump","air_ring_chain"].contains(&choice) {
                let fragment=templates::placed(choice,&end(&pieces))?;
                let mut valid=true;
                for id in &fragment.paths[0].pieces {
                    let p=authoring::piece(fragment.instances.iter().find(|i|&i.id==id).unwrap())?;
                    if pieces.iter().any(|other|authoring::conflict(&p,other)) {valid=false;break;}
                    pieces.push(p);
                }
                if valid && pieces.len()<=MAX_PIECES && pieces.iter().map(|p|p.path.len()).sum::<usize>()<=MAX_SAMPLES && valid_runs(&pieces,false) {
                    blocks.push(start);fragments.push((start,fragment));added=true;break;
                }
                pieces.truncate(start);continue;
            }
            let (id, w) = if choice == "driving" { driving_piece(&mut rng, section) }
                else if category(choice) == "action" { (choice, road_width(&mut rng, choice, section)) }
                else { (choice, special_width(&mut rng, choice)) };
            if add_block(&mut pieces, id, w) {
                blocks.push(start);added = true;break;
            }
        }
        if !added {
            if pieces.len() <= START_PIECES || backtracks >= 32 {
                break;
            }
            let start=blocks.pop().unwrap_or(START_PIECES);
            pieces.truncate(start);fragments.retain(|(at,_)|*at<start);
            backtracks += 1;
        }
    }
    if let Some((mut a,fragments)) = best {
        if !valid_runs(&a.pieces,s.circuit) || a.pieces.iter().any(|p|geometry::self_intersects(p) || !geometry::ordinary_grade_valid(p)) {return Ok(None);}
        let (obstacles, eligible, target) = obstacles::place(&a)?;
        a.obstacles=obstacles;a.obstacle_eligible_length_cm=eligible;a.obstacle_target_count=target;
        // A composed module reserves its full approach, recovery and merge.
        a.obstacles.retain(|obstacle| !fragments.iter().any(|(start, fragment)|
            (*start..*start + fragment.paths[0].pieces.len()).contains(&obstacle.piece_index)));
        for (start,fragment) in &fragments {
            let outside:Vec<_>=a.pieces.iter().enumerate().filter(|(index,_)|
                !(*start..*start+fragment.paths[0].pieces.len()).contains(index)).map(|(_,p)|p.clone()).collect();
            if !templates::clear_of_roads(fragment,&outside)? {return Ok(None);}
        }
        let mut source=authoring::from_assembly(&a);
        for (start,fragment) in fragments {
            let ids:Vec<_>=(start..start+fragment.paths[0].pieces.len()).map(|i|source.instances[i].id.clone()).collect();
            templates::merge(&mut source,&fragment,&ids,&format!("t{start}"));
        }
        source.checkpoints.clear();
        let compiled=authoring::compile(&source)?;
        if !compiled.geometry_issues.is_empty() {return Ok(None);}
        source.checkpoints=authoring::automatic_checkpoints(&compiled).into_iter().map(|(i,sample)|authoring::Checkpoint{piece:source.instances[i].id.clone(),sample}).collect();
        if source.checkpoints.is_empty() {return Ok(None);}
        source.checkpoints[0]=authoring::Checkpoint{piece:source.instances[2].id.clone(),sample:compiled.pieces[2].path.len()-1};
        source.checkpoints.dedup();
        let mut out=authoring::compile(&source)?;
        if authoring::executable(&out).is_err() {return Ok(None);}
        authoring::checkpoint_budget(&out)?;
        out.authoring=None;out.seed_source=Some(source);
        out.validate()?;
        return Ok(Some(out));
    }
    Ok(None)
}
pub(super) fn assemble(settings: &Settings) -> Result<Assembly> {
    let mut s = settings.normalized()?;
    s.difficulty="normal".into();
    s.time_minutes=track_seed_defaults(s.seed)?["time_minutes"].as_u64().unwrap() as u16;
    let plan=plan::Plan::new(s.seed);
    let target = u32::from(s.duration_seconds) * 1000;
    let mut best: Option<Assembly> = None;
    let mut support_failure = None;
    let mut checkpoint_failure = None;
    for attempt in 0..24 {
        cancellation::checkpoint()?;
        let candidate = match candidate(&s, &plan, attempt) {
            Err(e) if e.code == "E_TRACK_CLEARANCE" || e.code == "E_TRACK_GEOMETRY" || e.code == "E_TRACK_DRAFT" => continue,
            Err(e) if e.code == "E_TRACK_CHECKPOINT_LIMIT" => { checkpoint_failure=Some(e); continue; }
            Err(e) if e.code == "E_TRACK_SUPPORT" => { support_failure = Some(e); continue; }
            other => other?,
        };
        if let Some(a) = candidate {
            if best.as_ref().is_none_or(|b| {
                plan.rank(&a) < plan.rank(b)
            }) {
                best = Some(a);
            }
            // Stop only when duration and the intended section flow both fit.
            if best.as_ref().is_some_and(|a|a.estimated_msec.abs_diff(target)<=target/100 && plan.fit(a)<65) {break;}
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
    fn acceleration_cannot_lead_directly_into_generated_jump() {
        for boost in ["acceleration_panel", "boost_chain"] {
            for jump in ["jump_panel", "jump", "offset_jump", "air_ring"] {
                let pair = vec![variant(boost,400,400,400),variant(jump,400,400,400)];
                assert!(!valid_runs(&pair,false));
                let mut seam = pair.clone(); seam.reverse();
                assert!(valid_runs(&seam,false));
                assert!(!valid_runs(&seam,true));
                let mut separated = pair.clone();
                separated.insert(1,variant("straight",400,400,400));
                assert!(valid_runs(&separated,true));
                let mut pieces = vec![materialize(&pair[0])];
                let before = pieces.clone();
                assert!(!append(&mut pieces,jump,400));
                assert_eq!(pieces,before);
                assert!(append(&mut pieces,"straight",400));
                assert!(append(&mut pieces,jump,400));
            }
        }
    }
    #[test]
    fn seed_plans_preserve_geometry_axes_across_attempts() {
        let plans:Vec<_>=(0..64).map(plan::Plan::new).collect();
        assert!(plans.iter().any(|p|p.sections[0].families[0]>200));
        assert!(plans.iter().any(|p|p.sections[0].families[0]<25));
        assert!(plans.iter().any(|p|p.sections[0].categories[2]>30));
        assert!(plans.iter().any(|p|p.sections[0].categories[2]<3));
        for plan in plans {for section in plan.sections {
            let mut rng=42;
            for _ in 0..100 {
                let (id,width)=driving_piece(&mut rng,&section);
                assert!(supported_widths(id).contains(&width));
                let p=materialize(&variant(id,width,width,width));
                assert!(!geometry::self_intersects(&p),"{id}/{width}");
            }
        }}
    }
}
