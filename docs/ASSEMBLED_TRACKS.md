# Assembled track geometry and package validation

`mapkit_core::assembled_track` owns the catalogue, dimensions, seeded selection,
connections, corridors and collision geometry. `mapkit_package::assembled_track`
adds a matching public course and package save/verification. Consumers do not
reimplement the generator. All formats remain v1.

Generated track faces and authoring previews convert the tessellator's outward
mathematical winding to the same package winding as ordinary roads and terrain.
The shared Godot reflection/index adapter then produces outward clockwise faces.
Road tops, slab undersides, walls, supports and plaza shells use this one
publication boundary; analytic frames, occupied solids, vertex positions and
shared seam edges are unchanged. The build fingerprint invalidates generated
caches; source maps and assembly layout fingerprints remain unchanged.

`track_winding` checks top/bottom orientation, shared join vertices and closed
walls. Runtime's seam tests check actual generated geometry against a same-material
plane; geometry audits alone do not establish a Windows driving fix.

The compact `overpass` lower road uses analytic quarter-circle tangents and lane
edges, transformed before centimetre quantization. The ribbon is shared by road, walls, collision and occupancy. Its
263 lower-road stations, 65 shortcut stations, action/checkpoint indices, ports,
width and shortcut height are preserved. Empty-control `free_curve` and
`flight_curve` defaults also keep their straight exit tangent instead of
inheriting an ordinary 90-degree curve endpoint.

All seven `track_winding` tests pass on macOS: catalogue road orientation across
972 preset/width/rotation conditions, lower-road edge progression and triangle
overlap, preserved stations/ports and free/flight-curve endpoint frames. Package
roundtrip checks preserve the generated ribbons and course references. Runtime
checks the actual lower road with both cars at 60/120Hz; results belong to its
vehicle physics document. This fix applies to newly generated or recompiled
geometry. Loading or sharing a saved `.memap` preserves its stored interior
geometry. Export alone does not regenerate it, and normal export can require
explicit source recompilation: regenerate/recompile first, then export to a new
file to receive the correction. Original packages are never rewritten on load.

The opt-in `catalogue_geometry_audit_includes_tapers_editor_curves_and_special_surfaces`
test extends this to 2,694 catalogue/width/placement/port-taper/default-cubic
conditions and 26 unique special-track contact/shell meshes. All 1,334,310 faces
pass winding, nondegeneracy, ordinary ribbon progression/coplanar overlap and
special closed-shell checks. A 12 m descending left 360-degree spiral at rotation
`[27000,17000,11000]` remains rejected by the existing clearance validator;
passing the face audit does not make that placement executable. Run the test
explicitly with `cargo test -p mapkit-core --test track_winding catalogue_geometry_audit_includes_tapers_editor_curves_and_special_surfaces -- --ignored`.
Arbitrary user-authored control points and continuous placement parameters are
outside this finite catalogue audit.

The saved-package audit covered 76 tracked Client/Editor packages (57 unique
hashes): nine contained assemblies, with 321 piece paths and 9,603 ordinary road
faces. No reversed/degenerate faces, nonprogressing ribbon edges or coplanar face
overlaps were found; special/flight-only intervals were excluded from this
ordinary-surface check. None contained `overpass` or an empty-control free/flight
curve. Existing distributed maps remain unchanged; newly generated maps receive
the generator correction without converting saved packages.

Snapped pieces rebuild their first/last ribbon from the final integer centre and
frame, avoiding a second rounding of already rounded local edges. Geometry
preparation also canonicalizes saved endpoint ribbons, so loading an existing v1
source benefits without rewriting it. Road, walls, collision and occupancy share
these sections. The regression covers 420 rotated/pitched/rolled joins, including
saved endpoint perturbations and published shared edges.

Loop inner and shell endpoints match the ordinary centimetre ports exactly.
Quintic entrance/exit feet match position, slope and curvature to the loop, with
zero grade and curvature at the flat road. Shell thickness follows that same
normal; closed end caps remain present. The acceleration panel is on the final
straight approach, after the lateral entry transition. There is no extra overlap
or added runway. The upper arc distributes its longitudinal advance with radius
`r*(1.1 + 0.6*cos(t) + 0.3*cos(2*t))`: the minimum is 0.65r and the crown is
0.8r. Height remains 2r and the external ports
remain identical. Eight special-track tests verify ports, manifold orientation,
quantized upper curvature,
occupancy and bounded normals; all ten wall units pass. Ordinary curve wall
reductions remain 40–44% (the short vertical spiral remains the 29% exception).
The loop has 3,208 wall triangles; this is a geometry count, not a runtime-cost measurement.

## Generation and catalogue

[TRACK_AUTHORING](TRACK_AUTHORING.md) owns candidate categories, current dimensions,
60/90/120-second reference estimates, bounded search, authored graphs and APIs.
Categories have no mandatory family count. Current pipe bores are generated at
2/3/4 m and authored at 2/3/4/6 m. Generator settings and source/catalogue
fingerprints identify regeneration; loading a sound saved package does not
require rewriting it. Reference-speed time is not a measured player lap time.

