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

## Modular return routing

The final [duration/determinism probe](t07-duration-final.log) passes at 3.15s:
seed 42 sprint 59.611s, circuit 60.582s for a 60s request, repeated outputs equal.
[Three layout units](t07-unit.log) cover module-only return plus short seam,
exact materialization, no mutation on budget failure/cancel, and graph composition.
The initial uncached full probe was stopped; the cached intermediate took 68.10s.
Pruning impossible time budgets, immutable template reuse, four-second closure
spacing and deterministic one-percent early acceptance reduced unnecessary work.

The first [package run](t07-package.log) passes nine cases including roundtrip,
tampering, cancellation, original preservation and source/progress modes. Its
wall test exposed two issues: plane-only comparison can reject disjoint faces,
and a generated 8m zigzag actually folds its inner ribbon by 2cm. The corrected
geometric overlap test still caught the real overlap ([diagnostic](t07-wall-piece.log)).
The wave-curvature bound fixes it. [Final wall tests](t07-wall-repaired.log) pass
with explicit disjoint/opposite-diagonal controls; [wave edge tests](t06-wave-check.log)
cover five presets × five widths. A zero-length quantized edge is allowed;
a backwards edge is not. The earlier strict-positive assertion incorrectly
classified a coincident pair as a fold; production geometry already omits zero-area faces.

Final wave-corrected source: [42 category/mode/duration combinations](t07-categories-final.log)
PASS (192.90s), seven nonempty categories × sprint/circuit × 60/90/120s at seed42.
Every estimate stays within ±10%; excluded random content remains excluded.
The repeated [duration probe](t07-duration-wave.log) is unchanged and PASS (3.16s).
The unchanged nine package checks retain the earlier passing evidence; affected
wall geometry was rerun after the wave change. Consumer native/load evidence is
collected with the following ramp/rail update to avoid duplicate dependency builds.
