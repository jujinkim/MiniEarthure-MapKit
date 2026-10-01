# Playtest geometry verification — 2026-10-02

## Connected-road walls

Baseline MapKit 5023ced. The new regression reproduces an interior fragment on
`gentle90`, [237,0,253] → [249,0,283], with the original chord-rectangle algorithm
([baseline](t05-baseline.log)). Triangle clipping covers curved, rising/falling
and tapered interior walls while preserving outer/grade-separated walls.
Tests use synthetic public pieces only, optimized debug Rust on macOS.

- [Focused junction tests](t05-final.log): two tests.
- [Shared generated-wall faces](t05-package.log): no reversed coplanar duplicates.

An initially overbroad cargo command also tried to compile the pre-existing
`mapkit-core/tests/water.rs`, which lacks Triangle.contact_class and
Triangle.snow_retention_percent. No water source was changed. The relevant
core tests use `--lib`; no full-suite pass is claimed.

Native build and consumer load/contact results are recorded after the subsequent
road-surface change, sharing one final native build. Detailed junction driving
and platform acceptance remain user verification. All own formats stay v1.

## Curves and grades

[Baseline mesh](t06-baseline-mesh.log), [final mesh](t06-final-mesh.log): 15 affected
core tests pass, including the new grade-error/endpoint constraints, wall clips,
obstacle placement, grounding and source reconstruction. [Width/frame checks](t06-widths.log)
and [package roundtrip/tamper checks](t06-package.log) pass. The
[native MapKit/CLI build](t06-build.log) passes with the same production geometry.
The later change to the Rust test only tightens assertions; it changes no product algorithm.

Runtime fixture evidence belongs to the private consumer's geometry report;
no private implementation or user data is published here. General curve maximum
grade error improves 0.03032→0.00824; spiral error 0.10028→0.03645. Exact current
source fingerprints identify regenerated output. Existing packages are preserved.
