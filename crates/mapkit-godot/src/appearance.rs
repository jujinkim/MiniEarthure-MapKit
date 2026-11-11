//! Value-only regional presentation arrays. No mutation of packed collision geometry.
use godot::prelude::*;
use mapkit_core::{appearance, Cell, MapDocument, Result};
pub fn decorate(d:&MapDocument, chunk:&VarDictionary, presentation:&mut VarDictionary)->Result<()> {
    let cell=chunk.get("cell").unwrap().to::<VarDictionary>();
    let cell=Cell{x:cell.get("x").unwrap().to::<i32>(),y:cell.get("y").unwrap().to::<i32>()};
    let display=mapkit_core::regional_display::generate(d,cell)?;
    let mut hidden=presentation.get("hidden_proxies").unwrap().to::<PackedStringArray>();
    for id in &display.objects {hidden.push(&GString::from(id.as_str()));}
    presentation.set("hidden_proxies",&hidden);
    let mut vertices=Vec::new();let mut normals=Vec::new();let mut keys=PackedStringArray::new();let mut seeds=Vec::new();let mut tops=Vec::new();
    for face in &display.faces {
        let p=face.vertices.map(|v|Vector3::new(v[0] as f32*0.01,v[1] as f32*0.01,-v[2] as f32*0.01));
        let mut n=(p[1]-p[0]).cross(p[2]-p[0]).normalized();
        if face.key.starts_with("roof/") && n.y<0. {n=-n;}
        for i in [0,2,1] {vertices.push(p[i]);normals.push(n);seeds.push(Vector2::new(face.seed,1000.+face.base_cm*0.01));tops.push(1000.+face.eaves_cm*0.01);}
        keys.push(&GString::from(face.key.as_str()));
    }
    presentation.set("regional_vertices",&PackedVector3Array::from(vertices.as_slice()));
    presentation.set("regional_normals",&PackedVector3Array::from(normals.as_slice()));
    presentation.set("regional_keys",&keys);
    presentation.set("regional_seeds",&PackedVector2Array::from(seeds.as_slice()));
    presentation.set("regional_tops",&PackedFloat32Array::from(tops.as_slice()));
    presentation.set("regional_omitted_modules",display.omitted_modules as i64);
    if display.omitted_modules>0 {mapkit_core::profile::note(mapkit_core::Warning{code:"regional_modules".into(),actual:display.omitted_modules as u64+mapkit_core::regional_display::MODULE_LIMIT as u64,limit:mapkit_core::regional_display::MODULE_LIMIT as u64,message:"Regional facade decoration reaches the cell allowance; source building bodies remain complete".into(),runtime_rejects:false});}
    let mut object_styles=VarDictionary::new();
    if let Some(value)=chunk.get("geometry") {
        let mut geometry=value.to::<Gd<RefCounted>>();let view=geometry.call("view",&[]).to::<VarDictionary>();
        for id in view.get("object_ids").unwrap().to::<PackedStringArray>().as_slice() {
            let text=id.to_string();let style=appearance::triangle_style(d,&text,true);
            if !style.is_empty() {object_styles.set(id,style);}
        }
    }
    presentation.set("appearance_styles",&object_styles);
    let mut plants=VarDictionary::new();
    if let Some(value)=chunk.get("objects") {for object in value.to::<Array<VarDictionary>>().iter_shared() {
        mapkit_core::cancellation::checkpoint()?;
        let id=object.get("id").unwrap().to::<GString>();
        let style=mapkit_core::regional_plants::zone_style(d,&id.to_string());
        if !style.is_empty() {plants.set(&id,style);}
    }}
    presentation.set("vegetation_styles",&plants);
    Ok(())
}
pub fn bytes(p:&VarDictionary)->u64 {
    p.get("regional_keys").map_or(0,|v|v.to::<PackedStringArray>().len() as u64*512)
        +p.get("vegetation_styles").map_or(0,|v|v.to::<VarDictionary>().len() as u64*512)
        +p.get("appearance_styles").map_or(0,|v|v.to::<VarDictionary>().len() as u64*512)
}
