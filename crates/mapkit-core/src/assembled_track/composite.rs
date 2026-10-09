//! Transactional compilation into an existing document. Generated ownership is
//! derived from the previous assembly, never from a user object's name prefix.
use super::*;

pub fn road_port(document:&MapDocument,link:&authoring::RoadConnection)->Result<Sample> {
    let road=document.roads.iter().find(|r|r.id==link.road).ok_or_else(||error("E_SURFACE_CONNECTION","connected road missing"))?;
    let path=crate::road_design::path(road)?;
    let mut sample=if link.start {path[0].clone()} else {path.last().unwrap().clone()};
    if link.start {
        sample.forward=sample.forward.map(|v|-v);
        if let Some(ribbon)=&mut sample.ribbon_cm {ribbon.swap(0,1);}
    }
    Ok(sample)
}
pub fn align(document:&MapDocument,source:&mut authoring::Source)->Result<()> {
    let mut pending=std::collections::VecDeque::new();
    for link in &source.road_connections {
        let at=source.instances.iter().position(|i|i.id==link.instance).ok_or_else(||error("E_SURFACE_CONNECTION","connected instance missing"))?;
        source.instances[at]=authoring::snap_surface(&source.instances[at],&road_port(document,link)?)?;
        pending.push_back(at);
    }
    let mut seen=BTreeSet::new();
    while let Some(from)=pending.pop_front() {
        if !seen.insert(from) {continue;}
        for edge in &source.connections {
            if edge.from!=source.instances[from].id {continue;}
            let Some(to)=source.instances.iter().position(|i|i.id==edge.to) else {continue;};
            if seen.contains(&to) || source.road_connections.iter().any(|link|link.instance==edge.to) {continue;}
            source.instances[to]=authoring::snap(&source.instances[to],&source.instances[from])?;
            pending.push_back(to);
        }
    }
    Ok(())
}
pub fn verify_connections(document:&MapDocument)->Result<()> {
    let Some(source)=document.assembled_track.as_ref().and_then(|a|a.authoring.as_ref()) else {return Ok(());};
    for link in &source.road_connections {
        let at=source.instances.iter().position(|i|i.id==link.instance).ok_or_else(||error("E_SURFACE_CONNECTION","connected instance missing"))?;
        let expected=authoring::snap_surface(&source.instances[at],&road_port(document,link)?)?;
        if expected!=source.instances[at] {return Err(error("E_SURFACE_CONNECTION",format!("{} / {} port alignment is stale",link.road,link.instance)));}
    }
    Ok(())
}

pub fn environment_base(document:&MapDocument)->Result<MapDocument> {
    let mut base=document.clone();
    let attached:Vec<_>=base.surface_attachments.iter().filter(|a|a.surface.surface_id.starts_with("track:")).cloned().collect();
    let (generated,generated_lines)=surface::derived(document,&attached)?;
    base.gimmicks.retain(|g|!generated.contains(g));
    base.grind_lines.retain(|g|!generated_lines.contains(g));
    base.surface_attachments.retain(|a|!a.surface.surface_id.starts_with("track:"));
    if let Some(a)=base.assembled_track.take() {
        let mut owned=gimmicks(&a)?;owned.extend(authoring::action_gimmicks(&a)?);
        base.gimmicks.retain(|g|!owned.iter().any(|o|o==g));
        let lines=obstacles::grind_lines(&a);
        base.grind_lines.retain(|g|!lines.iter().any(|o|o==g));
    }
    Ok(base)
}

pub fn apply_source(document: &MapDocument, source: &authoring::Source) -> Result<MapDocument> {
    cancellation::checkpoint()?;
    let mut next = document.clone();
    if let Some(previous) = &document.assembled_track {
        verify_products(document, previous)?;
        let mut owned = gimmicks(previous)?;
        owned.extend(authoring::action_gimmicks(previous)?);
        next.gimmicks.retain(|g| !owned.iter().any(|o| o == g));
        let lines = obstacles::grind_lines(previous);
        next.grind_lines.retain(|g| !lines.iter().any(|o| o == g));
    }
    let mut source = source.clone();
    source.terrain_integration = true;
    align(document,&mut source)?;
    let assembly = authoring::compile(&source)?;
    if !assembly.pieces.is_empty() && next.roads.iter().map(|v|&v.id)
        .chain(next.buildings.iter().map(|v|&v.id)).chain(next.placements.iter().map(|v|&v.id))
        .chain(next.surface_areas.iter().map(|v|&v.id)).chain(next.gimmicks.iter().map(|v|&v.id))
        .any(|id|id.starts_with("assembled-")) {
        return Err(error("E_TRACK_OWNERSHIP","an independent object occupies the assembled geometry namespace"));
    }
    let mut owned = gimmicks(&assembly)?;
    owned.extend(authoring::action_gimmicks(&assembly)?);
    let lines = obstacles::grind_lines(&assembly);
    if owned.iter().any(|g| next.gimmicks.iter().any(|o| o.id == g.id))
        || lines.iter().any(|g| next.grind_lines.iter().any(|o| o.id == g.id)) {
        return Err(error("E_TRACK_OWNERSHIP", "track product ID is owned by an independent object"));
    }
    for p in &assembly.pieces {
        if !document.bounds.contains([p.reserved_min_cm[0], p.reserved_min_cm[2]])
            || !document.bounds.contains([p.reserved_max_cm[0], p.reserved_max_cm[2]]) {
            return Err(error("E_TRACK_BOUNDS", format!("{} at {:?} leaves the map", p.id, p.origin_cm)));
        }
    }
    next.gimmicks.extend(owned);
    next.grind_lines.extend(lines);
    next.assembled_track = if source.instances.is_empty() && source.structures.is_empty() && source.grind_lines.is_empty() {None} else {Some(assembly)};
    // Existing courses keep their stale content hash for an explicit revalidation.
    // A map edit must never manufacture player completion evidence.
    surface::refresh(&mut next)?;
    next.normalize();
    next.validate()?;
    cancellation::checkpoint()?;
    Ok(next)
}

/// A disconnected course is still usable in free roam. Geometric hazards always
/// block execution; route/start/checkpoint requirements belong to course publish.
pub fn executable(document: &MapDocument) -> Result<()> {
    let Some(a) = &document.assembled_track else { return Ok(()); };
    if !a.geometry_issues.is_empty() {
        return Err(error("E_TRACK_GEOMETRY", a.geometry_issues.join("; ")));
    }
    if !document.free_roam { authoring::executable(a)?; }
    Ok(())
}
