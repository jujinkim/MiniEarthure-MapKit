# Seeded modular tracks (current v1)

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

The base cube is 800 cm; ordinary roads are 240 cm wide with 120 cm walls.
Left/right 90-degree corners have a circular 300 cm centreline radius between
5 m straight approaches/exits, in an 8 m port span. Hairpins have a 400 cm radius
and 800 cm separation between return lanes. A 16 m chicane joins four 3 m-radius
corners, and a reserved 48 m bay adds two narrow returns and continuous corners.
Compact obstacle/panel pieces use a gentle lateral displacement so adjoining
straight portions cannot create a disguised long straight. Obstacle envelopes
leave room for small vehicles. Checkpoint radii are half road width plus 30 cm.

The cylinder selection is one `selection_groups.cylinder` family: right/left
90-degree, right/left U-turn, S and (hard difficulty) a gently rising/falling S.
The bore diameter is 200 cm, with a cosine mouth flare of at most 12 percent.
Centreline bends have a 400 cm radius. A `swept_cylinder` special track stores
bounded floor/normal/tangent frames; those same frames define its circular
mesh, hollow occupancy, spawn exclusion and full-section admission corridor.
The inner surface has 128 angular divisions. Entrances/exits connect to full
240 cm roads; no narrow exit wall traps a banked vehicle. Tube pieces cannot
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
