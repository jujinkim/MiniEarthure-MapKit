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

/// Shared authoring polygon operation; rings retain islands and integer shores.
pub fn union(rings: &[Vec<Vec<Point>>]) -> Result<Vec<Vec<Vec<Point>>>> {
    overlay(rings, &[], false)
}
pub fn overlap(a: &WaterBody, b: &WaterBody) -> Result<bool> {
    if !a.intersects(&crate::bounds_index::bounds(&b.polygon)) {return Ok(false);}
    Ok(!overlay(&[std::iter::once(a.polygon.clone()).chain(a.islands.clone()).collect()],
        &std::iter::once(b.polygon.clone()).chain(b.islands.clone()).collect::<Vec<_>>(), true)?.is_empty())
}
pub fn connected(a: &WaterBody,b: &WaterBody)->bool {
    if a.surface_cm!=b.surface_cm || !a.intersects(&crate::bounds_index::bounds(&b.polygon)){return false;}
    a.polygon.iter().any(|&p|b.contains_horizontal(p)) || b.polygon.iter().any(|&p|a.contains_horizontal(p)) ||
        (0..a.polygon.len()).any(|i|(0..b.polygon.len()).any(|j|intersects(a.polygon[i],a.polygon[(i+1)%a.polygon.len()],b.polygon[j],b.polygon[(j+1)%b.polygon.len()])))
}
fn overlay(subjects:&[Vec<Vec<Point>>],clip:&[Vec<Point>],intersection:bool)->Result<Vec<Vec<Vec<Point>>>> {
    use i_overlay::{core::{fill_rule::FillRule,overlay::{Overlay,ShapeType},overlay_rule::OverlayRule},i_float::int::point::IntPoint};
    crate::cancellation::checkpoint()?;
    let count:usize=subjects.iter().flat_map(|s|s.iter()).chain(clip).map(Vec::len).sum();
    if count>2_000_000 {return Err(error("E_BUDGET","water shoreline work allowance exceeded"));}
    let mut overlay=Overlay::<i64>::new(count);
    for (rings,kind) in subjects.iter().map(|s|(s.as_slice(),ShapeType::Subject)).chain(std::iter::once((clip,ShapeType::Clip))) {
        for ring in rings {overlay.add_contour(&ring.iter().map(|p|IntPoint::new(p[0],p[1])).collect::<Vec<_>>(),kind);}
    }
    let result=overlay.overlay(if intersection {OverlayRule::Intersect}else{OverlayRule::Subject},FillRule::NonZero);
    crate::cancellation::checkpoint()?;
    Ok(result.into_iter().map(|s|s.into_iter().map(|r|r.into_iter().map(|p|[p.x,p.y]).collect()).collect()).collect())
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

// Earth basins under real-city water -------------------------------------
//
// OSM-city documents (an `osm-extract-v1` scale notice, no assembled track)
// sink their generated ground inside water into an earth bed. Every rule below
// is local, so regional sources (which keep the water, roads, barriers,
// buildings and placements within their margin), prepared and unprepared
// generation and every cell see identical ground:
// - a vertex height depends only on the water bodies containing it and on
//   shore and keep-out edges within [`BASIN_REACH_CM`] of it;
// - water shores and bank loops belong to one body each, and a keep-out
//   boundary piece lies in one basin grid square and exists only within one
//   bank width of the bounds of a body that keep-out overlaps;
// - a keep-out piece can be a vertex's nearest edge only where it lies in
//   water: from a vertex in water the way to a dry point crosses a shore,
//   which is no farther. Water within the reach of a cell's vertices lies in
//   the cell's regional source, and a keep-out crossing it is wet there too,
//   so the source keeps every piece a vertex can measure.

/// Bank width bounds: `w = clamp(3 · (surface − bottom), 90, 240)` map cm.
pub const BASIN_MIN_BANK_CM: i64 = 90;
pub const BASIN_MAX_BANK_CM: i64 = 240;
/// A bank falls the body's depth plus this freeboard per bank width (the
/// flat-import surface lies 8 cm under the ground): the flat-map bank slope,
/// about 21° for the 60 cm import depth.
pub const BASIN_FREEBOARD_CM: i64 = 8;
/// Shore distance up to which a bank keeps falling at the flat-map slope
/// towards the bottom under terrain high above the water. Regional sources keep
/// water at least 1501 cm around their cells, more than this reach.
pub const BASIN_REACH_CM: i64 = 1200;
/// Keep-out boundaries split where they cross a multiple of this grid in x or
/// y. Regional sources keep water at least 1501 cm around their cells, more
/// than one grid square plus one bank width, so a kept piece meeting a cell
/// belongs to water the cell's source keeps.
pub const BASIN_GRID_CM: i64 = 1024;
/// Hard cap of basin bytes for one document, construction included; larger
/// water fails map preparation with `E_BUDGET` rather than exceeding admission.
pub const MAX_BASIN_BYTES: u64 = 8 * 1024 * 1024;
const BASIN_FIXED_BYTES: u64 = 64 * 1024;
/// Bytes allowed per water ring vertex. A vertex takes at least five JSON
/// bytes (`[x,y]`), so `basin_reserve_bytes` charges a fifth per byte.
const BASIN_BYTES_PER_VERTEX: u64 = 4000;
/// Water rings per body: the outline and at most 16 islands.
const MAX_RINGS: usize = 17;

/// Bank width of a body: three times its depth, within 90–240 cm.
pub fn bank_cm(surface_cm: i64, bottom_cm: i64) -> i64 {
    surface_cm.saturating_sub(bottom_cm).saturating_mul(3).clamp(BASIN_MIN_BANK_CM, BASIN_MAX_BANK_CM)
}

/// Upper bound of the bytes [`Basins`] allocates, at its construction peak and
/// retained, for a document whose JSON has `structure_bytes` bytes outside
/// strings (water vertices are numbers). Package admission charges it.
pub fn basin_reserve_bytes(structure_bytes: u64) -> u64 {
    BASIN_FIXED_BYTES.saturating_add(structure_bytes.saturating_mul(BASIN_BYTES_PER_VERTEX / 5)).min(MAX_BASIN_BYTES)
}
fn basin_limit(vertices: usize) -> u64 {
    BASIN_FIXED_BYTES.saturating_add(vertices as u64 * BASIN_BYTES_PER_VERTEX).min(MAX_BASIN_BYTES)
}

/// Whether a document sinks basins under its water.
pub fn basins_apply(d: &MapDocument) -> bool {
    !d.water_bodies.is_empty() && d.assembled_track.is_none() && crate::osm_source_denominator(d).is_some()
}

/// One water ring edge: its body and ring (0 the outline, k island k).
#[derive(Debug, Clone, Copy)]
struct RingEdge {
    a: Point,
    b: Point,
    body: u32,
    ring: u32,
}
#[derive(Debug, Clone, Copy, Default)]
struct RingHit {
    boundary: bool,
    odd: bool,
}
impl RingHit {
    /// `point_in_polygon` of the ring.
    fn inside(self) -> bool { self.boundary || self.odd }
}

/// Prepared basin geometry of one document: water rings, shores (water on the
/// left), keep-out polygons and the plan-view lines the ground arrangement adds.
#[derive(Debug)]
pub struct Basins {
    active: bool,
    body_index: crate::bounds_index::BoundsIndex,
    body_bounds: Vec<Bounds>,
    /// Surface and bottom of every body.
    levels: Vec<(i64, i64)>,
    /// Every water ring edge, indexed for exact point-in-water tests whose cost
    /// does not grow with the body's vertex count.
    edges: Vec<RingEdge>,
    edge_index: crate::bounds_index::BoundsIndex,
    /// Shores that are plan-view lines, shores that are not (water edges
    /// inside one keep-out), then bank loop lines; one index over all of them.
    segments: Vec<(Point, Point)>,
    segment_index: crate::bounds_index::BoundsIndex,
    shore_lines: usize,
    shores: usize,
    keep: Vec<Vec<Point>>,
    keep_index: crate::bounds_index::BoundsIndex,
    counts: BasinCounts,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct BasinCounts {
    pub water_shore_edges: u64,
    pub seam_edges: u64,
    pub road_keep_outs: u64,
    pub barrier_keep_outs: u64,
    pub building_keep_outs: u64,
    pub placement_keep_outs: u64,
    /// Keep-out boundary pieces near water (shores and plan-view lines).
    pub keep_out_pieces: u64,
    pub lines: u64,
}

/// Diagnostic of a document's basins. Components are the union of all water
/// bodies (any level). A component is `kept_out` when no generated vertex can
/// sink in it (every sampled interior point lies in a keep-out or mixes levels)
/// and `trimmed` when a road, barrier, building or placement keep-out meets it.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct BasinReport {
    pub active: bool,
    pub components: u64,
    pub sinking_components: u64,
    pub trimmed_components: u64,
    pub kept_out_components: u64,
    /// Component bodies, per kept-out component (diagnostic only).
    pub kept_out: Vec<Vec<String>>,
    pub counts: BasinCounts,
    pub retained_bytes: u64,
    pub limit_bytes: u64,
}

fn ring_area(ring: &[Point]) -> i128 {
    (0..ring.len()).map(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
    }).sum()
}
fn segment_distance2(p: Point, a: Point, b: Point) -> u128 {
    let (dx, dy) = ((b[0] - a[0]) as i128, (b[1] - a[1]) as i128);
    let (px, py) = ((p[0] - a[0]) as i128, (p[1] - a[1]) as i128);
    let len = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let d2 = if len == 0 || dot <= 0 {
        px * px + py * py
    } else if dot >= len {
        (px - dx) * (px - dx) + (py - dy) * (py - dy)
    } else {
        let area = px * dy - py * dx;
        area * area / len
    };
    d2 as u128
}
/// Integer octagon around `p` whose edges stay at least `radius` away from it.
fn octagon(p: Point, radius: i64) -> [Vertex; 8] {
    let r = radius + radius / 12 + 1;
    let s = libm::round(r as f64 / libm::sqrt(2.0)) as i64;
    [[r, 0], [s, s], [0, r], [-s, s], [-r, 0], [-s, -s], [0, -r], [s, -s]].map(|o| [p[0] + o[0], 0, p[1] + o[1]])
}
fn octagon_ring(p: Point, radius: i64) -> Vec<Point> {
    octagon(p, radius).into_iter().map(|v| [v[0], v[2]]).collect()
}
fn capsule(a: Point, b: Point, radius: i64) -> Vec<Point> {
    crate::roads::hull(octagon(a, radius).into_iter().chain(octagon(b, radius)).collect())
        .into_iter().map(|v| [v[0], v[2]]).collect()
}
/// `n / d` rounded half away from zero (`d > 0`).
fn div_round(n: i128, d: i128) -> i128 {
    if n >= 0 { (2 * n + d) / (2 * d) } else { -((-2 * n + d) / (2 * d)) }
}
/// `a`, the points where `a → b` crosses a multiple of [`BASIN_GRID_CM`] in x
/// or y (rounded, in order along the edge) and `b`: a function of the edge
/// alone, so consecutive points bound pieces that each lie in one grid square.
fn grid_split(a: Point, b: Point, work: &mut usize) -> Result<Vec<Point>> {
    let d = [b[0] - a[0], b[1] - a[1]];
    // (numerator, positive denominator) of each crossing's edge parameter.
    let mut cuts: Vec<(i128, i128, Point)> = Vec::new();
    for axis in 0..2 {
        if d[axis] == 0 { continue; }
        let (lo, hi) = (a[axis].min(b[axis]), a[axis].max(b[axis]));
        tick(work, ((hi - lo) / BASIN_GRID_CM) as usize + 1)?;
        let mut k = lo.div_euclid(BASIN_GRID_CM) + 1;
        while k * BASIN_GRID_CM < hi {
            let line = k * BASIN_GRID_CM;
            let (num, den) = ((line - a[axis]) as i128, d[axis] as i128);
            let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
            let mut p = [0; 2];
            p[axis] = line;
            p[1 - axis] = a[1 - axis] + div_round(num * d[1 - axis] as i128, den) as i64;
            cuts.push((num, den, p));
            k += 1;
        }
    }
    cuts.sort_by(|x, y| (x.0 * y.1).cmp(&(y.0 * x.1)).then(x.2.cmp(&y.2)));
    let mut out = Vec::with_capacity(cuts.len() + 2);
    out.push(a);
    out.extend(cuts.into_iter().map(|c| c.2));
    out.push(b);
    out.dedup();
    Ok(out)
}
/// Closed offset polyline of a ring whose edges have the basin on their left:
/// every edge moves `r` to the left, and consecutive moved edges join at their
/// crossing, at a mitre within `2r` when they diverge, or otherwise by a chord.
fn offset_loop(edges: &[(Point, Point)], r: i64) -> Vec<Point> {
    let moved: Vec<(Point, Point)> = edges.iter().map(|&(a, b)| {
        let (dx, dy) = ((b[0] - a[0]) as f64, (b[1] - a[1]) as f64);
        let len = libm::sqrt(dx * dx + dy * dy);
        if len == 0.0 { return (a, b); }
        let o = [libm::round(-dy / len * r as f64) as i64, libm::round(dx / len * r as f64) as i64];
        ([a[0] + o[0], a[1] + o[1]], [b[0] + o[0], b[1] + o[1]])
    }).collect();
    let mut out: Vec<Point> = Vec::with_capacity(moved.len() * 2);
    for i in 0..moved.len() {
        let ((p, q), (s, t)) = (moved[i], moved[(i + 1) % moved.len()]);
        let (u, v, w) = ([(q[0] - p[0]) as f64, (q[1] - p[1]) as f64], [(t[0] - s[0]) as f64, (t[1] - s[1]) as f64],
            [(s[0] - p[0]) as f64, (s[1] - p[1]) as f64]);
        let den = u[0] * v[1] - u[1] * v[0];
        let (a, b) = if den == 0.0 { (-1.0, -1.0) } else { ((w[0] * v[1] - w[1] * v[0]) / den, (w[0] * u[1] - w[1] * u[0]) / den) };
        let x = [p[0] as f64 + u[0] * a, p[1] as f64 + u[1] * a];
        let near = |e: Point| libm::sqrt((x[0] - e[0] as f64).powi(2) + (x[1] - e[1] as f64).powi(2)) <= 2.0 * r as f64;
        // Crossing moved edges meet at their crossing; diverging ones at their
        // extended crossing when it is near (a mitre), otherwise by a chord.
        if den != 0.0 && (((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b)) || (a >= 1.0 && b <= 0.0 && near(q) && near(s))) {
            out.push([libm::round(x[0]) as i64, libm::round(x[1]) as i64]);
        } else {
            out.push(q);
            out.push(s);
        }
    }
    out.dedup();
    while out.len() > 1 && out.first() == out.last() { out.pop(); }
    out
}
fn mid(a: Point, b: Point) -> Point {
    [(a[0] + b[0]).div_euclid(2), (a[1] + b[1]).div_euclid(2)]
}
fn tick(work: &mut usize, n: usize) -> Result<()> {
    crate::placement::tick(work, n)
}
fn overlaps(a: &Bounds, b: &Bounds) -> bool {
    (0..2).all(|i| a.max[i] >= b.min[i] && a.min[i] <= b.max[i])
}
fn vec_bytes<T>(v: &Vec<T>) -> u64 {
    v.capacity() as u64 * std::mem::size_of::<T>() as u64
}
/// Conservative bytes of a [`crate::bounds_index::BoundsIndex`] of `n` items,
/// with the item bounds its construction copies.
fn index_bytes(n: usize) -> u64 {
    // The index splits item lists at half their length down to leaves of at most
    // eight, so its shape depends on `n` alone: nodes (in a vector that doubles
    // from four) and item lists (a leaf keeps the capacity of its topmost
    // left-chain ancestor: the root list of `n`, or a split-off right half).
    // Each level holds at most two list lengths, so memoised it is O(log n).
    fn shape(n: u64, memo: &mut BTreeMap<u64, (u64, u64)>) -> (u64, u64) {
        if n <= 8 { return (1, 0); }
        if let Some(&s) = memo.get(&n) { return s; }
        let ((a, x), (b, y)) = (shape(n / 2, memo), shape(n - n / 2, memo));
        let s = (1 + a + b, x + y + (n - n / 2));
        memo.insert(n, s);
        s
    }
    if n == 0 { return 64; }
    let n = n as u64;
    let (nodes, split) = shape(n, &mut BTreeMap::new());
    let node = std::mem::size_of::<Bounds>() as u64 + 48;
    // Upper bound on the node size (bounds, children, item list) plus slack.
    node * nodes.next_power_of_two().max(4) + 8 * (n + split) + 256
}
/// Fails when `fixed` plus the growing buffers `parts` (plus the largest of
/// them again, a reallocation in flight) exceed `limit`.
fn within(limit: u64, fixed: u64, parts: &[u64]) -> Result<()> {
    let total = fixed + parts.iter().sum::<u64>() + parts.iter().copied().max().unwrap_or(0);
    if total > limit {
        return Err(error("E_BUDGET", "water basins exceed the basin memory allowance"));
    }
    Ok(())
}
/// The conservative `sidewalk_width` of a road from its own record: real-city
/// imports author `sidewalk_cm`; otherwise the widest generated sidewalk.
fn keep_sidewalk(d: &MapDocument, r: &Road) -> i64 {
    if r.kind != RoadKind::Ground { return 0; }
    match r.sidewalk_cm {
        Some(width) => i64::from(width),
        None if d.theme == "rural" || r.widths_cm.iter().copied().max().unwrap_or(0) < if d.theme == "urban" { 400 } else { 500 } => 0,
        None => 250,
    }
}

