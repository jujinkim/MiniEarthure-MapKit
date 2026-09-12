use mapkit_core::*;

fn document(json: &str) -> MapDocument {
    serde_json::from_str(json).unwrap()
}

fn minimal() -> MapDocument {
    document(include_str!("../../../examples/minimal/document.json"))
}

fn at(d: &MapDocument, point: Point) -> CellRegion {
    let min = d.cell_at(point).unwrap();
    CellRegion {
        min,
        end: Cell {
            x: min.x + 1,
            y: min.y + 1,
        },
    }
}

fn compare(d: &MapDocument, regions: &[CellRegion]) {
    let allowance = RegionSourcePlan::allocation_bound(canonical(d).unwrap().len() as u64);
    for local in [false, true] {
        let plan = RegionSourcePlan::new(d, local, allowance, || Ok(())).unwrap();
        assert!(plan.allocated_bytes() <= allowance);
        for &region in regions {
            let expected = if local {
                local_region_source(d, region)
            } else {
                region_source(d, region)
            }
            .unwrap();
            assert_eq!(
                canonical(&plan.derive(region).unwrap()).unwrap(),
                canonical(&expected).unwrap(),
                "recipe {} local {local} region {region:?}",
                d.recipe_version,
            );
        }
    }
}

fn compare_all_cells(d: &MapDocument) {
    let regions: Vec<_> = d
        .cells()
        .into_iter()
        .map(|min| CellRegion {
            min,
            end: Cell {
                x: min.x + 1,
                y: min.y + 1,
            },
        })
        .collect();
    compare(d, &regions);
}

#[test]
fn plan_matches_frozen_derivation_across_recipes_fallbacks_and_input_order() {
    for recipe in 1..=6 {
        let mut d = minimal();
        d.recipe_version = recipe;
        if recipe >= 3 {
            for road in &mut d.roads {
                if road.kind == RoadKind::Ground {
                    road.sidewalk_cm = Some(0);
                }
            }
        }
        d.nodes.reverse();
        d.roads.reverse();
        d.buildings.reverse();
        d.zones.reverse();
        compare_all_cells(&d);
        if recipe >= 3 {
            let road = d
                .roads
                .iter_mut()
                .find(|r| r.kind == RoadKind::Ground)
                .unwrap();
            road.sidewalk_cm = None;
            compare_all_cells(&d);
            let road = d
                .roads
                .iter_mut()
                .find(|r| r.kind == RoadKind::Ground)
                .unwrap();
            road.sidewalk_cm = Some(0);
            d.repetitions.push(Repetition {
                id: "distant-repeat".into(),
                asset_id: "builtin:fence".into(),
                points: vec![[70000, 0, 10000], [80000, 0, 10000]],
                spacing_cm: 500,
            });
            compare_all_cells(&d);
        }
    }
    for json in [
        include_str!("../../../examples/assets/document.json"),
        include_str!("../../../examples/placement/document.json"),
        include_str!("../../../examples/courtyard/document.json"),
    ] {
        let mut d = document(json);
        d.assets.reverse();
        d.placements.reverse();
        d.surface_areas.reverse();
        d.heightmaps.reverse();
        compare_all_cells(&d);
    }
}

fn rotate(mut point: Point, turns: u8) -> Point {
    for _ in 0..turns {
        point = [-point[1], point[0]];
    }
    point
}

