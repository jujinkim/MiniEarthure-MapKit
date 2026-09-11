//! Bounded per-triangle presentation references; never alter the collision archive.
use godot::prelude::*;
use mapkit_core::MapDocument;
use std::collections::BTreeMap;

pub fn decorate(d: &MapDocument, chunk: &VarDictionary, presentation: &mut VarDictionary) {
    if d.recipe_version < 6 {
        return;
    }
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
    let mut gaps = BTreeMap::<&str, f32>::new();
    for road in &d.roads {
        for (node, width) in [
            (&road.from, road.widths_cm[0]),
            (&road.to, *road.widths_cm.last().unwrap()),
        ] {
            let gap = gaps.entry(node).or_default();
            *gap = gap.max(width as f32 / 200.0);
        }
    }
    for (id, p, spawn, surface) in faces {
        let mut key = String::new();
        if let Some(r) = roads.get(id.as_str()).filter(|_| spawn && surface <= 1) {
            let point =
                |v: &mapkit_core::Vertex| Vector2::new(v[0] as f32 / 100.0, -v[2] as f32 / 100.0);
            let segment = r
                .points
                .windows(2)
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let dist = |s: &[mapkit_core::Vertex]| {
                        let a = point(&s[0]);
                        let v = point(&s[1]) - a;
                        let t = (p - a).dot(v) / v.length_squared();
                        (p - a - v * t.clamp(0.0, 1.0)).length_squared()
                    };
                    dist(a).total_cmp(&dist(b))
                })
                .unwrap()
                .0;
            key = format!("road:{}:{}:{}", r.id, segment, surface);
            if !styles.contains_key(key.as_str()) {
                let m = r.markings.as_ref().unwrap();
                let start = segment == 0;
                let end = segment + 2 == r.points.len();
                styles.set(key.as_str(),&vdict!{
                    "road_start"=>point(&r.points[segment]),"road_end"=>point(&r.points[segment+1]),
                    "road_width"=>r.widths_cm[segment] as f64/100.0,"lanes"=>m.lanes as i64,
                    "center_line"=>m.center_line,"edge_lines"=>m.edge_lines,
                    "crosswalk_start"=>start && m.crosswalk_start,"crosswalk_end"=>end && m.crosswalk_end,
                    "start_gap"=>if start {gaps[r.from.as_str()]} else {0.0},
                    "end_gap"=>if end {gaps[r.to.as_str()]} else {0.0},"surface"=>surface as i64});
            }
        }
        keys.push(&GString::from(key.as_str()));
    }
    presentation.set("road_materials", &keys);
    presentation.set("road_styles", &styles);
    presentation.set("urban_surfaces", true);
}
