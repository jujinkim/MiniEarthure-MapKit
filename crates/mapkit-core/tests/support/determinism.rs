//! Original MIT synthetic vectors shared by the native test and audit executable.
use mapkit_core::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Fixture {
    pub name: String,
    pub document: MapDocument,
    pub grids: BTreeMap<Cell, HeightGrid>,
}
impl Fixture {
    pub fn input(&self, cell: Cell) -> GenerationInput<'_> {
        GenerationInput {
            document: &self.document,
            cell,
            heightgrid: self.grids.get(&cell),
            max_triangles: 500_000,
        }
    }
}

pub fn fixtures() -> Vec<Fixture> {
    let mut out: Vec<_> = [
        (
            "recipe1",
            include_str!("../../../../examples/minimal/document.json"),
        ),
        (
            "recipe2",
            include_str!("../../../../examples/roads/document.json"),
        ),
        (
            "recipe3",
            include_str!("../../../../examples/placement/document.json"),
        ),
        (
            "recipe4",
            include_str!("../../../../examples/assets/document.json"),
        ),
    ]
    .into_iter()
    .map(|(name, source)| Fixture {
        name: name.into(),
        document: serde_json::from_str(source).unwrap(),
        grids: BTreeMap::new(),
    })
    .collect();
    // A signed global lattice and non-cell-aligned origin exercise div_euclid,
    // object/rule seeds and integer clipping independently of the positive examples.
    let mut d = out[2].document.clone();
    let shift = |p: &mut Point| {
        p[0] -= 17_137;
        p[1] -= 13_019;
    };
    let shift_vertex = |p: &mut Vertex| {
        p[0] -= 17_137;
        p[2] -= 13_019;
    };
    shift(&mut d.bounds.min);
    shift(&mut d.bounds.max);
    d.seed = 9_007_199_254_740_991;
    for n in &mut d.nodes {
        shift_vertex(&mut n.position);
    }
    for r in &mut d.roads {
        for p in &mut r.points {
            shift_vertex(p);
        }
    }
    for b in &mut d.buildings {
        for p in &mut b.footprint {
            shift(p);
        }
        for e in &mut b.entrances {
            for p in e {
                shift(p);
            }
        }
    }
    for z in &mut d.zones {
        for p in &mut z.polygon {
            shift(p);
        }
        for e in &mut z.exclusions {
            for p in e {
                shift(p);
            }
        }
    }
    for p in &mut d.placements {
        shift_vertex(&mut p.position);
    }
    for r in &mut d.repetitions {
        for p in &mut r.points {
            shift_vertex(p);
        }
    }
    out.push(Fixture {
        name: "signed-placement".into(),
        document: d,
        grids: BTreeMap::new(),
    });
    for recipe in 1..=4 {
        let mut d = out[3].document.clone();
        d.recipe_version = recipe;
        d.assets.clear();
        d.placements.clear();
        d.bounds = Bounds {
            min: [-837, -851],
            max: [436, 242],
        };
        d.cell_size_cm = 800;
        let mut grids = BTreeMap::new();
        for cell in d.cells() {
            d.heightmaps.push(Heightmap {
                cell,
                path: format!("terrain/{}-{}.png", cell.x, cell.y),
                spacing_cm: 200,
                offset_cm: -4000,
                step_cm: 1,
                source_accuracy_cm: Some(3000),
            });
            grids.insert(
                cell,
                HeightGrid {
                    side: 5,
                    heights_cm: (0..5)
                        .flat_map(|y| {
                            (0..5).map(move |x| {
                                -1000
                                    + i64::from(cell.x * 4 + x) * 6
                                    + i64::from(cell.y * 4 + y) * 12
                            })
                        })
                        .collect(),
                },
            );
        }
        out.push(Fixture {
            name: format!("signed-terrain-v{recipe}"),
            document: d,
            grids,
        });
    }
    for recipe in 1..=4 {
        let mut d = out[3].document.clone();
        d.recipe_version = recipe;
        d.assets.clear();
        d.placements.clear();
        d.bounds = Bounds {
            min: [-15037, -14019],
            max: [10563, 11581],
        };
        d.nodes = vec![
            RoadNode {
                id: "a".into(),
                position: [-14300, 313, -13319],
                level: 1,
            },
            RoadNode {
                id: "b".into(),
                position: [9963, 1533, 10981],
                level: 1,
            },
        ];
        d.roads = vec![Road {
            id: "oblique".into(),
            from: "a".into(),
            to: "b".into(),
            points: d.nodes.iter().map(|n| n.position).collect(),
            widths_cm: vec![613],
            surfaces: vec![Surface::Gravel],
            kind: RoadKind::Bridge,
            clearance_cm: None,
            sidewalk_cm: None,
            markings: None,
        }];
        out.push(Fixture {
            name: format!("oblique-bridge-v{recipe}"),
            document: d,
            grids: BTreeMap::new(),
        });
    }
    out
}

