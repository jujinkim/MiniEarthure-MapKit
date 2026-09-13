use mapkit_package::*;
use std::path::Path;

#[test]
fn distant_display_is_bounded_deterministic_and_preserves_generation() {
    for name in ["minimal", "roads", "courtyard", "assets"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name);
        let (document, files) = read_project(&path).unwrap();
        let package = read_bytes(&pack_bytes(document, files).unwrap()).unwrap();
        for cell in package.document.cells() {
            let original = package.generate(cell, 500_000).unwrap();
            let hash = original.hash().unwrap();
            let view = package.distant_from_generated(&original).unwrap();
            let generated = package.generate_distant(cell).unwrap();
            assert_eq!(view.vertices, generated.vertices);
            assert_eq!(view.colors, generated.colors);
            assert_eq!(view.vertices.len(), view.colors.len());
            assert!(
                view.vertices.len() as u64 <= package.distant_triangle_bound(cell).unwrap() * 3
            );
            assert!(view.vertices.iter().flatten().all(|v| v.is_finite()));
            assert_eq!(
                hash,
                package.generate(cell, 500_000).unwrap().hash().unwrap()
            );
            // Assets use independent visual proxies. Pure road/terrain surfaces
            // retain the exact reflected integer-centimetre triangle boundaries.
            if original.objects.is_empty() && package.document.assets.is_empty() {
                assert_eq!(view.vertices.len(), original.triangles.len() * 3);
            }
        }
    }
}
