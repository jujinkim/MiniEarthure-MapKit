mod support { pub mod basins; }
use mapkit_core::*;
use support::basins::*;
use std::collections::BTreeMap;
fn document() -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.placements.clear(); d.assets.clear(); d.repetitions.clear(); d.heightmaps.clear();
    d.bounds = Bounds{min:[0,0],max:[6400,3200]}; d.cell_size_cm=3200;
    d.water_bodies = vec![water::WaterBody{id:"lake".into(),polygon:vec![[100,100],[6200,100],[6200,3000],[100,3000]],islands:vec![vec![[1000,1000],[2000,1000],[2000,2000],[1000,2000]]],surface_cm:200,bottom_cm:-1000,flow_cm_s:[20,0]}];
    d
}
#[test]
fn water_is_non_solid_and_spawn_query_respects_islands_bridge_and_bottom() {
    let d=document(); d.validate().unwrap();
    let mut c=generate(GenerationInput{document:&d,cell:Cell{x:0,y:0},heightgrid:None,max_triangles:20000}).unwrap();
    assert_eq!(c.water_bodies.len(),1);
    assert!(!c.water_bodies[0].surface.is_empty());
    assert!(c.triangles.iter().all(|t| t.vertices.iter().all(|p| p[1] != 200)));
    assert!(c.spawn_options([500,500]).unwrap().is_empty());
    assert!(!c.spawn_options([1500,1500]).unwrap().is_empty());
    assert!(!c.spawn_options([1000,1500]).unwrap().is_empty()); // closed dry island
    assert!(water::sample(&d.water_bodies,[500,-1001,500]).is_none());
    assert!(water::sample(&d.water_bodies,[500,201,500]).is_none());
    assert!(water::sample(&d.water_bodies,[100,200,500]).is_some());
    c.triangles.push(Triangle{contact_class:3,snow_retention_percent:100,vertices:[[100,400,100],[900,400,100],[100,400,900]],surface:Surface::Concrete,object_id:"bridge".into(),spawnable:true});
    assert_eq!(c.spawn_options([200,200]).unwrap()[0].surface_id,"bridge");
    assert_eq!(c.hash().unwrap(),sha256(&canonical(&c).unwrap()));
}
#[test]
fn water_seams_region_cost_archive_and_hash_are_deterministic() {
    let d=document(); let p=PreparedMap::new(d.clone()).unwrap();
    for cell in d.cells() {
        let c=p.generate(cell,None,20000).unwrap();
        let cost=p.estimate(cell,20000).unwrap();
        assert!(cost.water_bytes >= canonical(&c.water_bodies).unwrap().len() as u64);
        let key=archive_key("water-test",cell);
        let bytes=encode_archive(&c,&key,archive_limit(&cost)).unwrap();
        assert_eq!(decode_archive(&bytes,&key,cell,&cost).unwrap(),c);
        assert!(decode_archive(&bytes[..bytes.len()-1],&key,cell,&cost).is_err());
        let local=region_source(&d,CellRegion{min:cell,end:Cell{x:cell.x+1,y:cell.y+1}}).unwrap();
        assert_eq!(generate(GenerationInput{document:&local,cell,heightgrid:None,max_triangles:20000}).unwrap(),c);
        let bounds=d.cell_bounds(cell).unwrap();
        assert!(c.water_bodies[0].surface.iter().flatten().all(|v| bounds.contains([v[0],v[2]])));
        assert!(c.water_bodies[0].body.contains([3200,100,500]));
    }
    let mut changed=d.clone(); changed.water_bodies[0].flow_cm_s=[30,0];
    assert_ne!(PreparedMap::new(changed).unwrap().generate(Cell{x:0,y:0},None,20000).unwrap().hash().unwrap(),p.generate(Cell{x:0,y:0},None,20000).unwrap().hash().unwrap());
}
#[test]
fn water_rejects_invalid_islands_heights_flow_and_shared_ids() {
    let d=document();
    for mutation in 0..4 { let mut bad=d.clone(); match mutation {
        0=>bad.water_bodies[0].bottom_cm=201,
        1=>bad.water_bodies[0].flow_cm_s=[1001,0],
        2=>bad.water_bodies[0].islands.push(vec![[0,0],[300,0],[300,300]]),
        _=>bad.water_bodies.push(bad.water_bodies[0].clone()),
    }; assert!(bad.validate().is_err()); }
    let mut bodies=d.water_bodies.clone(); let mut upper=bodies[0].clone(); upper.id="upper".into();upper.bottom_cm=400;upper.surface_cm=600;bodies.push(upper);
    assert_eq!(water::sample(&bodies,[500,500,500]).unwrap().id,"upper");
    assert!(water::sample(&bodies,[500,300,500]).is_none());
}

