# Seeded modular tracks (current v1)

Shared spiral surface correction, 2026-10-05
keeps longitudinal density and uses common sections, final-vertex diagonals and
bounded local transitions. It supersedes independent per-span strip rounding.

Current behavior, including the 2026-10-04 pipe material/bore replacement (03/08),
is defined by [category generation and free authoring](TRACK_AUTHORING.md)
(2026-09-30 replacement). The dated sections below preserve implementation/test
history; mandatory selections, old dimensions, lattice placement and duration
overrun rules are superseded.

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

The reversed track fronts previously relied on backface collision for support:
Jolt/CCD could shorten actual travel while reported velocity stayed normal and
excite vertical rocking. `track_winding` checks top/bottom orientation, exact
shared join vertices and closed wall orientation. Runtime's `vehicle_seam_validator`
uses actual generated straight/practice joins with both directions, RC sedan and
monster truck against an identical-material plane. On macOS M1/Godot 4.7.2/Jolt,
the new native build passes all 72 cases at each of 60/120 Hz. The two Rust
`track_winding` tests pass. Results and platform limits belong to Runtime's vehicle
physics document; this does not establish a Windows driving fix.

Settings contain an exactly representable nonnegative JSON integer `seed`,
`circuit`, `duration_seconds`, `difficulty` (easy/normal/hard), candidate `gimmicks`
and `time_minutes` (0–1439). Defaults are circuit, 60 seconds, normal and noon.
Circuit choices are 30/60/120 seconds per lap, with maximum 3/3/2 laps; sprint
choices are 60/120/180 seconds and one lap. The catalogue owns these options.
`track_json.max_laps` exposes the authoritative limit to Runtime and Client.
Basic straight/slope/zigzag/left- and right-curve connectors remain available independently of
the optional special pieces. A selected special is a candidate, not a promise
that it appears, except the cylinder family; helices have a complementary return connector.

The base cube is 800 cm; ordinary roads mix 400 cm passing lanes and 200 cm pinch sections with 60 cm walls.
Left/right 90-degree corners have a circular 300 cm centreline radius between
5 m straight approaches/exits, in an 8 m port span. Hairpins have a 400 cm radius
and 800 cm separation between return lanes. A 16 m chicane joins four 3 m-radius
corners, and a reserved 48 m bay adds two narrow returns and continuous corners.
Compact obstacle/panel pieces use a gentle lateral displacement so adjoining
straight portions cannot create a disguised long straight. Obstacle envelopes
leave room for small vehicles. Checkpoint radii are half road width plus 30 cm.

The cylinder selection is one `selection_groups.cylinder` family: right/left
90-degree, right/left U-turn, S and (hard difficulty) a gently rising/falling S.
The bore diameter alternates between 200 and 400 cm across selected tubes, with a cosine mouth flare of at most 12 percent.
Centreline bends have a 400 cm radius. A `swept_cylinder` special track stores
bounded floor/normal/tangent frames; those same frames define its circular
mesh, hollow occupancy, spawn exclusion and full-section admission corridor.
The inner surface has 128 angular divisions. Entrances/exits connect to full
400 cm roads through eased funnels; no narrow exit wall traps a banked vehicle. Tube pieces cannot
update safe recovery anchors. Existing standalone straight-cylinder authoring
remains a current public primitive; generated cylinders are all curved.

Loop radius (350 cm), tapered 220 cm ribbon and final 4 m aligned approach are
unchanged. Open jump ports have no blocking end wall; launch panels are flush
with the road.

Spirals retain the 800 cm radius and 800 cm rise/fall. Their 192 arc samples use
analytic surface normals and eased entry/exit grade, with eight strips across
the road to reduce twisted-quad ridges. Centerline grades remain below 23%.

