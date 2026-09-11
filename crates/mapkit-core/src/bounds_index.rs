//! Transient deterministic broad phase. Exact predicates remain authoritative.
use crate::*;
struct Node {
    bounds: Bounds,
    children: Option<(usize, usize)>,
    items: Vec<usize>,
}
pub(crate) struct BoundsIndex {
    nodes: Vec<Node>,
}
pub(crate) fn bounds(points: &[Point]) -> Bounds {
    Bounds {
        min: std::array::from_fn(|i| points.iter().map(|p| p[i]).min().unwrap()),
        max: std::array::from_fn(|i| points.iter().map(|p| p[i]).max().unwrap()),
    }
}
impl BoundsIndex {
    pub fn new(areas: &[Bounds]) -> Self {
        let mut result = Self { nodes: vec![] };
        if !areas.is_empty() {
            result.build(areas, (0..areas.len()).collect());
        }
        result
    }
    fn build(&mut self, areas: &[Bounds], mut items: Vec<usize>) -> usize {
        let bounds = Bounds {
            min: std::array::from_fn(|a| items.iter().map(|&i| areas[i].min[a]).min().unwrap()),
            max: std::array::from_fn(|a| items.iter().map(|&i| areas[i].max[a]).max().unwrap()),
        };
        let axis = usize::from(bounds.max[1] - bounds.min[1] > bounds.max[0] - bounds.min[0]);
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            children: None,
            items: vec![],
        });
        if items.len() <= 8 {
            self.nodes[index].items = items;
        } else {
            items.sort_by_key(|&i| (areas[i].min[axis] + areas[i].max[axis], i));
            let right = items.split_off(items.len() / 2);
            let a = self.build(areas, items);
            let b = self.build(areas, right);
            self.nodes[index].children = Some((a, b));
        }
        index
    }
    pub fn query(&self, area: &Bounds, work: &mut usize) -> Result<Vec<usize>> {
        if self.nodes.is_empty() {
            return Ok(vec![]);
        }
        let mut pending = vec![0];
        let mut out = vec![];
        while let Some(i) = pending.pop() {
            crate::placement::tick(work, 1)?;
            let node = &self.nodes[i];
            if (0..2).any(|a| area.max[a] < node.bounds.min[a] || area.min[a] > node.bounds.max[a])
            {
                continue;
            }
            if let Some((a, b)) = node.children {
                pending.extend([a, b]);
            } else {
                out.extend(&node.items);
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_never_loses_overlapping_bounds_and_keeps_source_order() {
        let boxes: Vec<_> = (0..200)
            .map(|i| Bounds {
                min: [(i * 97) % 500, (i * 211) % 700],
                max: [(i * 97) % 500 + 40, (i * 211) % 700 + 80],
            })
            .collect();
        let index = BoundsIndex::new(&boxes);
        for x in (-50..600).step_by(17) {
            let area = Bounds {
                min: [x, x],
                max: [x + 53, x + 81],
            };
            let hits = index.query(&area, &mut 0).unwrap();
            assert!(hits.windows(2).all(|p| p[0] < p[1]));
            for (i, b) in boxes.iter().enumerate() {
                if (0..2).all(|a| area.max[a] >= b.min[a] && area.min[a] <= b.max[a]) {
                    assert!(hits.contains(&i));
                }
            }
        }
        assert!(BoundsIndex::new(&[])
            .query(&boxes[0], &mut 0)
            .unwrap()
            .is_empty());
    }
}
