//! Optional v1 city records: default ground surface, barriers and building parts.
use mapkit_core::barrier::{Barrier, BarrierKind};
use mapkit_core::*;

fn minimal() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}
fn generate_cell(d: &MapDocument, cell: Cell, grid: Option<&HeightGrid>) -> GeneratedOccupancy {
    generate_with_occupancy(GenerationInput { document: d, cell, heightgrid: grid, max_triangles: 500_000 }, MAX_OCCUPIED_SOLIDS).unwrap()
}
fn barrier(id: &str, kind: BarrierKind, line: Vec<Point>) -> Barrier {
    Barrier { id: id.into(), kind, line, height_cm: 120, thickness_cm: 20 }
}
fn part(footprint: Vec<Point>, height: Option<u32>) -> BuildingPart {
    BuildingPart { footprint, holes: vec![], base_cm: None, height_cm: height }
}
fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<Point> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

#[test]
fn absent_fields_keep_canonical_bytes_and_round_trip() {
    let source = include_str!("../../../examples/minimal/document.json");
    let d: MapDocument = serde_json::from_str(source).unwrap();
    let text = String::from_utf8(canonical(&d).unwrap()).unwrap();
    for key in ["ground_surface", "barriers", "parts"] {
        assert!(!text.contains(key), "{key} must be omitted when absent");
    }
    let mut extended = d.clone();
    extended.ground_surface = Some(Surface::Concrete);
    extended.barriers.push(barrier("hedge-1", BarrierKind::Hedge, vec![[1000, 1000], [9000, 1000]]));
    extended.buildings[0].parts.push(part(rect(32000, 30000, 34000, 32000), Some(900)));
    extended.validate().unwrap();
    let bytes = canonical(&extended).unwrap();
    let back: MapDocument = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back, extended);
    assert_eq!(canonical(&back).unwrap(), bytes);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["ground_surface"], "concrete");
    assert_eq!(value["barriers"][0]["kind"], "hedge");
    assert_eq!(value["buildings"][0]["parts"][0]["height_cm"], 900);
    assert!(value["buildings"][0]["parts"][0].get("base_cm").is_none());
}

#[test]
fn default_ground_surface_replaces_unpainted_terrain_only() {
    let mut d = minimal();
    d.surface_areas.push(SurfaceArea { id: "park".into(), polygon: rect(40000, 40000, 45000, 45000), surface: Surface::Grass });
    let before = generate_cell(&d, Cell { x: 0, y: 0 }, None).chunk;
    assert!(before.triangles.iter().filter(|t| t.object_id == "terrain").all(|t| t.surface == Surface::Grass));
    d.ground_surface = Some(Surface::Concrete);
    let after = generate_cell(&d, Cell { x: 0, y: 0 }, None).chunk;
    let terrain: Vec<_> = after.triangles.iter().filter(|t| t.object_id == "terrain").collect();
    assert!(!terrain.is_empty() && terrain.iter().all(|t| t.surface == Surface::Concrete && t.spawnable && t.contact_class == 1));
    let park: Vec<_> = after.triangles.iter().filter(|t| t.object_id == "park").collect();
    assert!(!park.is_empty() && park.iter().all(|t| t.surface == Surface::Grass && t.contact_class == 1));
    // Only the unpainted surface label changes: same faces, same identities.
    assert_eq!(before.triangles.len(), after.triangles.len());
    for (a, b) in before.triangles.iter().zip(&after.triangles) {
        assert_eq!((&a.vertices, &a.object_id), (&b.vertices, &b.object_id));
        if a.object_id != "terrain" { assert_eq!(a.surface, b.surface); }
    }
    let probe = after.surface_probe(&SpawnRequest { position_cm: [10000, 10000], surface_id: "terrain".into() }).unwrap();
    assert_eq!(probe.surface_id, "terrain");
}

fn road_crossing() -> MapDocument {
    let mut d = minimal();
    // The 800 cm ground road runs along y = 25600; the 600 cm bridge along x = 25600.
    d.barriers.push(barrier("fence-1", BarrierKind::Fence, vec![[10000, 20000], [10000, 31000], [20000, 31000]]));
    d.barriers.push(barrier("wall-1", BarrierKind::Wall, vec![[40000, 22000], [43000, 29000]]));
    d
}

