//! Read-only course proposals. No route is guessed and no source is rewritten.
use super::{authoring::{self, Checkpoint, Source}, *};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub blocking: bool,
    pub piece: Option<String>,
    pub position_cm: Option<Vertex>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Suggestion {
    pub checkpoints: Vec<Checkpoint>,
    pub gates: Vec<crate::course::Checkpoint>,
    pub diagnostics: Vec<Diagnostic>,
    pub applicable: bool,
}
impl Suggestion {
    fn issue(&mut self, code: &str, message: impl Into<String>, blocking: bool, source: &Source, a: &Assembly, piece: Option<usize>) {
        self.diagnostics.push(Diagnostic {
            code: code.into(), message: message.into(), blocking,
            piece: piece.map(|i| source.instances[i].id.clone()),
            position_cm: piece.and_then(|i| a.pieces[i].path.last().map(|s| s.position_cm)),
        });
    }
}

pub fn suggest(source: &Source) -> Result<Suggestion> {
    if !authoring::within_source_limits(source) {
        return Err(error("E_TRACK_BUDGET", "authoring source exceeds bounded graph budget"));
    }
    let mut candidate = source.clone();
    candidate.checkpoints.clear();
    let a = authoring::compile(&candidate)?;
    let mut out = Suggestion { checkpoints: vec![], gates: vec![], diagnostics: vec![], applicable: false };
    for message in &a.issues {
        let located = source.instances.iter().position(|i| message.contains(&format!("{}:",i.id)) || message.contains(&format!("{} →",i.id)) || message.contains(&format!("{} /",i.id)));
        out.issue("E_TRACK_ROUTE", message.clone(), true, source, &a, located);
    }
    // An edge outside the declared paths is an unresolved branch, never an implicit route choice.
    for edge in &source.connections {
        let declared = source.paths.iter().any(|p| p.pieces.windows(2).any(|w| w[0]==edge.from && w[1]==edge.to)
            || source.settings.circuit && p.pieces.last()==Some(&edge.from) && p.pieces.first()==Some(&edge.to));
        if !declared {
            out.issue("E_TRACK_BRANCH", "Choose a base or alternative route for this connection", true, source, &a,
                source.instances.iter().position(|i|i.id==edge.from));
        }
    }
    // Give a location for missing edges (the compiler's route message alone has none).
    for path in &source.paths {
        let mut edges: Vec<_> = path.pieces.windows(2).map(|w|(&w[0],&w[1])).collect();
        if source.settings.circuit && !path.pieces.is_empty() { edges.push((path.pieces.last().unwrap(),&path.pieces[0])); }
        for (from,to) in edges {
            if !source.connections.iter().any(|c|&c.from==from && &c.to==to) {
                out.issue("E_TRACK_CONNECTION", format!("{from} → {to}: choose and connect these ports"), true, source, &a,
                    source.instances.iter().position(|i|&i.id==from));
            }
        }
    }
    let Some(base) = a.routes.first().filter(|r|!r.pieces.is_empty()) else { return Ok(out); };
    let first=base.pieces[0];
    let last=*base.pieces.last().unwrap();
    let Some(start_sample)=a.pieces[first].path.iter().position(|s|s.safe) else {
        out.issue("E_TRACK_START", "Choose a supported starting road", true, source, &a, Some(first));
        return Ok(out);
    };
    let start=(first,start_sample);
    let finish=(last,a.pieces[last].path.len()-1);
    let raw=authoring::checkpoints_between(&a,start,Some(finish));
    if raw.first()!=Some(&start) || raw.last()!=Some(&finish) {
        out.issue("E_TRACK_ROUTE", "All routes must share their ordered start and finish", true, source, &a, Some(first));
    }
    let gate=|(p,s):(usize,usize)| crate::course::Checkpoint::from_track_sample(&a.pieces[p].path[s],format!("assembled-road-{p}"));
    let mut accepted=vec![start];
    let mut gates=vec![gate(start)];
    // Reserve the sprint finish before intermediates so overlap removal cannot lose it.
    if !source.settings.circuit && finish!=start {
        let end=gate(finish);
        if crate::course::effective_checkpoint_indices(&[gates[0].clone(),end.clone()]).len()!=2 {
            out.issue("E_TRACK_CHECKPOINT_OVERLAP", "Start and finish overlap; extend the course or choose different endpoints", true, source, &a, Some(last));
        } else { accepted.push(finish); gates.push(end); }
    }
    for key in raw {
        cancellation::checkpoint()?;
        if accepted.contains(&key) { continue; }
        let next=gate(key);
        let mut checked=gates.clone(); checked.push(next.clone());
        if crate::course::effective_checkpoint_indices(&checked).len()!=checked.len() {
            out.issue("W_TRACK_CHECKPOINT_OVERLAP", "Overlapping intermediate checkpoint omitted", false, source, &a, Some(key.0));
            continue;
        }
        accepted.push(key); gates.push(next);
    }
    accepted.sort_by_key(|(p,s)|(base.pieces.iter().position(|i|i==p).unwrap(),*s));
    out.gates=accepted.iter().map(|&key|gate(key)).collect();
    out.checkpoints=accepted.iter().map(|&(p,s)|Checkpoint{piece:source.instances[p].id.clone(),sample:s}).collect();
    if out.checkpoints.len()>64 {
        out.issue("E_TRACK_CHECKPOINT_LIMIT", "Course needs more than 64 shared checkpoints", true, source, &a, None);
    }
    if out.checkpoints.len()<2 {
        out.issue("E_TRACK_CHECKPOINT_COUNT", "Course needs at least two non-overlapping checkpoints", true, source, &a, Some(first));
    }
    out.applicable=!out.diagnostics.iter().any(|d|d.blocking);
    Ok(out)
}
