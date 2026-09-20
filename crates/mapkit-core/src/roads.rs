//! Recipe 2: explicit graph aprons and planar terrain/road subdivision.
//! Recipe 1 remains in generation.rs. All cuts use one integer half-plane rule.
use crate::generation::Builder;
use crate::*;

pub(crate) const SCRATCH_BYTES: u64 = 16 * 1024 * 1024;
const MAX_LOCAL_PATCHES: usize = 16_384;
const MAX_FRAGMENTS: usize = 16_384;
const MAX_WORK: usize = 8_000_000;
type Poly = Vec<Vertex>;
#[derive(Clone)]
struct Patch<'a> {
    v: [Vertex; 3],
    road: &'a Road,
    surface: Surface,
    terrain_join: bool,
}
struct Wall<'a> {
    a: Vertex,
    b: Vertex,
    road: &'a Road,
}
#[derive(Clone)]
struct Arm<'a> {
    road: &'a Road,
    segment: usize,
    end: usize,
    point: Vertex,
    other: Vertex,
}
type Key = (bool, String, usize);
/// A junction mouth moves at most half the widest road along its arm, then
/// half a width sideways. Two rounded coordinates add at most two centimetres.
pub(crate) fn influence_margin(d: &MapDocument) -> i64 {
    width_influence_margin(
        d.roads
            .iter()
            .flat_map(|r| &r.widths_cm)
            .copied()
            .max()
            .unwrap_or(0),
    )
}
pub(crate) fn width_influence_margin(maximum_width: u32) -> i64 {
    i64::from(maximum_width) + 2
}
fn key(r: &Road, i: usize) -> Key {
    if i == 0 {
        (false, r.from.clone(), 0)
    } else if i + 1 == r.points.len() {
        (false, r.to.clone(), 0)
    } else {
        (true, r.id.clone(), i)
    }
}
fn xy(p: Vertex) -> Point {
    [p[0], p[2]]
}
fn orient(a: Vertex, b: Vertex, c: Vertex) -> i128 {
    cross(xy(a), xy(b), xy(c))
}
fn hit(v: &[Vertex], b: &Bounds, margin: i64) -> bool {
    (0..2).all(|a| {
        v.iter().map(|p| p[a * 2]).min().unwrap() - margin <= b.max[a]
            && v.iter().map(|p| p[a * 2]).max().unwrap() + margin >= b.min[a]
    })
}

