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
    if let Some(value) = chunk.get("geometry") {
        let mut geometry = value.to::<Gd<RefCounted>>();
        let view = geometry.call("view", &[]).to::<VarDictionary>();
        let ids = view.get("object_ids").unwrap().to::<PackedStringArray>();
        let indices = view.get("object_indices").unwrap().to::<PackedInt32Array>();
        let vertices = view.get("vertices_cm").unwrap().to::<PackedInt64Array>();
        let spawn = view.get("spawnable").unwrap().to::<PackedByteArray>();
        let surfaces = view.get("surface_indices").unwrap().to::<PackedByteArray>();
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
                let unmarked = mapkit_core::RoadMarkings { color: None, lanes: 1, center_line: false, edge_lines: false, crosswalk_start: false, crosswalk_end: false, one_way: 0, stop_lines: false, lane_width_cm: 0 };
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
                let tint = m.color.map(|c|Color::from_rgba(c[0] as f32/255.,c[1] as f32/255.,c[2] as f32/255.,1.));
                let mut style=vdict!{
                    "snow_retention_percent"=>r.snow_retention_percent as f64,
                    "marked"=>r.markings.is_some() && surface <= 1,
                    "road_paths"=>&paths,"road_metrics"=>&metrics,"road_borders"=>&borders,
                    "path_count"=>path_count as i64,"edge_count"=>edge_count as i64,
                    "lanes"=>m.lanes as i64,"center_line"=>m.center_line,
                    "edge_lines"=>m.edge_lines,"crosswalk_start"=>m.crosswalk_start,"crosswalk_end"=>m.crosswalk_end,"surface"=>surface as i64,
                    "stop_start"=>turns(&r.from, false),"stop_end"=>turns(&r.to, true),"one_way"=>m.one_way as i64,"left_hand"=>left_hand,
                    "lane_width"=>m.lane_width_cm as f64 / 100.0};
                if let Some(tint)=tint { style.set("base_color",tint); }
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
}