A circuit follows the rounded boundary of a seeded connected set of 32 m planning
cells, with bounded growth and checks for holes, point contacts, rectangles and
long edges. Each reserved run is filled with continuous corner/chicane/return modules.
Ordinary collinear driving is capped at 16 m across piece IDs. The initial 24 m
grid and mandatory large-gimmick approaches/landings are exceptions. Every
course includes left/right 3 m corners and a narrow hairpin. Normal/hard samples
have at least four sharp direction changes per 100 m of ordinary road. Sprint uses an open portion
of the same winding outline. Seeded shuffled special-piece bags, compact pieces
and difficulty-dependent spacing increase variety; selected pieces remain
candidates; choosing the cylinder family guarantees a curved tube. Full-size pieces receive an 8 m flat approach and an 8 m stable exit,
including jump landings. Complementary spirals restore
elevation before closure. Identical pieces are capped at four, including the
circuit seam. Finite lengths are compared by summed reference traversal time
(900 cm/s baseline); the closest is selected without a hard tolerance or a
clock-dependent cutoff. This is an estimate, not a vehicle lap-time guarantee.
No whole-course physical simulation runs during generation.

`assembled_track` source metadata stores normalized settings, generator and
catalogue fingerprints, resolved pieces, reference length and time. Each piece
contains its cube span, origin/orientation, connection width, entry speed range,
3D path, local normal/forward, safe-anchor eligibility, drive/flight clearances,
tube radius and reserved bounds. Authoring/export validation checks exact current presets,
connections, closure, repetition, overlap and bounded sample/piece work. Existing
package/cell/memory budgets still apply. Same settings, seed and fingerprints
produce identical placement, content hash and package bytes.

Assembly mode replaces ordinary terrain/road generation. Road/wall triangles,
gimmicks and their occupancy are generated; the toy stage in
`godot/track_stage.gd` is render-only. It contributes no collider, occupancy or
spawn point. Overview rendering includes assembly paths.

`verify_document` regenerates and compares the complete current source for authoring/export. Package
`verify` validates the saved geometry and compares the saved course against its driving-content
hash. Mutable provenance and attribution do not certify geometry. No completion
record or successful-generator flag is manufactured. Edited content loses this
verification eligibility.

```sh
mapkit track-catalogue
mapkit generate-track /absolute/settings.json /absolute/new.memap
mapkit verify-track /absolute/new.memap
```

The Godot `MapKitBridge` exposes `track_catalogue`, `generate_track`, `track_json`
and `verify_track`. `godot/track_job.gd` runs generation and save on one cancellable
worker with immutable requests and monotonic publication generations. A failed,
cancelled or late result cannot replace a newer request. Save requires a new
destination and never overwrites an existing file. A finished but cancelled
package may remain as an unselected artifact.

Tests: `cargo test -p mapkit-package --test assembled_track --test courses
--test package_contract`. Consumer vehicle/race acceptance remains separate.

The 2026-09-28 outline regression adds 32 seeds with both turn directions, bounded
straight runs, unique outlines, determinism and exact position/tangent/normal
closure. Eight assembly tests pass, including the existing duration, geometry,
roundtrip, cancellation and source-mutation checks. This replaces the original
four-sided circuit skeleton within v1; old generated files are preserved, but
loading validates their saved bounded geometry independently of the current generator; it never rewrites them.

The tight-track regression covers 32 seeds × three difficulties × both modes ×
three candidate selections using measured paths, not preset names alone. It
checks real corner radii, return separation, aggregate straights, sharp-turn
density and selected tube presence. Special-track tests check shared frames,
nondegenerate faces, hollow occupancy, bounds and malformed source rejection.

`MapKitBridge.special_track_bounds` exposes the same validated L1 envelope to
authoring consumers; they must not infer a straight-cylinder bound for a swept
track. Retained-source accounting charges frame vectors and assembled path
capacities, including reserved space, before indexed source admission.

## RC venue and dynamic pieces (2026-09-28 replacement)

This replaces the uniform 240cm road / 200cm tube decision above. The first 24m
is 400cm wide. `straight_narrow`, `chicane_narrow`, `zigzag_narrow` taper from
400cm ports to 200cm over 3m, with at least one narrow road per generated course.
The ordinary zigzag has eight 90-degree, 3m-radius turns and 16m lateral swing;
its centreline cannot be bypassed by driving straight through a wide lane.
Curved tubes have 200cm and 400cm bores under the same cylinder selection.
A 12cm floor dip, 4m eased width transition and low 25–55cm guide walls lead into
the flared mouth. Rising tubes retain that eased entry under the 80cm hill.

Eight optional pieces join the existing catalogue (43 resolved variants):