/// Adjacency uses authored endpoint IDs (and consequently their exact level).
/// A geometric crossing never creates an edge or joins distinct graph nodes.
impl MapDocument {
    pub fn connected_roads(&self, node_id: &str) -> Result<Vec<String>> {
        self.validate()?;
        if !self.nodes.iter().any(|n| n.id == node_id) {
            return Err(error("E_REFERENCE", "unknown road node"));
        }
        let mut ids: Vec<_> = self
            .roads
            .iter()
            .filter(|r| r.from == node_id || r.to == node_id)
            .map(|r| r.id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}
pub(crate) fn validate_graph(d: &MapDocument) -> Result<()> {
    let mut degree = BTreeMap::new();
    for r in &d.roads {
        for id in [&r.from, &r.to] {
            let n = degree.entry(id).or_insert(0usize);
            *n += 1;
            if *n > 32 {
                return Err(error("E_LIMIT", "recipe 2 junction exceeds 32 arms"));
            }
        }
        if r.from == r.to && r.points.len() == 2 {
            return Err(error("E_GEOMETRY", "road loop needs intermediate points"));
        }
    }
    Ok(())
}
pub(crate) fn tick(work: &mut usize, n: usize) -> Result<()> {
    *work = work.saturating_add(n);
    if *work > MAX_WORK {
        Err(error("E_BUDGET", "recipe 2 subdivision work exceeded"))
    } else {
        Ok(())
    }
}
// Both halves share the exact same intersection, independent of traversal.
pub(crate) fn split(poly: &[Vertex], plane: impl Fn(Vertex) -> i128) -> (Poly, Poly) {
    let mut inside = vec![];
    let mut outside = vec![];
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = plane(a);
        let db = plane(b);
        if da >= 0 {
            inside.push(a);
        }
        if da <= 0 {
            outside.push(a);
        }
        if (da < 0 && db > 0) || (da > 0 && db < 0) {
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            let da = plane(a);
            let db = plane(b);
            let p = std::array::from_fn(|j| {
                ((a[j] as i128 * (da - db) + (b[j] - a[j]) as i128 * da) / (da - db)) as i64
            });
            inside.push(p);
            outside.push(p);
        }
    }
    inside.dedup();
    outside.dedup();
    if inside.len() > 1 && inside.first() == inside.last() {
        inside.pop();
    }
    if outside.len() > 1 && outside.first() == outside.last() {
        outside.pop();
    }
    (inside, outside)
}
pub(crate) fn valid(poly: &[Vertex]) -> bool {
    poly.len() >= 3 && (1..poly.len() - 1).any(|i| orient(poly[0], poly[i], poly[i + 1]) != 0)
}
pub(crate) fn partition(
    poly: &[Vertex],
    clip: &[Vertex; 3],
    work: &mut usize,
) -> Result<(Poly, Vec<Poly>)> {
    tick(work, poly.len() * 3)?;
    let sign = orient(clip[0], clip[1], clip[2]).signum();
    if sign == 0 {
        return Ok((vec![], vec![poly.to_vec()]));
    }
    let mut remaining = poly.to_vec();
    let mut outside = vec![];
    for i in 0..3 {
        let (a, b) = (clip[i], clip[(i + 1) % 3]);
        let (yes, no) = split(&remaining, |p| orient(a, b, p) * sign);
        if valid(&no) {
            outside.push(no);
        }
        remaining = yes;
        if !valid(&remaining) {
            return Ok((vec![], outside));
        }
    }
    Ok((remaining, outside))
}
pub(crate) fn emit(
    b: &mut Builder,
    poly: &[Vertex],
    surface: Surface,
    id: &str,
    spawn: bool,
) -> Result<()> {
    for i in 1..poly.len().saturating_sub(1) {
        b.triangle([poly[0], poly[i], poly[i + 1]], surface, id, spawn)?;
    }
    Ok(())
}
fn floor_plane(v: &[Vertex; 3], p: Vertex) -> i128 {
    let area = orient(v[0], v[1], v[2]);
    let value = orient(v[1], v[2], p) * v[0][1] as i128
        + orient(v[2], v[0], p) * v[1][1] as i128
        + orient(v[0], v[1], p) * v[2][1] as i128;
    (p[1] as i128 * area - value) * area.signum()
}
pub(crate) fn on_plane(v: &[Vertex; 3], p: Vertex) -> Vertex {
    let area = orient(v[0], v[1], v[2]);
    let height = (orient(v[1], v[2], p) * v[0][1] as i128
        + orient(v[2], v[0], p) * v[1][1] as i128
        + orient(v[0], v[1], p) * v[2][1] as i128)
        / area;
    [p[0], height as i64, p[2]]
}
fn hull(mut points: Vec<Vertex>) -> Vec<Vertex> {
    points.sort_by_key(|p| (p[0], p[2], p[1]));
    points.dedup_by_key(|p| (p[0], p[2]));
    if points.len() < 3 {
        return points;
    }
    let mut out = vec![];
    for p in &points {
        while out.len() >= 2 && orient(out[out.len() - 2], out[out.len() - 1], *p) < 0 {
            out.pop();
        }
        out.push(*p);
    }
    let n = out.len();
    for p in points.iter().rev().skip(1) {
        while out.len() > n && orient(out[out.len() - 2], out[out.len() - 1], *p) < 0 {
            out.pop();
        }
        out.push(*p);
    }
    out.pop();
    out
}
fn plan<'a>(d: &'a MapDocument, bounds: &Bounds) -> Result<(Vec<Patch<'a>>, Vec<Wall<'a>>)> {
    let mut groups: BTreeMap<Key, Vec<Arm>> = BTreeMap::new();
    // Include complete endpoint junctions when a corridor touches this cell.
    let mut relevant = BTreeSet::new();
    let mut local_segments = 0;
    let margin = influence_margin(d);
    let width = |r: &Road, i: usize| r.widths_cm[i] as f64;
    for r in &d.roads {
        for (i, s) in r.points.windows(2).enumerate() {
            if hit(s, bounds, margin) {
                local_segments += 1;
                if local_segments > MAX_LOCAL_PATCHES / 8 {
                    return Err(error("E_BUDGET", "recipe 2 local segment limit"));
                }
                relevant.insert(key(r, i));
                relevant.insert(key(r, i + 1));
            }
        }
    }
    let mut arm_count = 0;
    for r in &d.roads {
        for (i, s) in r.points.windows(2).enumerate() {
            for end in 0..2 {
                let k = key(r, i + end);
                if relevant.contains(&k) {
                    arm_count += 1;
                    if arm_count > MAX_LOCAL_PATCHES {
                        return Err(error("E_BUDGET", "recipe 2 local arm limit"));
                    }
                    groups.entry(k).or_default().push(Arm {
                        road: r,
                        segment: i,
                        end,
                        point: s[end],
                        other: s[1 - end],
                    });
                }
            }
        }
    }
    let mut mouths: BTreeMap<(&str, usize, usize), [Vertex; 2]> = BTreeMap::new();
    let mut patches = vec![];
    let mut walls = vec![];
    for arms in groups.values() {
        let radius = arms
            .iter()
            .map(|a| width(a.road, a.segment) / 2.0)
            .fold(0.0, f64::max);
        let mut ring = vec![];
        for arm in arms {
            let dx = (arm.other[0] - arm.point[0]) as f64;
            let dy = (arm.other[2] - arm.point[2]) as f64;
            let len = libm::sqrt(dx * dx + dy * dy);
            let t = if arms.len() > 1 {
                radius.min(len * 0.45) / len
            } else {
                0.0
            };
            let center: Vertex = std::array::from_fn(|i| {
                arm.point[i] + libm::round((arm.other[i] - arm.point[i]) as f64 * t) as i64
            });
            let half = width(arm.road, arm.segment) / 2.0;
            let offset = [
                libm::round(-dy / len * half) as i64,
                libm::round(dx / len * half) as i64,
            ];
            let mouth = [
                [center[0] + offset[0], center[1], center[2] + offset[1]],
                [center[0] - offset[0], center[1], center[2] - offset[1]],
            ];
            mouths.insert((&arm.road.id, arm.segment, arm.end), mouth);
            ring.extend(mouth);
        }
        if arms.len() > 1 {
            let ring = hull(ring);
            let structural = arms.iter().any(|a| a.road.kind != RoadKind::Ground);
            let terrain_join = structural && arms.iter().any(|a| a.road.kind == RoadKind::Ground);
            if structural {
                for arm in arms {
                    let m = mouths[&(arm.road.id.as_str(), arm.segment, arm.end)];
                    if !ring
                        .iter()
                        .enumerate()
                        .any(|(i, a)| m.contains(a) && m.contains(&ring[(i + 1) % ring.len()]))
                    {
                        return Err(error(
                            "E_GEOMETRY",
                            format!("overlapping structural junction mouths at {} segment {}; author separated approaches", arm.road.id, arm.segment),
                        ));
                    }
                }
                let ceilings: BTreeSet<_> = arms
                    .iter()
                    .filter(|a| a.road.kind == RoadKind::Tunnel)
                    .map(|a| a.road.clearance_cm)
                    .collect();
                if ceilings.len() > 1 {
                    return Err(error("E_GEOMETRY", "joined tunnel clearances must agree"));
                }
            }
            for i in 0..ring.len() {
                let a = ring[i];
                let c = ring[(i + 1) % ring.len()];
                let owner = arms
                    .iter()
                    .find(|arm| mouths[&(arm.road.id.as_str(), arm.segment, arm.end)].contains(&a))
                    .unwrap();
                patches.push(Patch {
                    v: [arms[0].point, a, c],
                    road: owner.road,
                    surface: owner.road.surfaces[owner.segment],
                    terrain_join,
                });
                let is_mouth = arms.iter().any(|arm| {
                    let m = mouths[&(arm.road.id.as_str(), arm.segment, arm.end)];
                    m.contains(&a) && m.contains(&c)
                });
                if !is_mouth && matches!(owner.road.kind, RoadKind::Tunnel | RoadKind::Underpass) {
                    walls.push(Wall {
                        a,
                        b: c,
                        road: owner.road,
                    });
                }
            }
        }
        if patches.len() + walls.len() > MAX_LOCAL_PATCHES {
            return Err(error("E_BUDGET", "recipe 2 local junction limit"));
        }
    }
    for r in &d.roads {
        for (i, s) in r.points.windows(2).enumerate() {
            if !hit(s, bounds, margin) {
                continue;
            }
            let a = mouths[&(r.id.as_str(), i, 0)];
            let c = mouths[&(r.id.as_str(), i, 1)];
            let v = [a[0], c[1], c[0], a[1]];
            for t in [[v[0], v[1], v[2]], [v[0], v[2], v[3]]] {
                patches.push(Patch {
                    v: t,
                    road: r,
                    surface: r.surfaces[i],
                    terrain_join: false,
                });
            }
            if matches!(r.kind, RoadKind::Tunnel | RoadKind::Underpass) {
                for (a, b) in [(v[0], v[1]), (v[2], v[3])] {
                    walls.push(Wall { a, b, road: r });
                }
            }
            if patches.len() + walls.len() > MAX_LOCAL_PATCHES {
                return Err(error("E_BUDGET", "recipe 2 local corridor limit"));
            }
        }
    }
    patches.retain(|p| hit(&p.v, bounds, 0) && orient(p.v[0], p.v[1], p.v[2]) != 0);
    let order = |p: &Patch| {
        if matches!(p.road.kind, RoadKind::Tunnel | RoadKind::Underpass) {
            0
        } else {
            1
        }
    };
    patches.sort_by(|a, b| (order(a), &a.road.id, a.v).cmp(&(order(b), &b.road.id, b.v)));
    Ok((patches, walls))
}

