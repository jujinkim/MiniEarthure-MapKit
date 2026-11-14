//! Bounded per-triangle presentation references; never alter the collision archive.
use godot::prelude::*;
use mapkit_core::{MapDocument, RoadPaint};
use std::collections::BTreeMap;

/// Paint over the shader cap degrades display only; Authoring scopes warn.
/// `joined` marks a degree-two join that fell back to the road's own path.
fn note_overflow(count: usize, joined: bool) {
    mapkit_core::profile::note(mapkit_core::Warning {
        code: if joined { "road_paint_join" } else { "road_paint_segments" }.into(),
        actual: count as u64,
        limit: RoadPaint::SEGMENTS as u64,
        message: if joined {
            "Joined road paint over the 128-segment shader cap falls back to each road's own path"
        } else {
            "Road paint over the 128-segment shader cap is omitted in this cell"
        }.into(),
        runtime_rejects: false,
    });
}

pub fn decorate(
    d: &MapDocument,
    chunk: &VarDictionary,
    presentation: &mut VarDictionary,
) -> mapkit_core::Result<()> {
    let roads: BTreeMap<_, _> = d
        .roads
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut faces = Vec::new();
    // Packed cells only: the legacy dictionary path builds its own UVs.
    let mut corners: Vec<[[i64; 3]; 3]> = Vec::new();
    if let Some(value) = chunk.get("geometry") {
        let mut geometry = value.to::<Gd<RefCounted>>();
        let view = geometry.call("view", &[]).to::<VarDictionary>();
        let ids = view.get("object_ids").unwrap().to::<PackedStringArray>();
        let indices = view.get("object_indices").unwrap().to::<PackedInt32Array>();
        let vertices = view.get("vertices_cm").unwrap().to::<PackedInt64Array>();
        let spawn = view.get("spawnable").unwrap().to::<PackedByteArray>();
        let surfaces = view.get("surface_indices").unwrap().to::<PackedByteArray>();
        let flat = vertices.as_slice();
        corners = (0..indices.len()).map(|i| std::array::from_fn(|k| [flat[i * 9 + k * 3], flat[i * 9 + k * 3 + 1], flat[i * 9 + k * 3 + 2]])).collect();
        for i in 0..indices.len() {
            let p = Vector2::new(
                (vertices[i * 9] + vertices[i * 9 + 3] + vertices[i * 9 + 6]) as f32 / 300.0,
                -(vertices[i * 9 + 2] + vertices[i * 9 + 5] + vertices[i * 9 + 8]) as f32 / 300.0,
            );
            faces.push((
                ids[indices[i] as usize].to_string(),
                p,
                spawn[i] != 0,
                surfaces[i],
            ));
        }
    } else if let Some(value) = chunk.get("triangles") {
        for value in value.to::<Array<Variant>>().iter_shared() {
            let t = value.to::<VarDictionary>();
            let v = t.get("vertices").unwrap().to::<Array<Variant>>();
            let mut p = Vector2::ZERO;
            for point in v.iter_shared() {
                let a = point.to::<Array<Variant>>();
                p += Vector2::new(a.at(0).to::<f64>() as f32, -a.at(2).to::<f64>() as f32) / 300.0;
            }
            let surface = t.get("surface").unwrap().to::<GString>().to_string();
            faces.push((
                t.get("object_id").unwrap().to::<GString>().to_string(),
                p,
                t.get("spawnable").unwrap().to::<bool>(),
                match surface.as_str() {
                    "asphalt" => 0, "concrete" => 1, "dirt" => 2, "gravel" => 3, _ => 4,
                },
            ));
        }
    }
    let mut keys = PackedStringArray::new();
    let mut styles = VarDictionary::new();
    let cell = chunk.get("cell").unwrap().to::<VarDictionary>();
    let bounds = d.cell_bounds(mapkit_core::Cell {
        x: cell.get("x").unwrap().to::<i32>(),
        y: cell.get("y").unwrap().to::<i32>(),
    })?;
    let paint = d.road_paint(&bounds)?;
    if let Some(&count) = paint.overflow.values().max() {
        note_overflow(count, false);
    }
    if !corners.is_empty() {
        let ids: Vec<&str> = faces.iter().map(|f| f.0.as_str()).collect();
        let (triangles, frames) = sidewalk_frames(&paint, &roads, &ids, &corners);
        if !triangles.is_empty() {
            presentation.set("sidewalk_triangles", &PackedInt32Array::from(triangles.as_slice()));
            presentation.set("sidewalk_frames", &PackedVector4Array::from(frames.as_slice()));
        }
        // Display-only street life on the same tops (walk_lanes.rs).
        let surfaces: Vec<u8> = faces.iter().map(|f| f.3).collect();
        let walk = crate::walk_lanes::build(d, &paint, &roads, &ids, &corners, &surfaces, &bounds, &crate::walk_lanes::CAPS);
        walk.note();
        walk.publish(presentation);
    }
    for (id, _p, spawn, surface) in faces {
        let mut key = if id.ends_with(":safety:metal") {
            "safety:metal".into()
        } else {
            String::new()
        };
        if let Some(r) = roads.get(id.as_str()).filter(|_| spawn) {
            key = format!(
                "road:{}:{}:{}:{}",
                r.id, bounds.min[0], bounds.min[1], surface
            );
            if !styles.contains_key(key.as_str()) {
                let unmarked = mapkit_core::RoadMarkings { color: None, lanes: 1, center_line: false, edge_lines: false, crosswalk_start: false, crosswalk_end: false, one_way: 0, stop_lines: false, lane_width_cm: 0 , median_width_cm: 0, forward_lanes: 0};
                let m = r.markings.as_ref().unwrap_or(&unmarked);
                let mut paths = PackedVector4Array::new();
                let mut metrics = PackedVector4Array::new();
                let mut borders = PackedVector4Array::new();
                let segment = |a: mapkit_core::Vertex, b: mapkit_core::Vertex| {
                    Vector4::new(
                        a[0] as f32 / 100.0,
                        -a[2] as f32 / 100.0,
                        b[0] as f32 / 100.0,
                        -b[2] as f32 / 100.0,
                    )
                };
                let (path, overflow) = paint.joined(d, r);
                if let Some(count) = overflow {
                    note_overflow(count, true);
                }
                for s in path {
                    paths.push(segment(s.a, s.b));
                    metrics.push(Vector4::new(
                        s.width_cm as f32 / 100.0,
                        s.station_cm as f32 / 100.0,
                        s.period_cm as f32 / 100.0,
                        s.total_cm as f32 / 100.0,
                    ));
                }
                if let Some(edges) = paint.edges.get(&r.id) {
                    for e in edges {
                        borders.push(segment(e[0], e[1]));
                    }
                }
                let (path_count, edge_count) = (paths.len(), borders.len());
                paths.resize(128);
                metrics.resize(128);
                borders.resize(128);
                // Junction ends (three or more arms) of a road with stop lines:
                // bit 1 left, 2 straight, 4 right turns from the inbound heading,
                // bit 8 marks the junction itself.
                let turns = |node: &str, at_end: bool| -> i64 {
                    if !m.stop_lines || r.points.len() < 2 { return 0; }
                    let arms: Vec<_> = d.roads.iter().filter(|o| o.id != r.id && (o.from == node || o.to == node)).collect();
                    if arms.len() < 2 { return 0; }
                    let n = r.points.len();
                    let (a, b) = if at_end { (r.points[n - 2], r.points[n - 1]) } else { (r.points[1], r.points[0]) };
                    let inbound = [(b[0] - a[0]) as f64, (b[2] - a[2]) as f64];
                    let mut mask = 8;
                    for o in arms {
                        let k = o.points.len();
                        if k < 2 { continue; }
                        let (p0, p1) = if o.from == node { (o.points[0], o.points[1]) } else { (o.points[k - 1], o.points[k - 2]) };
                        let out = [(p1[0] - p0[0]) as f64, (p1[2] - p0[2]) as f64];
                        let angle = (inbound[0] * out[1] - inbound[1] * out[0]).atan2(inbound[0] * out[0] + inbound[1] * out[1]).to_degrees();
                        mask |= if angle.abs() < 35.0 { 2 } else if (35.0..150.0).contains(&angle) { 1 } else if (-150.0..=-35.0).contains(&angle) { 4 } else { 0 };
                    }
                    mask
                };
                let left_hand = d.traffic.is_some_and(|t| t.driving_side == mapkit_core::DrivingSide::Left);
                let white_centre = d.traffic.is_some_and(|t| t.centre_line == mapkit_core::CentreLine::White);
                let tint = m.color.map(|c|Color::from_rgba(c[0] as f32/255.,c[1] as f32/255.,c[2] as f32/255.,1.));
                let mut style=vdict!{
                    "snow_retention_percent"=>r.snow_retention_percent as f64,
                    "marked"=>r.markings.is_some() && surface <= 1,
                    "road_paths"=>&paths,"road_metrics"=>&metrics,"road_borders"=>&borders,
                    "path_count"=>path_count as i64,"edge_count"=>edge_count as i64,
                    "lanes"=>m.lanes as i64,"center_line"=>m.center_line,
                    "edge_lines"=>m.edge_lines,"crosswalk_start"=>m.crosswalk_start,"crosswalk_end"=>m.crosswalk_end,"surface"=>surface as i64,
                    "stop_start"=>turns(&r.from, false),"stop_end"=>turns(&r.to, true),"one_way"=>m.one_way as i64,"left_hand"=>left_hand,"white_centre"=>white_centre,
                    "lane_width"=>m.lane_width_cm as f64 / 100.0,
                    "median_width"=>m.median_width_cm as f64 / 100.0, "forward_lanes"=>m.forward_lanes as i64,
                    // Concrete carriageways keep cast slabs; kerbed marked roads draw a gutter.
                    "slabs"=>surface == 1,"curbed"=>r.markings.is_some() && d.sidewalk_width_cm(r) > 0};
                if let Some(tint)=tint { style.set("base_color",tint); }
                if let Some(ground)=r.style_id.split('/').nth(2).and_then(|g|mapkit_core::appearance::catalog().ground.get(g)) {
                    let c=ground.color;
                    style.set("base_color",Color::from_rgb(c[0] as f32/255.,c[1] as f32/255.,c[2] as f32/255.));
                    style.set("detail_kind",ground.detail.as_str());
                    let name=r.style_id.split('/').nth(2).unwrap();
                    style.set("appearance_mode",match name {"asphalt"=>1,"concrete"=>2,"pavers"=>3,"stone_paving"=>4,_=>5});
                    style.set("paving",matches!(name,"concrete"|"pavers"|"stone_paving"));
                    style.set("slabs",name=="concrete");
                }
                styles.set(key.as_str(),&style);
            }
        }
        keys.push(&GString::from(key.as_str()));
    }
    presentation.set("road_materials", &keys);
    presentation.set("road_styles", &styles);
    presentation.set("urban_surfaces", true);
    Ok(())
}