fn chunks(d: &MapDocument, grids: &BTreeMap<Cell, HeightGrid>) -> Vec<(Cell, GeneratedChunk)> {
    d.cells().into_iter().map(|cell| (cell, generate(input(d, grids, cell)).unwrap())).collect()
}

#[test]
fn city_basins_sink_an_earth_bed_with_watertight_unsunk_shores() {
    for (d, grids) in [(city(true), BTreeMap::new()), sloped(true)] {
        let report = water::basin_report(&d).unwrap();
        assert!(report.active && report.components == 2 && report.sinking_components == 2, "{report:?}");
        assert_eq!(report.counts.seam_edges, 2, "the shared pond seam is not a shore");
        assert!(report.counts.road_keep_outs > 0 && report.counts.barrier_keep_outs > 0 && report.trimmed_components == 2, "{report:?}");
        assert!(report.retained_bytes <= report.limit_bytes && report.limit_bytes <= water::MAX_BASIN_BYTES);
        let mut deepest = i64::MAX;
        let mut earth = 0;
        for (cell, c) in chunks(&d, &grids) {
            let bounds = d.cell_bounds(cell).unwrap();
            let spacing = if grids.is_empty() { 3200 } else { 400 };
            let side = (3200 / spacing + 1) as usize;
            let g = |p: Point| terrain_height(&bounds, spacing, side, grids.get(&cell), d.terrain_base_cm, p);
            for (p, h) in heights(&c) {
                assert_eq!(h.len(), 1, "{cell:?} crack at {p:?}: {h:?}");
                let (h, ground) = (h[0], g(p));
                let wet: Vec<_> = d.water_bodies.iter().filter(|b| b.contains_horizontal(p)).collect();
                // Between the terrain and the bed of its water, at most depth plus
                // freeboard under the terrain.
                assert!(h <= ground.max(BOTTOM) && h >= ground.min(BOTTOM) && h >= ground - (SURFACE - BOTTOM + 8), "{p:?} {h} {ground}");
                if wet.is_empty() || d.water_bodies.iter().any(|b| on_ring(p, &b.polygon) && !b.id.starts_with("pond")) {
                    assert_eq!(h, ground, "dry or shore vertex {p:?} sank");
                }
                deepest = deepest.min(h - BOTTOM.max(ground - (SURFACE - BOTTOM + 8)));
            }
            // Pond shores away from the seam keep the terrain height.
            for t in ground(&c) {
                for v in t.vertices {
                    let p = [v[0], v[2]];
                    if on_ring(p, &d.water_bodies[0].polygon) && p[0] != 1600 { assert_eq!(v[1], g(p), "{p:?}"); }
                }
            }
            assert!(open_edges(&c, &bounds).is_empty(), "{cell:?} {:?}", open_edges(&c, &bounds));
            for t in ground(&c).into_iter().filter(|t| t.surface == Surface::Dirt) {
                earth += 1;
                assert!(t.object_id == "terrain" && t.contact_class == 1 && t.spawnable);
                let centre = [t.vertices.iter().map(|v| v[0]).sum::<i64>() / 3, t.vertices.iter().map(|v| v[2]).sum::<i64>() / 3];
                assert!(d.water_bodies.iter().any(|b| b.contains_horizontal(centre)), "earth outside water at {centre:?}");
            }
            // The road keeps its surface: the same plan area, every vertex on the
            // terrain (basin lines may only add vertices). The bed refuses spawns.
            let (dry_d, dry_grids) = if grids.is_empty() { (city(false), BTreeMap::new()) } else { sloped(false) };
            let dry = generate(input(&dry_d, &dry_grids, cell)).unwrap();
            let road = |c: &GeneratedChunk| -> (i128, bool) {
                let faces: Vec<_> = c.triangles.iter().filter(|t| t.object_id == "crossing").collect();
                (faces.iter().map(|t| { let v = t.vertices; ((v[1][0] - v[0][0]) as i128 * (v[2][2] - v[0][2]) as i128
                    - (v[1][2] - v[0][2]) as i128 * (v[2][0] - v[0][0]) as i128).abs() }).sum(),
                 faces.iter().all(|t| t.vertices.iter().all(|v| v[1] == g([v[0], v[2]]))))
            };
            assert_eq!(road(&c), road(&dry));
            assert!(road(&c).1);
            let flat = dry;
            if grids.is_empty() && cell.x == 0 {
                let centre = SpawnRequest { position_cm: [1000, 1400], surface_id: "terrain".into() };
                assert!(c.spawn(&centre).is_err() && flat.spawn(&centre).is_ok(), "the pond bed is submerged");
            }
        }
        // The pond centre reaches its bed, or depth plus freeboard under higher
        // terrain: the bank is wide but not the pond.
        assert_eq!(deepest, 0);
        assert!(earth > 0);
    }
}

