//! Current-v1 editable source. A disconnected draft is valid source, never an executable course.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    pub preset: String,
    pub position_cm: Vertex,
    pub rotation_mdeg: [i32; 3],
    pub width_cm: u32,
    pub entry_width_cm: u32,
    pub exit_width_cm: u32,
    pub control_points: Vec<Vertex>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub from: String,
    pub to: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: String,
    pub pieces: Vec<usize>,
    pub estimated_msec: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Path {
    pub id: String,
    pub pieces: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub piece: String,
    pub sample: usize,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PanelAlignment { Left, Center, Right }
fn panel_width_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    let mut schema=schemars::schema_for!(u8).schema;
    schema.enum_values=Some([25,50,75,100].map(|v|serde_json::json!(v)).to_vec());
    schema.into()
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub kind: String,
    pub piece: String,
    pub sample: usize,
    pub height_cm: u32,
    #[schemars(schema_with = "panel_width_schema")]
    pub panel_width_percent: u8,
    pub panel_alignment: PanelAlignment,
    pub landing: Option<Checkpoint>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub kind: String,
    pub piece: String,
    pub path: String,
    pub station_cm: u64,
    pub side: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub original_seed: Option<Settings>,
    pub grounded_supports: bool,
    pub settings: Settings,
    pub instances: Vec<Instance>,
    pub connections: Vec<Connection>,
    /// First route is the base route. Other routes include the shared start/finish.
    pub paths: Vec<Path>,
    pub checkpoints: Vec<Checkpoint>,
    pub actions: Vec<Action>,
    pub attachments: Vec<Attachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grind_lines: Vec<crate::grind::GrindLine>,
    /// Explicit static collision, independent of roads and interaction lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structures: Vec<Gimmick>,
}
impl Source {
    pub fn empty() -> Self {
        Self {
            original_seed: None,
            grounded_supports: false,
            settings: Settings::default(),
            instances: vec![],
            connections: vec![],
            paths: vec![],
            checkpoints: vec![],
            actions: vec![],
            attachments: vec![],
            grind_lines: vec![],
            structures: vec![],
        }
    }
}
/// One bounded preset placement returns explicit, independently editable interactions.
pub fn attachment_lines(instance: &Instance, attachment: &Attachment) -> Result<Vec<crate::grind::GrindLine>> {
    let mut source=Source::empty();
    source.instances.push(instance.clone());
    source.attachments.push(attachment.clone());
    let assembly=compile(&source)?;
    Ok(obstacles::rail_lines(&assembly))
}

