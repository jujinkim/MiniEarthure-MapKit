//! Bounded per-triangle presentation references; never alter the collision archive.
use godot::prelude::*;
use mapkit_core::MapDocument;
use std::collections::BTreeMap;

pub fn decorate(
    d: &MapDocument,
    chunk: &VarDictionary,
    presentation: &mut VarDictionary,
) -> mapkit_core::Result<()> {
    let roads: BTreeMap<_, _> = d
        .roads
        .iter()
        .filter(|r| r.markings.is_some())
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
                if surface == "asphalt" {
                    0
                } else if surface == "concrete" {
                    1
                } else {
                    4
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
    for (id, _p, spawn, surface) in faces {
        let mut key = if id.ends_with(":safety:metal") {
            "safety:metal".into()
        } else {
            String::new()
        };
        if let Some(r) = roads.get(id.as_str()).filter(|_| spawn && surface <= 1) {
            key = format!(
                "road:{}:{}:{}:{}",
                r.id, bounds.min[0], bounds.min[1], surface
            );
            if !styles.contains_key(key.as_str()) {
                let m = r.markings.as_ref().unwrap();
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
                let mut connected = vec![r.id.as_str()];
                for node in [&r.from, &r.to] {
                    let arms: Vec<_> = d
                        .roads
                        .iter()
                        .filter(|other| &other.from == node || &other.to == node)
                        .collect();
                    if arms.len() == 2 {
                        for other in arms {
                            if !connected.contains(&other.id.as_str()) {
                                connected.push(&other.id);
                            }
                        }
                    }
                }
                for id in connected {
                    if let Some(path) = paint.paths.get(id) {
                        for s in path {
                            paths.push(segment(s.a, s.b));
                            metrics.push(Vector4::new(
                                s.width_cm as f32 / 100.0,
                                s.station_cm as f32 / 100.0,
                                s.period_cm as f32 / 100.0,
                                s.total_cm as f32 / 100.0,
                            ));
                        }
                    }
                }
                if paths.len() > 128 {
                    return Err(mapkit_core::Error {
                        code: "E_BUDGET".into(),
                        message: format!("road {} paint exceeds 128 joined segments", r.id),
                    });
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
                let tint = m.color.map(|c|Color::from_rgba(c[0] as f32/255.,c[1] as f32/255.,c[2] as f32/255.,1.));
                let mut style=vdict!{
                    "road_paths"=>&paths,"road_metrics"=>&metrics,"road_borders"=>&borders,
                    "path_count"=>path_count as i64,"edge_count"=>edge_count as i64,
                    "lanes"=>m.lanes as i64,"center_line"=>m.center_line,
                    "edge_lines"=>m.edge_lines,"crosswalk_start"=>m.crosswalk_start,"crosswalk_end"=>m.crosswalk_end,"surface"=>surface as i64};
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