impl Basins {
    fn empty() -> Self {
        let none = || crate::bounds_index::BoundsIndex::new(&[]);
        Self { active: false, body_index: none(), body_bounds: vec![], levels: vec![], edges: vec![], edge_index: none(),
            segments: vec![], segment_index: none(), shore_lines: 0, shores: 0, keep: vec![], keep_index: none(),
            counts: BasinCounts::default() }
    }
    /// Builds the basins of a validated document (empty unless [`basins_apply`]).
    /// Work is charged to the placement work limit, one counter per phase;
    /// allocated bytes, construction peak included, are checked against
    /// [`basin_reserve_bytes`] of the water's minimum encoding.
    pub fn new(d: &MapDocument) -> Result<Self> {
        if !basins_apply(d) {
            return Ok(Self::empty());
        }
        let mut work = 0usize;
        let bodies = &d.water_bodies;
        let vertices: usize = bodies.iter().map(WaterBody::vertices).sum();
        let limit = basin_limit(vertices);
        let body_bounds: Vec<Bounds> = bodies.iter().map(|b| crate::bounds_index::bounds(&b.polygon)).collect();
        let mut edges = Vec::with_capacity(vertices);
        for (i, body) in bodies.iter().enumerate() {
            for (k, ring) in std::iter::once(&body.polygon).chain(&body.islands).enumerate() {
                edges.extend((0..ring.len()).map(|e| RingEdge { a: ring[e], b: ring[(e + 1) % ring.len()], body: i as u32, ring: k as u32 }));
            }
        }
        let fixed = std::mem::size_of::<Self>() as u64 + vec_bytes(&body_bounds) + 16 * bodies.len() as u64
            + vec_bytes(&edges) + index_bytes(edges.len()) + 32 * edges.len() as u64 + index_bytes(bodies.len());
        within(limit, fixed, &[])?;
        tick(&mut work, edges.len().saturating_mul(edges.len().max(2).ilog2() as usize + 1))?;
        let edge_index = crate::bounds_index::BoundsIndex::new(
            &edges.iter().map(|e: &RingEdge| crate::bounds_index::bounds(&[e.a, e.b])).collect::<Vec<_>>());
        let none = || crate::bounds_index::BoundsIndex::new(&[]);
        let mut basins = Self {
            active: true,
            body_index: crate::bounds_index::BoundsIndex::new(&body_bounds),
            body_bounds,
            levels: bodies.iter().map(|b| (b.surface_cm, b.bottom_cm)).collect(),
            edges, edge_index,
            segments: vec![], segment_index: none(), shore_lines: 0, shores: 0, keep: vec![], keep_index: none(),
            counts: BasinCounts::default(),
        };
        let fixed = fixed - 32 * basins.edges.len() as u64;
        // Water shores, with the water on the left. A ring edge whose midpoint
        // lies in another body of the same surface and bottom is a seam between
        // import pieces (which share whole cut edges), not a shore; touching
        // bodies of different levels keep a bank between them.
        let mut water_shores: Vec<(Point, Point)> = Vec::new();
        for (i, body) in bodies.iter().enumerate() {
            for (k, ring) in std::iter::once(&body.polygon).chain(&body.islands).enumerate() {
                let reverse = (ring_area(ring) > 0) == (k != 0);
                for e in 0..ring.len() {
                    let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                    let doubled = [a[0] + b[0], a[1] + b[1]];
                    let m = mid(a, b);
                    let mut covered = false;
                    for j in basins.body_index.query(&Bounds { min: m, max: [m[0] + 1, m[1] + 1] }, &mut work)? {
                        if j == i || basins.levels[j] != basins.levels[i] { continue; }
                        let hit = basins.rings_at(j, doubled, 2, &mut work)?;
                        // On an island's shore still counts as water here.
                        if hit[0].inside() && !hit[1..].iter().any(|h| h.odd && !h.boundary) {
                            covered = true;
                            break;
                        }
                    }
                    if covered { basins.counts.seam_edges += 1; continue; }
                    water_shores.push(if reverse { (b, a) } else { (a, b) });
                }
            }
            within(limit, fixed, &[vec_bytes(&water_shores)])?;
        }
        basins.counts.water_shore_edges = water_shores.len() as u64;
        // Keep-outs: generated ground stays on the terrain under ground, tunnel
        // and underpass road surfaces (junction aprons and sidewalks included),
        // under barriers, buildings and authored placements. Only keep-outs
        // overlapping water matter: from a point in water, a dry keep-out lies
        // beyond the shore, which is nearer. Each boundary edge splits on the
        // basin grid and keeps the pieces within one bank width of the bounds
        // of a body the keep-out overlaps.
        work = 0;
        let mut keep: Vec<Vec<Point>> = Vec::new();
        let mut keep_bytes = 0u64;
        let mut pieces: Vec<(Point, Point)> = Vec::new();
        let mut add = |ring: Vec<Point>, basins: &Basins, work: &mut usize| -> Result<bool> {
            tick(work, ring.len())?;
            if ring.len() < 3 || ring_area(&ring) == 0 { return Ok(false); }
            let wet = basins.wet_bodies(d, &ring, work)?;
            if wet.is_empty() { return Ok(false); }
            let windows: Vec<Bounds> = wet.iter().map(|&i| {
                let b = &basins.body_bounds[i];
                Bounds { min: b.min.map(|v| v - BASIN_MAX_BANK_CM - 2), max: b.max.map(|v| v + BASIN_MAX_BANK_CM + 2) }
            }).collect();
            // Keep-out boundaries are shores too, with the keep-out on the right.
            let reverse = ring_area(&ring) > 0;
            for e in 0..ring.len() {
                let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                if a == b { continue; }
                let points = grid_split(a, b, work)?;
                tick(work, points.len().saturating_mul(windows.len()))?;
                for piece in points.windows(2) {
                    let area = crate::bounds_index::bounds(piece);
                    if !windows.iter().any(|w| overlaps(w, &area)) { continue; }
                    pieces.push(if reverse { (piece[1], piece[0]) } else { (piece[0], piece[1]) });
                }
            }
            keep_bytes += 24 + 16 * ring.capacity() as u64;
            keep.push(ring);
            within(limit, fixed, &[vec_bytes(&water_shores), vec_bytes(&keep) + keep_bytes, vec_bytes(&pieces)])?;
            Ok(true)
        };
        let kept = |r: &Road| !matches!(r.kind, RoadKind::Bridge | RoadKind::Elevated) || r.design.is_some();
        // Junction discs use every road at a node, all of which regional
        // sources keep with the node.
        let mut node_width: BTreeMap<&str, i64> = BTreeMap::new();
        let mut node_sidewalk: BTreeMap<&str, i64> = BTreeMap::new();
        for r in &d.roads {
            let width = i64::from(r.widths_cm.iter().copied().max().unwrap_or(0));
            let sidewalk = if kept(r) { keep_sidewalk(d, r) } else { 0 };
            for id in [&r.from, &r.to] {
                let e = node_width.entry(id.as_str()).or_insert(0);
                *e = (*e).max(width);
                let e = node_sidewalk.entry(id.as_str()).or_insert(0);
                *e = (*e).max(sidewalk);
            }
        }
        let mut discs: BTreeSet<&str> = BTreeSet::new();
        for r in d.roads.iter().filter(|r| kept(r)) {
            tick(&mut work, r.points.len())?;
            let sidewalk = keep_sidewalk(d, r);
            let mut area = crate::bounds_index::bounds(&r.points.iter().map(|p| [p[0], p[2]]).collect::<Vec<_>>());
            // Conservative broad phase: the widest road at either end, its sidewalk.
            let ends = node_width.get(r.from.as_str()).copied().unwrap_or(0).max(node_width.get(r.to.as_str()).copied().unwrap_or(0));
            let reach = (crate::roads::width_influence_margin(ends.clamp(0, u32::MAX as i64) as u32) + sidewalk + 2) * 13 / 12 + 2;
            for a in 0..2 { area.min[a] -= reach; area.max[a] += reach; }
            if !basins.body_index.hits(&area) { continue; }
            for (i, s) in r.points.windows(2).enumerate() {
                let (a, b) = ([s[0][0], s[0][2]], [s[1][0], s[1][2]]);
                let radius = (i64::from(r.widths_cm[i]) + 1) / 2 + sidewalk + 2;
                if add(capsule(a, b, radius), &basins, &mut work)? { basins.counts.road_keep_outs += 1; }
            }
            // A bend's outer corridor edges meet at a miter beyond the half width.
            for i in 1..r.points.len().saturating_sub(1) {
                let (p, q, n) = (r.points[i - 1], r.points[i], r.points[i + 1]);
                let (u, v) = ([(q[0] - p[0]) as f64, (q[2] - p[2]) as f64], [(n[0] - q[0]) as f64, (n[2] - q[2]) as f64]);
                let lengths = libm::sqrt(u[0] * u[0] + u[1] * u[1]) * libm::sqrt(v[0] * v[0] + v[1] * v[1]);
                if lengths == 0.0 { continue; }
                let half_cos = libm::sqrt(((1.0 + (u[0] * v[0] + u[1] * v[1]) / lengths) / 2.0).max(0.0));
                let width = i64::from(r.widths_cm[i - 1].max(r.widths_cm[i]));
                let influence = crate::roads::width_influence_margin(width as u32);
                let miter = if half_cos * influence as f64 > ((width + 1) / 2) as f64 {
                    libm::ceil(((width + 1) / 2) as f64 / half_cos) as i64 + 4
                } else { influence };
                if add(octagon_ring([q[0], q[2]], miter.min(influence) + sidewalk + 2), &basins, &mut work)? {
                    basins.counts.road_keep_outs += 1;
                }
            }
            discs.insert(r.from.as_str());
            discs.insert(r.to.as_str());
        }
        // Junction mouths and fillets stay within the plan's influence margin.
        for n in &d.nodes {
            if !discs.contains(n.id.as_str()) { continue; }
            let radius = crate::roads::width_influence_margin(node_width.get(n.id.as_str()).copied().unwrap_or(0) as u32)
                + node_sidewalk.get(n.id.as_str()).copied().unwrap_or(0) + 2;
            if add(octagon_ring([n.position[0], n.position[2]], radius), &basins, &mut work)? { basins.counts.road_keep_outs += 1; }
        }
        for barrier in &d.barriers {
            for s in barrier.line.windows(2) {
                if add(capsule(s[0], s[1], i64::from(barrier.thickness_cm) / 2 + 2), &basins, &mut work)? {
                    basins.counts.barrier_keep_outs += 1;
                }
            }
        }
        for building in &d.buildings {
            for footprint in building.footprints() {
                if add(footprint.clone(), &basins, &mut work)? { basins.counts.building_keep_outs += 1; }
            }
        }
        for placement in &d.placements {
            if add(crate::placement::footprint(d, placement), &basins, &mut work)? { basins.counts.placement_keep_outs += 1; }
        }
        basins.counts.keep_out_pieces = pieces.len() as u64;
        let keep_fixed = vec_bytes(&keep) + keep_bytes + index_bytes(keep.len()) + 32 * keep.len() as u64;
        basins.keep_index = crate::bounds_index::BoundsIndex::new(
            &keep.iter().map(|r| crate::bounds_index::bounds(r)).collect::<Vec<_>>());
        basins.keep = keep;
        let fixed = fixed + keep_fixed - 32 * basins.keep.len() as u64;
        // Plan-view lines: shores (keep-out pieces, water edges not inside one
        // keep-out) and one closed bank loop per ring. Lines split ground faces
        // only as closed curves, so each loop is a closed polyline.
        work = 0;
        let mut segments = pieces;
        let mut hidden = Vec::new();
        for (a, b) in water_shores.iter().copied() {
            if basins.inside_one_keep(a, b, &mut work)? { hidden.push((a, b)); } else { segments.push((a, b)); }
        }
        let shore_lines = segments.len();
        segments.append(&mut hidden);
        drop(hidden);
        drop(water_shores);
        let shores = segments.len();
        within(limit, fixed, &[vec_bytes(&segments)])?;
        let mut lines = Vec::new();
        let mut ring_edges = Vec::new();
        for body in bodies {
            let w = bank_cm(body.surface_cm, body.bottom_cm);
            // A loop farther in than the body's mean width (2 · area / perimeter,
            // seams included) would leave the water.
            let (mut area, mut perimeter) = (0f64, 0f64);
            for (k, ring) in std::iter::once(&body.polygon).chain(&body.islands).enumerate() {
                area += if k == 0 { 1.0 } else { -1.0 } * (ring_area(ring) as f64).abs() / 2.0;
                perimeter += (0..ring.len()).map(|e| {
                    let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                    libm::sqrt(((b[0] - a[0]) as f64).powi(2) + ((b[1] - a[1]) as f64).powi(2))
                }).sum::<f64>();
            }
            let reach = if perimeter > 0.0 { libm::floor(2.0 * area / perimeter) as i64 } else { i64::MAX };
            // One closed loop at the bank width, where flat-map banks (terrain
            // depth + freeboard above the bottom) meet the bed; past it a bank
            // under higher terrain keeps falling on the terrain tile vertices
            // only, so the plan view stays independent of terrain. A body
            // narrower than two banks gets one loop at 2/5 of its mean width,
            // near its middle.
            let r = if 2 * w < reach { w } else { reach * 2 / 5 };
            if r <= 0 { continue; }
            for (k, ring) in std::iter::once(&body.polygon).chain(&body.islands).enumerate() {
                let reverse = (ring_area(ring) > 0) == (k != 0);
                ring_edges.clear();
                ring_edges.extend((0..ring.len()).map(|e| {
                    let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                    if reverse { (b, a) } else { (a, b) }
                }));
                let looped = offset_loop(&ring_edges, r);
                tick(&mut work, looped.len())?;
                for i in 0..looped.len() {
                    let (p, q) = (looped[i], looped[(i + 1) % looped.len()]);
                    if p != q { lines.push(if p < q { (p, q) } else { (q, p) }); }
                }
                within(limit, fixed, &[vec_bytes(&segments), vec_bytes(&lines), vec_bytes(&ring_edges)])?;
            }
        }
        drop(ring_edges);
        lines.sort();
        lines.dedup();
        within(limit, fixed, &[vec_bytes(&segments), vec_bytes(&lines), 32 * (segments.len() + lines.len()) as u64])?;
        segments.extend(lines);
        segments.shrink_to_fit();
        within(limit, fixed + vec_bytes(&segments) + index_bytes(segments.len()), &[32 * segments.len() as u64])?;
        tick(&mut work, segments.len().saturating_mul(segments.len().max(2).ilog2() as usize + 1))?;
        basins.segment_index = crate::bounds_index::BoundsIndex::new(
            &segments.iter().map(|&(a, b)| crate::bounds_index::bounds(&[a, b])).collect::<Vec<_>>());
        basins.counts.lines = (segments.len() - (shores - shore_lines)) as u64;
        basins.segments = segments;
        basins.shore_lines = shore_lines;
        basins.shores = shores;
        if basins.retained_bytes() > limit {
            return Err(error("E_BUDGET", "water basins exceed the basin memory allowance"));
        }
        Ok(basins)
    }