#[test]
fn barriers_are_solid_prisms_gated_at_road_corridors() {
    let d = road_crossing();
    d.validate().unwrap();
    let cell = Cell { x: 0, y: 0 };
    let g = generate_cell(&d, cell, None);
    let cost = estimate_generation(&d, cell, 500_000).unwrap();
    assert!(cost.triangles >= g.chunk.triangles.len() as u64);
    assert!(cost.asset_convexes >= g.chunk.asset_convexes.len() as u64);
    assert!(cost.occupied_solids >= g.solids.len() as u64);
    for id in ["fence-1", "wall-1"] {
        let convexes: Vec<_> = g.chunk.asset_convexes.iter().filter(|c| c.object_id == id).collect();
        // One piece on each side of the ground road (plus the fence's corner leg).
        assert!(convexes.len() >= 2, "{id}");
        for c in &convexes {
            assert!(c.shape.valid(100_000_000));
            for axis in [0, 2] {
                let extent = c.shape.vertices.iter().map(|v| v[axis]).max().unwrap() - c.shape.vertices.iter().map(|v| v[axis]).min().unwrap();
                assert!(extent <= 800 + 20 + 2, "{id}: pieces stay within 8 m");
            }
            for v in &c.shape.vertices {
                // Road half width 400 + margin 50: no barrier volume reaches the carriageway.
                assert!((v[2] - 25600).abs() > 400, "{id} {v:?}");
                assert!(v[1] == -25 || v[1] == 120, "flat terrain base/top {v:?}");
            }
        }
        let below = convexes.iter().any(|c| c.shape.vertices.iter().all(|v| v[2] < 25600));
        let above = convexes.iter().any(|c| c.shape.vertices.iter().all(|v| v[2] > 25600));
        assert!(below && above, "{id} keeps both sides of the gate");
        assert!(g.chunk.triangles.iter().filter(|t| t.object_id == id).all(|t| !t.spawnable && t.contact_class == 0));
        assert!(g.solids.iter().any(|s| s.object_id == id));
    }
    let hedge_faces = g.chunk.triangles.iter().filter(|t| t.object_id == "fence-1");
    assert!(hedge_faces.into_iter().all(|t| t.surface == Surface::Concrete));
    // A barrier is not a generated object; its faces and convexes carry its ID.
    assert!(!g.chunk.objects.iter().any(|o| o.id == "fence-1"));
}

#[test]
fn barrier_bases_follow_the_cell_heightmap() {
    let mut d = minimal();
    d.roads.clear();
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.heightmaps.push(Heightmap { cell: Cell { x: 1, y: 1 }, path: "h.png".into(), spacing_cm: 25600, offset_cm: 0, step_cm: 1, source_accuracy_cm: None });
    d.barriers.push(Barrier { id: "slope-wall".into(), kind: BarrierKind::Wall, line: vec![[52000, 60000], [100000, 90000]], height_cm: 300, thickness_cm: 40 });
    d.validate().unwrap();
    // Heights rise with x: 0, 512, 1024 cm per sample column.
    let grid = HeightGrid { side: 3, heights_cm: (0..9).map(|i| (i % 3) as i64 * 512).collect() };
    let cell = Cell { x: 1, y: 1 };
    let g = generate_cell(&d, cell, Some(&grid));
    let pieces: Vec<_> = g.chunk.asset_convexes.iter().filter(|c| c.object_id == "slope-wall").collect();
    assert!(pieces.len() >= 3, "split at grid lines and tile diagonals: {}", pieces.len());
    let height = |x: i64| (x - 51200) * 512 / 25600;
    for c in pieces {
        for v in &c.shape.vertices {
            let ground = height(v[0]);
            // Lateral offset of at most 20 cm on a 2 % slope stays within the 25 cm embed.
            assert!(v[1] <= ground + 300 + 2 && v[1] >= ground - 25 - 2, "{v:?} vs {ground}");
        }
    }
    let cost = estimate_generation(&d, cell, 500_000).unwrap();
    assert!(cost.triangles >= g.chunk.triangles.len() as u64);
}

#[test]
fn barrier_validation_and_identity() {
    let mut d = road_crossing();
    d.barriers[0].height_cm = 5;
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = road_crossing();
    d.barriers[0].thickness_cm = 101;
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = road_crossing();
    d.barriers[0].line = vec![[10000, 20000]];
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = road_crossing();
    d.barriers[0].line[1] = d.barriers[0].line[0];
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = road_crossing();
    d.barriers[0].line.push([-5, 0]);
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = road_crossing();
    d.barriers[0].id = "building-1".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    let mut d = road_crossing();
    d.barriers[0].id = "builtin:fence".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    // A closed perimeter is accepted.
    let mut d = road_crossing();
    d.barriers[1].line = vec![[60000, 60000], [64000, 60000], [64000, 64000], [60000, 60000]];
    d.validate().unwrap();
}

