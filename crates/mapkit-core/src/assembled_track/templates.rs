//! Editable v1 graphs. Templates expand into ordinary roads/actions/branches;
//! no runtime template interpretation or second set of physics is required.
use super::*;
use authoring::{Action, Checkpoint, Connection, PanelAlignment, Path, Source};

pub fn catalogue() -> serde_json::Value {
    let mut rows = vec![
        serde_json::json!({"id":"double_jump","category":"action"}),
        serde_json::json!({"id":"air_ring_chain","category":"action"}),
        serde_json::json!({"id":"grind_shortcut","category":"gimmick"}),
    ];
    rows.extend(corner_shortcuts::IDS.map(|id|serde_json::json!({"id":id,"category":"gimmick"})));
    serde_json::json!(rows)
}
fn linear(ids: &[&str]) -> Result<Source> {
    let mut s = Source::empty();
    s.settings.circuit = false;
    let mut i = 0;
    while i < ids.len() {
        let id = ids[i];
        let mut count = 1;
        if id == "straight" {
            while i + count < ids.len() && ids[i + count] == "straight" {
                count += 1;
            }
        }
        let mut item = authoring::instance(
            &format!("t{i}"),
            if id == "straight" { "free_curve" } else { id },
            800,
        );
        if id == "straight" {
            let length = count as i64 * 800;
            item.control_points = vec![
                [0, 0, 0],
                [0, 0, length / 3],
                [0, 0, length * 2 / 3],
                [0, 0, length],
            ];
        }
        if let Some(previous) = s.instances.last() {
            item = authoring::snap(&item, previous)?;
        }
        s.instances.push(item);
        i += count;
    }
    s.paths.push(Path {
        id: "base".into(),
        pieces: s.instances.iter().map(|i| i.id.clone()).collect(),
    });
    s.connections = s.paths[0]
        .pieces
        .windows(2)
        .map(|w| Connection {
            from: w[0].clone(),
            to: w[1].clone(),
        })
        .collect();
    s.checkpoints = vec![
        Checkpoint {
            piece: s.instances[0].id.clone(),
            sample: 0,
        },
        Checkpoint {
            piece: s.instances.last().unwrap().id.clone(),
            sample: authoring::piece(s.instances.last().unwrap())?.path.len() - 1,
        },
    ];
    Ok(s)
}
pub fn source(id: &str) -> Result<Source> {
    if corner_shortcuts::IDS.contains(&id) { return corner_shortcuts::source(id,800,true); }
    match id {
        "double_jump" => linear(&[
            "straight", "straight", "jump", "straight", "straight", "straight", "straight",
            "straight", "jump", "straight", "straight", "straight", "straight", "straight",
        ]),
        "air_ring_chain" => {
            let mut s = linear(&[
                "straight", "straight", "jump", "straight", "straight", "straight", "straight",
                "straight", "straight", "straight", "straight",
            ])?;
            let p = authoring::piece(s.instances.iter().find(|i| i.id == "t2").unwrap())?;
            for (i, station) in [1600, 2500, 2900].iter().enumerate() {
                let (sample, point) = p
                    .path
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.position_cm[2].abs_diff(p.origin_cm[2] + station))
                    .unwrap();
                s.actions.push(Action {
                    id: format!("ring{i}"),
                    kind: "air_ring".into(),
                    piece: "t2".into(),
                    sample,
                    height_cm: ((if i == 0 { 300 } else { 360 }) - point.position_cm[1]) as u32,
                    panel_width_percent: 50,
                    panel_alignment: PanelAlignment::Center,
                    landing: None,
                });
            }
            Ok(s)
        }
        "grind_shortcut" => {
            let mut s = authoring::shortcut_source();
            s.instances.retain(|i| ["start-0", "start-1", "start-2", "lower-zigzag", "merge"].contains(&i.id.as_str()));
            // A broad bend on the ordinary route. The rail connects its shared
            // entry/rejoin, including supported approach and landing runways.
            s.instances[3].control_points = vec![
                [0,0,2400], [0,0,3200], [-4000,0,3200], [-4000,0,6400],
                [-4000,0,9600], [0,0,9600], [0,0,10400],
            ];
            for (id, preset, points) in [
                ("rail-entry", "free_curve", vec![[0,0,2400],[0,0,3000],[800,40,3800],[800,40,4400]]),
                ("rail-span", "flight_curve", vec![[800,40,4400],[800,40,5700],[800,40,7100],[800,40,8400]]),
                ("rail-exit", "free_curve", vec![[800,40,8400],[800,40,9000],[0,0,9800],[0,0,10400]]),
            ] {
                let mut item = authoring::instance(id, preset, 400);
                if id == "rail-entry" { item.entry_width_cm = 800; }
                if id == "rail-exit" { item.exit_width_cm = 800; }
                item.control_points = points;
                s.instances.push(item);
            }
            s.paths[0].pieces.retain(|id| id != "finish");
            s.paths[1].pieces = ["start-0", "start-1", "start-2", "rail-entry", "rail-span", "rail-exit", "merge"].map(String::from).to_vec();
            s.connections.clear();
            for path in &s.paths {
                for pair in path.pieces.windows(2) {
                    let c = Connection { from: pair[0].clone(), to: pair[1].clone() };
                    if !s.connections.contains(&c) { s.connections.push(c); }
                }
            }
            s.checkpoints.retain(|cp| cp.piece != "finish");
            s.actions.clear();
            s.actions.push(Action {
                id: "rail-entry-space".into(), kind: "manual_flight".into(), piece: "rail-entry".into(),
                sample: authoring::piece(&s.instances[5])?.path.len()-2,
                height_cm: 80, panel_width_percent: 50, panel_alignment: PanelAlignment::Center,
                landing: Some(Checkpoint { piece: "rail-exit".into(), sample: 0 }),
            });
            let position = [800, 0, 6400];
            s.structures.push(Gimmick {
                id: "authored-rail".into(),
                position,
                rotation_mdeg: [0; 3],
                scale_per_mille: [1000; 3],
                parts: vec![box_part([0, 20, 0], [40, 40, 4000])],
                track: None,
                curved_faces: vec![],
                effect: None,
                surface: Surface::Concrete,
                color: [255, 200, 40, 255],
                motion: Motion {
                    kind: MotionKind::Static,
                    delta_cm: [0; 3],
                    axis: 1,
                    period_ms: 4000,
                    phase_ms: 0,
                    impulse_cmps: [0; 3],
                    cooldown_ms: 1500,
                },
                safety_min_cm: position.map(|v| v - 4000),
                safety_max_cm: position.map(|v| v + 4000),
            });
            s.grind_lines.push(crate::grind::GrindLine {
                id: "rail".into(),
                control_points: vec![[800, 40, 4400], [800, 40, 8400]],
                up: [0, 1_000_000, 0],
                capture_width_cm: 40,
                start_connections: vec![],
                end_connections: vec![],
            });
            let length = |path: &Path| -> Result<u64> {
                path.pieces.iter().filter(|id| !id.starts_with("start-") && *id != "merge")
                    .map(|id| authoring::piece(s.instances.iter().find(|i| &i.id == id).unwrap())
                        .map(|p| p.path.windows(2).map(|w| distance(w[0].position_cm, w[1].position_cm)).sum::<u64>()))
                    .sum()
            };
            let main = length(&s.paths[0])?;
            let shortcut = length(&s.paths[1])?;
            if shortcut * 5 > main * 4 || main.saturating_sub(shortcut) < 500 {
                return Err(error("E_TRACK_CLEARANCE", "rail shortcut must save at least 20% and 5 m including entry/exit"));
            }
            Ok(s)
        }
        _ => Err(error("E_TRACK_SOURCE", "unknown track template")),
    }
}