// Recipe 9 cuts each terrain tile once, before assigning surface identities.
fn ground_tile(
    d: &MapDocument,
    v: [Vertex; 4],
    patches: &[Patch],
    b: &mut Builder,
    work: &mut usize,
) -> Result<()> {
    let bounds = Bounds {
        min: xy(v[0]),
        max: [v[2][0].min(b.bounds.max[0]), v[2][2].min(b.bounds.max[1])],
    };
    if (0..2).any(|i| bounds.min[i] >= bounds.max[i]) {
        return Ok(());
    }
    tick(
        work,
        patches.len()
            + d.surface_areas
                .iter()
                .map(|a| a.polygon.len())
                .sum::<usize>(),
    )?;
    let terrain = [[v[0], v[1], v[2]], [v[0], v[2], v[3]]];
    let nearby: Vec<_> = patches.iter().filter(|p| hit(&p.v, &bounds, 2)).collect();
    let mut lines = vec![(xy(v[0]), xy(v[2]))];
    for patch in &nearby {
        for i in 0..3 {
            lines.push((xy(patch.v[i]), xy(patch.v[(i + 1) % 3])));
        }
        for t in terrain {
            if patch.terrain_join {
                let (inside, _) = partition(&t, &patch.v, work)?;
                if inside
                    .iter()
                    .any(|p| (on_plane(&patch.v, *p)[1] - on_plane(&t, *p)[1]).abs() > 1)
                {
                    return Err(error("E_GEOMETRY", "ground/structure junction apron must match terrain; author a level approach"));
                }
            }
            if patch.road.kind == RoadKind::Tunnel {
                let ceiling = patch
                    .v
                    .map(|p| [p[0], p[1] + patch.road.clearance_cm.unwrap() as i64, p[2]]);
                let mut crossing = Vec::new();
                for i in 0..3 {
                    let (a, c) = (t[i], t[(i + 1) % 3]);
                    let (da, dc) = (floor_plane(&ceiling, a), floor_plane(&ceiling, c));
                    if da == 0 {
                        crossing.push(xy(a));
                    }
                    if (da < 0 && dc > 0) || (da > 0 && dc < 0) {
                        let p = std::array::from_fn(|j| {
                            ((a[j] as i128 * (da - dc) + (c[j] - a[j]) as i128 * da) / (da - dc))
                                as i64
                        });
                        crossing.push(xy(p));
                    }
                }
                crossing.sort();
                crossing.dedup();
                if crossing.len() >= 2 {
                    lines.push((crossing[0], *crossing.last().unwrap()));
                }
            }
        }
    }
    let paint: Vec<_> = d
        .surface_areas
        .iter()
        .filter(|a| {
            let verts: Vec<_> = a.polygon.iter().map(|p| [p[0], 0, p[1]]).collect();
            hit(&verts, &bounds, 2)
        })
        .collect();
    for area in &paint {
        for i in 0..area.polygon.len() {
            lines.push((area.polygon[i], area.polygon[(i + 1) % area.polygon.len()]));
        }
    }
    let shapes = crate::road_arrangement::subdivide(&bounds, &lines, work)?;
    let height = |p: Point| {
        let t = if (p[0] - bounds.min[0]) * (v[2][2] - v[0][2])
            >= (p[1] - bounds.min[1]) * (v[2][0] - v[0][0])
        {
            terrain[0]
        } else {
            terrain[1]
        };
        on_plane(&t, [p[0], 0, p[1]])
    };
    for rings in shapes {
        for triangle in crate::road_arrangement::triangulate(&rings, work)? {
            let center = [
                triangle.iter().map(|p| p[0]).sum::<i64>(),
                triangle.iter().map(|p| p[1]).sum::<i64>(),
            ];
            let t = if (center[0] - 3 * bounds.min[0]) * (v[2][2] - v[0][2])
                >= (center[1] - 3 * bounds.min[1]) * (v[2][0] - v[0][0])
            {
                terrain[0]
            } else {
                terrain[1]
            };
            let mut selected = Some((Surface::Grass, "terrain"));
            let mut road = false;
            for patch in &nearby {
                tick(work, 1)?;
                if !point_in_polygon(center, &patch.v.map(|p| [p[0] * 3, p[2] * 3])) {
                    continue;
                }
                match patch.road.kind {
                    RoadKind::Ground => {
                        selected = Some((patch.surface, patch.road.id.as_str()));
                        road = true;
                        break;
                    }
                    RoadKind::Underpass => {
                        selected = None;
                        break;
                    }
                    RoadKind::Tunnel => {
                        let ceiling = patch.v.map(|p| {
                            [
                                p[0] * 3,
                                (p[1] + patch.road.clearance_cm.unwrap() as i64) * 3,
                                p[2] * 3,
                            ]
                        });
                        let terrain3 = t.map(|p| p.map(|v| v * 3));
                        if floor_plane(&ceiling, on_plane(&terrain3, [center[0], 0, center[1]]))
                            <= 0
                        {
                            selected = None;
                            break;
                        }
                    }
                    RoadKind::Elevated | RoadKind::Bridge => {
                        if patch.v.iter().all(|p| floor_plane(&t, *p) == 0) {
                            selected = None;
                            break;
                        }
                    }
                }
            }
            if !road && selected.is_some() {
                for area in &paint {
                    tick(work, area.polygon.len())?;
                    if point_in_polygon(
                        center,
                        &area
                            .polygon
                            .iter()
                            .map(|p| [p[0] * 3, p[1] * 3])
                            .collect::<Vec<_>>(),
                    ) {
                        selected = Some((area.surface, area.id.as_str()));
                        break;
                    }
                }
            }
            if let Some((surface, id)) = selected {
                b.triangle(triangle.map(height), surface, id, true)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn generate(
    d: &MapDocument,
    bounds: &Bounds,
    grid: Option<&HeightGrid>,
    spacing: i64,
    side: usize,
    b: &mut Builder,
) -> Result<()> {
    let (patches, walls) = plan(d, bounds)?;
    let height =
        |x: usize, y: usize| grid.map_or(d.terrain_base_cm, |g| g.heights_cm[y * side + x]);
    let mut work = 0;
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let px = bounds.min[0] + x as i64 * spacing;
            let py = bounds.min[1] + y as i64 * spacing;
            let v = [
                [px, height(x, y), py],
                [px + spacing, height(x + 1, y), py],
                [px + spacing, height(x + 1, y + 1), py + spacing],
                [px, height(x, y + 1), py + spacing],
            ];

            ground_tile(d, v, &patches, b, &mut work)?;
        }
    }
    for patch in &patches {
        if patch.road.kind == RoadKind::Ground {
            continue;
        }
        b.triangle(patch.v, patch.surface, &patch.road.id, true)?;
        if patch.road.kind == RoadKind::Tunnel {
            b.triangle(
                patch
                    .v
                    .map(|p| [p[0], p[1] + patch.road.clearance_cm.unwrap() as i64, p[2]]),
                Surface::Concrete,
                &patch.road.id,
                false,
            )?;
        }
    }
    for wall in walls {
        if !hit(&[wall.a, wall.b], bounds, 0) {
            continue;
        }
        let h = wall.road.clearance_cm.unwrap() as i64;
        if wall.road.kind == RoadKind::Tunnel {
            b.quad(
                [
                    wall.a,
                    wall.b,
                    [wall.b[0], wall.b[1] + h, wall.b[2]],
                    [wall.a[0], wall.a[1] + h, wall.a[2]],
                ],
                Surface::Concrete,
                &wall.road.id,
                false,
            )?;
        } else {
            for y in 0..side - 1 {
                for x in 0..side - 1 {
                    tick(&mut work, 1)?;
                    let px = bounds.min[0] + x as i64 * spacing;
                    let py = bounds.min[1] + y as i64 * spacing;
                    if !hit(
                        &[wall.a, wall.b],
                        &Bounds {
                            min: [px, py],
                            max: [px + spacing, py + spacing],
                        },
                        0,
                    ) {
                        continue;
                    }
                    let v = [
                        [px, height(x, y), py],
                        [px + spacing, height(x + 1, y), py],
                        [px + spacing, height(x + 1, y + 1), py + spacing],
                        [px, height(x, y + 1), py + spacing],
                    ];
                    for terrain in [[v[0], v[1], v[2]], [v[0], v[2], v[3]]] {
                        let mut line = vec![wall.a, wall.b];
                        for i in 0..3 {
                            line = split(&line, |p| orient(terrain[i], terrain[(i + 1) % 3], p)).0;
                            line.dedup();
                        }
                        if line.len() < 2 {
                            continue;
                        }
                        let a = *line.first().unwrap();
                        let c = *line.last().unwrap();
                        if xy(a) == xy(c) {
                            continue;
                        }
                        let top = |p: Vertex| {
                            let terrain_height = on_plane(&terrain, p)[1];
                            [p[0], (p[1] + h).max(terrain_height), p[2]]
                        };
                        let roof = terrain.map(|p| [p[0], p[1] - h, p[2]]);
                        let (lo, hi) = split(&[a, c], |p| floor_plane(&roof, p));
                        for part in [lo, hi] {
                            if part.len() >= 2 {
                                let a = part[0];
                                let c = *part.last().unwrap();
                                if xy(a) != xy(c) {
                                    b.quad(
                                        [a, c, top(c), top(a)],
                                        Surface::Concrete,
                                        &wall.road.id,
                                        false,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Recipe 6 widened graph aprons cover corners; only restored ground receives
/// sidewalk tops. Road carriageways, independent decks and portals are untouched.
pub(crate) fn sidewalks(d: &MapDocument, b: &mut Builder) -> Result<()> {
    let expanded = Bounds {
        min: b.bounds.min.map(|v| v - 1000),
        max: b.bounds.max.map(|v| v + 1000),
    };
    let (original, _) = plan(d, &expanded)?;
    let mut patches = vec![];
    for patch in original {
        let width = crate::placement::sidewalk_width(d, patch.road) as i64;
        if width == 0 {
            continue;
        }
        let diagonal = libm::round(width as f64 / libm::sqrt(2.0)) as i64;
        let offsets = [
            [width, 0],
            [diagonal, diagonal],
            [0, width],
            [-diagonal, diagonal],
            [-width, 0],
            [-diagonal, -diagonal],
            [0, -width],
            [diagonal, -diagonal],
        ];
        let ring = hull(
            patch
                .v
                .iter()
                .flat_map(|p| offsets.map(|o| [p[0] + o[0], p[1], p[2] + o[1]]))
                .collect(),
        );
        for i in 1..ring.len().saturating_sub(1) {
            patches.push(Patch {
                v: [ring[0], ring[i], ring[i + 1]],
                ..patch.clone()
            });
        }
    }
    if patches.is_empty() {
        return Ok(());
    }
    let exclusions: Vec<_> = d
        .buildings
        .iter()
        .flat_map(|b| std::iter::once(&b.footprint).chain(&b.entrances))
        .collect();
    let exclusion_index = crate::bounds_index::BoundsIndex::new(
        &exclusions
            .iter()
            .map(|p| crate::bounds_index::bounds(p))
            .collect::<Vec<_>>(),
    );
    let ground: Vec<_> = b
        .chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == "terrain" || d.surface_areas.iter().any(|a| a.id == t.object_id))
        .cloned()
        .collect();
    let mut work = 0;
    for t in ground {
        let area = crate::bounds_index::bounds(&t.vertices.map(xy));
        let nearby = exclusion_index.query(&area, &mut work)?;
        let mut remaining = vec![t.vertices.to_vec()];
        for patch in &patches {
            tick(&mut work, 1)?;
            if !hit(&patch.v, &area, 0) {
                continue;
            }
            let mut next = vec![];
            for poly in remaining {
                let (inside, outside) = partition(&poly, &patch.v, &mut work)?;
                next.extend(outside);
                if valid(&inside) {
                    let mut fitted = vec![inside];
                    for &i in &nearby {
                        fitted = crate::urban::subtract(fitted, exclusions[i], &mut work)?;
                    }
                    let id = format!("{}:sidewalk", patch.road.id);
                    for p in fitted {
                        let bottom: Vec<_> = p.iter().map(|p| on_plane(&t.vertices, *p)).collect();
                        let top: Vec<_> = bottom.iter().map(|p| [p[0], p[1] + 12, p[2]]).collect();
                        emit(b, &top, Surface::Concrete, &id, true)?;
                        for i in 0..top.len() {
                            let j = (i + 1) % top.len();
                            b.quad(
                                [bottom[i], bottom[j], top[j], top[i]],
                                Surface::Concrete,
                                &id,
                                false,
                            )?;
                        }
                    }
                }
            }
            remaining = next;
            if remaining.len() > MAX_FRAGMENTS {
                return Err(error("E_BUDGET", "sidewalk fragments exceeded"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod arrangement_probe {
    use super::*;

    #[test]
    fn simultaneous_integer_cuts_preserve_curved_cell_area() {
        for shift in [[0, 0], [-837, -851], [999_990_000, -999_990_000]] {
            for widths in [
                [601, 799, 503],
                [599, 801, 501],
                [101, 99, 103],
                [997, 503, 799],
            ] {
                check_case(shift, widths);
            }
        }
    }

    fn check_case(shift: Point, widths: [u32; 3]) {
        let mut d: MapDocument =
            serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        d.roads.retain(|r| r.id == "ground-west");
        d.nodes
            .retain(|n| ["ground-west-from", "junction"].contains(&n.id.as_str()));
        d.roads[0].points = vec![
            [0, 0, 1000],
            [1801, 0, 1397],
            [3203, 0, 701],
            [5000, 0, 1000],
        ];
        d.roads[0].widths_cm = widths.to_vec();
        d.roads[0].surfaces = vec![Surface::Asphalt; 3];
        let bounds = d.cell_bounds(Cell { x: 0, y: 0 }).unwrap();
        let (patches, _) = plan(&d, &bounds).unwrap();
        let point = |p: Point| [p[0] + shift[0], p[1] + shift[1]];
        let mut work = 0;
        let mut shapes = Vec::new();
        for y in (0..5000).step_by(500) {
            for x in (0..5000).step_by(500) {
                let tile = Bounds {
                    min: [x, y],
                    max: [x + 500, y + 500],
                };
                let mut lines = vec![(point([x, y]), point([x + 500, y + 500]))];
                for patch in patches.iter().filter(|p| hit(&p.v, &tile, 2)) {
                    for i in 0..3 {
                        lines.push((point(xy(patch.v[i])), point(xy(patch.v[(i + 1) % 3]))));
                    }
                }
                let shifted = Bounds {
                    min: point(tile.min),
                    max: point(tile.max),
                };
                shapes.extend(
                    crate::road_arrangement::subdivide(&shifted, &lines, &mut work).unwrap(),
                );
            }
        }
        let mut area = 0i128;
        let mut triangles = Vec::new();
        for shape in &shapes {
            assert_eq!(shape.len(), 1, "probe faces are simply connected");
            let ring = shape[0].clone();
            for t in crate::generation::polygon_triangles(&ring).unwrap() {
                triangles.push(t.map(|i| ring[i]));
            }
            for (index, ring) in shape.iter().enumerate() {
                let a: i128 = (0..ring.len())
                    .map(|i| {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
                    })
                    .sum();
                area += if index == 0 { a.abs() } else { -a.abs() };
            }
        }
        assert_eq!(area, 2 * 5000 * 5000);
        assert_eq!(
            triangles
                .iter()
                .map(|t| cross(t[0], t[1], t[2]).abs())
                .sum::<i128>(),
            area
        );
        let mut edges = BTreeMap::new();
        for triangle in &triangles {
            for i in 0..3 {
                let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
                *edges
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_insert(0) += 1;
            }
        }
        for ((a, b), count) in edges {
            let outer = (0..2).any(|i| a[i] == b[i] && [shift[i], shift[i] + 5000].contains(&a[i]));
            assert_eq!(
                count,
                if outer { 1 } else { 2 },
                "unmatched internal edge {a:?}..{b:?}"
            );
        }
        eprintln!(
            "simultaneous arrangement: {} faces; exact area {}",
            shapes.len(),
            area
        );
    }
}
