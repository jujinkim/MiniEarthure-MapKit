# Seeded modular tracks (current v1)

`mapkit_core::assembled_track` owns the catalogue, dimensions, seeded selection,
connections, corridors and collision geometry. `mapkit_package::assembled_track`
adds a matching public course and package save/verification. Consumers do not
reimplement the generator. All formats remain v1.

Settings contain an exactly representable nonnegative JSON integer `seed`,
`circuit`, `minutes` (1/3/5), `difficulty` (easy/normal/hard), candidate `gimmicks`
and `time_minutes` (0–1439). Defaults are circuit, three minutes, normal and noon.
Basic straight/slope/zigzag/curve connectors remain available independently of
the optional special pieces. A selected special is a candidate, not a promise
that it appears; helices have a complementary return connector.

The base cube is 800 cm. Straight, gentle hill and zigzag presets occupy one
tile; turns and large structures reserve a four-tile cube. Origins stay on the
800 cm lattice, including helical changes of elevation. The ordinary road is
600 cm wide with 120 cm walls. Loop access tapers to the existing 220 cm ribbon
with radius 250 cm; its walls follow the ribbon normal. Cylinder walls belong
to its existing hollow mesh. Open jump ports have no blocking end wall.
Basic hills stay below 23% grade; helical climbs use an 800 cm radius. Jump
and loop approaches include speed panels; launch panels sit flush with the road.

A bounded rectangular circuit skeleton closes first, or a sprint skeleton runs
from start to finish. Seeded slots choose presets with stable approach sections;
difficulty changes special-piece spacing. Consecutive runs are capped at four,
including the circuit seam. Finite candidate lengths are compared by summed
reference traversal time (900 cm/s baseline); the closest is selected without
a hard tolerance or a clock-dependent cutoff. This is an estimate, not a lap
time guarantee. No whole-course physical simulation runs during generation.

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
