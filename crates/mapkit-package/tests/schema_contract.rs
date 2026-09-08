use mapkit_core::MapDocument;
use mapkit_package::PackageManifest;

#[test]
fn published_schemas_match_the_cli_types() {
    for (generated, published) in [
        (
            serde_json::to_string_pretty(&schemars::schema_for!(MapDocument)).unwrap(),
            include_str!("../../../spec/document.schema.json"),
        ),
        (
            serde_json::to_string_pretty(&schemars::schema_for!(PackageManifest)).unwrap(),
            include_str!("../../../spec/manifest.schema.json"),
        ),
    ] {
        assert_eq!(
            generated,
            published,
            "regenerate the published schema with mapkit schema"
        );
    }
}
