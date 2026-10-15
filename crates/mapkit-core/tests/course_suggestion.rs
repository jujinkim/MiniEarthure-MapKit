use mapkit_core::assembled_track::{authoring::{self, Connection, Path, Source}, course_suggestion::suggest};

fn chain(presets: &[&str], circuit: bool) -> Source {
    let mut source=Source::empty(); source.settings.circuit=circuit;
    for (i,preset) in presets.iter().enumerate() {
        let mut item=authoring::instance(&format!("p{i}"),preset,400);
        if let Some(last)=source.instances.last() {
            item=authoring::snap(&item,last).unwrap();
            source.connections.push(Connection{from:last.id.clone(),to:item.id.clone()});
        }
        source.instances.push(item);
    }
    source.paths=vec![Path{id:"base".into(),pieces:source.instances.iter().map(|i|i.id.clone()).collect()}];
    if circuit {source.connections.push(Connection{from:source.instances.last().unwrap().id.clone(),to:source.instances[0].id.clone()});}
    source
}

#[test]
fn manual_sprint_is_read_only_and_reserves_endpoints() {
    let source=chain(&["straight";6],false); let before=source.clone();
    let result=suggest(&source).unwrap();
    assert!(result.applicable,"{:?}",result.diagnostics);
    assert_eq!(source,before);
    assert_eq!(result.checkpoints.first().unwrap().piece,"p0");
    assert_eq!(result.checkpoints.last().unwrap().piece,"p5");
    assert_eq!(mapkit_core::course::effective_checkpoint_indices(&result.gates).len(),result.gates.len());
    assert_eq!(result,suggest(&source).unwrap());
    let mut applied=source; applied.checkpoints=result.checkpoints;
    assert!(authoring::compile(&applied).unwrap().issues.is_empty());
}
#[test]
fn circuit_uses_one_shared_start_finish_and_nonoverlapping_gates() {
    let source=chain(&["gentle90";4],true);
    let result=suggest(&source).unwrap();
    assert!(result.applicable,"{:?}",result.diagnostics);
    assert_eq!(result.checkpoints[0].piece,"p0");
    assert!(result.gates.len()>=2);
    assert_eq!(mapkit_core::course::effective_checkpoint_indices(&result.gates).len(),result.gates.len());
}
#[test]
fn disconnected_and_ambiguous_graphs_have_located_blockers() {
    let mut source=chain(&["straight";3],false);
    source.connections.remove(0);
    let result=suggest(&source).unwrap();
    assert!(!result.applicable);
    assert!(result.diagnostics.iter().any(|d|d.code=="E_TRACK_CONNECTION" && d.piece.as_deref()==Some("p0") && d.position_cm.is_some()));
    source.connections.push(Connection{from:"p0".into(),to:"p2".into()});
    assert!(suggest(&source).unwrap().diagnostics.iter().any(|d|d.code=="E_TRACK_BRANCH"));
}
#[test]
fn invalid_references_and_source_budgets_are_errors() {
    let mut source=chain(&["straight";2],false);
    source.paths[0].pieces.push("missing".into());
    assert!(suggest(&source).is_err());
    source.paths[0].pieces.pop();
    source.checkpoints=vec![authoring::Checkpoint{piece:"p0".into(),sample:0};129];
    assert_eq!(suggest(&source).unwrap_err().code,"E_TRACK_BUDGET");
}
#[test]
fn shared_branches_do_not_receive_branch_only_checkpoints() {
    let source=authoring::shortcut_source();
    let result=suggest(&source).unwrap();
    for cp in result.checkpoints { assert!(source.paths.iter().all(|p|p.pieces.contains(&cp.piece))); }
}

#[test]
fn effective_gate_budget_is_reported_without_truncation() {
    let source=chain(&["straight";260],false);
    let result=suggest(&source).unwrap();
    assert!(!result.applicable);
    assert!(result.checkpoints.len()>64);
    assert!(result.diagnostics.iter().any(|d|d.code=="E_TRACK_CHECKPOINT_LIMIT"));
}
