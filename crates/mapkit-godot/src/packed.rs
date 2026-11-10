//! Godot-specific packed view of the current generated v1 contract.
use godot::prelude::*;
use mapkit_core::{GeneratedChunk, Surface};
use std::collections::BTreeMap;

/// Native immutable owner. Each view has separate Godot array wrappers backed by COW.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init)]
pub struct MapKitPackedGeometry {
    base: Base<RefCounted>,
    water_bodies: Vec<mapkit_core::water::WaterCell>,
    vertices: PackedInt64Array,
    surfaces: PackedByteArray,
    contact_classes: PackedByteArray,
    snow_retention: PackedByteArray,
    indices: PackedInt32Array,
    spawnable: PackedByteArray,
    ids: PackedStringArray,
    scene_vertices: PackedVector3Array,
    scene_normals: PackedVector3Array,
    ground_uv: PackedVector2Array,
    wall_uv: PackedVector2Array,
    /// Per vertex: (seed, FACADE_BASE_OFFSET + base metres) on a generated
    /// building solid (one per connected solid of a record), (0, 0) elsewhere.
    /// The base lets ground-floor facades follow terrain. Display only.
    facade_seeds: PackedVector2Array,
    /// Per vertex: FACADE_BASE_OFFSET + the solid's top (eaves) height in scene metres
    /// on a generated building solid, 0 elsewhere, so facades fit whole storeys;
    /// flat roof faces carry ROOF_FRAME_OFFSET + the solid's roof frame angle.
    facade_tops: PackedFloat32Array,
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
    /// Read-only query over this already reserved immutable generated geometry.
    /// Uses the exact core surface selector; callers own cache lifetime/budget.
    #[func]
    fn surface_probe(&self, x_cm: i64, y_cm: i64, surface: GString) -> GString {
        let request = mapkit_core::SpawnRequest {
            position_cm: [x_cm, y_cm],
            surface_id: surface.to_string(),
        };
        let id = self.ids.as_slice().iter().position(|id| id == &surface);
        super::response((|| {
            let id = id.ok_or_else(|| {
                mapkit_core::error("E_SPAWN", "requested surface not in generated cell")
            })?;
            for (i, object) in self.indices.as_slice().iter().enumerate() {
                if *object != id as i32 || self.spawnable[i] == 0 {
                    continue;
                }
                let v = &self.vertices.as_slice()[i * 9..i * 9 + 9];
                let vertices = [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]];
                if (0..2).any(|axis| {
                    let p = request.position_cm[axis];
                    p < vertices.iter().map(|v| v[axis * 2]).min().unwrap()
                        || p > vertices.iter().map(|v| v[axis * 2]).max().unwrap()
                }) {
                    continue;
                }
                // Material does not participate in core surface selection/normal.
                let chunk = GeneratedChunk {
                    water_bodies: self.water_bodies.clone(),
                    grind_lines: vec![], gimmicks: vec![],
                    format_version: 1,
                    cell: mapkit_core::Cell { x: 0, y: 0 },
                    triangles: vec![mapkit_core::Triangle {
                        contact_class: 0, snow_retention_percent: 100,
                        vertices,
                        surface: Surface::Asphalt,
                        object_id: request.surface_id.clone(),
                        spawnable: true,
                    }],
                    objects: vec![],
                    building_prisms: vec![],
                    asset_convexes: vec![],
                };
                if let Ok(sample) = chunk.surface_probe(&request) {
                    return Ok(
                        serde_json::json!({"position_cm":sample.position_cm,"normal_q":sample.normal_q,
                        "surface_id":sample.surface_id}),
                    );
                }
            }
            Err(mapkit_core::error(
                "E_SPAWN",
                "requested surface not drivable at this position",
            ))
        })())
    }
    /// Display-only facade fixtures of this cell (facade_fixtures.rs) for a
    /// floor pitch in scene metres and the cell rect (scene min x, min z, max
    /// x, max z): the packed list and the count dropped by the cap.
    pub(crate) fn facade_fixtures(&self, pitch: f32, bounds: [f32; 4]) -> crate::facade_fixtures::Placed {
        let kinds: Vec<u8> = (0..self.materials.len()).map(|i| crate::facade_fixtures::facade_kind(&self.materials.get(i).unwrap_or_default().to_string(), &self.usages.get(i).unwrap_or_default().to_string())).collect();
        let concrete: Vec<bool> = (0..self.materials.len()).map(|i| self.materials.get(i).unwrap_or_default() == "concrete").collect();
        let keys: Vec<u8> = (0..self.materials.len()).map(|i| crate::roof_fixtures::palette_key(&self.materials.get(i).unwrap_or_default().to_string(), &self.usages.get(i).unwrap_or_default().to_string())).collect();
        let shares: Vec<f32> = (0..self.materials.len()).map(|i| mapkit_package::distant::facade_own_share(&format!("{}:{}", self.materials.get(i).unwrap_or_default(), self.usages.get(i).unwrap_or_default()))).collect();
        crate::facade_fixtures::place(&crate::facade_fixtures::Input {
            vertices: self.scene_vertices.as_slice(), normals: self.scene_normals.as_slice(), objects: self.indices.as_slice(),
            seeds: self.facade_seeds.as_slice(), tops: self.facade_tops.as_slice(), kinds: &kinds, concrete: &concrete, keys: &keys, shares: &shares, pitch, bounds,
        }, crate::facade_fixtures::MAX_FIXTURES)
    }
    /// Display-only runs and posts of this cell's barrier prisms
    /// (barrier_details.rs), from the generated convexes under barrier IDs.
    pub(crate) fn barrier_details(&self, d: &mapkit_core::MapDocument, cell: mapkit_core::Cell) -> mapkit_core::Result<crate::barrier_details::Listed> {
        let (offsets, vertices) = (self.convex_offsets.as_slice(), self.convex_vertices.as_slice());
        let ids: Vec<String> = (0..self.convex_ids.len()).map(|i| self.convex_ids.get(i).unwrap_or_default().to_string()).collect();
        crate::barrier_details::list(d, cell, ids.iter().enumerate()
            .map(|(i, id)| (id.as_str(), &vertices[offsets[i] as usize..offsets[i + 1] as usize])))
    }
    #[func]
    fn view(&self) -> VarDictionary {
        vdict! {
            "vertices_cm" => &self.vertices,
            "surface_indices" => &self.surfaces,
            "contact_classes" => &self.contact_classes,
            "snow_retention_percent" => &self.snow_retention,
            "object_indices" => &self.indices,
            "spawnable" => &self.spawnable,
            "object_ids" => &self.ids,
            "scene_vertices" => &self.scene_vertices,
            "scene_normals" => &self.scene_normals,
            "ground_uv" => &self.ground_uv,
            "wall_uv" => &self.wall_uv,
            "facade_seeds" => &self.facade_seeds,
            "facade_tops" => &self.facade_tops,
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

pub(super) fn respond(result: mapkit_core::Result<VarDictionary>) -> VarDictionary {
    match result {
        Ok(data) => vdict! { "ok" => true, "data" => &data },
        Err(error) => vdict! { "ok" => false, "error" => &vdict! {
            "code" => error.code.as_str(), "message" => error.message.as_str()
        } },
    }
}
/// `respond` plus top-level `warnings` (an Array of Dictionaries), omitted when empty.
pub(super) fn respond_warned((result, warnings): (mapkit_core::Result<VarDictionary>, Vec<mapkit_core::Warning>)) -> VarDictionary {
    let mut value = respond(result);
    if !warnings.is_empty() {
        value.set("warnings", &warnings_array(&warnings));
    }
    value
}
pub(super) fn warnings_array(warnings: &[mapkit_core::Warning]) -> VarArray {
    let mut list = VarArray::new();
    for w in warnings {
        list.push(&vdict! {
            "code" => &*w.code, "actual" => w.actual as i64, "limit" => w.limit as i64,
            "message" => &*w.message, "runtime_rejects" => w.runtime_rejects
        }.to_variant());
    }
    list
}
/// Added to a solid's scene-metre base height so that y > 0.5 marks a seed.
const FACADE_BASE_OFFSET: f32 = 1000.0;
/// Added to a flat roof face's frame angle in its facade top (CUSTOM0.x):
/// values above 3500 mark a roof frame, never a top (tops stay below 3000).
pub(crate) const ROOF_FRAME_OFFSET: f32 = 4000.0;

/// A solid's roof frame: the dominant direction of its facade walls modulo
/// 90° (the length-weighted mean of 4θ of its flat roof perimeter edges, see
/// `facade_seeds`), in [0, π/2) and quantized to 1/4096 rad, so every cell
/// showing one of a rectangular building's walls finds the same frame. The
/// frame's u axis is (cos a, 0, sin a), its v axis (-sin a, 0, cos a); 0
/// without perimeter edges.
pub(crate) fn roof_frame(sum_cos: f64, sum_sin: f64) -> f32 {
    if sum_cos.hypot(sum_sin) < 1e-9 { return 0.0; }
    let a = (0.25 * sum_sin.atan2(sum_cos)).rem_euclid(std::f64::consts::FRAC_PI_2);
    // 6434 / 4096 is π/2 to within 5e-6 rad: the same frame as 0.
    ((a * 4096.0).round() as i64 % 6434) as f32 / 4096.0
}
/// One stable seed per connected building solid: triangles of a building
/// object that share a vertex form one solid (walls and roof of one part).
/// Returns per-vertex seeds (seed, offset base) and offset solid tops (eaves).
/// A triangle rising above its solid's top (a gable end, the taller part of a
/// merged solid, a roof slope) carries the offset top negated: storeys still
/// fit below |top|, but the shader draws no parapet coping there. Flat roof
/// faces (horizontal, above the solid's base) carry the solid's roof frame
/// instead: ROOF_FRAME_OFFSET + its angle (`roof_frame`), which the roof
/// finishes of atmosphere_surface.gdshader and the rooftop fixtures of
/// roof_fixtures.rs align to.
pub(crate) fn facade_seeds(chunk: &GeneratedChunk, object_indices: &[i32], prism_indices: &[i32]) -> (Vec<Vector2>, Vec<f32>) {
    let buildings: std::collections::BTreeSet<i32> = prism_indices.iter().copied().collect();
    // Connected solids and their 12-bit solid seeds, shared with the distant
    // colours (mapkit-package distant.rs `facade_solids`).
    let building: Vec<bool> = object_indices.iter().map(|o| buildings.contains(o)).collect();
    let solids = mapkit_package::distant::facade_solids(&chunk.triangles, &building);
    let root_of = |t: usize| solids[t].unwrap().0;
    // The seed's high 12 bits are the record's palette seed and its low 12
    // bits the solid seed (record ID and solid top), both shared with the
    // distant mesh; base and top per solid.
    let mut extent = BTreeMap::<usize, (i64, i64)>::new();
    for (t, triangle) in chunk.triangles.iter().enumerate() {
        if !building[t] { continue; }
        let root = root_of(t);
        let base = triangle.vertices.iter().map(|v| v[1]).min().unwrap();
        let top = triangle.vertices.iter().map(|v| v[1]).max().unwrap();
        extent.entry(root).and_modify(|(b, h)| { *b = (*b).min(base); *h = (*h).max(top); }).or_insert((base, top));
    }
    // Storeys end at the eaves: the lowest vertex of the solid's roof faces
    // (non-vertical faces wholly above its base; bottoms lie at the base), so a
    // gable ridge does not stretch them. Solids without roof faces keep their top.
    let mut eaves = BTreeMap::<usize, i64>::new();
    for (t, triangle) in chunk.triangles.iter().enumerate() {
        if !building[t] { continue; }
        let root = root_of(t);
        let base = extent[&root].0;
        let [a, b, c] = triangle.vertices.map(|v| v.map(|x| x as f64));
        let (e, f) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
        let low = triangle.vertices.iter().map(|v| v[1]).min().unwrap();
        if 4.0 * n[1] * n[1] > n[0] * n[0] + n[1] * n[1] + n[2] * n[2] && low > base {
            eaves.entry(root).and_modify(|h| *h = (*h).min(low)).or_insert(low);
        }
    }
    // Roof frames from the perimeter edges of each solid's flat roof faces.
    // Generated buildings extrude every roof triangle as its own prism, so a
    // solid also carries coincident wall pairs on its triangulation and fan
    // diagonals and closing walls on the cell cut: walls do not tell the
    // facade apart. A roof edge shared by two flat roof triangles of the solid
    // is such a diagonal; an edge on the chunk's extent (everything a cell
    // generates lies inside its rect, so a cut lies there) is a cut. The rest
    // run along the facade walls (map y is scene -z).
    let mut flat_roofs = vec![false; chunk.triangles.len()];
    let mut edges = BTreeMap::<(usize, [i64; 3], [i64; 3]), u32>::new();
    let (mut lo, mut hi) = ([i64::MAX; 2], [i64::MIN; 2]);
    for (t, triangle) in chunk.triangles.iter().enumerate() {
        for v in triangle.vertices { for (k, axis) in [0, 2].into_iter().enumerate() { lo[k] = lo[k].min(v[axis]); hi[k] = hi[k].max(v[axis]); } }
        if !building[t] { continue; }
        let root = root_of(t);
        let [a, b, c] = triangle.vertices.map(|v| v.map(|x| x as f64));
        let (e, f) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
        let low = triangle.vertices.iter().map(|v| v[1]).min().unwrap();
        if n[0] == 0.0 && n[2] == 0.0 && n[1] != 0.0 && low > extent[&root].0 {
            flat_roofs[t] = true;
            for i in 0..3 {
                let (p, q) = (triangle.vertices[i], triangle.vertices[(i + 1) % 3]);
                *edges.entry((root, p.min(q), p.max(q))).or_insert(0) += 1;
            }
        }
    }
    let mut frames = BTreeMap::<usize, (f64, f64)>::new();
    for (&(root, p, q), &count) in &edges {
        let cut = [0, 2].into_iter().enumerate().any(|(k, axis)| (p[axis] == lo[k] && q[axis] == lo[k]) || (p[axis] == hi[k] && q[axis] == hi[k]));
        if count != 1 || cut { continue; }
        let (dx, dy) = ((q[0] - p[0]) as f64, (q[2] - p[2]) as f64);
        let length = dx.hypot(dy);
        if length == 0.0 { continue; }
        let theta = 4.0 * (-dy).atan2(dx);
        let sum = frames.entry(root).or_insert((0.0, 0.0));
        sum.0 += length * theta.cos();
        sum.1 += length * theta.sin();
    }
    let scale = mapkit_core::WORLD_SCALE as f32 / 100.0;
    let mut values = BTreeMap::<usize, f32>::new();
    let mut seeds = Vec::with_capacity(chunk.triangles.len() * 3);
    let mut tops = Vec::with_capacity(chunk.triangles.len() * 3);
    for (t, triangle) in chunk.triangles.iter().enumerate() {
        let value = if let Some((root, solid)) = solids[t] {
            let seed = *values.entry(root).or_insert_with(|| {
                let record = mapkit_package::distant::facade_record(&triangle.object_id);
                ((record << 12) | solid) as f32 / 16_777_216.0
            });
            let (base, top) = extent[&root];
            let top = eaves.get(&root).copied().unwrap_or(top);
            let rises = triangle.vertices.iter().map(|v| v[1]).max().unwrap() > top;
            let offset = FACADE_BASE_OFFSET + top as f32 * scale;
            let top = if flat_roofs[t] {
                let (c, s) = frames.get(&root).copied().unwrap_or((0.0, 0.0));
                ROOF_FRAME_OFFSET + roof_frame(c, s)
            } else if rises { -offset } else { offset };
            (Vector2::new(seed, FACADE_BASE_OFFSET + base as f32 * scale), top)
        } else {
            (Vector2::ZERO, 0.0)
        };
        seeds.extend([value.0; 3]);
        tops.extend([value.1; 3]);
    }
    (seeds, tops)
}

/// Scene-space corners (Godot order) and face normal of a generated triangle.
pub(crate) fn scene_triangle(triangle: &mapkit_core::Triangle) -> ([Vector3; 3], Vector3) {
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
    (points, normal)
}

pub(super) fn pack(chunk: GeneratedChunk) -> mapkit_core::Result<VarDictionary> {
    let hash = chunk.hash()?;
    let mut vertices = Vec::with_capacity(chunk.triangles.len() * 9);
    let mut contact_classes = Vec::with_capacity(chunk.triangles.len());
    let mut snow_retention = Vec::with_capacity(chunk.triangles.len());
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
        mapkit_core::cancellation::checkpoint()?;
        let (points, normal) = scene_triangle(triangle);
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
        contact_classes.push(triangle.contact_class);
        snow_retention.push(triangle.snow_retention_percent);
    }
    let mut objects = Array::<VarDictionary>::new();
    for object in &chunk.objects {
        mapkit_core::cancellation::checkpoint()?;
        objects.push(&vdict! {
            "id" => object.id.as_str(), "asset_id" => object.asset_id.as_str(),
            "position" => &varray![object.position[0], object.position[1], object.position[2]],
            "quarter_turns" => object.quarter_turns as i64,
            "yaw_offset_mdeg" => object.yaw_offset_mdeg as i64,
        });
    }
    let mut prism_vertices = Vec::with_capacity(chunk.building_prisms.len() * 18);
    let mut prism_indices = Vec::with_capacity(chunk.building_prisms.len());
    let mut materials = vec![GString::new(); ids.len()];
    let mut usages = materials.clone();
    for prism in &chunk.building_prisms {
        mapkit_core::cancellation::checkpoint()?;
        let id = *index
            .get(prism.object_id.as_str())
            .ok_or_else(|| mapkit_core::error("E_GEOMETRY", "building prism has no faces"))?;
        prism_vertices.extend(prism.vertices().iter().flatten().copied());
        prism_indices.push(id);
        materials[id as usize] = prism.material.as_str().into();
        usages[id as usize] = prism.usage.as_str().into();
    }
    let (facade_seeds, facade_tops) = facade_seeds(&chunk, &object_indices, &prism_indices);
    let mut convex_vertices = vec![];
    let mut convex_offsets = vec![0i32];
    let mut convex_ids = PackedStringArray::new();
    for c in &chunk.asset_convexes {
        mapkit_core::cancellation::checkpoint()?;
        convex_vertices.extend(c.shape.vertices.iter().flatten().copied());
        convex_offsets.push(convex_vertices.len() as i32);
        convex_ids.push(&GString::from(c.object_id.as_str()));
    }
    let geometry = Gd::from_init_fn(|base| MapKitPackedGeometry {
        water_bodies: chunk.water_bodies.clone(),
        base,
        convex_vertices: PackedInt64Array::from(convex_vertices.as_slice()),
        convex_offsets: PackedInt32Array::from(convex_offsets.as_slice()),
        convex_ids,
        vertices: PackedInt64Array::from(vertices.as_slice()),
        surfaces: PackedByteArray::from(surfaces.as_slice()),
        contact_classes: PackedByteArray::from(contact_classes.as_slice()),
        snow_retention: PackedByteArray::from(snow_retention.as_slice()),
        indices: PackedInt32Array::from(object_indices.as_slice()),
        spawnable: PackedByteArray::from(spawnable.as_slice()),
        ids,
        scene_vertices: PackedVector3Array::from(scene_vertices.as_slice()),
        scene_normals: PackedVector3Array::from(scene_normals.as_slice()),
        ground_uv: PackedVector2Array::from(ground_uv.as_slice()),
        wall_uv: PackedVector2Array::from(wall_uv.as_slice()),
        facade_seeds: PackedVector2Array::from(facade_seeds.as_slice()),
        facade_tops: PackedFloat32Array::from(facade_tops.as_slice()),
        prism_vertices: PackedInt64Array::from(prism_vertices.as_slice()),
        prism_indices: PackedInt32Array::from(prism_indices.as_slice()),
        materials: PackedStringArray::from(materials.as_slice()),
        usages: PackedStringArray::from(usages.as_slice()),
    });
    let data = vdict! {
        "water_bodies_json" => serde_json::to_string(&chunk.water_bodies).unwrap().as_str(),
        "grind_lines_json" => serde_json::to_string(&chunk.grind_lines.iter().map(|g|g.resolved_json()).collect::<Vec<_>>()).unwrap().as_str(),
        "gimmicks_json" => serde_json::to_string(&chunk.gimmicks.iter().map(|g|g.resolved_json()).collect::<Vec<_>>()).unwrap().as_str(),
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
    Ok(
        vdict! { "chunk" => &data, "generated_sha256" => hash.as_str(), "generated_counts" => &counts },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle(object: &str, vertices: [[i64; 3]; 3]) -> mapkit_core::Triangle {
        mapkit_core::Triangle { contact_class: 0, snow_retention_percent: 100, vertices, surface: Surface::Concrete, object_id: object.into(), spawnable: false }
    }

    #[test]
    fn facade_seeds_follow_connected_building_solids_their_base_and_top() {
        // Object 0: two solids (one raised); object 1: ground, not a building.
        let triangles = vec![
            triangle("b", [[0, 300, 0], [100, 300, 0], [0, 600, 0]]),
            triangle("b", [[100, 300, 0], [100, 600, 0], [0, 600, 0]]),
            triangle("b", [[500, 900, 0], [600, 900, 0], [500, 1200, 0]]),
            triangle("g", [[0, 0, 0], [100, 0, 0], [0, 0, 100]]),
        ];
        let chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], format_version: 1,
            cell: mapkit_core::Cell { x: 0, y: 0 }, triangles, objects: vec![], building_prisms: vec![], asset_convexes: vec![] };
        let (seeds, tops) = facade_seeds(&chunk, &[0, 0, 0, 1], &[0]);
        assert_eq!(seeds.len(), 12);
        assert_eq!(tops.len(), 12);
        assert!(seeds[..6].iter().all(|s| *s == seeds[0]), "one seed per connected solid");
        assert_ne!(seeds[0].x, seeds[6].x, "separate solids of a record vary");
        let scale = mapkit_core::WORLD_SCALE as f32 / 100.0;
        assert_eq!(seeds[0].y, FACADE_BASE_OFFSET + 300.0 * scale);
        assert_eq!(seeds[6].y, FACADE_BASE_OFFSET + 900.0 * scale);
        assert!(seeds[9..].iter().all(|s| *s == Vector2::ZERO), "non-building triangles carry no seed");
        assert!(tops[..6].iter().all(|t| *t == FACADE_BASE_OFFSET + 600.0 * scale), "one top per connected solid");
        assert_eq!(tops[6], FACADE_BASE_OFFSET + 1200.0 * scale, "a raised solid keeps its own top");
        assert!(tops[9..].iter().all(|t| *t == 0.0), "non-building triangles carry no top");
        assert_eq!(facade_seeds(&chunk, &[0, 0, 0, 1], &[0]), (seeds, tops), "stable");
    }

    #[test]
    fn facade_tops_end_at_the_eaves_of_a_gable() {
        // A wall, two roof slopes up to a 600 cm ridge and a gable end wall.
        let triangles = vec![
            triangle("b", [[0, 0, 0], [1000, 0, 0], [0, 400, 0]]),
            triangle("b", [[0, 400, 0], [1000, 400, 0], [500, 600, 300]]),
            triangle("b", [[0, 400, 0], [0, 400, 600], [500, 600, 300]]),
            triangle("b", [[0, 0, 0], [0, 400, 0], [0, 600, 300]]),
        ];
        let chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], format_version: 1,
            cell: mapkit_core::Cell { x: 0, y: 0 }, triangles, objects: vec![], building_prisms: vec![], asset_convexes: vec![] };
        let (seeds, tops) = facade_seeds(&chunk, &[0, 0, 0, 0], &[0]);
        let scale = mapkit_core::WORLD_SCALE as f32 / 100.0;
        assert!(seeds.iter().all(|s| *s == seeds[0]) && seeds[0].y == FACADE_BASE_OFFSET);
        assert!(tops.iter().all(|t| t.abs() == FACADE_BASE_OFFSET + 400.0 * scale), "storeys end at the eaves, not the ridge");
        assert!(tops[..3].iter().all(|t| *t > 0.0), "the eaves wall ends at the top: coping");
        assert!(tops[3..].iter().all(|t| *t < 0.0), "roof slopes and the gable end rise above it: no coping");
    }

    #[test]
    fn the_taller_part_of_a_merged_solid_draws_no_coping_at_the_lower_top() {
        // A 400 cm part and an 800 cm part of one record touching at a vertex.
        let triangles = vec![
            triangle("b", [[0, 0, 0], [100, 0, 0], [0, 400, 0]]),
            triangle("b", [[0, 400, 0], [100, 400, 0], [0, 400, 100]]),
            triangle("b", [[100, 0, 0], [200, 0, 0], [100, 800, 0]]),
            triangle("b", [[100, 800, 0], [200, 800, 0], [100, 800, 100]]),
            // Ground reaching past the building: the chunk's extent (cell rect).
            triangle("g", [[-1000, 0, -1000], [1000, 0, -1000], [-1000, 0, 1000]]),
        ];
        let chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], format_version: 1,
            cell: mapkit_core::Cell { x: 0, y: 0 }, triangles, objects: vec![], building_prisms: vec![], asset_convexes: vec![] };
        let (_, tops) = facade_seeds(&chunk, &[0, 0, 0, 0, 1], &[0]);
        let top = FACADE_BASE_OFFSET + 400.0 * (mapkit_core::WORLD_SCALE as f32 / 100.0);
        assert!(tops[..3].iter().all(|t| *t == top), "the lower part ends at the top");
        assert!(tops[6..9].iter().all(|t| *t == -top), "the taller part rises above it");
        // Flat roof faces carry the roof frame of their edges (along x: angle 0).
        assert!(tops[3..6].iter().chain(&tops[9..12]).all(|t| *t == ROOF_FRAME_OFFSET), "flat roofs carry the frame: {tops:?}");
    }

    #[test]
    fn flat_roofs_carry_the_frame_of_their_walls() {
        // A box rotated by atan(3/4) (about 36.87°) in the map plane: walls
        // along (400, 300) and (-300, 400), a flat roof at 600 cm.
        let corners = [[0i64, 0], [400, 300], [100, 700], [-300, 400]];
        let mut triangles = Vec::new();
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            triangles.push(triangle("b", [[a[0], 0, a[1]], [b[0], 0, b[1]], [b[0], 600, b[1]]]));
            triangles.push(triangle("b", [[a[0], 0, a[1]], [b[0], 600, b[1]], [a[0], 600, a[1]]]));
        }
        let top = |c: [i64; 2]| [c[0], 600, c[1]];
        triangles.push(triangle("b", [top(corners[0]), top(corners[1]), top(corners[2])]));
        triangles.push(triangle("b", [top(corners[0]), top(corners[2]), top(corners[3])]));
        let objects = vec![0; triangles.len()];
        let chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], format_version: 1,
            cell: mapkit_core::Cell { x: 0, y: 0 }, triangles, objects: vec![], building_prisms: vec![], asset_convexes: vec![] };
        let (_, tops) = facade_seeds(&chunk, &objects, &[0]);
        let frame = tops[24] - ROOF_FRAME_OFFSET;
        assert!(tops[24..].iter().all(|t| *t == tops[24]), "one frame per roof");
        assert!(tops[..24].iter().all(|t| (*t - (FACADE_BASE_OFFSET + 600.0 * mapkit_core::WORLD_SCALE as f32 / 100.0)).abs() < 1e-3), "walls keep their top");
        // Scene walls run along (4, -3) and (3, 4) (map y is -z): the frame
        // is one of them modulo 90°.
        let expected = (4.0f32).atan2(3.0);
        assert!((frame - expected).abs() < 1e-3, "frame {frame} vs {expected}");
        let (u, v) = (Vector3::new(frame.cos(), 0.0, frame.sin()), Vector3::new(-frame.sin(), 0.0, frame.cos()));
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let d = Vector3::new((b[0] - a[0]) as f32, 0.0, -(b[1] - a[1]) as f32).normalized();
            assert!(d.dot(u).abs() > 0.9999 || d.dot(v).abs() > 0.9999, "wall {k} runs along a frame axis");
        }
        assert_eq!(roof_frame(0.0, 0.0), 0.0);
        assert!(roof_frame(1.0, -1e-7) == 0.0 && roof_frame(1.0, 0.0) == 0.0, "the frame wraps at 90°");
    }

    /// The roof frames (angles) of the flat roofs packed for one cell.
    fn generated_frames(d: &mapkit_core::MapDocument, cell: mapkit_core::Cell) -> Vec<f32> {
        let chunk = mapkit_core::generate(mapkit_core::GenerationInput { document: d, cell, heightgrid: None, max_triangles: 500_000 }).unwrap();
        let mut ids: Vec<&str> = Vec::new();
        let objects: Vec<i32> = chunk.triangles.iter().map(|t| match ids.iter().position(|id| *id == t.object_id) {
            Some(i) => i as i32,
            None => { ids.push(&t.object_id); ids.len() as i32 - 1 }
        }).collect();
        let prisms: Vec<i32> = chunk.building_prisms.iter().map(|p| ids.iter().position(|id| *id == p.object_id).unwrap() as i32).collect();
        let (_, tops) = facade_seeds(&chunk, &objects, &prisms);
        let mut frames: Vec<f32> = tops.iter().filter(|t| **t > ROOF_FRAME_OFFSET - 500.0).map(|t| t - ROOF_FRAME_OFFSET).collect();
        frames.dedup();
        frames
    }

    #[test]
    fn generated_rectangles_get_the_frame_of_their_walls_in_every_cell() {
        // Generated buildings carry internal walls on every roof triangulation
        // diagonal and closing walls on the cell cut: the frame follows the
        // facade walls of rotated non-square rectangles anyway, inside a cell
        // and across the east edge of cell (0, 0) at x 51200 cm.
        let mut d: mapkit_core::MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        let template = d.buildings[0].clone();
        let rotated = |centre: [f64; 2], w: f64, h: f64, degrees: f64| -> Vec<[i64; 2]> {
            let (s, c) = degrees.to_radians().sin_cos();
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter()
                .map(|(i, j)| { let (u, v) = (i * w / 2.0, j * h / 2.0); [(centre[0] + u * c - v * s).round() as i64, (centre[1] + u * s + v * c).round() as i64] }).collect()
        };
        let (inside, across) = (vec![mapkit_core::Cell { x: 0, y: 0 }], vec![mapkit_core::Cell { x: 0, y: 0 }, mapkit_core::Cell { x: 1, y: 0 }]);
        for (centre, w, h, degrees, cells) in [([20000.0, 20000.0], 2000.0, 1000.0, 30.0, &inside), ([30000.0, 40000.0], 1200.0, 1000.0, 20.0, &inside),
                                                ([10000.0, 10000.0], 3000.0, 1000.0, 0.0, &inside), ([51200.0, 30000.0], 2000.0, 1000.0, 30.0, &across)] {
            let mut b = template.clone();
            b.id = "slab".into();
            b.footprint = rotated(centre, w, h, degrees);
            d.buildings = vec![b];
            // Map direction (cos α, sin α) is scene (cos α, 0, -sin α).
            let expected = (-(degrees as f32).to_radians()).rem_euclid(std::f32::consts::FRAC_PI_2);
            for &cell in cells {
                let frames = generated_frames(&d, cell);
                assert!(!frames.is_empty(), "a flat roof in {cell:?}");
                for f in frames {
                    let off = (f - expected).abs().min(std::f32::consts::FRAC_PI_2 - (f - expected).abs());
                    assert!(off < 2e-3, "{degrees}° rectangle in {cell:?}: frame {f} vs {expected}");
                }
            }
        }
    }

    #[test]
    fn near_and_distant_solids_share_their_tops_and_looks_across_a_cell_cut() {
        // One concrete commercial record of three solids, two of them crossing
        // the edge of cell (0, 0) at x 12800 cm: heights picked so the record
        // shows both an own look and the record's.
        use mapkit_package::distant;
        let mut d: mapkit_core::MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        d.nodes.clear(); d.roads.clear(); d.zones.clear(); d.buildings.clear();
        d.bounds.max = [25600, 25600]; d.cell_size_cm = 12800;
        let id = "site-1";
        let record = distant::facade_record(id);
        let share = distant::facade_own_share("concrete:commercial");
        let own = |h: u32| distant::facade_look(record, distant::facade_solid(id, h as i64), share) != record;
        let own_height = (3..40).map(|k| k * 300).find(|&h| own(h)).unwrap();
        let record_heights: Vec<u32> = (3..40).map(|k| k * 300).filter(|&h| !own(h)).take(2).collect();
        let rect = |x0: i64, x1: i64, y0: i64| vec![[x0, y0], [x1, y0], [x1, y0 + 1000], [x0, y0 + 1000]];
        d.buildings.push(mapkit_core::Building { id: id.into(), footprint: rect(12000, 13600, 1000), holes: vec![], base_cm: 0,
            height_cm: own_height, usage: "commercial".into(), material: "concrete".into(), roof: "flat".into(), entrances: vec![],
            parts: vec![
                mapkit_core::BuildingPart { footprint: rect(11500, 14000, 4000), holes: vec![], base_cm: None, height_cm: Some(record_heights[0]) },
                mapkit_core::BuildingPart { footprint: rect(3000, 5000, 4000), holes: vec![], base_cm: None, height_cm: Some(record_heights[1]) },
            ] });
        let p = mapkit_package::read_bytes(&mapkit_package::pack_bytes(d, Default::default()).unwrap()).unwrap();
        let mut sides = Vec::new();
        for cell in [mapkit_core::Cell { x: 0, y: 0 }, mapkit_core::Cell { x: 1, y: 0 }] {
            let chunk = p.generate(cell, 500_000).unwrap();
            let mut ids: Vec<&str> = Vec::new();
            let objects: Vec<i32> = chunk.triangles.iter().map(|t| match ids.iter().position(|id| *id == t.object_id) {
                Some(i) => i as i32,
                None => { ids.push(&t.object_id); ids.len() as i32 - 1 }
            }).collect();
            let prisms: Vec<i32> = chunk.building_prisms.iter().map(|p| ids.iter().position(|id| *id == p.object_id).unwrap() as i32).collect();
            let (seeds, _) = facade_seeds(&chunk, &objects, &prisms);
            let far = p.generate_distant(cell).unwrap();
            // Solid bits per solid top, and every near wall and roof matched to
            // its distant triangle takes the colour of the near look.
            let mut tops = BTreeMap::<i64, u32>::new();
            let mut matched = 0;
            for (t, triangle) in chunk.triangles.iter().enumerate() {
                if triangle.object_id != id { continue; }
                let bits = (seeds[t * 3].x * 16_777_216.0).round() as u32;
                assert_eq!(bits >> 12, record, "the record keeps its high bits");
                let top = triangle.vertices.iter().map(|v| v[1]).max().unwrap();
                if top > 0 { assert_eq!(*tops.entry(top).or_insert(bits & 0xFFF), bits & 0xFFF, "one solid seed per top in {cell:?}"); }
                let vertex = [0, 2, 1].map(|i| { let v = triangle.vertices[i]; [v[0] as f32 * 0.01, v[1] as f32 * 0.01, -v[2] as f32 * 0.01] });
                let Some(position) = far.vertices.chunks_exact(3).position(|v| v == vertex) else { continue };
                let [a, b, c] = triangle.vertices;
                let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                let n = [(u[1] * v[2] - u[2] * v[1]) as f64, (u[2] * v[0] - u[0] * v[2]) as f64, (u[0] * v[1] - u[1] * v[0]) as f64];
                let roof = (n[1] / (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()).abs() >= 0.5;
                let look = crate::facade_fixtures::look(bits, share);
                assert_eq!(far.colors[position * 3], distant::facade_color("concrete", "commercial", look, roof), "{cell:?} triangle {t}");
                matched += 1;
            }
            assert!(matched > 0, "distant triangles in {cell:?}");
            sides.push(tops);
        }
        // The two crossing solids keep one seed on both sides of the cut.
        for h in [own_height as i64, record_heights[0] as i64] {
            assert_eq!(sides[0].get(&h), sides[1].get(&h), "solid of top {h} across the cut");
            assert_eq!(sides[0][&h], distant::facade_solid(id, h));
        }
        assert!(!sides[1].contains_key(&(record_heights[1] as i64)) && sides[0].contains_key(&(record_heights[1] as i64)));
        let looks: std::collections::BTreeSet<u32> = sides[0].values().map(|&s| crate::facade_fixtures::look((record << 12) | s, share)).collect();
        assert!(looks.len() == 2 && looks.contains(&record), "an own look and the record's: {looks:?}");
    }

    #[test]
    fn facade_tops_end_at_the_eaves_of_a_gable_taller_than_its_walls() {
        // Walls 0-100 cm, a 225 cm ridge (rise above the wall height) and the
        // flat bottom face at the base, which is not a roof.
        let triangles = vec![
            triangle("b", [[0, 0, 0], [1000, 0, 0], [0, 100, 0]]),
            triangle("b", [[0, 100, 0], [1000, 100, 0], [500, 225, 300]]),
            triangle("b", [[0, 100, 0], [500, 225, 300], [0, 100, 600]]),
            triangle("b", [[0, 0, 0], [0, 0, 600], [1000, 0, 0]]),
        ];
        let chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], format_version: 1,
            cell: mapkit_core::Cell { x: 0, y: 0 }, triangles, objects: vec![], building_prisms: vec![], asset_convexes: vec![] };
        let (_, tops) = facade_seeds(&chunk, &[0, 0, 0, 0], &[0]);
        let scale = mapkit_core::WORLD_SCALE as f32 / 100.0;
        assert!(tops.iter().all(|t| t.abs() == FACADE_BASE_OFFSET + 100.0 * scale), "a steep gable still ends storeys at the eaves");
        assert!(tops[0] > 0.0 && tops[9] > 0.0 && tops[3] < 0.0 && tops[6] < 0.0, "only the slopes rise above the top");
    }
}
