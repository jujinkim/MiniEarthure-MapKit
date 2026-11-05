//! Display-only, deterministic distant geometry. No collision/archive contract changes.
use crate::Package;
use mapkit_core::{cancellation::checkpoint, Cell, GeneratedChunk, GeneratedObject, Result, Surface, Triangle, water::WaterCell};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct DistantMesh {
    pub vertices: Vec<[f32; 3]>,
    /// Display sRGB, matching Godot colour properties (alpha remains linear).
    pub colors: Vec<[u8; 4]>,
    /// Light seed/role for solids; scene-space flow m/s for water (kind 2).
    pub light_data: Vec<[f32; 2]>,
    /// Display kind: 0 solid, 1 small prop, 2 water. No collision semantics.
    pub decoration: Vec<u8>,
}

/// City trees of the near renderer (map-kit godot/vegetation_meshes.gd `TREES`):
/// canopy base, canopy height and silhouette radius in metres at scale 1 (the
/// near crowns reach at most 1.2 m at the largest scale, the 120 cm placement
/// footprint). A tree's variant, scale and yaw come from its object identity
/// and footprint radius exactly as in chunk_renderer.gd `vegetation_placement`
/// and vegetation_meshes.gd `tree_set`, so the silhouette matches the near
/// tree it replaces.
const CITY_TREES: [[f32; 3]; 3] = [[1.5, 2.3, 0.95], [1.45, 2.9, 0.85], [1.7, 1.8, 1.0]];
/// Wide park trees (vegetation_meshes.gd `WIDE_TREES`) of zone footprints from
/// `WIDE_FROM_CM`: canopy base and height to the near crown's top, and a
/// hexagon whose mean radius matches the near crown's mean silhouette (between
/// its leaf cards and far hull), at most 2.0 m at the largest scale.
const WIDE_CITY_TREES: [[f32; 3]; 3] = [[1.65, 3.47, 1.73], [1.6, 3.57, 1.53], [1.75, 3.18, 1.74]];
const WIDE_FROM_CM: i64 = 160;
/// One distant city tree: a three-sided trunk (6 triangles), a hexagonal crown
/// frustum (12) and a hexagonal cone over it (6). The full generation's 12
/// trunk-box triangles of the tree, which the distant generation omits, plus the
/// per-object `maximum` (>= 12) in `distant_triangle_bound` cover it, so the
/// bound needs no city-tree term.
const CITY_TREE_TRIANGLES: u64 = 24;
const _: () = assert!(CITY_TREE_TRIANGLES <= 12 + 12);
/// Street trees of the near renderer (map-kit godot/street_planting_meshes.gd
/// `FORMS`, in `mapkit_core::STREET_TREES` order: ginkgo, zelkova, plane,
/// cherry, pine): canopy base and height in metres at scale 1 and a hexagon
/// radius matching the near crown's mean silhouette, inside the species'
/// crown reach at the largest scale. Variant (12 % young replacements at
/// scale 1 with the crown x 0.65), scale (0.92-1.08) and quarter turn come
/// from the object identity exactly as in street_planting_meshes.gd
/// `tree_piece`, so the silhouette matches the near tree it replaces. A
/// street tree draws CITY_TREE_TRIANGLES like a city tree.
const STREET_TREES: [[f32; 3]; 5] = [[1.35, 2.55, 0.47], [1.35, 2.10, 0.98], [1.45, 2.35, 0.9], [1.35, 1.45, 0.94], [1.35, 1.55, 0.67]];
const STREET_TREE_SCALE: [f32; 2] = [0.92, 1.08];
const STREET_YOUNG_SHARE: f32 = 0.12;
const STREET_YOUNG_CROWN: f32 = 0.65;
/// Flat sRGB colours per species: bark, shaded lower crown, lit upper crown
/// (ginkgo light yellow-green, zelkova and plane mid green, cherry dark green,
/// pine blue-green).
const STREET_TREE_COLORS: [[[u8; 4]; 3]; 5] = [
    [[94, 82, 72, 255], [104, 128, 56, 255], [128, 152, 70, 255]],
    [[138, 131, 122, 255], [62, 96, 56, 255], [80, 118, 66, 255]],
    [[160, 156, 130, 255], [70, 104, 60, 255], [88, 124, 70, 255]],
    [[75, 59, 54, 255], [50, 82, 50, 255], [64, 98, 60, 255]],
    [[130, 88, 62, 255], [40, 70, 58, 255], [54, 88, 70, 255]]];
/// The pine of a median bed keeps its proxy faces (bed slab and trunk box) and
/// adds only a crown: a hexagonal bicone of 12 triangles, inside the
/// per-object `maximum` (>= 12) of `distant_triangle_bound`.
const MEDIAN_PINE_TRIANGLES: u64 = 12;
const _: () = assert!(MEDIAN_PINE_TRIANGLES <= 12);
/// Flower-bed proxy faces (planters and median beds, near
/// street_planting_meshes.gd `FAR_SIDE` and `FAR_TOP`): granite sides and a
/// planted top; the median pine's trunk box in pine bark.
const BED_SIDE: [u8; 4] = [170, 166, 158, 255];
const BED_TOP: [u8; 4] = [86, 112, 70, 255];

#[derive(Clone, Debug)]
struct Proxy {
    min: [f32; 3],
    max: [f32; 3],
    color: [u8; 4],
    role: u8,
}

