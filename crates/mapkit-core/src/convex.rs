//! Explicit, bounded convex proxy authoring. No hull solver or floating point.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollisionConvex {
    pub vertices: Vec<Vertex>,
    /// Outward triangular faces in the document's (x,height,y) axes.
    pub faces: Vec<[u8; 3]>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct GeneratedConvex {
    pub object_id: String,
    pub shape: CollisionConvex,
}
impl CollisionConvex {
    pub fn planes(&self) -> impl Iterator<Item = ([i128; 3], Vertex)> + '_ {
        self.faces.iter().map(|f| {
            let [a, b, c] = f.map(|i| self.vertices[i as usize]);
            let u: [i128; 3] = std::array::from_fn(|i| i128::from(b[i]) - i128::from(a[i]));
            let v: [i128; 3] = std::array::from_fn(|i| i128::from(c[i]) - i128::from(a[i]));
            (
                [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ],
                a,
            )
        })
    }
    pub fn valid(&self, coordinate_limit: u64) -> bool {
        if !(4..=32).contains(&self.vertices.len())
            || !(4..=60).contains(&self.faces.len())
            || self
                .vertices
                .iter()
                .flatten()
                .any(|v| v.unsigned_abs() > coordinate_limit)
            || self.vertices.iter().collect::<BTreeSet<_>>().len() != self.vertices.len()
            || self
                .faces
                .iter()
                .flatten()
                .any(|i| *i as usize >= self.vertices.len())
        {
            return false;
        }
        let mut edges = BTreeMap::<(u8, u8), u8>::new();
        let mut used = BTreeSet::new();
        let mut faces = BTreeSet::new();
        for f in &self.faces {
            let mut key = *f;
            key.sort();
            if key[0] == key[1] || key[1] == key[2] || !faces.insert(key) {
                return false;
            }
            for i in 0..3 {
                used.insert(f[i]);
                *edges.entry((f[i], f[(i + 1) % 3])).or_default() += 1;
            }
        }
        if used.len() != self.vertices.len()
            || edges
                .iter()
                .any(|(&(a, b), &n)| n != 1 || edges.get(&(b, a)) != Some(&1))
            || self.vertices.len() + self.faces.len() != edges.len() / 2 + 2
        {
            return false;
        }
        for (normal, point) in self.planes() {
            if normal == [0; 3] {
                return false;
            }
            let mut interior = false;
            for vertex in &self.vertices {
                let side: i128 = (0..3)
                    .map(|a| normal[a] * (i128::from(vertex[a]) - i128::from(point[a])))
                    .sum();
                if side > 0 {
                    return false;
                }
                interior |= side < 0;
            }
            if !interior {
                return false;
            }
        }
        true
    }
    pub fn placed(&self, p: &Placement) -> Self {
        let mut shape = self.clone();
        for v in &mut shape.vertices {
            *v = p.transform_point(*v);
        }
        shape
    }
    pub fn bounds(&self) -> Bounds {
        Bounds {
            min: std::array::from_fn(|a| self.vertices.iter().map(|v| v[a * 2]).min().unwrap()),
            max: std::array::from_fn(|a| self.vertices.iter().map(|v| v[a * 2]).max().unwrap()),
        }
    }
}

impl Placement {
    /// Source axes are (x,height,map-y); scene axes reflect map-y.
    pub fn transform_point(&self, mut v: Vertex) -> Vertex {
        for _ in 0..self.quarter_turns { v = [-v[2], v[1], v[0]]; }
        if self.yaw_offset_mdeg != 0 {
            let angle = f64::from(self.yaw_offset_mdeg).to_radians() / 1000.0;
            let (s,c) = (libm::sin(angle),libm::cos(angle));
            v = [libm::round(v[0] as f64*c-v[2] as f64*s) as i64,v[1],
                 libm::round(v[0] as f64*s+v[2] as f64*c) as i64];
        }
        std::array::from_fn(|a| v[a]+self.position[a])
    }
}
impl CollisionBox {
    pub fn placed(&self, placement: &Placement) -> CollisionConvex {
        let mut proxy = self.clone();
        let mut pose = placement.clone();
        if pose.yaw_offset_mdeg == 0 {
            // Keep the existing cardinal convention for odd-width boxes.
            for _ in 0..pose.quarter_turns { proxy.center=[-proxy.center[2],proxy.center[1],proxy.center[0]]; proxy.size_cm.swap(0,2); }
            pose.quarter_turns=0;
        }
        let min: Vertex = std::array::from_fn(|a| proxy.center[a]-i64::from(proxy.size_cm[a]/2));
        let vertices: Vec<Vertex> = (0..8).map(|i| pose.transform_point(
            std::array::from_fn(|a| min[a]+if i & (1<<a) != 0 {i64::from(proxy.size_cm[a])} else {0})
        )).collect();
        CollisionConvex { vertices, faces: vec![
            [0,4,6],[0,6,2],[1,3,7],[1,7,5],
            [0,1,5],[0,5,4],[2,6,7],[2,7,3],
            [0,2,3],[0,3,1],[4,5,7],[4,7,6]] }
    }
}
