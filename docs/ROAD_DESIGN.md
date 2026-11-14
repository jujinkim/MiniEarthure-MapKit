# Independent roads and composite worlds

Owner of designed ordinary roads, track overlays on existing maps, shared
surface attachments and the shapes of displayed road paint. Field bounds live in the generated schema; the overall
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
- Seeded races: `road_course::seed(document, request)` (Godot `seed_road_course`,
  or `seed_package_road_course(request)` on the opened package's source) takes
  `seed`, `mode`, `duration_seconds` and optional `min_width_cm`, `gimmicks` and
  `shortcuts`. The seed orders the eligible roads; up to 12 start mid-road in a
  seeded direction, and the first route within the length tolerance wins
  (otherwise the first applicable one, otherwise the diagnostics). Gimmicks and
  shortcuts are then proposed with the same seed for that start only. The result
  carries the resolved `start_cm`/`heading`, the `suggestion` and the seed
  `time_minutes`/`weather` from `track_seed_defaults`, which consumers apply to the
  play or preview environment only, never to the map.
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
courtyard and multi-part buildings, road-gated barriers, an optional default
ground surface, indexed placement validation, authored tree assets and
environment profiles, with the current road partition and three-dimensional seam
rules. Regional source derivation follows [REGIONAL_SOURCE](../spec/REGIONAL_SOURCE.md).
Road rounding and safety furniture follow [ROAD_SAFETY](ROAD_SAFETY.md).

## Junction admission for imports

`unbuildable_junctions(document)` (Godot `unbuildable_junctions`) lists graph nodes
joining two or more road ends whose junction the current road planner rejects, for
example a source way that doubles back on itself at a node or approaches too close to
form one passage. Each node is planned from its own incident roads only, so a failing
neighbour never implicates it. Consumers may separate such nodes into independent road
ends at the same position; the planner rules themselves are unchanged.

A local road plan admits up to 24,576 faces and boundaries, enough for the densest
1:8 city cells with kept source junctions; larger plans fail with `E_BUDGET`.
A prepared source keeps its junction plans in one thread-safe 2 MiB cache (least
recently used first out) shared by every cell's estimate, ground and sidewalk plans.
Each junction is a complete endpoint star, so plans, errors and work never depend on
it; junctions with designed approaches sample under the caller's profile and are
planned per call.
Building/road clearance checks test footprint containment and crossings through
the footprint's edge index.

## Road paint

`urban_surface.gdshader` draws every mark from the road style uniforms
([ROAD_SAFETY](ROAD_SAFETY.md) owns which roads get which marks); it is display
only, so cells, collision and hashes are unchanged. Sizes are scene metres, real
sizes at the cards' 1:3 in parentheses; stations count from the path end.

- Each mark is box filtered over the pixel footprint along and across the nearest
  path segment, taken from world-position derivatives, so edges are antialiased
  near the camera and distant marks settle to their area coverage without shimmer.
  The windows that skip mark evaluation widen by half the footprint, so grazing
  views never cut a filtered edge. No mark adds a loop over the 128 path segments.
- Dividers are 5.6 cm (17 cm) lines dashed over 37.5% of each fitted period
  (about 0.9 m line, 1.5 m gap: Korean urban 3:5). On the inbound half the last
  10 m (30 m) before a stop line, at most 45% of the road, is solid. Even lane
  counts with `center_line` paint a double yellow centre (5 cm lines 9 cm apart);
  two-lane roads without `edge_lines` paint a single 5 cm (15 cm) yellow instead.
  A document whose `traffic.centre_line` is `white` paints the same centre lines
  white (Japan, most of Europe, the UK, Oceania). A reflective stud sits on the centre every two dash periods;
  studs glow faint amber while the sun is below the horizon. Lines end at the
  stop line, or at the crosswalk on a departure end.
- Crosswalks lie 0.3–1.7 m from the end: whole 16.7 cm (50 cm) bars along traffic
  on a 38.3 cm pitch (65 cm gaps), centred on the road and kept 20 cm inside its
  edges. The 13 cm stop line starts 1.95 m from the end (0.55 m without a
  crosswalk) and spans only the inbound half, also before mid-block crosswalks;
  one-way roads stop only at their travel end.
- Arrows are 1.7 m (5 m) long, tips 0.5 m behind the stop line, drawn only where
  the far end's crosswalk or stop line leaves room and repeated 7.5 m further
  back when the road is at least 14 m longer than the stop distance. Lanes with
  no movement get none. Straight and turn parts share one shaft (combined
  straight and turn arrows); a turn bends through a quarter circle into its
  head. Each arrow stays 4 cm clear of dividers, the double yellow and edge
  lines (about 11 cm from a single yellow): a turn-only shaft prefers 12 cm away
  from its turn, any shaft shifts as far as needed, and turn heads shorten on
  narrow lanes.
- Before a crosswalk every inbound lane gets 1.5 × 0.5 m warning diamond
  outlines (횡단보도 예고) 5.5 m and 17 m (16 and 51 m) behind the stop line, each
  only where the road leaves room for it.
- Paint is 0.25 smoother than asphalt with specular 0.45 (asphalt 0.35). Coverage
  wears with broad noise and wheel tracks, lets aggregate and cracks show through,
  and tyre tracks grey it; crosswalk bars vary per bar. With `detail_quality` ≥ 1,
  where the footprint resolves them, bars chip at their edges, their gaps collect
  grime and all paint shows fine abrasion; within 8 m the paint edge raises the
  normal like 1 mm (3 mm) thermoplastic, and within 3 m glass beads glint in fixed
  8 mm world cells (integer hash, never time-varying). Quality 0 keeps the filtered
  marks, broad and wheel-track wear, aggregate and crack show-through, per-bar
  wear and the studs; it skips chips, gap grime, abrasion, the raised edge and
  glints.
- `road_marking_validator` (rendered) reads road paint back from top-down
  orthographic renders, which check geometry only (surface effects are off).
  On a straight two-way road at qualities 0 and 2 it checks bar count, width,
  pitch and centring, inbound-only stop lines, solid lines before stop lines and
  the 3:5 dash ratio, combined arrows clear of the double yellow, diamond
  placement and the double yellow centre, and compares far crosswalk pixels with
  analytic bar coverage. A one-way six-lane road checks the travel-end-only stop
  line, lines cut at the departure crosswalk and bare lanes without a movement; a
  short left-hand road checks the mid-block stop line, the 45% solid cap and a
  kerb-lane arrow clear of the edge line; a two-lane road checks the single
  yellow without edge lines, the double yellow with them and the centre studs.
  A near perspective shot with surface effects on checks that quality 2 chips,
  abrades and grimes the crosswalk while keeping its bars, that quality 0 does
  not, and that frames repeat exactly. `MAPKIT_ROAD_MARKING_CAPTURE` optionally
  writes the images.

## Sidewalks and curbs

Sidewalk paving, curbs and gutters are display only: sidewalk geometry, curb
heights, collision and cell hashes are unchanged
([ROAD_CONTACT](ROAD_CONTACT.md#sidewalk-boundaries-and-shared-geometry)). Sizes
are map metres with real sizes at 1:3 in parentheses.

- Frames: presentation (`road_style.rs`) gives every sidewalk top and wall
  (`<road>:sidewalk`) of a packed cell a frame, uploaded as UV/UV2 of the shared
  `concrete` batches (`sidewalk_triangles`, `sidewalk_frames`;
  `render_memory.sidewalk_arrays`): the distance along a curb, the distance across
  from it (walls: the rise above their bottom), the offset from a crosswalk centre,
  and a kind plus a section hashed from the road record. A curb is a sidewalk wall
  whose outer side is a carriageway face; almost collinear pieces share one line,
  and the along distance projects onto its direction snapped to 0.25°, so a
  straight curb keeps its paving phase across cells. A top takes the nearest curb
  within 3 m, otherwise it is paved world-aligned; bends and corners change frame
  at triangle edges. Legacy dictionary cells have no frames.
- Tops: a granite curb band 0.06 m (18 cm) wide, a mortar line, a soldier course of
  0.033 × 0.067 m blocks, then one of three bonds per section: 90° herringbone or
  running bond of 0.067 × 0.033 m (20 × 10 cm) blocks, or 0.067 m squares, in one
  of four Korean mixes (grey, red-brown, ochre, charcoal; three tones at
  45/35/20 %). Each block varies in tone; 7 % are replacements in another tone, 2 %
  sunken (darker, tilted) and 1.5 % cracked. Recessed sand joints darken and gather
  grime, chamfers darken block edges, and joint coverage and block tones settle to
  their mean over the pixel footprint.
- Curbs: granite with a per-stone tone and black, white and feldspar grains in
  1.5 mm cells that fade with the footprint; sawn joints every 1/3 m (1 m) line
  up between the band and the curb face, which share the along distance; a worn
  arris runs along the road edge. Faces darken at their base and along a damp
  line. Other sidewalk edges show the block mix, darkened.
- Tactile paving (점자블록): at a painted crosswalk (`crosswalk_start` or
  `crosswalk_end` on a road longer than 5 m), tops whose curb runs within about
  30° of the crossed road get a yellow dotted warning pad (5 × 5 studs per 0.1 m
  (30 cm) tile) 0.1–0.3 m from the curb and as wide as the 1.4 m crossing, and a
  0.1 m linear guide strip of four raised bars leading 1.3 m in from the pad. A
  triangle holds one crossing; unpainted crossings and curbs across the crossed
  road get no pad.
- Gutters: marked roads with a sidewalk (`curbed`) draw a 0.12 m (36 cm) concrete
  gutter inside their carriageway edges along the painted body, with joints every
  1/3 m, a damp line at the curb and a dark seam to the asphalt; paint covers it.
  Unmarked roads and junction areas have none. Concrete carriageways (`slabs`)
  keep 0.7 m cast slabs; other concrete ground lays world-aligned 0.1 m (30 cm)
  squares in its base tone.
- Quality: `detail_quality` 0 keeps the curb band and stone joints, the bonds with
  filtered joints and block tones, the tactile pad and strip and the gutter; it
  skips grains, replaced, sunken and cracked blocks, joint grime, chamfer shading
  and stud shading. Quality 2 also tilts blocks, chamfers, studs and the arris in
  the normal near the camera. No per-pixel loop is added: tops read their frame
  and the gutter reuses the paint loop's nearest segment.
- `sidewalk_paving_validator` (rendered) reads a framed sidewalk back from
  top-down orthographic renders: the 0.06 m granite band and its 1/3 m stone
  joints, the tactile pad and guide strip at a crossing mouth and nothing beside
  them, at qualities 2 and 0; no tactile paving without a crossing; running-bond,
  row and soldier-course pitches; the far render matching the near mean; repeated
  frames; and a kerbed road's gutter, damp line and seam (none when not kerbed).
  It also checks the frame upload of `concrete` batches.
  `MAPKIT_SIDEWALK_CAPTURE` optionally writes the images.

## CLI and audit

- `mapkit design-roads INPUT.json NEW.json` authors a new document.
- `mapkit compose-world DOCUMENT.json SOURCE.json ATTACHMENTS.json NEW.json` applies
  a v1 authored overlay (or JSON `null`) and the shared surface attachments to a
  fresh document. Validation failures and existing destinations preserve all inputs.
- `mapkit audit-roads PACKAGE` samples generated collision every 0.5 m on five
  width lines against the designed surface (at most 1 cm deviation), checks
  junction fans/overlap, duplicate terrain contacts, per-cell road-paint budgets
  (a road, or a road joined with its degree-two neighbours, over the 128-segment
  paint cap is an audit error) and the default-world 12% design grade.
- Every road's paint (centre path and edge-line exterior, designed or not) is
  compacted within 1 cm for display, with dash stations measured before
  compaction; compacted exterior chains keep their plan direction, which the
  shader's inward edge-line offset relies on. Collision keeps full curve sampling.
  Shader path/exterior arrays stay capped at 128 segments. A road still over the
  cap in a cell, after compaction, is drawn unpainted in that cell (Editor
  responses warn `road_paint_segments`), and a degree-two joined path over the cap
  falls back to the road's own path (warning `road_paint_join`); the cell still
  loads. Paint never fails a cell.

Detailed real driving on designed roads is user verification.
