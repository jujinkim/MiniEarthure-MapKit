use mapkit_core::*;

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}

#[test]
fn closed_seams_outer_edges_and_negative_origins_match_exhaustive_intersection() {
    let mut d = document();
    d.cell_size_cm = 200;
    d.bounds = Bounds {
        min: [-400, -600],
        max: [501, 201],
    };
    for x in [
        -1000, -421, -420, -401, -400, -399, -201, -200, -199, 0, 200, 500, 501, 521, 522,
    ] {
        for y in [-1000, -620, -600, -599, -400, 0, 200, 201, 221, 222] {
            for size in [0, 1, 19, 200, 501] {
                let bounds = Bounds {
                    min: [x, y],
                    max: [x + size, y + size],
                };
                let plan = d.query_cells(&bounds, 25).unwrap();
                let expected = |halo| {
                    d.cells()
                        .into_iter()
                        .filter(|&c| {
                            let cell = d.cell_bounds(c).unwrap();
                            (0..2).all(|a| {
                                cell.max[a] >= bounds.min[a] - halo
                                    && cell.min[a] <= bounds.max[a] + halo
                            })
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(plan.geometry_cells, expected(0), "{bounds:?}");
                assert_eq!(plan.occupancy_cells, expected(20), "{bounds:?}");
            }
        }
    }
}

#[test]
fn query_collects_protruding_trunk_from_neighbor_owner() {
    let mut d = document();
    d.cell_size_cm = 50000;
    d.buildings.clear();
    d.zones[0].polygon = vec![
        [49000, 30000],
        [51000, 30000],
        [51000, 32000],
        [49000, 32000],
    ];
    let plan = d
        .query_cells(
            &Bounds {
                min: [49999, 31000],
                max: [49999, 31000],
            },
            2,
        )
        .unwrap();
    assert_eq!(plan.geometry_cells, [Cell { x: 0, y: 0 }]);
    assert_eq!(
        plan.occupancy_cells,
        [Cell { x: 0, y: 0 }, Cell { x: 1, y: 0 }]
    );
    let solids: Vec<_> = plan
        .occupancy_cells
        .iter()
        .flat_map(|&cell| {
            generate_with_occupancy(
                GenerationInput {
                    document: &d,
                    cell,
                    heightgrid: None,
                    max_triangles: 500_000,
                },
                100,
            )
            .unwrap()
            .solids
        })
        .collect();
    assert!(solids
        .iter()
        .any(|solid| solid.object_id == "orchard-1:50:31"
            && matches!(
                solid.shape, SolidShape::Box { min, max } if min[0] <= 49999 && max[0] >= 49999
            )));
}

#[test]
fn malformed_topology_bounds_and_union_budget_fail_closed() {
    let mut d = document();
    let q = Bounds {
        min: [51199, 100],
        max: [51199, 100],
    };
    assert_eq!(d.query_cells(&q, 1).unwrap_err().code, "E_BUDGET");
    assert!(d.query_cells(&q, 2).is_ok());
    for cap in [0, 16385, usize::MAX] {
        assert_eq!(d.query_cells(&q, cap).unwrap_err().code, "E_BUDGET");
    }
    for bad in [
        Bounds {
            min: [2, 0],
            max: [1, 0],
        },
        Bounds {
            min: [i64::MIN, 0],
            max: [i64::MAX, 0],
        },
    ] {
        assert_eq!(d.query_cells(&bad, 16).unwrap_err().code, "E_QUERY");
    }
    d.cell_size_cm = 0;
    assert_eq!(d.query_cells(&q, 16).unwrap_err().code, "E_DOCUMENT");
    d.cell_size_cm = 200;
    assert_eq!(d.query_cells(&q, 16).unwrap_err().code, "E_LIMIT");
    d.bounds.min = [i64::MIN; 2];
    assert_eq!(d.query_cells(&q, 16).unwrap_err().code, "E_DOCUMENT");
}