Ordinary boundaries are closed 50 cm outward volumes. Shared section caps cancel
across seams; rendering, collision and occupancy share quantized geometry. Wall
tops are support but never spawn/recovery candidates. Connected-road clipping
uses actual road triangles with a 2 cm inset and 5 cm height tolerance, retaining
outside and grade-separated walls. RC venue decoration has no driving collision.
Ramp attachments are 160 cm wide; rail tops are 40 cm with an explicit independent
grind line. Placement requires supported, bounded clearance, never a checkpoint shortcut.

## Grounded supports

New seed assemblies ground the venue at the minimum of the actual road slab,
finish plaza, branch geometry and transformed special-track inner/shell mesh.
Vehicle-clearance envelopes, flight trajectories and decorations do not lower it.
Track positions, ports and start/finish are preserved. Ordinary ribbons/plazas
now expose their 10cm underside and side faces. Twisted ribbons use the same
subdivision on both sides. Integer centimetre bounds round outward; the
special mesh retains its existing 1/100cm vertex quantization.

`Assembly.supports` records the owning piece index and one explicit convex
20×20cm vertical column for each piece with an airborne underside. Its flat cap
uses the minimum clipped underside over the entire square, preventing curved
or sloping decks from being pierced. Deterministic centre-first longitudinal
and lateral candidates include rising/falling pieces and alternate branches;
pure flight has no support. The floor and support geometry are recomputed during
source validation and included in canonical content hashes.

Columns may intersect lower roads if the union of nearby columns and obstacle
motion sweeps leaves a continuous 110cm lane. Starting runways, bores, required
flight, launch and landing space are protected. No valid placement yields
`E_TRACK_SUPPORT`; the seed generator rejects that candidate within its existing
24 attempts and reports the reason if none succeeds. Consumers retain the live
map after a failed/cancelled generation.

Display triangles, collision triangles and occupancy consume the same column
convex under `assembled-support-<piece_index>`. Columns and
`assembled-shell-<piece_index>` are non-spawnable structures, never venue floors
or recovery roads. Cell triangle/solid costs, retained convex capacities and
pre-validation mesh workspace are accounted. Grounding retains at most one
300,000-face piece mesh; both package readers reserve its bounded workspace.

`Source.grounded_supports` is false for independent manual sources. Conversion
from a seed preserves true with its original settings, so edits, preview,
project/package saves and reopening deterministically rebuild the policy.
External imports are unchanged. Current own formats remain v1, with the updated
required schema and source/catalogue/build fingerprints; no original files are
converted or overwritten.

Focused verification: six grounding unit tests cover actual slab/shell extrema,
flat/graded/high endpoints, helices, pipes, loops, branches and plaza, 20cm caps,
partial intrusion/110cm boundaries, combined obstructions, rejected placement,
seed conversion and cell-seam collision/occupancy/cost identity. Existing core,
special-track, authoring/category, package roundtrip/tampering/cancellation,
retained-memory and source-budget regressions passed. Schema regeneration and
Draft 7 validation passed without a format increment. Native Godot binding,
shared venue rendering and consumer contact/authoring checks passed. Detailed
application driving/editing and platform acceptance remain user verification.

## Saved-package verification

`MapDocument::validate` checks bounded saved assembly coordinates, frames, paths,
connections, support convexes and references without recompilation. Package reads
also validate payload hashes, manifest, world identity and resource limits.
`verify_track` binds the candidate to the saved course; changing a generator or
catalogue fingerprint does not invalidate a sound package. Fingerprints identify
compiled caches. `pack_bytes`, `pack_source` and authoring `verify_document` retain
current-source equality; load never edits a file.

macOS Rust checks: 22 package contract tests include stale-fingerprint/interior
geometry acceptance, corruption and invalid reference rejection. The 15 assembly
package cases and compiler reuse tests cover export equality and cancellation.

## Analytic surface sampling

Ordinary curves now sample the unrounded analytic centre and ribbon, quantizing
only final centimetre vertices. Adaptive intervals bound tangent/frame changes
to 4 degrees, surface approximation to 1cm and longitudinal spacing to 1.5m.
Quarter-point probes detect inflections; planar trapezoids need one strip, while
non-planar ribbons add strips from their actual surface error. Rendering, barriers,
collision and occupancy consume the same final edges. The former minimum sample
counts and post-quantization curve refinement are removed.

Spirals preserve width and rise, using radius max(8m, half-width + 7m). Their
final inside/centre/outside edges are validated against 23% grade. Ordinary
ramps ease in/out over the first/last eighth with constant middle grade. Loops
and pipes use adaptive longitudinal and angular sampling while retaining their
high-precision surfaces and vertical sections, without the ordinary grade cap.
Costs, bounds and placement use the resulting geometry. Shared seams and
internal-face cancellation remain active. All formats remain v1.