#[test]
fn basins_are_city_only_and_leave_authored_maps_byte_identical() {
    let authored = city(false);
    assert!(!water::basins_apply(&authored) && !water::basin_report(&authored).unwrap().active);
    let mut tracked = city(true);
    tracked.water_bodies.clear();
    assert!(!water::basins_apply(&tracked));
    for (cell, c) in chunks(&authored, &BTreeMap::new()) {
        assert!(ground(&c).iter().all(|t| t.surface != Surface::Dirt && t.vertices.iter().all(|v| v[1] == 0)), "{cell:?}");
        // Water bodies change nothing in an authored map's ground.
        let mut dry = authored.clone();
        dry.water_bodies.clear();
        let without = generate(input(&dry, &BTreeMap::new(), cell)).unwrap();
        assert_eq!(c.triangles, without.triangles);
    }
}

#[test]
fn basin_levels_bank_widths_and_region_sources_are_local() {
    assert_eq!((water::bank_cm(-8, -68), water::bank_cm(0, -10), water::bank_cm(0, -500)), (180, 90, 240));
    // Touching pieces at different levels keep a bank between them.
    let mut stepped = city(true);
    stepped.water_bodies[1].surface_cm -= 20;
    stepped.water_bodies[1].bottom_cm -= 20;
    let report = water::basin_report(&stepped).unwrap();
    assert_eq!(report.counts.seam_edges, 0);
    let c = generate(input(&stepped, &BTreeMap::new(), Cell { x: 0, y: 0 })).unwrap();
    let seam: Vec<_> = heights(&c).into_iter().filter(|(p, _)| p[0] == 1600 && p[1] > 400 && p[1] < 2400).collect();
    assert!(!seam.is_empty() && seam.iter().all(|(_, h)| h == &vec![0]), "{seam:?}");
    // Regional sources, prepared maps and reordered sources generate the same ground.
    for (d, grids) in [(city(true), BTreeMap::new()), sloped(true)] {
        let prepared = PreparedMap::new(d.clone()).unwrap();
        let mut reordered = d.clone();
        reordered.water_bodies.reverse();
        reordered.roads.reverse();
        reordered.barriers.reverse();
        for cell in d.cells() {
            let c = generate(input(&d, &grids, cell)).unwrap();
            assert_eq!(prepared.generate(cell, grids.get(&cell), 500_000).unwrap(), c);
            assert_eq!(generate(input(&reordered, &grids, cell)).unwrap(), c);
            let local = region_source(&d, CellRegion { min: cell, end: Cell { x: cell.x + 1, y: cell.y + 1 } }).unwrap();
            assert_eq!(generate(input(&local, &grids, cell)).unwrap(), c, "{cell:?}");
        }
    }
}

