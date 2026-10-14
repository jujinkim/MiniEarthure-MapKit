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