pub fn from_assembly(a: &Assembly) -> Source {
    if let Some(source) = a.authoring.as_ref().or(a.seed_source.as_ref()) {
        let mut source=source.clone();
        source.grind_lines=obstacles::grind_lines(a);
        return source;
    }
    let ids: Vec<_> = (0..a.pieces.len()).map(|i| format!("piece-{i}")).collect();
    let mut connections = vec![];
    for route in &a.routes {
        for pair in route.pieces.windows(2) {
            let edge = Connection {
                from: ids[pair[0]].clone(),
                to: ids[pair[1]].clone(),
            };
            if !connections.contains(&edge) {
                connections.push(edge);
            }
        }
        if a.settings.circuit {
            connections.push(Connection {
                from: ids[*route.pieces.last().unwrap()].clone(),
                to: ids[route.pieces[0]].clone(),
            });
        }
    }
    let paths = a
        .routes
        .iter()
        .map(|r| Path {
            id: r.id.clone(),
            pieces: r.pieces.iter().map(|i| ids[*i].clone()).collect(),
        })
        .collect();
    let checkpoints = common_checkpoints(a)
        .into_iter()
        .map(|(i, s)| Checkpoint {
            piece: ids[i].clone(),
            sample: s,
        })
        .collect();
    Source {
        original_seed: Some(a.settings.clone()),
        grounded_supports: true,
        settings: a.settings.clone(),
        instances: a
            .pieces
            .iter()
            .enumerate()
            .map(|(i, p)| Instance {
                id: ids[i].clone(),
                preset: p.id.clone(),
                position_cm: p.origin_cm,
                rotation_mdeg: p.rotation_mdeg,
                width_cm: p.width_cm,
                entry_width_cm: p.entry_width_cm,
                exit_width_cm: p.exit_width_cm,
                control_points: p.control_points.clone(),
            })
            .collect(),
        connections,
        paths,
        checkpoints,
        actions: vec![],
        grind_lines: obstacles::grind_lines(a),
        structures: vec![],
        attachments: a
            .obstacles
            .iter()
            .map(|o| Attachment {
                kind: o.kind.clone(),
                piece: ids[o.piece_index].clone(),
                path: o.path.clone(),
                station_cm: o.station_cm,
                side: if o.lateral_cm < 0 { -1 } else { 1 },
            })
            .collect(),
    }
}
pub fn instance(id: &str, preset: &str, width: u32) -> Instance {
    Instance {
        id: id.into(),
        preset: preset.into(),
        position_cm: [0; 3],
        rotation_mdeg: [0; 3],
        width_cm: width,
        entry_width_cm: width,
        exit_width_cm: width,
        control_points: vec![],
    }
}
pub fn piece(instance: &Instance) -> Result<Piece> {
    if (instance.preset.starts_with("cylinder") || ["tube_entry", "tube_exit"].contains(&instance.preset.as_str()))
        && [instance.width_cm, instance.entry_width_cm, instance.exit_width_cm].iter().any(|v| *v < 200) {
        return Err(error("E_PIPE_DIMENSIONS", "Pipe bore and port width must be at least 200 cm (radius 100 cm); source was not modified"));
    }
    if !catalogue_ids().contains(&instance.preset.as_str())
        || !supported_widths(&instance.preset).contains(&instance.width_cm)
        || !(minimum_port_width(&instance.preset)..=1200).contains(&instance.entry_width_cm)
        || !(minimum_port_width(&instance.preset)..=1200).contains(&instance.exit_width_cm)
        || instance
            .position_cm
            .iter()
            .any(|v| v.unsigned_abs() > 10_000_000)
        || instance
            .rotation_mdeg
            .iter()
            .any(|v| v.unsigned_abs() > 360000)
        || instance.control_points.len() > 193
        || instance
            .control_points
            .iter()
            .flatten()
            .any(|v| v.unsigned_abs() > 1_000_000)
        || (!instance.control_points.is_empty()
            && (instance.control_points.len() < 4
                || (instance.control_points.len() - 1) % 3 != 0
                || !["free_curve", "flight_curve"].contains(&instance.preset.as_str())))
    {
        return Err(error(
            "E_TRACK_SOURCE",
            "invalid instance dimensions, preset or cubic control points",
        ));
    }
    let estimated_samples: usize = instance
        .control_points
        .windows(4)
        .step_by(3)
        .map(|cp| {
            ((cp.windows(2).map(|w| distance(w[0], w[1])).sum::<u64>() / 40).clamp(8, 1024) + 1)
                as usize
        })
        .sum();
    if estimated_samples > MAX_SAMPLES {
        return Err(error("E_TRACK_BUDGET", "curve exceeds sample budget"));
    }
    let mut p = variant(
        &instance.preset,
        instance.width_cm,
        instance.entry_width_cm,
        instance.exit_width_cm,
    );
    p.origin_cm = instance.position_cm;
    p.rotation_mdeg = instance.rotation_mdeg;
    p.quarter_turns = (instance.rotation_mdeg[1].rem_euclid(360000) / 90000) as u8;
    p.control_points = instance.control_points.clone();
    let p = materialize(&p);
    if p.path.len()+p.alternate_path.len()>MAX_SAMPLES {
        return Err(error("E_TRACK_BUDGET", "curve exceeds final sample budget"));
    }
    if p.path
        .iter()
        .any(|s| distance(s.forward, [0; 3]) < 999990 || distance(s.normal, [0; 3]) < 999990)
    {
        return Err(error("E_TRACK_SOURCE", "curve has a stationary tangent"));
    }
    Ok(p)
}
pub fn ports(instance: &Instance) -> Result<serde_json::Value> {
    let p = piece(instance)?;
    Ok(serde_json::json!({"entry":p.path.first(),"exit":p.path.last()}))
}
pub fn snap(instance: &Instance, target: &Instance) -> Result<Instance> {
    let t = piece(target)?;
    let end = t.path.last().unwrap();
    let mut out = instance.clone();
    out.rotation_mdeg = [0; 3];
    let local = piece(&out)?;
    let target_basis = geometry::basis(end);
    let local_basis = geometry::basis(&local.path[0]);
    let rotation = std::array::from_fn(|i| {
        std::array::from_fn(|j| (0..3).map(|k| target_basis[i][k] * local_basis[j][k]).sum())
    });
    out.rotation_mdeg = geometry::euler(rotation);
    let local = piece(&out)?;
    out.position_cm = std::array::from_fn(|j| {
        out.position_cm[j] + end.position_cm[j] - local.path[0].position_cm[j]
    });
    let drop = portal_drop(&target.preset, &instance.preset, instance.width_cm);
    for j in 0..3 {
        out.position_cm[j] -= end.normal[j] * drop / 1_000_000;
    }
    out.entry_width_cm = end.lateral_cm * 2;
    Ok(out)
}
// One bounded compiler result per worker thread. Validation still compares every
// derived field against this compiler-owned value; callers cannot supply a
// certificate or disable validation. Never cache errors or cancelled work.
thread_local! {
    static LAST_COMPILED: std::cell::RefCell<Option<(Source, Assembly)>> = const { std::cell::RefCell::new(None) };
    #[cfg(test)] static COMPILE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub fn compile(source: &Source) -> Result<Assembly> {
    cancellation::checkpoint()?;
    if let Some(result) = LAST_COMPILED.with(|cache| cache.borrow().as_ref()
        .filter(|(input, _)| input == source).map(|(_, result)| result.clone())) {
        return Ok(result);
    }
    #[cfg(test)] COMPILE_COUNT.with(|count| count.set(count.get() + 1));
    let result = compile_uncached(source)?;
    cancellation::checkpoint()?;
    LAST_COMPILED.with(|cache| *cache.borrow_mut() = Some((source.clone(), result.clone())));
    Ok(result)
}

/// The stored authoring snapshot and the compiler share the same bounded input.
/// This is independent of the smaller effective driving-course checkpoint limit.
pub(super) fn within_source_limits(source: &Source) -> bool {
    source.instances.len() <= MAX_PIECES
        && source.connections.len() <= 1024
        && source.paths.len() <= 32
        && source.checkpoints.len() <= 128
        && source.actions.len() <= 64
        && source.attachments.len() <= 128
        && source.structures.len() <= 32
}

