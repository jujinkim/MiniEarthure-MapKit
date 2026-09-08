use crate::{error, Bounds, Cell, MapDocument, Result};
use serde::{Deserialize, Serialize};

// Shared with generation: centre-owned trunks can extend into another cell.
pub(crate) const TREE_PROXY_SIZE_CM: [u32; 3] = [40, 400, 40];

/// Broad-phase requirements only; no generation, allocation reservation or readiness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryCells {
    pub geometry_cells: Vec<Cell>,
    /// Includes geometry cells and neighbouring owners of protruding tree proxies.
    pub occupancy_cells: Vec<Cell>,
}

impl MapDocument {
    /// Closed rectangle query, in local centimetres. Both sides of an exact cell
    /// seam are included. Results are unique, row-major and clipped to this map.
    /// The cap applies to the union, before either result vector is allocated.
    pub fn query_cells(&self, query: &Bounds, max_cells: usize) -> Result<QueryCells> {
        let size = i64::from(self.cell_size_cm);
        let dimensions = self.cell_dimensions()?;
        if (0..2).any(|a| {
            query.min[a].unsigned_abs() > 1_000_000_000
                || query.max[a].unsigned_abs() > 1_000_000_000
                || query.min[a] > query.max[a]
        }) {
            return Err(error("E_QUERY", "invalid query bounds"));
        }
        if !(1..=16_384).contains(&max_cells) {
            return Err(error("E_BUDGET", "invalid query cell allowance"));
        }
        let halo = [TREE_PROXY_SIZE_CM[0] / 2, TREE_PROXY_SIZE_CM[2] / 2];
        let expanded = Bounds {
            min: std::array::from_fn(|a| query.min[a] - i64::from(halo[a])),
            max: std::array::from_fn(|a| query.max[a] + i64::from(halo[a])),
        };
        let range = |bounds: &Bounds| -> Option<([i64; 2], [i64; 2])> {
            if (0..2)
                .any(|a| bounds.max[a] < self.bounds.min[a] || bounds.min[a] > self.bounds.max[a])
            {
                return None;
            }
            let lo = std::array::from_fn(|a| {
                // Subtract one centimetre only for the inclusive lower seam.
                ((bounds.min[a].max(self.bounds.min[a]) - self.bounds.min[a] - 1).div_euclid(size))
                    .clamp(0, dimensions[a] - 1)
            });
            let hi = std::array::from_fn(|a| {
                ((bounds.max[a].min(self.bounds.max[a]) - self.bounds.min[a]) / size)
                    .clamp(0, dimensions[a] - 1)
            });
            Some((lo, hi))
        };
        let owners = range(&expanded);
        let count = owners.map_or(0, |(lo, hi)| (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1));
        if count > max_cells as i64 {
            return Err(error("E_BUDGET", "query exceeds cell allowance"));
        }
        let collect = |range: Option<([i64; 2], [i64; 2])>| {
            range.map_or_else(Vec::new, |(lo, hi)| {
                (lo[1]..=hi[1])
                    .flat_map(|y| {
                        (lo[0]..=hi[0]).map(move |x| Cell {
                            x: x as i32,
                            y: y as i32,
                        })
                    })
                    .collect()
            })
        };
        Ok(QueryCells {
            geometry_cells: collect(range(query)),
            occupancy_cells: collect(owners),
        })
    }
}
