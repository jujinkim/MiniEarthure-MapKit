//! Synthetic flat cells for cache-index workload; no user datasets.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: cache_population SYNTHETIC.memap NEW_DIRECTORY".into());
    }
    let package = mapkit_package::read(Path::new(&args[1]))?;
    let destination = Path::new(&args[2]);
    std::fs::create_dir_all(destination)?;
    for (index, cell) in package.document.cells().into_iter().take(2000).enumerate() {
        let chunk = package.generate(cell, 500_000)?;
        let key = mapkit_core::archive_key(&package.inspection.world_content_hash, cell);
        let cost = package.document.estimate(cell, 500_000)?;
        let bytes = mapkit_core::encode_archive(&chunk, &key, mapkit_core::archive_limit(&cost))?;
        let dir = destination.join(&key);
        std::fs::create_dir(&dir)?;
        std::fs::write(dir.join("cell.bin"), &bytes)?;
        std::fs::write(
            dir.join("info.json"),
            serde_json::to_vec(
                &serde_json::json!({"cell_key":key,"cell_bytes":bytes.len(),"access":index+1}),
            )?,
        )?;
    }
    Ok(())
}