fn compile_uncached(source: &Source) -> Result<Assembly> {
    crate::grind::validate(&source.grind_lines)?;
    if source.structures.len() > 32 || source.structures.iter().any(|g| !g.valid() || g.motion.kind != MotionKind::Static || g.effect.is_some() || !g.id.starts_with("authored-")) {
        return Err(error("E_TRACK_SOURCE", "at most 32 valid authored- static structures required"));
    }
    if !within_source_limits(source) {
        return Err(error(
            "E_TRACK_BUDGET",
            "authoring source exceeds bounded graph budget",
        ));
    }
    let mut names = BTreeMap::new();
    let mut pieces = vec![];
    let mut samples = 0usize;
    for i in &source.instances {
        cancellation::checkpoint()?;
        if i.id.is_empty() || i.id.len() > 128 || names.insert(i.id.clone(), pieces.len()).is_some()
        {
            return Err(error("E_TRACK_SOURCE", "instance IDs must be unique"));
        }
        let p = piece(i)?;
        samples += p.path.len() + p.alternate_path.len();
        if samples > MAX_SAMPLES {
            return Err(error("E_TRACK_BUDGET", "sample budget exceeded"));
        }
        pieces.push(p);
    }
    if pieces.iter().map(|p| p.path.len()).sum::<usize>() > MAX_SAMPLES {
        return Err(error("E_TRACK_BUDGET", "sample budget exceeded"));
    }
    let mut issues = vec![];
    let mut routes = vec![];
    for path in &source.paths {
        if path.pieces.len() > MAX_PIECES || path.id.len() > 128 {
            return Err(error("E_TRACK_BUDGET", "route budget exceeded"));
        }
        let indices = path
            .pieces
            .iter()
            .map(|id| {
                names
                    .get(id)
                    .copied()
                    .ok_or_else(|| error("E_TRACK_SOURCE", "route references missing instance"))
            })
            .collect::<Result<Vec<_>>>()?;
        if indices.len() < 2 {
            issues.push(format!("{}: start and finish route required", path.id));
        }
        if indices.iter().collect::<BTreeSet<_>>().len() != indices.len() {
            issues.push(format!("{}: repeated route instance", path.id));
        }
        let time = indices.iter().map(|i| pieces[*i].reference_msec).sum();
        routes.push(Route {
            id: path.id.clone(),
            pieces: indices,
            estimated_msec: time,
        });
    }
    let connected: BTreeSet<_> = source
        .connections
        .iter()
        .map(|c| (c.from.as_str(), c.to.as_str()))
        .collect();
    for c in &source.connections {
        let (Some(a), Some(b)) = (names.get(&c.from), names.get(&c.to)) else {
            return Err(error(
                "E_TRACK_SOURCE",
                "connection references missing instance",
            ));
        };
        let mut end = pieces[*a].path.last().unwrap().clone();
        let drop = portal_drop(&pieces[*a].id, &pieces[*b].id, pieces[*b].width_cm);
        for j in 0..3 {
            end.position_cm[j] -= end.normal[j] * drop / 1_000_000;
        }
        let b = &pieces[*b].path[0];
        if !joined(&end, b) {
            issues.push(format!(
                "{} → {}: ports do not meet in position, direction or width",
                c.from, c.to
            ));
        }
    }
    for path in &source.paths {
        for pair in path.pieces.windows(2) {
            if !connected.contains(&(pair[0].as_str(), pair[1].as_str())) {
                issues.push(format!("{}: missing connection", path.id));
            }
        }
        if source.settings.circuit
            && !path.pieces.is_empty()
            && !connected.contains(&(
                path.pieces.last().unwrap().as_str(),
                path.pieces[0].as_str(),
            ))
        {
            issues.push(format!("{}: circuit is open", path.id));
        }
    }
    if routes.is_empty() {
        issues.push("Set a base route, start and finish".into());
    }
    for r in routes.iter().skip(1) {
        if r.pieces.first() != routes[0].pieces.first()
            || r.pieces.last() != routes[0].pieces.last()
        {
            issues.push(format!("{}: alternative must share start and finish", r.id));
        }
    }
    let used: BTreeSet<_> = routes.iter().flat_map(|r| r.pieces.iter()).collect();
    if used.len() != pieces.len() {
        issues.push("Every road must belong to a base or alternative route".into());
    }
    for cp in &source.checkpoints {
        let Some(&i) = names.get(&cp.piece) else {
            return Err(error(
                "E_TRACK_SOURCE",
                "checkpoint references missing instance",
            ));
        };
        if cp.sample >= pieces[i].path.len() {
            return Err(error(
                "E_TRACK_SOURCE",
                "checkpoint sample is outside its road",
            ));
        }
        if source.checkpoints.first() == Some(cp) && !pieces[i].path[cp.sample].safe {
            issues.push("Ground start checkpoint needs a supported driving surface".into());
        }
        if routes.iter().any(|r| !r.pieces.contains(&i)) {
            issues.push("Checkpoints must be in shared progress sections".into());
        }
    }
    for route in &routes {
        let mut previous = None;
        for cp in &source.checkpoints {
            let i = names[&cp.piece];
            if let Some(at) = route.pieces.iter().position(|p| *p == i) {
                let current = (at, cp.sample);
                if previous.is_some_and(|p| p >= current) {
                    issues.push(
                        "Shared checkpoints must follow progress order on every route".into(),
                    );
                }
                previous = Some(current);
            }
        }
    }
    for action in &source.actions {
        let Some(&i) = names.get(&action.piece) else {
            return Err(error("E_TRACK_SOURCE", "action references missing road"));
        };
        if action.sample >= pieces[i].path.len()
            || ![
                "jump_panel",
                "manual_flight",
                "acceleration_panel",
                "boost_chain",
                "air_ring",
            ]
            .contains(&action.kind.as_str())
            || !(50..=1000).contains(&action.height_cm)
            || ![25,50,75,100].contains(&action.panel_width_percent)
        {
            return Err(error("E_TRACK_SOURCE", "invalid action"));
        }
        if ["jump_panel","acceleration_panel","boost_chain"].contains(&action.kind.as_str())
            && ["flight","loop","cylinder","halfpipe"].contains(&pieces[i].path[action.sample].mode.as_str()) {
            return Err(error("E_TRACK_PANEL_SUPPORT","panel anchor needs an ordinary supporting road surface"));
        }
        if action.kind == "manual_flight" && (action.landing.is_none() || !pieces[i].path[action.sample].safe) {
            return Err(error("E_TRACK_SOURCE", "manual flight requires supported takeoff and landing references"));
        }
        if action.kind == "boost_chain"
            && pieces[i].path[action.sample..]
                .windows(2)
                .map(|w| distance(w[0].position_cm, w[1].position_cm))
                .sum::<u64>()
                < 1000
        {
            issues.push(format!(
                "{}: booster chain needs ten metres of road",
                action.id
            ));
        }
        if let Some(landing) = &action.landing {
            let Some(&j) = names.get(&landing.piece) else {
                return Err(error("E_TRACK_SOURCE", "landing road missing"));
            };
            let Some(t) = pieces[j].path.get(landing.sample) else {
                return Err(error("E_TRACK_SOURCE", "landing sample missing"));
            };
            let runway: u64 = pieces[j].path[landing.sample..]
                .windows(2)
                .take_while(|w| {
                    w.iter()
                        .all(|s| s.safe && s.lateral_cm >= 100 && s.normal[1] >= 900000)
                })
                .map(|w| distance(w[0].position_cm, w[1].position_cm))
                .sum();
            if runway < 600 {
                issues.push(format!(
                    "{}: landing needs six metres of supported runway",
                    action.id
                ));
            }
            let s = &pieces[i].path[action.sample];
            if t.mode == "flight"
                || t.lateral_cm < 100
                || t.position_cm[1] - s.position_cm[1] >= i64::from(action.height_cm)
                || distance(s.position_cm, t.position_cm) > 5000
            {
                issues.push(format!(
                    "{}: landing height, space or reach invalid",
                    action.id
                ));
            }
        }
    }
    for path in &source.paths {
        for (at, id) in path.pieces.iter().enumerate() {
            let i = names[id];
            if pieces[i].id != "flight_curve"
                || at > 0 && pieces[names[&path.pieces[at - 1]]].id == "flight_curve"
            {
                continue;
            }
            let landing = path.pieces[at..]
                .iter()
                .find(|id| pieces[names[*id]].id != "flight_curve");
            let declared = at > 0
                && source.actions.iter().any(|a| {
                    (a.kind == "jump_panel" || a.kind == "manual_flight")
                        && a.piece == path.pieces[at - 1]
                        && a.landing
                            .as_ref()
                            .is_some_and(|cp| Some(&cp.piece) == landing)
                });
            if !declared {
                issues.push(format!(
                    "{}: flight requires an automatic or manual approach and declared supported landing",
                    id
                ));
            }
        }
    }
    // Full road volume clearance, excluding only a small shared port neighbourhood.
    for i in 0..pieces.len() {
        for j in 0..i {
            cancellation::checkpoint()?;
            if conflict(&pieces[i], &pieces[j]) {
                issues.push(format!(
                    "{} / {}: road clearance collision",
                    source.instances[i].id, source.instances[j].id
                ));
            }
        }
    }
    for (i, p) in pieces.iter().enumerate() {
        if geometry::self_intersects(p) {
            issues.push(format!(
                "{}: ribbon self intersection or insufficient clearance",
                source.instances[i].id
            ));
        }
    }
    issues.sort();
    issues.dedup();
    let base: Vec<_> = routes.first().map_or(vec![], |r| {
        r.pieces.iter().map(|i| pieces[*i].clone()).collect()
    });
    let stats = if base.is_empty() {
        (
            0,
            0,
            0,
            0,
            VenueFloor {
                min_cm: [-1600, -235, -1600],
                max_cm: [1600, -235, 1600],
            },
        )
    } else {
        statistics(&base)
    };
    let floor = if pieces.is_empty() {
        stats.4
    } else {
        statistics(&pieces).4
    };
    let finish = routes.first().and_then(|r| r.pieces.last()).and_then(|i| {
        let mut f = finish_plaza(std::slice::from_ref(&pieces[*i]))?;
        f.piece_index = *i;
        Some(f)
    });
    let mut assembly = Assembly {
        authoring: Some(source.clone()),
        seed_source: None,
        routes,
        issues,
        settings: source.settings.clone(),
        generator_fingerprint: fingerprint(),
        catalogue_fingerprint: catalogue_fingerprint(),
        pieces,
        supports: vec![],
        obstacles: vec![],
        obstacle_eligible_length_cm: 0,
        obstacle_target_count: 0,
        length_cm: stats.0,
        ordinary_length_cm: stats.1,
        ordinary_straight_cm: stats.2,
        estimated_msec: stats.3,
        floor,
        finish_plaza: finish,
    };
    for attachment in &source.attachments {
        let Some(&index) = names.get(&attachment.piece) else {
            return Err(error("E_TRACK_SOURCE", "obstacle road missing"));
        };
        if !obstacles::KINDS.contains(&attachment.kind.as_str())
            || !["main", "alternate"].contains(&attachment.path.as_str())
            || ![-1, 1].contains(&attachment.side)
        {
            return Err(error("E_TRACK_SOURCE", "invalid obstacle source"));
        }
        if let Some(o) = obstacles::authored(&assembly, index, attachment) {
            assembly.obstacles.push(o);
        } else {
            assembly.issues.push(format!(
                "{}: obstacle no longer has safe clearance",
                attachment.piece
            ));
        }
    }
    if source.grounded_supports { grounding::apply(&mut assembly)?; }
    assembly.issues.sort();
    assembly.issues.dedup();
    Ok(assembly)
}
pub(super) fn joined(a: &Sample, b: &Sample) -> bool {
    distance(a.position_cm, b.position_cm) <= 2
        && (a.mode == "flight"
            || b.mode == "flight"
            || (distance(a.forward, b.forward) <= 20000 && distance(a.normal, b.normal) <= 20000))
        && a.lateral_cm.abs_diff(b.lateral_cm) <= 1
}
pub(super) fn conflict(a: &Piece, b: &Piece) -> bool {
    let mut joints: Vec<_> = [&a.path[0], a.path.last().unwrap()]
        .into_iter()
        .flat_map(|s| {
            [&b.path[0], b.path.last().unwrap()]
                .into_iter()
                .filter(move |t| distance(s.position_cm, t.position_cm) < 3)
                .map(|_| s.position_cm)
        })
        .collect();
    for (first, second) in [(a, b), (b, a)] {
        let drop = portal_drop(&first.id, &second.id, second.width_cm);
        let port = first.path.last().unwrap();
        let end = port.position_cm;
        let start = second.path[0].position_cm;
        if drop > 0
            && distance(
                std::array::from_fn(|j| end[j] - port.normal[j] * drop / 1_000_000),
                start,
            ) <= 2
        {
            joints.push(end);
            joints.push(start);
        }
    }
    let straight_overlap = geometry::straight_overlap(a, b);
    if joints.is_empty() {
        if let Some(overlap) = straight_overlap {
            // Sample phase must not gate continuous straight-road clearance.
            return overlap;
        }
    }
    if !layout::overlaps(a, b) || straight_overlap == Some(false) {
        return false;
    }
    a.path.iter().step_by(2).any(|s| {
        b.path.iter().step_by(2).any(|t| {
            if joints.iter().any(|v| {
                distance(*v, s.position_cm) < u64::from(s.lateral_cm + t.lateral_cm) + 350
                    && distance(*v, t.position_cm) < u64::from(s.lateral_cm + t.lateral_cm) + 350
            }) {
                return false;
            }
            geometry::volume_overlap(s, t)
        })
    })
}
pub fn common_checkpoints(a: &Assembly) -> Vec<(usize, usize)> {
    if let Some(source) = a.authoring.as_ref().or(a.seed_source.as_ref()) {
        return source
            .checkpoints
            .iter()
            .filter_map(|cp| {
                source
                    .instances
                    .iter()
                    .position(|i| i.id == cp.piece)
                    .map(|i| (i, cp.sample))
            })
            .collect();
    }
    automatic_checkpoints(a)
}

