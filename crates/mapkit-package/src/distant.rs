//! Display-only, deterministic distant geometry. No collision/archive contract changes.
use crate::Package;
use mapkit_core::{Cell, GeneratedChunk, Result, Surface};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct DistantMesh {
    pub vertices: Vec<[f32; 3]>,
    pub colors: Vec<[u8; 4]>,
    pub light_data: Vec<[f32; 2]>,
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
                    primitive
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_factor()
                        .map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
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
    /// Conservative anchor-to-visual horizontal extent, independent of collision proxies.
    pub fn visual_margin_cm(&self) -> Result<i64> {
        let mut radius = 1.84_f32;
        for asset in &self.document.assets {
            if !asset.path.ends_with(".glb") { continue; }
            for proxy in proxies(&self.files[&asset.path], None, None)? {
                let x = proxy.min[0].abs().max(proxy.max[0].abs());
                let z = proxy.min[2].abs().max(proxy.max[2].abs());
                radius = radius.max(x.hypot(z));
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
            let glb = gltf::Gltf::from_slice(&self.files[&asset.path])
                .map_err(|_| mapkit_core::error("E_ASSET", "invalid distant asset"))?;
            let count: usize = glb
                .nodes()
                .filter_map(|n| n.mesh())
                .map(|m| m.primitives().count())
                .sum();
            maximum = maximum.max(count as u64 * 12);
        }
        Ok(cost
            .triangles
            .saturating_add(cost.objects.saturating_mul(maximum)))
    }

    pub fn generate_distant(&self, cell: Cell) -> Result<DistantMesh> {
        let chunk = self.generate(cell, 500_000)?;
        self.distant_from_generated(&chunk)
    }

    pub fn distant_from_generated(&self, chunk: &GeneratedChunk) -> Result<DistantMesh> {
        let mut result = DistantMesh::default();
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
                    Surface::Grass => [115, 134, 100, 255],
                }
            };
            for i in [0, 2, 1] {
                let p = triangle.vertices[i];
                result
                    .vertices
                    .push([p[0] as f32 * 0.01, p[1] as f32 * 0.01, -p[2] as f32 * 0.01]);
                result.colors.push(color);
                result.light_data.push([0.,0.]);
            }
        }
        let mut templates = BTreeMap::new();
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
                    object.quarter_turns, 0.,
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
            let hash = mapkit_core::sha256(format!("{}/{}",self.document.map_id,object.id).as_bytes());
            let seed = u32::from_str_radix(&hash[..6],16).unwrap() as f32 / 16777215.;
            for proxy in &templates[&asset.id] {
                add_box(&mut result, proxy, object.position, object.quarter_turns, seed);
            }
        }
        Ok(result)
    }
}

fn add_box(output: &mut DistantMesh, proxy: &Proxy, position: [i64; 3], quarter_turns: u8, seed: f32) {
    let points: [[f32; 3]; 8] = std::array::from_fn(|i| {
        let mut p = std::array::from_fn(|a| {
            if i & (1 << a) == 0 {
                proxy.min[a]
            } else {
                proxy.max[a]
            }
        });
        for _ in 0..quarter_turns {
            p = [p[2], p[1], -p[0]];
        }
        [
            p[0] + position[0] as f32 * 0.01,
            p[1] + position[1] as f32 * 0.01,
            p[2] - position[2] as f32 * 0.01,
        ]
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
            1, 0.,
        );
        assert_eq!(mesh.vertices.len(), 36);
        assert!(mesh.vertices.iter().all(|p| (13. ..=14.).contains(&p[0])
            && (22. ..=25.).contains(&p[1])
            && (-32. ..=-31.).contains(&p[2])));
        assert!(mesh.colors.iter().all(|c| *c == [1, 2, 3, 255]));
    }
}