/// Native buildings are generated as closed triangular prisms. Adjacent prisms
/// carry opposite copies of an internal vertical quad (often with different
/// diagonals). Remove only an exact pair of those quads from the distant product.
/// Exterior, roof, courtyard, cell-cut and authored-asset geometry stays intact.
fn internal_building_faces(document: &mapkit_core::MapDocument, triangles: &[Triangle]) -> Result<Vec<bool>> {
    if document.buildings.is_empty() { return Ok(Vec::new()); }
    type Face = ([[i64; 3]; 4], usize, bool);
    // One record per two source triangles, fixed capacity: no geometric growth.
    let mut faces: Vec<Face> = Vec::with_capacity(triangles.len() / 2);
    let mut index = 0;
    while index + 1 < triangles.len() {
        checkpoint()?;
        let a = &triangles[index];
        let b = &triangles[index + 1];
        if a.object_id != b.object_id || document.buildings.binary_search_by(|v| v.id.cmp(&a.object_id)).is_err() {
            index += 1;
            continue;
        }
        let direction = |t: &Triangle| -> Option<bool> {
            let u = std::array::from_fn::<_, 3, _>(|i| i128::from(t.vertices[1][i]) - i128::from(t.vertices[0][i]));
            let v = std::array::from_fn::<_, 3, _>(|i| i128::from(t.vertices[2][i]) - i128::from(t.vertices[0][i]));
            let n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]];
            (n[1] == 0 && (n[0] != 0 || n[2] != 0)).then_some(n[0] < 0 || (n[0] == 0 && n[2] < 0))
        };
        let Some(side) = direction(a).filter(|s| Some(*s) == direction(b)) else { index += 1; continue; };
        let mut points = [a.vertices[0], a.vertices[1], a.vertices[2], b.vertices[0], b.vertices[1], b.vertices[2]];
        points.sort_unstable();
        let mut key = [[0; 3]; 4];
        let mut count = 0;
        for i in 0..6 {
            if i > 0 && points[i] == points[i-1] { continue; }
            if count < 4 { key[count] = points[i]; }
            count += 1;
        }
        if count != 4 { index += 1; continue; }
        let mut horizontal = key.map(|p| [p[0], p[2]]);
        horizontal.sort_unstable();
        if horizontal[0] != horizontal[1] || horizontal[2] != horizontal[3] || horizontal[0] == horizontal[2] {
            index += 1;
            continue;
        }
        faces.push((key, index, side));
        index += 2;
    }
    faces.sort_unstable_by_key(|face| face.0);
    let mut hidden = vec![false; triangles.len()];
    let mut first = 0;
    while first < faces.len() {
        checkpoint()?;
        let mut end = first + 1;
        while end < faces.len() && faces[end].0 == faces[first].0 { end += 1; }
        // Ambiguous/nonmanifold groups stay visible rather than guessing a union.
        if end == first + 2 && faces[first].2 != faces[first+1].2 {
            for face in &faces[first..end] { hidden[face.1] = true; hidden[face.1+1] = true; }
        }
        first = end;
    }
    Ok(hidden)
}

fn multiply(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}

fn gltf_color(value: [f32; 4]) -> [u8; 4] {
    std::array::from_fn(|c| {
        let v = value[c].clamp(0., 1.);
        let display = if c == 3 { v } else if v <= 0.0031308 { v * 12.92 }
            else { 1.055 * libm::powf(v, 1. / 2.4) - 0.055 };
        (display * 255.).clamp(0., 255.).round() as u8
    })
}

/// Read the authored silhouette, preserving holes and the complete node hierarchy.
/// GLTF uses counter-clockwise faces; the common Godot display view uses clockwise.
fn silhouette(bytes: &[u8], tint: Option<[u8; 4]>, binding: Option<&mapkit_core::environment::LightBinding>) -> Result<DistantMesh> {
    let glb = gltf::Gltf::from_slice(bytes)
        .map_err(|_| mapkit_core::error("E_ASSET", "invalid distant GLB"))?;
    let blob = glb.blob.as_deref().unwrap_or_default();
    let identity = [[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]];
    let mut result = DistantMesh::default();
    let mut pending = Vec::new();
    if let Some(scene) = glb.default_scene().or_else(|| glb.scenes().next()) {
        pending.extend(scene.nodes().map(|node| (node, identity)));
    }
    while let Some((node, parent)) = pending.pop() {
        checkpoint()?;
        let m = multiply(parent, node.transform().matrix());
        let determinant = m[0][0]*(m[1][1]*m[2][2]-m[1][2]*m[2][1])
            - m[1][0]*(m[0][1]*m[2][2]-m[0][2]*m[2][1])
            + m[2][0]*(m[0][1]*m[1][2]-m[0][2]*m[1][1]);
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|_| Some(blob));
                let points: Vec<_> = reader.read_positions().unwrap().collect();
                let indices: Vec<_> = reader.read_indices().map(|i| i.into_u32().collect())
                    .unwrap_or_else(|| (0..points.len() as u32).collect());
                let colors: Option<Vec<_>> = reader.read_colors(0).map(|c| c.into_rgba_f32().collect());
                let base = primitive.material().pbr_metallic_roughness().base_color_factor();
                let index = primitive.material().index().unwrap_or(usize::MAX) as u16;
                let role = binding.map_or(0, |b| if b.window_materials.contains(&index) {1} else if b.bulb_materials.contains(&index) {2} else {0});
                for face in indices.chunks_exact(3) {
                    checkpoint()?;
                    for corner in if determinant < 0. { [0,1,2] } else { [0,2,1] } {
                        let i = face[corner] as usize;
                        result.vertices.push(std::array::from_fn(|r| m[3][r]+(0..3).map(|c| m[c][r]*points[i][c]).sum::<f32>()));
                        // A source override is already sRGB and replaces the
                        // GLTF material/vertex tint, as the primary importer does.
                        result.colors.push(tint.unwrap_or_else(|| gltf_color(std::array::from_fn(|c|
                            base[c]*colors.as_ref().map_or(1., |v| v[i][c])))));
                        result.light_data.push([0.,role as f32]);
                        result.decoration.push(0);
                    }
                }
            }
        }
        pending.extend(node.children().map(|child| (child, m)));
    }
    Ok(result)
}

fn placed(mut p: [f32; 3], position: [i64; 3], quarter_turns: u8, yaw_offset_mdeg: i32) -> [f32; 3] {
    for _ in 0..quarter_turns { p = [p[2],p[1],-p[0]]; }
    if yaw_offset_mdeg != 0 {
        let a = f64::from(yaw_offset_mdeg).to_radians()/1000.;
        let (s,c) = (a.sin() as f32,a.cos() as f32);
        p = [p[0]*c+p[2]*s,p[1],-p[0]*s+p[2]*c];
    }
    [p[0]+position[0] as f32*0.01,p[1]+position[1] as f32*0.01,p[2]-position[2] as f32*0.01]
}