/// Select only after every branch and obstacle is known. Authored checkpoints
/// never enter this selector. All selected samples have the same route order.
pub(super) fn automatic_checkpoints(a: &Assembly) -> Vec<(usize, usize)> {
    let Some(base) = a.routes.first() else { return vec![]; };
    let mut common = vec![];
    let mut previous = vec![0; a.routes.len()];
    for (order, &piece) in base.pieces.iter().enumerate() {
        let positions: Option<Vec<_>> = a.routes.iter().map(|r|r.pieces.iter().position(|p|*p==piece)).collect();
        if let Some(positions) = positions {
            if !common.is_empty() && positions.iter().zip(&previous).any(|(n,p)|n<=p) {continue;}
            previous=positions;
            common.push((order,piece));
        }
    }
    let Some(start)=common.iter().position(|(_,p)|*p==2) else {return vec![];};
    let common=&common[start..];
    let mut selected=BTreeSet::new();
    let start_key=(common[0].0,a.pieces[2].path.len()/2);
    selected.insert(start_key);
    let ordinary=|i:usize| {
        let p=&a.pieces[i];
        p.width_cm>=400 && p.entry_width_cm>=400 && p.exit_width_cm>=400
            && ["straight","approach","finish_plaza","curve","curve_left","gentle45","gentle45_left",
                "slope","slope_up","slope_down","curve_up","curve_down","curve_left_up","curve_left_down"].contains(&p.id.as_str())
            && !a.obstacles.iter().any(|o|o.piece_index==i)
    };
    let mut run=0;
    for (n,&(order,i)) in common.iter().enumerate() {
        let piece=&a.pieces[i];
        if run==4 {selected.insert((order,0));run=0;}
        if ordinary(i) {run+=1;} else {
            selected.insert((order,0));
            selected.insert((order,piece.path.len()-1));
            run=0;
        }
        if let Some(&(next_order,next))=common.get(n+1) {
            // Every split and merge brackets the complete non-common section,
            // including hazards that exist only on an alternate route.
            let divergent=a.routes.iter().any(|r| {
                let here=r.pieces.iter().position(|p|*p==i).unwrap();
                r.pieces.get(here+1)!=Some(&next)
            });
            if divergent {
                selected.insert((order,piece.path.len()-1));
                selected.insert((next_order,0));
                run=0;
            }
        }
    }
    if let Some(&(order,i))=common.last() {
        let piece=&a.pieces[i];
        let sample=if let Some(plaza)=&a.finish_plaza {
            piece.path.iter().position(|p|p.position_cm==plaza.checkpoint_cm).unwrap_or(piece.path.len()-1)
        } else {piece.path.len()-1};
        selected.insert((order,sample));
    }
    let mut out:Vec<(usize,usize)>=vec![];
    for (order,sample) in selected.into_iter().filter(|v|*v>=start_key) {
        let i=base.pieces[order];
        let position=a.pieces[i].path[sample].position_cm;
        if out.last().is_none_or(|(p,s)|a.pieces[*p].path[*s].position_cm!=position) {
            out.push((i,sample));
        }
    }
    out
}
pub(super) fn checkpoint_budget(a: &Assembly) -> Result<()> {
    let count=common_checkpoints(a).len();
    if count>64 {return Err(error("E_TRACK_CHECKPOINT_LIMIT",format!("Final routes need {count} shared checkpoints; allowed 0..64")));}
    Ok(())
}

