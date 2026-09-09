mod support {
    pub mod determinism;
}
use mapkit_core::*;
use std::collections::{BTreeMap, BTreeSet};
use support::determinism::*;

#[test]
fn portable_vectors_match_frozen_geometry_occupancy_queries_and_archives() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../spec/determinism-vectors.json")).unwrap();
    assert_eq!(
        vectors(),
        expected,
        "compare fixture/cell/component; never silently bless a new hash"
    );
}

#[test]
fn cell_order_source_order_queries_and_failed_attempts_do_not_change_generation() {
    for mut f in fixtures() {
        let before: BTreeMap<_, _> = f
            .document
            .cells()
            .into_iter()
            .map(|cell| {
                (
                    cell,
                    generate_with_occupancy(f.input(cell), MAX_OCCUPIED_SOLIDS).unwrap(),
                )
            })
            .collect();
        f.document.nodes.reverse();
        f.document.roads.reverse();
        f.document.buildings.reverse();
        f.document.zones.reverse();
        f.document.assets.reverse();
        f.document.placements.reverse();
        f.document.repetitions.reverse();
        f.document.heightmaps.reverse();
        f.document.attributions.reverse();
        for cell in f.document.cells().into_iter().rev() {
            let mut failed = f.input(cell);
            failed.max_triangles = 1;
            assert_eq!(generate(failed).unwrap_err().code, "E_BUDGET");
            let c = &before[&cell].chunk;
            let bounds = f.document.cell_bounds(cell).unwrap();
            for p in [
                bounds.max,
                bounds.min,
                [
                    (bounds.min[0] + bounds.max[0]) / 2,
                    (bounds.min[1] + bounds.max[1]) / 2,
                ],
            ] {
                let _ = c.spawn(&SpawnRequest {
                    position_cm: p,
                    surface_id: "terrain".into(),
                });
                let _ = c.spawn(&SpawnRequest {
                    position_cm: p,
                    surface_id: "absent".into(),
                });
                let _ = f.document.window(p);
            }
            let after = generate_with_occupancy(f.input(cell), MAX_OCCUPIED_SOLIDS).unwrap();
            assert_eq!(after.chunk, *c, "{} {cell:?}", f.name);
            assert_eq!(after.solids, before[&cell].solids);
            assert_eq!(
                generate(f.input(cell)).unwrap(),
                *c,
                "optional occupancy must not change geometry"
            );
        }
    }
}

#[test]
fn signed_terrain_seams_have_identical_heights_and_single_visual_owners() {
    let mut samples = 0;
    for f in fixtures() {
        let mut owners = BTreeSet::new();
        let cells: BTreeMap<_, _> = f
            .document
            .cells()
            .into_iter()
            .map(|cell| (cell, generate(f.input(cell)).unwrap()))
            .collect();
        for (cell, chunk) in &cells {
            for object in &chunk.objects {
                assert!(
                    owners.insert(object.id.clone()),
                    "duplicate instance {}",
                    object.id
                );
                assert_eq!(
                    f.document.cell_at([object.position[0], object.position[2]]),
                    Some(*cell)
                );
            }
        }
        if f.grids.is_empty() {
            continue;
        }
        for (cell, chunk) in &cells {
            let bounds = f.document.cell_bounds(*cell).unwrap();
            for axis in 0..2 {
                let next = Cell {
                    x: cell.x + i32::from(axis == 0),
                    y: cell.y + i32::from(axis == 1),
                };
                let Some(other) = cells.get(&next) else {
                    continue;
                };
                for along in bounds.min[1 - axis]..=bounds.max[1 - axis] {
                    let mut p = [0; 2];
                    p[axis] = bounds.max[axis];
                    p[1 - axis] = along;
                    let request = SpawnRequest {
                        position_cm: p,
                        surface_id: "terrain".into(),
                    };
                    assert_eq!(
                        chunk.spawn(&request).unwrap(),
                        other.spawn(&request).unwrap(),
                        "{} {p:?}",
                        f.name
                    );
                    samples += 1;
                }
            }
        }
    }
    assert_eq!(samples, 9480); // Every centimetre, including shared corners and partial edges.
}

#[test]
fn vegetation_seed_is_object_local_and_does_not_depend_on_unrelated_objects() {
    let mut f = fixtures().remove(2);
    let baseline: Vec<_> = f
        .document
        .cells()
        .into_iter()
        .flat_map(|cell| generate(f.input(cell)).unwrap().objects)
        .filter(|o| o.id.starts_with("forest:"))
        .collect();
    assert!(baseline.len() > 10);
    // A distant zone sorts first and changes its numeric index, but must not alter its stream.
    f.document.zones[0].id = "aaa-unrelated-orchard".into();
    let after: Vec<_> = f
        .document
        .cells()
        .into_iter()
        .flat_map(|cell| generate(f.input(cell)).unwrap().objects)
        .filter(|o| o.id.starts_with("forest:"))
        .collect();
    assert_eq!(baseline, after);
    f.document.seed += 1;
    let changed: Vec<_> = f
        .document
        .cells()
        .into_iter()
        .flat_map(|cell| generate(f.input(cell)).unwrap().objects)
        .filter(|o| o.id.starts_with("forest:"))
        .collect();
    assert_ne!(
        baseline, changed,
        "seed vector must actually exercise randomness"
    );
}

#[test]
fn oblique_sloped_road_seams_keep_coverage_height_and_unique_faces() {
    let mut samples = 0;
    for f in fixtures()
        .into_iter()
        .filter(|f| f.name.starts_with("oblique"))
    {
        let cells: BTreeMap<_, _> = f
            .document
            .cells()
            .into_iter()
            .map(|cell| (cell, generate(f.input(cell)).unwrap()))
            .collect();
        for (cell, chunk) in &cells {
            let mut faces = BTreeSet::new();
            for t in &chunk.triangles {
                let mut vertices = t.vertices;
                vertices.sort();
                assert!(
                    faces.insert((t.object_id.clone(), vertices)),
                    "duplicate face at {cell:?}"
                );
            }
            let bounds = f.document.cell_bounds(*cell).unwrap();
            for axis in 0..2 {
                let next = Cell {
                    x: cell.x + i32::from(axis == 0),
                    y: cell.y + i32::from(axis == 1),
                };
                let Some(other) = cells.get(&next) else {
                    continue;
                };
                for along in bounds.min[1 - axis]..=bounds.max[1 - axis] {
                    let mut point = [0; 2];
                    point[axis] = bounds.max[axis];
                    point[1 - axis] = along;
                    let request = SpawnRequest {
                        position_cm: point,
                        surface_id: "oblique".into(),
                    };
                    let a = chunk.spawn(&request);
                    let b = other.spawn(&request);
                    assert_eq!(a.is_ok(), b.is_ok(), "coverage {} {point:?}", f.name);
                    if let (Ok(a), Ok(b)) = (a, b) {
                        assert_eq!(a, b, "height {} {point:?}", f.name);
                        samples += 1;
                    }
                }
            }
        }
    }
    assert!(samples > 6000, "nonempty cm-by-cm bridge seams: {samples}");
}
