use mapkit_core::*;
#[test]
fn streamed_hash_matches_canonical_v6_for_empty_and_dense_escaped_content() {
    let mut chunk = GeneratedChunk { building_prisms: vec![], format_version: GENERATED_VERSION,
        cell: Cell { x: -7, y: 12 }, triangles: vec![], objects: vec![] };
    assert_eq!(chunk.hash().unwrap(), sha256(&canonical(&chunk).unwrap()));
    for i in 0..8192 {
        chunk.triangles.push(Triangle { vertices: [[-i, i, 0], [i, 7, -9], [0, 0, i]],
            surface: Surface::Gravel, object_id: format!("道路\"\n{i}"), spawnable: i % 2 == 0 });
    }
    chunk.objects.push(GeneratedObject { id: "é\\\"".into(), asset_id: "builtin:tree".into(),
        position: [i64::MAX, -5, 0], quarter_turns: 3 });
    assert_eq!(chunk.hash().unwrap(), sha256(&canonical(&chunk).unwrap()));
}