| ID | Physical feature |
| --- | --- |
| `banked_chicane` | Open U-section, 2m-radius banks following four 4m-radius turns |
| `jump_barrier` | 36cm-high full-width hurdle; jump input, no launch pad |
| `overpass` | 2m-wide ramp and 2m-high straight shortcut over eight 3m-radius ground turns |
| `roller_waves` | Four smooth 80cm hills in 32m |
| `offset_jump` | Existing open gap with a landing displaced sideways 1.5m |
| `slalom_gates` | Three alternating low walls and a weaving road |
| `swing_gates` | Alternating rotating sweepers with staggered phases |
| `piston_gates` | Alternating gates moving up/down 1.8m with staggered phases |

`Piece.alternate_path` shares the entry/exit ports and declares the overpass
route. Exact validation, source accounting, renderer, collision, occupancy,
bounds, overlap checks and overview include both paths. Checkpoints stay at
shared piece ports. A current `swept_half_pipe` primitive uses the swept tube
frames but has an open crown, exposed edge rims and hollow occupancy.
The overpass ramp access/departure and hurdle/roller/jump run-ups are explicit
large-gimmick straight exceptions. The ordinary 16m rule remains unchanged.

Shared `rc_venue.gd`, shader and `track_stage.gd` select seed modulo three:
indoor blue/teal carpet, outdoor asphalt/grass, or multicolour plastic. Seed is
attached to native presentation; chunk batching and material allocation use the
same keys. The bounded stage adds striped perimeter, pits, banners, grandstands
or flags/blocks, and a checkerboard start gate. It has no physics/collision.
Authored non-assembled maps keep their existing materials.

Focused regressions cover actual widths/radii/port equality, alternate-source
mutation, shared halfpipe geometry, retained branch capacity, overview widths,
all 43 presets, deterministic saves, cancellation and 576 layout samples.
Godot fixtures render all three venue styles and eight pieces with a scale car;
consumer physics and detailed driving are separately recorded by the owning app.

## Seed variety and mandatory selections — 2026-09-28 replacement

The current v1 generator reserves every selected family before ordinary fill.
`sprint_lane` is a plain 32m road without an acceleration effect. Special chains
start at one piece and extend with a seeded 50% continuation draw; piece, sample,
cell and memory budgets remain binding. An impossible required layout returns
`E_TRACK_BUDGET`, never a reduced selection. Target duration is a preference and
may be exceeded. The bounded 24 candidates vary reservation positions, run lengths,
ordinary curve clusters, slopes and widths; the winning valid candidate balances
ordinary-distance straight share (seed target 10–90%) with duration. Sharp-turn
frequency, mandatory left/right hairpins and the 16m straight ceiling are removed.

Road/bore widths use 6/4/2m weights 2:2:1, with matched eased road ports and at least
4m corner radius. Dedicated loops, halfpipes and the overpass retain their own
profiles. Each piece records width, entry/exit width, ordinary-road membership and
chain ID/index/count. Assembly statistics exclude required approaches and special
pieces. Cylinder chains use one constant bore, open matching end rings and no
internal funnel. Only the outer ends have ramps: height is diameter/3 and eased
slope is at most 12%. The AI road reference and cylinder floor frames are separate.

`floor` defines the common venue height/extent. Generated collision triangles and
occupancy include it as `assembled-venue-floor`, always non-spawnable. The stage
uses that height; generic chunk display hides its duplicate face. Checkpoint sizes,
overview widths, ribbons, collision and occupancy consume actual piece samples.
All own formats stay v1; new generator/catalogue/source fingerprints require fresh
output. Original packages and user documents remain untouched.

Focused automated evidence is linked from root `docs/SEED_TRACK_VARIETY.md`.
Detailed driving, whole AI races, multiplayer and device acceptance remain user tests.

## Random extension replacement — 2026-09-28

The current v1 definition replaces the four-side scaffold, straight-share target,
probabilistic mandatory chain lengths and automatically balanced helices. Each
selected family requires one piece. Random ordinary pieces and optional selected
pieces extend the route with bounded backtracking; optional pieces consume only
the remaining time after mandatory pieces and a connection allowance. A final
safe connection exceeding that allowance rolls optional extensions back. Required
pieces are retained even when they alone exceed the requested duration.