#[test]
fn plan_preserves_transformed_convex_footprints_and_separate_placement_anchor() {
    let assets = document(include_str!("../../../examples/assets/document.json"));
    for turns in 0..4 {
        let mut d = minimal();
        d.recipe_version = 6;
        d.bounds = Bounds {
            min: [-3000, -3000],
            max: [14000, 14000],
        };
        d.cell_size_cm = 200;
        d.roads.clear();
        d.nodes.clear();
        d.buildings.clear();
        d.zones.clear();
        d.assets = assets.assets.clone();
        let asset = d.assets.iter_mut().find(|a| a.id == "tinted").unwrap();
        // Odd extents and an offset collision proxy exercise the existing
        // integer footprint rounding after each of the four rotations.
        asset.convex_collision[0].vertices = vec![
            [6001, 0, -301],
            [6902, 0, -301],
            [6001, 0, 400],
            [6001, 801, -301],
        ];
        asset.collision = vec![CollisionBox {
            center: [6301, 101, -50],
            size_cm: [103, 203, 105],
        }];
        d.placements = vec![Placement {
            id: "offset-convex".into(),
            asset_id: "tinted".into(),
            position: [5000, 0, 5000],
            quarter_turns: turns,
        }];
        d.validate().unwrap();
        let shifted = |point| {
            let p = rotate(point, turns);
            [5000 + p[0], 5000 + p[1]]
        };
        let anchor = at(&d, [5000, 5000]);
        let proxy = at(&d, shifted([6001, -301]));
        let gap = at(&d, shifted([3000, 0]));
        compare(&d, &[anchor, proxy, gap]);
        let plan = RegionSourcePlan::new(&d, true, u64::MAX, || Ok(())).unwrap();
        for region in [anchor, proxy] {
            let source = plan.derive(region).unwrap();
            assert_eq!(source.placements.len(), 1);
            assert!(source.assets.iter().any(|a| a.id == "checker"));
        }
        // A union of the anchor and footprint AABBs would wrongly retain it.
        assert!(plan.derive(gap).unwrap().placements.is_empty());
    }
}

fn add_road(d: &mut MapDocument, id: &str, from: &str, to: &str, points: Vec<Vertex>, width: u32) {
    let template = document(include_str!("../../../examples/roads/document.json"));
    let mut road = template.roads[0].clone();
    road.id = id.into();
    road.from = from.into();
    road.to = to.into();
    road.widths_cm = vec![width; points.len() - 1];
    road.surfaces = vec![Surface::Asphalt; points.len() - 1];
    road.sidewalk_cm = Some(0);
    for (id, position) in [(from, points[0]), (to, *points.last().unwrap())] {
        if !d.nodes.iter().any(|n| n.id == id) {
            let mut node = template.nodes[0].clone();
            node.id = id.into();
            node.position = position;
            d.nodes.push(node);
        }
    }
    road.points = points;
    d.roads.push(road);
}

#[test]
fn plan_keeps_exact_segments_halo_touch_one_hop_and_input_order_widest_ties() {
    let mut d = minimal();
    d.recipe_version = 6;
    d.bounds = Bounds {
        min: [0, 0],
        max: [64000, 32000],
    };
    d.cell_size_cm = 1600;
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    add_road(
        &mut d,
        "local",
        "local-a",
        "joint-a",
        vec![[8000, 0, 8000], [15000, 0, 8000]],
        400,
    );
    add_road(
        &mut d,
        "one-hop",
        "joint-a",
        "joint-b",
        vec![[15000, 0, 8000], [18000, 0, 8000]],
        400,
    );
    add_road(
        &mut d,
        "two-hop",
        "joint-b",
        "joint-c",
        vec![[18000, 0, 8000], [19000, 0, 8000]],
        400,
    );
    add_road(
        &mut d,
        "aabb-gap",
        "gap-a",
        "gap-b",
        vec![
            [1000, 0, 1000],
            [20000, 0, 1000],
            [20000, 0, 24000],
            [1000, 0, 24000],
        ],
        400,
    );
    // Region max 9600 + vegetation 1501 + road 2002 = 13103.
    add_road(
        &mut d,
        "halo-touch",
        "touch-a",
        "touch-b",
        vec![[13103, 0, 11000], [13103, 0, 18000]],
        400,
    );
    add_road(
        &mut d,
        "widest-z",
        "wide-z-a",
        "wide-z-b",
        vec![[45000, 0, 16000], [46000, 0, 16000]],
        1000,
    );
    add_road(
        &mut d,
        "widest-a",
        "wide-a-a",
        "wide-a-b",
        vec![[50000, 0, 16000], [51000, 0, 16000]],
        1000,
    );
    let region = at(&d, [8000, 8000]);
    compare(&d, &[region]);
    let plan = RegionSourcePlan::new(&d, true, u64::MAX, || Ok(())).unwrap();
    let source = plan.derive(region).unwrap();
    let ids: Vec<_> = source.roads.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["halo-touch", "local", "one-hop", "widest-a"]);
    drop(plan);
    d.roads.reverse();
    d.nodes.reverse();
    compare(&d, &[region]);
    let plan = RegionSourcePlan::new(&d, true, u64::MAX, || Ok(())).unwrap();
    let source = plan.derive(region).unwrap();
    let ids: Vec<_> = source.roads.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["halo-touch", "local", "one-hop", "widest-z"]);
}

