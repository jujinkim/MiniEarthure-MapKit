//! Cancellable connected flood over the exact two triangles of each terrain square.
//! No drainage: an open map edge is a shore at the requested fixed level.
use crate::working::*;
use mapkit_core::{water::WaterBody, *};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
type Triangle = [i32; 3]; // x,y,0=(a,b,c) or 1=(a,c,d)
const FLOOD_TRIANGLES: usize = 1_000_000;

fn vertices(t: Triangle) -> [[i32; 2]; 3] {
    let [x, y, k] = t;
    if k == 0 {
        [[x, y], [x + 1, y], [x + 1, y + 1]]
    } else {
        [[x, y], [x + 1, y + 1], [x, y + 1]]
    }
}
fn neighbors(t: Triangle) -> [Triangle; 3] {
    let [x, y, k] = t;
    if k == 0 {
        [[x, y - 1, 1], [x + 1, y, 1], [x, y, 1]]
    } else {
        [[x, y, 0], [x, y + 1, 0], [x - 1, y, 0]]
    }
}
fn clipped(s: &mut WorkingSnapshot, t: Triangle, level: i64) -> Result<Vec<Point>> {
    let v = vertices(t);
    let mut points = Vec::new();
    let h = [
        s.surface_sample(v[0])?,
        s.surface_sample(v[1])?,
        s.surface_sample(v[2])?,
    ];
    if h.iter().all(|&h| h >= level) {
        return Ok(points);
    }
    for i in 0..3 {
        let j = (i + 1) % 3;
        let a = s.point(v[i]);
        let b = s.point(v[j]);
        if h[i] < level {
            points.push(a);
        }
        if (h[i] < level) != (h[j] < level) {
            // Symmetric rational interpolation: both incident faces must round
            // the same edge to the identical cm point, independent of direction.
            points.push([0, 1].map(|k| {
                ((a[k] as i128 * (h[j] - level) as i128 + b[k] as i128 * (level - h[i]) as i128)
                    / (h[j] - h[i]) as i128) as i64
            }));
        }
    }
    // Clip the regular grid to exact map bounds; moving its last vertex would
    // change its slope and disagree with the saved PNG and generated terrain.
    for axis in 0..2 {
        for (edge, lower) in [
            (s.document.bounds.min[axis], true),
            (s.document.bounds.max[axis], false),
        ] {
            let input = std::mem::take(&mut points);
            for i in 0..input.len() {
                let a = input[i];
                let b = input[(i + 1) % input.len()];
                let inside = |p: Point| {
                    if lower {
                        p[axis] >= edge
                    } else {
                        p[axis] <= edge
                    }
                };
                if inside(a) {
                    points.push(a);
                }
                if inside(a) != inside(b) {
                    let mut p = a;
                    p[axis] = edge;
                    let other = 1 - axis;
                    p[other] = ((a[other] as i128 * (b[axis] - edge) as i128
                        + b[other] as i128 * (edge - a[axis]) as i128)
                        / (b[axis] - a[axis]) as i128) as i64;
                    points.push(p);
                }
            }
        }
    }
    points.dedup();
    if points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 {
        points.clear();
    }
    Ok(points)
}
fn flood(
    s: &mut WorkingSnapshot,
    seeds: Vec<Triangle>,
    level: i64,
) -> Result<Vec<Vec<Vec<Point>>>> {
    let dim = s.dimensions();
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<_> = seeds.into();
    let mut blocks: BTreeMap<[i32; 2], Vec<Vec<Vec<Point>>>> = BTreeMap::new();
    while let Some(t) = queue.pop_front() {
        if t[0] < 0 || t[1] < 0 || t[0] >= dim[0] || t[1] >= dim[1] || !seen.insert(t) {
            continue;
        }
        if seen.len() > FLOOD_TRIANGLES {
            return Err(error("E_BUDGET","water search exceeds one million triangles; reduce the edited area or grid density"));
        }
        if seen.len() % 256 == 0 {
            cancellation::checkpoint()?;
        }
        let polygon = clipped(s, t, level)?;
        if polygon.is_empty() {
            continue;
        }
        let v = vertices(t);
        let near = neighbors(t);
        for i in 0..3 {
            if s.surface_sample(v[i])?
                .min(s.surface_sample(v[(i + 1) % 3])?)
                < level
            {
                queue.push_back(near[i]);
            }
        }
        let p = s.point([t[0], t[1]]);
        let o = s.document.bounds.min;
        blocks
            .entry([((p[0] - o[0]) / 3200) as i32, ((p[1] - o[1]) / 3200) as i32])
            .or_default()
            .push(vec![polygon]);
    }
    let mut pieces = Vec::new();
    for polygons in blocks.values() {
        cancellation::checkpoint()?;
        pieces.extend(water::union(polygons)?);
    }
    // Prefer a whole lake, but keep existing v1 per-body limits for complex shores.
    let merged = water::union(&pieces)?;
    if merged
        .iter()
        .all(|s| s.len() <= 17 && s.iter().map(Vec::len).sum::<usize>() <= 512)
    {
        return Ok(merged);
    }
    if pieces
        .iter()
        .any(|s| s.len() > 17 || s.iter().map(Vec::len).sum::<usize>() > 512)
    {
        return Err(error(
            "E_BUDGET",
            "water fragments exceed v1 polygon limits",
        ));
    }
    Ok(pieces)
}
fn bodies(shapes: Vec<Vec<Vec<Point>>>, level: i64, bottom: i64, flow: [i32; 2]) -> Vec<WaterBody> {
    shapes
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|mut s| {
            let id = format!(
                "water-{}",
                &sha256(&serde_json::to_vec(&(level, &s)).unwrap())[..24]
            );
            WaterBody {
                id,
                polygon: s.remove(0),
                islands: s,
                surface_cm: level,
                bottom_cm: bottom.min(level - 1).max(-1_000_000),
                flow_cm_s: flow,
            }
        })
        .collect()
}
fn triangle_at(s: &WorkingSnapshot, p: Point) -> Triangle {
    let sp = s.spacing();
    let o = s.document.bounds.min;
    let x = (p[0] - o[0]).div_euclid(sp);
    let y = (p[1] - o[1]).div_euclid(sp);
    [
        x as i32,
        y as i32,
        if (p[0] - o[0]) % sp >= (p[1] - o[1]) % sp {
            0
        } else {
            1
        },
    ]
}
fn groups(bodies: &[WaterBody]) -> Result<Vec<Vec<usize>>> {
    let mut unseen: BTreeSet<_> = (0..bodies.len()).collect();
    let mut result = Vec::new();
    while let Some(&first) = unseen.first() {
        cancellation::checkpoint()?;
        unseen.remove(&first);
        let mut group = vec![first];
        let mut at = 0;
        while at < group.len() {
            cancellation::checkpoint()?;
            let adjacent: Vec<_> = unseen
                .iter()
                .copied()
                .filter(|&i| water::connected(&bodies[group[at]], &bodies[i]))
                .collect();
            for i in adjacent {
                unseen.remove(&i);
                group.push(i);
            }
            at += 1;
        }
        result.push(group);
    }
    Ok(result)
}
fn resolve_overlap(mut all: Vec<WaterBody>) -> Result<Vec<WaterBody>> {
    let components = groups(&all)?;
    let mut removed: BTreeSet<usize> = BTreeSet::new();
    for (a, group) in components.iter().enumerate() {
        for other in components.iter().skip(a + 1) {
            let ah = all[group[0]].surface_cm;
            let bh = all[other[0]].surface_cm;
            if ah == bh {
                continue;
            }
            let mut intersects = false;
            for &i in group {
                for &j in other {
                    if water::overlap(&all[i], &all[j])? {
                        intersects = true;
                        break;
                    }
                }
                if intersects {
                    break;
                }
            }
            if intersects {
                removed.extend((if ah < bh { group } else { other }).iter().copied());
            }
        }
    }
    all = all
        .into_iter()
        .enumerate()
        .filter_map(|(i, b)| (!removed.contains(&i)).then_some(b))
        .collect();
    // Deduplicate identical floods and merge same-height connected pieces.
    let mut merged = Vec::new();
    for group in groups(&all)? {
        let first = &all[group[0]];
        let rings: Vec<_> = group
            .iter()
            .map(|&i| {
                std::iter::once(all[i].polygon.clone())
                    .chain(all[i].islands.clone())
                    .collect()
            })
            .collect();
        let shapes = water::union(&rings)?;
        if shapes
            .iter()
            .all(|s| s.len() <= 17 && s.iter().map(Vec::len).sum::<usize>() <= 512)
        {
            merged.extend(bodies(
                shapes,
                first.surface_cm,
                first.bottom_cm,
                first.flow_cm_s,
            ));
        } else {
            merged.extend(group.iter().map(|&i| all[i].clone()));
        }
    }
    merged.sort_by(|a, b| a.id.cmp(&b.id));
    merged.dedup_by(|a, b| a.id == b.id);
    if merged.len() > 1024 {
        return Err(error("E_BUDGET", "water exceeds 1024 v1 fragments"));
    }
    Ok(merged)
}
pub fn edit(s: &mut WorkingSnapshot, point: Option<Point>, remove: bool) -> Result<Vec<WaterBody>> {
    let old = s.document.water_bodies.clone();
    if remove {
        let p = point.ok_or_else(|| error("E_WATER", "missing removal point"))?;
        let hit = old
            .iter()
            .enumerate()
            .filter(|(_, b)| b.contains_horizontal(p))
            .max_by_key(|(_, b)| b.surface_cm)
            .map(|(i, _)| i);
        let removed = groups(&old)?
            .into_iter()
            .find(|g| hit.is_some_and(|i| g.contains(&i)))
            .unwrap_or_default();
        return Ok(old
            .into_iter()
            .enumerate()
            .filter_map(|(i, b)| (!removed.contains(&i)).then_some(b))
            .collect());
    }
    let dim = s.dimensions();
    let fill = if let Some(p) = point {
        let t = triangle_at(s, p);
        let level = s.surface_height(p)?;
        let mut seeds = Vec::new();
        // A click on a grid edge/vertex must include the downhill incident face,
        // even when the triangle chosen by height interpolation slopes uphill.
        for y in (t[1] - 1).max(0)..=t[1].min(dim[1] - 1) {
            for x in (t[0] - 1).max(0)..=t[0].min(dim[0] - 1) {
                for k in 0..2 {
                    let candidate = [x, y, k];
                    let v = vertices(candidate);
                    let polygon = v.map(|v| s.point(v));
                    let contains = (0..3).all(|i| {
                        let a = polygon[i];
                        let b = polygon[(i + 1) % 3];
                        (b[0] - a[0]) as i128 * (p[1] - a[1]) as i128
                            - (b[1] - a[1]) as i128 * (p[0] - a[0]) as i128
                            >= 0
                    });
                    if contains && !clipped(s, candidate, level)?.is_empty() {
                        seeds.push(candidate);
                    }
                }
            }
        }
        if seeds.is_empty() {
            return Ok(old);
        }
        Some((level, seeds))
    } else {
        None
    };
    let mut result = Vec::new();
    for group in groups(&old)? {
        cancellation::checkpoint()?;
        let body = &old[group[0]];
        let mut seeds = BTreeSet::new();
        let mut inspected = 0;
        for &index in &group {
            let b = &old[index];
            let lo = [0, 1].map(|a| b.polygon.iter().map(|p| p[a]).min().unwrap());
            let hi = [0, 1].map(|a| b.polygon.iter().map(|p| p[a]).max().unwrap());
            let min = triangle_at(s, lo);
            let max = triangle_at(s, hi);
            for y in min[1].max(0)..=max[1].min(dim[1] - 1) {
                for x in min[0].max(0)..=max[0].min(dim[0] - 1) {
                    for k in 0..2 {
                        inspected += 1;
                        if inspected > FLOOD_TRIANGLES {
                            return Err(error(
                                "E_BUDGET",
                                "water refresh search allowance exceeded",
                            ));
                        }
                        if inspected % 256 == 0 {
                            cancellation::checkpoint()?;
                        }
                        let t = [x, y, k];
                        let polygon = clipped(s, t, body.surface_cm)?;
                        if polygon.is_empty() {
                            continue;
                        }
                        let candidate = WaterBody {
                            id: String::new(),
                            polygon,
                            islands: vec![],
                            surface_cm: body.surface_cm,
                            bottom_cm: body.bottom_cm,
                            flow_cm_s: [0, 0],
                        };
                        if water::overlap(b, &candidate)? {
                            seeds.insert(t);
                        }
                    }
                }
            }
        }
        let shapes = flood(s, seeds.into_iter().collect(), body.surface_cm)?;
        result.extend(bodies(
            shapes,
            body.surface_cm,
            body.bottom_cm,
            body.flow_cm_s,
        ));
    }
    if let Some((level, seeds)) = fill {
        let shapes = flood(s, seeds, level)?;
        result.extend(bodies(shapes, level, -1_000_000, [0, 0]));
    }
    resolve_overlap(result)
}
