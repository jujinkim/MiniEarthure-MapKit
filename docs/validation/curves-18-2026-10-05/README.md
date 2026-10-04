# Curve simplification [18] — 2026-10-05

Baseline MapKit `6fac8980`; current source changes only ordinary analytic and
authored cubic sampling. Longitudinal spans 150→250cm, horizontal angle ×5/3
(4→6⅔°, spiral3.5→5⅚°), horizontal approximation error ×25/9
(.75→2.0833cm, spiral .6→1.6667cm). Height error remains .75/.6cm;
pitch and full normal change remain4/3.5°. Quarter probes, mandatory breaks,
one-time quantization, integer-height helix stations,23% inner/centre/outer
grade and width/radius/rise/ports remain. Loop and pipe sampling is unchanged.

Representative 6m ribbons (loop uses its supported4m width):

| Shape | Segments old→18 | Road triangles | Wall triangles | Wall volumes | Wall collision tetrahedra |
| --- | --- | --- | --- | --- | --- |
| Gentle90 | 36→22 | 288→176 | 584→360 | 72→44 | 432→264 |
| Hairpin | 46→29 | 368→232 | 744→472 | 92→58 | 552→348 |
| Curve up | 36→22 | 288→176 | 584→360 | 72→44 | 432→264 |
| Spiral90 left up | 30→20 | 328→288 | 488→328 | 60→40 | 360→240 |
| Spiral up/down (each) | 108→65 | 1048→740 | 1736→1048 | 216→130 | 1296→780 |
| Authored planar cubic | 45→27 | 360→216 | 728→440 | 90→54 | 540→324 |
| Loop4m | 212→212 | 368→368 | 3400→3400 | 424→424 | 2544→2544 |
| Cylinder curve6m | 29→29 | separate gimmick mesh | separate shell | separate shell | separate shell |

The seven ordinary fixtures reduce409→250 segments (38.88%). Spiral90 is33.33%,
limited by the retained height/normal bounds. Long helix road triangles fall29.39%
rather than39.81% because the unchanged≤1cm diagonal-ridge bound needs more
lateral strips per longer span. No denser road/wall objects or substitute ellipses.
Road rendering/collision/occupancy and walls all consume the final shared vertices.
Road counts include top/underside/sides; collision tetrahedra count walls only.

Checks: sampling limit/determinism1; assembled geometry/wall/junction/grounding/
checkpoint/preparation/cancellation43; authoring14 of15; panels2; assembled package
15 and published schemas1 passed. Native/CLI build passed. Closed/manifold walls,
connected ports, grade/quantization, widths, sample references, tamper rejection,
cancellation and budgets stay covered. Ordered geometry transcript was explicitly
renewed for the new sampler:100718 entries,
`a6ab88eb77963c0e15b4fc7190f966865dad60c4b5592138963befaab54c1085`.

The category combination test fails at mask2/sprint120s with72 required checkpoints
over the64 budget, identically with the original sampler. It remains a known
failure; the budget is not widened. Two package tests initially failed only with
18 because its changed candidate contains a1m bore gate: radius80cm violates the
existing100cm course minimum. Gate radius now takes `max(lateral+30,100)` while
keeping the exact sample. A narrow-pipe gate/reference/roundtrip regression and
both original package tests pass. Baseline comparisons and failed logs are retained.

Godot sedan3/15m/s, straight/left/right held-throttle probes distinguish original
geometry from18, before any Runtime fix. In the mid-helix15m/s straight probe,
peak normal speed is2.343→.748m/s uphill and.445→.354 downhill, with zero lost
supports; geometry simplification alone is not a general rough-ground fix.
Runtime owns the19 response comparison. No full-course, platform or user acceptance.

Reproduce with `rtk cargo test --locked -p mapkit-core --lib assembled_track`,
`--lib curve_sampling`, core `--test track_authoring --test panel_surfaces`,
package `--test assembled_track --test schema_contract`, and build
`-p mapkit-godot -p mapkit-cli`. Use the root Rust toolchain environment if needed.
Logs below preserve the exact scoped commands/results. Formats and public schemas
remain v1. Original packages and paused AI training are untouched.
