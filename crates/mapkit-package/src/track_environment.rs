//! Export-time checks of a mixed map against actual terrain and independently
//! generated occupancy. A course draft never suppresses these geometry checks.
use crate::*;
use mapkit_core::{assembled_track as track, road_design::TerrainPolicy, SolidShape};

fn solid_bounds(shape:&SolidShape)->([i64;3],[i64;3]) {
    match shape {
        SolidShape::Box{min,max}=>(*min,*max),
        SolidShape::Convex(c)=>(std::array::from_fn(|j|c.vertices.iter().map(|p|p[j]).min().unwrap()),std::array::from_fn(|j|c.vertices.iter().map(|p|p[j]).max().unwrap())),
        SolidShape::TriangularPrism{footprint,bottom_cm,top_cm}=>
            ([footprint.iter().map(|p|p[0]).min().unwrap(),*bottom_cm,footprint.iter().map(|p|p[1]).min().unwrap()],
             [footprint.iter().map(|p|p[0]).max().unwrap(),*top_cm,footprint.iter().map(|p|p[1]).max().unwrap()]),
        SolidShape::SlopedPrism{footprint,bottom_cm,top_cm}=>
            ([footprint.iter().map(|p|p[0]).min().unwrap(),*bottom_cm,footprint.iter().map(|p|p[1]).min().unwrap()],
             [footprint.iter().map(|p|p[0]).max().unwrap(),*top_cm.iter().max().unwrap(),footprint.iter().map(|p|p[1]).max().unwrap()]),
    }
}

pub fn validate(document:&MapDocument,files:&BTreeMap<String,Vec<u8>>)->Result<()> {
    validate_road_terrain(document,files)?;
    let Some(a)=document.assembled_track.as_ref().filter(|a|a.terrain_integration()) else {return Ok(());};
    let base=track::composite::environment_base(document)?;
    let prepared=mapkit_core::PreparedMap::new(base)?;
    // One immutable generated cell at a time; no world-sized geometry cache.
    let mut current=None;
    let mut grid=None;
    let mut chunk=None;
    let mut work=0usize;
    let mut visited=BTreeSet::new();
    for (index,piece) in a.pieces.iter().enumerate() {
        for path in [&piece.path,&piece.alternate_path] {for pair in path.windows(2) {
            mapkit_core::cancellation::checkpoint()?;
            let length=libm::sqrt((0..3).map(|j|((pair[1].position_cm[j]-pair[0].position_cm[j]) as f64).powi(2)).sum());
            let steps=(length/50.0).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let t=step as f64/steps as f64;
                let center:[i64;3]=std::array::from_fn(|j|pair[0].position_cm[j]+libm::round((pair[1].position_cm[j]-pair[0].position_cm[j]) as f64*t) as i64);
                let f=pair[0].forward.map(|n|n as f64/1e6);
                let n=pair[0].normal.map(|n|n as f64/1e6);
                let side=[n[1]*f[2]-n[2]*f[1],n[2]*f[0]-n[0]*f[2],n[0]*f[1]-n[1]*f[0]];
                for lateral in [-0.9,0.0,0.9] {
                    let pos:[i64;3]=std::array::from_fn(|j|center[j]+libm::round(side[j]*lateral*pair[0].lateral_cm as f64) as i64);
                    let xy=[pos[0],pos[2]];
                    let cell=document.cell_at(xy).ok_or_else(||error("E_TRACK_BOUNDS",format!("piece {index} at {pos:?}: surface leaves map")))?;
                    visited.insert(cell);
                    if current!=Some(cell) {
                        grid=prepared.heightmap(cell).map(|h|decode_heightmap(h,prepared.cell_size_cm,files.get(&h.path).ok_or_else(||error("E_REFERENCE","terrain payload missing"))?)).transpose()?;
                        chunk=Some(prepared.generate_with_occupancy(cell,grid.as_ref(),2_000_000,Some(mapkit_core::MAX_OCCUPIED_SOLIDS))?);
                        current=Some(cell);
                    }
                    let bounds=prepared.cell_bounds(cell)?;
                    let spacing=prepared.heightmap(cell).map_or(prepared.cell_size_cm,|h|h.spacing_cm) as i64;
                    let terrain=mapkit_core::terrain_height(&bounds,spacing,prepared.cell_size_cm as usize/spacing as usize+1,grid.as_ref(),prepared.terrain_base_cm,xy);
                    if a.terrain_policy(index)!=TerrainPolicy::AutoFit && pair[0].mode!="flight" && terrain>pos[1]+1 {
                        return Err(error("E_TRACK_ENVIRONMENT",format!("piece {index} at {pos:?}: terrain penetrates driving surface ({terrain} cm)")));
                    }
                    for face in chunk.as_ref().unwrap().chunk.triangles.iter().filter(|f|f.spawnable && prepared.roads.iter().any(|r|r.id==f.object_id)) {
                        work+=1;
                        if let Some(y)=crate::road_audit::height(face.vertices,[pos[0] as f64,pos[2] as f64]) {
                            if (y-pos[1] as f64).abs()>1.0 && (y-pos[1] as f64).abs()<300.0 {
                                return Err(error("E_TRACK_ENVIRONMENT",format!("piece {index} at {pos:?}: insufficient clearance over {}",face.object_id)));
                            }
                        }
                    }
                    // Conservatively reject occupied driving/head space. The
                    // source objects remain in place for explicit user repair.
                    for solid in &chunk.as_ref().unwrap().solids {
                        work+=1;
                        if work>8_000_000 {return Err(error("E_BUDGET","track environment clearance work limit"));}
                        let (lo,hi)=solid_bounds(&solid.shape);
                        for height in [20.0,100.0,200.0] {
                            let point:[i64;3]=std::array::from_fn(|j|pos[j]+libm::round(n[j]*height) as i64);
                            if (0..3).all(|j|point[j]>lo[j] && point[j]<hi[j]) {
                                return Err(error("E_TRACK_ENVIRONMENT",format!("piece {index} at {pos:?}: driving space intersects {}",solid.object_id)));
                            }
                        }
                    }
                }
            }
        }}
    }
    let composed=mapkit_core::PreparedMap::new(document.clone())?;
    for cell in visited {
        cancellation::checkpoint()?;
        let grid=prepared.heightmap(cell).map(|h|decode_heightmap(h,prepared.cell_size_cm,files.get(&h.path).ok_or_else(||error("E_REFERENCE","terrain payload missing"))?)).transpose()?;
        let built=composed.generate_with_occupancy(cell,grid.as_ref(),2_000_000,Some(mapkit_core::MAX_OCCUPIED_SOLIDS))?;
        let supports:Vec<_>=built.solids.iter().filter(|s|s.object_id.starts_with("assembled-terrain-support-")).collect();
        if supports.is_empty(){continue;}
        let base=prepared.generate_with_occupancy(cell,grid.as_ref(),2_000_000,Some(mapkit_core::MAX_OCCUPIED_SOLIDS))?;
        for support in supports {
            let (lo,hi)=solid_bounds(&support.shape);
            let position=std::array::from_fn::<_,3,_>(|j|(lo[j]+hi[j])/2);
            for solid in &base.solids {
                let (a,b)=solid_bounds(&solid.shape);
                work+=1;
                if (0..3).all(|j|lo[j]<b[j] && a[j]<hi[j]) {
                    return Err(error("E_TRACK_ENVIRONMENT",format!("{} at {position:?}: support intersects {}",support.object_id,solid.object_id)));
                }
            }
            for face in base.chunk.triangles.iter().filter(|f|f.spawnable && prepared.roads.iter().any(|r|r.id==f.object_id)) {
                for p in [[lo[0],lo[2]],[lo[0],hi[2]],[hi[0],lo[2]],[hi[0],hi[2]],[position[0],position[2]]] {
                    work+=1;
                    if crate::road_audit::height(face.vertices,p.map(|v|v as f64)).is_some_and(|y|y>=lo[1] as f64 && y<hi[1] as f64+200.0 && y+200.0>lo[1] as f64) {
                        return Err(error("E_TRACK_ENVIRONMENT",format!("{} at {position:?}: support blocks road {}",support.object_id,face.object_id)));
                    }
                }
            }
            if work>8_000_000 {return Err(error("E_BUDGET","track environment clearance work limit"));}
        }
    }
    Ok(())
}

