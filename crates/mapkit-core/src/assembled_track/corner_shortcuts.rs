//! Large ordinary corners and optional, independently sampled grind branches.
use super::*;
use authoring::{Action, Checkpoint, Connection, Path, PanelAlignment, Source};

pub(super) const IDS: [&str; 8] = [
    "grind_uturn_left", "grind_uturn_right", "grind_s_left", "grind_s_right",
    "grind_spiral_left_up", "grind_spiral_left_down",
    "grind_spiral_right_up", "grind_spiral_right_down",
];

pub(super) fn template(preset: &str) -> Option<&'static str> {
    let id = base_preset(preset);
    match id {
        "hairpin" | "banked_hairpin" => Some("grind_uturn_right"),
        "hairpin_left" | "banked_hairpin_left" => Some("grind_uturn_left"),
        "spatial_s" => Some("grind_s_right"),
        "spatial_s_left" => Some("grind_s_left"),
        "spiral360_right_up" => Some("grind_spiral_right_up"),
        "spiral360_right_down" => Some("grind_spiral_right_down"),
        "spiral360_left_up" => Some("grind_spiral_left_up"),
        "spiral360_left_down" => Some("grind_spiral_left_down"),
        _ => None,
    }
}

// An immutable per-corner ticket. Layout retries and backtracking cannot redraw
// this independent stream; the nth accepted large corner always has ticket n.
pub(super) fn selected(seed: u64, ordinal: usize) -> bool {
    let mut rng = seed ^ 0x6772_696e_6430_3031;
    let mut chosen = false;
    for _ in 0..=ordinal {
        loop {
            let ticket=next(&mut rng);
            if ticket<u64::MAX {chosen=ticket%3==0;break;}
        }
    }
    chosen
}

fn curve(id: &str, points: Vec<Vertex>, width: u32) -> authoring::Instance {
    let mut p = authoring::instance(id, "free_curve", width);
    p.control_points = points;
    p
}
fn connections(s: &mut Source) {
    s.connections.clear();
    for path in &s.paths {
        for w in path.pieces.windows(2) {
            let edge = Connection { from: w[0].clone(), to: w[1].clone() };
            if !s.connections.contains(&edge) { s.connections.push(edge); }
        }
    }
}

