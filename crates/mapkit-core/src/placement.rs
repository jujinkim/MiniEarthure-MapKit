//! Explicit current buildings and placement. All decisions use source identity,
//! bounded integer geometry and portable math, never a loaded-neighbour cache.
use crate::generation::Builder;
use crate::*;

pub const SCRATCH_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_WORK: usize = 8_000_000; // About 11,000 dense 1:8 city buildings.
pub const MAX_REPEATED: usize = 20_000;
const TREE_RADIUS: i64 = 200; // encloses the common renderer's 184 cm canopy

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildingPrism {
    pub object_id: String,
    pub material: String,
    pub usage: String,
    pub footprint: [Point; 3],
    pub bottom_cm: i64,
    pub top_cm: [i64; 3],
}
impl BuildingPrism {
    pub fn vertices(&self) -> [Vertex; 6] {
        std::array::from_fn(|i| {
            [
                self.footprint[i % 3][0],
                if i < 3 {
                    self.bottom_cm
                } else {
                    self.top_cm[i - 3]
                },
                self.footprint[i % 3][1],
            ]
        })
    }
    pub fn valid(&self) -> bool {
        cross(self.footprint[0], self.footprint[1], self.footprint[2]) != 0
            && self
                .top_cm
                .iter()
                .all(|h| *h > self.bottom_cm && h.unsigned_abs() <= 8_100_000)
            && self.bottom_cm.unsigned_abs() <= 8_100_000
            && self
                .footprint
                .iter()
                .flatten()
                .all(|v| v.unsigned_abs() <= 10_000_000)
            && matches!(self.material.as_str(), "concrete" | "brick" | "wood")
            && matches!(
                self.usage.as_str(),
                "residential" | "commercial" | "industrial" | "public"
            )
            && !self.object_id.is_empty()
            && self.object_id.len() <= 128
    }
}
pub(crate) const WORK: crate::profile::Budget = crate::profile::Budget { code: "placement_work", limit: MAX_WORK as u64,
    guard: MAX_WORK as u64 * 8, message: "Building and placement checks need more work than the game allows", runtime_rejects: true };
const REPEATED: crate::profile::Budget = crate::profile::Budget { code: "repetition_candidates", limit: MAX_REPEATED as u64,
    guard: MAX_REPEATED as u64 * 8, message: "Repeated objects create more candidates than the game allows", runtime_rejects: true };
const ZONE_CANDIDATES: crate::profile::Budget = crate::profile::Budget { code: "zone_candidates", limit: 300_000,
    guard: 2_400_000, message: "A vegetation zone visits more candidates per cell than the game allows", runtime_rejects: true };
