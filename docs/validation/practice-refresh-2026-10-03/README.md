# Finite straight-road clearance — 2026-10-03

macOS arm64, local Rust dev profile; starting MapKit `0232724e`. Exact source
digests and generator identity are in [sources.json](sources.json). Each final
command, elapsed seconds and exit code is retained beside its log. All final
commands passed; this is scoped authoring/package verification, not a full suite.

| Evidence | Scope |
| --- | --- |
| [Clearance](clearance-final.log) | 2 tests: 8m/6m left/right corners at seven yaw angles with translated origins; crossings, overlap, side/end margins, touching, shared port and overpass height |
| [Fallback](fallback-final.log) | 1 unit: tilted/curved/tapered/graded/tube/flight/alternate paths cannot use the separation shortcut; actual ribbon edges enlarge bounds |
| [Authoring](authoring-final.log) | 8 tests: draft/tamper, branch/shared progress, widths/curves/helices/jump, oriented pipe ports, vertical/degenerate curves, tilted clearance, declared flight and invalid action/landing indices |
| [Authoring units](authoring-units.log) | 5 preparation/manual-flight/checkpoint units, including source validation and budgets |
| [Authored package](package-authored.log) | Source roundtrip, draft export and tampering |
| [Manual flight](package-manual.log) | Manual flight/static structures in both containers |
| [Cancellation](package-cancel.log) | Cancelled and invalid package requests |
| [Build](mapkit-build.log) | Current CLI and Godot native bridge |

The [first clearance run](clearance-first-toolchain.log) passed the corner test
but failed a new near-tangent parallel fixture. Its 959cm lateral / 100cm
longitudinal offset phase-shifted the existing discrete sample-volume checks;
that broad phase already misses this 1cm margin case. The final margin fixture
aligns stations to test that the new finite-footprint check does not discard an
existing collision. The sampled broad phase was deliberately retained, as were
shared-port exclusions. This pre-existing sample-phase limitation remains;
the new SAT is only a proof of separation, not a replacement continuous collision
detector. No passing result is claimed for the original offset fixture.

No format increment, schema/public API addition, safety margin reduction or
course-specific exception. Detailed driving and device acceptance are user work.

Tracked logs omit trailing whitespace/blank EOF lines; raw local logs are retained.