pub(super) fn source(id: &str, width: u32, rail: bool) -> Result<Source> {
    if !IDS.contains(&id) { return Err(error("E_TRACK_SOURCE", "unknown corner template")); }
    let uturn = id.contains("uturn");
    let spiral = id.contains("spiral");
    let rise = if spiral { if id.ends_with("down") { -600 } else { 600 } } else { 0 };
    let mut main;
    let entry;
    let exit;
    let span;
    let finish;
    if uturn {
        main = vec![[0,0,800],[0,0,2000],[-1400,0,2000],[-1400,0,4000],
            [-1400,0,5878],[122,0,7400],[2000,0,7400],
            [3878,0,7400],[5400,0,5878],[5400,0,4000],
            [5400,0,2000],[4000,0,2000],[4000,0,800]];
        entry = vec![[0,0,800],[0,0,1200],[800,0,1600],[800,0,2200]];
        span = vec![[800,0,2200],[800,0,2400],[800,0,2600],[800,0,2800],
            [800,0,3463],[1337,0,4000],[2000,0,4000],
            [2663,0,4000],[3200,0,3463],[3200,0,2800],
            [3200,0,2600],[3200,0,2400],[3200,0,2200]];
        exit = vec![[3200,0,2200],[3200,0,1600],[4000,0,1200],[4000,0,800]];
        finish = vec![[4000,0,800],[4000,0,533],[4000,0,267],[4000,0,0]];
    } else {
        let end = if spiral { 6800 } else { 8200 };
        if spiral {
            // One full turn with forward drift and smooth rise: its portals are
            // open, and the crossing has vertical vehicle clearance.
            let at = |t: f64| {
                let a = t * std::f64::consts::TAU;
                let p = [-2500.0+2500.0*libm::cos(a), rise as f64*t*t*(3.0-2.0*t),
                    800.0+2500.0*libm::sin(a)+6000.0*t];
                let d = [-2500.0*std::f64::consts::TAU*libm::sin(a), rise as f64*6.0*t*(1.0-t),
                    2500.0*std::f64::consts::TAU*libm::cos(a)+6000.0];
                (p,d)
            };
            main = vec![at(0.0).0.map(round)];
            for n in 0..8 {
                let (a, da) = at(n as f64/8.0);
                let (b, db) = at((n+1) as f64/8.0);
                main.push(std::array::from_fn(|j|round(a[j]+da[j]/24.0)));
                main.push(std::array::from_fn(|j|round(b[j]-db[j]/24.0)));
                main.push(b.map(round));
            }
        } else {
            main = vec![[0,0,800],[0,0,1800],[-6000,0,1800],[-6000,0,3500],
                [-6000,0,4500],[-2800,0,4500],[-2800,0,5500],
                [-2800,0,7200],[0,0,7200],[0,0,end]];
        }
        entry = vec![[0,0,800],[0,0,1000],[1600,0,1200],[1600,0,2400]];
        // A two-span continuous S gives curved lateral movement with flat ends.
        let mid = (2400+end-1600)/2;
        span = vec![[1600,0,2400],[1600,0,2600],[1600,0,2800],[1600,0,3000],
            [1600,0,3200],[2200,rise/2,mid-200],[2200,rise/2,mid],
            [2200,rise/2,mid+200],[1600,rise,end-2400],[1600,rise,end-2200],
            [1600,rise,end-2000],[1600,rise,end-1800],[1600,rise,end-1600]];
        exit = vec![[1600,rise,end-1600],[1600,rise,end-400],[0,rise,end-200],[0,rise,end]];
        finish = vec![[0,rise,end],[0,rise,end+267],[0,rise,end+533],[0,rise,end+800]];
    }
    let sign = if id.contains("left") == uturn { -1 } else { 1 };
    let transform = |points: Vec<Vertex>| points.into_iter().map(|p|[p[0]*sign,p[1],p[2]]).collect();
    let mut s = Source::empty();
    s.settings.circuit = false;
    s.instances = vec![curve("split",vec![[0,0,0],[0,0,267],[0,0,533],[0,0,800]],800),
        curve("corner",transform(main),width),curve("merge",transform(finish),800)];
    s.instances[1].entry_width_cm = 800;
    s.instances[1].exit_width_cm = 800;
    s.paths.push(Path{id:"base".into(),pieces:["split","corner","merge"].map(String::from).to_vec()});
    s.checkpoints = vec![Checkpoint{piece:"split".into(),sample:0},
        Checkpoint{piece:"split".into(),sample:authoring::piece(&s.instances[0])?.path.len()-1},
        Checkpoint{piece:"merge".into(),sample:0},
        Checkpoint{piece:"merge".into(),sample:authoring::piece(&s.instances[2])?.path.len()-1}];
    if rail {
        let mut entry = curve("rail-entry",transform(entry),400);
        entry.entry_width_cm = 800;
        let mut exit = curve("rail-exit",transform(exit),400);
        exit.exit_width_cm = 800;
        let mut flight = curve("rail-span",transform(span),400);
        flight.preset = "flight_curve".into();
        let mut points = flight.control_points.clone();
        for p in &mut points { p[1] += 40; }
        let line = crate::grind::GrindLine{id:"rail".into(),control_points:points,up:[0,1_000_000,0],
            capture_width_cm:40,start_connections:vec![],end_connections:vec![]};
        s.structures = rail_solids(&line);
        s.grind_lines.push(line);
        s.actions.push(Action{id:"rail-jump".into(),kind:"manual_flight".into(),piece:entry.id.clone(),
            sample:authoring::piece(&entry)?.path.len()-2,height_cm:800,panel_width_percent:50,
            panel_alignment:PanelAlignment::Center,landing:Some(Checkpoint{piece:exit.id.clone(),sample:0})});
        s.instances.extend([entry,flight,exit]);
        s.paths.push(Path{id:"grind".into(),pieces:["split","rail-entry","rail-span","rail-exit","merge"].map(String::from).to_vec()});
        if !saves_length(&s)? { return Err(error("E_TRACK_CLEARANCE","corner rail must save 20% and 5 m including entry and exit")); }
    }
    connections(&mut s);
    Ok(s)
}

