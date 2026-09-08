//! Opt-in validated byte views. Host paths never call this or construct GPU resources.
use godot::prelude::*;
use mapkit_core::{Cell, Result};
use mapkit_package::Package;
use std::collections::BTreeSet;

pub fn cost(p: &Package, cell: Cell) -> u64 {
    // Per-instance allowance deliberately includes both importer scratch and retained
    // templates/copies. Driver/RSS calibration remains the caller's responsibility.
    p.document
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
        .map(|a| asset_cost(p, a))
        .sum()
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
    decorate_document(&p.document, &p.files, data)
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
        .and_then(|v| v.try_to().ok())
        .ok_or_else(|| mapkit_core::error("E_STATE", "generated objects required"))?;
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
        for t in value.to::<Array<VarDictionary>>().iter_shared() {
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
    chunk.set(
        "presentation",
        &vdict! {"assets"=>&assets,"hidden_proxies"=>&hidden,"proxy_materials"=>&materials},
    );
    data.set("chunk", &chunk);
    Ok(data)
}
