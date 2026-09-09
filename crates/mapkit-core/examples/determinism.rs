//! Print portable vectors; no OS/build metadata enters the comparable output.
#[path = "../tests/support/determinism.rs"]
mod audit;
fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&audit::vectors()).unwrap()
    );
}
