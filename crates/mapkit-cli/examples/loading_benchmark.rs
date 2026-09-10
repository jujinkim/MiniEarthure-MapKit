//! Read-only same-input native timing; executable output, not a core clock dependency.
use mapkit_core::Cell;
use std::{path::Path, time::Instant};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let opened = Instant::now();
    let package = mapkit_package::read(Path::new(&args[1])).unwrap();
    if args.get(2).is_some_and(|s| s == "--vegetation") {
        let mut objects = Vec::new();
        for cell in package.document.cells() {
            objects.extend(
                package
                    .generate(cell, 500_000)
                    .unwrap()
                    .objects
                    .into_iter()
                    .filter(|o| o.asset_id == "builtin:tree"),
            );
        }
        objects.sort_by(|a, b| a.id.cmp(&b.id));
        println!(
            "{}",
            serde_json::json!({"package_sha256":package.inspection.package_sha256,"objects":objects})
        );
        return;
    }
    println!(
        "{}",
        serde_json::json!({"stage":"open", "ms":opened.elapsed().as_secs_f64()*1000.0,
        "package_sha256":package.inspection.package_sha256})
    );
    for cell in [
        Cell { x: 1, y: 4 },
        Cell { x: 8, y: 3 },
        Cell { x: 10, y: 10 },
    ] {
        let started = Instant::now();
        let cost = package.document.estimate(cell, 500_000).unwrap();
        let estimate_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let chunk = package.generate(cell, 500_000).unwrap();
        println!(
            "{}",
            serde_json::json!({"cell":cell,"estimate_ms":estimate_ms,
            "generate_ms":started.elapsed().as_secs_f64()*1000.0,"triangles":chunk.triangles.len(),
            "objects":chunk.objects.len(),"estimated_triangles":cost.triangles,"hash":chunk.hash().unwrap()})
        );
    }
}
