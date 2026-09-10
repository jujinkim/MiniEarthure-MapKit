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
    scene_vertices: PackedVector3Array,
    scene_normals: PackedVector3Array,
    ground_uv: PackedVector2Array,
    wall_uv: PackedVector2Array,
    prism_vertices: PackedInt64Array,
    prism_indices: PackedInt32Array,
    materials: PackedStringArray,
    usages: PackedStringArray,
    convex_vertices: PackedInt64Array,
    convex_offsets: PackedInt32Array,
    convex_ids: PackedStringArray,
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
            "scene_vertices" => &self.scene_vertices,
            "scene_normals" => &self.scene_normals,
            "ground_uv" => &self.ground_uv,
            "wall_uv" => &self.wall_uv,
            "building_prism_vertices_cm" => &self.prism_vertices,
            "building_prism_object_indices" => &self.prism_indices,
            "building_materials" => &self.materials,
            "building_usages" => &self.usages,
            "asset_convex_vertices_cm" => &self.convex_vertices,
            "asset_convex_offsets" => &self.convex_offsets,
            "asset_convex_ids" => &self.convex_ids,
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
    let mut scene_vertices = Vec::with_capacity(chunk.triangles.len() * 3);
    let mut scene_normals = Vec::with_capacity(chunk.triangles.len() * 3);
    let mut ground_uv = Vec::with_capacity(chunk.triangles.len() * 3);
    let mut wall_uv = Vec::with_capacity(chunk.triangles.len() * 3);
    let mut ids = PackedStringArray::new();
    let mut index = BTreeMap::<&str, i32>::new();
    for triangle in &chunk.triangles {
        let points = [0, 2, 1].map(|i| {
            let v = triangle.vertices[i];
            Vector3::new(v[0] as f32, v[1] as f32, -v[2] as f32)
                * (mapkit_core::WORLD_SCALE as f32 / 100.0)
        });
        // Godot front faces are clockwise, after the map-y to -z reflection.
        let cross = (points[2] - points[0]).cross(points[1] - points[0]);
        let normal = if cross.length_squared() > 0.0 {
            cross.normalized()
        } else {
            Vector3::ZERO
        };
        for point in points {
            scene_vertices.push(point);
            scene_normals.push(normal);
            let ground = Vector2::new(point.x, point.z);
            ground_uv.push(ground);
            let n = normal.abs();
            wall_uv.push(if n.y < n.x.max(n.z) {
                if n.x > n.z {
                    Vector2::new(point.z, point.y)
                } else {
                    Vector2::new(point.x, point.y)
                }
            } else {
                ground
            });
        }
        vertices.extend(triangle.vertices.iter().flatten().copied());
        surfaces.push(match triangle.surface {
            Surface::Asphalt => 0,
            Surface::Concrete => 1,
            Surface::Dirt => 2,
            Surface::Gravel => 3,
            Surface::Grass => 4,
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
    let mut prism_vertices = Vec::with_capacity(chunk.building_prisms.len() * 18);
    let mut prism_indices = Vec::with_capacity(chunk.building_prisms.len());
    let mut materials = vec![GString::new(); ids.len()];
    let mut usages = materials.clone();
    for prism in &chunk.building_prisms {
        let id = *index
            .get(prism.object_id.as_str())
            .ok_or_else(|| mapkit_core::error("E_GEOMETRY", "building prism has no faces"))?;
        prism_vertices.extend(prism.vertices().iter().flatten().copied());
        prism_indices.push(id);
        materials[id as usize] = prism.material.as_str().into();
        usages[id as usize] = prism.usage.as_str().into();
    }
    let mut convex_vertices = vec![];
    let mut convex_offsets = vec![0i32];
    let mut convex_ids = PackedStringArray::new();
    for c in &chunk.asset_convexes {
        convex_vertices.extend(c.shape.vertices.iter().flatten().copied());
        convex_offsets.push(convex_vertices.len() as i32);
        convex_ids.push(&GString::from(c.object_id.as_str()));
    }
    let geometry = Gd::from_init_fn(|base| MapKitPackedGeometry {
        base,
        convex_vertices: PackedInt64Array::from(convex_vertices.as_slice()),
        convex_offsets: PackedInt32Array::from(convex_offsets.as_slice()),
        convex_ids,
        vertices: PackedInt64Array::from(vertices.as_slice()),
        surfaces: PackedByteArray::from(surfaces.as_slice()),
        indices: PackedInt32Array::from(object_indices.as_slice()),
        spawnable: PackedByteArray::from(spawnable.as_slice()),
        ids,
        scene_vertices: PackedVector3Array::from(scene_vertices.as_slice()),
        scene_normals: PackedVector3Array::from(scene_normals.as_slice()),
        ground_uv: PackedVector2Array::from(ground_uv.as_slice()),
        wall_uv: PackedVector2Array::from(wall_uv.as_slice()),
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
    // Measured immutable output counts let consumers retire unused admission
    // allowance after the worker joins. These are not generated archive fields.
    let counts = vdict! {
        "triangles" => chunk.triangles.len() as i64,
        "objects" => chunk.objects.len() as i64,
        "building_prisms" => chunk.building_prisms.len() as i64,
        "asset_convexes" => chunk.asset_convexes.len() as i64,
    };
    Ok(vdict! { "chunk" => &data, "generated_sha256" => hash.as_str(), "generated_counts" => &counts })
}