pub fn executable(a: &Assembly) -> Result<()> {
    if !a.issues.is_empty() {
        return Err(error("E_TRACK_DRAFT", a.issues.join("; ")));
    }
    Ok(())
}

pub(super) fn action_gimmicks(a: &Assembly) -> Result<Vec<Gimmick>> {
    let Some(source) = a.authoring.as_ref().or(a.seed_source.as_ref()) else {
        return Ok(vec![]);
    };
    let mut out = source.structures.clone();
    for action in &source.actions {
        if action.kind == "manual_flight" { continue; }
        let i = source
            .instances
            .iter()
            .position(|i| i.id == action.piece)
            .ok_or_else(|| error("E_TRACK_SOURCE", "action road missing"))?;
        let sample = &a.pieces[i].path[action.sample];
        let mut g = Gimmick {
            id: format!("action-{}", action.id),
            position: sample.position_cm,
            rotation_mdeg: geometry::euler(geometry::basis(sample)),
            scale_per_mille: [1000; 3],
            parts: vec![],
            track: None,
            curved_faces: vec![],
            effect: Some(Effect {
                strength_percent: 100,
                jump_height_cm: action.height_cm,
                ring_radius_cm: if action.kind == "air_ring" { 150 } else { 250 },
            }),
            surface: Surface::Asphalt,
            color: if action.kind == "jump_panel" { [30, 220, 210, 255] } else { [255, 113, 35, 255] },
            motion: Motion {
                kind: if action.kind == "jump_panel" {
                    MotionKind::JumpHeight
                } else if action.kind == "air_ring" {
                    MotionKind::AirRing
                } else {
                    MotionKind::TargetSpeed
                },
                delta_cm: [0; 3],
                axis: 1,
                period_ms: 4000,
                phase_ms: 0,
                impulse_cmps: [0; 3],
                cooldown_ms: 1500,
            },
            safety_min_cm: sample.position_cm.map(|v| v - 5000),
            safety_max_cm: sample.position_cm.map(|v| v + 5000),
        };
        if action.kind == "air_ring" {
            g.position[1] += i64::from(action.height_cm);
            g.parts.clear();
            for side in [-1, 1] {
                g.parts.push(box_part([side * 175, 0, 0], [50, 400, 30]));
                g.parts.push(box_part([0, side * 175, 0], [300, 50, 30]));
            }
        }
        if action.kind == "boost_chain" {
            let mut station = 0;
            let mut next = 0;
            let path = &a.pieces[i].path;
            for n in action.sample..path.len() {
                if n > action.sample {
                    station += distance(path[n - 1].position_cm, path[n].position_cm);
                }
                if station < next * 500 {
                    continue;
                }
                let mut panel = g.clone();
                panel.id = format!("{}-{next}", panel.id);
                panel.position = path[n].position_cm;
                panel.rotation_mdeg = geometry::euler(geometry::basis(&path[n]));
                panels::fit(a, i, &mut panel, path[n].lateral_cm, action.panel_width_percent, action.panel_alignment)?;
                panel.safety_min_cm = panel.position.map(|v| v - 5000);
                panel.safety_max_cm = panel.position.map(|v| v + 5000);
                out.push(panel);
                next += 1;
                if next == 3 {
                    break;
                }
            }
        } else {
            if action.kind != "air_ring" { panels::fit(a, i, &mut g, sample.lateral_cm, action.panel_width_percent, action.panel_alignment)?; }
            out.push(g);
        }
    }
    Ok(out)
}

