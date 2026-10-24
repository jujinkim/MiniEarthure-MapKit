//! Obstacles, checkpoints, the single compile and validation of a grown layout.
use super::*;

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
/// never leads straight into a jump or a drop takeoff, and no special family runs three times.
pub(super) fn valid_runs(pieces: &[Piece], circuit: bool) -> bool {
    let n = pieces.len();
    for i in 0..n {
        if !circuit && i + 1 == n { break; }
        if ["acceleration_panel", "boost_chain"].contains(&pieces[i].id.as_str())
            && (["jump_panel", "jump", "offset_jump", "air_ring"].contains(&pieces[(i + 1) % n].id.as_str())
                || (circuit || i + 2 < n) && pieces[(i + 2) % n].id == "flight_curve") {
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
/// Shared checkpoints from the common policy over every route of `source`,
/// thinned evenly to the 64 effective gates. No compile is needed: only shared
/// base pieces, which `a` holds, carry checkpoints.
fn checkpoints(a: &Assembly, source: &authoring::Source) -> Vec<authoring::Checkpoint> {
    let names: std::collections::BTreeMap<_, _> = source.instances.iter().enumerate().map(|(i, v)| (v.id.as_str(), i)).collect();
    let mut probe = a.clone();
    probe.routes = source.paths.iter().map(|p| authoring::Route { id: p.id.clone(), pieces: p.pieces.iter().map(|id| names[id.as_str()]).collect(), estimated_msec: 0 }).collect();
    let raw = authoring::checkpoints_between(&probe, (2, a.pieces[2].path.len() - 1), None);
    let gates: Vec<_> = raw.iter().map(|&(p, s)| crate::course::Checkpoint::from_track_sample(&a.pieces[p].path[s], String::new())).collect();
    let effective: Vec<_> = crate::course::effective_checkpoint_indices(&gates).into_iter().map(|i| raw[i]).collect();
    let mut out = if effective.len() <= 64 { raw } else { let n = effective.len() - 1; (0..64).map(|i| effective[i * n / 63]).collect() };
    out.dedup();
    out.into_iter().map(|(i, sample)| authoring::Checkpoint { piece: source.instances[i].id.clone(), sample }).collect()
}
/// The jump panel near the end of a drop's level takeoff, landing at the start of its landing road.
fn drop_action(a: &Assembly, source: &authoring::Source, takeoff: usize) -> authoring::Action {
    let path = &a.pieces[takeoff].path;
    let mut station = 0;
    let stations: Vec<_> = std::iter::once(0).chain(path.windows(2).map(|w| { station += distance(w[0].position_cm, w[1].position_cm); station })).collect();
    let sample = stations.iter().rposition(|s| *s + 400 <= station).unwrap_or(0);
    authoring::Action { id: format!("drop-{takeoff}"), kind: "jump_panel".into(), piece: source.instances[takeoff].id.clone(), sample, height_cm: 125,
        panel_width_percent: 100, panel_alignment: authoring::PanelAlignment::Center,
        landing: Some(authoring::Checkpoint { piece: source.instances[takeoff + 2].id.clone(), sample: 0 }) }
}
/// Attachments, drop actions, shortcut branches, the narrow section and
/// checkpoints, then one compile. Shortcuts, then the narrow section, are
/// dropped only when that compile rejects them (≤2 recompiles). Err(reason) rejects the attempt.
pub(super) fn finish(grown: grow::Grown, s: &Settings) -> Result<std::result::Result<Assembly, String>> {
    cancellation::progress("finishing", 0, Some(3), "steps");
    let grow::Grown { pieces, reserved, drops, shortcuts, .. } = grown;
    if !valid_runs(&pieces, s.circuit) || pieces.iter().any(|p| geometry::self_intersects(p) || !geometry::ordinary_grade_valid(p)) {
        return Ok(Err("generated piece sequence violates run or grade rules".into()));
    }
    let mut a = assembly(pieces, s);
    let (obstacles, eligible, target) = obstacles::place_reserved(&a, &reserved)?;
    (a.obstacles, a.obstacle_eligible_length_cm, a.obstacle_target_count) = (obstacles, eligible, target);
    let mut plain = authoring::from_assembly(&a);
    for &t in &drops { plain.actions.push(drop_action(&a, &plain, t)); }
    let mut full = plain.clone();
    let mut rails = vec![];
    for (range, shortcut) in &shortcuts {
        let port = a.pieces[range.start - 1].path.last().unwrap();
        let Ok(fragment) = shortcut.source(true).and_then(|f| templates::place(f, port)) else { continue };
        let merge = |j: &jump_shortcuts::Shortcut| a.pieces[range.start + fragment.paths[0].pieces.iter().position(|id| *id == j.merge.piece).unwrap()].path[j.merge.sample].position_cm;
        if !fragment.jump_shortcuts.iter().all(|j| jump_shortcuts::corridor_clear(j, merge(j), &a.pieces, range)) { continue; }
        let ids: Vec<_> = range.clone().map(|i| full.instances[i].id.clone()).collect();
        let prefix = format!("{}{}", shortcut.prefix(), range.start);
        templates::merge(&mut full, &fragment, &ids, &prefix);
        if matches!(shortcut, motifs::Shortcut::Grind(..)) { rails.push((prefix, ids)); }
    }
    let narrow = narrow::select(&a, &plain, &reserved);
    let with_narrow = |mut source: authoring::Source| { narrow::apply(&mut source, &narrow); source };
    let mut candidates = vec![(with_narrow(full.clone()), true)];
    if full != plain { candidates.push((with_narrow(plain.clone()), false)); }
    if !narrow.is_empty() { candidates.push((plain, false)); }
    let mut reason = String::new();
    for (mut source, shortcut) in candidates {
        source.checkpoints = checkpoints(&a, &source);
        cancellation::progress("finishing", 1, Some(3), "steps");
        let out = match authoring::compile(&source) {
            Err(e) if e.code == "E_CANCELLED" => return Err(e),
            Err(e) => { reason = format!("{}: {}", e.code, e.message); continue; }
            Ok(out) => out,
        };
        cancellation::progress("finishing", 2, Some(3), "steps");
        if let Err(e) = crate::profile::runtime(|| authoring::executable(&out)).and_then(|_| crate::profile::runtime(|| authoring::checkpoint_budget(&out))) {
            reason = format!("{}: {}", e.code, e.message);
            continue;
        }
        let mut clear = true;
        for (prefix, ids) in rails.iter().filter(|_| shortcut) {
            match corner_shortcuts::clear(&out, prefix, ids) {
                Err(e) if e.code == "E_CANCELLED" => return Err(e),
                Ok(true) => {}
                _ => clear = false,
            }
        }
        if !clear { reason = "grind corridor occupied".into(); continue; }
        if !narrow::valid(&out) { reason = "narrow road budget".into(); continue; }
        let mut out = out;
        out.authoring = None;
        out.seed_source = Some(source);
        cancellation::progress("validating", 0, None, "checks");
        out.validate()?;
        return Ok(Ok(out));
    }
    Ok(Err(reason))
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
        match finish(grown, &s)? {
            Ok(a) => return Ok((a, stats)),
            Err(why) => reason = why,
        }
    }
    Err(error("E_TRACK_LAYOUT", format!("Requested {} length; no feasible seed layout in {} attempts: {reason}", s.length.id(), grow::ATTEMPTS)))
}