#[test]
fn barrier_piece_budget_refuses_runtime_and_warns_authoring() {
    let mut d = minimal();
    d.zones.clear();
    d.roads.clear();
    d.nodes.clear();
    // 126 zigzags of 4,000 segments: at least two pieces each, above the 1,000,000 Runtime value.
    for i in 0..126 {
        let y = 1000 + i * 800;
        let line = (0..4001).map(|k| [1000 + k * 20, y + if k % 2 == 0 { 0 } else { 300 }]).collect();
        d.barriers.push(Barrier { id: format!("zig-{i}"), kind: BarrierKind::Fence, line, height_cm: 100, thickness_cm: 5 });
    }
    let error = d.validate().unwrap_err();
    assert_eq!(error.code, "E_LIMIT");
    let warnings = d.validate_authoring().unwrap();
    assert!(warnings.iter().any(|w| w.code == "barrier_pieces" && w.runtime_rejects && w.limit == 1_000_000 && w.actual > 1_000_000));
}

fn parted() -> MapDocument {
    let mut d = minimal();
    // Parent 30000..32000 x 30000..34000; a touching taller tower and a detached annex.
    d.buildings[0].parts = vec![
        part(rect(32000, 30000, 34000, 32000), Some(4000)),
        BuildingPart { footprint: rect(36000, 30000, 38000, 32000), holes: vec![], base_cm: Some(100), height_cm: None },
    ];
    d
}

#[test]
fn building_parts_are_one_object_with_separate_prisms_and_collision() {
    let d = parted();
    d.validate().unwrap();
    let cell = Cell { x: 0, y: 0 };
    let g = generate_cell(&d, cell, None);
    let prisms: Vec<_> = g.chunk.building_prisms.iter().filter(|p| p.object_id == "building-1").collect();
    // Two triangles per rectangle, three solids.
    assert_eq!(prisms.len(), 6);
    let tops: std::collections::BTreeSet<_> = prisms.iter().map(|p| (p.bottom_cm, p.top_cm[0])).collect();
    assert_eq!(tops, [(0, 1500), (0, 4000), (100, 1600)].into_iter().collect());
    assert!(prisms.iter().all(|p| p.material == "concrete" && p.usage == "residential"));
    assert_eq!(g.solids.iter().filter(|s| s.object_id == "building-1").count(), 6);
    let cost = estimate_generation(&d, cell, 500_000).unwrap();
    assert!(cost.building_prisms >= g.chunk.building_prisms.len() as u64);
    assert!(cost.triangles >= g.chunk.triangles.len() as u64);
    // Overview lists every solid under the one record identity.
    let o = overview(&d).unwrap();
    assert_eq!(o.buildings.iter().filter(|b| b.id == "building-1").count(), 3);
    // A regional source that only reaches the annex keeps the whole record.
    let mut big = d.clone();
    big.bounds.max = [204800, 204800];
    big.buildings[0].parts[1].footprint = rect(60000, 60000, 62000, 62000);
    for road in &mut big.roads { road.sidewalk_cm = Some(0); }
    big.validate().unwrap();
    let region = region_source(&big, CellRegion { min: Cell { x: 1, y: 1 }, end: Cell { x: 2, y: 2 } }).unwrap();
    assert!(region.buildings.iter().any(|b| b.id == "building-1" && b.parts.len() == 2));
    let far = region_source(&big, CellRegion { min: Cell { x: 3, y: 3 }, end: Cell { x: 4, y: 4 } }).unwrap();
    assert!(far.buildings.is_empty(), "pruning stays active for parted buildings");
}

