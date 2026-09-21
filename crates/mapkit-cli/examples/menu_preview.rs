//! Offline selection hints from a validated immutable package; never spawn authority.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: menu_preview INPUT.memap OUTPUT.json".into());
    }
    let package = mapkit_package::read(Path::new(&args[1]))?;
    let mut overview = mapkit_core::overview(&package.document)?;
    overview.buildings.clear();
    let data = serde_json::json!({"version":1, "package_sha256":package.inspection.package_sha256, "overview":overview});
    std::fs::write(&args[2], serde_json::to_vec(&data)?)?;
    Ok(())
}