pub(super) fn saves_length(s: &Source) -> Result<bool> {
    let length = |p: &Path| -> Result<u64> { p.pieces.iter().skip(1).take(p.pieces.len()-2)
        .map(|id| authoring::piece(s.instances.iter().find(|i|i.id==*id).unwrap())
            .map(|p|p.path.windows(2).map(|w|distance(w[0].position_cm,w[1].position_cm)).sum::<u64>())).sum() };
    let main = length(&s.paths[0])?;
    let branch = length(&s.paths[1])? + 80; // vertical jump/landing allowance
    Ok(branch*5<=main*4 && main.saturating_sub(branch)>=500)
}

/// Build shared checkpoints only after all route combinations are present.
pub(super) fn with_checkpoints(mut source: Source) -> Result<Source> {
    let start = source.checkpoints.first().cloned();
    source.checkpoints.clear();
    let a = authoring::compile(&source)?;
    authoring::executable(&a)?;
    let start = start.and_then(|cp|source.instances.iter().position(|i|i.id==cp.piece).map(|i|(i,cp.sample)))
        .unwrap_or((2,a.pieces[2].path.len()-1));
    source.checkpoints = authoring::checkpoints_between(&a,start,None).into_iter().map(|(i,sample)|
        Checkpoint{piece:source.instances[i].id.clone(),sample}).collect();
    if source.checkpoints.is_empty() {return Err(error("E_TRACK_CHECKPOINT_LIMIT","no common checkpoint"));}
    source.checkpoints.dedup();
    authoring::checkpoint_budget(&authoring::compile(&source)?)?;
    Ok(source)
}

/// Transactional optional insertion: no failed candidate changes the base graph.
pub(super) fn install(source: &Source, fragment: &Source, ids: &[String], prefix: &str) -> Result<Option<Source>> {
    let attempt = || -> Result<Source> {
        if !saves_length(fragment)? {return Err(error("E_TRACK_CLEARANCE","insufficient shortcut saving"));}
        let mut candidate = source.clone();
        templates::merge(&mut candidate,fragment,ids,prefix);
        candidate = with_checkpoints(candidate)?;
        let a = authoring::compile(&candidate)?;
        if !clear(&a, prefix, ids)? {return Err(error("E_TRACK_CLEARANCE","rail corridor is occupied"));}
        document_from_assembly(a)?; // Existing document, structure and resource limits.
        Ok(candidate)
    };
    match attempt() {
        Ok(candidate) => Ok(Some(candidate)),
        Err(e) if e.code=="E_CANCELLED" => Err(e),
        Err(_) => Ok(None),
    }
}