/// Sidewalk frame kinds (`w` of a frame is the kind plus 8 times the section).
const FRAME_TOP: f32 = 1.0;
const FRAME_CROSSING: f32 = 2.0;
const FRAME_CURB: f32 = 3.0;
const FRAME_EDGE: f32 = 4.0;
/// Tops farther than this from every curb in the cell keep world-aligned paving.
const FRAME_REACH_M: f64 = 3.0;

pub(crate) type P2 = [f64; 2];
pub(crate) fn scene(v: [i64; 3]) -> P2 { [v[0] as f64 / 100.0, -(v[2] as f64) / 100.0] }
pub(crate) fn sub(a: P2, b: P2) -> P2 { [a[0] - b[0], a[1] - b[1]] }
pub(crate) fn dot(a: P2, b: P2) -> f64 { a[0] * b[0] + a[1] * b[1] }
pub(crate) fn perp(a: P2) -> P2 { [-a[1], a[0]] }
pub(crate) fn unit(a: P2) -> P2 { let l = dot(a, a).sqrt().max(1e-9); [a[0] / l, a[1] / l] }
pub(crate) fn segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
    let q = sub(p, [a[0] + ab[0] * t, a[1] + ab[1] * t]);
    dot(q, q).sqrt()
}
pub(crate) fn vertical(v: &[[i64; 3]; 3]) -> bool {
    let [a, b, c] = v.map(|p| p.map(|x| x as f64));
    let (e, f) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
    10.0 * n[1] * n[1] < n[0] * n[0] + n[1] * n[1] + n[2] * n[2]
}
pub(crate) fn inside(t: [P2; 3], p: P2) -> bool {
    let side = |a: P2, b: P2| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    let s = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}
