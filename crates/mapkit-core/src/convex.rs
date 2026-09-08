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
            for _ in 0..p.quarter_turns {
                *v = [-v[2], v[1], v[0]];
            }
            for a in 0..3 {
                v[a] += p.position[a];
            }
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
