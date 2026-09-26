//! Non-solid water volumes. Integer source geometry is shared by rendering,
//! spawn selection and the engine adapters; water never enters collision faces.
use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaterBody {
    pub id: String,
    pub polygon: Vec<Point>,
    #[serde(default)]
    pub islands: Vec<Vec<Point>>,
    pub surface_cm: i64,
    pub bottom_cm: i64,
    /// Map x / map y velocity, centimetres per second.
    pub flow_cm_s: [i32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct WaterCell {
    pub body: WaterBody,
    /// Clipped display-only triangles. Never append these to chunk.triangles.
    pub surface: Vec<[Vertex; 3]>,
}

impl WaterBody {
    pub fn contains_horizontal(&self, p: Point) -> bool {
        point_in_polygon(p, &self.polygon)
            && !self.islands.iter().any(|island| point_in_polygon(p, island))
    }
    pub fn contains(&self, p: Vertex) -> bool {
        p[1] >= self.bottom_cm && p[1] <= self.surface_cm
            && self.contains_horizontal([p[0], p[2]])
    }
    pub fn intersects(&self, area: &Bounds) -> bool {
        let b = crate::bounds_index::bounds(&self.polygon);
        (0..2).all(|a| b.max[a] >= area.min[a] && b.min[a] <= area.max[a])
    }
    pub fn vertices(&self) -> usize {
        self.polygon.len() + self.islands.iter().map(Vec::len).sum::<usize>()
    }
    /// Includes JSON/Variant copies, mesh arrays, query records and render nodes.
    pub fn memory_bytes(&self) -> u64 {
        32768 + self.vertices() as u64 * 16384
    }
}

pub fn sample(bodies: &[WaterBody], p: Vertex) -> Option<&WaterBody> {
    bodies.iter().filter(|b| b.contains(p))
        .max_by(|a,b| a.surface_cm.cmp(&b.surface_cm).then_with(|| b.id.cmp(&a.id)))
}

pub(crate) fn validate(d: &MapDocument) -> Result<()> { validate_bodies(&d.water_bodies, &d.bounds) }

pub(crate) fn validate_bodies(bodies: &[WaterBody], bounds: &Bounds) -> Result<()> {
    if bodies.len() > 1024 { return Err(error("E_LIMIT", "too many water bodies")); }
    let mut work = 0;
    for body in bodies {
        crate::cancellation::checkpoint()?;
        if body.vertices() > 512 || body.islands.len() > 16
            || !polygon_valid(&body.polygon, bounds)
            || body.bottom_cm >= body.surface_cm
            || body.bottom_cm.unsigned_abs() > 1_000_000
            || body.surface_cm.unsigned_abs() > 1_000_000
            || body.flow_cm_s.iter().any(|v| v.unsigned_abs() > 1000) {
            return Err(error("E_WATER", format!("invalid water volume: {}", body.id)));
        }
        for (i, ring) in body.islands.iter().enumerate() {
            if !polygon_valid(ring, bounds) || !point_in_polygon(ring[0], &body.polygon) {
                return Err(error("E_WATER", format!("invalid island: {}", body.id)));
            }
            for other in std::iter::once(&body.polygon).chain(body.islands[..i].iter()) {
                for a in 0..ring.len() { for b in 0..other.len() {
                    crate::placement::tick(&mut work, 1)?;
                    if intersects(ring[a], ring[(a+1)%ring.len()], other[b], other[(b+1)%other.len()]) {
                        return Err(error("E_WATER", "island boundaries intersect"));
                    }
                }}
            }
            if body.islands[..i].iter().any(|other| point_in_polygon(ring[0], other) || point_in_polygon(other[0], ring)) {
                return Err(error("E_WATER", "islands overlap or nest"));
            }
        }
    }
    Ok(())
}

/// Bounding-volume index used for cell/region selection. Exact polygon tests
/// are applied to every physical query, including island and shoreline edges.
pub(crate) fn candidates(d: &MapDocument, area: &Bounds) -> Result<Vec<usize>> {
    let bounds: Vec<_> = d.water_bodies.iter().map(|b| crate::bounds_index::bounds(&b.polygon)).collect();
    Ok(crate::bounds_index::BoundsIndex::new(&bounds).query(area, &mut 0)?
        .into_iter().filter(|&i| d.water_bodies[i].intersects(area)).collect())
}

pub(crate) fn generate(d: &MapDocument, area: &Bounds) -> Result<Vec<WaterCell>> {
    candidates(d, area)?.into_iter().map(|i| {
        let body = &d.water_bodies[i];
        let faces = crate::courtyard::triangulate_rings(&body.polygon, &body.islands, &mut 0)?;
        let surface = crate::generation::water_surface(area, body.surface_cm, &faces)?;
        Ok(WaterCell { body: body.clone(), surface })
    }).collect()
}
