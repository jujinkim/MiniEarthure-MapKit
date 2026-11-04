mod support { pub mod basins; }
use mapkit_core::*;
#[test]
fn streamed_hash_matches_canonical_v6_for_empty_and_dense_escaped_content() {
    let mut chunk = GeneratedChunk { water_bodies: vec![], grind_lines: vec![], gimmicks: vec![], asset_convexes: vec![], building_prisms: vec![], format_version: GENERATED_VERSION,
        cell: Cell { x: -7, y: 12 }, triangles: vec![], objects: vec![] };
    assert_eq!(chunk.hash().unwrap(), sha256(&canonical(&chunk).unwrap()));
    for i in 0..8192 {
        chunk.triangles.push(Triangle { contact_class: 0, snow_retention_percent: 100, vertices: [[-i, i, 0], [i, 7, -9], [0, 0, i]],
            surface: Surface::Gravel, object_id: format!("道路\"\n{i}"), spawnable: i % 2 == 0 });
    }
    chunk.objects.push(GeneratedObject { id: "é\\\"".into(), asset_id: "builtin:tree".into(),
        position: [i64::MAX, -5, 0], quarter_turns: 3, yaw_offset_mdeg: 0,
    });
    assert_eq!(chunk.hash().unwrap(), sha256(&canonical(&chunk).unwrap()));
}

/// Water basins are a real-city rule: the same water, road and fence without
/// the OSM scale notice keep the hashes generated before basins existed, flat
/// and on a heightmap; with the notice the ground (and only then) changes.
#[test]
fn basin_rule_leaves_authored_water_hashes_frozen() {
    let frozen = [
        ("8e3c5cdf5ce3acbe7e503a9224519806a5f3c1adfc4fe3dbdde0eb2552af4b7f", "8b99b11ea162a8eaa89135f1d7b04f12e047141335853ff6637ee450679de551"),
        ("ac5921a91a28c02abb3e89b7f085c2d4abc34f30e7614b5537bfa20243884557", "c2d5b4c4293e1d42f961f7236732fbbabedcdbacbd9c34d91e58612a472d029d"),
    ];
    for osm in [false, true] {
        let d = support::basins::city(osm);
        let (sloped, grids) = support::basins::sloped(osm);
        for (cell, (flat_hash, sloped_hash)) in d.cells().into_iter().zip(frozen) {
            let flat = generate(support::basins::input(&d, &Default::default(), cell)).unwrap().hash().unwrap();
            let slope = generate(support::basins::input(&sloped, &grids, cell)).unwrap().hash().unwrap();
            assert_eq!((flat == flat_hash, slope == sloped_hash), (!osm, !osm), "{osm} {cell:?}");
        }
    }
}
