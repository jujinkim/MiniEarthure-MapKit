# Parametric special driving structures

The current contract adds `target_speed`, `jump_height` and `air_ring` motion
kinds with an `effect` object. `strength_percent` is 1–100 (default 100),
`jump_height_cm` is 10–1000 (default 300), and `ring_radius_cm` is 75–600
(default 180). Height describes a still-air, horizontal launch; a physics
consumer must account for its gravity and body drag. The forward axis is local
+Z in map coordinates, and launch normal is local +Y. Existing `boost`/`launch`
impulses retain their meaning. Activation, aperture crossing, cooldown and
vehicle speed remain the consumer's responsibility.

A static definition can instead contain `track` with `kind: loop|cylinder`,
`radius_cm: 150..600`, `width_cm: 140..600`, `length_cm: 600..3200`. Tracks have
empty `parts` and identity scale. Radius controls both shapes, width controls
the loop ribbon, and length controls the cylinder axis. Other dimensions are
retained for switching templates. Defaults are radius 250, width 220, length
1600 cm. Position and pitch/yaw/roll still apply. Loop ends are separated;
its floor radius is 1.6 times the radius parameter and crown radius 0.4 times.
Cylinders have a flared entrance and exit, a hollow interior, and no end caps.

`special_track.rs` produces the only geometry used for presentation, collision
and per-tile occupied bounds. Inner faces and the outer shell/sides are distinct;
only inner faces may provide curved wheel support. Neither is a spawn/recovery
surface. Spawn APIs also exclude underlying ordinary surfaces inside the track.
Material and curved-driving role are independent.

Faces use deterministic integer tenths of a millimetre, explicitly marked
`units_per_metre: 10000`; authored coordinates and occupied bounds remain cm.
Fixed libm sampling, 256 angular sections and 16/32/64 adaptive cylinder bands
keep adjacent normals within 5 degrees and analytic approximation error below
1 cm throughout the supported parameter range. Source/archive records contain
parameters, not expanded faces. Godot `resolve_gimmick` and packed output expand
`track_mesh` plus `memory_bytes` for consumers. This is a bridge view, not an
alternate file format or parser fallback.

Cost is 16 KiB per definition + 32 KiB per ordinary convex part + 4 KiB per
track tile. Spatial selection, regional dependencies, occupied bounds and the
pre-entry driving window include the authored safety rectangle. No ordinary
32-part, memory or cell limit is raised; oversized placements are rejected.
The 2026-09-26 arcade decision sets every own format, including `.memap`, to 1. Build fingerprints
invalidate generated caches after these changes.

Focused validation (2026-09-26): special-track shape/range/cost/archive/order,
5-degree/1-cm bounds, spawn exclusions, occupancy, spatial/chunk hashes,
package/region/schema contracts and Godot native build. The consumer separately
verifies physical driving and effects. Detailed application acceptance is not
claimed here.

## Pipe material (03, 2026-10-04 replacement)

New assembled cylinders and the standalone straight-cylinder template use
`#596168`. Shared `gimmick_geometry.pipe_material` sets roughness 0.82, metallic
0.65 and emission off in both production display and prepared Editor previews.
Explicit stored colours are authoritative; loading/rendering never recolours a
saved object. Loops, halfpipes and panels retain their existing materials.
Geometry, collision and contact/friction properties are unchanged. Pipe selection
highlights the private preview material and restores its stored colour on deselect.

Scoped macOS arm64 / Godot 4.7.2 checks passed: native/CLI build, five special-track
Rust tests and `pipe_material_validator` with strict diagnostics. The validator
compares production/preview vertices and material properties, custom saved colour,
selection/restoration, resource reuse and weak-reference release after node disposal.
Fixed-camera/light synthetic comparison: previous colour/material
and matte metal. The prior material
is recreated on the same geometry for comparison. Detailed art preference and
real driving remain user verification. Development-only validator errors (panel
shader treated as StandardMaterial, JSON numeric array equality) were corrected;
final validation is clean. Execution logs stay in root local `docs/tasks/pipes-03-08/`.

All formats stay v1. The assembly source fingerprint changes with its generated
default colour; active generated packages are refreshed with the 08 delivery
while preserving original artifacts.

## Air ring defaults 09 — 2026-10-04 replacement

New automatic track rings, manual air-ring actions and the standalone template
use a 150cm inner radius (3m opening) and 100% target speed. The square track
rim retains 50cm width / 30cm depth; the standalone 24-sector round rim retains
20cm width / depth. Placement, rotation, colour and 1500ms cooldown are unchanged.
The existing declared vertices drive display, authoring preview and collision.
Explicit saved dimensions and strengths are preserved; no conversion, new fields
or format change is introduced (all own formats remain v1).

Scoped Rust/native results pass seven core cases
and one package case: automatic/manual geometry and pose, standalone quantization,
effect limits, costs/occupancy, deterministic compilation, source/package roundtrip,
explicit 6m/37% preservation and derived-radius tamper rejection. CLI and Godot
extension builds pass on macOS arm64. Consumer trigger/preview and standalone
startup are recorded by their owning repositories. Detailed driving is user verification.
