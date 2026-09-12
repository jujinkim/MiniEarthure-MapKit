//! Immutable, validated source shared by package queries and cell generation.
use crate::*;
use std::{ops::Deref, sync::RwLock};

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct PreparedMap {
    document: MapDocument,
    #[serde(skip)]
    costs: RwLock<BTreeMap<(Cell, usize), GenerationCost>>,
    #[serde(skip)]
    region: Option<CellRegion>,
}

impl PreparedMap {
    pub fn new(mut document: MapDocument) -> Result<Self> {
        document.normalize();
        document.validate()?;
        Ok(Self {
            document,
            costs: RwLock::new(BTreeMap::new()),
            region: None,
        })
    }
    pub fn new_region(mut document: MapDocument, region: CellRegion) -> Result<Self> {
        document = document.into_indexed_source()?;
        region.validate(&document)?;
        // Normalization only reorders checked records; it cannot invalidate
        // geometry or references and must follow untrusted-input work limits.
        document.normalize();
        Ok(Self { document, costs: RwLock::new(BTreeMap::new()), region: Some(region) })
    }
    /// Editing must create a new validated snapshot; no mutable source alias exists.
    pub fn to_document(&self) -> MapDocument {
        self.document.clone()
    }
    pub fn estimate(&self, cell: Cell, max_triangles: usize) -> Result<GenerationCost> {
        if self.region.is_some_and(|r| !r.contains(cell)) {
            return Err(error("E_CELL", "cell outside prepared source region"));
        }
        self.document.cell_bounds(cell)?;
        let limit = max_triangles.min(2_000_000);
        if let Some(cost) = self.costs.read().unwrap().get(&(cell, limit)) {
            return Ok(cost.clone());
        }
        let cost = crate::cost::estimate_validated(&self.document, cell, limit)?;
        // Bound metadata independently of caller-selected limits. No generated geometry retained.
        let mut costs = self.costs.write().unwrap();
        if costs.len() < 16_384 {
            costs.insert((cell, limit), cost.clone());
        }
        Ok(cost)
    }
    pub fn generate(
        &self,
        cell: Cell,
        grid: Option<&HeightGrid>,
        limit: usize,
    ) -> Result<GeneratedChunk> {
        self.generate_with_occupancy(cell, grid, limit, None)
            .map(|v| v.chunk)
    }
    pub fn generate_with_occupancy(
        &self,
        cell: Cell,
        grid: Option<&HeightGrid>,
        limit: usize,
        solids: Option<usize>,
    ) -> Result<GeneratedOccupancy> {
        if solids.is_some_and(|n| n > MAX_OCCUPIED_SOLIDS) {
            return Err(error("E_BUDGET", "occupancy limit exceeds 200000 solids"));
        }
        let cost = self.estimate(cell, limit)?;
        crate::generation::generate_prepared(
            GenerationInput {
                document: &self.document,
                cell,
                heightgrid: grid,
                max_triangles: limit,
            },
            solids,
            &cost,
        )
    }
}
impl Deref for PreparedMap {
    type Target = MapDocument;
    fn deref(&self) -> &MapDocument {
        &self.document
    }
}
impl From<PreparedMap> for MapDocument {
    fn from(prepared: PreparedMap) -> Self {
        prepared.document
    }
}
