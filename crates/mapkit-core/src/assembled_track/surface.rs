//! Stable attachments shared by authored ordinary roads and track instances.
use super::*;

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq,JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Ordinary road ID, or `track:<authoring instance ID>` (never array index).
    pub surface_id:String,
    pub station_cm:u64,
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq,JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub id:String,
    pub surface:Reference,
    pub kind:String,
    pub height_cm:u32,
    pub panel_width_percent:u8,
    pub panel_alignment:authoring::PanelAlignment,
    pub side:i64,
}

pub fn path(d:&MapDocument,id:&str)->Result<Vec<Sample>> {
    if let Some(id)=id.strip_prefix("track:") {
        let a=d.assembled_track.as_ref().ok_or_else(||error("E_SURFACE_REFERENCE","track missing"))?;
        let source=a.authoring.as_ref().ok_or_else(||error("E_SURFACE_REFERENCE","editable track required"))?;
        let index=source.instances.iter().position(|i|i.id==id).ok_or_else(||error("E_SURFACE_REFERENCE","track instance missing"))?;
        return Ok(a.pieces[index].path.clone());
    }
    let r=d.roads.iter().find(|r|r.id==id).ok_or_else(||error("E_SURFACE_REFERENCE","ordinary road missing"))?;
    crate::road_design::path(r)
}

pub fn resolve(d:&MapDocument,r:&Reference)->Result<Sample> {
    let path=path(d,&r.surface_id)?;
    let length=path.windows(2).map(|w|distance(w[0].position_cm,w[1].position_cm)).sum::<u64>();
    if r.station_cm>length {return Err(error("E_SURFACE_REFERENCE","station is beyond the current surface"));}
    Ok(obstacles::sample(&path,r.station_cm))
}

pub fn products(d:&MapDocument,items:&[Attachment])->Result<Vec<Gimmick>> {
    Ok(derived(d,items)?.0)
}
pub fn derived(d:&MapDocument,items:&[Attachment])->Result<(Vec<Gimmick>,Vec<crate::grind::GrindLine>)> {
    if items.len()>64 {return Err(error("E_BUDGET","at most 64 shared surface attachments"));}
    let mut ids=BTreeSet::new();let mut out=Vec::new();let mut lines=Vec::new();
    for item in items {
        if item.id.is_empty() || item.id.len()>64 || !ids.insert(&item.id)
            || !(50..=1000).contains(&item.height_cm) || ![25,50,75,100].contains(&item.panel_width_percent) || ![-1,1].contains(&item.side) {
            return Err(error("E_SURFACE_ATTACHMENT","invalid attachment settings"));
        }
        let path=path(d,&item.surface.surface_id)?;
        let sample=resolve(d,&item.surface)?;
        if !sample.safe || ["loop","cylinder","halfpipe","flight"].contains(&sample.mode.as_str()) {
            return Err(error("E_SURFACE_ATTACHMENT","attachment requires a supported ordinary surface"));
        }
        let mut source=authoring::Source::empty();source.instances.push(authoring::instance("surface","straight",400));
        let mut a=authoring::compile(&source)?;
        a.pieces[0].path=path.clone();a.pieces[0].width_cm=sample.lateral_cm*2;
        let mut distance_cm=0;let mut sample_index=0;
        for (i,pair) in path.windows(2).enumerate() {
            let length=distance(pair[0].position_cm,pair[1].position_cm);
            if item.surface.station_cm<=distance_cm+length {sample_index=i+1;break;}
            distance_cm+=length;
        }
        if let Some(index)=path.iter().position(|p|p.position_cm==sample.position_cm) {sample_index=index;}
        else {a.pieces[0].path.insert(sample_index,sample.clone());}
        if obstacles::KINDS.contains(&item.kind.as_str()) {
            let attachment=authoring::Attachment{kind:item.kind.clone(),piece:"surface".into(),path:"main".into(),station_cm:item.surface.station_cm,side:item.side};
            let obstacle=obstacles::authored(&a,0,&attachment).ok_or_else(||error("E_SURFACE_ATTACHMENT","obstacle has insufficient driving clearance"))?;
            let mut g=obstacles::gimmick(&a,&obstacle,0);g.id=format!("surface-attachment-{}",item.id);out.push(g);
            if item.kind=="grind_rail" {
                a.obstacles=vec![obstacle];
                for mut line in obstacles::rail_lines(&a) {line.id=format!("surface-rail-{}",&crate::sha256(item.id.as_bytes())[..16]);lines.push(line);}
            }
        } else {
            if !["jump_panel","acceleration_panel","boost_chain","air_ring"].contains(&item.kind.as_str()) {return Err(error("E_SURFACE_ATTACHMENT","unknown attached action"));}
            source.actions.push(authoring::Action{id:item.id.clone(),kind:item.kind.clone(),piece:"surface".into(),sample:sample_index,height_cm:item.height_cm,panel_width_percent:item.panel_width_percent,panel_alignment:item.panel_alignment,landing:None});
            a.authoring=Some(source);
            for (i,mut g) in authoring::action_gimmicks(&a)?.into_iter().enumerate() {
                g.id=format!("surface-attachment-{}-{i}",item.id);
                if !item.surface.surface_id.starts_with("track:") && item.kind!="air_ring" {
                    panels::fit_road(d,&item.surface.surface_id,&mut g,sample.lateral_cm,item.panel_width_percent,item.panel_alignment)?;
                }
                out.push(g);
            }
        }
    }
    Ok((out,lines))
}

