//! Read-only per-cell generation allowances for map authoring audits.
use std::path::Path;

fn main() {
    let path = std::env::args().nth(1).expect("usage: chunk_cost_report PACKAGE");
    let package = mapkit_package::read(Path::new(&path)).expect("valid package");
    let mut rows = Vec::new();
    for cell in package.document.cells() {
        let cost = package.document.estimate(cell, 500_000).expect("cell estimate");
        rows.push(serde_json::json!({"cell":cell,"estimate":cost}));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row["estimate"]["triangles"].as_u64().unwrap()));
    println!("{}", serde_json::json!({"package_sha256":package.inspection.package_sha256,
        "counts_are_estimates":true,"cells":rows}));
}