pub fn record(fixture: &Fixture) -> Value {
    let mut d = fixture.document.clone();
    d.normalize();
    let grids: Vec<_> = fixture
        .grids
        .iter()
        .map(|(cell, grid)| {
            json!({
                "cell": cell, "side": grid.side, "heights_cm": grid.heights_cm,
            })
        })
        .collect();
    // Vector identity includes restored height inputs, not just PNG descriptors.
    let input_hash = sha256(&canonical(&json!({"document": d, "grids": grids})).unwrap());
    let cells: Vec<_> = d
        .cells()
        .into_iter()
        .map(|cell| {
            let generated =
                generate_with_occupancy(fixture.input(cell), MAX_OCCUPIED_SOLIDS).unwrap();
            let c = &generated.chunk;
            let cost = estimate_generation(&d, cell, 500_000).unwrap();
            let key = archive_key(&input_hash, cell);
            let archive = encode_archive(c, &key, archive_limit(&cost)).unwrap();
            assert_eq!(decode_archive(&archive, &key, cell, &cost).unwrap(), *c);
            assert_eq!(c.hash().unwrap(), sha256(&canonical(c).unwrap()));
            let probes: Vec<_> = c
                .triangles
                .iter()
                .filter(|t| t.spawnable)
                .step_by(17)
                .take(8)
                .map(|t| {
                    let p = t.vertices[0];
                    c.surface_probe(&SpawnRequest {
                        position_cm: [p[0], p[2]],
                        surface_id: t.object_id.clone(),
                    })
                    .unwrap()
                })
                .collect();
            json!({"cell": cell, "generated_sha256": c.hash().unwrap(),
            "triangles": c.triangles.len(), "objects": c.objects.len(),
            "building_prisms": c.building_prisms.len(), "asset_convexes": c.asset_convexes.len(),
            "triangles_sha256": sha256(&canonical(&c.triangles).unwrap()),
            "objects_sha256": sha256(&canonical(&c.objects).unwrap()),
            "occupancy_sha256": sha256(&canonical(&occupancy_vector(&generated.solids)).unwrap()),
            "archive_sha256": sha256(&archive), "archive_bytes": archive.len(),
            "surface_probes": probes})
        })
        .collect();
    json!({"name": fixture.name, "input_sha256": input_hash, "cells": cells})
}

fn occupancy_vector(solids: &[OccupiedSolid]) -> Vec<Value> {
    // Test-only named encoding; the optional sidecar has no public JSON wire format.
    solids.iter().map(|s| {
        let shape = match &s.shape {
            SolidShape::Box { min, max } => json!({"kind":"box", "min":min, "max":max}),
            SolidShape::TriangularPrism { footprint, bottom_cm, top_cm } =>
                json!({"kind":"prism", "footprint":footprint, "bottom_cm":bottom_cm, "top_cm":top_cm}),
            SolidShape::SlopedPrism { footprint, bottom_cm, top_cm } =>
                json!({"kind":"sloped_prism", "footprint":footprint, "bottom_cm":bottom_cm, "top_cm":top_cm}),
            SolidShape::Convex(shape) => json!({"kind":"convex", "shape":shape}),
        };
        json!({"object_id":s.object_id, "shape":shape})
    }).collect()
}

pub fn vectors() -> Value {
    json!({"vector_version": 1, "generated_version": GENERATED_VERSION,
        "fixtures": fixtures().iter().map(record).collect::<Vec<_>>()})
}
