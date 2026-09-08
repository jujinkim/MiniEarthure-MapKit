use mapkit_core::*;
use mapkit_package::*;
use std::path::Path;
#[test]
fn independent_assets_roundtrip_materials_costs_and_exact_cached_geometry() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets");
    let (d, files) = read_project(&path).unwrap();
    let bytes = pack_bytes(d.clone(), files.clone()).unwrap();
    let package = read_bytes(&bytes).unwrap();
    assert_eq!(bytes, pack_bytes(d, files).unwrap());
    assert!(package.asset_presentation_cost("tetra").unwrap() > 65536);
    assert!(package.asset_presentation_cost("checker").unwrap() > 65536);
    assert!(package.asset_presentation_cost("missing").is_err());
    let chunk = package.generate(Cell { x: 0, y: 0 }, 500_000).unwrap();
    let cost = estimate_generation(&package.document, chunk.cell, 500_000).unwrap();
    let key = archive_key(&package.inspection.world_content_hash, chunk.cell);
    let cached = encode_archive(&chunk, &key, archive_limit(&cost)).unwrap();
    assert_eq!(&cached[..8], b"MKCELL03");
    assert_eq!(
        decode_archive(&cached, &key, chunk.cell, &cost).unwrap(),
        chunk
    );
    let mut changed = package.document.clone();
    changed
        .assets
        .iter_mut()
        .find(|a| a.id == "tinted")
        .unwrap()
        .material
        .as_mut()
        .unwrap()
        .roughness_per_mille = 20;
    let rebuilt = read_bytes(&pack_bytes(changed, package.files.clone()).unwrap()).unwrap();
    assert_ne!(
        package.inspection.world_content_hash,
        rebuilt.inspection.world_content_hash
    );
    assert_eq!(chunk, rebuilt.generate(chunk.cell, 500_000).unwrap());
}