/// A wide real-city map: a river cut into import pieces along cell lines, a
/// long ground road wet only at its far end (a pond beyond every other cell's
/// region margin) and roads ending on tile lines inside the river.
fn wide_city() -> MapDocument {
    let mut d = city(true);
    d.bounds = Bounds { min: [0, 0], max: [19200, 6400] };
    d.barriers.clear();
    let body = |id: String, polygon: Vec<Point>| water::WaterBody { id, polygon, islands: vec![],
        surface_cm: SURFACE, bottom_cm: BOTTOM, flow_cm_s: [0, 0] };
    d.water_bodies = (0..6).map(|k| {
        let (a, b) = (k * 3200, (k + 1) * 3200);
        body(format!("river-{k}"), vec![[a, 2500 + k * 20], [b, 2500 + (k + 1) * 20], [b, 3700 - (k + 1) * 30], [a, 3700 - k * 30]])
    }).collect();
    d.water_bodies.push(body("pond".into(), vec![[17000, 5000], [18600, 5100], [18500, 6200], [17100, 6100]]));
    d.nodes = serde_json::from_value(serde_json::json!([
        {"id": "west", "position": [600, 0, 5900], "level": 0},
        {"id": "east", "position": [17800, 0, 5500], "level": 0},
        {"id": "pier-n", "position": [8000, 0, 4400], "level": 0},
        {"id": "pier-s", "position": [8000, 0, 3200], "level": 0},
        {"id": "ford-n", "position": [12400, 0, 2000], "level": 0},
        {"id": "ford-s", "position": [12400, 0, 2800], "level": 0}])).unwrap();
    let road = |id: &str, from: &str, to: &str, points: serde_json::Value, width: u32| serde_json::json!({"id": id, "from": from, "to": to,
        "points": points, "widths_cm": [width], "surfaces": ["asphalt"], "kind": "ground", "clearance_cm": null, "sidewalk_cm": 0});
    d.roads = serde_json::from_value(serde_json::json!([
        road("long", "west", "east", serde_json::json!([[600, 0, 5900], [17800, 0, 5500]]), 700),
        // Ends exactly on a 400 cm tile line inside the river.
        road("pier", "pier-n", "pier-s", serde_json::json!([[8000, 0, 4400], [8000, 0, 3200]]), 300),
        road("ford", "ford-n", "ford-s", serde_json::json!([[12400, 0, 2000], [12400, 0, 2800]]), 300)])).unwrap();
    d.validate().unwrap();
    d
}
fn wide_grids(d: &MapDocument, spacing: i64) -> BTreeMap<Cell, HeightGrid> {
    let side = 3200 / spacing + 1;
    d.cells().into_iter().map(|cell| {
        let heights = (0..side).flat_map(|j| (0..side).map(move |i| {
            let (x, y) = (cell.x as i64 * 3200 + i * spacing, cell.y as i64 * 3200 + j * spacing);
            x / 60 + (y - 3200).abs() / 25 + ((x * 7 + y * 3) % 17)
        })).collect();
        (cell, HeightGrid { side: side as usize, heights_cm: heights })
    }).collect()
}

#[test]
fn region_sources_match_full_generation_beyond_their_margins() {
    let flat = wide_city();
    let report = water::basin_report(&flat).unwrap();
    assert!(report.counts.seam_edges >= 10 && report.counts.road_keep_outs > 0, "{report:?}");
    let mut sloped = flat.clone();
    sloped.heightmaps = flat.cells().into_iter().map(|cell| Heightmap { cell, path: format!("h{}-{}.png", cell.x, cell.y),
        spacing_cm: 400, offset_cm: 0, step_cm: 1, source_accuracy_cm: None }).collect();
    for (d, grids) in [(flat.clone(), BTreeMap::new()), (sloped, wide_grids(&flat, 400))] {
        for cell in d.cells() {
            let bounds = d.cell_bounds(cell).unwrap();
            let full = generate(input(&d, &grids, cell)).unwrap();
            let local = region_source(&d, CellRegion { min: cell, end: Cell { x: cell.x + 1, y: cell.y + 1 } }).unwrap();
            assert_eq!(generate(input(&local, &grids, cell)).unwrap(), full, "{cell:?}");
            assert!(heights(&full).values().all(|h| h.len() == 1), "{cell:?} crack");
            // No T-junction: every interior plan edge has two faces.
            assert!(open_edges(&full, &bounds).is_empty(), "{cell:?} {:?}", open_edges(&full, &bounds));
        }
        // The long road's keep-out adds nothing to dry cells far from the pond.
        let mut dry = d.clone();
        dry.water_bodies.retain(|b| b.id != "pond");
        for cell in [Cell { x: 0, y: 1 }, Cell { x: 2, y: 1 }] {
            assert_eq!(generate(input(&d, &grids, cell)).unwrap().triangles, generate(input(&dry, &grids, cell)).unwrap().triangles, "{cell:?}");
        }
    }
}

/// `g − min(g − bottom, depth + 8) · min(d, w) / w` for a document whose only
/// shores are its water outlines.
fn expected_height(body: &water::WaterBody, p: Point, g: i64) -> i64 {
    if !body.contains_horizontal(p) { return g; }
    let w = water::bank_cm(body.surface_cm, body.bottom_cm);
    let ring = &body.polygon;
    let d2 = (0..ring.len()).map(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dy, px, py) = ((b[0] - a[0]) as i128, (b[1] - a[1]) as i128, (p[0] - a[0]) as i128, (p[1] - a[1]) as i128);
        let (len, dot) = (dx * dx + dy * dy, px * dx + py * dy);
        if dot <= 0 { px * px + py * py } else if dot >= len { (px - dx).pow(2) + (py - dy).pow(2) } else { (px * dy - py * dx).pow(2) / len }
    }).min().unwrap() as u128;
    let depth = (d2.isqrt() as i64).min(w);
    let sink = (g - body.bottom_cm).min(body.surface_cm - body.bottom_cm + 8);
    g - (sink as i128 * depth as i128 / w as i128) as i64
}

