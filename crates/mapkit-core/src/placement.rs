//! Explicit recipe-3 buildings and placement. All decisions use source identity,
//! bounded integer geometry and portable math, never a loaded-neighbour cache.
use crate::generation::Builder;
use crate::*;

pub const SCRATCH_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_WORK: usize = 4_000_000;
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
pub(crate) fn tick(work: &mut usize, amount: usize) -> Result<()> {
    *work = work.saturating_add(amount);
    if *work > MAX_WORK {
        Err(error("E_BUDGET", "recipe-3 placement work limit"))
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
fn building_overlap(poly: &[Point], b: &Building, work: &mut usize) -> Result<bool> {
    if !polygons_overlap(poly, &b.footprint, work)? {
        return Ok(false);
    }
    for hole in &b.holes {
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
pub(crate) fn proxies(d: &MapDocument, p: &Placement) -> Vec<CollisionBox> {
    builtin(&p.asset_id)
        .unwrap_or_else(|| {
            d.assets
                .iter()
                .find(|a| a.id == p.asset_id)
                .unwrap()
                .collision
                .clone()
        })
        .into_iter()
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
    let mut boxes = proxies(d, p);
    if let Some(asset) = d.assets.iter().find(|a| a.id == p.asset_id) {
        for c in &asset.convex_collision {
            let c = c.placed(p);
            let min: Vertex =
                std::array::from_fn(|a| c.vertices.iter().map(|v| v[a]).min().unwrap());
            let max: Vertex =
                std::array::from_fn(|a| c.vertices.iter().map(|v| v[a]).max().unwrap());
            boxes.push(CollisionBox {
                center: std::array::from_fn(|a| min[a] + (max[a] - min[a]) / 2),
                size_cm: std::array::from_fn(|a| (max[a] - min[a]) as u32),
            });
        }
    }
    let min = std::array::from_fn(|a| {
        boxes
            .iter()
            .map(|b| b.center[a * 2] - i64::from(b.size_cm[a * 2] / 2))
            .min()
            .unwrap()
    });
    let max = std::array::from_fn(|a| {
        boxes
            .iter()
            .map(|b| {
                b.center[a * 2] - i64::from(b.size_cm[a * 2] / 2) + i64::from(b.size_cm[a * 2])
            })
            .max()
            .unwrap()
    });
    rectangle(min, max).to_vec()
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
                    return Err(error("E_BUDGET", "repetition candidate limit"));
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
    if d.recipe_version < 3 {
        if !d.repetitions.is_empty() || d.buildings.iter().any(|b| !b.entrances.is_empty()) {
            return Err(error(
                "E_VERSION",
                "placement extensions require explicit recipe 3",
            ));
        }
        return Ok(());
    }
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
        if b.roof == "gable" {
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
            "recipe-3 zone spacing exceeds 100000 cm",
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
        .collect();
    let road_ids: BTreeSet<_> = d.roads.iter().map(|r| r.id.as_str()).collect();
    let repetition_ids: BTreeSet<_> = d.repetitions.iter().map(|r| r.id.as_str()).collect();
    for id in &source_ids {
        if id.starts_with("builtin:")
            || id
                .strip_suffix(":sidewalk")
                .is_some_and(|id| road_ids.contains(id))
            || id
                .rsplit_once(":repeat:")
                .is_some_and(|(id, _)| repetition_ids.contains(id))
        {
            return Err(error(
                "E_ID",
                "source ID aliases recipe-3 generated identity",
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
    // Validate authored objects against coarse road bounds once, then inspect
    // segments only for nearby roads. Exact predicates and the work cap remain.
    let mut road_bounds = Vec::with_capacity(d.roads.len());
    for road in &d.roads {
        tick(&mut work, road.points.len())?;
        let mut area = aabb(&road.points.iter().copied().map(xy).collect::<Vec<_>>());
        let radius = i64::from(road.widths_cm.iter().copied().max().unwrap().div_ceil(2));
        for axis in 0..2 {
            area.min[axis] -= radius;
            area.max[axis] += radius;
        }
        road_bounds.push(area);
    }
    if d.recipe_version >= 6 {
        return validate_indexed(d, &road_bounds, &mut work);
    }
    for (i, b) in d.buildings.iter().enumerate() {
        for other in &d.buildings[..i] {
            if b.base_cm < other.base_cm + other.height_cm as i64 + roof_rise(other)
                && other.base_cm < b.base_cm + b.height_cm as i64 + roof_rise(b)
                && building_overlap(&b.footprint, other, &mut work)?
                && building_overlap(&other.footprint, b, &mut work)?
            {
                return Err(error(
                    "E_GEOMETRY",
                    "overlapping building footprints/height ranges",
                ));
            }
        }
        let area = aabb(&b.footprint);
        for (index, road) in d.roads.iter().enumerate() {
            tick(&mut work, 1)?;
            if !overlaps(&area, &road_bounds[index]) {
                continue;
            }
            // Conservative horizontal clearance is deliberate for authored buildings.
            let overlaps = if b.holes.is_empty() {
                road_overlap(&b.footprint, road, 0, &mut work)?
            } else {
                let mut hit = false;
                for triangle in crate::courtyard::triangulate(b, &mut work)? {
                    if road_overlap(&triangle, road, 0, &mut work)? {
                        hit = true;
                        break;
                    }
                }
                hit
            };
            if overlaps {
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
    for (i, p) in d.placements.iter().enumerate() {
        if builtin(&p.asset_id).is_none()
            && d.assets
                .iter()
                .find(|a| a.id == p.asset_id)
                .unwrap()
                .collision
                .is_empty()
            && d.assets
                .iter()
                .find(|a| a.id == p.asset_id)
                .unwrap()
                .convex_collision
                .is_empty()
        {
            return Err(error(
                "E_GEOMETRY",
                "recipe-3 manual assets require a declared footprint proxy",
            ));
        }
        let poly = footprint(d, p);
        if !source_clear_indexed(d, &poly, 0, &mut work, Some(&road_bounds))? {
            return Err(error(
                "E_GEOMETRY",
                "manual placement footprint intersects bounds/building/access/road",
            ));
        }
        for other in &d.placements[..i] {
            if polygons_overlap(&poly, &footprint(d, other), &mut work)? {
                return Err(error("E_GEOMETRY", "manual placement footprints overlap"));
            }
        }
    }
    repeated(d)?; // bound repetition expansion before any geometry allocation
    Ok(())
}
fn roof_rise(b: &Building) -> i64 {
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
    for building in &d.buildings {
        if !overlaps(&aabb(&building.footprint), &b.bounds) {
            continue;
        }
        for roof in roof_triangles(building)? {
            let original = BuildingPrism {
                object_id: building.id.clone(),
                material: building.material.clone(),
                usage: building.usage.clone(),
                footprint: roof.map(xy),
                bottom_cm: building.base_cm,
                top_cm: roof.map(|p| p[1]),
            };
            b.solid(
                &building.id,
                SolidShape::SlopedPrism {
                    footprint: original.footprint,
                    bottom_cm: original.bottom_cm,
                    top_cm: original.top_cm,
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
                let part = BuildingPrism {
                    footprint: top.map(xy),
                    top_cm: top.map(|p| p[1]),
                    ..original.clone()
                };
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
                b.chunk.building_prisms.push(part);
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
    let nearby = d
        .buildings
        .iter()
        .filter(|b| {
            let mut ignored = 0;
            road_overlap(&b.footprint, r, 3000, &mut ignored).unwrap_or(false)
        })
        .count();
    let road_width = r.widths_cm.iter().copied().max().unwrap_or(0);
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
fn sidewalks(d: &MapDocument, b: &mut Builder, work: &mut usize) -> Result<()> {
    let terrain_count = b.chunk.triangles.len();
    for r in &d.roads {
        let width = i64::from(sidewalk_width(d, r));
        if width == 0 {
            continue;
        }
        for (index, s) in r.points.windows(2).enumerate() {
            let (dx, dy) = ((s[1][0] - s[0][0]) as f64, (s[1][2] - s[0][2]) as f64);
            let length = libm::sqrt(dx * dx + dy * dy);
            let half = (i64::from(r.widths_cm[index]) + 1) / 2;
            // Open approaches around every explicit endpoint/bend. Other roads
            // and authored access corridors also suppress whole 2 m pieces.
            let gap = half + width + 100;
            let steps = libm::ceil(length / 200.0) as i64;
            for step in 0..steps {
                tick(work, 1)?;
                let start = step * 200;
                let end = ((step + 1) * 200).min(libm::floor(length) as i64);
                if start < gap || end > libm::floor(length) as i64 - gap {
                    continue;
                }
                for side in [-1, 1] {
                    let point = |along: i64, away: i64| {
                        [
                            s[0][0]
                                + libm::round(
                                    (dx * along as f64 - dy * (away * side) as f64) / length,
                                ) as i64,
                            s[0][2]
                                + libm::round(
                                    (dy * along as f64 + dx * (away * side) as f64) / length,
                                ) as i64,
                        ]
                    };
                    let mut poly = [
                        point(start, half + 1),
                        point(end, half + 1),
                        point(end, half + width),
                        point(start, half + width),
                    ];
                    if !overlaps(&aabb(&poly), &b.bounds) {
                        continue;
                    }
                    if r.sidewalk_cm.is_none() {
                        let mut fitted = width;
                        loop {
                            let mut touches = false;
                            for building in &d.buildings {
                                if building_overlap(&poly, building, work)? {
                                    touches = true;
                                    break;
                                }
                            }
                            if !touches || fitted <= 50 {
                                break;
                            }
                            fitted = (fitted - 25).max(50);
                            poly[2] = point(end, half + fitted);
                            poly[3] = point(start, half + fitted);
                        }
                    }
                    let mut blocked = false;
                    for building in &d.buildings {
                        if building_overlap(&poly, building, work)? {
                            blocked = true;
                            break;
                        }
                        for entrance in &building.entrances {
                            if polygons_overlap(&poly, entrance, work)? {
                                blocked = true;
                                break;
                            }
                        }
                    }
                    for other in &d.roads {
                        if other.id != r.id && road_overlap(&poly, other, 100, work)? {
                            blocked = true;
                            break;
                        }
                    }
                    for p in &d.placements {
                        if polygons_overlap(&poly, &footprint(d, p), work)? {
                            blocked = true;
                            break;
                        }
                    }
                    if blocked {
                        continue;
                    }
                    let id = format!("{}:sidewalk", r.id);
                    for i in 0..terrain_count {
                        tick(work, 1)?;
                        let triangle = &b.chunk.triangles[i];
                        if triangle.object_id != "terrain"
                            || !overlaps(&aabb(&triangle.vertices.map(xy)), &aabb(&poly))
                        {
                            continue;
                        }
                        let original = triangle.vertices;
                        let mut clipped = original.to_vec();
                        let sign = cross(poly[0], poly[1], poly[2]).signum();
                        for j in 0..4 {
                            clipped = crate::roads::split(&clipped, |p| {
                                cross(poly[j], poly[(j + 1) % 4], xy(p)) * sign
                            })
                            .0;
                        }
                        clipped = clipped
                            .into_iter()
                            .map(|p| crate::roads::on_plane(&original, p))
                            .collect();
                        for j in 1..clipped.len().saturating_sub(1) {
                            let top = [clipped[0], clipped[j], clipped[j + 1]]
                                .map(|p| [p[0], p[1] + 12, p[2]]);
                            b.triangle(top, Surface::Concrete, &id, true)?;
                        }
                        for j in 0..clipped.len() {
                            let a = clipped[j];
                            let c = clipped[(j + 1) % clipped.len()];
                            b.quad(
                                [a, c, [c[0], c[1] + 12, c[2]], [a[0], a[1] + 12, a[2]]],
                                Surface::Concrete,
                                &id,
                                false,
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn emit_placement(
    d: &MapDocument,
    cell: Cell,
    p: &Placement,
    b: &mut Builder,
    unclipped: bool,
) -> Result<()> {
    let saved = b.bounds.clone();
    if unclipped {
        b.bounds = d.bounds.clone();
    } // one owner emits the entire small trunk
    for proxy in proxies(d, p) {
        b.box_shape(proxy.center, proxy.size_cm, &p.id)?;
    }
    if let Some(asset) = d.assets.iter().find(|a| a.id == p.asset_id) {
        for shape in &asset.convex_collision {
            b.convex_shape(shape.placed(p), &p.id)?;
        }
    }
    b.bounds = saved;
    if d.cell_at(xy(p.position)) == Some(cell) {
        b.chunk.objects.push(GeneratedObject {
            id: p.id.clone(),
            asset_id: p.asset_id.clone(),
            position: p.position,
            quarter_turns: p.quarter_turns,
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
fn tree_footprint(p: Point) -> [Point; 4] {
    rectangle(
        [p[0] - TREE_RADIUS, p[1] - TREE_RADIUS],
        [p[0] + TREE_RADIUS, p[1] + TREE_RADIUS],
    )
}
fn eligible(
    d: &MapDocument,
    c: &Candidate,
    occupied: &[Vec<Point>],
    work: &mut usize,
) -> Result<bool> {
    let zone = &d.zones[c.zone];
    let poly = tree_footprint(c.p);
    if !inside(&poly, &zone.polygon, work)? || !source_clear(d, &poly, 100, work)? {
        return Ok(false);
    }
    for excluded in &zone.exclusions {
        if polygons_overlap(&poly, excluded, work)? {
            return Ok(false);
        }
    }
    for p in occupied {
        if polygons_overlap(&poly, p, work)? {
            return Ok(false);
        }
    }
    // Reserve the authored/automatic sidewalk width, even when a piece is
    // suppressed. Vegetation must never occupy a future access strip.
    for road in &d.roads {
        if road_overlap(&poly, road, i64::from(sidewalk_width(d, road)) + 100, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn vegetation(
    d: &MapDocument,
    cell: Cell,
    b: &mut Builder,
    occupied: &[Vec<Point>],
    work: &mut usize,
) -> Result<()> {
    for (zi, zone) in d.zones.iter().enumerate() {
        let s = i64::from(zone.spacing_cm);
        let area = aabb(&zone.polygon);
        if !overlaps(&area, &b.bounds) || zone.density_per_mille == 0 {
            continue;
        }
        let mut count = 0;
        for y in b.bounds.min[1].div_euclid(s) - 1..=b.bounds.max[1].div_euclid(s) + 1 {
            for x in b.bounds.min[0].div_euclid(s) - 1..=b.bounds.max[0].div_euclid(s) + 1 {
                count += 1;
                tick(work, 1)?;
                if count > 300_000 {
                    return Err(error("E_BUDGET", "zone candidate limit"));
                }
                let Some(c) = candidate(d, zi, x, y)? else {
                    continue;
                };
                if d.cell_at(c.p) != Some(cell) || !eligible(d, &c, occupied, work)? {
                    continue;
                }
                let mut blocked = false;
                // Source-global local-minimum thinning. A lower-ranked eligible
                // competitor wins even outside this cell; generation order and
                // rejection chains cannot create seam duplicates or overlap.
                for (other_index, other) in d.zones.iter().enumerate() {
                    let reach =
                        i64::from(zone.spacing_cm.max(other.spacing_cm)).max(TREE_RADIUS * 2 + 1);
                    let other_s = i64::from(other.spacing_cm);
                    let jitter = if other.kind == ZoneKind::Forest {
                        other_s / 3
                    } else {
                        0
                    };
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
                            let overlap = dx.abs() <= TREE_RADIUS as i128 * 2
                                && dy.abs() <= TREE_RADIUS as i128 * 2;
                            if (overlap || dx * dx + dy * dy < (reach as i128) * (reach as i128))
                                && eligible(d, &other_c, occupied, work)?
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
                    asset_id: "builtin:tree".into(),
                    position,
                    quarter_turns: (c.rank % 4) as u8,
                };
                emit_placement(d, cell, &p, b, true)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn generate(d: &MapDocument, cell: Cell, b: &mut Builder) -> Result<()> {
    let mut work = 0;
    if d.recipe_version >= 6 {
        crate::roads::sidewalks(d, b)?;
    } else {
        sidewalks(d, b, &mut work)?;
    }
    buildings(d, b)?;
    let mut occupied: Vec<_> = d.placements.iter().map(|p| footprint(d, p)).collect();
    for p in &d.placements {
        emit_placement(d, cell, p, b, false)?;
    }
    for p in repeated(d)? {
        let poly = footprint(d, &p);
        if !source_clear(d, &poly, 0, &mut work)? {
            continue;
        }
        let mut blocked = false;
        for prior in &occupied {
            tick(&mut work, 1)?;
            let a = aabb(&poly);
            let c = aabb(prior);
            // Declared proxy footprints are rectangles. Adjacent fence panels
            // may share an edge; they must never share positive-area interiors.
            if (0..2).all(|i| a.min[i] < c.max[i] && c.min[i] < a.max[i]) {
                blocked = true;
                break;
            }
        }
        if blocked {
            continue;
        }
        emit_placement(d, cell, &p, b, false)?;
        occupied.push(poly);
    }
    vegetation(d, cell, b, &occupied, &mut work)
}

fn validate_indexed(d: &MapDocument, road_bounds: &[Bounds], work: &mut usize) -> Result<()> {
    use crate::bounds_index::BoundsIndex;
    let building_bounds: Vec<_> = d
        .buildings
        .iter()
        .map(|b| {
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
    let roads = BoundsIndex::new(road_bounds);
    for (i, b) in d.buildings.iter().enumerate() {
        let area = aabb(&b.footprint);
        for index in buildings.query(&area, work)?.into_iter().filter(|&j| j < i) {
            let other = &d.buildings[index];
            if b.base_cm < other.base_cm + other.height_cm as i64 + roof_rise(other)
                && other.base_cm < b.base_cm + b.height_cm as i64 + roof_rise(b)
                && building_overlap(&b.footprint, other, work)?
                && building_overlap(&other.footprint, b, work)?
            {
                return Err(error(
                    "E_GEOMETRY",
                    "overlapping building footprints/height ranges",
                ));
            }
        }
        for index in roads.query(&area, work)? {
            let road = &d.roads[index];
            let hit = if b.holes.is_empty() {
                road_overlap(&b.footprint, road, 0, work)?
            } else {
                let mut hit = false;
                for triangle in crate::courtyard::triangulate(b, work)? {
                    hit |= road_overlap(&triangle, road, 0, work)?;
                }
                hit
            };
            if hit {
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
            let b = &d.buildings[j];
            if building_overlap(poly, b, work)? {
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
        for j in roads.query(area, work)? {
            if road_overlap(poly, &d.roads[j], 0, work)? {
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