    /// Conservative retained bytes: vector capacities plus index nodes.
    pub fn retained_bytes(&self) -> u64 {
        std::mem::size_of::<Self>() as u64
            + vec_bytes(&self.body_bounds) + vec_bytes(&self.levels) + vec_bytes(&self.edges)
            + vec_bytes(&self.segments)
            + vec_bytes(&self.keep) + self.keep.iter().map(vec_bytes).sum::<u64>()
            + index_bytes(self.edges.len()) + index_bytes(self.segments.len()) + index_bytes(self.keep.len())
            + index_bytes(self.body_bounds.len())
    }
    pub fn counts(&self) -> &BasinCounts { &self.counts }
    pub fn is_active(&self) -> bool { self.active }

    /// Whether any water body's bounds meet `area` (a part needs vertex heights).
    /// Exact on body bounds (the index's leaf union bounds reach far beyond its
    /// bodies), so the answer depends only on bodies a regional source keeps.
    pub(crate) fn touches(&self, area: &Bounds) -> Result<bool> {
        if !self.active || !self.body_index.hits(area) { return Ok(false); }
        // Uncharged: Count and Emit ask the same parts, and the broad phase
        // visits O(log n) nodes plus the bodies it returns.
        Ok(self.body_index.query(area, &mut 0)?.into_iter().any(|i| overlaps(&self.body_bounds[i], area)))
    }
    /// The plan-view basin lines whose bounds meet `area`, in line order.
    pub(crate) fn lines_in(&self, area: &Bounds, work: &mut usize) -> Result<Vec<(Point, Point)>> {
        if !self.active { return Ok(vec![]); }
        Ok(self.segment_index.query(area, work)?.into_iter()
            .filter(|&i| i < self.shore_lines || i >= self.shores).map(|i| self.segments[i]).collect())
    }
    /// Boundary and crossing parity of `q` (coordinates scaled by `scale`) for
    /// every ring of body `i`: per ring exactly `point_in_polygon`, from the ring
    /// edges a ray from `q` to the nearest side of the body's bounds meets.
    fn rings_at(&self, i: usize, q: Point, scale: i64, work: &mut usize) -> Result<[RingHit; MAX_RINGS]> {
        let b = &self.body_bounds[i];
        let lo = [q[0].div_euclid(scale), q[1].div_euclid(scale)];
        let hi = [-(-q[0]).div_euclid(scale), -(-q[1]).div_euclid(scale)];
        let gaps = [b.max[0] * scale - q[0], q[0] - b.min[0] * scale, b.max[1] * scale - q[1], q[1] - b.min[1] * scale];
        let dir = (0..4).min_by_key(|&k| (gaps[k], k)).unwrap();
        let area = match dir {
            0 => Bounds { min: lo, max: [b.max[0].max(hi[0]), hi[1]] },
            1 => Bounds { min: [b.min[0].min(lo[0]), lo[1]], max: hi },
            2 => Bounds { min: lo, max: [hi[0], b.max[1].max(hi[1])] },
            _ => Bounds { min: [lo[0], b.min[1].min(lo[1])], max: hi },
        };
        // A frame in which the ray points along +x; the crossing rule is that of
        // `point_in_polygon`, whose parity any ray direction reproduces.
        let frame = |p: Point| -> Point {
            match dir { 0 => p, 1 => [-p[0], p[1]], 2 => [p[1], p[0]], _ => [-p[1], p[0]] }
        };
        let found = self.edge_index.query(&area, work)?;
        tick(work, found.len())?;
        let qf = frame(q);
        let mut out = [RingHit::default(); MAX_RINGS];
        for e in found {
            let edge = self.edges[e];
            if edge.body as usize != i { continue; }
            let hit = &mut out[edge.ring as usize];
            if hit.boundary { continue; }
            let (a, c) = ([edge.a[0] * scale, edge.a[1] * scale], [edge.b[0] * scale, edge.b[1] * scale]);
            if on_segment(a, c, q) { hit.boundary = true; continue; }
            let (a, c) = (frame(a), frame(c));
            if (a[1] > qf[1]) != (c[1] > qf[1]) && (cross(a, c, qf) > 0) == (c[1] > a[1]) { hit.odd = !hit.odd; }
        }
        Ok(out)
    }
    /// [`WaterBody::contains_horizontal`] of body `i`.
    fn contains(&self, i: usize, p: Point, work: &mut usize) -> Result<bool> {
        let hit = self.rings_at(i, p, 1, work)?;
        Ok(hit[0].inside() && !hit[1..].iter().any(|h| h.inside()))
    }
    /// Bodies a keep-out ring overlaps, in body order.
    fn wet_bodies(&self, d: &MapDocument, ring: &[Point], work: &mut usize) -> Result<Vec<usize>> {
        let area = crate::bounds_index::bounds(ring);
        // Uncharged boolean broad phase first: most keep-out candidates are dry.
        if !self.body_index.hits(&area) { return Ok(vec![]); }
        let mut wet = BTreeSet::new();
        for e in self.edge_index.query(&area, work)? {
            let edge = self.edges[e];
            if wet.contains(&(edge.body as usize)) { continue; }
            tick(work, ring.len())?;
            if (0..ring.len()).any(|k| intersects(ring[k], ring[(k + 1) % ring.len()], edge.a, edge.b)) {
                wet.insert(edge.body as usize);
            }
        }
        for i in self.body_index.query(&area, work)? {
            if wet.contains(&i) { continue; }
            let b = &self.body_bounds[i];
            let enclosed = (0..2).all(|a| b.min[a] >= area.min[a] && b.max[a] <= area.max[a]) && {
                tick(work, ring.len())?;
                point_in_polygon(d.water_bodies[i].polygon[0], ring)
            };
            if enclosed || self.contains(i, ring[0], work)? { wet.insert(i); }
        }
        Ok(wet.into_iter().collect())
    }
    fn in_keep(&self, p: Point, work: &mut usize) -> Result<bool> {
        for k in self.keep_index.query(&Bounds { min: p, max: p }, work)? {
            tick(work, self.keep[k].len())?;
            if point_in_polygon(p, &self.keep[k]) { return Ok(true); }
        }
        Ok(false)
    }
    /// Whether one keep-out holds both ends and the midpoint of a water edge.
    fn inside_one_keep(&self, a: Point, b: Point, work: &mut usize) -> Result<bool> {
        let m = mid(a, b);
        for k in self.keep_index.query(&crate::bounds_index::bounds(&[a, b]), work)? {
            let ring = &self.keep[k];
            tick(work, 3 * ring.len())?;
            if point_in_polygon(a, ring) && point_in_polygon(b, ring) && point_in_polygon(m, ring) { return Ok(true); }
        }
        Ok(false)
    }
    /// Surface and bottom where `p` may sink: inside water bodies of one
    /// surface and bottom and outside every keep-out.
    fn level(&self, p: Point, work: &mut usize) -> Result<Option<(i64, i64)>> {
        let mut found = None;
        for i in self.body_index.query(&Bounds { min: p, max: p }, work)? {
            if !self.contains(i, p, work)? { continue; }
            match found {
                None => found = Some(self.levels[i]),
                Some(other) if other != self.levels[i] => return Ok(None),
                _ => {}
            }
        }
        if found.is_none() || self.in_keep(p, work)? { return Ok(None); }
        Ok(found)
    }
    /// Distance in whole centimetres to the nearest shore or keep-out edge,
    /// at most `cap`.
    fn depth(&self, p: Point, cap: i64, work: &mut usize) -> Result<i64> {
        let area = Bounds { min: [p[0] - cap, p[1] - cap], max: [p[0] + cap, p[1] + cap] };
        let mut best = (cap as u128) * (cap as u128);
        let found = self.segment_index.query(&area, work)?;
        tick(work, found.len())?;
        for s in found.into_iter().filter(|&s| s < self.shores) {
            let (a, b) = self.segments[s];
            best = best.min(segment_distance2(p, a, b));
        }
        Ok((best.isqrt() as i64).min(cap))
    }
    /// The basin height of a generated ground vertex at `p` whose terrain height
    /// is `g`, `d` its shore distance, `w` the bank width and `k = depth +
    /// freeboard` the bank fall per bank width: terrain above the bottom moves to
    /// `g − min(g − bottom, k · min(d, reach) / w)`, a bank at the flat-map slope
    /// down to the bottom (within `w` where the terrain lies `k` above the
    /// bottom, as on flat imports; farther under higher terrain, at most `k ·
    /// reach / w` under it); terrain at or below the bottom rises to it,
    /// `g − (g − bottom) · min(d, w) / w`. Shore, keep-out and dry vertices keep
    /// `g`. A function of the vertex alone, so faces sharing it never crack.
    pub(crate) fn height(&self, p: Point, g: i64, work: &mut usize) -> Result<i64> {
        let Some((surface, bottom)) = self.level(p, work)? else { return Ok(g) };
        let w = bank_cm(surface, bottom);
        let rise = g - bottom;
        if rise <= 0 {
            let depth = self.depth(p, w, work)?;
            return Ok(g - (rise as i128 * depth as i128 / w as i128) as i64);
        }
        let fall = surface - bottom + BASIN_FREEBOARD_CM;
        // The bank meets the bottom `rise · w / fall` from the shore: no farther
        // edge can change the height, so the shore query stops there.
        let reach = (rise as i128 * w as i128 + fall as i128 - 1) / fall as i128;
        let depth = self.depth(p, reach.min(BASIN_REACH_CM as i128) as i64, work)?;
        Ok(g - rise.min((fall as i128 * depth as i128 / w as i128) as i64))
    }
    /// Whether ground at `p` lies in a basin bed (an earth face).
    pub(crate) fn bed(&self, p: Point, work: &mut usize) -> Result<bool> {
        let Some((surface, bottom)) = self.level(p, work)? else { return Ok(false) };
        Ok(self.depth(p, bank_cm(surface, bottom), work)? > 0)
    }
}

