use mapkit_core::*;

fn document() -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.recipe_version = 7;
    d.bounds = Bounds { min: [0,0], max: [3200,1600] };
    d.cell_size_cm = 1600;
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.placements.clear();
    d.assets.clear(); d.zones.clear(); d.heightmaps.clear(); d.repetitions.clear();
    d.assets.push(Asset {
        id: "shared-tree".into(), path: "assets/tree.glb".into(),
        attribution: Attribution { source: "synthetic test".into(), license: "MIT".into(), notice: "Original test geometry".into() },
        collision: vec![CollisionBox { center: [0,12,0], size_cm: [300,24,300] },
            CollisionBox { center: [0,100,0], size_cm: [16,180,16] }],
        convex_collision: vec![], material: None,
    });
    d.zones.push(Zone {
        id: "garden".into(), polygon: vec![[100,100],[3100,100],[3100,1500],[100,1500]],
        kind: ZoneKind::Orchard, spacing_cm: 500, density_per_mille: 1000, exclusions: vec![],
        tree: Some(ZoneTree { asset_id: "shared-tree".into(), radius_cm: 150, clearance_cm: 5 }),
    });
    d
}

fn generate_cell(d: &MapDocument, cell: Cell) -> GeneratedOccupancy {
    generate_with_occupancy(GenerationInput { document: d, cell, heightgrid: None, max_triangles: 500_000 }, MAX_OCCUPIED_SOLIDS).unwrap()
}

#[test]
fn custom_tree_requires_opt_in_real_asset_and_enclosing_footprint() {
    let d = document(); d.validate().unwrap();
    let mut bad = d.clone(); bad.recipe_version = 6;
    assert_eq!(bad.validate().unwrap_err().code, "E_VERSION");
    let mut bad = d.clone(); bad.zones[0].tree.as_mut().unwrap().asset_id = "missing".into();
    assert_eq!(bad.validate().unwrap_err().code, "E_ASSET");
    for radius in [0,149,201] {
        let mut bad = d.clone(); bad.zones[0].tree.as_mut().unwrap().radius_cm = radius;
        assert_eq!(bad.validate().unwrap_err().code, "E_ASSET");
    }
    let mut bad = d.clone(); bad.assets[0].collision.clear();
    assert_eq!(bad.validate().unwrap_err().code, "E_ASSET");
    let mut old = d; old.recipe_version = 6; old.zones[0].tree = None;
    assert!(!String::from_utf8(canonical(&old).unwrap()).unwrap().contains("\"tree\":"));
    old.validate().unwrap();
}

#[test]
fn density_spacing_exclusions_and_polygon_control_count_without_relocation() {
    let mut d = document();
    let objects = |d: &MapDocument| d.cells().into_iter().flat_map(|c| generate_cell(d,c).chunk.objects).collect::<Vec<_>>();
    let original = objects(&d); assert!(!original.is_empty());
    assert!(original.iter().all(|o| o.asset_id == "shared-tree"));
    d.zones[0].density_per_mille = 0; assert!(objects(&d).is_empty());
    d.zones[0].density_per_mille = 1000; d.zones[0].spacing_cm = 1000;
    assert!(objects(&d).len() < original.len());
    d.zones[0].spacing_cm = 500;
    d.zones[0].exclusions.push(vec![[300,300],[800,300],[800,800],[300,800]]);
    let fewer = objects(&d);
    assert!(fewer.len() < original.len());
    assert!(fewer.iter().all(|o| original.contains(o)), "rejected trees must not move elsewhere");
    d.zones[0].polygon = vec![[100,100],[300,100],[300,1500],[100,1500]];
    d.zones[0].exclusions.clear();
    assert!(objects(&d).is_empty(), "too narrow for the declared canopy");
}

#[test]
fn seam_collision_owner_is_queried_from_regular_and_indexed_metadata() {
    let d = document();
    let left = generate_cell(&d, Cell { x:0,y:0 });
    let right = generate_cell(&d, Cell { x:1,y:0 });
    let id = "garden:3:1";
    assert!(left.chunk.objects.iter().any(|o| o.id == id));
    assert!(!right.chunk.objects.iter().any(|o| o.id == id));
    assert!(left.chunk.triangles.iter().filter(|t| t.object_id == id)
        .flat_map(|t| t.vertices).any(|v| v[0] == 1650));
    let query = Bounds { min:[1640,500], max:[1640,500] };
    for source in [&d, &source_metadata(&d)] {
        let cells = source.query_cells(&query,2).unwrap();
        assert_eq!(cells.geometry_cells,vec![Cell { x:1,y:0 }]);
        assert!(cells.occupancy_cells.contains(&Cell { x:0,y:0 }));
        assert_eq!(source.query_cells(&query,1).unwrap_err().code,"E_BUDGET");
    }
}

#[test]
fn region_source_keeps_tree_assets_and_cost_bounds_all_output() {
    let d = document();
    for cell in d.cells() {
        let expected = generate_cell(&d,cell);
        let cost = estimate_generation(&d,cell,500_000).unwrap();
        assert!(expected.solids.len() as u64 <= cost.occupied_solids);
        assert!(expected.chunk.triangles.len() as u64 <= cost.triangles);
        assert!(expected.chunk.objects.len() as u64 <= cost.objects);
        for local in [false,true] {
            let region = CellRegion { min:cell,end:Cell { x:cell.x+1,y:cell.y+1 } };
            let source = if local { local_region_source(&d,region) } else { region_source(&d,region) }.unwrap();
            assert_eq!(source.assets,d.assets);
            let actual = generate_cell(&source,cell);
            assert_eq!(actual.chunk,expected.chunk);
            assert_eq!(actual.solids,expected.solids);
        }
    }
}
