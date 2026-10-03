# Continuous straight-road clearance — 2026-10-03

macOS arm64, Rust optimized dev/test profile. Starting MapKit `01b1ce5d`;
[source and binary digests](sources.json). All current formats remain v1.

The original [959cm lateral / 100cm longitudinal regression](before-test.log)
fails before the fix: two 8m roads have no clearance issue despite missing the
required separation by 1cm. The same named test passes after the fix. This closes
the limited straight-pair failure recorded in the [earlier report](../practice-refresh-2026-10-03/README.md),
whose failed evidence is preserved.

| Evidence | Result and scope |
| --- | --- |
| [Authoring](authoring.log) / [command](authoring.json) | 13 pass: original failure, phase/order matrix (nine offsets, three widths, three boundary distances, both orders), seven yaw angles/two origins, crossing/height boundaries, ends, 8m/6m corners, shared ports and existing authoring safety |
| [Straight unit](straight-units.log) / [command](straight-units.json) | 1 pass: eleven unsupported geometry cases retain sampled collisions in both eligibility orders; actual delivered ribbon bounds remain authoritative |
| [Authoring units](units-authoring.log) / [command](units-authoring.json) | 5 pass: cached/uncached compilation, tampering, cancellation, manual flight, static structures, budgets and checkpoint order |
| [Package](package.log) / [command](package.json) | 6 pass: new clearance issue blocks both execution containers with E_TRACK_DRAFT; source roundtrip/tampering, manual structures, cancellation, failed/cancelled save destination preservation |
| [Native/CLI build](mapkit-build.log) / [command](mapkit-build.json) | Pass with locked dependencies |

Total: 25 affected Rust tests. No duration/category matrix, full suite, recursive
clone, export matrix or detailed application/driving acceptance. An initial
shell invocation could not find Cargo; the installed Homebrew rustup toolchain
resolved it. One over-filtered discovery command ran zero tests and is not counted;
the explicit authoring and straight-unit commands above executed the intended six.

Only level, constant-width straight drive ribbons without shared ports bypass
the discrete sample gate. SAT encloses the existing edges and 50cm wall + 30cm
vehicle margins on sides/ends; horizontal touching overlaps. The existing open
vertical volume interval is unchanged (250cm level separation is clear). Shared
ports and portal drops keep their previous sampled exclusions; unsupported shapes
keep the previous route. Curved/slope/taper/tube continuous detection remains
outside this change. No API/schema/version increment or clearance reduction.