/// Preserving terrain never means silently cutting a buried authored road out
/// of it. Check original payloads even in maps without a track overlay.
fn validate_road_terrain(d:&MapDocument,files:&BTreeMap<String,Vec<u8>>)->Result<()> {
    let heightmaps:BTreeMap<_,_>=d.heightmaps.iter().map(|h|(h.cell,h)).collect();
    let mut current=None;
    let mut grid=None;
    let mut work=0usize;
    for road in d.roads.iter().filter(|r|r.design.as_ref().is_some_and(|s|s.terrain_policy!=TerrainPolicy::AutoFit)) {
        let path=mapkit_core::road_design::path(road)?;
        for pair in path.windows(2) {
            cancellation::checkpoint()?;
            let distance=libm::hypot((pair[1].position_cm[0]-pair[0].position_cm[0]) as f64,(pair[1].position_cm[2]-pair[0].position_cm[2]) as f64);
            let count=(distance/50.0).ceil().max(1.0) as usize;
            for step in 0..=count {for across in [0.05,0.5,0.95] {
                work+=1;
                if work>8_000_000{return Err(error("E_BUDGET","road terrain clearance work limit"));}
                let at=|sample:&track::Sample| {
                    let [a,b]=sample.ribbon_cm.unwrap();
                    std::array::from_fn::<_,3,_>(|j|a[j] as f64+(b[j]-a[j]) as f64*across)
                };
                let (a,b)=(at(&pair[0]),at(&pair[1]));
                let pos=std::array::from_fn::<_,3,_>(|j|libm::round(a[j]+(b[j]-a[j])*step as f64/count as f64) as i64);
                let xy=[pos[0],pos[2]];
                let Some(cell)=d.cell_at(xy) else {continue;};
                let heightmap=heightmaps.get(&cell).copied();
                if current!=Some(cell) {
                    grid=heightmap.map(|h|decode_heightmap(h,d.cell_size_cm,files.get(&h.path).ok_or_else(||error("E_REFERENCE","terrain payload missing"))?)).transpose()?;
                    current=Some(cell);
                }
                let spacing=heightmap.map_or(d.cell_size_cm,|h|h.spacing_cm) as i64;
                let terrain=mapkit_core::terrain_height(&d.cell_bounds(cell)?,spacing,d.cell_size_cm as usize/spacing as usize+1,grid.as_ref(),d.terrain_base_cm,xy);
                if terrain>pos[1]+1 {
                    return Err(error("E_ROAD_ENVIRONMENT",format!("{} at {pos:?}: preserved terrain penetrates driving surface ({terrain} cm)",road.id)));
                }
            }}
        }
    }
    Ok(())
}