fn proxies(bytes: &[u8], tint: Option<[u8; 4]>, binding: Option<&mapkit_core::environment::LightBinding>) -> Result<Vec<Proxy>> {
    let glb = gltf::Gltf::from_slice(bytes)
        .map_err(|_| mapkit_core::error("E_ASSET", "invalid distant GLB"))?;
    let blob = glb.blob.as_deref().unwrap_or_default();
    let identity = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    let mut output = Vec::new();
    let mut pending = Vec::new();
    if let Some(scene) = glb.default_scene().or_else(|| glb.scenes().next()) {
        pending.extend(scene.nodes().map(|node| (node, identity)));
    }
    while let Some((node, parent)) = pending.pop() {
        checkpoint()?;
        let transform = multiply(parent, node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            // One box per material in a mesh: small details merge, while canopy,
            // trunk and differently coloured building volumes stay distinguishable.
            let mut groups = BTreeMap::<([u8; 4], u8), Proxy>::new();
            for primitive in mesh.primitives() {
                let color = tint.unwrap_or_else(|| {
                    gltf_color(primitive
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_factor())
                });
                let index = primitive.material().index().unwrap_or(usize::MAX) as u16;
                let role = binding.map_or(0, |b| if b.window_materials.contains(&index) {1} else if b.bulb_materials.contains(&index) {2} else {0});
                let group = groups.entry((color,role)).or_insert(Proxy {
                    min: [f32::INFINITY; 3],
                    max: [f32::NEG_INFINITY; 3],
                    color, role,
                });
                if let Some(positions) = primitive.reader(|_| Some(blob)).read_positions() {
                    for point in positions {
                        for r in 0..3 {
                            let v = transform[3][r]
                                + (0..3).map(|c| transform[c][r] * point[c]).sum::<f32>();
                            group.min[r] = group.min[r].min(v);
                            group.max[r] = group.max[r].max(v);
                        }
                    }
                }
            }
            output.extend(
                groups
                    .into_values()
                    .filter(|g| g.min.iter().chain(&g.max).all(|v| v.is_finite())),
            );
        }
        pending.extend(node.children().map(|child| (child, transform)));
    }
    Ok(output)
}

