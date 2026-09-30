//! Bounded continuous-space extension, backtracking and direct cubic closure.
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
fn closure(pieces: &mut Vec<Piece>, s: &Settings) -> bool {
    if !s.circuit {
        return append(pieces, "finish_plaza", 400);
    }
    let end = end(pieces);
    let goal = &pieces[0].path[0];
    let reach = (distance(end.position_cm, goal.position_cm) as i64 / 2).max(2400);
    let approach = add(end.position_cm, end.forward.map(|v| v * reach / 1_000_000));
    let departure = add(
        goal.position_cm,
        goal.forward.map(|v| -v * reach / 1_000_000),
    );
    let delta = unit(std::array::from_fn(|j| {
        (goal.position_cm[j] - end.position_cm[j]) as f64
    }));
    let offset = (reach / 2).max(2400);
    let side = if s.seed % 2 == 0 { 1 } else { -1 };
    let middle = std::array::from_fn(|j| {
        (end.position_cm[j] + goal.position_cm[j]) / 2
            + match j {
                0 => delta[2] * offset * side / 1_000_000,
                2 => -delta[0] * offset * side / 1_000_000,
                _ => 0,
            }
    });
    let before = add(middle, delta.map(|v| -v * reach / 1_000_000));
    let after = add(middle, delta.map(|v| v * reach / 1_000_000));
    let mut best: Option<Piece> = None;
    let elapsed: u32 = pieces.iter().map(|p| p.reference_msec).sum();
    let target = u32::from(s.duration_seconds) * 1000;
    for controls in [
        vec![end.position_cm, approach, departure, goal.position_cm],
        vec![
            end.position_cm,
            approach,
            before,
            middle,
            after,
            departure,
            goal.position_cm,
        ],
    ] {
        let mut p = variant("free_curve", 400, end.lateral_cm * 2, goal.lateral_cm * 2);
        p.control_points = controls;
        p = materialize(&p);
        if geometry::self_intersects(&p)
            || pieces
                .iter()
                .enumerate()
                .any(|(i, other)| i > 0 && i + 1 < pieces.len() && authoring::conflict(&p, other))
        {
            continue;
        }
        if best.as_ref().is_none_or(|b| {
            (elapsed + p.reference_msec).abs_diff(target)
                < (elapsed + b.reference_msec).abs_diff(target)
        }) {
            best = Some(p);
        }
    }
    if let Some(p) = best {
        pieces.push(p);
        true
    } else {
        false
    }
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
    for _ in 0..MAX_PIECES {
        cancellation::checkpoint()?;
        let elapsed: u32 = pieces.iter().map(|p| p.reference_msec).sum();
        if elapsed > target * 11 / 10 {
            pieces.pop();
            break;
        }
        if elapsed > target / 3 {
            let mut connected = pieces.clone();
            if closure(&mut connected, s) {
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
        if a.validate().is_ok() {
            if shortcut {
                return match authoring::seed_shortcut(&a) {
                    Ok(graph) => Ok(Some(graph)),
                    Err(e) if e.code == "E_CANCELLED" || e.code == "E_TRACK_SUPPORT" => Err(e),
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
    for attempt in 0..24 {
        cancellation::checkpoint()?;
        let candidate = match candidate(&s, attempt) {
            Err(e) if e.code == "E_TRACK_SUPPORT" => { support_failure = Some(e); continue; }
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
