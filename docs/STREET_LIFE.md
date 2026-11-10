# Street life

Voxel walkers on the generated sidewalks and pigeon flocks at benches and bus
shelters. Everything here is derived presentation: no collision, no Runtime or
race state, no document or format field, and nothing enters a generated hash.
Walkers use only generated road sidewalks and painted junction crosswalks.

## Walk lanes (native)

`mapkit-godot` `walk_lanes.rs` runs in presentation decorate, right after the
sidewalk frames ([ROAD_DESIGN](ROAD_DESIGN.md#sidewalks-and-curbs)), on packed
cells. It reads the non-vertical sidewalk tops (`<road>:sidewalk`) on a 2 m grid
and joins tops that share a vertex into connected pieces.

- **Lanes.** For each road with a sidewalk (`sidewalk_width_cm > 0`), each side
  and each paint segment, a probe at the segment middle walks outward from the
  carriageway half width in 0.1 m steps (to the sidewalk width + 1 m) to find
  the top band. The lane runs at the band middle, sampled every 1 m; a sample
  off the tops splits the run. Runs keep a vertex at least every 4 m and at
  turns above 10°, drop below 2 m, take their height from the top under each
  point and a half width of half the band less 8 cm (at least 10 cm).
- **Corner links.** A lane end links to the nearest end of another road on the
  same connected top within 6 m (ends within 1.5 m need none, the field merges
  them); the link follows the chord over the tops, or the nearest top centroid
  where the chord leaves them.
- **Crossings.** Each painted crosswalk (`road_style::crosswalk_centres`, shared
  with the sidewalk frames) gets two kerb mouths 0.15 m past the carriageway
  half width, at the height of the top under them; a crosswalk without a top
  under both mouths has no crossing. A signal head (`builtin:signal_h`,
  `builtin:signal_v`) within 3 m (game scale) of the crosswalk centre marks it
  signalled. Each mouth joins the nearest lane within 2.5 m: the lane is split
  at the mouth's foot and a short link reaches the mouth.
- **Density** (walkers per map metre, at the lane middle): 0.06, + 0.06 near a
  bus shelter, + 0.09 near a subway entrance or station sign, + 0.03 near a
  crossing mouth, + 0.06 × the commercial share of building footprint area
  near it, + 0.03 near a food or shop POI; at most 0.30. "Near" measures source
  geography: 40 m, 150 m, 30 m and 40 m ÷ the map scale 1:D
  (`osm_source_denominator`), D = 3 for a map without one.
- **Ambience** of the cell (always, lanes or not): the projected areas of its
  grass ground, carriageway and building roof faces over the cell area, and the
  share of 16 × 16 points inside a water body.

Caps per cell: 2,048 points, 512 lanes and links, 256 crossings. Lanes come
first, then links, then crossings; over a cap the rest is left out and
Authoring scopes warn `walk_lane_points` (`runtime_rejects=false`).

Presentation keys (scene metres; only with lanes, except `ambience`):

| Key | Type | Content |
| --- | --- | --- |
| `walk_points` | PackedVector3Array | lane and link points |
| `walk_lanes` | PackedVector4Array | (first point, count, half width, density) |
| `walk_links` | PackedVector4Array | corner and mouth links, same encoding (density 0) |
| `walk_crossings` | PackedVector3Array | two kerb mouths per crossing |
| `walk_crossing_signals` | PackedByteArray | 1 per signalled crossing |
| `ambience` | Vector4 | area shares: grass, asphalt, building, water |

Native decorate counts these bytes exactly in `presentation_bytes`: 12 B per
point, 16 B per lane or link, 25 B per crossing (mouths and flag), 16 B for
`ambience`. `presentation::cost` adds no bound (it also serves far plans and
the MapEditor work estimate): a lane needs sidewalk tops and walls, and each
triangle already reserves far more before generation.

## Field (`godot/street_life_field.gd`)

One camera-local field per resource cache (`render_resource_cache.street_life_field`,
key `builtin:street_life_field`, a fixed `MEMORY_BYTES` claim pinned by every
registered cell); none on low or without a cache. `chunk_renderer` registers a
cell after its grass (`_street_life_step`, `street_life_cell`, released by
`cancel`) whatever the live switch; each cell's graph is in its estimate
(`render_memory.street_life_bytes`), returned at attachment when nothing
registers. Walker and pigeon state live in packed arrays, not objects.

Display quality (`display_quality.profile`):

| Key | Low | Medium | High |
| --- | --- | --- | --- |
| `walker_count` | 0 | 40 | 80 |
| `walker_distance` (m) | 14 | 20 | 28 |
| `walker_near` (m, six-box figure) | 0 | 6 | 8 |
| `pigeon_count` | 0 | 16 | 32 |
| `street_life` | false | false | false |

The Client owns `street_life`; spawning and drawing follow the live profile, so
the switch and adaptive trims need no reload.

- **Scale.** Game scale, like the builtins and the RC car: figures 57 cm ×
  0.94–1.06, pace 0.33–0.53 m/s, give-way at 3.3 m, pigeon flush at 1 m.
- **Graph.** Lanes, links and crossings are edges; ends of any registered cell
  within 1.5 m share a node (2 m hash grid). Walkers keep their pace and a
  lateral slot on the right (left under left-hand traffic).
- **Nodes.** A crossing (or the kerb link to one) is taken one time in three;
  else another lane or link (never a spur ending only at a signalled
  crossing's kerb); else the walker turns round. Signalled crossings are never
  entered: the signal heads cycle locally and are not reproduced on the CPU. A
  crossing is entered only when no vehicle is within 3 m of it, now or along
  its course over the next 1.5 s; otherwise the walker waits at the kerb (6 s
  at most).
- **Give way.** A vehicle within 3.3 m that would meet a walker within 2 s
  makes it step aside at 0.3 m/s, up to 0.5 m past the lane when the vehicle is
  on the sidewalk; boxed in it stands and turns round after 2 s.
- **Vehicles.** `render_resource_cache.street_life_obstacles(points,
  velocities)` takes up to 8 vehicle display positions every frame; without
  velocities they follow from the previous call, and a list older than 0.5 s
  is dropped.
- **Population.** min(`walker_count`, Σ lane length × density × time ×
  weather) over lane segments whose middle is within `walker_distance`: time
  0.25 at night to 1 by day (environment context daylight), × 0.6 on wet or
  snowy streets. Walkers spawn 0.6–1.0 × that reach (out of view when
  possible), fade in over 0.5 s and are dropped past reach + 3 m. Seeds are
  local (`seed_random` for validators).
- **Fades.** Dithered within 0.35–0.8 m of the camera and inside a vehicle's
  0.25 m box.
- **Pigeons.** Flocks of 3–7 at a hashed 30 % of benches and bus shelters
  within 16 m of the camera, by day and in dry weather. A vehicle within 1 m,
  or within 2 m above 1.5 m/s, flushes the flock: it arcs up 1.5–3 m, circles
  at 2–4 m for 6–12 s and lands back.
- **Ambience query.** `ambience_at(eye)` (also on the cache) returns
  `{grass, asphalt, built, water, walkers_near}`: an inverse-distance mix of
  the registered cells within 96 m and the walkers within 6 m.

## Meshes

`pedestrian_meshes.gd` builds voxel box figures once per process: a near
walker of six boxes, a far walker of three and a pigeon of five. UV carries
limb and colour slot, UV2 the pivot (exact floats; vertex COLOR is not exact on
every renderer); `INSTANCE_CUSTOM` is (top and bottom palette
index, gait phase, alpha), for pigeons (tone, flap, wing phase, alpha).
`pedestrian.gdshader` (warmed when the scripts load) swings the limbs and
picks clothes from a muted Korean street palette. Two walker MultiMeshes and
one pigeon MultiMesh draw without shadows.

## Validation

- `cargo test -p mapkit-godot`: the `walk_lanes` tests on a synthetic two-road
  junction (lanes, corner link, mouths, signals, bare road, cap note, bytes,
  footfall radii) and the sidewalk frame test.
- `street_life_field_validator`, bundled with `grass_field_validator` and
  `sidewalk_paving_validator`: tiers, estimate, registration with the switch
  off, refused and low claims, reach, live toggle, give way, kerb holds,
  signalled crossings, the crossing share, fades, pigeons, ambience, memory,
  release and close.
- Runtime `package_presentation_source_validator --fixture`.

Walkers in each camera view, occlusion near the camera, pigeons and mobile
cost are visual and device checks.