#[test]
fn building_parts_follow_building_rules() {
    let mut d = parted();
    d.buildings[0].parts[0].footprint = rect(31000, 30000, 34000, 32000);
    assert!(d.validate().unwrap_err().message.contains("parts overlap"), "parts overlapping the parent");
    let mut d = parted();
    d.buildings[0].parts[1].footprint = rect(33000, 31000, 35000, 33000);
    assert!(d.validate().unwrap_err().message.contains("parts overlap"), "parts overlapping each other");
    let mut d = parted();
    d.buildings[0].parts[1].footprint = rect(20000, 24000, 22000, 26000);
    assert!(d.validate().unwrap_err().message.contains("intersects road"), "part on the road corridor");
    let mut d = parted();
    d.buildings[0].parts[1].height_cm = Some(0);
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY");
    let mut d = parted();
    d.buildings[0].parts[1].footprint = vec![[36000, 30000], [38000, 30000], [36000, 32000], [38000, 32000]];
    assert_eq!(d.validate().unwrap_err().code, "E_GEOMETRY", "self-intersecting part");
    let mut d = parted();
    d.buildings.push(Building { id: "neighbour".into(), footprint: rect(37000, 31000, 39000, 33000), holes: vec![], base_cm: 0,
        height_cm: 500, usage: "public".into(), material: "brick".into(), roof: "flat".into(), entrances: vec![], parts: vec![] });
    assert!(d.validate().unwrap_err().message.contains("overlapping building"), "another record overlapping a part");
    let mut d = parted();
    d.placements.push(Placement { id: "bench".into(), asset_id: "builtin:streetlight".into(), position: [37000, 0, 31000], quarter_turns: 0, yaw_offset_mdeg: 0 });
    assert!(d.validate().unwrap_err().message.contains("intersects building building-1"), "placement inside a part");
    // A courtyard part, with a separate tower inside its courtyard.
    let mut d = parted();
    d.buildings[0].parts[1] = BuildingPart { footprint: rect(40000, 30000, 46000, 36000), holes: vec![rect(42000, 32000, 44000, 34000)], base_cm: None, height_cm: None };
    d.buildings[0].parts.push(part(rect(42500, 32500, 43500, 33500), Some(6000)));
    d.validate().unwrap();
    let g = generate_cell(&d, Cell { x: 0, y: 0 }, None);
    assert!(g.chunk.building_prisms.iter().any(|p| p.top_cm[0] == 6000));
}

#[test]
fn barriers_keep_regional_pruning_and_regional_pieces_match() {
    let mut d = road_crossing();
    d.bounds.max = [204800, 204800];
    for road in &mut d.roads { road.sidewalk_cm = Some(0); }
    d.barriers.push(barrier("far-hedge", BarrierKind::Hedge, vec![[180000, 180000], [190000, 185000]]));
    d.validate().unwrap();
    let region = region_source(&d, CellRegion { min: Cell { x: 0, y: 0 }, end: Cell { x: 1, y: 1 } }).unwrap();
    assert!(region.barriers.iter().any(|b| b.id == "fence-1"));
    assert!(!region.barriers.iter().any(|b| b.id == "far-hedge"));
    assert!(region.roads.len() <= d.roads.len());
    let whole = generate_cell(&d, Cell { x: 0, y: 0 }, None).chunk;
    let local = generate_cell(&region, Cell { x: 0, y: 0 }, None).chunk;
    let pieces = |c: &GeneratedChunk| c.asset_convexes.iter().filter(|v| v.object_id.contains('-') && d.barriers.iter().any(|b| b.id == v.object_id)).cloned().collect::<Vec<_>>();
    assert_eq!(pieces(&whole), pieces(&local));
}

fn sloped(d: &mut MapDocument, cell: Cell, spacing: u32) {
    d.roads.clear();
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.heightmaps.push(Heightmap { cell, path: "h.png".into(), spacing_cm: spacing, offset_cm: 0, step_cm: 1, source_accuracy_cm: None });
}

#[test]
fn cell_cost_stops_at_the_output_cap_and_generation_fails_closed() {
    let mut d = minimal();
    sloped(&mut d, Cell { x: 0, y: 0 }, 200);
    for i in 0..20 {
        d.barriers.push(barrier(&format!("long-{i}"), BarrierKind::Wall, vec![[100, 100 + i * 2000], [51100, 2100 + i * 2000]]));
    }
    d.validate().unwrap();
    let cell = Cell { x: 0, y: 0 };
    // 20 diagonal walls cut by a 2 m grid: thousands of pieces, far above 100.
    let cost = estimate_generation(&d, cell, 1200).unwrap();
    assert!(cost.asset_convexes <= 1200 / 12 + 2, "counting stops at the cap: {}", cost.asset_convexes);
    assert_eq!(cost.triangles, 1200);
    let side = 51200 / 200 + 1;
    let grid = HeightGrid { side, heights_cm: vec![0; side * side] };
    let e = generate(GenerationInput { document: &d, cell, heightgrid: Some(&grid), max_triangles: 1200 }).unwrap_err();
    assert_eq!(e.code, "E_BUDGET");
    let full = estimate_generation(&d, cell, 500_000).unwrap();
    assert!(full.asset_convexes > 1000);
}