#[test]
fn plan_checks_allocation_and_cancellation_during_bounded_construction() {
    let mut d = minimal();
    d.recipe_version = 6;
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    d.placements = (0..200)
        .map(|i| Placement {
            id: format!("tree-{i}"),
            asset_id: "builtin:tree".into(),
            position: [1000 + (i % 20) * 1000, 0, 1000 + (i / 20) * 1000],
            quarter_turns: 0,
        })
        .collect();
    let allowance = RegionSourcePlan::allocation_bound(canonical(&d).unwrap().len() as u64);
    let allocated = RegionSourcePlan::new(&d, true, allowance, || Ok(()))
        .unwrap()
        .allocated_bytes();
    assert!(allocated <= allowance);
    assert_eq!(
        RegionSourcePlan::new(&d, true, allocated - 1, || Ok(()))
            .err()
            .unwrap()
            .code,
        "E_MEMORY_BUDGET",
    );
    let mut calls = 0;
    let cancelled = RegionSourcePlan::new(&d, true, allowance, || {
        calls += 1;
        if calls == 3 {
            Err(error("E_CANCELLED", "cancel during plan construction"))
        } else {
            Ok(())
        }
    });
    assert_eq!(cancelled.err().unwrap().code, "E_CANCELLED");
    assert_eq!(calls, 3);
    let mut calls = 0;
    RegionSourcePlan::new(&d, true, allowance, || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    assert!(
        calls >= 5,
        "200 placements need intermediate cancellation checks"
    );
}

#[test]
fn plan_cancels_fallback_global_scans_without_allocating_spatial_arrays() {
    for recipe in [1, 6] {
        for scan_roads in [false, true] {
            let mut d = minimal();
            let zone_template = d.zones[0].clone();
            d.recipe_version = recipe;
            d.nodes.clear();
            d.roads.clear();
            d.buildings.clear();
            d.zones.clear();
            if recipe == 6 {
                d.repetitions.push(Repetition {
                    id: "fallback-repeat".into(),
                    asset_id: "builtin:fence".into(),
                    points: vec![[1000, 0, 95000], [2000, 0, 95000]],
                    spacing_cm: 500,
                });
            }
            for i in 0..200 {
                let y = 1000 + i * 450;
                if scan_roads {
                    add_road(
                        &mut d,
                        &format!("road-{i}"),
                        &format!("from-{i}"),
                        &format!("to-{i}"),
                        vec![[1000, 0, y], [2000, 0, y]],
                        400,
                    );
                    if recipe == 1 {
                        d.roads.last_mut().unwrap().sidewalk_cm = None;
                    }
                } else {
                    let mut zone = zone_template.clone();
                    zone.id = format!("zone-{i}");
                    zone.polygon = vec![[1000, y], [1300, y], [1300, y + 300], [1000, y + 300]];
                    d.zones.push(zone);
                }
            }
            let allowance = RegionSourcePlan::allocation_bound(canonical(&d).unwrap().len() as u64);
            let mut calls = 0;
            let cancelled = RegionSourcePlan::new(&d, true, allowance, || {
                calls += 1;
                if calls == 3 {
                    Err(error("E_CANCELLED", "cancel a fallback global scan"))
                } else {
                    Ok(())
                }
            });
            assert_eq!(cancelled.err().unwrap().code, "E_CANCELLED");
            assert_eq!(calls, 3);
            let mut calls = 0;
            let plan = RegionSourcePlan::new(&d, true, allowance, || {
                calls += 1;
                Ok(())
            })
            .unwrap();
            assert_eq!(
                plan.allocated_bytes(),
                RegionSourcePlan::allocation_bound(0)
            );
            assert!(
                calls >= 6,
                "fallback recipe {recipe}, roads {scan_roads} needs intermediate checks"
            );
            let region = at(&d, [1000, 1000]);
            assert_eq!(
                canonical(&plan.derive(region).unwrap()).unwrap(),
                canonical(&region_source(&d, region).unwrap()).unwrap()
            );
        }
    }
}
