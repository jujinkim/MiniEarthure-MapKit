use mapkit_core::*;
use mapkit_package::*;
use std::collections::{BTreeMap, BTreeSet};

fn document() -> MapDocument {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear();
    d.roads.clear();
    d.buildings.clear();
    d.zones.clear();
    d.cell_size_cm = 800;
    d.bounds = Bounds {
        min: [-837, -851],
        max: [436, 242],
    };
    d
}
fn png(side: u32, samples: &[u16]) -> Vec<u8> {
    assert_eq!(samples.len(), (side * side) as usize);
    let mut bytes = vec![];
    {
        let mut e = png::Encoder::new(&mut bytes, side, side);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Sixteen);
        e.write_header()
            .unwrap()
            .write_image_data(
                &samples
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    bytes
}
fn fixture() -> (MapDocument, BTreeMap<String, Vec<u8>>) {
    let mut d = document();
    let mut files = BTreeMap::new();
    for cell in d.cells() {
        let step = [1, 2, 3, 6][(cell.y * 2 + cell.x) as usize];
        let h = Heightmap {
            cell,
            path: format!("terrain/{}-{}.png", cell.x, cell.y),
            spacing_cm: 200,
            offset_cm: -4000,
            step_cm: step,
            source_accuracy_cm: Some(3000),
        };
        let samples = (0..5)
            .flat_map(|y| {
                (0..5).map(move |x| {
                    ((3000 + (cell.x * 4 + x) * 6 + (cell.y * 4 + y) * 12) / step as i32) as u16
                })
            })
            .collect::<Vec<_>>();
        files.insert(h.path.clone(), png(5, &samples));
        d.heightmaps.push(h);
    }
    (d, files)
}

#[test]
fn unsigned_big_endian_samples_offset_step_and_default_two_metre_grid() {
    let h = Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "terrain/h.png".into(),
        spacing_cm: 200,
        offset_cm: -1_000_000,
        step_cm: 100,
        source_accuracy_cm: None,
    };
    let samples = [0, 1, 256, 65535];
    assert_eq!(
        decode_heightmap(&h, 200, &png(2, &samples))
            .unwrap()
            .heights_cm,
        [-1_000_000, -999_900, -974_400, 5_553_500]
    );
    let full =
        decode_heightmap(&h, DEFAULT_CELL_CM as u32, &png(257, &vec![123; 257 * 257])).unwrap();
    assert_eq!(full.side, 257);
    assert!(full.heights_cm.iter().all(|v| *v == -987_700));
    for bad_size in [0, 1, 201, 102_600, u32::MAX] {
        assert_eq!(
            decode_heightmap(&h, bad_size, &png(1, &[0]))
                .unwrap_err()
                .code,
            "E_HEIGHTMAP"
        );
    }
    for (offset, step, spacing) in [
        (i64::MIN, 1, 200),
        (i64::MAX, 1, 200),
        (0, 0, 200),
        (0, 101, 200),
        (0, 1, 0),
        (0, 1, 300),
    ] {
        let bad = Heightmap {
            offset_cm: offset,
            step_cm: step,
            spacing_cm: spacing,
            ..h.clone()
        };
        assert_eq!(
            decode_heightmap(&bad, 800, &png(5, &[0; 25]))
                .unwrap_err()
                .code,
            "E_HEIGHTMAP"
        );
    }
    assert_eq!(
        decode_heightmap(&h, 800, &png(4, &[0; 16]))
            .unwrap_err()
            .code,
        "E_HEIGHTMAP"
    );
}

#[test]
fn partial_cells_preserve_shared_restored_edges_cover_area_and_source_accuracy_independence() {
    let (d, files) = fixture();
    let bytes = pack_bytes(d.clone(), files.clone()).unwrap();
    let p = read_bytes(&bytes).unwrap();
    let chunks: BTreeMap<_, _> = d
        .cells()
        .into_iter()
        .rev()
        .map(|c| (c, p.generate(c, 1000).unwrap()))
        .collect();
    for (&cell, chunk) in &chunks {
        let b = d.cell_bounds(cell).unwrap();
        let area: i128 = chunk
            .triangles
            .iter()
            .map(|t| {
                assert!(t.vertices.iter().all(|v| b.contains([v[0], v[2]])));
                let [a, b, c] = t.vertices;
                let twice = (b[0] - a[0]) as i128 * (c[2] - a[2]) as i128
                    - (b[2] - a[2]) as i128 * (c[0] - a[0]) as i128;
                assert!(twice > 0);
                twice
            })
            .sum();
        assert_eq!(
            area,
            2 * (b.max[0] - b.min[0]) as i128 * (b.max[1] - b.min[1]) as i128
        );
        let cost = estimate_generation(&d, cell, 1000).unwrap();
        assert!(cost.triangles >= chunk.triangles.len() as u64);
        assert_eq!(cost.height_samples, 25); // partial cells still decode a full grid
        for (axis, neighbor) in [
            (
                0,
                Cell {
                    x: cell.x + 1,
                    y: cell.y,
                },
            ),
            (
                2,
                Cell {
                    x: cell.x,
                    y: cell.y + 1,
                },
            ),
        ] {
            if let Some(next) = chunks.get(&neighbor) {
                let limit = b.max[usize::from(axis == 2)];
                let edge = |c: &GeneratedChunk| {
                    c.triangles
                        .iter()
                        .flat_map(|t| t.vertices)
                        .filter(|v| v[axis] == limit)
                        .collect::<BTreeSet<_>>()
                };
                assert!(!edge(chunk).is_empty());
                assert_eq!(edge(chunk), edge(next));
            }
        }
    }
    let mut changed = d.clone();
    changed.heightmaps.reverse();
    for h in &mut changed.heightmaps {
        h.source_accuracy_cm = None;
    }
    let other = read_bytes(&pack_bytes(changed, files).unwrap()).unwrap();
    assert_ne!(
        p.inspection.world_content_hash,
        other.inspection.world_content_hash
    );
    for (cell, chunk) in chunks {
        assert_eq!(chunk, other.generate(cell, 1000).unwrap());
    }
}

