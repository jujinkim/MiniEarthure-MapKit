//! Engine-, filesystem-, network- and clock-independent map domain and generation.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PACKAGE_VERSION: u32 = 1;
pub const RECIPE_VERSION: u32 = 1;
pub const GENERATED_VERSION: u32 = 6;
pub const WORLD_SCALE: f64 = 0.125;
pub const DEFAULT_CELL_CM: i64 = 51_200;
pub type Point = [i64; 2];
pub type Vertex = [i64; 3]; // x, height, local y, centimetres
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub fn error(code: &str, message: impl Into<String>) -> Error {
    Error {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn contains(&self, p: Point) -> bool {
        (0..2).all(|a| p[a] >= self.min[a] && p[a] <= self.max[a])
    }
}
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, JsonSchema,
)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub tool_id: String,
    pub version: String,
    pub build_id: String,
    pub fingerprint: String,
    pub first_created: String,
    pub last_edited: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub source: String,
    pub license: String,
    pub notice: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Asphalt,
    Concrete,
    Dirt,
    Gravel,
    Grass,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoadKind {
    Ground,
    Elevated,
    Bridge,
    Underpass,
    Tunnel,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoadNode {
    pub id: String,
    pub position: Vertex,
    pub level: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Road {
    pub id: String,
    pub from: String,
    pub to: String,
    pub points: Vec<Vertex>,
    /// One width/surface per centreline segment.
    pub widths_cm: Vec<u32>,
    pub surfaces: Vec<Surface>,
    pub kind: RoadKind,
    /// Required for tunnel/underpass; portals connect only explicit graph nodes.
    pub clearance_cm: Option<u32>,
    pub sidewalk_cm: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Building {
    pub id: String,
    pub footprint: Vec<Point>,
    pub base_cm: i64,
    pub height_cm: u32,
    pub usage: String,
    pub material: String,
    pub roof: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    Forest,
    Orchard,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub id: String,
    pub polygon: Vec<Point>,
    pub kind: ZoneKind,
    pub spacing_cm: u32,
    pub density_per_mille: u16,
    pub exclusions: Vec<Vec<Point>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Heightmap {
    pub cell: Cell,
    pub path: String,
    pub spacing_cm: u32,
    pub offset_cm: i64,
    pub step_cm: u32,
    pub source_accuracy_cm: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub path: String,
    pub attribution: Attribution,
    pub collision: Vec<CollisionBox>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollisionBox {
    pub center: Vertex,
    pub size_cm: [u32; 3],
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub id: String,
    pub asset_id: String,
    pub position: Vertex,
    pub quarter_turns: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapDocument {
    pub map_id: String,
    pub revision: u32,
    pub bounds: Bounds,
    pub cell_size_cm: u32,
    pub seed: u64,
    pub recipe_version: u32,
    pub theme: String,
    pub terrain_base_cm: i64,
    pub heightmaps: Vec<Heightmap>,
    pub nodes: Vec<RoadNode>,
    pub roads: Vec<Road>,
    pub buildings: Vec<Building>,
    pub zones: Vec<Zone>,
    pub assets: Vec<Asset>,
    pub placements: Vec<Placement>,
    pub attributions: Vec<Attribution>,
    pub provenance: Provenance,
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Sorted keys, integer-only numbers, UTF-8 and no insignificant whitespace.
pub fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let v = serde_json::to_value(value).map_err(|e| error("E_JSON", e.to_string()))?;
    serde_json::to_vec(&v).map_err(|e| error("E_JSON", e.to_string()))
}
pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && path.is_ascii()
        && !path.contains(['\\', ':'])
        && path.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_- .".contains(&c))
                && !s.ends_with(['.', ' '])
                && !matches!(
                    s.split('.').next().unwrap().to_ascii_uppercase().as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}
fn cross(a: Point, b: Point, c: Point) -> i128 {
    (b[0] - a[0]) as i128 * (c[1] - a[1]) as i128 - (b[1] - a[1]) as i128 * (c[0] - a[0]) as i128
}
fn on_segment(a: Point, b: Point, p: Point) -> bool {
    cross(a, b, p) == 0 && (0..2).all(|i| p[i] >= a[i].min(b[i]) && p[i] <= a[i].max(b[i]))
}
fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (ab_c, ab_d, cd_a, cd_b) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    (ab_c.signum() * ab_d.signum() < 0 && cd_a.signum() * cd_b.signum() < 0)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}
pub fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if on_segment(a, b, p) {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let c = cross(a, b, p);
            if (c > 0) == (b[1] > a[1]) {
                inside = !inside;
            }
        }
    }
    inside
}
fn polygon_valid(poly: &[Point], bounds: &Bounds) -> bool {
    if poly.len() < 3 || poly.len() > 512 || poly.iter().any(|p| !bounds.contains(*p)) {
        return false;
    }
    let mut area = 0i128;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if a == b {
            return false;
        }
        area += a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128;
        for j in i + 1..poly.len() {
            if j == i + 1 || (i == 0 && j == poly.len() - 1) {
                continue;
            }
            if intersects(a, b, poly[j], poly[(j + 1) % poly.len()]) {
                return false;
            }
        }
    }
    area != 0
}
impl MapDocument {
    pub fn normalize(&mut self) {
        self.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        self.roads.sort_by(|a, b| a.id.cmp(&b.id));
        self.buildings.sort_by(|a, b| a.id.cmp(&b.id));
        self.zones.sort_by(|a, b| a.id.cmp(&b.id));
        self.assets.sort_by(|a, b| a.id.cmp(&b.id));
        self.placements.sort_by(|a, b| a.id.cmp(&b.id));
        self.heightmaps.sort_by_key(|h| h.cell);
        self.attributions.sort_by(|a, b| {
            (&a.source, &a.license, &a.notice).cmp(&(&b.source, &b.license, &b.notice))
        });
    }
    pub fn validate(&self) -> Result<()> {
        let fail = |m: &str| Err(error("E_GEOMETRY", m));
        if self.recipe_version != RECIPE_VERSION {
            return Err(error("E_VERSION", "unsupported recipe"));
        }
        if self.seed > 9_007_199_254_740_991 {
            return Err(error(
                "E_DOCUMENT",
                "seed exceeds exact public JSON integer profile",
            ));
        }
        if self.map_id.is_empty() || self.map_id.len() > 128 || self.theme != "default" {
            return Err(error("E_DOCUMENT", "map ID or unsupported theme"));
        }
        if !(200..=102_400).contains(&self.cell_size_cm) || !self.cell_size_cm.is_multiple_of(200) {
            return fail("cell size must be 2..1024 metres, multiple of 2 metres");
        }
        if (0..2).any(|i| {
            self.bounds.min[i].unsigned_abs() > 10_000_000
                || self.bounds.max[i].unsigned_abs() > 10_000_000
                || self.bounds.min[i] >= self.bounds.max[i]
        }) || self.terrain_base_cm.unsigned_abs() > 1_000_000
        {
            return fail("bounds or height out of range");
        }
        if self.cells().len() > 16_384 {
            return Err(error("E_LIMIT", "too many cells"));
        }
        let count = self.nodes.len()
            + self.roads.len()
            + self.buildings.len()
            + self.zones.len()
            + self.assets.len()
            + self.placements.len();
        if count > 200_000 {
            return Err(error("E_LIMIT", "too many objects"));
        }
        let vertices = self.roads.iter().map(|r| r.points.len()).sum::<usize>()
            + self
                .buildings
                .iter()
                .map(|b| b.footprint.len())
                .sum::<usize>()
            + self
                .zones
                .iter()
                .map(|z| z.polygon.len() + z.exclusions.iter().map(Vec::len).sum::<usize>())
                .sum::<usize>();
        if vertices > 1_000_000 {
            return Err(error("E_LIMIT", "too many input vertices"));
        }
        let mut ids = BTreeSet::new();
        for id in self
            .nodes
            .iter()
            .map(|x| &x.id)
            .chain(self.roads.iter().map(|x| &x.id))
            .chain(self.buildings.iter().map(|x| &x.id))
            .chain(self.zones.iter().map(|x| &x.id))
            .chain(self.assets.iter().map(|x| &x.id))
            .chain(self.placements.iter().map(|x| &x.id))
        {
            if id.is_empty() || id.len() > 128 || !ids.insert(id) {
                return Err(error("E_ID", "invalid or duplicate object ID"));
            }
        }
        let nodes: BTreeMap<_, _> = self.nodes.iter().map(|n| (&n.id, n)).collect();
        for n in &self.nodes {
            if !self.bounds.contains([n.position[0], n.position[2]])
                || n.position[1].unsigned_abs() > 1_000_000
            {
                return fail("node outside bounds");
            }
        }
        for r in &self.roads {
            if r.points.len() < 2
                || r.points.len() > 65536
                || r.widths_cm.len() != r.points.len() - 1
                || r.surfaces.len() != r.widths_cm.len()
            {
                return fail("road segment arrays mismatch");
            }
            if r.points
                .iter()
                .any(|p| !self.bounds.contains([p[0], p[2]]) || p[1].unsigned_abs() > 1_000_000)
                || r.widths_cm.iter().any(|w| !(20..=10_000).contains(w))
            {
                return fail("road point or width out of range");
            }
            if r.points
                .windows(2)
                .any(|p| p[0][0] == p[1][0] && p[0][2] == p[1][2])
            {
                return fail("road has zero horizontal length");
            }
            if nodes.get(&r.from).map(|n| n.position) != r.points.first().copied()
                || nodes.get(&r.to).map(|n| n.position) != r.points.last().copied()
            {
                return fail("road endpoints must match explicit graph nodes");
            }
            if matches!(r.kind, RoadKind::Tunnel | RoadKind::Underpass)
                && !r.clearance_cm.is_some_and(|v| (200..=5000).contains(&v))
            {
                return fail("tunnel/underpass requires clearance");
            }
        }
        for b in &self.buildings {
            if !polygon_valid(&b.footprint, &self.bounds)
                || b.height_cm == 0
                || b.height_cm > 100_000
                || b.base_cm.unsigned_abs() > 1_000_000
            {
                return fail("invalid building");
            }
        }
        for z in &self.zones {
            if !polygon_valid(&z.polygon, &self.bounds)
                || z.exclusions.iter().any(|p| !polygon_valid(p, &self.bounds))
                || z.spacing_cm < 200
                || z.density_per_mille > 1000
            {
                return fail("invalid zone");
            }
        }
        let mut heights = BTreeSet::new();
        for h in &self.heightmaps {
            if !self.has_cell(h.cell)
                || !heights.insert(h.cell)
                || !safe_path(&h.path)
                || !h.path.ends_with(".png")
                || h.spacing_cm < 200
                || !self.cell_size_cm.is_multiple_of(h.spacing_cm)
                || h.step_cm == 0
                || h.step_cm > 100
                || h.offset_cm.unsigned_abs() > 1_000_000
            {
                return fail("invalid heightmap descriptor");
            }
        }
        let assets: BTreeSet<_> = self.assets.iter().map(|a| &a.id).collect();
        for a in &self.assets {
            if !safe_path(&a.path)
                || ![".glb", ".png", ".webp"]
                    .iter()
                    .any(|e| a.path.ends_with(e))
                || a.attribution.license.is_empty()
                || a.collision.iter().any(|b| {
                    b.size_cm.contains(&0)
                        || b.size_cm.iter().any(|v| *v > 100_000)
                        || b.center.iter().any(|v| v.unsigned_abs() > 1_000_000)
                })
            {
                return Err(error("E_ASSET", "invalid asset or collision proxy"));
            }
        }
        for p in &self.placements {
            if !assets.contains(&p.asset_id)
                || p.quarter_turns > 3
                || !self.bounds.contains([p.position[0], p.position[2]])
                || p.position[1].unsigned_abs() > 1_000_000
            {
                return fail("invalid placement");
            }
        }
        Ok(())
    }
    pub fn cells(&self) -> Vec<Cell> {
        let size = self.cell_size_cm as i64;
        if size == 0 {
            return vec![];
        }
        let (nx, ny) = (
            (self.bounds.max[0] - self.bounds.min[0] + size - 1) / size,
            (self.bounds.max[1] - self.bounds.min[1] + size - 1) / size,
        );
        if nx <= 0 || ny <= 0 || nx.saturating_mul(ny) > 16_384 {
            return vec![Cell { x: -1, y: -1 }; 16_385];
        }
        (0..ny as i32)
            .flat_map(|y| (0..nx as i32).map(move |x| Cell { x, y }))
            .collect()
    }
    pub fn has_cell(&self, c: Cell) -> bool {
        let s = self.cell_size_cm as i64;
        c.x >= 0
            && c.y >= 0
            && s > 0
            && self.bounds.min[0] + c.x as i64 * s < self.bounds.max[0]
            && self.bounds.min[1] + c.y as i64 * s < self.bounds.max[1]
    }
    pub fn cell_at(&self, p: Point) -> Option<Cell> {
        if !self.bounds.contains(p) {
            return None;
        }
        let s = self.cell_size_cm as i64;
        Some(Cell {
            x: ((p[0].min(self.bounds.max[0] - 1) - self.bounds.min[0]) / s) as i32,
            y: ((p[1].min(self.bounds.max[1] - 1) - self.bounds.min[1]) / s) as i32,
        })
    }
    pub fn window(&self, p: Point) -> Vec<Cell> {
        let Some(c) = self.cell_at(p) else {
            return vec![];
        };
        (-1..=1)
            .flat_map(|dy| {
                (-1..=1).map(move |dx| Cell {
                    x: c.x + dx,
                    y: c.y + dy,
                })
            })
            .filter(|c| self.has_cell(*c))
            .collect()
    }
    pub fn cell_bounds(&self, c: Cell) -> Result<Bounds> {
        if !self.has_cell(c) {
            return Err(error("E_CELL", "cell outside map"));
        }
        let s = self.cell_size_cm as i64;
        let min = [
            self.bounds.min[0] + c.x as i64 * s,
            self.bounds.min[1] + c.y as i64 * s,
        ];
        Ok(Bounds {
            min,
            max: [
                (min[0] + s).min(self.bounds.max[0]),
                (min[1] + s).min(self.bounds.max[1]),
            ],
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpawnRequest {
    pub position_cm: Point,
    pub surface_id: String,
}
/// Quantized geometric contact, independent of vehicle or race policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct SurfaceProbe {
    pub position_cm: Vertex,
    pub normal_q: [i32; 3], // Upward unit normal in millionths, using portable math.
    pub surface_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GameReleaseIdentity {
    pub game_release_id: String,
    pub execution_contract_hash: String,
}
impl GameReleaseIdentity {
    pub fn admits(&self, other: &Self) -> Result<()> {
        if self != other
            || self.game_release_id.is_empty()
            || self.execution_contract_hash.len() != 64
        {
            return Err(error(
                "E_RELEASE",
                "game release or execution contract mismatch",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Triangle {
    pub vertices: [Vertex; 3],
    pub surface: Surface,
    pub object_id: String,
    pub spawnable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct GeneratedObject {
    pub id: String,
    pub asset_id: String,
    pub position: Vertex,
    pub quarter_turns: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct GeneratedChunk {
    pub format_version: u32,
    pub cell: Cell,
    pub triangles: Vec<Triangle>,
    pub objects: Vec<GeneratedObject>,
}
impl GeneratedChunk {
    pub fn hash(&self) -> Result<String> {
        // Canonical top-level keys in lexical order; buffer only one object/triangle.
        // This is byte-identical to canonical(self), without a whole serde Value tree.
        let mut hash = Sha256::new();
        hash.update(b"{\"cell\":");
        hash.update(canonical(&self.cell)?);
        hash.update(b",\"format_version\":");
        hash.update(canonical(&self.format_version)?);
        hash.update(b",\"objects\":[");
        for (index, object) in self.objects.iter().enumerate() {
            if index > 0 { hash.update(b","); }
            hash.update(canonical(object)?);
        }
        hash.update(b"],\"triangles\":[");
        for (index, triangle) in self.triangles.iter().enumerate() {
            if index > 0 { hash.update(b","); }
            hash.update(canonical(triangle)?);
        }
        hash.update(b"]}");
        Ok(format!("{:x}", hash.finalize()))
    }
    pub fn spawn(&self, request: &SpawnRequest) -> Result<Vertex> {
        self.surface_triangle(request).map(|(_, position)| position)
    }
    pub fn surface_probe(&self, request: &SpawnRequest) -> Result<SurfaceProbe> {
        let (triangle, position_cm) = self.surface_triangle(request)?;
        let [a, b, c] = triangle.vertices;
        let u = std::array::from_fn::<_, 3, _>(|i| b[i] as i128 - a[i] as i128);
        let v = std::array::from_fn::<_, 3, _>(|i| c[i] as i128 - a[i] as i128);
        let n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]].map(|v| v as f64);
        let length = libm::sqrt(n.iter().map(|v| v*v).sum());
        let sign = if n[1] < 0.0 { -1.0 } else { 1.0 };
        Ok(SurfaceProbe { position_cm, surface_id: request.surface_id.clone(),
            normal_q: n.map(|v| libm::round(v / length * sign * 1_000_000.0) as i32) })
    }
    fn surface_triangle(&self, request: &SpawnRequest) -> Result<(&Triangle, Vertex)> {
        for t in &self.triangles {
            if !t.spawnable || t.object_id != request.surface_id {
                continue;
            }
            let [a, b, c] = t.vertices;
            let flat = |v: Vertex| [v[0], v[2]];
            let area = cross(flat(a), flat(b), flat(c));
            if area == 0 || !point_in_polygon(request.position_cm, &[flat(a), flat(b), flat(c)]) {
                continue;
            }
            let wb = cross(flat(a), request.position_cm, flat(c));
            let wc = cross(flat(a), flat(b), request.position_cm);
            let h =
                a[1] + ((wb * (b[1] - a[1]) as i128 + wc * (c[1] - a[1]) as i128) / area) as i64;
            return Ok((t, [request.position_cm[0], h, request.position_cm[1]]));
        }
        Err(error(
            "E_SPAWN",
            "requested surface not drivable at this position",
        ))
    }
}
#[derive(Debug, Clone)]
pub struct HeightGrid {
    pub side: usize,
    pub heights_cm: Vec<i64>,
}
pub struct GenerationInput<'a> {
    pub document: &'a MapDocument,
    pub cell: Cell,
    pub heightgrid: Option<&'a HeightGrid>,
    pub max_triangles: usize,
}
mod cost;
mod generation;
mod overview;
pub use overview::{overview, MapOverview, OverviewBuilding, OverviewCost, OverviewRoad, OverviewSource};
pub use cost::{estimate_generation, GenerationCost};
pub use generation::generate;