pub(crate) fn tick(work: &mut usize, amount: usize) -> Result<()> {
    crate::cancellation::checkpoint()?;
    *work = work.saturating_add(amount);
    if *work > MAX_WORK {
        WORK.check(*work as u64, || error("E_BUDGET", "current placement work limit"))
    } else {
        Ok(())
    }
}
fn xy(p: Vertex) -> Point {
    [p[0], p[2]]
}
fn rectangle(min: Point, max: Point) -> [Point; 4] {
    [min, [max[0], min[1]], max, [min[0], max[1]]]
}
fn aabb(poly: &[Point]) -> Bounds {
    Bounds {
        min: std::array::from_fn(|a| poly.iter().map(|p| p[a]).min().unwrap()),
        max: std::array::from_fn(|a| poly.iter().map(|p| p[a]).max().unwrap()),
    }
}
fn overlaps(a: &Bounds, b: &Bounds) -> bool {
    (0..2).all(|i| a.min[i] <= b.max[i] && b.min[i] <= a.max[i])
}
fn polygons_overlap(a: &[Point], b: &[Point], work: &mut usize) -> Result<bool> {
    tick(work, 1)?;
    if !overlaps(&aabb(a), &aabb(b)) {
        return Ok(false);
    }
    tick(work, a.len() * b.len())?;
    Ok(a.iter().any(|p| point_in_polygon(*p, b))
        || b.iter().any(|p| point_in_polygon(*p, a))
        || (0..a.len()).any(|i| {
            (0..b.len()).any(|j| intersects(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]))
        }))
}
/// Any solid of the record (footprint or part) overlaps `poly`.
fn building_overlap(poly: &[Point], b: &Building, work: &mut usize) -> Result<bool> {
    for (footprint, holes) in b.rings() {
        if solid_overlap(poly, footprint, holes, work)? {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Bounds of solid `k`; solid 0 also covers the record's entrances.
fn solid_area(b: &Building, k: usize) -> Bounds {
    let ring = if k == 0 { &b.footprint } else { &b.parts[k - 1].footprint };
    let mut area = aabb(ring);
    for ring in b.entrances.iter().filter(|_| k == 0) {
        let other = aabb(ring);
        for a in 0..2 {
            area.min[a] = area.min[a].min(other.min[a]);
            area.max[a] = area.max[a].max(other.max[a]);
        }
    }
    area
}
fn solid_overlap(poly: &[Point], footprint: &[Point], holes: &[Vec<Point>], work: &mut usize) -> Result<bool> {
    if !polygons_overlap(poly, footprint, work)? {
        return Ok(false);
    }
    for hole in holes {
        if inside(poly, hole, work)? {
            let mut touching = false;
            tick(work, poly.len() * hole.len())?;
            for i in 0..poly.len() {
                for j in 0..hole.len() {
                    touching |= intersects(
                        poly[i],
                        poly[(i + 1) % poly.len()],
                        hole[j],
                        hole[(j + 1) % hole.len()],
                    );
                }
            }
            if !touching {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// The document has already validated simple rings. Two connected polygons
/// overlap iff one contains the other's first vertex or their boundaries meet.
/// Index the fixed building edges once instead of charging every vertex pair
/// for every neighbouring road triangle or building.
struct BuildingIndex<'a> {
    building: &'a Building,
    area: Bounds,
    edges: crate::bounds_index::BoundsIndex,
    edge_bounds: Vec<Bounds>,
}
impl<'a> BuildingIndex<'a> {
    fn new(building: &'a Building, work: &mut usize) -> Result<Self> {
        let ring = &building.footprint;
        tick(work, ring.len())?;
        let edge_bounds: Vec<_> = (0..ring.len())
            .map(|i| aabb(&[ring[i], ring[(i + 1) % ring.len()]]))
            .collect();
        Ok(Self { building, area: aabb(ring), edges: crate::bounds_index::BoundsIndex::new(&edge_bounds), edge_bounds })
    }

    /// `point_in_polygon` for the footprint, reading only the edges that can meet
    /// the +x ray from `p` (or contain `p`); other edges never change the result.
    fn contains(&self, p: Point, work: &mut usize) -> Result<bool> {
        let ring = &self.building.footprint;
        let ray = Bounds { min: p, max: [self.area.max[0].max(p[0]), p[1]] };
        let candidates = self.edges.query(&ray, work)?;
        tick(work, candidates.len())?;
        let mut inside = false;
        for i in candidates {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if on_segment(a, b, p) { return Ok(true); }
            if (a[1] > p[1]) != (b[1] > p[1]) && (cross(a, b, p) > 0) == (b[1] > a[1]) { inside = !inside; }
        }
        Ok(inside)
    }

    fn overlaps(&self, poly: &[Point], work: &mut usize) -> Result<bool> {
        tick(work, 1)?;
        let poly_area = aabb(poly);
        if !overlaps(&poly_area, &self.area) { return Ok(false); }
        let ring = &self.building.footprint;
        let mut hit = false;
        if self.area.contains(poly[0]) {
            hit = self.contains(poly[0], work)?;
        }
        if !hit && poly_area.contains(ring[0]) {
            tick(work, poly.len())?;
            hit = point_in_polygon(ring[0], poly);
        }
        if !hit {
            // One index query for the whole polygon: footprint edges away from it
            // cannot cross any of its sides.
            let candidates = self.edges.query(&poly_area, work)?;
            tick(work, candidates.len())?;
            'boundary: for j in candidates {
                for i in 0..poly.len() {
                    let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                    if overlaps(&aabb(&[a, b]), &self.edge_bounds[j])
                        && intersects(a, b, ring[j], ring[(j + 1) % ring.len()]) {
                        hit = true;
                        break 'boundary;
                    }
                }
            }
        }
        if !hit { return Ok(false); }
        for hole in &self.building.holes {
            if !overlaps(&poly_area, &aabb(hole)) { continue; }
            if inside(poly, hole, work)? {
                tick(work, poly.len() * hole.len())?;
                let touching = (0..poly.len()).any(|i| (0..hole.len()).any(|j|
                    intersects(poly[i], poly[(i + 1) % poly.len()], hole[j], hole[(j + 1) % hole.len()])));
                if !touching { return Ok(false); }
            }
        }
        Ok(true)
    }
}
fn inside(poly: &[Point], boundary: &[Point], work: &mut usize) -> Result<bool> {
    tick(work, poly.len() * boundary.len())?;
    if !poly.iter().all(|p| point_in_polygon(*p, boundary)) {
        return Ok(false);
    }
    if boundary.iter().any(|p| {
        point_in_polygon(*p, poly)
            && !(0..poly.len()).any(|i| on_segment(poly[i], poly[(i + 1) % poly.len()], *p))
    }) {
        return Ok(false);
    }
    // Reject a boundary crossing even when all candidate corners are inside a
    // concave polygon. Touching its boundary is allowed; crossing a notch is not.
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if !point_in_polygon([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], boundary) {
            return Ok(false);
        }
        for j in 0..boundary.len() {
            let (c, d) = (boundary[j], boundary[(j + 1) % boundary.len()]);
            if cross(a, b, c).signum() * cross(a, b, d).signum() < 0
                && cross(c, d, a).signum() * cross(c, d, b).signum() < 0
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
fn near_segment(p: Point, a: Point, b: Point, radius: i64) -> bool {
    let dx = (b[0] - a[0]) as i128;
    let dy = (b[1] - a[1]) as i128;
    let px = (p[0] - a[0]) as i128;
    let py = (p[1] - a[1]) as i128;
    let len = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let radius2 = radius as i128 * radius as i128;
    if dot <= 0 {
        px * px + py * py <= radius2
    } else if dot >= len {
        (px - dx) * (px - dx) + (py - dy) * (py - dy) <= radius2
    } else {
        let area = px * dy - py * dx;
        area * area <= radius2 * len
    }
}
fn road_overlap(poly: &[Point], r: &Road, extra: i64, work: &mut usize) -> Result<bool> {
    let bounds = aabb(poly);
    for (i, s) in r.points.windows(2).enumerate() {
        tick(work, 1)?;
        let (a, b) = (xy(s[0]), xy(s[1]));
        let radius = (r.widths_cm[i] as i64 + 1) / 2 + extra;
        // Charge the broad phase separately; only intersecting bounds require
        // the per-vertex polygon predicates. The work ceiling is unchanged.
        if (0..2).any(|axis| {
            a[axis].max(b[axis]) + radius < bounds.min[axis]
                || a[axis].min(b[axis]) - radius > bounds.max[axis]
        }) {
            continue;
        }
        tick(work, poly.len())?;
        if point_in_polygon(a, poly) || point_in_polygon(b, poly) {
            return Ok(true);
        }
        for j in 0..poly.len() {
            let (c, d) = (poly[j], poly[(j + 1) % poly.len()]);
            if intersects(a, b, c, d)
                || near_segment(c, a, b, radius)
                || near_segment(a, c, d, radius)
                || near_segment(b, c, d, radius)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
/// Exact carriageways and safety strips share generation's plan. A caller
/// prepares it once per footprint and supplies its exact solid predicate.
fn planned_overlap(
    plan: &(Vec<crate::road_plan::Patch>, Vec<crate::road_plan::Edge>),
    r: &Road,
    work: &mut usize,
    mut intersects_solid: impl FnMut(&[Point], &mut usize) -> Result<bool>,
) -> Result<bool> {
    let (patches, edges) = plan;
    for patch in patches.iter().filter(|p|p.road.id==r.id) {
        if intersects_solid(&patch.v.map(xy),work)? { return Ok(true); }
    }
    if matches!(r.kind,RoadKind::Bridge|RoadKind::Elevated) {
        for edge in edges.iter().filter(|e|e.road.id==r.id) {
            let len=crate::road_plan::length(edge.a,edge.b);
            if len<1.0 {continue;}
            let offset=[libm::round((edge.b[2]-edge.a[2]) as f64*20.0/len) as i64,
                -libm::round((edge.b[0]-edge.a[0]) as f64*20.0/len) as i64];
            let (a,b)=(xy(edge.a),xy(edge.b));
            if intersects_solid(&[a,b,[b[0]+offset[0],b[1]+offset[1]],[a[0]+offset[0],a[1]+offset[1]]],work)? {return Ok(true);}
        }
    }
    Ok(false)
}
pub(crate) fn builtin(id: &str) -> Option<Vec<CollisionBox>> {
    let proxy = |center, size_cm| CollisionBox { center, size_cm };
    match id {
        "builtin:tree" => Some(vec![proxy([0, 200, 0], [40, 400, 40])]),
        "builtin:fence" => Some(vec![proxy([0, 60, 0], [200, 120, 12])]),
        "builtin:streetlight" => Some(vec![
            proxy([0, 250, 0], [20, 500, 20]),
            proxy([0, 500, 0], [80, 25, 40]),
        ]),
        _ => None,
    }
}
fn local_proxies(d: &MapDocument, p: &Placement) -> Vec<CollisionBox> {
    builtin(&p.asset_id)
        .unwrap_or_else(|| {
            d.assets
                .iter()
                .find(|a| a.id == p.asset_id)
                .unwrap()
                .collision
                .clone()
        })
}
pub(crate) fn proxies(d: &MapDocument, p: &Placement) -> Vec<CollisionBox> {
    local_proxies(d,p).into_iter()
        .map(|mut proxy| {
            for _ in 0..p.quarter_turns {
                proxy.center = [-proxy.center[2], proxy.center[1], proxy.center[0]];
                proxy.size_cm.swap(0, 2);
            }
            for a in 0..3 {
                proxy.center[a] += p.position[a];
            }
            proxy
        })
        .collect()
}
pub(crate) fn footprint(d: &MapDocument, p: &Placement) -> Vec<Point> {
    if p.asset_id == "builtin:tree" {
        return rectangle(
            [p.position[0] - TREE_RADIUS, p.position[2] - TREE_RADIUS],
            [p.position[0] + TREE_RADIUS, p.position[2] + TREE_RADIUS],
        )
        .to_vec();
    }
    let mut points = vec![];
    for proxy in local_proxies(d, p) {
        points.extend(proxy.placed(p).vertices.into_iter().map(|v| [v[0], 0, v[2]]));
    }
    if let Some(asset) = d.assets.iter().find(|a| a.id == p.asset_id) {
        for c in &asset.convex_collision {
            points.extend(c.placed(p).vertices.into_iter().map(|v| [v[0], 0, v[2]]));
        }
    }
    crate::roads::hull(points).into_iter().map(xy).collect()
}

/// A pier may occupy the space below an independently elevated deck. Ground,
/// tunnel and underpass corridors remain excluded. Use the minimum deck height
/// over the complete footprint, not the centre, and require a 1 cm separation.
fn below_deck(d: &MapDocument, p: &Placement, poly: &[Point], r: &Road, work: &mut usize) -> Result<bool> {
    if !matches!(r.kind, RoadKind::Bridge | RoadKind::Elevated) { return Ok(false); }
    tick(work, poly.len() * r.points.len().saturating_sub(1))?;
    let mut top = i64::MIN;
    for b in proxies(d, p) {
        top = top.max(b.center[1] - b.size_cm[1] as i64 / 2 + b.size_cm[1] as i64);
    }
    if let Some(asset) = d.assets.iter().find(|a| a.id == p.asset_id) {
        for c in &asset.convex_collision {
            for v in c.placed(p).vertices { top = top.max(v[1]); }
        }
    }
    // Every authored segment is considered, including adjoining approaches.
    // This is conservative where a bent deck folds back over the same parcel.
    Ok(r.points.windows(2).all(|s| {
        let dx = (s[1][0] - s[0][0]) as i128;
        let dz = (s[1][2] - s[0][2]) as i128;
        let length = dx * dx + dz * dz;
        if length == 0 { return false; }
        poly.iter().all(|v| {
            let dot = ((v[0] - s[0][0]) as i128 * dx + (v[1] - s[0][2]) as i128 * dz).clamp(0, length);
            let deck = s[0][1] as i128 * length + (s[1][1] - s[0][1]) as i128 * dot;
            (top as i128 + 1) * length <= deck
        })
    }))
}

/// Roof-to-deck gap for a building under an independently elevated deck.
pub(crate) const BUILDING_DECK_CLEARANCE_CM: i64 = 50;
/// Range along `a`→`e` (in segment lengths, 0..=1) of the part of `poly` inside
/// the segment's corridor: lateral distance at most `radius`, reaching `radius`
/// beyond both ends. `None` when the corridor misses the polygon.
pub(crate) fn corridor_extent(poly: &[Point], a: Point, e: Point, radius: f64) -> Option<(f64, f64)> {
    let (dx, dz) = ((e[0] - a[0]) as f64, (e[1] - a[1]) as f64);
    let length = dx.hypot(dz);
    if length <= 0.0 { return None; }
    let (ux, uz) = (dx / length, dz / length);
    let mut ring: Vec<[f64; 2]> = poly.iter().map(|v| {
        let (px, pz) = ((v[0] - a[0]) as f64, (v[1] - a[1]) as f64);
        [px * ux + pz * uz, pz * ux - px * uz]
    }).collect();
    // Sutherland–Hodgman against the convex corridor box; concave input keeps its extent.
    for (axis, limit, below) in [(0, -radius, false), (0, length + radius, true), (1, -radius, false), (1, radius, true)] {
        let inside = |p: &[f64; 2]| if below { p[axis] <= limit } else { p[axis] >= limit };
        let mut next = Vec::with_capacity(ring.len() + 4);
        for i in 0..ring.len() {
            let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
            if inside(&p) { next.push(p); }
            if inside(&p) != inside(&q) {
                let t = (limit - p[axis]) / (q[axis] - p[axis]);
                next.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
            }
        }
        ring = next;
        if ring.is_empty() { return None; }
    }
    let (lo, hi) = ring.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    Some(((lo / length).clamp(0.0, 1.0), (hi / length).clamp(0.0, 1.0)))
}
/// A whole building may stand below a bridge/elevated deck. Wherever a deck
/// segment's corridor and safety strip cover the footprint, the deck must stay at
/// least the clearance above the roof (including a gable rise).
fn building_below_deck(b: &Building, r: &Road, work: &mut usize) -> Result<bool> {
    if !matches!(r.kind, RoadKind::Bridge | RoadKind::Elevated) { return Ok(false); }
    let top = (b.base_cm + i64::from(b.height_cm) + roof_rise(b) + BUILDING_DECK_CLEARANCE_CM) as f64;
    let mut near = false;
    for (i, s) in r.points.windows(2).enumerate() {
        tick(work, b.footprint.len())?;
        let radius = (r.widths_cm[i.min(r.widths_cm.len() - 1)] as i64 + 1) / 2 + crate::road_safety::OUTSET_CM + 1;
        let Some((t0, t1)) = corridor_extent(&b.footprint, xy(s[0]), xy(s[1]), radius as f64) else { continue; };
        near = true;
        // Deck height is linear along the segment, so its lowest point is at an end.
        if [t0, t1].iter().any(|t| (s[0][1] as f64 + (s[1][1] - s[0][1]) as f64 * t) < top) { return Ok(false); }
    }
    Ok(near)
}

fn source_clear(
    d: &MapDocument,
    poly: &[Point],
    road_margin: i64,
    work: &mut usize,
) -> Result<bool> {
    source_clear_indexed(d, poly, road_margin, work, None)
}
fn source_clear_indexed(
    d: &MapDocument,
    poly: &[Point],
    road_margin: i64,
    work: &mut usize,
    road_bounds: Option<&[Bounds]>,
) -> Result<bool> {
    if !poly.iter().all(|p| d.bounds.contains(*p)) {
        return Ok(false);
    }
    for b in &d.buildings {
        if building_overlap(poly, b, work)? {
            return Ok(false);
        }
        for entrance in &b.entrances {
            if polygons_overlap(poly, entrance, work)? {
                return Ok(false);
            }
        }
    }
    let area = aabb(poly);
    for (index, r) in d.roads.iter().enumerate() {
        if let Some(bounds) = road_bounds {
            tick(work, 1)?;
            if !overlaps(&area, &bounds[index]) {
                continue;
            }
        }
        if road_overlap(poly, r, road_margin, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(crate) fn repeated(d: &MapDocument) -> Result<Vec<Placement>> {
    let mut result = vec![];
    for r in &d.repetitions {
        let mut distance = if r.asset_id == "builtin:fence" {
            i64::from(r.spacing_cm) / 2
        } else {
            0
        };
        let mut traversed = 0;
        let mut index = 0;
        for s in r.points.windows(2) {
            let (dx, dy) = (s[1][0] - s[0][0], s[1][2] - s[0][2]);
            let length = libm::floor(libm::sqrt(
                (dx as f64) * (dx as f64) + (dy as f64) * (dy as f64),
            )) as i64;
            while distance < traversed + length {
                if result.len() >= MAX_REPEATED {
                    REPEATED.check(result.len() as u64 + 1, || error("E_BUDGET", "repetition candidate limit"))?;
                }
                let part = distance - traversed;
                let position = std::array::from_fn(|a| {
                    s[0][a] + ((s[1][a] - s[0][a]) as i128 * part as i128 / length as i128) as i64
                });
                result.push(Placement {
                    id: format!("{}:repeat:{index}", r.id),
                    asset_id: r.asset_id.clone(),
                    position,
                    quarter_turns: if dy.abs() > dx.abs() { 1 } else { 0 },
                    yaw_offset_mdeg: 0,
                });
                index += 1;
                distance += i64::from(r.spacing_cm);
            }
            traversed += length;
        }
    }
    Ok(result)
}
pub(crate) fn validate(d: &MapDocument) -> Result<()> {
    for r in &d.roads {
        if r.sidewalk_cm.is_some_and(|w| w > 1000 || w > 0 && w < 20) {
            return Err(error("E_GEOMETRY", "sidewalk width exceeds 1000 cm"));
        }
    }
    for b in &d.buildings {
        if !matches!(b.material.as_str(), "concrete" | "brick" | "wood")
            || !matches!(
                b.usage.as_str(),
                "residential" | "commercial" | "industrial" | "public"
            )
            || !matches!(b.roof.as_str(), "flat" | "gable")
            || b.entrances.iter().any(|p| !polygon_valid(p, &d.bounds))
        {
            return Err(error(
                "E_GEOMETRY",
                "unsupported building material/use/roof or invalid entrance",
            ));
        }
        for b in b.solids().filter(|_| b.roof == "gable") {
            let area = aabb(&b.footprint);
            if b.footprint.len() != 4
                || (0..2).any(|a| area.max[a] - area.min[a] < 2)
                || !b
                    .footprint
                    .iter()
                    .all(|p| (0..2).all(|a| p[a] == area.min[a] || p[a] == area.max[a]))
            {
                return Err(error(
                    "E_GEOMETRY",
                    "gable roof requires an axis-aligned rectangular footprint",
                ));
            }
        }
    }
    if d.zones.iter().any(|z| z.spacing_cm > 100_000) {
        return Err(error(
            "E_GEOMETRY",
            "current zone spacing exceeds 100000 cm",
        ));
    }
    let source_ids: BTreeSet<_> = d
        .nodes
        .iter()
        .map(|v| v.id.as_str())
        .chain(d.roads.iter().map(|v| v.id.as_str()))
        .chain(d.buildings.iter().map(|v| v.id.as_str()))
        .chain(d.zones.iter().map(|v| v.id.as_str()))
        .chain(d.assets.iter().map(|v| v.id.as_str()))
        .chain(d.placements.iter().map(|v| v.id.as_str()))
        .chain(d.repetitions.iter().map(|v| v.id.as_str()))
        .chain(d.barriers.iter().map(|v| v.id.as_str()))
        .collect();
    let road_ids: BTreeSet<_> = d.roads.iter().map(|r| r.id.as_str()).collect();
    let repetition_ids: BTreeSet<_> = d.repetitions.iter().map(|r| r.id.as_str()).collect();
    for id in &source_ids {
        if id.starts_with("builtin:")
            || id
                .strip_suffix(":sidewalk")
                .is_some_and(|id| road_ids.contains(id))
            || [":safety:metal",":safety:base"].iter().any(|suffix|
                id.strip_suffix(suffix).is_some_and(|road|road_ids.contains(road)))
            || id
                .rsplit_once(":repeat:")
                .is_some_and(|(id, _)| repetition_ids.contains(id))
        {
            return Err(error(
                "E_ID",
                "source ID aliases current generated identity",
            ));
        }
    }
    for r in &d.repetitions {
        if !matches!(r.asset_id.as_str(), "builtin:fence" | "builtin:streetlight")
            || r.points.len() < 2
            || r.points.len() > 65536
            || r.spacing_cm < 200
            || r.spacing_cm > 100_000
            || r.points
                .iter()
                .any(|p| !d.bounds.contains(xy(*p)) || p[1].unsigned_abs() > 1_000_000)
            || r.points.windows(2).any(|s| {
                xy(s[0]) == xy(s[1])
                    || r.asset_id == "builtin:fence" && s[0][0] != s[1][0] && s[0][2] != s[1][2]
            })
        {
            return Err(error(
                "E_GEOMETRY",
                "invalid repetition; fences require cardinal segments",
            ));
        }
    }
    let mut work = 0;
    for b in d.buildings.iter().filter(|b| !b.parts.is_empty()) {
        parts_disjoint(b, &mut work)?;
    }
    // Validate authored objects against coarse road bounds once, then inspect
    // segments only for nearby roads. Exact predicates and the work cap remain.
    let mut road_bounds = Vec::with_capacity(d.roads.len());
    for road in &d.roads {
        tick(&mut work, road.points.len())?;
        let mut area = aabb(&road.points.iter().copied().map(xy).collect::<Vec<_>>());
        let radius = crate::road_plan::width_influence_margin(road.widths_cm.iter().copied().max().unwrap());
        for axis in 0..2 {
            area.min[axis] -= radius;
            area.max[axis] += radius;
        }
        road_bounds.push(area);
    }

    return validate_indexed(d, &road_bounds, &mut work);
}

/// Footprint and part interiors (courtyards excluded) may touch, never overlap:
/// exact integer separation of their triangulations, indexed by bounds.
fn parts_disjoint(b: &Building, work: &mut usize) -> Result<()> {
    let mut triangles = Vec::new();
    for (index, solid) in b.solids().enumerate() {
        for t in crate::courtyard::triangulate(&solid, &mut 0)? {
            triangles.push((index, t));
        }
    }
    tick(work, triangles.len().saturating_mul(triangles.len().max(1).ilog2() as usize + 1))?;
    let bounds: Vec<_> = triangles.iter().map(|(_, t)| aabb(t)).collect();
    let index = crate::bounds_index::BoundsIndex::new(&bounds);
    let lift = |t: &[Point; 3]| t.map(|p| [p[0], 0, p[1]]);
    for (i, (solid, t)) in triangles.iter().enumerate() {
        for j in index.query(&bounds[i], work)?.into_iter().filter(|&j| j < i) {
            tick(work, 1)?;
            if triangles[j].0 != *solid && crate::urban::interiors_overlap(&lift(t), &lift(&triangles[j].1)) {
                return Err(error("E_GEOMETRY", format!("building {} parts overlap", b.id)));
            }
        }
    }
    Ok(())
}
pub(crate) fn roof_rise(b: &Building) -> i64 {
    if b.roof != "gable" {
        return 0;
    }
    let area = aabb(&b.footprint);
    ((area.max[0] - area.min[0]).min(area.max[1] - area.min[1]) / 4).clamp(1, 1000)
}
fn roof_triangles(b: &Building) -> Result<Vec<[Vertex; 3]>> {
    let top = b.base_cm + i64::from(b.height_cm);
    if b.roof == "flat" {
        return Ok(crate::courtyard::triangulate(b, &mut 0)?
            .into_iter()
            .map(|t| t.map(|p| [p[0], top, p[1]]))
            .collect());
    }
    let area = aabb(&b.footprint);
    let ridge = roof_rise(b);
    let axis = if area.max[0] - area.min[0] <= area.max[1] - area.min[1] {
        0
    } else {
        1
    };
    let mid = (area.min[axis] + area.max[axis]) / 2;
    let mut out = vec![];
    for side in 0..2 {
        let mut min = area.min;
        let mut max = area.max;
        if side == 0 {
            max[axis] = mid;
        } else {
            min[axis] = mid;
        }
        let q =
            rectangle(min, max).map(|p| [p[0], top + if p[axis] == mid { ridge } else { 0 }, p[1]]);
        out.extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
    }
    Ok(out)
}
fn buildings(d: &MapDocument, b: &mut Builder) -> Result<()> {
    for record in &d.buildings {
    for (k, (footprint, _)) in record.rings().enumerate() {
        if !overlaps(&aabb(footprint), &b.bounds) {
            continue;
        }
        let building = record.solid(k);
        let building = &*building;
        for roof in roof_triangles(building)? {
            let original = (!b.distant).then(|| BuildingPrism {
                object_id: building.id.clone(),
                material: building.material.clone(),
                usage: building.usage.clone(),
                footprint: roof.map(xy),
                bottom_cm: building.base_cm,
                top_cm: roof.map(|p| p[1]),
            });
            b.solid(
                &building.id,
                SolidShape::SlopedPrism {
                    footprint: roof.map(xy),
                    bottom_cm: building.base_cm,
                    top_cm: roof.map(|p| p[1]),
                },
            )?;
            let mut poly = roof.to_vec();
            for (axis, limit, sign) in [
                (0, b.bounds.min[0], 1),
                (0, b.bounds.max[0], -1),
                (2, b.bounds.min[1], 1),
                (2, b.bounds.max[1], -1),
            ] {
                poly = crate::roads::split(&poly, |p| (p[axis] - limit) as i128 * sign).0;
            }
            for i in 1..poly.len().saturating_sub(1) {
                let top = [poly[0], poly[i], poly[i + 1]].map(|p| crate::roads::on_plane(&roof, p));
                if cross(xy(top[0]), xy(top[1]), xy(top[2])) == 0 {
                    continue;
                }
                let part = original.as_ref().map(|original| BuildingPrism {
                    footprint: top.map(xy),
                    top_cm: top.map(|p| p[1]),
                    ..original.clone()
                });
                b.triangle(top, Surface::Concrete, &building.id, false)?;
                let bottom = top.map(|p| [p[0], building.base_cm, p[2]]);
                b.triangle(
                    [bottom[2], bottom[1], bottom[0]],
                    Surface::Concrete,
                    &building.id,
                    false,
                )?;
                for j in 0..3 {
                    b.quad(
                        [bottom[j], bottom[(j + 1) % 3], top[(j + 1) % 3], top[j]],
                        Surface::Concrete,
                        &building.id,
                        false,
                    )?;
                }
                if let Some(part) = part {
                    b.chunk.building_prisms.push(part);
                }
            }
        }
    }
    }
    Ok(())
}
pub(crate) fn sidewalk_width(d: &MapDocument, r: &Road) -> u32 {
    if r.kind != RoadKind::Ground {
        return 0;
    }
    if let Some(width) = r.sidewalk_cm {
        return width;
    }
    let road_width = r.widths_cm.iter().copied().max().unwrap_or(0);
    if d.theme == "rural" || road_width < if d.theme == "urban" {400} else {500} {
        return 0;
    }
    let nearby = d
        .buildings
        .iter()
        .filter(|b| {
            let mut ignored = 0;
            b.footprints().any(|f| road_overlap(f, r, 3000, &mut ignored).unwrap_or(false))
        })
        .count();
    match d.theme.as_str() {
        "urban" if road_width >= 400 => {
            if nearby >= 3 {
                250
            } else {
                180
            }
        }
        "rural" => 0,
        _ if nearby >= 2 && road_width >= 500 => 150,
        _ => 0,
    }
}
fn emit_placement(
    d: &MapDocument,
    cell: Cell,
    p: &Placement,
    b: &mut Builder,
    unclipped: bool,
    authored: bool,
) -> Result<()> {
    let owner = d.cell_at(xy(p.position)) == Some(cell);
    let asset = d.assets.binary_search_by(|a| a.id.cmp(&p.asset_id)).ok().map(|i| &d.assets[i]);
    // Match distant_from_generated's omission exactly: authored GLBs in every
    // touched cell, generated GLBs and builtin trees only in their anchor cell.
    let omit_proxy = b.distant && ((asset.is_some_and(|a| a.path.ends_with(".glb"))
        && (authored || owner)) || (p.asset_id == "builtin:tree" && owner));
    if !omit_proxy {
        let saved = b.bounds.clone();
        if unclipped {
            b.bounds = d.bounds.clone();
        } // one owner emits the entire small trunk
        if p.yaw_offset_mdeg == 0 {
            for proxy in proxies(d, p) { b.box_shape(proxy.center, proxy.size_cm, &p.id)?; }
        } else {
            for proxy in local_proxies(d,p) { b.convex_shape(proxy.placed(p), &p.id)?; }
        }
        if let Some(asset) = asset {
            for shape in &asset.convex_collision {
                b.convex_shape(shape.placed(p), &p.id)?;
            }
        }
        b.bounds = saved;
    }
    if owner {
        b.chunk.objects.push(GeneratedObject {
            id: p.id.clone(),
            asset_id: p.asset_id.clone(),
            position: p.position,
            quarter_turns: p.quarter_turns,
            yaw_offset_mdeg: p.yaw_offset_mdeg,
        });
    }
    Ok(())
}
#[derive(Clone)]
struct Candidate {
    p: Point,
    rank: u64,
    zone: usize,
}
fn candidate(d: &MapDocument, zone: usize, x: i64, y: i64) -> Result<Option<Candidate>> {
    let z = &d.zones[zone];
    let s = i64::from(z.spacing_cm);
    let seed = sha256(&canonical(&(d.seed, &z.id, "vegetation-v3", x, y))?);
    let random = u64::from_str_radix(&seed[..16], 16).unwrap();
    if random % 1000 >= u64::from(z.density_per_mille) {
        return Ok(None);
    }
    let jitter = if z.kind == ZoneKind::Forest { s / 3 } else { 0 };
    let offset = |bits| {
        if jitter > 0 {
            ((random >> bits) % (jitter as u64 * 2 + 1)) as i64 - jitter
        } else {
            0
        }
    };
    Ok(Some(Candidate {
        p: [x * s + offset(10), y * s + offset(32)],
        rank: random,
        zone,
    }))
}
pub(crate) fn zone_tree_radius(zone: &Zone) -> i64 {
    zone.tree
        .as_ref()
        .map_or(TREE_RADIUS, |tree| i64::from(tree.radius_cm))
}
fn tree_footprint(p: Point, radius: i64) -> [Point; 4] {
    rectangle(
        [p[0] - radius, p[1] - radius],
        [p[0] + radius, p[1] + radius],
    )
}
fn eligible(
    d: &MapDocument,
    c: &Candidate,
    prepared: &PreparedPlacements,
    work: &mut usize,
) -> Result<bool> {
    let zone = &d.zones[c.zone];
    let poly = tree_footprint(c.p, zone_tree_radius(zone));
    // A footprint outside the zone's bounds cannot be inside the zone.
    let (tree_area, zone_area) = (aabb(&poly), aabb(&zone.polygon));
    if (0..2).any(|a| tree_area.min[a] < zone_area.min[a] || tree_area.max[a] > zone_area.max[a]) {
        return Ok(false);
    }
    let clearance = zone
        .tree
        .as_ref()
        .map_or(100, |tree| i64::from(tree.clearance_cm));
    if !inside(&poly, &zone.polygon, work)? || !prepared.source_clear(d, &poly, clearance, work)? {
        return Ok(false);
    }
    for excluded in &zone.exclusions {
        if polygons_overlap(&poly, excluded, work)? {
            return Ok(false);
        }
    }
    let occupied_area = aabb(&tree_footprint(c.p, zone_tree_radius(zone) + clearance));
    for i in prepared.occupied_index.query(&occupied_area, work)? {
        let p = &prepared.occupied[i];
        let footprint = if zone.tree.is_some() {
            tree_footprint(c.p, zone_tree_radius(zone) + clearance)
        } else {
            poly
        };
        if polygons_overlap(&footprint, p, work)? {
            return Ok(false);
        }
    }
    // Reserve the authored/automatic sidewalk width, even when a piece is
    // suppressed. Vegetation must never occupy a future access strip.
    let mut road_area = aabb(&poly);
    for a in 0..2 { road_area.min[a] -= 1000 + clearance; road_area.max[a] += 1000 + clearance; }
    for i in prepared.roads.query(&road_area, work)? {
        let road = &d.roads[i];
        if road_overlap(
            &poly,
            road,
            i64::from(sidewalk_width(d, road)) + clearance,
            work,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn vegetation(
    d: &MapDocument,
    cell: Cell,
    b: &mut Builder,
    prepared: &PreparedPlacements,
    work: &mut usize,
) -> Result<()> {
    for (zi, zone) in d.zones.iter().enumerate() {
        let s = i64::from(zone.spacing_cm);
        let area = aabb(&zone.polygon);
        if !overlaps(&area, &b.bounds) || zone.density_per_mille == 0 {
            continue;
        }
        // Eligibility requires the complete tree footprint to stay in its zone.
        // Clip the visited lattice to that envelope, retaining the one-spacing
        // halo for forest jitter. Global lattice coordinates/order and seam
        // competitors are unchanged; empty cell area consumes no candidate work.
        let min: Point = std::array::from_fn(|a| area.min[a].max(b.bounds.min[a]));
        let max: Point = std::array::from_fn(|a| area.max[a].min(b.bounds.max[a]));
        let mut count = 0;
        for y in min[1].div_euclid(s) - 1..=max[1].div_euclid(s) + 1 {
            for x in min[0].div_euclid(s) - 1..=max[0].div_euclid(s) + 1 {
                count += 1;
                tick(work, 1)?;
                if count > 300_000 {
                    ZONE_CANDIDATES.check(count as u64, || error("E_BUDGET", "zone candidate limit"))?;
                }
                let Some(c) = candidate(d, zi, x, y)? else {
                    continue;
                };
                if d.cell_at(c.p) != Some(cell) || !eligible(d, &c, prepared, work)? {
                    continue;
                }
                let mut blocked = false;
                // Source-global local-minimum thinning. A lower-ranked eligible
                // competitor wins even outside this cell; generation order and
                // rejection chains cannot create seam duplicates or overlap.
                for (other_index, other) in d.zones.iter().enumerate() {
                    let radii = zone_tree_radius(zone) + zone_tree_radius(other);
                    let gap = zone
                        .tree
                        .as_ref()
                        .map_or(0, |t| i64::from(t.clearance_cm))
                        .max(other.tree.as_ref().map_or(0, |t| i64::from(t.clearance_cm)));
                    let reach =
                        i64::from(zone.spacing_cm.max(other.spacing_cm)).max(radii + gap + 1);
                    let other_s = i64::from(other.spacing_cm);
                    let jitter = if other.kind == ZoneKind::Forest {
                        other_s / 3
                    } else {
                        0
                    };
                    // Every competitor scanned below lies within this window and
                    // must be inside its own zone to be eligible.
                    let window = reach + 2 * jitter + other_s;
                    let other_area = aabb(&other.polygon);
                    if (0..2).any(|a| other_area.max[a] < c.p[a] - window || other_area.min[a] > c.p[a] + window) {
                        continue;
                    }
                    for oy in (c.p[1] - reach - jitter).div_euclid(other_s)
                        ..=(c.p[1] + reach + jitter).div_euclid(other_s)
                    {
                        for ox in (c.p[0] - reach - jitter).div_euclid(other_s)
                            ..=(c.p[0] + reach + jitter).div_euclid(other_s)
                        {
                            tick(work, 1)?;
                            let Some(other_c) = candidate(d, other_index, ox, oy)? else {
                                continue;
                            };
                            if (other_c.rank, other.id.as_str(), ox, oy)
                                >= (c.rank, zone.id.as_str(), x, y)
                            {
                                continue;
                            }
                            let dx = (c.p[0] - other_c.p[0]) as i128;
                            let dy = (c.p[1] - other_c.p[1]) as i128;
                            let overlap = dx.abs() <= i128::from(radii + gap)
                                && dy.abs() <= i128::from(radii + gap);
                            if (overlap || dx * dx + dy * dy < (reach as i128) * (reach as i128))
                                && eligible(d, &other_c, prepared, work)?
                            {
                                blocked = true;
                                break;
                            }
                        }
                        if blocked {
                            break;
                        }
                    }
                    if blocked {
                        break;
                    }
                }
                if blocked {
                    continue;
                }
                let Ok(position) = b.chunk.spawn(&SpawnRequest {
                    position_cm: c.p,
                    surface_id: "terrain".into(),
                }) else {
                    continue;
                };
                let p = Placement {
                    id: format!("{}:{x}:{y}", zone.id),
                    asset_id: zone
                        .tree
                        .as_ref()
                        .map_or_else(|| "builtin:tree".into(), |t| t.asset_id.clone()),
                    position,
                    quarter_turns: (c.rank % 4) as u8,
                    yaw_offset_mdeg: 0,
                };
                emit_placement(d, cell, &p, b, true, false)?;
            }
        }
    }
    Ok(())
}
/// Source-global decisions are immutable. Keep the accepted repetition sequence
/// and ALL occupied footprints: a cell-local subset would change vegetation seams.
#[derive(Debug)]
pub(crate) struct PreparedPlacements {
    repeated: Vec<Placement>,
    occupied: Vec<Vec<Point>>,
    index: crate::bounds_index::BoundsIndex,
    occupied_index: crate::bounds_index::BoundsIndex,
    roads: crate::bounds_index::BoundsIndex,
    /// One building index entry per solid: (record, solid).
    buildings: crate::bounds_index::BoundsIndex,
    solids: Vec<(usize, usize)>,
    pub(crate) barriers: crate::barrier::Spans,
}
impl PreparedPlacements {
    pub(crate) fn new(d: &MapDocument) -> Result<Self> {
        let mut work = 0;
        let mut occupied: Vec<_> = d.placements.iter().map(|p| footprint(d, p)).collect();
        let mut accepted = vec![];
        for p in repeated(d)? {
            crate::cancellation::checkpoint()?;
            let poly = footprint(d, &p);
            if !source_clear(d, &poly, 0, &mut work)? { continue; }
            let area = aabb(&poly);
            let mut blocked = false;
            for prior in &occupied {
                tick(&mut work, 1)?;
                let other = aabb(prior);
                if (0..2).all(|i| area.min[i] < other.max[i] && other.min[i] < area.max[i]) {
                    blocked = true; break;
                }
            }
            if !blocked { occupied.push(poly); accepted.push(p); }
        }
        let bounds: Vec<_> = occupied.iter().zip(d.placements.iter().chain(&accepted)).map(|(poly,p)| {
            let mut area=aabb(poly);
            // Display belongs to the anchor cell even for offset proxy geometry.
            for (a,coord) in [p.position[0],p.position[2]].into_iter().enumerate() {
                area.min[a]=area.min[a].min(coord); area.max[a]=area.max[a].max(coord);
            }
            area
        }).collect();
        let road_bounds: Vec<_> = d.roads.iter().map(|r| {
            let mut b=aabb(&r.points.iter().copied().map(xy).collect::<Vec<_>>());
            let radius=crate::road_plan::width_influence_margin(r.widths_cm.iter().copied().max().unwrap_or(0));
            for a in 0..2 { b.min[a]-=radius; b.max[a]+=radius; } b
        }).collect();
        let solids: Vec<(usize, usize)> = d.buildings.iter().enumerate()
            .flat_map(|(i, b)| (0..=b.parts.len()).map(move |k| (i, k))).collect();
        let building_bounds: Vec<_> = solids.iter().map(|&(i, k)| solid_area(&d.buildings[i], k)).collect();
        Ok(Self { repeated: accepted,
            occupied_index: crate::bounds_index::BoundsIndex::new(&occupied.iter().map(|p|aabb(p)).collect::<Vec<_>>()),
            occupied, index: crate::bounds_index::BoundsIndex::new(&bounds),
            roads: crate::bounds_index::BoundsIndex::new(&road_bounds),
            buildings: crate::bounds_index::BoundsIndex::new(&building_bounds), solids,
            barriers: crate::barrier::Spans::new(d)? })
    }
}
impl PreparedPlacements {
    pub(crate) fn authored_candidates<'a>(&self, d: &'a MapDocument, cell: Cell) -> Result<Vec<&'a Placement>> {
        let mut work = 0;
        Ok(self.index.query(&d.cell_bounds(cell)?, &mut work)?.into_iter()
            .filter_map(|i| d.placements.get(i)).collect())
    }
    fn source_clear(&self, d: &MapDocument, poly: &[Point], margin: i64, work: &mut usize) -> Result<bool> {
        if !poly.iter().all(|p| d.bounds.contains(*p)) { return Ok(false); }
        let mut area=aabb(poly);
        for i in self.buildings.query(&area,work)? {
            let (record, k) = self.solids[i];
            let b=&d.buildings[record];
            let (footprint, holes) = if k == 0 { (&b.footprint, &b.holes) } else { (&b.parts[k-1].footprint, &b.parts[k-1].holes) };
            if solid_overlap(poly,footprint,holes,work)? { return Ok(false); }
            for entrance in b.entrances.iter().filter(|_| k == 0) { if polygons_overlap(poly,entrance,work)? { return Ok(false); } }
        }
        for a in 0..2 { area.min[a]-=margin; area.max[a]+=margin; }
        for i in self.roads.query(&area,work)? {
            if road_overlap(poly,&d.roads[i],margin,work)? { return Ok(false); }
        }
        Ok(true)
    }
}
pub(crate) fn generate(d: &MapDocument, cell: Cell, b: &mut Builder, prepared: Option<&PreparedPlacements>) -> Result<()> {
    let mut work=0;
    crate::roads::sidewalks(d,b).map_err(|mut e| {e.message=format!("sidewalk: {}",e.message);e})?;
    buildings(d,b)?;
    let owned;
    let prepared = if let Some(value)=prepared { value } else { owned=PreparedPlacements::new(d)?; &owned };
    for i in prepared.index.query(&b.bounds, &mut work)? {
        let p=if i < d.placements.len() { &d.placements[i] } else { &prepared.repeated[i-d.placements.len()] };
        emit_placement(d,cell,p,b,false,i < d.placements.len())?;
    }
    vegetation(d,cell,b,prepared,&mut work)
}

fn validate_indexed(d: &MapDocument, road_bounds: &[Bounds], work: &mut usize) -> Result<()> {
    use crate::bounds_index::BoundsIndex;
    let plans = crate::road_plan::PlanIndex::new(d)?;
    // Every solid of every record; a record's own solids never collide with
    // each other here (see parts_disjoint) and only its first carries entrances.
    let solids: Vec<(usize, std::borrow::Cow<'_, Building>)> = d.buildings.iter().enumerate()
        .flat_map(|(i, b)| b.solids().map(move |s| (i, s))).collect();
    let building_bounds: Vec<_> = solids
        .iter()
        .map(|(_, b)| {
            let mut area = aabb(&b.footprint);
            for ring in &b.entrances {
                let other = aabb(ring);
                for a in 0..2 {
                    area.min[a] = area.min[a].min(other.min[a]);
                    area.max[a] = area.max[a].max(other.max[a]);
                }
            }
            area
        })
        .collect();
    let buildings = BoundsIndex::new(&building_bounds);
    let building_shapes = solids.iter().map(|(_, b)| BuildingIndex::new(b, work)).collect::<Result<Vec<_>>>()?;
    // Dense authored splines must not multiply every placement by every road
    // station. Index segment corridors; preserve the same exact plan predicates
    // and the shared work ceiling after this broad phase.
    let segments:Vec<_>=d.roads.iter().enumerate().flat_map(|(id,r)|(0..r.points.len()-1).step_by(16).map(move |i|{
        let end=(i+16).min(r.points.len()-1);
        let mut area=aabb(&r.points[i..=end].iter().copied().map(xy).collect::<Vec<_>>());
        let margin=crate::road_plan::width_influence_margin(*r.widths_cm[i..end].iter().max().unwrap());
        for j in 0..2 {area.min[j]-=margin;area.max[j]+=margin;}
        (id,area)
    })).collect();
    tick(work,segments.len())?;
    let roads = BoundsIndex::new(&segments.iter().map(|(_,b)|b.clone()).collect::<Vec<_>>());
    let nearby_roads=|area:&Bounds,work:&mut usize|->Result<BTreeSet<usize>> {
        Ok(roads.query(area,work)?.into_iter().filter(|&i|overlaps(area,&segments[i].1)).map(|i|segments[i].0).collect())
    };
    let _=road_bounds;
    let mut effort = [0usize; 3];
    for (i, (record, b)) in solids.iter().enumerate() {
        let b: &Building = b;
        macro_rules! measured {
            ($phase:expr, $operation:expr) => {{
                let before = *work;
                let result = $operation;
                effort[$phase] += *work - before;
                result.map_err(|mut e: Error| {
                    if e.code == "E_BUDGET" {
                        e.message = format!("{} at building {} ({}/{}); neighbour/plan/footprint work {:?}", e.message, b.id, i + 1, solids.len(), effort);
                    }
                    e
                })?
            }};
        }
        let area = aabb(&b.footprint);
        for index in measured!(0, buildings.query(&area, work)).into_iter().filter(|&j| j < i) {
            let (other_record, other) = &solids[index];
            if other_record == record { continue; }
            if b.base_cm < other.base_cm + other.height_cm as i64 + roof_rise(other)
                && other.base_cm < b.base_cm + b.height_cm as i64 + roof_rise(b)
                && measured!(0, building_shapes[index].overlaps(&b.footprint, work))
                && measured!(0, building_shapes[i].overlaps(&other.footprint, work))
            {
                return Err(error(
                    "E_GEOMETRY",
                    "overlapping building footprints/height ranges",
                ));
            }
        }
        let nearby = measured!(1, nearby_roads(&area, work));
        if nearby.is_empty() { continue; }
        let plan = measured!(1, plans.plan(&area, work));
        for index in nearby {
            let road = &d.roads[index];
            // A corridor triangle fully inside an open courtyard is empty
            // space. Testing it against the exact rings avoids triangulating
            // the same building again for every neighbouring road/triangle.
            let hit = measured!(2, planned_overlap(&plan, road, work, |poly, work| building_shapes[i].overlaps(poly, work)));
            if hit && !measured!(2, building_below_deck(b, road, work)) {
                return Err(error(
                    "E_GEOMETRY",
                    format!(
                        "building {} footprint intersects road {} corridor",
                        b.id, road.id
                    ),
                ));
            }
        }
    }
    let mut footprints = vec![];
    for p in &d.placements {
        if builtin(&p.asset_id).is_none()
            && d.assets
                .iter()
                .find(|a| a.id == p.asset_id)
                .is_some_and(|a| a.collision.is_empty() && a.convex_collision.is_empty())
        {
            return Err(error(
                "E_GEOMETRY",
                "manual assets require a declared footprint proxy",
            ));
        }
        footprints.push(footprint(d, p));
    }
    let placement_bounds: Vec<_> = footprints.iter().map(|p| aabb(p)).collect();
    let placements = BoundsIndex::new(&placement_bounds);
    for (i, p) in d.placements.iter().enumerate() {
        let poly = &footprints[i];
        let area = &placement_bounds[i];
        if !poly.iter().all(|p| d.bounds.contains(*p)) {
            return Err(error(
                "E_GEOMETRY",
                format!("placement {} outside bounds", p.id),
            ));
        }
        for j in buildings.query(area, work)? {
            let b = &solids[j].1;
            if building_shapes[j].overlaps(poly, work)? {
                return Err(error(
                    "E_GEOMETRY",
                    format!("placement {} intersects building {}", p.id, b.id),
                ));
            }
            for entrance in &b.entrances {
                if polygons_overlap(poly, entrance, work)? {
                    return Err(error(
                        "E_GEOMETRY",
                        format!("placement {} intersects entrance {}", p.id, b.id),
                    ));
                }
            }
        }
        let nearby = nearby_roads(area, work)?;
        let plan = if nearby.is_empty() { (vec![], vec![]) } else { plans.plan(area, work)? };
        for j in nearby {
            if planned_overlap(&plan, &d.roads[j], work, |road_poly, work| polygons_overlap(poly, road_poly, work))? && !below_deck(d, p, poly, &d.roads[j], work)? {
                return Err(error(
                    "E_GEOMETRY",
                    format!("placement {} intersects road {}", p.id, d.roads[j].id),
                ));
            }
        }
        for j in placements.query(area, work)?.into_iter().filter(|&j| j < i) {
            if polygons_overlap(poly, &footprints[j], work)? {
                return Err(error(
                    "E_GEOMETRY",
                    format!(
                        "placement {} intersects placement {}",
                        p.id, d.placements[j].id
                    ),
                ));
            }
        }
    }
    repeated(d)?;
    Ok(())
}

#[cfg(test)]
mod city_tests {
    use super::*;

    #[test]
    fn direct_courtyard_overlap_matches_triangulated_solid_including_wall_contacts() {
        let d: MapDocument = serde_json::from_str(include_str!("../../../examples/courtyard/document.json")).unwrap();
        let b = &d.buildings[0];
        let solid = crate::courtyard::triangulate(b, &mut 0).unwrap();
        let index = BuildingIndex::new(b, &mut 0).unwrap();
        for x in (-1000..25000).step_by(1000) {
            for y in (-1000..25000).step_by(1000) {
                for size in [1, 200, 1500, 12000] {
                    let p = [[x, y], [x + size, y], [x, y + size]];
                    let expected = solid.iter().any(|triangle| polygons_overlap(triangle, &p, &mut 0).unwrap());
                    assert_eq!(building_overlap(&p, b, &mut 0).unwrap(), expected, "{p:?}");
                    assert_eq!(index.overlaps(&p, &mut 0).unwrap(), expected, "indexed {p:?}");
                    assert_eq!(index.contains([x + size / 2, y + size / 3], &mut 0).unwrap(), point_in_polygon([x + size / 2, y + size / 3], &b.footprint), "contains {p:?}");
                }
            }
        }
    }
}