Circuits close their position, heading and height through ordinary straight,
corner and grade pieces. Sprints retain the finish plaza. At most 24 valid/failed
candidate attempts are compared using absolute estimated-time error only. There
is no fixed outline or turn-count quota. Search bounds are 192 mandatory extension
attempts, 32 random-growth rollbacks, 4096 nodes per closure, and 16 terminal
rollback attempts, within the
existing 512-piece/32000-sample and downstream cell/memory limits. Cancellation
propagates through extension, closure, validation and packaging.

Gimmick families allow at most two in succession, including across a circuit
boundary. Mandatory approach/exit pieces do not reset this count. All cylinder
widths and bends share one family; upward/downward helices are separate. Randomly
chosen ordinary roads reset it. The initial 24m grid is preserved. Road/bore width
choices retain 6/4/2m weights 2:2:1; the closure solver uses 4m ordinary ports.

`slope_up/down`, `curve_up/down` and `curve_left_up/down` are ordinary pieces, not
checkboxes. Horizontal ports remain on the 8m lattice; they change elevation by
1m. Turning grades rise on their straight leads and stay level through the
circular arc so the 6m road's inner edge remains below 23%. Cubic easing, flat
quantized end segments and equal port forward/normal vectors join neighbouring
roads. Helices retain their 8m rise/drop.

Cylinder path samples now represent the actual inner floor, also used verbatim
by `TubeFrame.floor_cm`. The floor is raised above the neighbouring roads by the
existing diameter/3 eased ramp (maximum 12%); the exit starts at that same floor
and descends. There is no path-to-floor offset. Rendering, mesh collision, hollow
occupancy and AI thus describe the same 2/4/6m bore. Document bounds include the
actual generated gimmick safety envelope, including bends near an outer edge.

`straight_target_percent` is removed from the strict schema and metadata. Actual
length, estimated time and measured ordinary-road straight share remain. Formats
stay v1; source/catalogue fingerprints change. Existing files are preserved.

Focused tests: core assembly 3, package assembly 9, schema 1, special geometry 5,
overview 5, audit memory 8 and cooperative cancellation 2. The integration root
records exact logs and synthetic physics checks in `docs/SIMPLIFIED_SEED_TRACKS.md`.
Detailed driving, complete races and platform acceptance remain user checks.

## Attached obstacles and compact connections — 2026-09-29 replacement

This replaces the obstacle road presets, 32m sprint lane, old duration choices,
mandatory 1–4 ordinary spacer pieces and unconditional straight approach/exit
insertion above. `selection_ids` is the public selection contract, separate from
`pieces`. It includes one `obstacles` choice. The seven old obstacle IDs are
attachment kinds only; new generation requests reject them as selections.
`sprint_lane` is exactly 1600cm (two tiles), with no speed effect. Both modes offer
60/90/120 seconds; circuit lap limits are 3/2/2. Defaults remain circuit/60/normal.

Each required road gimmick tries direct connection first, then bounded minimal
ordinary connectors. Presets retain their internal run-up/landing space, and
pipes retain their explicit floor-height ramps. Optional growth is rolled back
when final connections exceed the target. The closest valid result among at most
24 attempts wins; mandatory-only overruns remain visible.

After closure and width materialization, a separately seeded stream attaches
obstacles without changing any road sample, length or time statistic. Metadata
records kind, owning piece, `main`/`alternate` path, arc station, lateral placement,
resolved position/frame, sweep width, avoidance lane and optional jump position.
The source validator deterministically reconstructs the attachment list and its
eligible-distance/target-count statistics. Rendering, collision, motion and AI
use these same records. Source retained-memory accounting charges vector capacity
and owned strings; existing per-cell geometry/gimmick admission remains in force.

Target density is one per 64/32/16m of usable surface for easy/normal/hard, rounded
to the nearest count (minimum one when selected). Count usable road length rather
than candidate-centre count. Bounded spatial bins distribute safe candidates;
object budget, existing action envelopes and free space can reduce the count.
Zero placements try other finite candidates, then fail with `E_TRACK_OBSTACLES`.
No attachment adds road. Seven kinds are seed-mixed without a coverage quota.