pub(super) fn clear(a: &Assembly, prefix: &str, base_ids: &[String]) -> Result<bool> {
    let source = a.authoring.as_ref().unwrap();
    let line_id = format!("{prefix}-rail");
    let line = source.grind_lines.iter().find(|l|l.id==line_id).unwrap();
    let owned = |id: &str| id==base_ids[0] || id==base_ids[base_ids.len()-1]
        || id==format!("{prefix}-rail-entry") || id==format!("{prefix}-rail-exit") || id==format!("{prefix}-rail-span");
    let roads: Vec<_> = source.instances.iter().zip(&a.pieces).filter(|(i,_)|!owned(&i.id)).map(|(_,p)|p.clone()).collect();
    let mut corridor = Source::empty(); corridor.grind_lines.push(line.clone());
    if !templates::clear_of_roads(&corridor,&roads)? {return Ok(false);}
    let mut boxes = vec![];
    let mut objects = gimmicks(a)?;
    objects.extend(source.structures.iter().filter(|g|!g.id.starts_with(&format!("authored-{prefix}-"))).cloned());
    for g in objects {
        for part in &g.parts {
            let vertices: Vec<_> = part.vertices.iter().map(|v|add(g.position,geometry::rotate3(
                std::array::from_fn(|j|v[j]*i64::from(g.scale_per_mille[j])/1000),g.rotation_mdeg))).collect();
            boxes.push(bounds(&vertices));
        }
    }
    boxes.extend(a.supports.iter().map(|s|bounds(&s.shape.vertices)));
    for other in source.grind_lines.iter().filter(|l|l.id!=line_id) {
        for w in other.samples().windows(2) {
            let (lo,hi)=bounds(&[w[0].position_cm,w[1].position_cm]);
            boxes.push((add(lo,[-220,-40,-220]),add(hi,[220,235,220])));
        }
    }
    let mut paths = vec![(line.samples(),-40)];
    for (i,p) in source.instances.iter().zip(&a.pieces) {
        if i.id==format!("{prefix}-rail-entry") || i.id==format!("{prefix}-rail-exit") {paths.push((p.path.clone(),1));}
    }
    for (path,low) in paths {for w in path.windows(2) {
        cancellation::checkpoint()?;
        let steps=distance(w[0].position_cm,w[1].position_cm).div_ceil(50).max(1);
        for step in 0..=steps {
            let center:Vertex=std::array::from_fn(|j|w[0].position_cm[j]+(w[1].position_cm[j]-w[0].position_cm[j])*step as i64/steps as i64);
            let lo=[center[0]-220,center[1]+low,center[2]-220];
            let hi=[center[0]+220,center[1]+235,center[2]+220];
            if boxes.iter().any(|(b,c)|(0..3).all(|j|lo[j]<c[j] && b[j]<hi[j])) {return Ok(false);}
        }
    }}
    Ok(true)
}
fn bounds(vertices: &[Vertex]) -> (Vertex,Vertex) {
    (std::array::from_fn(|j|vertices.iter().map(|v|v[j]).min().unwrap()),
     std::array::from_fn(|j|vertices.iter().map(|v|v[j]).max().unwrap()))
}

// Identical transported cross sections supply the top contact line, visible beam
// and convex collision. Each chunk is a v1 structure with at most 32 convexes.
pub(super) fn rail_solids(line: &crate::grind::GrindLine) -> Vec<Gimmick> {
    let samples = line.samples();
    let rings: Vec<[Vertex;4]> = samples.iter().map(|s| {
        let basis = geometry::basis(s);
        std::array::from_fn(|i|std::array::from_fn(|j| s.position_cm[j]
            + round(basis[j][0]*if i&1==0 {-20.0} else {20.0})
            + round(basis[j][1]*if i&2==0 {-40.0} else {0.0})))
    }).collect();
    rings.windows(2).collect::<Vec<_>>().chunks(32).enumerate().map(|(n,chunk)| {
        let position = chunk[0][0][0];
        let parts: Vec<_> = chunk.iter().map(|pair| {
            let mut part = box_part([0;3],[40;3]);
            part.vertices = pair.iter().flatten().map(|v|std::array::from_fn(|j|v[j]-position[j])).collect();
            part.faces = hull_faces(&part.vertices);
            part
        }).collect();
        let radius = parts.iter().flat_map(|p|&p.vertices).map(|v|v.iter().map(|n|n.abs()).sum::<i64>()).max().unwrap();
        Gimmick{id:format!("authored-rail-{n}"),position,rotation_mdeg:[0;3],scale_per_mille:[1000;3],parts,
            track:None,curved_faces:vec![],effect:None,surface:Surface::Concrete,color:[255,200,40,255],
            motion:Motion{kind:MotionKind::Static,delta_cm:[0;3],axis:1,period_ms:4000,phase_ms:0,impulse_cmps:[0;3],cooldown_ms:1500},
            safety_min_cm:position.map(|v|v-radius),safety_max_cm:position.map(|v|v+radius)}
    }).collect()
}

