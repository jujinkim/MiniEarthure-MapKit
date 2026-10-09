//! Display-only, deterministic distant geometry. No collision/archive contract changes.
use crate::Package;
use mapkit_core::{Cell, GeneratedChunk, Result, Surface};
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

#[derive(Clone, Debug)]
struct Proxy {
    min: [f32; 3],
    max: [f32; 3],
    color: [u8; 4],
    role: u8,
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

impl Package {
    /// Parsing workspace and retained per-asset silhouette templates. This is
    /// generation scratch, never a near-render cache or collision allocation.
    pub fn distant_workspace_bytes(&self, cell: Cell) -> Result<u64> {
        let mut ids: BTreeSet<_> = self.document.authored_placement_candidates(cell)?
            .into_iter().map(|p| p.asset_id.as_str()).collect();
        ids.extend(self.document.zones.iter().filter_map(|z| z.tree.as_ref().map(|t| t.asset_id.as_str())));
        Ok(self.document.assets.iter().filter(|a| ids.contains(a.id.as_str()))
            .filter_map(|a| a.distant_path.as_ref())
            .map(|path| crate::assets::presentation_cost(path, &self.files[path])).sum())
    }
    /// Conservative anchor-to-visual horizontal extent, independent of collision proxies.
    pub fn visual_margin_cm(&self) -> Result<i64> {
        let mut radius = 1.84_f32;
        for asset in &self.document.assets {
            if !asset.path.ends_with(".glb") { continue; }
            for path in asset.paths() {
              for proxy in proxies(&self.files[path], None, None)? {
                let x = proxy.min[0].abs().max(proxy.max[0].abs());
                let z = proxy.min[2].abs().max(proxy.max[2].abs());
                radius = radius.max(x.hypot(z));
              }
            }
        }
        Ok((radius * 100.0).ceil() as i64)
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
        let chunk = self.generate(cell, 500_000)?;
        self.distant_from_generated(&chunk)
    }

    pub fn distant_from_generated(&self, chunk: &GeneratedChunk) -> Result<DistantMesh> {
        let mut result = DistantMesh::default();
        // Keep all small props contiguous: at most one additional display batch,
        // covered by the existing fixed display allowance, not one per object.
        let mut decorations = DistantMesh::default();
        let mut omitted = BTreeSet::new();
        for placement in &self.document.placements {
            if self
                .document
                .assets
                .iter()
                .any(|a| a.id == placement.asset_id && a.path.ends_with(".glb"))
            {
                omitted.insert(placement.id.as_str());
            }
        }
        for object in &chunk.objects {
            if object.asset_id == "builtin:tree"
                || self
                    .document
                    .assets
                    .iter()
                    .any(|a| a.id == object.asset_id && a.path.ends_with(".glb"))
            {
                omitted.insert(object.id.as_str());
            }
        }
        for triangle in &chunk.triangles {
            if omitted.contains(triangle.object_id.as_str()) {
                continue;
            }
            let color = if let Some(building) = self
                .document
                .buildings
                .iter()
                .find(|b| b.id == triangle.object_id)
            {
                match building.material.as_str() {
                    "brick" => [180, 108, 80, 255],
                    "wood" => [153, 115, 71, 255],
                    _ => [183, 184, 176, 255],
                }
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
                // Negative roles are display tags, never emissive bindings.
                result.light_data.push([0., if matches!(triangle.surface, Surface::Grass | Surface::Dirt) { -1. } else { 0. }]);
                result.decoration.push(0);
            }
        }
        // Distant water keeps the same clipped shoreline and elevation. This
        // tagged surface uses the common water material and never enters
        // collision/physical triangles. Flow uses the existing two UV channels.
        for water in &chunk.water_bodies {
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
        for object in &chunk.objects {
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
            let Some(asset) = self
                .document
                .assets
                .iter()
                .find(|a| a.id == object.asset_id && a.path.ends_with(".glb"))
            else {
                continue;
            };
            let hash = mapkit_core::sha256(format!("{}/{}",self.document.map_id,object.id).as_bytes());
            let seed = u32::from_str_radix(&hash[..6],16).unwrap() as f32 / 16777215.;
            if let Some(path) = &asset.distant_path {
                if !silhouettes.contains_key(&asset.id) {
                    silhouettes.insert(&asset.id, silhouette(&self.files[path],
                        asset.material.as_ref().map(|m| m.albedo_rgba),
                        self.document.environment.as_ref().and_then(|e| e.lights.iter().find(|b| b.asset_id==asset.id)))?);
                }
                let mesh = &silhouettes[&asset.id];
                let mut low = [f32::INFINITY;3];let mut high = [f32::NEG_INFINITY;3];
                for p in &mesh.vertices { for a in 0..3 { low[a]=low[a].min(p[a]);high[a]=high[a].max(p[a]); } }
                let small = (0..3).map(|a| (high[a]-low[a]).powi(2)).sum::<f32>() <= 36.;
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
