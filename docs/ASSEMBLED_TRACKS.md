# Seeded modular tracks (current v1)

Current behavior is the **Random extension replacement** below. Earlier dated
sections preserve their implementation/test history, not current generation rules.

`mapkit_core::assembled_track` owns the catalogue, dimensions, seeded selection,
connections, corridors and collision geometry. `mapkit_package::assembled_track`
adds a matching public course and package save/verification. Consumers do not
reimplement the generator. All formats remain v1.

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
tube radius and reserved bounds. Validation checks exact current presets,
connections, closure, repetition, overlap and bounded sample/piece work. Existing
package/cell/memory budgets still apply. Same settings, seed and fingerprints
produce identical placement, content hash and package bytes.

Assembly mode replaces ordinary terrain/road generation. Road/wall triangles,
gimmicks and their occupancy are generated; the toy stage in
`godot/track_stage.gd` is render-only. It contributes no collider, occupancy or
spawn point. Overview rendering includes assembly paths.

`verify_document` regenerates and compares the complete current source. Package
`verify` also compares the expected course against its actual driving-content
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
current-generator verification requires regenerating them with their settings.

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