/// Several roads and one action, not a monolithic overpass preset.
pub fn shortcut_source() -> Source {
    let mut s = Source::empty();
    s.settings.circuit = false;
    for i in 0..3 {
        let mut p = instance(&format!("start-{i}"), "straight", 400);
        p.position_cm = [0, 0, i * 800];
        s.instances.push(p);
    }
    let mut lower = instance("lower-zigzag", "free_curve", 800);
    lower.entry_width_cm = 400;
    lower.exit_width_cm = 400;
    lower.control_points = vec![
        [0, 0, 2400],
        [0, 0, 3000],
        [-2400, 0, 3200],
        [-2400, 0, 4400],
        [-2400, 0, 5600],
        [2400, 0, 6000],
        [2400, 0, 7200],
        [2400, 0, 8400],
        [0, 0, 9800],
        [0, 0, 10400],
    ];
    s.instances.push(lower);
    let mut join = instance("merge", "straight", 400);
    join.position_cm = [0, 0, 10400];
    s.instances.push(join);
    let mut finish = instance("finish", "finish_plaza", 400);
    finish.position_cm = [0, 0, 11200];
    s.instances.push(finish);
    let mut approach = instance("jump-approach", "free_curve", 200);
    approach.entry_width_cm = 400;
    approach.control_points = vec![[0, 0, 2400], [0, 0, 3000], [800, 0, 3000], [800, 0, 3600]];
    s.instances.push(approach);
    let mut jump = instance("jump-entry", "flight_curve", 200);
    jump.control_points = vec![
        [800, 0, 3600],
        [800, 900, 4000],
        [800, 900, 5100],
        [800, 500, 5600],
    ];
    s.instances.push(jump);
    let mut upper = instance("upper-shortcut", "free_curve", 200);
    upper.control_points = vec![
        [800, 500, 5600],
        [800, 500, 6200],
        [800, 500, 6900],
        [800, 500, 7500],
    ];
    s.instances.push(upper);
    let mut exit = instance("landing-merge", "free_curve", 200);
    exit.exit_width_cm = 400;
    exit.control_points = vec![
        [800, 500, 7500],
        [800, 500, 7900],
        [-2400, 0, 8400],
        [-2400, 0, 8800],
        [-2400, 0, 9600],
        [0, 0, 10000],
        [0, 0, 10400],
    ];
    s.instances.push(exit);
    s.paths = vec![
        Path {
            id: "base".into(),
            pieces: [
                "start-0",
                "start-1",
                "start-2",
                "lower-zigzag",
                "merge",
                "finish",
            ]
            .map(String::from)
            .to_vec(),
        },
        Path {
            id: "shortcut".into(),
            pieces: [
                "start-0",
                "start-1",
                "start-2",
                "jump-approach",
                "jump-entry",
                "upper-shortcut",
                "landing-merge",
                "merge",
                "finish",
            ]
            .map(String::from)
            .to_vec(),
        },
    ];
    for path in &s.paths {
        for pair in path.pieces.windows(2) {
            let edge = Connection {
                from: pair[0].clone(),
                to: pair[1].clone(),
            };
            if !s.connections.contains(&edge) {
                s.connections.push(edge);
            }
        }
    }
    s.checkpoints = vec![
        Checkpoint {
            piece: "start-2".into(),
            sample: 2,
        },
        Checkpoint {
            piece: "merge".into(),
            sample: 0,
        },
        Checkpoint {
            piece: "finish".into(),
            sample: 2,
        },
    ];
    s.actions.push(Action {
        id: "shortcut-launch".into(),
        kind: "jump_panel".into(),
        piece: "jump-approach".into(),
        sample: piece(&s.instances[6]).unwrap().path.len() - 2,
        height_cm: 900,
        panel_width_percent: 50,
        panel_alignment: PanelAlignment::Center,
        landing: Some(Checkpoint {
            piece: "upper-shortcut".into(),
            sample: 0,
        }),
    });
    s
}

