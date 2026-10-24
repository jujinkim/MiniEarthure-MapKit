//! Obstacles, checkpoints, the single compile and validation of a grown layout.
use super::*;
use std::ops::Range;

/// An uncompiled base-route assembly over generated pieces.
pub(super) fn assembly(pieces: Vec<Piece>, s: &Settings) -> Assembly {
    let stats = statistics(&pieces);
    let route = authoring::Route { id: "base".into(), pieces: (0..pieces.len()).collect(), estimated_msec: stats.3 };
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
/// Generation constraint (not an authored-action rule): an acceleration piece
/// never leads straight into a jump, and no special family runs three times.
pub(super) fn valid_runs(pieces: &[Piece], circuit: bool) -> bool {
    for i in 0..pieces.len() {
        if !circuit && i + 1 == pieces.len() { break; }
        if ["acceleration_panel", "boost_chain"].contains(&pieces[i].id.as_str())
            && ["jump_panel", "jump", "offset_jump", "air_ring"].contains(&pieces[(i + 1) % pieces.len()].id.as_str()) {
            return false;
        }
    }
    let seq: Vec<_> = pieces.iter().filter(|p| !["tube_entry", "tube_exit", "approach", "finish_plaza"].contains(&p.id.as_str()))
        .map(|p| if category(&p.id) == "driving" { None } else { Some(family(&p.id)) }).collect();
    if seq.len() < 3 { return true; }
    (0..seq.len()).all(|i| {
        !circuit && i + 2 >= seq.len() || seq[i].is_none() || seq[i] != seq[(i + 1) % seq.len()] || seq[i] != seq[(i + 2) % seq.len()]
    })
}
/// Shared checkpoints from the common policy, thinned evenly to the 64 effective gates.
fn checkpoints(a: &Assembly) -> Vec<(usize, usize)> {
    let raw = authoring::checkpoints_between(a, (2, a.pieces[2].path.len() - 1), None);
    let gates: Vec<_> = raw.iter().map(|&(p, s)| crate::course::Checkpoint::from_track_sample(&a.pieces[p].path[s], String::new())).collect();
    let effective: Vec<_> = crate::course::effective_checkpoint_indices(&gates).into_iter().map(|i| raw[i]).collect();
    if effective.len() <= 64 { return raw; }
    let n = effective.len() - 1;
    let mut out: Vec<_> = (0..64).map(|i| effective[i * n / 63]).collect();
    out.dedup();
    out
}
/// Attachments, checkpoints and exactly one compile. Err(reason) rejects the attempt.
pub(super) fn finish(pieces: Vec<Piece>, reserved: &[Range<usize>], s: &Settings) -> Result<std::result::Result<Assembly, String>> {
    cancellation::progress("finishing", 0, Some(3), "steps");
    if !valid_runs(&pieces, s.circuit) || pieces.iter().any(|p| geometry::self_intersects(p) || !geometry::ordinary_grade_valid(p)) {
        return Ok(Err("generated piece sequence violates run or grade rules".into()));
    }
    let mut a = assembly(pieces, s);
    let (obstacles, eligible, target) = obstacles::place_reserved(&a, reserved)?;
    (a.obstacles, a.obstacle_eligible_length_cm, a.obstacle_target_count) = (obstacles, eligible, target);
    let mut source = authoring::from_assembly(&a);
    source.checkpoints = checkpoints(&a).into_iter().map(|(i, sample)| authoring::Checkpoint { piece: source.instances[i].id.clone(), sample }).collect();
    cancellation::progress("finishing", 1, Some(3), "steps");
    let mut out = match authoring::compile(&source) {
        Err(e) if e.code == "E_CANCELLED" => return Err(e),
        Err(e) => return Ok(Err(format!("{}: {}", e.code, e.message))),
        Ok(out) => out,
    };
    cancellation::progress("finishing", 2, Some(3), "steps");
    if let Err(e) = crate::profile::runtime(|| authoring::executable(&out)).and_then(|_| crate::profile::runtime(|| authoring::checkpoint_budget(&out))) {
        return Ok(Err(format!("{}: {}", e.code, e.message)));
    }
    out.authoring = None;
    out.seed_source = Some(source);
    cancellation::progress("validating", 0, None, "checks");
    out.validate()?;
    Ok(Ok(out))
}
/// The seeded generator: ≤4 deterministic attempts, each grown once and compiled once.
pub(super) fn assemble(settings: &Settings) -> Result<(Assembly, grow::Stats)> {
    let mut s = settings.normalized()?;
    s.difficulty = "normal".into();
    s.time_minutes = seed_streams::track_seed_defaults(s.seed)?["time_minutes"].as_u64().unwrap() as u16;
    let style = motifs::Style::new(s.seed);
    let mut reason = String::from("no closed layout within the growth budget");
    let mut stats = grow::Stats::default();
    for attempt in 0..grow::ATTEMPTS {
        cancellation::checkpoint()?;
        let grown = match grow::grow(&s, &style, attempt)? {
            Ok(grown) => grown,
            Err(effort) => { stats.add(effort); continue; }
        };
        stats.add(grown.stats);
        match finish(grown.pieces, &grown.reserved, &s)? {
            Ok(a) => return Ok((a, stats)),
            Err(why) => reason = why,
        }
    }
    Err(error("E_TRACK_LAYOUT", format!("Requested {} length; no feasible seed layout in {} attempts: {reason}", s.length.id(), grow::ATTEMPTS)))
}
