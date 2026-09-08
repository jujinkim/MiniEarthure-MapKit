//! Godot-specific packed view of the unchanged generated v6 contract.
use godot::prelude::*;
use mapkit_core::{GeneratedChunk, Surface};
use std::collections::BTreeMap;

/// Native immutable owner. Each view has separate Godot array wrappers backed by COW.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init)]
pub struct MapKitPackedGeometry {
    base: Base<RefCounted>,
    vertices: PackedInt64Array,
    surfaces: PackedByteArray,
    indices: PackedInt32Array,
    spawnable: PackedByteArray,
    ids: PackedStringArray,
    prism_vertices: PackedInt64Array,
    prism_indices: PackedInt32Array,
    materials: PackedStringArray,
    usages: PackedStringArray,
}
#[godot_api]
impl MapKitPackedGeometry {
    #[func]
    fn view(&self) -> VarDictionary {
        vdict! {
            "vertices_cm" => &self.vertices,
            "surface_indices" => &self.surfaces,
            "object_indices" => &self.indices,
            "spawnable" => &self.spawnable,
            "object_ids" => &self.ids,
            "building_prism_vertices_cm" => &self.prism_vertices,
            "building_prism_object_indices" => &self.prism_indices,
            "building_materials" => &self.materials,
            "building_usages" => &self.usages,
        }
    }
}

pub fn response(result: mapkit_core::Result<GeneratedChunk>) -> VarDictionary {
    respond(result.and_then(pack))
}
pub(super) fn respond(result: mapkit_core::Result<VarDictionary>) -> VarDictionary {
    match result {
        Ok(data) => vdict! { "ok" => true, "data" => &data },
        Err(error) => vdict! { "ok" => false, "error" => &vdict! {
            "code" => error.code.as_str(), "message" => error.message.as_str()
        } },
    }
}
pub(super) fn pack(chunk: GeneratedChunk) -> mapkit_core::Result<VarDictionary> {
    let hash = chunk.hash()?;
    let mut vertices = Vec::with_capacity(chunk.triangles.len() * 9);
    let mut surfaces = Vec::with_capacity(chunk.triangles.len());
    let mut object_indices = Vec::with_capacity(chunk.triangles.len());
    let mut spawnable = Vec::with_capacity(chunk.triangles.len());
    let mut ids = PackedStringArray::new();
    let mut index = BTreeMap::<&str, i32>::new();
    for triangle in &chunk.triangles {
        vertices.extend(triangle.vertices.iter().flatten().copied());
        surfaces.push(match triangle.surface {
            Surface::Asphalt => 0, Surface::Concrete => 1, Surface::Dirt => 2,
            Surface::Gravel => 3, Surface::Grass => 4,
        });
        let id = *index.entry(triangle.object_id.as_str()).or_insert_with(|| {
            let next = ids.len() as i32;
            ids.push(&GString::from(triangle.object_id.as_str()));
            next
        });
        object_indices.push(id);
        spawnable.push(u8::from(triangle.spawnable));
    }
    let mut objects = Array::<VarDictionary>::new();
    for object in &chunk.objects {
        objects.push(&vdict! {
            "id" => object.id.as_str(), "asset_id" => object.asset_id.as_str(),
            "position" => &varray![object.position[0], object.position[1], object.position[2]],
            "quarter_turns" => object.quarter_turns as i64,
        });
    }
    let mut prism_vertices=Vec::with_capacity(chunk.building_prisms.len()*18);
    let mut prism_indices=Vec::with_capacity(chunk.building_prisms.len());
    let mut materials=vec![GString::new();ids.len()];
    let mut usages=materials.clone();
    for prism in &chunk.building_prisms {
        let id=*index.get(prism.object_id.as_str()).ok_or_else(||mapkit_core::error("E_GEOMETRY","building prism has no faces"))?;
        prism_vertices.extend(prism.vertices().iter().flatten().copied());prism_indices.push(id);
        materials[id as usize]=prism.material.as_str().into();usages[id as usize]=prism.usage.as_str().into();
    }
    let geometry = Gd::from_init_fn(|base| MapKitPackedGeometry {
        base,
        vertices: PackedInt64Array::from(vertices.as_slice()),
        surfaces: PackedByteArray::from(surfaces.as_slice()),
        indices: PackedInt32Array::from(object_indices.as_slice()),
        spawnable: PackedByteArray::from(spawnable.as_slice()),
        ids,
        prism_vertices: PackedInt64Array::from(prism_vertices.as_slice()),
        prism_indices: PackedInt32Array::from(prism_indices.as_slice()),
        materials: PackedStringArray::from(materials.as_slice()),
        usages: PackedStringArray::from(usages.as_slice()),
    });
    let data = vdict! {
        "packed_version" => 1i64,
        "format_version" => chunk.format_version as i64,
        "cell" => &vdict! { "x" => chunk.cell.x, "y" => chunk.cell.y },
        "geometry" => &geometry,
        "objects" => &objects,
    };
    Ok(vdict! { "chunk" => &data, "generated_sha256" => hash.as_str() })
}
