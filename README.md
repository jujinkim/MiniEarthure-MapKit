# MiniEarthure MapKit

Independent public MIT map contracts, deterministic generation, package I/O and
shared Godot rendering. No private game dependency. All own formats are v1.
Documentation index: [docs/README.md](docs/README.md).

## Build and use

Rust stable and Cargo are required; CLI/core need no Godot installation.

```sh
cargo test --locked -p mapkit-package --test package_contract
cargo build --locked -p mapkit-cli
./target/debug/mapkit pack examples/minimal /tmp/example.memap
./target/debug/mapkit inspect /tmp/example.memap
./target/debug/mapkit validate /tmp/example.memap
./target/debug/mapkit unpack /tmp/example.memap /tmp/editable-map
./target/debug/mapkit generate-chunk /tmp/example.memap 0 0 /tmp/chunk.json
```

Choose unused output paths; tools never replace existing outputs. Edit the
extracted document, then pack it to another destination. `examples/minimal` is an
original synthetic fixture with crossing ground/bridge surfaces and an orchard.

Godot 4.7.2: mount this repository at `addons/mapkit` (or a nested addon path),
run `cargo build --locked -p mapkit-godot`, then import the consuming project.
Native Windows uses its own Rust build; Android needs the Android Rust target and
toolchain. Headless/host code must not load `godot/chunk_renderer.gd`.

The dev profile uses `opt-level = 2` (debug symbols, assertions and overflow checks
stay on). Set `CARGO_PROFILE_DEV_OPT_LEVEL=0` for instruction-level debugging and
record the profile in performance comparisons.

## Crates and boundaries

- `mapkit-core` (pure), `mapkit-package` (filesystem/ZIP/PNG adapter),
  `mapkit-cli`, `mapkit-godot` (engine adapter). APIs use typed boundaries; JSON
  stays in adapters. Generated JSON Schemas live in `spec/`.
- `MapKitBridge` validates immutable packages and exposes cells, spawn options,
  courses and conservative costs. Consumers own acquisition, leases, collision
  admission, session authority, scheduling and memory budgets.
- Cost estimates are allocation inputs, not measured RSS/GPU limits.

Tests use synthetic public fixtures; run the affected Cargo targets and
schema/contract checks. In the superproject use its `.venv`; standalone Python
checks need their declared dependencies, including jsonschema. No CI/CD is included.
