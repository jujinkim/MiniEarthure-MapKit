//! Validate the saved geometry without asking today's generator to reproduce it.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn coordinate(p: Vertex) -> bool { p.iter().all(|v| v.unsigned_abs() <= 10_000_000) }
fn frame(n: Vertex, f: Vertex) -> bool {
    let norm = |v: Vertex| v.iter().map(|n| (*n as f64 / 1e6).powi(2)).sum::<f64>();
    coordinate(n) && coordinate(f) && (norm(n)-1.0).abs()<0.01 && (norm(f)-1.0).abs()<0.01
        && (0..3).map(|i| n[i] as f64*f[i] as f64/1e12).sum::<f64>().abs()<0.02
}
impl Assembly {
    pub fn validate_stored(&self) -> Result<()> {
        let fail = || error("E_TRACK_ASSEMBLY", "invalid saved track geometry, connection or reference");
        self.settings.normalized()?;
        if self.pieces.len()>MAX_PIECES
            || self.pieces.iter().map(|p|p.path.len()+p.alternate_path.len()).sum::<usize>()>MAX_SAMPLES
            || self.routes.len()>64 || self.supports.len()>MAX_SAMPLES || self.obstacles.len()>MAX_SAMPLES
            || self.generator_fingerprint.len()>128 || self.catalogue_fingerprint.len()>128
            || !coordinate(self.floor.min_cm) || !coordinate(self.floor.max_cm)
            || self.floor.min_cm[1]!=self.floor.max_cm[1]
            || [0,2].iter().any(|&i|self.floor.min_cm[i]>=self.floor.max_cm[i]) {
            return Err(fail());
        }
        for p in &self.pieces {
            cancellation::checkpoint()?;
            if !catalogue_ids().contains(&p.id.as_str()) || !(1..=1200).contains(&p.width_cm)
                || !(1..=1200).contains(&p.entry_width_cm) || !(1..=1200).contains(&p.exit_width_cm)
                || p.path.len()<2 || (!p.alternate_path.is_empty() && p.alternate_path.len()<2)
                || !coordinate(p.origin_cm) || !coordinate(p.reserved_min_cm) || !coordinate(p.reserved_max_cm)
                || (0..3).any(|i|p.reserved_min_cm[i]>p.reserved_max_cm[i])
                || p.rotation_mdeg.iter().any(|v|v.unsigned_abs()>360000) || p.quarter_turns>3
                || p.control_points.len()>193 || p.control_points.iter().any(|&p|!coordinate(p)) {
                return Err(fail());
            }
            for path in [&p.path,&p.alternate_path] {
                for s in path {
                    if !coordinate(s.position_cm) || !frame(s.normal,s.forward)
                        || s.ribbon_cm.is_some_and(|r|r.iter().any(|&v|!coordinate(v)))
                        || s.lateral_cm==0 || s.lateral_cm>10000 || s.tube_radius_cm>10000
                        || s.above_cm>100000 || s.below_cm>100000 || s.min_speed_cmps>100000
                        || !["drive","drift","bridge","boost","spiral","flight","loop","cylinder","halfpipe"].contains(&s.mode.as_str()) {
                        return Err(fail());
                    }
                }
                if path.windows(2).any(|w|distance(w[0].position_cm,w[1].position_cm)>100000) {return Err(fail());}
            }
        }
        let mut routes=BTreeSet::new();
        for r in &self.routes {
            if r.id.is_empty() || r.id.len()>128 || !routes.insert(&r.id) || r.pieces.is_empty()
                || r.pieces.len()>MAX_PIECES || r.pieces.iter().any(|&i|i>=self.pieces.len()) {return Err(fail());}
            if self.issues.is_empty() {
                let mut pairs:Vec<_>=r.pieces.windows(2).map(|w|(w[0],w[1])).collect();
                if self.settings.circuit {pairs.push((*r.pieces.last().unwrap(),r.pieces[0]));}
                for (i,j) in pairs {
                    let (a,b)=(&self.pieces[i],&self.pieces[j]);
                    let mut end=a.path.last().unwrap().clone();
                    let drop=portal_drop(&a.id,&b.id,b.width_cm);
                    for k in 0..3 {end.position_cm[k]-=end.normal[k]*drop/1_000_000;}
                    if !authoring::joined(&end,&b.path[0]) {return Err(fail());}
                }
            }
        }
        if self.issues.is_empty() && (self.pieces.is_empty() || self.routes.is_empty()) {return Err(fail());}
        for s in &self.supports {
            if s.piece_index>=self.pieces.len() || !s.shape.valid(10_000_000) {return Err(fail());}
        }
        for o in &self.obstacles {
            if o.piece_index>=self.pieces.len() || !obstacles::KINDS.contains(&o.kind.as_str())
                || !["main","alternate"].contains(&o.path.as_str()) || !coordinate(o.position_cm)
                || !frame(o.normal,o.forward) || o.half_width_cm>10000
                || o.jump_position_cm.is_some_and(|p|!coordinate(p)) {return Err(fail());}
            let p=&self.pieces[o.piece_index];
            let path=if o.path=="alternate" {&p.alternate_path} else {&p.path};
            let len=path.windows(2).map(|w|distance(w[0].position_cm,w[1].position_cm)).sum::<u64>();
            if path.len()<2 || o.station_cm>len {return Err(fail());}
        }
        if let Some(f)=&self.finish_plaza {
            if f.piece_index>=self.pieces.len() || !frame(f.normal,f.forward)
                || [f.center_cm,f.recovery_cm,f.checkpoint_cm].iter().any(|&p|!coordinate(p))
                || !(1..=100000).contains(&f.radius_cm) {return Err(fail());}
        }
        for source in [&self.authoring,&self.seed_source].into_iter().flatten() {
            if source.instances.len()!=self.pieces.len() || source.connections.len()>MAX_PIECES*4
                || source.paths.len()>64 || source.checkpoints.len()>64 || source.actions.len()>MAX_SAMPLES
                || source.attachments.len()>MAX_SAMPLES {return Err(fail());}
            let ids:BTreeMap<_,_>=source.instances.iter().enumerate().map(|(i,p)|(&p.id,i)).collect();
            if ids.len()!=source.instances.len() {return Err(fail());}
            let checkpoint=|cp:&authoring::Checkpoint| ids.get(&cp.piece).is_some_and(|&i|cp.sample<self.pieces[i].path.len());
            if source.checkpoints.iter().any(|c|!checkpoint(c))
                || source.connections.iter().any(|c|!ids.contains_key(&c.from)||!ids.contains_key(&c.to))
                || source.paths.iter().any(|p|p.pieces.len()>MAX_PIECES || p.pieces.iter().any(|i|!ids.contains_key(i)))
                || source.actions.iter().any(|a|!ids.get(&a.piece).is_some_and(|&i|a.sample<self.pieces[i].path.len()) || a.landing.as_ref().is_some_and(|c|!checkpoint(c)))
                || source.attachments.iter().any(|a|!ids.contains_key(&a.piece)) {return Err(fail());}
        }
        Ok(())
    }
}