pub fn apply(d:&MapDocument,items:Vec<Attachment>)->Result<MapDocument> {
    let (old,old_lines)=derived(d,&d.surface_attachments)?;
    let mut next=d.clone();next.gimmicks.retain(|g|!old.iter().any(|v|v==g));
    next.grind_lines.retain(|g|!old_lines.contains(g));
    let (new,lines)=derived(&next,&items)?;
    if new.iter().any(|g|next.gimmicks.iter().any(|v|v.id==g.id)) || lines.iter().any(|g|next.grind_lines.iter().any(|v|v.id==g.id)) {return Err(error("E_SURFACE_OWNERSHIP","attachment product identity already in use"));}
    next.surface_attachments=items;next.gimmicks.extend(new);next.grind_lines.extend(lines);next.normalize();next.validate()?;Ok(next)
}

pub fn refresh(d:&mut MapDocument)->Result<()> {
    let (new,lines)=derived(d,&d.surface_attachments)?;
    for g in new {
        let target=d.gimmicks.iter_mut().find(|v|v.id==g.id).ok_or_else(||error("E_SURFACE_OWNERSHIP","attachment product missing"))?;
        *target=g;
    }
    for line in lines {
        let target=d.grind_lines.iter_mut().find(|v|v.id==line.id).ok_or_else(||error("E_SURFACE_OWNERSHIP","attachment interaction line missing"))?;
        *target=line;
    }
    Ok(())
}

pub fn validate(d:&MapDocument)->Result<()> {
    if d.surface_attachments.len()>64 {return Err(error("E_BUDGET","at most 64 shared surface attachments"));}
    let mut ids=BTreeSet::new();
    for item in &d.surface_attachments {
        if item.id.is_empty() || item.id.len()>64 || !ids.insert(&item.id) || !(50..=1000).contains(&item.height_cm)
            || ![25,50,75,100].contains(&item.panel_width_percent) || ![-1,1].contains(&item.side) {
            return Err(error("E_SURFACE_ATTACHMENT","invalid stored attachment"));
        }
        resolve(d,&item.surface)?;
    }
    Ok(())
}
pub fn verify(d:&MapDocument)->Result<()> {
    validate(d)?;
    composite::verify_connections(d)?;
    let (objects,lines)=derived(d,&d.surface_attachments)?;
    for expected in objects {
        if !d.gimmicks.iter().any(|g|g==&expected) {return Err(error("E_SURFACE_REFERENCE","attachment no longer matches its surface"));}
    }
    for expected in lines {
        if !d.grind_lines.contains(&expected) {return Err(error("E_SURFACE_REFERENCE","grind interaction no longer matches its surface"));}
    }
    Ok(())
}