Grid, finish plaza, pipes/ramps, halfpipe, vertical loop, flight/landing and active
pad/ring pieces stay clear. Ordinary roads, curves, grades, helices, hills and both
overpass routes are candidates. Placement checks actual taper width, slope,
footprint, avoidance room, other road levels and full movement envelopes. World-Y
rotation is restricted to level supports; translating/static objects can follow
slopes. Slalom is two staggered walls with a bounded lateral AI transition.
The hurdle is still 36cm high. Dynamic objects preserve a lane throughout their
cycle; jump run-up/landing needs a sufficiently straight, level span.

All formats remain v1. The schema and source/catalogue fingerprints change; no
historical reader, conversion or source-file overwrite is introduced. Focused
Rust regressions cover geometry, attachment tampering, density/budgets,
reproducibility, road identity with obstacles toggled, all durations/lap limits,
save roundtrips, cancellation and original preservation. Consumer checks and
actual driving acceptance are recorded by their owners.

## Point queries and starting support — 2026-09-29

`MapKitBridge.cell_at(x_cm,y_cm)` and `MapKitRegionReader.cell_at` return a
PackedInt32Array `[x,y]`, or empty when unopened/outside the map. They query the
same authoritative cell topology as `cell_window` without JSON/window expansion.
`MapKitBridge.track_start_surface` resolves the actual piece ID in a validated
assembly's fixed 24m starting runway, or empty outside it. It grants no placement
admission; consumers still verify the full car, four wheels and registered physics.
Runway geometry, individual road IDs and current v1 format remain unchanged.
Focused checks: `assembled_start` (all runway pieces/outside edges),
`godot/tests/cell_query_validator.gd` (regular/regional packed lookup versus window).

## Pipe portal replacement — 2026-09-29

The approved lowered-entry/raised-exit effect uses the user's alternative:
entry road ends diameter/3 above the inner floor, and departure road starts
diameter/3 below it. Both outer roads use eased ramps with at most 12% grade;
the departure climbs back to the ordinary road level. Cylinders and the banked
halfpipe receive this treatment. Internal tube joints keep matching open bores
and no height step. The drop is explicit only at named entry/pipe/exit joins,
with exact integer centimetre validation. Arbitrary gaps remain invalid.

Route samples, collision, occupancy, overview and checkpoint generation use the
same lowered pipe and departure positions. Stunt floor geometry is unchanged;
only portal placement and the departure ramp direction change. The block returns
to its original height, so circuits retain ordinary closure. All own formats
stay v1; source/catalogue fingerprints change. Existing user packages and generated
artifacts are preserved, with no converter or rewritten files.

Validation: the 11 affected `mapkit-package --test assembled_track` tests passed
(191.84 s, optimized local Rust). New cases cover both pipe types, exact entry/exit
height differences, physical floor alignment and tamper rejection. Existing cases
cover 2/4/6 m bores, internal mesh seams, slope limits, deterministic roundtrip,
cancellation, original-file preservation, collision-only floor, circuit/sprint
generation and attached obstacles. Actual entry/exit driving is a user check.

## Wall topology (2026-09-30)

Track and finish-plaza walls emit one geometric sheet. Renderers and physics
consumers use two-sided handling; a reversed coplanar quad must not be emitted
as a second sheet with a different diagonal. Such duplicates create non-manifold
edges during continuous collision sweeps. Occupancy solids and wall dimensions
are unchanged. Cell clipping remains canonical; consumers that retain a complete
world may assemble adjoining obstacle triangles into a continuous physics mesh.

The focused `generated_walls_have_no_reverse_coplanar_duplicates` package test
passes for generated straight and finish boundary faces, and the Godot extension
build succeeds. Format/recipe numbers remain v1; source fingerprints invalidate
derived caches. Existing source packages and user artifacts are preserved.


## Grounded seed structures (2026-09-30)

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

## Connected-road wall clipping — 2026-10-02

Wall visibility now clips against the two actual triangles between adjacent
integer ribbon edges, shared with road generation. The previous chord rectangles
left wedge-shaped gaps on curves and tapered connections, producing repeated
wall fragments inside passable junctions. The 2cm outer-edge inset and 5cm
surface-height tolerance remain, preserving exterior boundaries and separated
road levels. Only connected roads, split/merge siblings and the alternate branch
can remove a wall. The same surviving segments emit render/collision faces and
occupancy. Formats remain v1; derived geometry uses the changed source fingerprint.