/// Rigidly place a template at a supported port. References stay local until
/// merge; roads, solid rail and interaction line all receive the same transform.
pub(super) fn placed(id: &str, port: &Sample) -> Result<Source> {
    place(source(id)?, port)
}
pub(super) fn place(mut s: Source, port: &Sample) -> Result<Source> {
    let first = authoring::snap_surface(&s.instances[0], port)?;
    let rotation = first.rotation_mdeg;
    let origin = first.position_cm;
    let point = |v| add(origin, geometry::rotate3(v, rotation));
    let rotate = |old| {
        let axes = [[1_000_000, 0, 0], [0, 1_000_000, 0], [0, 0, 1_000_000]]
            .map(|v| geometry::rotate3(geometry::rotate3(v, old), rotation));
        geometry::euler(std::array::from_fn(|i| {
            std::array::from_fn(|j| axes[j][i] as f64 / 1e6)
        }))
    };
    for i in &mut s.instances {
        i.position_cm = point(i.position_cm);
        i.rotation_mdeg = rotate(i.rotation_mdeg);
    }
    s.instances[0].entry_width_cm = first.entry_width_cm;
    for g in &mut s.structures {
        g.position = point(g.position);
        g.rotation_mdeg = rotate(g.rotation_mdeg);
        let radius = g.parts.iter().flat_map(|p|&p.vertices).map(|v|v.iter().map(|n|n.abs()).sum::<i64>()).max().unwrap_or(5000);
        g.safety_min_cm = g.position.map(|v| v - radius);
        g.safety_max_cm = g.position.map(|v| v + radius);
    }
    for l in &mut s.grind_lines {
        for p in &mut l.control_points {
            *p = point(*p);
        }
        l.up = geometry::rotate3(l.up, rotation);
    }
    if s.instances.iter().any(|i|i.id=="corner") && !s.grind_lines.is_empty() {
        // Placement rounds control points to cm. Resample the final curve once
        // so its collision and interaction frames remain identical after yaw.
        s.structures = s.grind_lines.iter().flat_map(corner_shortcuts::rail_solids).collect();
    }
    Ok(s)
}