#[test]
fn full_padding_edges_and_implicit_flat_neighbors_are_validated_before_generation() {
    let (d, mut files) = fixture();
    // The top padding sample is outside the partial map but still part of the
    // required complete shared grid edge; don't silently discard corrupted data.
    let h = d
        .heightmaps
        .iter()
        .find(|h| h.cell == Cell { x: 1, y: 1 })
        .unwrap();
    let g = decode_heightmap(h, 800, &files[&h.path]).unwrap();
    let mut samples: Vec<_> = g
        .heights_cm
        .iter()
        .map(|v| ((v - h.offset_cm) / h.step_cm as i64) as u16)
        .collect();
    samples[20] += 1;
    files.insert(h.path.clone(), png(5, &samples));
    assert_eq!(pack_bytes(d, files).unwrap_err().code, "E_SEAM");
    let mut d = document();
    d.heightmaps.push(Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "terrain/flat.png".into(),
        spacing_cm: 200,
        offset_cm: -100,
        step_cm: 2,
        source_accuracy_cm: Some(3000),
    });
    assert!(pack_bytes(
        d.clone(),
        BTreeMap::from([("terrain/flat.png".into(), png(5, &[50; 25]))])
    )
    .is_ok());
    assert_eq!(
        pack_bytes(
            d,
            BTreeMap::from([("terrain/flat.png".into(), png(5, &[51; 25]))])
        )
        .unwrap_err()
        .code,
        "E_SEAM"
    );
    let (mut d, mut files) = fixture();
    let h = &mut d.heightmaps[1];
    h.spacing_cm = 400;
    files.insert(h.path.clone(), png(3, &[1500; 9]));
    assert_eq!(pack_bytes(d, files).unwrap_err().code, "E_SEAM");
}

#[test]
fn surface_queries_round_the_shared_height_independently_of_triangle_origin() {
    let mut d = document();
    d.bounds = Bounds {
        min: [0, 0],
        max: [400, 200],
    };
    d.cell_size_cm = 200;
    let mut files = BTreeMap::new();
    for x in 0..2 {
        let path = format!("terrain/{x}.png");
        d.heightmaps.push(Heightmap {
            cell: Cell { x, y: 0 },
            path: path.clone(),
            spacing_cm: 200,
            offset_cm: -50,
            step_cm: 1,
            source_accuracy_cm: None,
        });
        files.insert(
            path,
            png(
                2,
                if x == 0 {
                    &[100, 0, 100, 100]
                } else {
                    &[0, 0, 100, 100]
                },
            ),
        );
    }
    let p = read_bytes(&pack_bytes(d, files).unwrap()).unwrap();
    let left = p.generate(Cell { x: 0, y: 0 }, 10).unwrap();
    let right = p.generate(Cell { x: 1, y: 0 }, 10).unwrap();
    for y in 0..=200 {
        let request = SpawnRequest {
            position_cm: [200, y],
            surface_id: "terrain".into(),
        };
        assert_eq!(
            left.spawn(&request).unwrap(),
            right.spawn(&request).unwrap(),
            "shared y={y}"
        );
    }
}

#[test]
fn recipe_v1_tree_anchor_quantization_stays_frozen_while_queries_are_corrected() {
    let mut d = document();
    d.cell_size_cm = 400;
    d.bounds = Bounds {
        min: [-1, -1],
        max: [399, 399],
    };
    d.zones.push(Zone {
                tree: None,
        id: "orchard".into(),
        polygon: vec![[-1, -1], [399, -1], [399, 399], [-1, 399]],
        kind: ZoneKind::Orchard,
        spacing_cm: 200,
        density_per_mille: 1000,
        exclusions: vec![],
    });
    d.heightmaps.push(Heightmap {
        cell: Cell { x: 0, y: 0 },
        path: "terrain/h.png".into(),
        spacing_cm: 200,
        offset_cm: -50,
        step_cm: 1,
        source_accuracy_cm: None,
    });
    let p = read_bytes(
        &pack_bytes(
            d,
            BTreeMap::from([(
                "terrain/h.png".into(),
                png(3, &[100, 0, 0, 100, 100, 100, 100, 100, 100]),
            )]),
        )
        .unwrap(),
    )
    .unwrap();
    let chunk = p.generate(Cell { x: 0, y: 0 }, 1000).unwrap();
    let tree = chunk
        .objects
        .iter()
        .find(|o| o.id == "orchard:1:0")
        .unwrap();
    assert_eq!(
        tree.position,
        [200, -50, 0],
        "recipe-v1 generated tree bytes must not migrate"
    );
    assert_eq!(
        chunk
            .spawn(&SpawnRequest {
                position_cm: [200, 0],
                surface_id: "terrain".into()
            })
            .unwrap(),
        [200, -49, 0]
    );
}
