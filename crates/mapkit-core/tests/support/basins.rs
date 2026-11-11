//! Synthetic real-city water: a pond of two import pieces sharing a seam, a
//! narrow stream across a cell seam crossed by a ground road, and a fence in
//! the pond. `osm` adds the `osm-extract-v1` scale notice that enables basins.
#![allow(dead_code)]
use mapkit_core::*;
use std::collections::BTreeMap;

pub const SURFACE: i64 = -8;
pub const BOTTOM: i64 = -68;

pub fn city(osm: bool) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.placements.clear();
    d.assets.clear(); d.repetitions.clear(); d.heightmaps.clear(); d.surface_areas.clear();
    d.bounds = Bounds { min: [0, 0], max: [6400, 3200] };
    d.cell_size_cm = 3200;
    d.ground_surface = Some(Surface::Concrete);
    if osm {
        d.attributions.push(Attribution { source: "synthetic OSM".into(), license: "ODbL-1.0".into(),
            notice: r#"{"adapter":"osm-extract-v1","authored_units":{"source_denominator":3}}"#.into() });
    }
    let body = |id: &str, polygon: Vec<Point>| water::WaterBody { id: id.into(), polygon, islands: vec![],
        surface_cm: SURFACE, bottom_cm: BOTTOM, flow_cm_s: [0, 0] };
    d.water_bodies = vec![
        body("pond-a", vec![[400, 400], [1600, 400], [1600, 2400], [400, 2400]]),
        body("pond-b", vec![[1600, 400], [2600, 600], [2600, 2200], [1600, 2400]]),
        body("stream", vec![[2900, 1400], [6000, 1300], [6000, 1460], [2900, 1560]]),
    ];
    d.nodes = serde_json::from_value(serde_json::json!([
        {"id": "south", "position": [4800, 0, 200], "level": 0},
        {"id": "north", "position": [4800, 0, 3000], "level": 0}])).unwrap();
    d.roads = serde_json::from_value(serde_json::json!([{"id": "crossing", "from": "south", "to": "north",
        "points": [[4800, 0, 200], [4800, 0, 3000]], "widths_cm": [300], "surfaces": ["asphalt"],
        "kind": "ground", "clearance_cm": null, "sidewalk_cm": 0}])).unwrap();
    d.barriers = vec![barrier::Barrier { style_id: String::new(), id: "fence".into(), kind: barrier::BarrierKind::Fence,
        line: vec![[2000, 1000], [2400, 1000]], height_cm: 40, thickness_cm: 10 }];
    d
}

/// The same document on a sloped 400 cm heightmap in both cells.
pub fn sloped(osm: bool) -> (MapDocument, BTreeMap<Cell, HeightGrid>) {
    let mut d = city(osm);
    let mut grids = BTreeMap::new();
    for x in 0..2 {
        let cell = Cell { x, y: 0 };
        d.heightmaps.push(Heightmap { cell, path: format!("h{x}.png"), spacing_cm: 400, offset_cm: 0, step_cm: 1, source_accuracy_cm: None });
        let heights = (0..9).flat_map(|j| (0..9).map(move |i| {
            let (px, py) = (x as i64 * 3200 + i * 400, j * 400);
            px / 40 + py / 80 + ((px * 7 + py * 3) % 13)
        })).collect();
        grids.insert(cell, HeightGrid { side: 9, heights_cm: heights });
    }
    (d, grids)
}

pub fn input<'a>(d: &'a MapDocument, grids: &'a BTreeMap<Cell, HeightGrid>, cell: Cell) -> GenerationInput<'a> {
    GenerationInput { document: d, cell, heightgrid: grids.get(&cell), max_triangles: 500_000 }
}

/// Ground faces (terrain, paint and ground roads) of a chunk.
pub fn ground(c: &GeneratedChunk) -> Vec<&Triangle> {
    c.triangles.iter().filter(|t| t.spawnable && matches!(t.contact_class, 1 | 2)).collect()
}

/// Heights of every generated ground vertex position; a crack would give one
/// position two heights.
pub fn heights(c: &GeneratedChunk) -> BTreeMap<Point, Vec<i64>> {
    let mut out: BTreeMap<Point, Vec<i64>> = BTreeMap::new();
    for t in ground(c) {
        for v in t.vertices {
            let h = out.entry([v[0], v[2]]).or_default();
            if !h.contains(&v[1]) { h.push(v[1]); }
        }
    }
    out
}

/// Plan-view edges of ground faces used by exactly one face, excluding the cell
/// border: a planar arrangement leaves none (no gaps and no T-junctions).
pub fn open_edges(c: &GeneratedChunk, bounds: &Bounds) -> Vec<(Point, Point)> {
    let mut edges: BTreeMap<(Point, Point), u32> = BTreeMap::new();
    for t in ground(c) {
        for i in 0..3 {
            let (a, b) = ([t.vertices[i][0], t.vertices[i][2]], [t.vertices[(i + 1) % 3][0], t.vertices[(i + 1) % 3][2]]);
            *edges.entry(if a < b { (a, b) } else { (b, a) }).or_default() += 1;
        }
    }
    edges.into_iter().filter(|((a, b), n)| *n == 1
        && !(0..2).any(|i| a[i] == b[i] && (a[i] == bounds.min[i] || a[i] == bounds.max[i])))
        .map(|(e, _)| e).collect()
}

pub fn on_ring(p: Point, ring: &[Point]) -> bool {
    (0..ring.len()).any(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        (b[0] - a[0]) as i128 * (p[1] - a[1]) as i128 == (b[1] - a[1]) as i128 * (p[0] - a[0]) as i128
            && (0..2).all(|k| p[k] >= a[k].min(b[k]) && p[k] <= a[k].max(b[k]))
    })
}