#[test]
fn post_cut_piece_bound_refuses_dense_grids_at_validation() {
    let mut d = minimal();
    sloped(&mut d, Cell { x: 0, y: 0 }, 200);
    // 1,000 straight 900 m walls: few spans, but each cell grid and length cut
    // adds pieces; the bound exceeds 1,000,000 before any cell is visited.
    for i in 0..1000 {
        d.barriers.push(barrier(&format!("w-{i}"), BarrierKind::Wall, vec![[1000, 1000 + i * 100], [91000, 1000 + i * 100]]));
    }
    assert_eq!(d.validate().unwrap_err().code, "E_LIMIT");
    assert!(d.validate_authoring().unwrap().iter().any(|w| w.code == "barrier_pieces"));
}

#[test]
fn implicit_sidewalks_widen_gates() {
    let mut d = road_crossing();
    d.theme = "urban".into();
    d.validate().unwrap();
    // Urban 800 cm road, one nearby building: implicit 180 cm sidewalk.
    let g = generate_cell(&d, Cell { x: 0, y: 0 }, None);
    for c in g.chunk.asset_convexes.iter().filter(|c| c.object_id == "fence-1") {
        assert!(c.shape.vertices.iter().all(|v| (v[2] - 25600).abs() > 400 + 180), "{:?}", c.shape.vertices);
    }
}

#[test]
fn structures_gate_only_barriers_at_their_level() {
    let mut d = minimal();
    // The bridge deck is at 700 cm over x = 25600 (600 cm wide).
    d.barriers.push(barrier("low-fence", BarrierKind::Fence, vec![[20000, 10000], [31000, 10000]]));
    d.barriers.push(Barrier { id: "tall-wall".into(), kind: BarrierKind::Wall, line: vec![[20000, 12000], [31000, 12000]], height_cm: 1000, thickness_cm: 40 });
    d.validate().unwrap();
    let g = generate_cell(&d, Cell { x: 0, y: 0 }, None);
    let under = |id: &str| g.chunk.asset_convexes.iter().filter(|c| c.object_id == id)
        .any(|c| c.shape.vertices.iter().any(|v| (v[0] - 25600).abs() < 300));
    assert!(under("low-fence"), "a fence under the deck is kept");
    assert!(!under("tall-wall"), "a wall reaching the deck is gated");
    let cost = estimate_generation(&d, Cell { x: 0, y: 0 }, 500_000).unwrap();
    assert!(cost.asset_convexes >= g.chunk.asset_convexes.len() as u64 && cost.triangles >= g.chunk.triangles.len() as u64);
}

#[test]
fn bases_stay_below_cross_slopes_and_pieces_stay_short() {
    let mut d = minimal();
    let cell = Cell { x: 1, y: 1 };
    sloped(&mut d, cell, 400);
    d.barriers.push(Barrier { id: "terrace".into(), kind: BarrierKind::Wall, line: vec![[52000, 60000], [100000, 60010]], height_cm: 200, thickness_cm: 100 });
    d.validate().unwrap();
    // Steep 50 % rise with y across the wall's thickness.
    let side = 51200 / 400 + 1;
    let grid = HeightGrid { side, heights_cm: (0..side * side).map(|i| (i / side) as i64 * 200).collect() };
    let g = generate_cell(&d, cell, Some(&grid));
    let ground = |v: &Vertex| (v[2] - 51200) / 2;
    let pieces: Vec<_> = g.chunk.asset_convexes.iter().filter(|c| c.object_id == "terrace").collect();
    assert!(!pieces.is_empty());
    for c in pieces {
        let span = c.shape.vertices.iter().map(|v| v[0]).max().unwrap() - c.shape.vertices.iter().map(|v| v[0]).min().unwrap();
        assert!(span <= 800 + 2, "piece longer than 8 m: {span}");
        let bottom: Vec<_> = c.shape.vertices.iter().filter(|v| v[1] < ground(v) + 100).collect();
        assert_eq!(bottom.len(), 4);
        for v in bottom {
            assert!(v[1] <= ground(v) - EMBED, "{v:?} floats above {}", ground(v));
        }
    }
}
const EMBED: i64 = mapkit_core::barrier::EMBED_CM;