// Eight rounded ring vertices can have non-coplanar side quads. Triangulate
// their actual convex hull instead of assuming a diagonal is an outward face.
fn hull_faces(vertices: &[Vertex]) -> Vec<[u8;3]> {
    let mut facets = std::collections::BTreeMap::new();
    for a in 0..vertices.len() { for b in a+1..vertices.len() { for c in b+1..vertices.len() {
        let u: [i128;3] = std::array::from_fn(|j|(vertices[b][j]-vertices[a][j]) as i128);
        let v: [i128;3] = std::array::from_fn(|j|(vertices[c][j]-vertices[a][j]) as i128);
        let mut n = [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        if n==[0;3] {continue;}
        let sides: Vec<i128> = vertices.iter().map(|p|(0..3).map(|j|n[j]*(p[j]-vertices[a][j]) as i128).sum()).collect();
        if sides.iter().any(|v|*v>0) && sides.iter().any(|v|*v<0) {continue;}
        if sides.iter().all(|v|*v==0) {continue;}
        if sides.iter().any(|v|*v>0) {n=n.map(|v|-v);}
        let on: Vec<usize> = sides.iter().enumerate().filter(|(_,s)|**s==0).map(|(i,_)|i).collect();
        facets.entry(on).or_insert(n);
    }}}
    let mut out = vec![];
    for (mut on,n) in facets {
        let axis = (0..3).max_by_key(|j|n[*j].abs()).unwrap();
        let x = (axis+1)%3; let y = (axis+2)%3;
        let center = [x,y].map(|j|on.iter().map(|i|vertices[*i][j] as f64).sum::<f64>()/on.len() as f64);
        on.sort_by(|a,b| {
            let angle = |i:usize|libm::atan2(vertices[i][y] as f64-center[1],vertices[i][x] as f64-center[0]);
            angle(*a).total_cmp(&angle(*b))
        });
        if n[axis]<0 {on.reverse();}
        for i in 1..on.len()-1 {out.push([on[0] as u8,on[i] as u8,on[i+1] as u8]);}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corner_templates_are_executable() {
        for id in IDS { for width in [400,600,800] {
            let s=source(id,width,true).unwrap_or_else(|e|panic!("{id}/{width}: {e:?}"));
            let a=authoring::compile(&s).unwrap();
            assert!(a.issues.is_empty(),"{id}/{width}: {:?}",a.issues);
            assert!(s.structures.iter().all(Gimmick::valid),"{id}: invalid beam");
            assert!(saves_length(&s).unwrap());
            assert!(a.routes[1].estimated_msec<a.routes[0].estimated_msec);
        }}
    }
    #[test]
    fn corner_tickets_are_independent_and_near_one_third() {
        let count=(0..3000).filter(|&seed|selected(seed,0)).count();
        assert!((900..1100).contains(&count),"{count}");
        let first:Vec<_>=(0..64).map(|seed|selected(seed,0)).collect();
        let second:Vec<_>=(0..64).map(|seed|selected(seed,1)).collect();
        assert_ne!(first,second,"each corner has its own ticket");
    }
    #[test]
    fn optional_install_is_atomic_and_checks_sampled_corridors_and_budgets() {
        let base=source("grind_s_right",800,false).unwrap();
        let fragment=source("grind_s_right",800,true).unwrap();
        let ids=base.paths[0].pieces.clone();
        let installed=install(&base,&fragment,&ids,"one").unwrap();
        assert!(installed.is_some(),"clear corner should admit a rail");
        assert_eq!(base.paths.len(),1);
        let line=&fragment.grind_lines[0];
        let samples=line.samples();
        let at=samples[samples.len()/2].position_cm;
        let mut obstacle=fragment.structures[0].clone();
        obstacle.id="authored-blocker".into();
        obstacle.position=add(at,[0,100,0]);
        obstacle.parts=vec![box_part([0;3],[100;3])];
        obstacle.safety_min_cm=obstacle.position.map(|v|v-150);
        obstacle.safety_max_cm=obstacle.position.map(|v|v+150);
        let mut blocked=base.clone(); blocked.structures.push(obstacle.clone());
        assert!(install(&blocked,&fragment,&ids,"one").unwrap().is_none());
        assert_eq!(blocked.paths,base.paths);
        let mut budget=base.clone();
        obstacle.position=[20000,0,20000];
        obstacle.safety_min_cm=obstacle.position.map(|v|v-150);
        obstacle.safety_max_cm=obstacle.position.map(|v|v+150);
        for i in 0..32 {let mut g=obstacle.clone();g.id=format!("authored-budget-{i}");budget.structures.push(g);}
        assert!(install(&budget,&fragment,&ids,"one").unwrap().is_none());
        let mut rail_blocked=base.clone();
        let mut other=line.clone();other.id="other".into();rail_blocked.grind_lines.push(other);
        assert!(install(&rail_blocked,&fragment,&ids,"one").unwrap().is_none());
        let mut grounded=base;grounded.grounded_supports=true;
        assert!(install(&grounded,&fragment,&ids,"one").unwrap().is_some());
    }
    #[test]
    fn beam_top_matches_every_transported_line_sample() {
        for id in IDS {for yaw in [0,45000,137000] {
            let mut port=authoring::piece(&authoring::instance("origin","straight",800)).unwrap().path[0].clone();
            port.position_cm=[1350,700,-2600];
            port.forward=geometry::rotate3(port.forward,[0,yaw,0]);
            let s=templates::place(source(id,800,true).unwrap(),&port).unwrap();
            assert!(s.structures.iter().all(Gimmick::valid));
            let line=&s.grind_lines[0];
            let samples=line.samples();
            let parts:Vec<_>=s.structures.iter().flat_map(|g|g.parts.iter().map(move |p|(g,p))).collect();
            assert_eq!(parts.len(),samples.len()-1);
            for (i,(g,p)) in parts.iter().enumerate() {for end in 0..2 {
                let top:[Vertex;2]=[2,3].map(|j|add(g.position,p.vertices[end*4+j]));
                let center:Vertex=std::array::from_fn(|j|(top[0][j]+top[1][j])/2);
                assert!(distance(center,samples[i+end].position_cm)<=1);
                assert!((39..=41).contains(&distance(top[0],top[1])));
            }}
        }}
    }
    #[test]
    fn two_shortcuts_expand_the_cartesian_route_set() {
        let first=source("grind_s_right",800,true).unwrap();
        let both=templates::append(&first,"grind_s_right","merge","two").unwrap();
        let both=with_checkpoints(both).unwrap();
        let a=authoring::compile(&both).unwrap();
        authoring::executable(&a).unwrap();
        assert_eq!(both.paths.len(),4);
        let masks:std::collections::BTreeSet<_>=both.paths.iter().map(|p|(
            p.pieces.contains(&"rail-span".into()),p.pieces.contains(&"two-rail-span".into()))).collect();
        assert_eq!(masks,[(false,false),(true,false),(false,true),(true,true)].into());
        for route in &both.paths {
            let order:Vec<_>=both.checkpoints.iter().map(|cp|(route.pieces.iter().position(|p|p==&cp.piece).unwrap(),cp.sample)).collect();
            assert!(order.windows(2).all(|w|w[0]<w[1]));
        }
        assert_eq!(authoring::compile(&authoring::from_assembly(&a)).unwrap(),a);
    }

}