Focused results: playtest geometry verification.

## Quantized curved-road sampling — 2026-10-02 replacement

The [2026-10-05 [18] replacement](validation/curves-18-2026-10-05/README.md)
separates horizontal limits from height/pitch/full-normal limits. Ordinary spans
are now2.5m, horizontal angle/error limits are×5/3 and×25/9; vertical limits,
23% grades, final shared vertices and loop/pipe precision remain. Representative
segments fall38.88%; safety-limited short helices fall33.33%. Narrow generated
gates retain their exact samples and the existing100cm minimum course radius.
The following measurements describe the prior revisions.

Ordinary analytic curves and authored cubics start with 80cm spans. Shared
refinement limits tangent changes to two degrees while retaining the existing
outer-edge/twist error bound. This replaces 35cm/one-degree refinement, whose
repeated subdivision amplified centimetre height rounding into alternating
short flat/steep collision faces. End positions, normals, widths, surface strips
and common display/collision vertices are preserved. No format increment.

A 6m-wide curve's maximum centreline grade error falls from 0.03032 to 0.00824;
spirals fall from 0.10028 to 0.03645. Tests bound error below 0.04 and preserve
flat endpoint frames. Synthetic held-throttle Runtime checks pass all six
ordinary grades and both spirals. Downward spiral unsupported ticks fall 12→0
and road-relative chassis height span 0.1042→0.0329m. These are short fixture
results, not full-course or platform acceptance.