/// Public diagnostic of a document's basins (see [`BasinReport`]).
pub fn basin_report(d: &MapDocument) -> Result<BasinReport> {
    let mut d = d.clone();
    d.normalize();
    d.validate()?;
    let basins = Basins::new(&d)?;
    let mut report = BasinReport { active: basins.active, counts: basins.counts.clone(),
        retained_bytes: basins.retained_bytes(),
        limit_bytes: basin_limit(d.water_bodies.iter().map(WaterBody::vertices).sum()), ..Default::default() };
    if !basins.active { return Ok(report); }
    let rings: Vec<Vec<Vec<Point>>> = d.water_bodies.iter()
        .map(|b| std::iter::once(b.polygon.clone()).chain(b.islands.clone()).collect()).collect();
    let mut work = 0;
    for shape in union(&rings)? {
        report.components += 1;
        let area = crate::bounds_index::bounds(&shape[0]);
        let members: Vec<String> = d.water_bodies.iter().filter(|b| b.intersects(&area)
            && b.polygon.iter().any(|&p| point_in_polygon(p, &shape[0]))).map(|b| b.id.clone()).collect();
        let crosses = |ring: &[Point], other: &[Point]| (0..ring.len()).any(|i| (0..other.len()).any(|j|
            intersects(ring[i], ring[(i + 1) % ring.len()], other[j], other[(j + 1) % other.len()])));
        let trimmed = basins.keep_index.query(&area, &mut work)?.into_iter().any(|k|
            point_in_polygon(basins.keep[k][0], &shape[0]) || point_in_polygon(shape[0][0], &basins.keep[k])
                || crosses(&basins.keep[k], &shape[0]));
        // Sample the component on a lattice of a quarter bank width.
        let step = (BASIN_MIN_BANK_CM / 2).max((area.max[0] - area.min[0]).max(area.max[1] - area.min[1]) / 64);
        let mut sinking = false;
        let mut work = 0;
        let mut y = area.min[1];
        'scan: while y <= area.max[1] {
            let mut x = area.min[0];
            while x <= area.max[0] {
                if point_in_polygon([x, y], &shape[0]) && basins.bed([x, y], &mut work)? { sinking = true; break 'scan; }
                x += step;
            }
            y += step;
        }
        if sinking { report.sinking_components += 1; } else { report.kept_out_components += 1; report.kept_out.push(members); }
        if trimmed { report.trimmed_components += 1; }
    }
    Ok(report)
}