pub(super) fn seed_shortcut(a: &Assembly) -> Result<Assembly> {
    let mut source = from_assembly(a);
    let example = shortcut_source();
    source
        .instances
        .extend(example.instances[6..].iter().cloned());
    source.connections.extend([
        Connection {
            from: "piece-2".into(),
            to: "jump-approach".into(),
        },
        Connection {
            from: "jump-approach".into(),
            to: "jump-entry".into(),
        },
        Connection {
            from: "jump-entry".into(),
            to: "upper-shortcut".into(),
        },
        Connection {
            from: "upper-shortcut".into(),
            to: "landing-merge".into(),
        },
        Connection {
            from: "landing-merge".into(),
            to: "piece-4".into(),
        },
    ]);
    let mut route = source.paths[0].clone();
    route.id = "shortcut".into();
    route.pieces.splice(
        3..4,
        [
            "jump-approach".into(),
            "jump-entry".into(),
            "upper-shortcut".into(),
            "landing-merge".into(),
        ],
    );
    source.paths.push(route);
    source.checkpoints.retain(|cp| cp.piece != "piece-3");
    source.actions.push(example.actions[0].clone());
    let compiled = compile(&source)?;
    source.checkpoints=automatic_checkpoints(&compiled).into_iter().map(|(piece,sample)|Checkpoint{piece:source.instances[piece].id.clone(),sample}).collect();
    let mut out = compile(&source)?;
    executable(&out)?;
    checkpoint_budget(&out)?;
    out.authoring = None;
    out.seed_source = Some(source);
    Ok(out)
}

#[cfg(test)]
mod preparation_tests {
    use super::*;

    #[test]
    fn compiler_owned_reuse_keeps_full_external_validation() {
        let mut source = shortcut_source();
        source.grounded_supports = true;
        LAST_COMPILED.with(|cache| *cache.borrow_mut() = None);
        COMPILE_COUNT.with(|count| count.set(0));
        let expected = compile_uncached(&source).unwrap();
        let actual = compile(&source).unwrap();
        assert_eq!(actual, expected); // paths, issues, supports, actions and bounds
        let document = document_from_assembly(actual.clone()).unwrap();
        document.validate().unwrap();
        verify_document(&document).unwrap();
        COMPILE_COUNT.with(|count| assert_eq!(count.get(), 1));
        for change in 0..4 {
            let mut corrupt = document.clone();
            let assembly = corrupt.assembled_track.as_mut().unwrap();
            match change {
                0 => assembly.pieces[0].path[0].position_cm[0] += 1,
                1 => assembly.issues.push("forged issue".into()),
                2 => assembly.supports.clear(),
                _ => assembly.routes[0].estimated_msec += 1,
            }
            assert!(verify_document(&corrupt).is_err(), "tampering {change} passed");
        }
        let mut corrupt = document.clone();
        corrupt.gimmicks[0].position[0] += 1;
        assert!(verify_document(&corrupt).is_err());
        // A caller's mutations cannot affect the cached compiler result.
        let mut changed = actual;
        changed.pieces.clear();
        assert_eq!(compile(&source).unwrap(), expected);
        let cancelled = cancellation::CancellationToken::default();
        cancelled.cancel();
        assert_eq!(cancelled.run(|| compile(&source)).unwrap_err().code, "E_CANCELLED");
        let mut different = source.clone();
        different.instances[0].position_cm[0] += 37;
        assert_eq!(compile(&different).unwrap(), compile_uncached(&different).unwrap());
        assert_eq!(compile(&source).unwrap(), expected);
    }
}