The 2026-10-02 [modular return policy](TRACK_AUTHORING.md#modular-circuit-returns--2026-10-02-replacement) replaces long direct cubic closure with bounded catalogue-piece routing and a short final seam.

The widened-wave follow-up limits zigzag/chicane amplitude by a conservative
curvature bound: `min(1.5 × width, length² / (300 × (width/2 + 100cm)))`.
A straight narrow piece retains a straight centreline. This prevents an 8m-wide
wave's inner ribbon from folding and emitting overlapping reversed walls.
Across five wave presets and all 2/4/6/8/12m widths, neither edge moves backwards;
coincident quantized edge points are harmless and zero-area faces are omitted.

## Playtest ramp and corner rail placement — 2026-10-02

Low/standard/triple ramp width is 160cm (formerly 100cm). Rail beam top is
40cm (formerly 60cm). Rails resolve a 30–90 degree chord across a level corner,
with a 110cm corridor wholly inside its supported road. Endpoints are at least
150cm from the piece ports; length is bounded by a 1600cm arc. Unsupported,
steep, straight, crossing-road or wall-intersecting sites are rejected.
The same quantized placement defines the beam/legs and independent grind line
(41cm source height); no inferred collider metadata or checkpoint shortcut is
introduced. Existing files remain intact. Current source regeneration uses v1.
Focused results.

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


## Closed outward barriers — 2026-10-02, root §44.262

This replaces single-sheet boundary walls with 50cm outward volumes on seed and
authored tracks, alternate routes and the finish plaza. Lane edges and wall
heights stay fixed. Quantized inner/outer/top/bottom/end faces drive rendering and
collision; matching section caps cancel across piece seams. Convex occupancy uses
the same vertices. Wall tops are physical support, never spawn/recovery candidates.

Edge miters preserve thickness on bends and width changes; each endpoint uses its
own surface normal and height. Connected-road clipping tests the full four-corner
section, including the outer face and top, against the existing road triangles.
It retains the 2cm edge inset and 5cm height tolerance. Bounds, sample-volume
clearance and cell costs include both barriers. Loop entry/exit lateral offset
adds 50cm on each side to avoid new barrier overlap, preserving the 220cm lane.
The same loop offset is shared with special-track generation. No format changes:
current v1 source/catalogue fingerprints identify the new geometry.

Loop miters use the actual fixed-axis ribbon edges, including rotated pieces,
to keep 50cm normal thickness. Endpoint heights are shared across segments.
Canonical quad diagonals and cancellation remove opposing faces collapsed by
centimetre quantization at the loop crown; emitted walls remain closed.
Ordinary edge tangents use a 50cm neighborhood so centimetre noise cannot fold
the outer wall backward. The original lane vertices stay fixed; tested normal
thickness stays within the two-centimetre endpoint quantization allowance.

Affected verification passed: 25 assembled-track unit tests, the generated-wall
opposing-face regression, all category/time
combinations (including the initially failing 90-second gimmick-only seed),
occupancy regressions and native bridge build. Runtime's generated-wall fixture
reports four supported wheels on the 60cm wall top (chassis height 0.71879m),
20m/s impact rejection from both sides, and both-seed/both-side scrape regressions
without relocation or ghost impulses. Actual course driving and platform acceptance
remain user checks.

## Analytic surface sampling — 2026-10-02 replacement

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
Two implementation failures were corrected: vertical endpoint side vectors and
loop-crown miter noise under adaptive sampling. Detailed driving is a user check.

## Difficulty-weighted routes — 2026-10-02 replacement

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

Focused verification in `validation/driving-map-2026-10-02/`: five layout units
pass, including distribution/uniform variants, seed17 repeatability in both modes,
category exclusion and bounded geometry. Aggregate route-family complexity is
70/139/175 for easy/normal/hard. All 42 category/mode/duration cases, package
roundtrip/tamper rejection, cancellation/invalid requests and native/CLI build pass.
The first unscoped core command failed to compile the unrelated existing
`tests/water.rs` fixture (missing `contact_class`/`snow_retention_percent`); the
affected `--lib` and named integration targets pass. Detailed driving remains
user verification.

## Quantized wall occupancy — 2026-10-03

The former eight-vertex wall proxy could contain repeated vertices, nonplanar
faces or concave corners while claiming to be convex. Walls now emit deterministic
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

Scoped results in validation/grid-input-2026-10-03:
33 assembled units, six occupancy tests and twelve assembled-package tests pass.
After the final cost refinement, seven wall tests and the cell-seam cost test pass.
Fixtures cover duplicate vertices, a concave plaza corner, nonzero folded wedges,
exactly flat sections, straight/curve/grade/spiral/loop/taper/plaza exterior parity,
closed representative meshes, wall thickness, cap cancellation and explicit budget
failure. Seed 7, 60-second one-way start cell (0,0) contains 6,802 solids, including
6,195 convexes; every convex passes strict validation. Branch composition and the
existing difficulty distribution are retained. Native bridge and CLI builds pass.
The unrelated earlier water-fixture compile failure above remains historical;
detailed application driving and platform acceptance remain user checks.

## Finite straight-road clearance — 2026-10-03

Authored 8m-wide approaches/departures separated by a 6m-radius quarter turn
were incorrectly rejected: the endpoint distance is about 8.49m, below the
9.6m sum of the sample-volume radii. After the existing broad check, level,
straight, constant-width drive ribbons now receive a conservative finite
footprint check. Bounds enclose every quantized ribbon edge and extend the sides
and ends by the existing 50cm wall plus 30cm vehicle clearance. A separating axis
must prove strict separation before a collision can be dismissed. Touching or
uncertain bounds retain the old result. No course IDs, smaller margins or larger
shared-port exclusions are involved. Curves, banks, grades, tapers, tubes, flight
and alternate paths retain the previous check. No public API/schema/version changes.

Focused results: two new
clearance integration tests, one fallback/edge-bound unit, eight other authoring
tests, five authoring units and three package tests pass; CLI/native build passes.
Coverage includes both turn directions at seven rotations, crossings/overlap,
insufficient side/end clearance, shared ports, low/clear overpasses, tilted
ribbons, invalid action/landing samples, source identity/tampering and cancellation.
The existing sampled broad phase is unchanged; its pre-existing near-tangent
sample-phase limitation is recorded with the initial fixture failure. Detailed
application driving and platform acceptance remain user verification.


## Continuous unjoined straight-road clearance — 2026-10-03 replacement

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
Earlier failures remain recorded; detailed consumer acceptance is separate.

Course sealing now retains stable non-overlapping checkpoints only. Stored package source is preserved; the public course view derives a new course identity only if overlap removal changes its geometry. Fewer than two effective gates disables racing without rejecting the map. Sphere/hemisphere overlap and continuous chassis-capsule entry tests cover grazing, stationary/initial-inside rejection and separated half volumes.