/// A detailed 512-vertex lake in a 128 m cell: basin heights cost a bounded
/// amount per vertex (not per lake vertex), generation stays within the
/// runtime work limits and the estimate, and every height follows the bank rule
/// (terrain far above the bed sinks depth plus freeboard; terrain below the
/// bottom rises to it).
#[test]
fn detailed_lakes_generate_within_runtime_limits_and_follow_the_bank_rule() {
    let mut d = city(true);
    d.bounds = Bounds { min: [0, 0], max: [12800, 12800] };
    d.cell_size_cm = 12800;
    d.roads.clear(); d.nodes.clear(); d.barriers.clear();
    let polygon: Vec<Point> = (0..512).map(|i| {
        let t = i as f64 / 512.0 * std::f64::consts::TAU;
        let r = 5000.0 * (1.0 + 0.05 * (t * 37.0).sin());
        [6400 + (r * t.cos()) as i64, 6400 + (r * t.sin()) as i64]
    }).collect();
    d.water_bodies = vec![water::WaterBody { id: "lake".into(), polygon, islands: vec![], surface_cm: SURFACE, bottom_cm: BOTTOM, flow_cm_s: [0, 0] }];
    let cell = Cell { x: 0, y: 0 };
    for spacing in [400i64, 200] {
        let side = 12800 / spacing + 1;
        let mut grid = d.clone();
        grid.heightmaps = vec![Heightmap { cell, path: "h.png".into(), spacing_cm: spacing as u32, offset_cm: 0, step_cm: 1, source_accuracy_cm: None }];
        // A ridge 2 m above the bed and a hole below the bottom inside the lake.
        let samples: Vec<i64> = (0..side).flat_map(|j| (0..side).map(move |i| {
            let (x, y) = (i * spacing, j * spacing);
            if (x - 4000).abs() < 1200 && (y - 6400).abs() < 1200 { -300 } else { (x / 50 + y / 90) % 40 + if x > 8000 { 200 } else { 0 } }
        })).collect();
        let grids = BTreeMap::from([(cell, HeightGrid { side: side as usize, heights_cm: samples })]);
        let prepared = PreparedMap::new(grid.clone()).unwrap();
        let cost = prepared.estimate(cell, 500_000).unwrap();
        let chunk = prepared.generate(cell, grids.get(&cell), 500_000).unwrap();
        assert!(chunk.triangles.len() as u64 <= cost.triangles);
        assert_eq!(generate(input(&grid, &grids, cell)).unwrap(), chunk);
        let bounds = grid.cell_bounds(cell).unwrap();
        let (mut raised, mut capped, mut edge, mut off) = (0, 0, 0, 0i64);
        for (p, h) in heights(&chunk) {
            assert_eq!(h.len(), 1, "crack at {p:?}");
            let g = terrain_height(&bounds, spacing, side as usize, grids.get(&cell), grid.terrain_base_cm, p);
            let expected = expected_height(&grid.water_bodies[0], p, g);
            // A bed vertex on a tile edge (not a corner) follows the edge's
            // piecewise-linear anchors instead: close to the rule, within its bounds.
            let on = |a: usize| p[a].rem_euclid(spacing) == 0;
            if on(0) != on(1) && expected != g {
                edge += 1;
                off = off.max((h[0] - expected).abs());
                assert!((h[0] - expected).abs() <= 2 && h[0] >= g.min(BOTTOM) - 1 && h[0] <= g.max(BOTTOM) + 1
                    && h[0] >= g - (SURFACE - BOTTOM + 8) - 1, "{spacing} {p:?} terrain {g}: {} vs {expected}", h[0]);
            } else {
                assert_eq!(h[0], expected, "{spacing} {p:?} terrain {g}");
            }
            if h[0] > g { raised += 1; }
            if g - h[0] == SURFACE - BOTTOM + 8 && h[0] > BOTTOM { capped += 1; }
        }
        assert!(raised > 0 && capped > 0 && edge > 0, "{raised} {capped} {edge}");
        println!("{spacing}: {edge} tile-edge bed vertices, at most {off} cm from the vertex rule");
        assert!(open_edges(&chunk, &bounds).is_empty());
    }
}
