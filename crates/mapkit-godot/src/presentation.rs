//! Opt-in validated byte views. Host paths never call this or construct GPU resources.
use godot::prelude::*;
use mapkit_core::{Cell, Result};
use mapkit_package::Package;
use std::collections::{BTreeMap, BTreeSet};

pub fn cost(p: &Package, cell: Cell) -> u64 {
    // The common renderer imports each asset once per cell; duplicate(0) shares
    // resources. Count importer/shared data once and scene nodes per instance.
    let mut instances = BTreeMap::<&str, u64>::new();
    for asset in p
        .document
        .placements
        .iter()
        .filter(|v| {
            if p.document.cell_at([v.position[0], v.position[2]]) == Some(cell) {
                return true;
            }
            let Some(asset) = p.document.assets.iter().find(|a| a.id == v.asset_id) else {
                return false;
            };
            let area = p.document.cell_bounds(cell).unwrap();
            let overlaps = |b: mapkit_core::Bounds| {
                (0..2).all(|i| b.max[i] >= area.min[i] && b.min[i] <= area.max[i])
            };
            asset
                .convex_collision
                .iter()
                .any(|c| overlaps(c.placed(v).bounds()))
                || asset.collision.iter().any(|b| {
                    let mut center = b.center;
                    let mut size = b.size_cm;
                    for _ in 0..v.quarter_turns {
                        center = [-center[2], center[1], center[0]];
                        size.swap(0, 2);
                    }
                    let min = std::array::from_fn(|a| {
                        v.position[a * 2] + center[a * 2] - i64::from(size[a * 2] / 2)
                    });
                    overlaps(mapkit_core::Bounds {
                        min,
                        max: std::array::from_fn(|a| min[a] + i64::from(size[a * 2])),
                    })
                })
        })
        .filter_map(|v| p.document.assets.iter().find(|a| a.id == v.asset_id))
    {
        *instances.entry(&asset.id).or_default() += 1;
    }
    let styles = if p.document.recipe_version >= 6 {
        p.document
            .estimate(cell, 500_000)
            .map_or(0, |c| c.triangles * 256)
            + p.document
                .roads
                .iter()
                .filter(|r| r.markings.is_some())
                .map(|r| (r.points.len() - 1) as u64 * 4096)
                .sum::<u64>()
    } else {
        0
    };
    styles
        + instances
            .into_iter()
            .map(|(id, count)| {
                let asset = p.document.assets.iter().find(|a| a.id == id).unwrap();
                asset_cost(p, asset) + p.asset_instance_cost(id).unwrap() * count
            })
            .sum::<u64>()
}
fn asset_cost(p: &Package, a: &mapkit_core::Asset) -> u64 {
    let own = p.asset_presentation_cost(&a.id).unwrap();
    own + a
        .material
        .as_ref()
        .and_then(|m| m.albedo_texture.as_ref())
        .and_then(|id| p.document.assets.iter().find(|a| &a.id == id))
        .map_or(0, |texture| p.asset_presentation_cost(&texture.id).unwrap())
}
pub fn decorate(p: &Package, data: VarDictionary) -> Result<VarDictionary> {
    let mut data = decorate_document(&p.document, &p.files, data)?;
    if p.document.recipe_version >= 6 {
        let chunk = data.get("chunk").unwrap().to::<VarDictionary>();
        let presentation = chunk.get("presentation").unwrap().to::<VarDictionary>();
        let assets = presentation.get("assets").unwrap().to::<VarDictionary>();
        let objects = chunk.get("objects").unwrap().to::<Array<VarDictionary>>();
        let mut bytes = presentation
            .get("road_materials")
            .unwrap()
            .to::<PackedStringArray>()
            .len() as u64
            * 256
            + presentation
                .get("road_styles")
                .unwrap()
                .to::<VarDictionary>()
                .len() as u64
                * 4096;
        for asset in &p.document.assets {
            if !assets.contains_key(asset.id.as_str()) {
                continue;
            }
            bytes += asset_cost(p, asset);
            bytes += objects
                .iter_shared()
                .filter(|o| o.get("asset_id").unwrap().to::<GString>().to_string() == asset.id)
                .count() as u64
                * p.asset_instance_cost(&asset.id).unwrap();
        }
        if let Some(value) = data.get("generated_counts") {
            let mut counts = value.to::<VarDictionary>();
            counts.set("presentation_bytes", bytes as i64);
            data.set("generated_counts", &counts);
        }
    }
    Ok(data)
}
pub fn decorate_document(
    document: &mapkit_core::MapDocument,
    files: &std::collections::BTreeMap<String, Vec<u8>>,
    mut data: VarDictionary,
) -> Result<VarDictionary> {
    let mut chunk: VarDictionary = data
        .get("chunk")
        .and_then(|v| v.try_to().ok())
        .ok_or_else(|| mapkit_core::error("E_STATE", "generated chunk required"))?;
    let objects: Array<VarDictionary> = chunk
        .get("objects")
        .and_then(|v| {
            if let Ok(typed) = v.try_to::<Array<VarDictionary>>() {
                return Some(typed);
            }
            let array = v.try_to::<Array<Variant>>().ok()?;
            let mut typed = Array::<VarDictionary>::new();
            for item in array.iter_shared() {
                typed.push(&item.try_to::<VarDictionary>().ok()?);
            }
            Some(typed)
        })
        .ok_or_else(|| mapkit_core::error("E_STATE", "generated objects required"))?;
    chunk.set("objects", &objects); // Normalize JSON arrays for the typed packed/display adapter.
    let mut required = BTreeSet::new();
    for o in objects.iter_shared() {
        let id = o.get("asset_id").unwrap().to::<GString>().to_string();
        if !id.starts_with("builtin:") {
            required.insert(id);
        }
    }
    // Image proxies can overlap a cell whose visual anchor is in its neighbor.
    let mut ids = BTreeSet::new();
    if let Some(value) = chunk.get("geometry") {
        let mut geometry = value.to::<Gd<RefCounted>>();
        let view = geometry.call("view", &[]).to::<VarDictionary>();
        for id in view
            .get("object_ids")
            .unwrap()
            .to::<PackedStringArray>()
            .as_slice()
        {
            ids.insert(id.to_string());
        }
    } else if let Some(value) = chunk.get("triangles") {
        for value in value.to::<Array<Variant>>().iter_shared() {
            let t = value.to::<VarDictionary>();
            ids.insert(t.get("object_id").unwrap().to::<GString>().to_string());
        }
    }
    let mut hidden = PackedStringArray::new();
    let mut materials = VarDictionary::new();
    for placement in &document.placements {
        if !ids.contains(&placement.id) {
            continue;
        }
        if let Some(a) = document.assets.iter().find(|a| a.id == placement.asset_id) {
            if a.path.ends_with(".glb") {
                hidden.push(&GString::from(placement.id.as_str()));
            } else {
                required.insert(a.id.clone());
                materials.set(placement.id.as_str(), a.id.as_str());
            }
        }
    }
    for id in &ids {
        if let Some(asset) = objects
            .iter_shared()
            .find(|o| o.get("id").unwrap().to::<GString>().to_string() == *id)
            .map(|o| o.get("asset_id").unwrap().to::<GString>().to_string())
        {
            if asset.starts_with("builtin:") {
                materials.set(id.as_str(), asset.as_str());
            }
        } else if let Some(repetition) = document
            .repetitions
            .iter()
            .find(|r| id.starts_with(&format!("{}:repeat:", r.id)))
        {
            materials.set(id.as_str(), repetition.asset_id.as_str());
        } else if let Some(v) = document
            .placements
            .iter()
            .find(|v| v.id == *id && v.asset_id.starts_with("builtin:"))
        {
            materials.set(id.as_str(), v.asset_id.as_str());
        }
    }
    let initial = required.clone();
    for id in initial {
        if let Some(texture) = document
            .assets
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.material.as_ref())
            .and_then(|m| m.albedo_texture.as_ref())
        {
            required.insert(texture.clone());
        }
    }
    let mut assets = VarDictionary::new();
    for id in required {
        let a = document
            .assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| mapkit_core::error("E_ASSET", "missing display asset"))?;
        assets.set(id.as_str(),&vdict!{"path"=>a.path.as_str(),"bytes"=>&PackedByteArray::from(files[&a.path].as_slice()),
            "material_json"=>serde_json::to_string(&a.material).unwrap().as_str()});
    }
    let mut presentation =
        vdict! {"assets"=>&assets,"hidden_proxies"=>&hidden,"proxy_materials"=>&materials};
    crate::road_style::decorate(document, &chunk, &mut presentation);
    chunk.set("presentation", &presentation);
    data.set("chunk", &chunk);
    Ok(data)
}