#[test]
fn default_spellings_normalize_to_absent() {
    let mut d = parted();
    d.ground_surface = Some(Surface::Grass);
    d.buildings[0].parts[0].height_cm = Some(d.buildings[0].height_cm);
    d.buildings[0].parts[1].base_cm = Some(d.buildings[0].base_cm);
    d.normalize();
    assert_eq!(d.ground_surface, None);
    assert_eq!(d.buildings[0].parts[0].height_cm, None);
    assert_eq!(d.buildings[0].parts[1].base_cm, None);
    assert_eq!(d.buildings[0].parts[1].height_cm, None);
}

#[test]
fn structure_gate_work_is_charged_at_validation() {
    let mut d = minimal();
    d.roads.clear();
    d.nodes.clear();
    // An 18,000-segment bridge beside six parallel 900 m walls: each wall span
    // tests every nearby structure for every bounded piece.
    let points: Vec<Vertex> = (0..=18_000).map(|i| [i * 5, 700, 10400]).collect();
    d.nodes = vec![RoadNode { id: "w".into(), position: points[0], level: 1 }, RoadNode { id: "e".into(), position: *points.last().unwrap(), level: 1 }];
    d.roads.push(Road { design: None, snow_retention_percent: 100, id: "deck".into(), from: "w".into(), to: "e".into(),
        widths_cm: vec![600; points.len() - 1], surfaces: vec![Surface::Concrete; points.len() - 1], points, kind: RoadKind::Bridge,
        clearance_cm: None, sidewalk_cm: None, markings: None });
    for (i, y) in [10000, 10020, 10040, 10060, 10080, 10100].into_iter().enumerate() {
        d.barriers.push(barrier(&format!("w-{i}"), BarrierKind::Wall, vec![[0, y], [90000, y]]));
    }
    let e = d.validate().unwrap_err();
    assert_eq!(e.code, "E_BUDGET", "{e}");
    assert!(d.validate_authoring().unwrap().iter().any(|w| w.code == "barrier_work"));
}

#[test]
fn bases_stay_below_creased_terrain() {
    let mut d = minimal();
    let cell = Cell { x: 1, y: 1 };
    sloped(&mut d, cell, 400);
    let lines = [([52000, 60000], [99000, 71000]), ([60000, 99000], [61000, 53000]), ([53000, 80000], [98000, 80000])];
    for (i, (a, b)) in lines.into_iter().enumerate() {
        d.barriers.push(Barrier { id: format!("crease-{i}"), kind: BarrierKind::Wall, line: vec![a, b], height_cm: 200, thickness_cm: 100 });
    }
    d.validate().unwrap();
    // Deterministic rough terrain: tile creases in every direction.
    let side = 51200 / 400 + 1;
    let grid = HeightGrid { side, heights_cm: (0..side * side).map(|i| ((i * 7919 + i / side * 104_729) % 613) as i64).collect() };
    let bounds = d.cell_bounds(cell).unwrap();
    let terrain = |p: Point| terrain_height(&bounds, 400, side, Some(&grid), 0, p);
    let g = generate_cell(&d, cell, Some(&grid));
    let mut checked = 0;
    for c in g.chunk.asset_convexes.iter().filter(|c| c.object_id.starts_with("crease-")) {
        // Bottom vertices 0,1 (one side) and 4,5 (other side): both long base edges.
        for (a, b) in [(0, 1), (4, 5)] {
            let (a, b) = (c.shape.vertices[a], c.shape.vertices[b]);
            for k in 0..=40 {
                let t = k as f64 / 40.0;
                let p = [a[0] + ((b[0] - a[0]) as f64 * t) as i64, a[2] + ((b[2] - a[2]) as f64 * t) as i64];
                let base = a[1] as f64 + (b[1] - a[1]) as f64 * t;
                // 1 cm quantization of sample and crease points on slopes up to 150 %.
                assert!(base <= (terrain(p) - EMBED) as f64 + 4.0, "{} floats at {p:?}: {base} vs {}", c.object_id, terrain(p));
                checked += 1;
            }
        }
    }
    assert!(checked > 1000);
}