#[cfg(test)]
mod practice_tests {
    use super::*;
    #[test]
    fn manual_flight_keeps_landings_budgets_and_exact_static_structures() {
        let mut source=shortcut_source();
        source.actions[0].kind="manual_flight".into();
        let assembly=compile(&source).unwrap();
        executable(&assembly).unwrap();
        assert!(action_gimmicks(&assembly).unwrap().is_empty());
        let mut missing=source.clone();
        missing.actions[0].landing=None;
        assert_eq!(compile(&missing).unwrap_err().code,"E_TRACK_SOURCE");
        let mut short=source.clone();
        let landing=short.actions[0].landing.as_mut().unwrap();
        let i=short.instances.iter().position(|i|i.id==landing.piece).unwrap();
        landing.sample=assembly.pieces[i].path.len()-1;
        assert!(executable(&compile(&short).unwrap()).is_err());
        let mut undeclared=source.clone();undeclared.actions.clear();
        assert!(executable(&compile(&undeclared).unwrap()).is_err());
        let mut solid=action_gimmicks(&compile(&shortcut_source()).unwrap()).unwrap()[0].clone();
        solid.motion.kind=MotionKind::Static;
        solid.effect=None;
        solid.id="authored-beam".into();
        source.structures.push(solid.clone());
        let document=document_from_assembly(compile(&source).unwrap()).unwrap();
        assert!(document.gimmicks.contains(&solid));
        verify_document(&document).unwrap();
        let mut modified=document.clone();
        modified.gimmicks.iter_mut().find(|g|g.id==solid.id).unwrap().position[0]+=1;
        assert!(verify_document(&modified).is_err());
        source.structures=vec![solid;33];
        assert!(compile(&source).is_err());
        source.structures.clear();
        source.actions=vec![source.actions[0].clone();65];
        assert_eq!(compile(&source).unwrap_err().code,"E_TRACK_BUDGET");
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    fn road(count:usize)->Assembly {
        let mut pieces=vec![];let mut origin=[0;3];
        for _ in 0..count {push_piece(&mut pieces,&mut origin,0,"straight",400,true,[0;3]);}
        layout::finish(pieces,&Settings{circuit:false,..Default::default()})
    }
    #[test]
    fn ordinary_four_pieces_and_exact_endpoints() {
        let a=road(16);
        let cps=common_checkpoints(&a);
        assert_eq!(cps.iter().map(|p|p.0).collect::<Vec<_>>(),[2,6,10,14,15]);
        assert_eq!(cps[0].1,a.pieces[2].path.len()/2);
        assert_eq!(cps.last().unwrap().1,a.pieces[15].path.len()-1);
        let mut pieces=a.pieces;let mut origin=pieces.last().unwrap().path.last().unwrap().position_cm;
        push_piece(&mut pieces,&mut origin,0,"finish_plaza",400,true,[0;3]);
        let sprint=layout::finish(pieces,&a.settings);
        let end=*common_checkpoints(&sprint).last().unwrap();
        assert_eq!(sprint.pieces[end.0].path[end.1].position_cm,sprint.finish_plaza.unwrap().checkpoint_cm);
    }
    #[test]
    fn mandatory_hazards_deduplicate_seams_and_limit() {
        let mut a=road(12);
        a.pieces[4].id="sharp_curve".into();
        a.pieces[5].width_cm=200;
        let cps=common_checkpoints(&a);
        let points:Vec<_>=cps.iter().map(|(p,s)|a.pieces[*p].path[*s].position_cm).collect();
        for p in [4,5] {for s in [0,a.pieces[p].path.len()-1] {assert!(points.contains(&a.pieces[p].path[s].position_cm));}}
        assert!(!points.windows(2).any(|p|p[0]==p[1]));
        assert_eq!(checkpoint_budget(&road(280)).unwrap_err().code,"E_TRACK_CHECKPOINT_LIMIT");
    }
    #[test]
    fn final_branch_order_and_authored_preservation() {
        let source=shortcut_source();let a=compile(&source).unwrap();
        let manual=common_checkpoints(&a);
        assert_eq!(manual.len(),source.checkpoints.len());
        let selected=automatic_checkpoints(&a);
        assert_ne!(selected,manual);
        for route in &a.routes {
            let keys:Vec<_>=selected.iter().map(|(piece,sample)|(route.pieces.iter().position(|i|i==piece).unwrap(),*sample)).collect();
            assert!(keys.windows(2).all(|w|w[0]<w[1]));
        }
        assert!(selected.contains(&(2,a.pieces[2].path.len()-1))); // before split
        assert!(selected.iter().any(|(p,s)|*p==4 && *s==0)); // after merge
        assert_eq!(common_checkpoints(&a),manual);
    }
}
