# Independent roads and composite worlds

Owner of designed ordinary roads, track overlays on existing maps and shared
surface attachments. Field bounds live in the generated schema; the overall
contract is [CURRENT_V1](../spec/CURRENT_V1.md).

## Designed roads

An ordinary `Road` may opt into `design`:

| Field | Meaning |
| --- | --- |
| `control_points` | Joined cubic controls, 3n+1 source-centimetre points |
| `terrain_policy` | `auto_fit`, `preserve` or `elevated` |
| `shoulder_cm` | Shoulder width used for local cut/fill |

- Sampled points, widths, lane markings, sidewalks and graph node IDs remain part
  of the road. Export verifies that the samples match the controls.
- Heights and shared tangent frames come from the authored curve; integer terrain
  samples never pull the carriageway up or down. Roads without `design` keep the
  imported terrain-following path. Opening a source never adds or converts designs;
  the first explicit edit adds controls (`road_design::{from_points,compile,edit}`).
- `road_design::edit` moves shared endpoints, connected authored roads, attached
  tools and connected track entries atomically.
- Same-level crossings require an explicit shared node; separated bridges/tunnels
  keep their own surface level.
- Original heightmap payloads stay immutable. `auto_fit` roads derive cut/fill and
  shoulders locally from the nearest road edge; deleting or moving a road restores
  the previous terrain. Redundant terrain contacts beneath the road are removed.
- `preserve`/`elevated` roads reject terrain penetration on execution export.
- Authored road tops use contact class 3 ([ROAD_CONTACT](ROAD_CONTACT.md)).

## Track overlays on an existing map

`assembled_track.authoring.terrain_integration` makes a track an overlay on the
same document. `assembled_track::composite::apply_source(&document, &source)`
(Godot `apply_track_source`) recompiles the overlay and replaces only its owned
products, keeping terrain, buildings, water, assets and independent gimmicks.
An empty overlay restores the original document.

- `Source.terrain_policies` selects `auto_fit`, `preserve` or `elevated` per
  instance ID.
- `road_connections` records `{road, start, instance}`; `composite::road_port`
  gives the ordinary road's actual entry/exit frame, and edits propagate through
  connected track chains.
- Special meshes keep their shape and receive terrain-based supports.
- Export checks terrain, independently occupied driving space, road clearance and
  support interference against the composed map, without moving or deleting source
  objects. Diagnostics name the failing object and source coordinates.
- Composite generation, distant display, occupancy, costs and content hashes
  include both the environment and the track.

## Surface attachments

`surface_attachments` bind actions, obstacles and rails to a stable `surface_id`
and `station_cm`. Ordinary road IDs and `track:<instance-id>` share one resolver
(`surface::{path,resolve,apply}`). Derived physical geometry and rail interaction
lines move with the source surface.

## Road course proposals

Read-only `road_course::suggest(document, request)` (Godot
`suggest_road_course(document_json, request_json)`) proposes a sprint or circuit
over the authored road graph without changing the document.

- Request: `start_cm` (map x/z), optional `heading`, `mode`, `duration_seconds`
  20–300 (length at the shared 900 cm/s reference speed) and optional
  `min_width_cm`. The start snaps to the nearest eligible centreline within 50 m.
- Routes use explicit graph nodes only, never geometric crossings, and skip
  self-loop roads and arms sharper than 150°. The search is deterministic and
  bounded (200,000 expansions, 512 roads): straightest arms first; circuits head
  home after 40% of the target and accept 40–160% of it, stopping within 10%.
  Sprints finish inside the last road at the target length.
- Gates sit on the road centreline height with radius half width + 30 cm
  (≥ 100 cm): start, sprint finish, every junction turning ≥ 30°, then even fill
  at max(50 m, length/48). Earlier gates win overlaps; more than 64 gates blocks.
- Result: `mode`, `start_direction`, `gates`, ordered `roads`, `length_cm`,
  `target_cm`, `attachments`, `road_designs`, `shortcuts`, `diagnostics` (`code`,
  `message`, `blocking`, nullable `road`/`position_cm`) and `applicable`. Consumers seal gates as an ordinary v1
  course; roads outside the course stay drivable.
