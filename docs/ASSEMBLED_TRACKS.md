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
that it appears; helices have a complementary return connector.

The base cube is 800 cm and the ordinary road is 400 cm wide with 120 cm walls.
Straight/gentle-hill presets occupy one cube. Zigzag uses a full 32 m length and
about 5.8 m lateral excursion; compact obstacles and panels occupy 16 m. Sharp
left/right corners have an 8 m radius. Hairpins follow a continuous half ellipse
with 16 m and 8 m radii, joining two 8 m corners around a narrow tip. Sharp turns,
hairpins and zigzags declare the drift action. Large structures reserve a 32 m
cube. Loop access still tapers to its 220 cm ribbon, with radius 350 cm for fleet clearance at the crown. Cylinder radius
is 200 cm (one third of the previous diameter), with a tapered road entrance.
Loop access completes its lateral alignment before a final 4 m straight climb
approach. Open jump ports have no blocking end wall; launch panels are flush with
the road.

Spirals retain the 800 cm radius and 800 cm rise/fall. Their 192 arc samples use
analytic surface normals and eased entry/exit grade, with eight strips across
the road to reduce twisted-quad ridges. Centerline grades remain below 23%.

A circuit follows the rounded boundary of a seeded connected set of 32 m planning
cells, with bounded growth and checks for holes, point contacts, rectangles and
long edges. Corners bound straight headings to 48 m. Sprint uses an open portion
of the same winding outline. Seeded shuffled special-piece bags, compact pieces
and difficulty-dependent spacing increase variety; selected pieces remain
candidates, not guarantees. Full-size pieces receive an 8 m flat approach and an 8 m stable exit,
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