Scoped automated results: assembled geometry units (all widths, grades, frames,
spacing, deterministic edges), five wall units (closed seams and thickness),
five special-track units, eight authoring units excluding the unrelated category
combination matrix, all twelve assembled-package units and native/CLI build pass.
Vertical endpoint and loop-crown cases are included. Detailed driving is a user check.

## Difficulty-weighted routes

Difficulty now selects a driving family before a uniform left/right/up/down
variant. This replaces the uniform 26-ID driving pool; enabled category tickets
retain 26:9:5 for driving/gimmick/action. Family weights (easy/normal/hard) are:
straight 34/16/8, grade 12/10/6, gentle corner 34/24/14, right angle 16/18/18,
zigzag 4/8/12, sharp135 0/8/12, hairpin 0/6/12, spiral90 0/6/8,
spiral180 0/3/6, spiral360 0/1/4. Ordinary width weights for 2/4/6/8/12m are
0/20/40/30/10, 10/30/30/20/10 and 30/40/20/8/2. Supported widths only; required
connections and fixed gimmick geometry retain their dimensions. Obstacle spacing
remains 64/32/16m. Existing files, API default seed1, v1 formats, cancellation,
24 candidates and all search/geometry budgets remain unchanged. Source fingerprints
identify new generated layouts. No weather field is added to generation settings.

Focused verification: five layout units
pass, including distribution/uniform variants, seed17 repeatability in both modes,
category exclusion and bounded geometry. Aggregate route-family complexity is
70/139/175 for easy/normal/hard. All 42 category/mode/duration cases, package
roundtrip/tamper rejection, cancellation/invalid requests and native/CLI build pass.
Detailed driving remains user verification.

## Quantized wall occupancy

Walls emit deterministic
outward tetrahedra. Canonical quad diagonals handle concave corners, shared section
diagonals remain stable, and exact i128 determinants remove only zero-volume
parts. Repeated quantized vertices and duplicate tetrahedra are deduplicated.
Nonzero wedges at folded centimetre sections remain occupied. The displayed and
colliding triangles come from these same parts, with opposing shared faces
cancelled across parts and piece seams. Width, height, clipping and spawn policy
are unchanged; no consumer validation is relaxed.

Cost estimation traces the common clipping plan and counts the resulting parts
and local boundary faces, including split sections. It also reserves the complete
wall-vector/face-map and clipping workspace, even for geometry outside the output
cell. Existing triangle, occupancy and consumer memory limits remain unchanged;
exceeding them fails explicitly. All formats and public APIs stay v1. Source-based
fingerprints identify new generation; existing input files are not converted.

Scoped results:
33 assembled units, six occupancy tests and twelve assembled-package tests pass.
After the final cost refinement, seven wall tests and the cell-seam cost test pass.
Fixtures cover duplicate vertices, a concave plaza corner, nonzero folded wedges,
exactly flat sections, straight/curve/grade/spiral/loop/taper/plaza exterior parity,
closed representative meshes, wall thickness, cap cancellation and explicit budget
failure. Seed 7, 60-second one-way start cell (0,0) contains 6,802 solids, including
6,195 convexes; every convex passes strict validation. Branch composition and the
existing difficulty distribution are retained. Native bridge and CLI builds pass.
Detailed application driving and platform acceptance remain user checks.

## Continuous unjoined straight-road clearance

For level, constant-width straight drive ribbons without a shared port, the
finite footprint now decides horizontal overlap before any discrete sample gate.
This replaces the separation-only rule above for that limited pair class. Bounds
still enclose every actual ribbon edge plus the same 50cm wall and 30cm vehicle
margin at sides and ends. Horizontal boundary contact counts as overlap; vertical
clearance uses the unchanged open sample-volume interval. The result no longer
depends on longitudinal sample phase, including the 959cm/100cm missed case.

Shared ports (including the existing portal-drop recognition) retain the sampled
exclusions and straight separation correction. Curves, grades, banks, tapers,
tubes, flights and alternate paths retain the existing sampled path. No public
API, schema, own format, margin or connection exception changes. Invalid placements
remain draft issues and both execution containers reject them with E_TRACK_DRAFT.
Twenty-five affected tests and native/CLI build pass.
Detailed consumer acceptance is separate.

Course sealing now retains stable non-overlapping checkpoints only. Stored package source is preserved; the public course view derives a new course identity only if overlap removal changes its geometry. Fewer than two effective gates disables racing without rejecting the map. Sphere/hemisphere overlap and continuous chassis-capsule entry tests cover grazing, stationary/initial-inside rejection and separated half volumes.

## RC direction marking

The common stage adds up to64 flat-road chevrons in one display-only mesh at
24m path intervals, using the stored tangent/normal. Steep/special sections are
excluded. The marks have no collision, occupancy, spawn role or shadow.
The scoped rc_venue validator passes bounded nodes, common material consumption
and collision-free display on all three venue themes. Native generation is unchanged.