// Shared with the complete regional audit; no GLB or mutable source is retained.
pub(crate) fn visual_margin_cm(
    document: &mapkit_core::MapDocument,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<i64> {
    let mut radius = 1.84_f32;
    for asset in &document.assets {
        if !asset.path.ends_with(".glb") { continue; }
        for path in asset.paths() {
            for proxy in proxies(&files[path], None, None)? {
                let x = proxy.min[0].abs().max(proxy.max[0].abs());
                let z = proxy.min[2].abs().max(proxy.max[2].abs());
                radius = radius.max(x.hypot(z));
            }
        }
    }
    Ok((radius * 100.0).ceil() as i64)
}

impl Package {
    /// Native face matching, parsing and per-asset silhouette templates. This is
    /// generation scratch, never a near-render cache or collision allocation.
    pub fn distant_workspace_bytes(&self, cell: Cell) -> Result<u64> {
        let mut ids: BTreeSet<_> = self.document.authored_placement_candidates(cell)?
            .into_iter().map(|p| p.asset_id.as_str()).collect();
        ids.extend(self.document.zones.iter().filter_map(|z| z.tree.as_ref().map(|t| t.asset_id.as_str())));
        let building_workspace = if self.document.buildings.is_empty() { 0 }
            else { self.document.estimate(cell, 500_000)?.triangles.saturating_mul(128) };
        Ok(building_workspace.saturating_add(self.document.assets.iter().filter(|a| ids.contains(a.id.as_str()))
            .filter_map(|a| a.distant_path.as_ref())
            .map(|path| crate::assets::presentation_cost(path, &self.files[path])).sum::<u64>()))
    }
    /// Conservative anchor-to-visual horizontal extent, independent of collision proxies.
    pub fn visual_margin_cm(&self) -> Result<i64> {
        visual_margin_cm(&self.document, &self.files)
    }

    /// Conservative upper bound before generation; parsing/import workspace is
    /// charged separately through the existing validated asset allowances.
    pub fn distant_triangle_bound(&self, cell: Cell) -> Result<u64> {
        let cost = self.document.estimate(cell, 500_000)?;
        let mut maximum = 12;
        for asset in &self.document.assets {
            if !asset.path.ends_with(".glb") {
                continue;
            }
            let path = asset.distant_path.as_ref().unwrap_or(&asset.path);
            let glb = gltf::Gltf::from_slice(&self.files[path])
                .map_err(|_| mapkit_core::error("E_ASSET", "invalid distant asset"))?;
            let count: usize = glb
                .nodes()
                .filter_map(|n| n.mesh())
                .map(|m| m.primitives().map(|p| {
                    if asset.distant_path.is_some() {
                        p.indices().map_or_else(|| p.get(&gltf::Semantic::Positions).unwrap().count(), |a| a.count())/3
                    } else { 12 }
                }).sum::<usize>())
                .sum();
            maximum = maximum.max(count as u64);
        }
        let area = self.document.cell_bounds(cell)?;
        let water: u64 = self.document.water_bodies.iter().filter(|b| b.intersects(&area))
            .map(|b| (b.vertices() + 2 * b.islands.len() - 2) as u64 * 5).sum();
        Ok(cost
            .triangles
            .saturating_add(water)
            .saturating_add(cost.objects.saturating_mul(maximum)))
    }

    pub fn generate_distant(&self, cell: Cell) -> Result<DistantMesh> {
        checkpoint()?;
        let grid = self.generation_heightgrid(cell)?;
        let source = self.document.generate_distant_source(cell, grid.as_ref(), 500_000)?;
        self.distant_from_parts(&source.triangles, &source.objects, &source.water_bodies)
    }

    pub fn distant_from_generated(&self, chunk: &GeneratedChunk) -> Result<DistantMesh> {
        self.distant_from_parts(&chunk.triangles, &chunk.objects, &chunk.water_bodies)
    }

    fn distant_from_parts(&self, triangles: &[Triangle], objects: &[GeneratedObject], water_bodies: &[WaterCell]) -> Result<DistantMesh> {
        checkpoint()?;
        let mut result = DistantMesh::default();
        // Keep all small props contiguous: at most one additional display batch,
        // covered by the existing fixed display allowance, not one per object.
        let mut decorations = DistantMesh::default();
        let mut omitted = BTreeSet::new();
        let internal = internal_building_faces(&self.document, triangles)?;
        // Barrier variants by barrier index: one design per barrier, the
        // map scale read once.
        let denominator = mapkit_core::osm_source_denominator(&self.document).unwrap_or(1);
        let mut barrier_colors: BTreeMap<usize, [u8; 4]> = BTreeMap::new();
        for placement in &self.document.placements {
            if self.document.asset(&placement.asset_id).is_some_and(|a| a.path.ends_with(".glb"))
            {
                omitted.insert(placement.id.as_str());
            }
        }
        for object in objects {
            checkpoint()?;
            if mapkit_core::distant_tree(&object.asset_id)
                || self.document.asset(&object.asset_id).is_some_and(|a| a.path.ends_with(".glb"))
            {
                omitted.insert(object.id.as_str());
            }
        }
        for (index, triangle) in triangles.iter().enumerate() {
            checkpoint()?;
            if internal.get(index).copied().unwrap_or(false) || omitted.contains(triangle.object_id.as_str()) {
                continue;
            }
            let building = self.document.buildings.binary_search_by(|b| b.id.cmp(&triangle.object_id))
                .ok().map(|i| &self.document.buildings[i]);
            let color = if let Some(building) = building {
                let [a, b, c] = triangle.vertices;
                let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                let n = [(u[1] * v[2] - u[2] * v[1]) as f64, (u[2] * v[0] - u[0] * v[2]) as f64, (u[0] * v[1] - u[1] * v[0]) as f64];
                let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let roof = length > 0.0 && (n[1] / length).abs() >= 0.5;
                facade_color(&building.material, &building.usage, facade_record(&triangle.object_id), roof)
            } else if let Some(color) = self.document.placements.binary_search_by(|p| p.id.cmp(&triangle.object_id)).ok()
                .and_then(|i| planting_color(&self.document.placements[i], triangle)) {
                color
            } else if let Ok(i) = self.document.barriers.binary_search_by(|b| b.id.cmp(&triangle.object_id)) {
                // The near cells' variant (barrier_design.rs): wall and hedge
                // paint, fences as the haze their far meshes settle to.
                *barrier_colors.entry(i).or_insert_with(|| {
                    let b = &self.document.barriers[i];
                    crate::barrier_design::distant_color(crate::barrier_design::design(&b.id, b.kind, b.height_cm, denominator))
                })
            } else {
                match triangle.surface {
                    Surface::Asphalt => [48, 52, 59, 255],
                    Surface::Concrete => [183, 184, 176, 255],
                    Surface::Dirt => [146, 116, 86, 255],
                    Surface::Gravel => [136, 132, 119, 255],
                    Surface::Grass => self.document.environment.as_ref().and_then(|e| e.ground_color)
                        .map(|c| [c[0],c[1],c[2],255]).unwrap_or([115,134,100,255]),
                }
            };
            for i in [0, 2, 1] {
                let p = triangle.vertices[i];
                result
                    .vertices
                    .push([p[0] as f32 * 0.01, p[1] as f32 * 0.01, -p[2] as f32 * 0.01]);
                result.colors.push(color);
                // Terrain keeps the same large-scale shading in both paths.
                // Negative roles are display tags, never emissive bindings; role 3
                // lights native building walls at night with a per-object seed.
                result.light_data.push(if building.is_some() {
                    [object_seed(&self.document.map_id, &triangle.object_id), 3.]
                } else {
                    [0., if matches!(triangle.surface, Surface::Grass | Surface::Dirt) { -1. } else { 0. }]
                });
                result.decoration.push(0);
            }
        }
        // Distant water keeps the same clipped shoreline and elevation. This
        // tagged surface uses the common water material and never enters
        // collision/physical triangles. Flow uses the existing two UV channels.
        for water in water_bodies {
            checkpoint()?;
            for triangle in &water.surface {
                for i in [0, 2, 1] {
                    let p = triangle[i];
                    result.vertices.push([p[0] as f32 * 0.01, p[1] as f32 * 0.01, -p[2] as f32 * 0.01]);
                    result.colors.push([56, 103, 112, 255]);
                    result.light_data.push([water.body.flow_cm_s[0] as f32 * 0.01,
                        -water.body.flow_cm_s[1] as f32 * 0.01]);
                    result.decoration.push(2);
                }
            }
        }
        let mut templates = BTreeMap::new();
        let mut silhouettes = BTreeMap::new();
        let mut city_radii = None;
        for object in objects {
            checkpoint()?;
            if object.asset_id == "builtin:tree" {
                add_box(
                    &mut result,
                    &Proxy {
                        min: [-1.84, 2.8, -1.84],
                        max: [1.84, 6.8, 1.84],
                        color: [72, 100, 71, 255], role:0,
                    },
                    object.position,
                    object.quarter_turns, object.yaw_offset_mdeg, 0.,
                );
                continue;
            }
            if object.asset_id == "builtin:city_tree" {
                let radii = city_radii.get_or_insert_with(|| mapkit_core::city_tree_zone_radii(&self.document));
                add_city_tree(&mut result, &object.id, object.position, mapkit_core::city_tree_radius(radii, &object.id));
                continue;
            }
            if let Some(species) = mapkit_core::STREET_TREES.iter().position(|(id, _)| *id == object.asset_id) {
                add_street_tree(&mut result, object, species);
                continue;
            }
            if object.asset_id == "builtin:median_bed_pine" {
                add_median_pine(&mut result, object);
                continue;
            }
            let Some(asset) = self.document.asset(&object.asset_id).filter(|a| a.path.ends_with(".glb"))
            else {
                continue;
            };
            let seed = object_seed(&self.document.map_id, &object.id);
            if let Some(path) = &asset.distant_path {
                if !silhouettes.contains_key(&asset.id) {
                    let mesh = silhouette(&self.files[path],
                        asset.material.as_ref().map(|m| m.albedo_rgba),
                        self.document.environment.as_ref().and_then(|e| e.lights.iter().find(|b| b.asset_id==asset.id)))?;
                    let mut low = [f32::INFINITY;3];let mut high = [f32::NEG_INFINITY;3];
                    for p in &mesh.vertices { for a in 0..3 { low[a]=low[a].min(p[a]);high[a]=high[a].max(p[a]); } }
                    let small = !asset.has_visible_collision()
                        && (0..3).map(|a| (high[a]-low[a]).powi(2)).sum::<f32>() <= 36.;
                    silhouettes.insert(&asset.id, (mesh, small));
                }
                let (mesh, small) = &silhouettes[&asset.id];
                let small = *small;
                let target = if small { &mut decorations } else { &mut result };
                target.vertices.extend(mesh.vertices.iter().map(|&p| placed(p,object.position,object.quarter_turns,object.yaw_offset_mdeg)));
                target.colors.extend_from_slice(&mesh.colors);
                target.light_data.extend(mesh.light_data.iter().map(|p| [seed,p[1]]));
                target.decoration.extend(std::iter::repeat_n(u8::from(small),mesh.vertices.len()));
                continue;
            }
            if !templates.contains_key(&asset.id) {
                templates.insert(
                    &asset.id,
                    proxies(
                        &self.files[&asset.path],
                        asset.material.as_ref().map(|m| m.albedo_rgba),
                        self.document.environment.as_ref().and_then(|e| e.lights.iter().find(|b| b.asset_id==asset.id)),
                    )?,
                );
            }
            for proxy in &templates[&asset.id] {
                add_box(&mut result, proxy, object.position, object.quarter_turns, object.yaw_offset_mdeg, seed);
            }
        }
        result.vertices.append(&mut decorations.vertices);
        result.colors.append(&mut decorations.colors);
        result.light_data.append(&mut decorations.light_data);
        result.decoration.append(&mut decorations.decoration);
        Ok(result)
    }
}

/// Shapes, size factor, scale span and trunk radii (foot, crown) of the city
/// tree set for a placement footprint of `radius_cm` (vegetation_meshes.gd
/// `tree_set`): wide park trees scaled by radius / 200 from `WIDE_FROM_CM`,
/// else the street trees scaled by radius / 120.
fn city_tree_set(radius_cm: i64) -> (&'static [[f32; 3]; 3], f32, [f32; 2], [f32; 2]) {
    let radius = radius_cm.clamp(120, 200);
    if radius >= WIDE_FROM_CM {
        (&WIDE_CITY_TREES, radius as f32 / 200., [0.95, 1.1], [0.22, 0.15])
    } else {
        (&CITY_TREES, radius as f32 / 120., [0.85, 1.15], [0.16, 0.12])
    }
}

/// Trunk, crown frustum and crown cone of a city tree with a placement
/// footprint of `radius_cm` in scene metres. Flat sRGB colours like the other
/// proxies: bark, a darker lower crown (shaded underside) and a lighter top.
fn add_city_tree(output: &mut DistantMesh, object_id: &str, position: [i64; 3], radius_cm: i64) {
    let hash = mapkit_core::sha256(object_id.as_bytes());
    let pick = u32::from_str_radix(&hash[..6], 16).unwrap() as f32 / 16_777_216.;
    let (shapes, factor, span, trunk) = city_tree_set(radius_cm);
    let [base, height, radius] = shapes[(pick * 3.) as usize % 3];
    let scale = factor * (span[0] + (span[1] - span[0]) * (pick * 7.).fract());
    let yaw = std::f32::consts::TAU * (pick * 13.).fract();
    let origin = [position[0] as f32 * 0.01, position[1] as f32 * 0.01, -position[2] as f32 * 0.01];
    let ring = |sides: usize, r: f32, y: f32| -> Vec<[f32; 3]> {
        (0..sides).map(|k| {
            let a = yaw + std::f32::consts::TAU * (k as f32 + 0.5) / sides as f32;
            [origin[0] + a.cos() * r * scale, origin[1] + y * scale, origin[2] + a.sin() * r * scale]
        }).collect()
    };
    let mut triangle = |points: [[f32; 3]; 3], color: [u8; 4]| {
        for p in points {
            output.vertices.push(p);
            output.colors.push(color);
            output.light_data.push([0., 0.]);
            output.decoration.push(0);
        }
    };
    // Side quads between two rings, or a cone to one apex; (b - a) x (c - a)
    // faces outward.
    let mut band = |lower: &[[f32; 3]], upper: &[[f32; 3]], color: [u8; 4]| {
        let n = lower.len();
        for k in 0..n {
            let j = (k + 1) % n;
            if upper.len() == n { triangle([lower[k], upper[k], upper[j]], color); }
            triangle([lower[k], upper[j % upper.len()], lower[j]], color);
        }
    };
    band(&ring(3, trunk[0], -0.1), &ring(3, trunk[1], base + 0.3), [90, 68, 51, 255]);
    let middle = ring(6, radius, base + height * 0.5);
    band(&ring(6, radius * 0.55, base + height * 0.05), &middle, [62, 90, 58, 255]);
    band(&middle, &[[origin[0], origin[1] + (base + height) * scale, origin[2]]], [80, 110, 70, 255]);
}

/// Identity unit of an object (chunk_renderer.gd `_identity_unit`).
fn identity_unit(object_id: &str) -> f32 {
    let hash = mapkit_core::sha256(object_id.as_bytes());
    u32::from_str_radix(&hash[..6], 16).unwrap() as f32 / 16_777_216.
}

/// Shape (base, height, radius), scale and yaw of a street tree or the median
/// pine (`bed`: never young) from its identity, as street_planting_meshes.gd
/// `tree_piece`.
fn street_tree_shape(object: &GeneratedObject, species: usize, bed: bool) -> ([f32; 3], f32, f32) {
    let pick = identity_unit(&object.id);
    let young = !bed && (pick * 11.).fract() < STREET_YOUNG_SHARE;
    let [base, height, radius] = STREET_TREES[species];
    let shape = if young { [base, height * STREET_YOUNG_CROWN, radius * STREET_YOUNG_CROWN] } else { [base, height, radius] };
    let scale = if young { 1. } else { STREET_TREE_SCALE[0] + (STREET_TREE_SCALE[1] - STREET_TREE_SCALE[0]) * (pick * 7.).fract() };
    let quarter = ((pick * 13.).fract() * 4.) as u32 % 4;
    let yaw = (f32::from(object.quarter_turns) + quarter as f32) * std::f32::consts::FRAC_PI_2
        + (f64::from(object.yaw_offset_mdeg).to_radians() / 1000.) as f32;
    (shape, scale, yaw)
}

/// Trunk, crown frustum and crown cone of a street tree (CITY_TREE_TRIANGLES)
/// in its species colours; the pine's crown is a flatter frustum of tiers.
fn add_street_tree(output: &mut DistantMesh, object: &GeneratedObject, species: usize) {
    let ([base, height, radius], scale, yaw) = street_tree_shape(object, species, false);
    let [bark, lower, upper] = STREET_TREE_COLORS[species];
    let origin = [object.position[0] as f32 * 0.01, object.position[1] as f32 * 0.01, -object.position[2] as f32 * 0.01];
    let ring = |sides: usize, r: f32, y: f32| -> Vec<[f32; 3]> { hex_ring(origin, yaw, scale, sides, r, y) };
    band_faces(output, &ring(3, 0.13, -0.1), &ring(3, 0.08, base + 0.3), bark);
    let pine = species == 4;
    let middle = ring(6, radius, base + height * if pine { 0.4 } else { 0.5 });
    band_faces(output, &ring(6, radius * if pine { 0.75 } else { 0.55 }, base + height * 0.05), &middle, lower);
    band_faces(output, &middle, &[[origin[0], origin[1] + (base + height) * scale, origin[2]]], upper);
}

/// Crown of a median bed's pine (MEDIAN_PINE_TRIANGLES): a hexagonal bicone
/// from the canopy base to the top; its trunk is the bed's proxy box.
fn add_median_pine(output: &mut DistantMesh, object: &GeneratedObject) {
    let ([base, height, radius], scale, yaw) = street_tree_shape(object, 4, true);
    let [_, lower, upper] = STREET_TREE_COLORS[4];
    let origin = [object.position[0] as f32 * 0.01, object.position[1] as f32 * 0.01, -object.position[2] as f32 * 0.01];
    let middle = hex_ring(origin, yaw, scale, 6, radius, base + height * 0.4);
    band_faces(output, &[[origin[0], origin[1] + base * scale, origin[2]]], &middle, lower);
    band_faces(output, &middle, &[[origin[0], origin[1] + (base + height) * scale, origin[2]]], upper);
}

fn hex_ring(origin: [f32; 3], yaw: f32, scale: f32, sides: usize, r: f32, y: f32) -> Vec<[f32; 3]> {
    (0..sides).map(|k| {
        let a = yaw + std::f32::consts::TAU * (k as f32 + 0.5) / sides as f32;
        [origin[0] + a.cos() * r * scale, origin[1] + y * scale, origin[2] + a.sin() * r * scale]
    }).collect()
}

/// Side quads between two rings, or a cone from a ring to one apex (or from
/// one point up to a ring); (b - a) x (c - a) faces outward.
fn band_faces(output: &mut DistantMesh, lower: &[[f32; 3]], upper: &[[f32; 3]], color: [u8; 4]) {
    let mut triangle = |points: [[f32; 3]; 3]| {
        for p in points {
            output.vertices.push(p);
            output.colors.push(color);
            output.light_data.push([0., 0.]);
            output.decoration.push(0);
        }
    };
    if lower.len() == 1 {
        let n = upper.len();
        for k in 0..n { triangle([lower[0], upper[(k + 1) % n], upper[k]]); }
        return;
    }
    let n = lower.len();
    for k in 0..n {
        let j = (k + 1) % n;
        if upper.len() == n { triangle([lower[k], upper[k], upper[j]]); }
        triangle([lower[k], upper[j % upper.len()], lower[j]]);
    }
}

/// Distant colour of a street-planting proxy triangle: flower beds BED_SIDE
/// and BED_TOP (the median pine's trunk box in pine bark); a street tree's
/// trunk box in its species bark (only the clipped sliver a neighbour cell
/// keeps, as the anchor cell draws the far tree instead). None for other
/// placements.
fn planting_color(placement: &mapkit_core::Placement, triangle: &Triangle) -> Option<[u8; 4]> {
    if let Some(species) = mapkit_core::STREET_TREES.iter().position(|(id, _)| *id == placement.asset_id) {
        return Some(STREET_TREE_COLORS[species][0]);
    }
    if !matches!(placement.asset_id.as_str(), "builtin:planter" | "builtin:planter_square" | "builtin:median_bed" | "builtin:median_bed_pine") {
        return None;
    }
    let [a, b, c] = triangle.vertices;
    if placement.asset_id == "builtin:median_bed_pine" && [a, b, c].iter().any(|p| p[1] > placement.position[1] + 10) {
        return Some(STREET_TREE_COLORS[4][0]);
    }
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [(u[1] * v[2] - u[2] * v[1]) as f64, (u[2] * v[0] - u[0] * v[2]) as f64, (u[0] * v[1] - u[1] * v[0]) as f64];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    Some(if length > 0.0 && (n[1] / length).abs() >= 0.5 { BED_TOP } else { BED_SIDE })
}

fn add_box(output: &mut DistantMesh, proxy: &Proxy, position: [i64; 3], quarter_turns: u8, yaw_offset_mdeg: i32, seed: f32) {
    let points: [[f32; 3]; 8] = std::array::from_fn(|i| {
        let p = std::array::from_fn(|a| {
            if i & (1 << a) == 0 {
                proxy.min[a]
            } else {
                proxy.max[a]
            }
        });
        placed(p,position,quarter_turns,yaw_offset_mdeg)
    });
    for face in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
    ] {
        for i in [0, 1, 2, 0, 2, 3] {
            output.vertices.push(points[face[i]]);
            output.colors.push(proxy.color);
            output.light_data.push([seed,proxy.role as f32]);
            output.decoration.push(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn city_tree_is_a_trunk_frustum_and_cone_matching_the_near_tree() {
        for (id, footprint) in [("tree-a", 120), ("tree-b", 120), ("tree-c", 140), ("zone-1:4:7", 120),
            ("park-1:4:7", 200), ("park-1:5:7", 200), ("park-1:6:7", 200), ("park-1:4:8", 170)] {
            let mut mesh = DistantMesh::default();
            add_city_tree(&mut mesh, id, [1000, 200, -3000], footprint);
            assert_eq!(mesh.vertices.len() as u64, CITY_TREE_TRIANGLES * 3);
            assert_eq!(mesh.colors.len(), mesh.vertices.len());
            assert!(mesh.decoration.iter().all(|&d| d == 0) && mesh.light_data.iter().all(|l| *l == [0., 0.]));
            let hash = mapkit_core::sha256(id.as_bytes());
            let pick = u32::from_str_radix(&hash[..6], 16).unwrap() as f32 / 16_777_216.;
            let (shapes, factor, span) = if footprint >= WIDE_FROM_CM {
                (WIDE_CITY_TREES, footprint as f32 / 200., [0.95, 1.1])
            } else {
                (CITY_TREES, footprint as f32 / 120., [0.85, 1.15])
            };
            let [base, height, radius] = shapes[(pick * 3.) as usize % 3];
            let scale = factor * (span[0] + (span[1] - span[0]) * (pick * 7.).fract());
            let top = mesh.vertices.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            let low = mesh.vertices.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            assert!((top - (2. + (base + height) * scale)).abs() < 1e-4 && (low - (2. - 0.1 * scale)).abs() < 1e-4);
            let reach = mesh.vertices.iter().map(|p| (p[0] - 10.).hypot(p[2] - 30.)).fold(0f32, f32::max);
            assert!((reach - radius * scale).abs() < 1e-4);
            // The silhouette stays inside the placement footprint at every scale.
            assert!(radius * factor * span[1] <= footprint as f32 / 100., "{id}: {reach} m of {footprint} cm");
            if footprint == 200 {
                assert!(reach >= 1.4 && top - 2. >= 4.6, "{id}: a wide park silhouette ({reach} m, {} m tall)", top - 2.);
            }
            // Every face points away from the tree axis or up (outward winding).
            for t in mesh.vertices.chunks(3) {
                let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
                let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let c = [(t[0][0] + t[1][0] + t[2][0]) / 3. - 10., (t[0][2] + t[1][2] + t[2][2]) / 3. - 30.];
                assert!(n[0] * c[0] + n[2] * c[1] > 0. || n[1] > 0., "{id}: inward face");
            }
        }
    }

    fn object(id: &str, asset_id: &str) -> GeneratedObject {
        GeneratedObject { id: id.into(), asset_id: asset_id.into(), position: [1000, 200, -3000], quarter_turns: 1, yaw_offset_mdeg: 15000 }
    }

    /// The far street trees mirror the near renderer's constants
    /// (street_planting_meshes.gd) and core's crown table.
    #[test]
    fn far_street_trees_match_the_near_constants() {
        let near = include_str!("../../../godot/street_planting_meshes.gd");
        let line = |name: &str| near.lines().find(|l| l.starts_with(&format!("const {name} :=")))
            .unwrap_or_else(|| panic!("{name} in street_planting_meshes.gd")).to_owned();
        let crowns = line("CROWN_CM");
        let forms = format!("{}{}", line("FORMS"), near.lines().skip_while(|l| !l.starts_with("const FORMS :=")).nth(1).unwrap());
        for (i, (id, reach_cm)) in mapkit_core::STREET_TREES.iter().enumerate() {
            let species = id.trim_start_matches("builtin:street_tree_");
            assert!(crowns.contains(&format!("\"{species}\": {reach_cm}")), "{species} crown reach");
            let [base, height, radius] = STREET_TREES[i];
            assert!(forms.contains(&format!("\"{species}\": Vector3({base:.2}, {height:.2},")), "{species} form {base} {height}: {forms}");
            // The silhouette stays inside the crown reach at the largest scale.
            assert!(radius * STREET_TREE_SCALE[1] <= *reach_cm as f32 / 100. && radius * STREET_TREE_SCALE[1] >= *reach_cm as f32 / 100. * 0.8, "{species} silhouette");
            assert!(mapkit_core::distant_tree(id) && mapkit_core::street_tree_crown_cm(id) == Some(*reach_cm));
        }
        assert!(near.contains("const SCALE := Vector2(0.92, 1.08)") && near.contains("const YOUNG_SHARE := 0.12") && near.contains("const YOUNG_CROWN := 0.65"));
        assert!(near.contains("const FAR_SIDE := \"aaa69e\"") && near.contains("const FAR_TOP := \"567046\""));
        assert_eq!((BED_SIDE, BED_TOP), ([0xaa, 0xa6, 0x9e, 255], [0x56, 0x70, 0x46, 255]));
        assert!(!mapkit_core::distant_tree("builtin:median_bed_pine") && !mapkit_core::distant_tree("builtin:planter"));
    }

    #[test]
    fn street_tree_is_a_trunk_frustum_and_cone_inside_its_crown_reach() {
        let mut young = 0;
        for i in 0..200 {
            for (species, (id, reach_cm)) in mapkit_core::STREET_TREES.iter().enumerate() {
                let tree = object(&format!("tree-{i}"), id);
                let mut mesh = DistantMesh::default();
                add_street_tree(&mut mesh, &tree, species);
                assert_eq!(mesh.vertices.len() as u64, CITY_TREE_TRIANGLES * 3);
                assert!(mesh.decoration.iter().all(|&d| d == 0) && mesh.light_data.iter().all(|l| *l == [0., 0.]));
                let ([base, height, _], scale, _) = street_tree_shape(&tree, species, false);
                if species == 0 && scale == 1. { young += 1; }
                let top = mesh.vertices.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                assert!((top - (2. + (base + height) * scale)).abs() < 1e-4);
                let reach = mesh.vertices.iter().map(|p| (p[0] - 10.).hypot(p[2] - 30.)).fold(0f32, f32::max);
                assert!(reach <= *reach_cm as f32 / 100. + 1e-4, "{id}: {reach}");
                // Above 30 cm of the axis every vertex is 1.2 m up or more.
                assert!(mesh.vertices.iter().all(|p| (p[0] - 10.).hypot(p[2] - 30.) <= 0.3 || p[1] - 2. >= 1.2 - 1e-4));
                for t in mesh.vertices.chunks(3) {
                    let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
                    let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
                    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    let c = [(t[0][0] + t[1][0] + t[2][0]) / 3. - 10., (t[0][2] + t[1][2] + t[2][2]) / 3. - 30.];
                    assert!(n[0] * c[0] + n[2] * c[1] > 0. || n[1] > 0., "{id}: inward face");
                }
            }
        }
        assert!((10..=45).contains(&young), "about 12 % young replacements: {young}");
        let mut pine = DistantMesh::default();
        add_median_pine(&mut pine, &object("median", "builtin:median_bed_pine"));
        assert_eq!(pine.vertices.len() as u64, MEDIAN_PINE_TRIANGLES * 3);
        assert!(pine.vertices.iter().all(|p| p[1] - 2. >= 1.35 * 0.92 - 1e-4));
    }

    #[test]
    fn bed_proxy_faces_take_granite_sides_and_a_planted_top() {
        let bed = |asset: &str| mapkit_core::Placement { id: "bed".into(), asset_id: asset.into(), position: [0, 100, 0], quarter_turns: 0, yaw_offset_mdeg: 0 };
        let face = |a: [i64; 3], b: [i64; 3], c: [i64; 3]| Triangle { contact_class: 0, snow_retention_percent: 0, vertices: [a, b, c], surface: Surface::Concrete, object_id: "bed".into(), spawnable: false };
        let top = face([0, 108, 0], [100, 108, 0], [0, 108, 100]);
        let side = face([0, 100, 0], [100, 100, 0], [0, 108, 0]);
        let trunk = face([0, 100, 0], [10, 100, 0], [0, 240, 0]);
        assert_eq!(planting_color(&bed("builtin:planter"), &top), Some(BED_TOP));
        assert_eq!(planting_color(&bed("builtin:median_bed"), &side), Some(BED_SIDE));
        assert_eq!(planting_color(&bed("builtin:median_bed_pine"), &trunk), Some(STREET_TREE_COLORS[4][0]));
        assert_eq!(planting_color(&bed("builtin:street_tree_zelkova"), &side), Some(STREET_TREE_COLORS[1][0]), "a neighbour cell's clipped trunk sliver");
        assert_eq!(planting_color(&bed("builtin:bench"), &top), None);
    }

    #[test]
    fn proxy_keeps_visual_height_and_scene_rotation() {
        let mut mesh = DistantMesh::default();
        add_box(
            &mut mesh,
            &Proxy {
                min: [1., 2., 3.],
                max: [2., 5., 4.],
                color: [1, 2, 3, 255], role:0,
            },
            [1000, 2000, 3000],
            1, 0, 0.,
        );
        assert_eq!(mesh.vertices.len(), 36);
        assert!(mesh.vertices.iter().all(|p| (13. ..=14.).contains(&p[0])
            && (22. ..=25.).contains(&p[1])
            && (-32. ..=-31.).contains(&p[2])));
        assert!(mesh.colors.iter().all(|c| *c == [1, 2, 3, 255]));
    }
}

/// Stable night-window seed shared by near instances and distant proxies.
fn object_seed(map_id: &str, object_id: &str) -> f32 {
    let hash = mapkit_core::sha256(format!("{}/{}", map_id, object_id).as_bytes());
    u32::from_str_radix(&hash[..6], 16).unwrap() as f32 / 16777215.
}

/// Record palettes shared with the near facade shader (`godot/facade_palette.json`).
#[derive(serde::Deserialize)]
struct FacadePalette {
    body: BTreeMap<String, Vec<[u8; 3]>>,
    roof: BTreeMap<String, Vec<[u8; 3]>>,
}

fn facade_palette() -> &'static FacadePalette {
    static PALETTE: std::sync::OnceLock<FacadePalette> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| serde_json::from_str(include_str!("../../../godot/facade_palette.json")).expect("valid facade palette"))
}

/// The 12-bit record (site and category) seed of a building, shared with the
/// packed facade UV2 of near cells (mapkit-godot packed.rs).
pub fn facade_record(object_id: &str) -> u32 {
    u32::from_str_radix(&mapkit_core::sha256(object_id.as_bytes())[..3], 16).unwrap()
}

/// A record's generated wall (body: bits 0..2) or roof (bits 3..5) colour, the
/// same entries the near facade shader picks.
pub fn facade_color(material: &str, usage: &str, record: u32, roof: bool) -> [u8; 4] {
    let palette = facade_palette();
    let entries = if roof {
        palette.roof.get(if material == "brick" || material == "wood" { material } else { "concrete" })
    } else if material == "brick" || material == "wood" {
        palette.body.get(material)
    } else {
        palette.body.get(&format!("concrete:{}", if ["commercial", "industrial", "public"].contains(&usage) { usage } else { "residential" }))
    };
    let entries = entries.expect("palette entry");
    let [r, g, b] = entries[(if roof { record >> 3 } else { record } & 7) as usize];
    [r, g, b, 255]
}

/// The twelve generated wall colours of `chunk_renderer.material_color`:
/// material base blended towards the use tint (concrete 0.6, brick/wood 0.3).
pub fn building_color(material: &str, usage: &str) -> [u8; 4] {
    let base: [f32; 3] = match material {
        "brick" => [180., 108., 80.],
        "wood" => [153., 115., 71.],
        _ => [183., 184., 176.],
    };
    let tint: [f32; 3] = match usage {
        "commercial" => [159., 180., 196.],
        "industrial" => [139., 143., 147.],
        "public" => [217., 200., 154.],
        _ => [232., 214., 189.],
    };
    let weight = if material == "concrete" { 0.6 } else { 0.3 };
    let mix = |i: usize| ((base[i] / 255. + (tint[i] / 255. - base[i] / 255.) * weight) * 255. + 0.5).floor() as u8;
    [mix(0), mix(1), mix(2), 255]
}
