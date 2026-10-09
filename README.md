# MiniEarthure MapKit

Independent public MIT map contracts, deterministic generation, package I/O and
shared Godot rendering. No private game dependency is required. All own formats
are current v1; original files are never converted automatically.

## Build and use

Rust stable and Cargo are required. CLI/core need no Godot installation.

```sh
cargo test --locked -p mapkit-package --test package_contract
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

Godot 4.7.2 integration: mount this repository at `addons/mapkit` or a nested addon path, build
`cargo build --locked -p mapkit-godot`, then import the consuming Godot project.
Native Windows uses its own Rust build; Android requires the Android Rust target
and toolchain. Host code must not load `godot/chunk_renderer.gd`.

The development profile uses `opt-level = 2`, including the library loaded by
Godot editor runs and debug exports. Debug symbols, assertions and overflow checks
remain enabled. This avoids measuring unoptimized geometry/codec loops as normal
editor performance. For instruction-level debugging, explicitly set
`CARGO_PROFILE_DEV_OPT_LEVEL=0`; record the profile in performance comparisons.
Release settings and generated/package identities are unchanged.

Crates: `mapkit-core` (pure), `mapkit-package` (filesystem/ZIP/PNG adapter),
`mapkit-cli` (composition), `mapkit-godot` (engine adapter). Shared renderer is
separate from collision registration, which belongs to the game.

See [format specification](spec/FORMAT.md), generated JSON Schemas in `spec/`, and
[test cases](crates/mapkit-package/tests/package_contract.rs). API exports use
explicit typed boundaries; JSON is restricted to adapters. No CI/CD is included.


## Contracts and ownership

- [Current domain](spec/CURRENT_V1.md), [container](spec/FORMAT.md),
  [regional source](spec/REGIONAL_SOURCE.md) and [errors](spec/ERRORS.md).
- [Assembled geometry](docs/ASSEMBLED_TRACKS.md) and
  [generation/track authoring](docs/TRACK_AUTHORING.md).
- [Prepared generation](docs/PREPARED_GENERATION.md),
  [renderer resource ownership](RENDER_MEMORY.md),
  [distant rendering](docs/DISTANT_RENDERING.md) and [world assets](spec/WORLD_ASSETS.md).
- [Road geometry](docs/STREET_GEOMETRY.md), [contact](docs/ROAD_CONTACT.md),
  [special driving](docs/SPECIAL_DRIVING.md), [water](docs/WATER.md),
  [environment](docs/ENVIRONMENT.md), [vegetation](VEGETATION.md).
- [Determinism](spec/DETERMINISM.md), [limitations](LIMITATIONS.md) and
  [third-party licenses](THIRD_PARTY.md).

`MapKitBridge` validates immutable packages and exposes cells, spawn options,
courses and bounded costs. Consumers own acquisition, leases, collision admission,
session authority, frame scheduling and memory budgets. Cost estimates are
conservative allocation inputs, not measured RSS/GPU limits.
`godot/chunk_renderer.gd` provides begin/advance/cancel and shared resource contexts;
headless consumers do not load it. Source fingerprints invalidate derived caches.

Tests use synthetic public fixtures. Run affected Cargo targets, schema/contract
checks and required native probes; no whole-suite or device acceptance is implied.
In the superproject use its `.venv` and development workflow. Standalone public
Python checks need the dependencies declared by their scripts, including jsonschema.