- Optional `gimmicks` (0–16) with `seed` propose `attachments`: seeded kinds from
  acceleration panel, jump panel, boost chain, low ramp and grind rail, spread
  evenly along the course on roads travelled in their own direction, sized for 1:8
  city blocks: at least 3 m from leg ends, launching kinds keep 15 m before a turning
  junction, panel height 100 cm. A seeded kind that does not fit (ramps and rails
  need wider roads) yields to the next kind. Each one must pass the local resolver,
  then full document validation with the map's existing attachments (≤ 12 tries per
  kind; edge-of-map safety bounds reject). Unaligned host roads get the Editor's
  default alignment, returned as `road_designs`; shortfalls warn
  `W_ROAD_COURSE_GIMMICKS`.
- Optional `shortcuts` (0–4) propose ordinary-road corner cuts: at junctions where
  the course turns ≥ 60°, a straight ground chord joins new graph nodes 15, 10 or
  6 m before and after the junction (longest that fits; hosts keep 2 m beyond).
  Hosts must be unaligned ground roads without attachments or track ports, not the
  start road and not already split. The chord takes the narrower host width and
  keeps half its width + 30 cm from buildings, water and other roads; the split
  map must validate. Gates inside the bypassed stretch are omitted so both paths
  count; gimmicks skip split roads. Shortfalls warn `W_ROAD_COURSE_SHORTCUTS`.
- Where buildings block every ground chord, the corner tries an elevated chord
  (reaches 40, 30, 20, 15 or 10 m) on maps without heightmaps. The seed orders two
  `form`s: `ramp` (at most 12% grade) and `crest` (at most 30%; fast cars may lift
  off its crests). A ground stub (the width, at least 3 m) leaves each split
  junction; the elevated chord then runs a level lead (1.5 widths + 2.5 m) at the
  terrain base, ramps up to one flat deck 51 cm above the tallest roof its corridor
  covers, and back down. Ramps may pass over lower buildings they already clear. The
  chord, like a ground chord, keeps clear of water and other roads, so it never
  needs a separated crossing.
- Each `Shortcut` carries its `form`, the exact `replaces` records, the `[kept, new]`
  halves per host (first keeps the road ID), then the ground chord, or the entry
  stub, elevated chord and exit stub, and its nodes.
- `apply_extras(document, extras)` (Godot `apply_road_course_extras`) applies
  `shortcuts` (stale hosts fail `E_ROAD_COURSE_STALE`), then `road_designs`, then
  appends `attachments`; it refuses alignments for already aligned roads.

## Export rules

- Driving-content edits make existing course hashes/proofs stale.
- Free-roam exports may have no course or only disconnected route drafts; unsafe
  geometry always blocks export.
- Race-track export also verifies routes, ground start and common checkpoints,
  then binds a course to the current source/assets without inventing player
  completion evidence.
- Execution cells remain 32 m; format versions and resource caps are unchanged.

## Ordinary generation

Ordinary map generation uses integer road arrangement, connected sidewalks,
courtyard buildings, indexed placement validation, authored tree assets and
environment profiles, with the current road partition and three-dimensional seam
rules. Regional source derivation follows [REGIONAL_SOURCE](../spec/REGIONAL_SOURCE.md).
Road rounding and safety furniture follow [ROAD_SAFETY](ROAD_SAFETY.md).

## CLI and audit

- `mapkit design-roads INPUT.json NEW.json` authors a new document.
- `mapkit compose-world DOCUMENT.json SOURCE.json ATTACHMENTS.json NEW.json` applies
  a v1 authored overlay (or JSON `null`) and the shared surface attachments to a
  fresh document. Validation failures and existing destinations preserve all inputs.
- `mapkit audit-roads PACKAGE` samples generated collision every 0.5 m on five
  width lines against the designed surface (at most 1 cm deviation), checks
  junction fans/overlap, duplicate terrain contacts, per-cell road-paint budgets
  and the default-world 12% design grade.
- Dense road paint is compacted within 1 cm for display; collision keeps full curve
  sampling, and shader path/exterior arrays stay capped at 128 segments.

Detailed real driving on designed roads is user verification.
