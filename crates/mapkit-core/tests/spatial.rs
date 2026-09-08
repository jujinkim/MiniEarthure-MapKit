use mapkit_core::*;
use std::collections::{BTreeMap, BTreeSet};

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
}

#[test]
fn invalid_unvalidated_topology_never_panics_or_invents_cells() {
    let mut cases = vec![];
    for size in [0, 1, 201, u32::MAX] {
        let mut d = document();
        d.cell_size_cm = size;
        cases.push(d);
    }
    for bounds in [
        Bounds {
            min: [i64::MIN; 2],
            max: [i64::MAX; 2],
        },
        Bounds {
            min: [0; 2],
            max: [0; 2],
        },
        Bounds {
            min: [2; 2],
            max: [1; 2],
        },
    ] {
        let mut d = document();
        d.bounds = bounds;
        cases.push(d);
    }
    let mut too_many = document();
    too_many.cell_size_cm = 200;
    cases.push(too_many);
    for d in cases {
        assert_eq!(d.cell_at(d.bounds.min), None);
        assert!(d.cells().is_empty());
        assert!(d.window(d.bounds.min).is_empty());
        assert!(!d.has_cell(Cell { x: 0, y: 0 }));
        assert_eq!(
            d.cell_bounds(Cell { x: 0, y: 0 }).unwrap_err().code,
            "E_CELL"
        );
        assert!(d.validate().is_err());
    }
}

#[test]
fn reserved_surface_and_generated_instance_ids_cannot_alias_authored_objects() {
    let mut d = document();
    d.buildings[0].id = "terrain".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    let mut d = document();
    d.buildings[0].id = format!("{}:60:60", d.zones[0].id);
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    // Signed global lattice coordinates, including zones whose ID contains colons.
    d.zones[0].id = "trees:west".into();
    d.buildings[0].id = "trees:west:-1:0".into();
    assert_eq!(d.validate().unwrap_err().code, "E_ID");
    for ordinary in [
        "trees:west:01:0",
        "trees:west:+1:0",
        "trees:west:-0:0",
        "trees:west:1:label",
        "another:1:0",
    ] {
        d.buildings[0].id = ordinary.into();
        d.validate().unwrap();
    }
}

#[test]
fn half_open_ownership_closed_references_and_partial_negative_origin_agree() {
    let mut d = document();
    d.bounds = Bounds {
        min: [-51_217, -51_219],
        max: [51_200, 19],
    };
    let origin = d.bounds.min;
    assert_eq!(d.cell_size_cm as i64, DEFAULT_CELL_CM);
    assert_eq!(
        d.cells(),
        (0..2)
            .flat_map(|y| (0..3).map(move |x| Cell { x, y }))
            .collect::<Vec<_>>()
    );
    for (point, expected) in [
        (origin, Cell { x: 0, y: 0 }),
        ([-18, -20], Cell { x: 0, y: 0 }),
        ([-17, -19], Cell { x: 1, y: 1 }),
        ([51_183, 19], Cell { x: 2, y: 1 }),
        (d.bounds.max, Cell { x: 2, y: 1 }),
    ] {
        assert_eq!(d.cell_at(point), Some(expected));
        assert!(d.cell_bounds(expected).unwrap().contains(point));
        assert!(d.window(point).into_iter().all(|c| d.has_cell(c)));
    }
    assert_eq!(d.cell_at([d.bounds.max[0] + 1, 0]), None);
    assert!(!d.has_cell(Cell {
        x: i32::MAX,
        y: i32::MAX
    }));
    let corner = Bounds {
        min: [-17, -19],
        max: [-17, -19],
    };
    assert_eq!(d.query_cells(&corner, 4).unwrap().geometry_cells.len(), 4);
    assert_eq!(
        d.cell_bounds(Cell { x: 2, y: 1 }).unwrap(),
        Bounds {
            min: [51_183, -19],
            max: [51_200, 19]
        }
    );
}

#[test]
fn anchors_are_owned_once_while_clipped_proxies_keep_the_original_id() {
    let mut d = document();
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    d.bounds = Bounds {
        min: [-600, -800],
        max: [201, 1],
    };
    d.cell_size_cm = 400;
    d.assets.push(Asset { convex_collision: vec![], material: None,
        id: "marker".into(),
        path: "assets/marker.png".into(),
        attribution: Attribution {
            source: "synthetic".into(),
            license: "MIT".into(),
            notice: "".into(),
        },
        collision: vec![CollisionBox {
            center: [0, 40, 0],
            size_cm: [40, 80, 40],
        }],
    });
    for (id, position) in [
        ("origin", [-600, 0, -800]),
        ("corner", [-200, 0, -400]),
        ("maximum", [201, 0, 1]),
    ] {
        d.placements.push(Placement {
            id: id.into(),
            asset_id: "marker".into(),
            position,
            quarter_turns: 1,
        });
    }
    let generate_cell = |d: &MapDocument, cell| {
        generate(GenerationInput {
            document: d,
            cell,
            heightgrid: None,
            max_triangles: 1000,
        })
        .unwrap()
    };
    let mut owners = BTreeMap::new();
    let mut references = BTreeSet::new();
    let before: BTreeMap<_, _> = d
        .cells()
        .into_iter()
        .map(|c| (c, generate_cell(&d, c)))
        .collect();
    for (&cell, chunk) in &before {
        for object in &chunk.objects {
            assert!(owners.insert(object.id.clone(), cell).is_none());
            assert_eq!(
                d.cell_at([object.position[0], object.position[2]]),
                Some(cell)
            );
        }
        for triangle in &chunk.triangles {
            assert!(triangle
                .vertices
                .iter()
                .all(|v| d.cell_bounds(cell).unwrap().contains([v[0], v[2]])));
            if triangle.object_id == "corner" {
                references.insert(cell);
            }
        }
    }
    assert_eq!(owners.len(), 3);
    assert_eq!(owners["maximum"], Cell { x: 2, y: 2 });
    assert_eq!(references.len(), 4);
    d.placements.reverse();
    for cell in d.cells().into_iter().rev() {
        assert_eq!(before[&cell], generate_cell(&d, cell));
    }
}
