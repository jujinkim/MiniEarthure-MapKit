//! Opt-in validated byte views. Host paths never call this or construct GPU resources.
use godot::prelude::*;
use mapkit_core::{Cell, Result};
use mapkit_package::Package;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Cache {
    assets: BTreeMap<String, (Option<PackedByteArray>, String, u64, u64)>,
    placements: BTreeMap<String, usize>,
}
impl Cache {
    fn bytes(&mut self, p: &Package, id: &str) -> PackedByteArray {
        self.asset(p, id);
        self.assets
            .get_mut(id)
            .unwrap()
            .0
            .get_or_insert_with(|| {
                PackedByteArray::from(p.files[&p.document.asset(id).unwrap().path].as_slice())
            })
            .clone()
    }
    fn asset(&mut self, p: &Package, id: &str) -> &(Option<PackedByteArray>, String, u64, u64) {
        self.assets.entry(id.to_owned()).or_insert_with(|| {
            let a = p.document.asset(id).unwrap();
            (
                None,
                mapkit_core::sha256(&p.files[&a.path]),
                asset_cost(p, a),
                p.asset_instance_cost(id).unwrap(),
            )
        })
    }
}
pub fn cost(p: &Package, cell: Cell, cache: &mut Cache) -> Result<u64> {
    // The common renderer imports each asset once per cell; duplicate(0) shares
    // resources. Count importer/shared data once and scene nodes per instance.
    let mut instances = BTreeMap::<&str, u64>::new();
    for asset in p
        .document
        .authored_placement_candidates(cell)?
        .into_iter()
        .filter(|v| {
            if p.document.cell_at([v.position[0], v.position[2]]) == Some(cell) {
                return true;
            }
            let Some(asset) = p.document.asset(&v.asset_id) else {
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
        .filter_map(|v| p.document.asset(&v.asset_id))
    {
        *instances.entry(&asset.id).or_default() += 1;
    }
    let styles = {
        p.document
            .estimate(cell, 500_000)
            .map_or(0, |c| c.triangles * 256)
            + p.document
                .roads
                .iter()
                .filter(|r| r.markings.is_some())
                .map(|r| (r.points.len() - 1) as u64 * 16384)
                .sum::<u64>()
    };
    Ok(environment_cost(&p.document)
        + styles
        + instances
            .into_iter()
            .map(|(id, count)| {
                let asset = cache.asset(p, id);
                asset.2 + asset.3 * count
            })
            .sum::<u64>())
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
pub fn decorate(p: &Package, data: VarDictionary, cache: &mut Cache) -> Result<VarDictionary> {
    let mut data = decorate_inner(&p.document, &p.files, data, Some((p, cache)))?;

    let chunk = data.get("chunk").unwrap().to::<VarDictionary>();
    let presentation = chunk.get("presentation").unwrap().to::<VarDictionary>();
    let assets = presentation.get("assets").unwrap().to::<VarDictionary>();
    let objects = chunk.get("objects").unwrap().to::<Array<VarDictionary>>();
    let mut bytes = environment_cost(&p.document)
        + presentation
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
            * 16384;
    for asset in &p.document.assets {
        mapkit_core::cancellation::checkpoint()?;
        if !assets.contains_key(asset.id.as_str()) {
            continue;
        }
        // Additive presentation metadata, excluded from generated serialization/hash.
        // The consumer can reserve one immutable resource across multiple cells.
        let mut view = assets.get(asset.id.as_str()).unwrap().to::<VarDictionary>();
        view.set("content_hash", cache.asset(p, &asset.id).1.as_str());
        view.set("memory_bytes", cache.asset(p, &asset.id).2 as i64);
        bytes += cache.asset(p, &asset.id).2;
        bytes += objects
            .iter_shared()
            .filter(|o| o.get("asset_id").unwrap().to::<GString>().to_string() == asset.id)
            .count() as u64
            * cache.asset(p, &asset.id).3;
    }
    if let Some(value) = data.get("generated_counts") {
        let mut counts = value.to::<VarDictionary>();
        counts.set("presentation_bytes", bytes as i64);
        data.set("generated_counts", &counts);
    }

    Ok(data)
}
pub fn decorate_document(
    document: &mapkit_core::MapDocument,
    files: &BTreeMap<String, Vec<u8>>,
    data: VarDictionary,
) -> Result<VarDictionary> {
    decorate_inner(document, files, data, None)
}
fn decorate_inner(
    document: &mapkit_core::MapDocument,
    files: &std::collections::BTreeMap<String, Vec<u8>>,
    mut data: VarDictionary,
    mut cached: Option<(&Package, &mut Cache)>,
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
    let placements: Vec<_> = if let Some((_, cache)) = cached.as_mut() {
        if cache.placements.is_empty() {
            cache.placements = document
                .placements
                .iter()
                .enumerate()
                .map(|(i, p)| (p.id.clone(), i))
                .collect();
        }
        ids.iter()
            .filter_map(|id| cache.placements.get(id).map(|&i| &document.placements[i]))
            .collect()
    } else {
        document.placements.iter().collect()
    };
    for placement in placements {
        mapkit_core::cancellation::checkpoint()?;
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
        } else if let Some(v) = if let Some((_, cache)) = cached.as_ref() {
            cache
                .placements
                .get(id)
                .map(|&i| &document.placements[i])
                .filter(|v| v.asset_id.starts_with("builtin:"))
        } else {
            document
                .placements
                .iter()
                .find(|v| v.id == *id && v.asset_id.starts_with("builtin:"))
        } {
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
        let bytes = if let Some((p, cache)) = cached.as_mut() {
            cache.bytes(p, &id)
        } else {
            PackedByteArray::from(files[&a.path].as_slice())
        };
        assets.set(
            id.as_str(),
            &vdict! {"path"=>a.path.as_str(),"bytes"=>&bytes,
            "material_json"=>serde_json::to_string(&a.material).unwrap().as_str()},
        );
    }
    let mut presentation =
        vdict! {"assets"=>&assets,"hidden_proxies"=>&hidden,"proxy_materials"=>&materials};
    crate::road_style::decorate(document, &chunk, &mut presentation)?;
    if let Some(environment) = &document.environment {
        presentation.set(
            "environment_json",
            serde_json::to_string(environment).unwrap().as_str(),
        );
    }
    presentation.set("map_id", document.map_id.as_str());
    chunk.set("presentation", &presentation);
    data.set("chunk", &chunk);
    Ok(data)
}

fn environment_cost(d: &mapkit_core::MapDocument) -> u64 {
    // Native serialization plus Godot UTF-32 dictionary presentation, per admitted cell.
    d.environment.as_ref().map_or(0, |e| {
        serde_json::to_string(e).unwrap().len() as u64 * 8 + 4096
    }) + d.map_id.len() as u64 * 8
        + 128
}