/// Reserve the entire ring aperture/rim, including clearance above the road's
/// ordinary vehicle envelope. Module roads already passed their own geometry check.
pub(super) fn clear_of_roads(fragment: &Source, others: &[Piece]) -> Result<bool> {
    for line in &fragment.grind_lines {
        for pair in line.samples().windows(2) {
            let pair = [pair[0].position_cm, pair[1].position_cm];
            let steps = distance(pair[0], pair[1]).div_ceil(100).max(1);
            for step in 0..=steps {
                let center: Vertex = std::array::from_fn(|j| pair[0][j] + (pair[1][j]-pair[0][j])*step as i64/steps as i64);
                if others.iter().flat_map(|p| p.path.iter().chain(&p.alternate_path)).any(|s| {
                    let (point, radius, low, high) = geometry::volume(s);
                    low < center[1]+235 && center[1]-40 < high
                        && (center[0]-point[0]).pow(2)+(center[2]-point[2]).pow(2) < (radius+220).pow(2)
                }) { return Ok(false); }
            }
        }
    }
    for action in fragment.actions.iter().filter(|a| a.kind == "air_ring") {
        let piece = authoring::piece(
            fragment
                .instances
                .iter()
                .find(|i| i.id == action.piece)
                .unwrap(),
        )?;
        let mut center = piece.path[action.sample].position_cm;
        center[1] += i64::from(action.height_cm);
        if others
            .iter()
            .flat_map(|p| p.path.iter().chain(&p.alternate_path))
            .any(|sample| {
                let (point, radius, low, high) = geometry::volume(sample);
                low < center[1] + 300
                    && center[1] - 300 < high
                    && (center[0] - point[0]).pow(2) + (center[2] - point[2]).pow(2)
                        < (radius + 300).pow(2)
            })
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Merge a placed graph, mapping its base instances onto an existing sequence.
/// This also creates whole-course alternatives with shared branch checkpoints.
pub(super) fn merge(s: &mut Source, fragment: &Source, base_ids: &[String], prefix: &str) {
    let base = &fragment.paths[0].pieces;
    let map: std::collections::BTreeMap<_, _> = fragment
        .instances
        .iter()
        .map(|i| {
            let name = base
                .iter()
                .position(|id| id == &i.id)
                .map_or_else(|| format!("{prefix}-{}", i.id), |at| base_ids[at].clone());
            (i.id.clone(), name)
        })
        .collect();
    for item in &fragment.instances {
        if !base.contains(&item.id) {
            let mut item = item.clone();
            item.id = map[&item.id].clone();
            s.instances.push(item);
        }
    }
    for c in &fragment.connections {
        let edge = Connection {
            from: map[&c.from].clone(),
            to: map[&c.to].clone(),
        };
        if !s.connections.contains(&edge) {
            s.connections.push(edge);
        }
    }
    let existing = s.paths.clone();
    for path in fragment.paths.iter().skip(1) {
        for prior in &existing {
            if let Some(first) = prior.pieces.windows(base_ids.len()).position(|ids|ids==base_ids) {
                let mut route = prior.clone();
                route.id = format!("{}-{prefix}-{}", prior.id, path.id);
                route.pieces.splice(first..first + base_ids.len(), path.pieces.iter().map(|id| map[id].clone()));
                s.paths.push(route);
            }
        }
    }
    for a in &fragment.actions {
        let mut a = a.clone();
        a.id = format!("{prefix}-{}", a.id);
        a.piece = map[&a.piece].clone();
        if let Some(cp) = &mut a.landing {
            cp.piece = map[&cp.piece].clone();
        }
        s.actions.push(a);
    }
    for g in &fragment.structures {
        let mut g = g.clone();
        g.id = format!("authored-{prefix}-{}", g.id.trim_start_matches("authored-"));
        s.structures.push(g);
    }
    for l in &fragment.grind_lines {
        let mut l = l.clone();
        l.id = format!("{prefix}-{}", l.id);
        for link in l.start_connections.iter_mut().chain(&mut l.end_connections) {
            link.line = format!("{prefix}-{}", link.line);
        }
        s.grind_lines.push(l);
    }
}

pub fn append(source: &Source, id: &str, anchor: &str, prefix: &str) -> Result<Source> {
    if prefix.is_empty()
        || prefix.len() > 12
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(error(
            "E_TRACK_SOURCE",
            "template prefix must be 1..12 ASCII letters/digits/hyphens",
        ));
    }
    let port = if source.instances.is_empty() {
        authoring::piece(&authoring::instance("origin", "straight", 800))?.path[0].clone()
    } else {
        authoring::piece(
            source
                .instances
                .iter()
                .find(|i| i.id == anchor)
                .ok_or_else(|| error("E_TRACK_SOURCE", "select a route endpoint"))?,
        )?
        .path
        .last()
        .unwrap()
        .clone()
    };
    let fragment = placed(id, &port)?;
    let existing = source
        .instances
        .iter()
        .map(authoring::piece)
        .collect::<Result<Vec<_>>>()?;
    if !clear_of_roads(&fragment, &existing)? {
        return Err(error(
            "E_TRACK_CLEARANCE",
            "template rail corridor or ring aperture needs clear space",
        ));
    }
    let mut out = source.clone();
    out.original_seed = None;
    if out.paths.is_empty() {
        out.paths.push(Path {
            id: "base".into(),
            pieces: vec![],
        });
    }
    if !out.paths[0].pieces.is_empty() && out.paths[0].pieces.last().unwrap() != anchor {
        return Err(error(
            "E_TRACK_SOURCE",
            "select the end of the base route to append a template",
        ));
    }
    let ids: Vec<_> = fragment.paths[0]
        .pieces
        .iter()
        .map(|id| format!("{prefix}-{id}"))
        .collect();
    for (old, new) in fragment.paths[0].pieces.iter().zip(&ids) {
        let mut item = fragment
            .instances
            .iter()
            .find(|i| &i.id == old)
            .unwrap()
            .clone();
        item.id = new.clone();
        out.instances.push(item);
    }
    if !anchor.is_empty() {
        out.connections.push(Connection {
            from: anchor.into(),
            to: ids[0].clone(),
        });
    }
    for route in &mut out.paths {
        if route.pieces.is_empty() || route.pieces.last().is_some_and(|last| last == anchor) {
            route.pieces.extend(ids.iter().cloned());
        }
    }
    merge(&mut out, &fragment, &ids, prefix);
    if corner_shortcuts::IDS.contains(&id) && !corner_shortcuts::clear(&authoring::compile(&out)?,prefix,&ids)? {
        return Err(error("E_TRACK_CLEARANCE","template rail corridor is occupied"));
    }
    Ok(out)
}
