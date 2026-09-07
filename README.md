# MiniEarthure MapKit

Independent MIT map document, deterministic generation and `.memap` package tools.
No private MiniEarthure repository is required. Development foundation; current
scope and outstanding gameplay requirements are explicit in [LIMITATIONS.md](LIMITATIONS.md).

## Build and use

Rust stable and Cargo are required. CLI/core need no Godot installation.

```sh
cargo test --locked
cargo build --locked -p mapkit-cli
./target/debug/mapkit pack examples/minimal /tmp/example.memap
./target/debug/mapkit inspect /tmp/example.memap
./target/debug/mapkit validate /tmp/example.memap
./target/debug/mapkit unpack /tmp/example.memap /tmp/editable-map
./target/debug/mapkit generate-chunk /tmp/example.memap 0 0 /tmp/chunk.json
```

Choose unused output paths; tools preserve existing outputs. Edit the extracted
document, then pack the directory to another destination. `examples/minimal` is
an original synthetic fixture from an unknown third-party producer, including
crossing ground/bridge surfaces and an orchard. No external data or game assets.

Godot 4.7.1 integration: mount this repository at `addons/mapkit`, build
`cargo build --locked -p mapkit-godot`, then import the consuming Godot project.
Native Windows uses its own Rust build; Android requires the Android Rust target
and toolchain. Host code must not load `godot/chunk_renderer.gd`.

Crates: `mapkit-core` (pure), `mapkit-package` (filesystem/ZIP/PNG adapter),
`mapkit-cli` (composition), `mapkit-godot` (engine adapter). Shared renderer is
separate from collision registration, which belongs to the game.

See [format specification](spec/FORMAT.md), generated JSON Schemas in `spec/`, and
[test cases](crates/mapkit-package/tests/package_contract.rs). API exports use
explicit typed boundaries; JSON is restricted to adapters. No CI/CD is included.
