//! Bounded draft cells use the same geometry and presentation as execution.
use mapkit_core::*;
use std::{collections::{BTreeMap,BTreeSet},path::Path};

pub fn payload(root:&Path,path:&str,limit:u64)->Result<Vec<u8>> {
    let root=std::fs::canonicalize(root).map_err(|e|error("E_IO",e.to_string()))?;
    let file=std::fs::canonicalize(root.join(path)).map_err(|e|error("E_IO",e.to_string()))?;
    if !file.starts_with(root) {return Err(error("E_PATH","preview resource outside project"));}
    if std::fs::metadata(&file).map_err(|e|error("E_IO",e.to_string()))?.len()>limit {return Err(error("E_BUDGET","preview resource exceeds allowance"));}
    std::fs::read(file).map_err(|e|error("E_IO",e.to_string()))
}
pub fn cell(text:&str,project:&Path,cell:Cell)->Result<(MapDocument,GeneratedChunk)> {
    let mut d=super::engine_document(text)?;
    d.validate()?;
    let mut view=d.clone();view.cell_size_cm=3200;view.heightmaps.clear();
    let bounds=view.cell_bounds(cell)?;
    let original=d.cell_at(bounds.min).ok_or_else(||error("E_CELL","preview cell outside document"))?;
    let grid=if let Some(h)=d.heightmaps.iter().find(|h|h.cell==original) {
        let grid=mapkit_package::decode_heightmap(h,d.cell_size_cm,&payload(project,&h.path,16*1024*1024)?)?;
        let source=d.cell_bounds(original)?;
        let values=(0..=16).flat_map(|y|(0..=16).map(move |x|(x,y))).map(|(x,y)|terrain_height(&source,h.spacing_cm as i64,grid.side,Some(&grid),d.terrain_base_cm,[bounds.min[0]+x*200,bounds.min[1]+y*200])).collect();
        let mut descriptor=h.clone();descriptor.cell=cell;descriptor.spacing_cm=200;view.heightmaps.push(descriptor);
        Some(HeightGrid{side:17,heights_cm:values})
    } else {None};
    // This is a disposable view. The source document and original PNGs stay intact.
    std::mem::swap(&mut d,&mut view);
    let p=PreparedMap::new(d.clone())?;
    let cost=p.estimate(cell,500_000)?;
    if cost.generation_scratch_bytes+cost.triangles*256>256*1024*1024 {return Err(error("E_BUDGET","preview cell work allowance"));}
    let chunk=p.generate(cell,grid.as_ref(),500_000)?;
    Ok((d,chunk))
}
pub fn assets(d:&MapDocument,c:&GeneratedChunk,root:&Path)->Result<(BTreeMap<String,Vec<u8>>,u64)> {
    assets_from(d,c,|p,limit|payload(root,p,limit))
}
pub fn assets_from(d:&MapDocument,c:&GeneratedChunk,mut read:impl FnMut(&str,u64)->Result<Vec<u8>>)->Result<(BTreeMap<String,Vec<u8>>,u64)> {
    let mut needed:BTreeSet<_>=c.objects.iter().map(|o|o.asset_id.clone()).filter(|id|!id.starts_with("builtin:")).collect();
    let objects:BTreeSet<_>=c.triangles.iter().map(|t|t.object_id.as_str()).collect();
    for p in &d.placements {if objects.contains(p.id.as_str()) && !p.asset_id.starts_with("builtin:"){needed.insert(p.asset_id.clone());}}
    for id in needed.clone() {if let Some(texture)=d.assets.iter().find(|a|a.id==id).and_then(|a|a.material.as_ref()).and_then(|m|m.albedo_texture.as_ref()) {needed.insert(texture.clone());}}
    let mut subset=d.clone();subset.assets.retain(|a|needed.contains(&a.id));
    if let Some(e)=&mut subset.environment {e.lights.retain(|l|needed.contains(&l.asset_id));}
    let mut files=BTreeMap::new();let mut remaining=16*1024*1024;
    for a in &mut subset.assets {
        a.distant_path=None;
        let bytes=read(&a.path,remaining)?;remaining-=bytes.len() as u64;files.insert(a.path.clone(),bytes);
    }
    let cost=mapkit_package::preview_asset_cost(&subset,&files,c.objects.len())?;
    Ok((files,cost))
}