/// A curb line: a point on it, its unit direction, and the direction the
/// paving runs along: folded into one half-plane and snapped to 0.25 degrees,
/// so the pieces of one straight curb in neighbouring cells share it.
#[derive(Clone, Copy)]
struct Line { a: P2, d: P2, q: P2 }
impl Line {
    fn new(a: P2, b: P2) -> Line {
        let d = unit(sub(b, a));
        let mut angle = d[1].atan2(d[0]);
        if angle <= -std::f64::consts::FRAC_PI_2 { angle += std::f64::consts::PI; }
        if angle > std::f64::consts::FRAC_PI_2 { angle -= std::f64::consts::PI; }
        let step = 0.25f64.to_radians();
        let snapped = (angle / step).round() * step;
        Line { a, d, q: [snapped.cos(), snapped.sin()] }
    }
    fn offset(&self, p: P2) -> f64 { dot(sub(p, self.a), perp(self.d)) }
}

/// Painted crosswalk centres: 1 m in from a crossed end of a road longer than
/// 5 m (urban_surface.gdshader draws the bars 0.3–1.7 m in), with the road's
/// unit direction there, its half width (scene metres) and its id. Shared by the
/// sidewalk frames (crossing mouths) and the street-life crossings (walk_lanes.rs).
pub(crate) fn crosswalk_centres<'a>(paint: &RoadPaint, roads: &BTreeMap<&'a str, &mapkit_core::Road>) -> Vec<(P2, P2, f64, &'a str)> {
    let mut crossings: Vec<(P2, P2, f64, &'a str)> = Vec::new();
    for (id, r) in roads {
        let Some(m) = &r.markings else { continue };
        let Some(path) = paint.paths.get(*id) else { continue };
        for (crossed, start) in [(m.crosswalk_start, true), (m.crosswalk_end, false)] {
            if !crossed { continue; }
            for s in path {
                let (a, b) = (scene(s.a), scene(s.b));
                let length = dot(sub(b, a), sub(b, a)).sqrt() * 100.0;
                let target = if start { 100.0 } else { s.total_cm - 100.0 };
                if s.total_cm <= 500.0 || length <= 0.0 || target < s.station_cm || target > s.station_cm + length { continue; }
                let t = (target - s.station_cm) / length;
                crossings.push(([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], unit(sub(b, a)), s.width_cm / 200.0, *id));
                break;
            }
        }
    }
    crossings
}

/// Display-only frames of sidewalk tops and walls (`<road>:sidewalk`) in a packed
/// cell: the sorted triangle indices and, per corner in packed scene order,
/// (along, across, crossing, kind + 8 * section) in scene metres. Tops measure
/// across from their nearest curb (a wall whose outer side is a carriageway),
/// walls carry their height above the wall bottom instead; `crossing` is the
/// offset along a crossed road from its painted crosswalk centre. Every value
/// is affine over its triangle, so the shader interpolates it exactly; `along`
/// projects onto the snapped curb direction, so a straight curb keeps one
/// paving phase across cells. Uploaded as UV/UV2 of the shared `concrete` batches.
fn sidewalk_frames(
    paint: &RoadPaint,
    roads: &BTreeMap<&str, &mapkit_core::Road>,
    ids: &[&str],
    corners: &[[[i64; 3]; 3]],
) -> (Vec<i32>, Vec<Vector4>) {
    fn sidewalk_of<'a>(id: &'a str, roads: &BTreeMap<&str, &mapkit_core::Road>) -> Option<&'a str> {
        id.strip_suffix(":sidewalk").filter(|r| roads.contains_key(r))
    }
    let owner = |id: &'_ str| sidewalk_of(id, roads).is_some();
    if !ids.iter().any(|id| owner(id)) { return (Vec::new(), Vec::new()); }
    // Carriageway faces on a 2 m grid for the outer-side probes of walls.
    const GRID: f64 = 2.0;
    let cell = |p: P2| ((p[0] / GRID).floor() as i64, (p[1] / GRID).floor() as i64);
    let mut grid = std::collections::HashMap::<(i64, i64), Vec<usize>>::new();
    for (i, v) in corners.iter().enumerate() {
        if !roads.contains_key(ids[i]) || vertical(v) { continue; }
        let p = v.map(scene);
        let lo = cell([p[0][0].min(p[1][0]).min(p[2][0]), p[0][1].min(p[1][1]).min(p[2][1])]);
        let hi = cell([p[0][0].max(p[1][0]).max(p[2][0]), p[0][1].max(p[1][1]).max(p[2][1])]);
        if (hi.0 - lo.0 + 1) * (hi.1 - lo.1 + 1) > 4096 { continue; }
        for x in lo.0..=hi.0 { for y in lo.1..=hi.1 { grid.entry((x, y)).or_default().push(i); } }
    }
    let carriageway = |p: P2| grid.get(&cell(p)).is_some_and(|list| list.iter().any(|&i| inside(corners[i].map(scene), p)));
    // Wall bottoms and tops per corner position: walls rise from these heights.
    let mut heights = BTreeMap::<(i64, i64), (i64, i64)>::new();
    let mut walls = BTreeMap::<usize, (P2, P2)>::new();
    for (i, v) in corners.iter().enumerate() {
        if !owner(ids[i]) || !vertical(v) { continue; }
        let mut ends: Vec<[i64; 3]> = Vec::new();
        for p in v {
            let h = heights.entry((p[0], p[2])).or_insert((p[1], p[1]));
            *h = (h.0.min(p[1]), h.1.max(p[1]));
            if !ends.iter().any(|e| e[0] == p[0] && e[2] == p[2]) { ends.push(*p); }
        }
        if ends.len() == 2 { walls.insert(i, (scene(ends[0]), scene(ends[1]))); }
    }
    // Curb segments (deduplicated across the two halves of a wall quad), each
    // merged into the line of a longer, almost collinear curb.
    let mut segments = BTreeMap::<[i64; 4], (P2, P2)>::new();
    let mut curb_walls = BTreeMap::<usize, [i64; 4]>::new();
    for (&i, &(a, b)) in &walls {
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        let n = perp(unit(sub(b, a)));
        let probe = |s: f64| carriageway([mid[0] + n[0] * s, mid[1] + n[1] * s]);
        if !(probe(0.05) || probe(-0.05)) { continue; }
        let key = |p: P2| [(p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64];
        let (ka, kb) = (key(a), key(b));
        let k = if ka <= kb { [ka[0], ka[1], kb[0], kb[1]] } else { [kb[0], kb[1], ka[0], ka[1]] };
        segments.insert(k, (a, b));
        curb_walls.insert(i, k);
    }
    let mut order: Vec<_> = segments.iter().collect();
    order.sort_by(|x, y| { let l = |s: &(&[i64; 4], &(P2, P2))| dot(sub(s.1 .1, s.1 .0), sub(s.1 .1, s.1 .0)); l(y).total_cmp(&l(x)).then(x.0.cmp(y.0)) });
    let mut lines: Vec<Line> = Vec::new();
    let mut line_of = BTreeMap::<[i64; 4], usize>::new();
    for (k, &(a, b)) in order {
        let d = unit(sub(b, a));
        let found = lines.iter().position(|l| (l.d[0] * d[1] - l.d[1] * d[0]).abs() < 0.0087 && l.offset(a).abs() < 0.015 && l.offset(b).abs() < 0.015);
        line_of.insert(*k, found.unwrap_or_else(|| { lines.push(Line::new(a, b)); lines.len() - 1 }));
    }
    let crossings = crosswalk_centres(paint, roads);
    let mut triangles = Vec::new();
    let mut frames = Vec::new();
    for (i, v) in corners.iter().enumerate() {
        let Some(road) = sidewalk_of(ids[i], roads) else { continue };
        let digest = mapkit_core::sha256(road.as_bytes());
        let section = (u32::from_str_radix(&digest[..2], 16).unwrap() % 32) as f32 * 8.0;
        // Packed scene order (packed.rs scene_triangle).
        let ordered = [v[0], v[2], v[1]];
        let points = ordered.map(scene);
        let mut push = |values: [[f64; 3]; 3], kind: f32| {
            triangles.push(i as i32);
            for c in values { frames.push(Vector4::new(c[0] as f32, c[1] as f32, c[2] as f32, kind + section)); }
        };
        if let Some(&(a, b)) = walls.get(&i) {
            let curb = curb_walls.get(&i).map(|k| lines[line_of[k]]);
            let line = curb.unwrap_or_else(|| Line::new(a, b));
            let rise = ordered.map(|p| (p[1] - heights[&(p[0], p[2])].0) as f64 / 100.0);
            push(std::array::from_fn(|k| [dot(points[k], line.q), rise[k], 0.0]), if curb.is_some() { FRAME_CURB } else { FRAME_EDGE });
            continue;
        }
        if vertical(v) || lines.is_empty() { continue; }
        let centre = [(points[0][0] + points[1][0] + points[2][0]) / 3.0, (points[0][1] + points[1][1] + points[2][1]) / 3.0];
        let (mut best, mut nearest) = (f64::MAX, 0);
        for (k, &(a, b)) in &segments {
            let distance = segment_distance(centre, a, b);
            if distance < best { best = distance; nearest = line_of[k]; }
        }
        if best > FRAME_REACH_M { continue; }
        let line = lines[nearest];
        let side = if line.offset(centre) < 0.0 { -1.0 } else { 1.0 };
        // The crossing whose mouth (pad and guide strip) this top may hold: its
        // curb must run along the crossed road, as the pad lies along that curb.
        let mut crossing: Option<(P2, P2)> = None;
        let mut closest = f64::MAX;
        for &(o, t, half, _) in &crossings {
            if dot(line.d, t).abs() < 0.85 { continue; }
            let along = points.map(|p| dot(sub(p, o), t));
            let lateral = points.map(|p| dot(sub(p, o), perp(t)).abs());
            let (lo, hi) = (along.iter().copied().fold(f64::MAX, f64::min), along.iter().copied().fold(f64::MIN, f64::max));
            let (near, far) = (lateral.iter().copied().fold(f64::MAX, f64::min), lateral.iter().copied().fold(f64::MIN, f64::max));
            let centre_along = dot(sub(centre, o), t).abs();
            if hi > -0.85 && lo < 0.85 && far > half - 0.1 && near < half + 2.0 && centre_along < closest {
                closest = centre_along;
                crossing = Some((o, t));
            }
        }
        let values = std::array::from_fn(|k| {
            let p = points[k];
            [dot(p, line.q), side * line.offset(p), crossing.map_or(0.0, |(o, t)| dot(sub(p, o), t))]
        });
        push(values, if crossing.is_some() { FRAME_CROSSING } else { FRAME_TOP });
    }
    (triangles, frames)
}
#[cfg(test)]
mod tests {
    use mapkit_core::{Profile, RoadPaint, RoadPaintSegment};
    #[test]
    fn paint_over_the_shader_cap_degrades_without_an_error() {
        let mut d: mapkit_core::MapDocument =
            serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        let mut road = d.roads[0].clone();
        road.markings = None;
        d.roads = ["a", "b", "c"].iter().zip([("n0", "n1"), ("n1", "n2"), ("n3", "n4")]).map(|(id, (from, to))| {
            mapkit_core::Road { id: (*id).into(), from: from.into(), to: to.into(), ..road.clone() }
        }).collect();
        let segments = |n| (0..n).map(|i| RoadPaintSegment { a: [i, 0, 0], b: [i + 1, 0, 0], width_cm: 400.0,
            station_cm: i as f64, period_cm: 240.0, total_cm: n as f64 }).collect::<Vec<_>>();
        let mut paint = RoadPaint::default();
        paint.paths.insert("a".into(), segments(60));
        paint.paths.insert("b".into(), segments(60));
        paint.overflow.insert("c".into(), 140);
        let (path, overflow) = paint.joined(&d, &d.roads[0]);
        assert_eq!((path.len(), overflow), (120, None), "degree-two neighbours join within the cap");
        paint.paths.insert("b".into(), segments(100));
        let (path, overflow) = paint.joined(&d, &d.roads[0]);
        assert_eq!((path.len(), overflow), (60, Some(160)), "an over-cap join falls back to the road's own path");
        let (path, overflow) = paint.joined(&d, &d.roads[2]);
        assert_eq!((path.len(), overflow), (0, None), "core omitted the over-cap road's paint");
        let (_, warnings) = Profile::Authoring.run(|| { super::note_overflow(160, false); super::note_overflow(140, true); Ok(()) });
        let warnings: Vec<_> = warnings.iter().map(|w| (w.code.as_ref(), w.actual, w.limit, w.runtime_rejects)).collect();
        assert_eq!(warnings, [("road_paint_segments", 160, 128, false), ("road_paint_join", 140, 128, false)]);
        assert!(Profile::Runtime.run(|| { super::note_overflow(160, false); Ok(()) }).1.is_empty());
    }
    #[test]
    fn sidewalk_frames_measure_from_the_curb_and_mark_the_crossing_mouth() {
        let d: mapkit_core::MapDocument =
            serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        let mut road = d.roads[0].clone();
        road.id = "r".into();
        road.markings = Some(mapkit_core::RoadMarkings { color: None, lanes: 2, center_line: true, edge_lines: false,
            crosswalk_start: true, crosswalk_end: false, one_way: 0, stop_lines: true, lane_width_cm: 0 , median_width_cm: 0, forward_lanes: 0});
        let roads = std::collections::BTreeMap::from([("r", &road)]);
        let mut paint = RoadPaint::default();
        paint.paths.insert("r".into(), vec![RoadPaintSegment { a: [0, 0, 0], b: [2000, 0, 0], width_cm: 600.0,
            station_cm: 0.0, period_cm: 240.0, total_cm: 2000.0 }]);
        // A 6 m carriageway along +x, a 1 m sidewalk 3 cm up beside it (map z 300–400),
        // its curb wall at z 300, its back wall at z 400, and a plain terrain face.
        let quad = |a: [i64; 3], b: [i64; 3], c: [i64; 3], e: [i64; 3]| [[a, b, c], [a, c, e]];
        let mut faces: Vec<(&str, [[i64; 3]; 3])> = Vec::new();
        for t in quad([0, 0, -300], [2000, 0, -300], [2000, 0, 300], [0, 0, 300]) { faces.push(("r", t)); }
        for t in quad([0, 3, 300], [2000, 3, 300], [2000, 3, 400], [0, 3, 400]) { faces.push(("r:sidewalk", t)); }
        for t in quad([0, 0, 300], [2000, 0, 300], [2000, 3, 300], [0, 3, 300]) { faces.push(("r:sidewalk", t)); }
        for t in quad([0, 0, 400], [2000, 0, 400], [2000, 3, 400], [0, 3, 400]) { faces.push(("r:sidewalk", t)); }
        faces.push(("terrain", [[0, 0, 500], [100, 0, 500], [100, 0, 600]]));
        let ids: Vec<&str> = faces.iter().map(|f| f.0).collect();
        let corners: Vec<_> = faces.iter().map(|f| f.1).collect();
        let (triangles, frames) = super::sidewalk_frames(&paint, &roads, &ids, &corners);
        assert_eq!(triangles, [2, 3, 4, 5, 6, 7], "sidewalk tops and walls only");
        let kind = |t: usize| frames[t * 3].w as u32 % 8;
        assert_eq!((0..6).map(kind).collect::<Vec<_>>(), [2, 2, 3, 3, 4, 4], "crossing tops, curb wall, back wall");
        assert!(frames.iter().all(|f| f.w == frames[0].w || f.w as u32 % 8 != 2), "one section per record");
        for (t, &triangle) in triangles.iter().enumerate().take(2) {
            for k in 0..3 {
                let v = corners[triangle as usize];
                let corner = [v[0], v[2], v[1]][k];
                let f = frames[t * 3 + k];
                assert!((f.x - corner[0] as f32 / 100.0).abs() < 1e-4, "along runs with the curb");
                assert!((f.y - (corner[2] - 300) as f32 / 100.0).abs() < 1e-4, "across from the curb: {f:?}");
                assert!((f.z - (corner[0] as f32 / 100.0 - 1.0)).abs() < 1e-4, "crossing offset from the crosswalk centre");
            }
        }
        for f in &frames[6..12] {
            assert!((f.y == 0.0 || (f.y - 0.03).abs() < 1e-6) && f.z == 0.0, "wall corners carry their rise: {f:?}");
        }
    }
}
