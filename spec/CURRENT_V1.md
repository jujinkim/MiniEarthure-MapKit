# Current map contract

All own formats and protocols, including `.memap`, are version 1. Readers accept
exactly 1, with no historical loader or automatic converter. Original files are
preserved; active packages are exported anew from authored source. Build
fingerprints invalidate disposable caches; world hashes separate changed content.
This page lists the domain invariants and links each feature's owner.

| Area | Owner |
| --- | --- |
| Container, sharing, preview | [FORMAT](FORMAT.md) |
| Regional storage | [REGIONAL_SOURCE](REGIONAL_SOURCE.md) |
| Errors | [ERRORS](ERRORS.md) |
| Designed roads, composite worlds | [ROAD_DESIGN](../docs/ROAD_DESIGN.md) |
| Track source, compiler, manual authoring | [TRACK_AUTHORING](../docs/TRACK_AUTHORING.md) |
| Seeded generation, track geometry | [ASSEMBLED_TRACKS](../docs/ASSEMBLED_TRACKS.md) |
| Special structures, air rings, pipes | [SPECIAL_DRIVING](../docs/SPECIAL_DRIVING.md) |
| Water | [WATER](../docs/WATER.md) |
| Lights, weather | [ENVIRONMENT](../docs/ENVIRONMENT.md) |
| Assets, placement yaw | [WORLD_ASSETS](WORLD_ASSETS.md) |
| Distant meshes | [DISTANT_RENDERING](../docs/DISTANT_RENDERING.md) |

## Facility points of interest

- Optional `MapDocument.pois`: at most 20,000 informational facilities. Each has a
  unique `id`, nonempty `name` (≤256 characters), `category` (≤128), local-centimetre
  `[x, y]` `position` inside map bounds and a `source` attribution (source,
  license, notice).
- Heights are unspecified. POIs create no collision, road connection, spawn
  authority or inferred building.
- POIs survive source/package/regional round trips and participate in canonical
  hashing and retained-memory accounting.
- `godot/poi_renderer.gd` draws shared screen-space pins/labels, suppresses
  overlaps and shows at most 128 entries. Precision does not imply surveyed accuracy.

## In-memory authoring snapshots

- `mapkit_package::working::WorkingSnapshot` owns copy-on-write height arrays and a
  bounded `Resources` provider (memory bytes or confined project paths). Godot
  `MapKitWorkingSnapshot` exposes configuration/fork, queries, brush lifecycle,
  sparse delta history, water operations, memory validation and packed cell
  generation. None of these write PNGs or project/recovery files.
- Explicit `materialize` validates and encodes only modified tiles as PNG16;
  unmodified PNGs are never re-encoded. A modified cell whose relief fits
  655.35 m uses `step_cm` 1 (byte-identical). Larger relief uses the smallest
  power-of-two step ≤64, flooring heights onto a lattice through
  `terrain_base_cm`. A sample shared with an unmodified cell or flat terrain keeps
  that cell's stored (decoded) value exactly; a sample shared only by modified
  cells takes their coarsest step. If a kept shared sample is off the required
  lattice, Save is refused (`E_HEIGHTMAP`). Warning `heightmap_step` when a step
  is above 1 cm. Working memory keeps exact heights;
  consumers own atomic publication and saved baselines. Disk-source hashes detect
  external changes; forks isolate concurrent preview/output workers. Source
  descriptors stay stable across publication, so earlier imports remain undoable
  after sculpting and saving. Save As carries referenced source payloads.
- Cell admission prepares an immutable document once per batch of 1–256 unique
  cells and discards each generated chunk. Materialization uses successive batches
  for all modified cells. Later edits need a new preparation.
- Brushes (raise/lower, flatten, smooth) follow existing spacing (new terrain
  200 cm), interpolate paths and update shared seam owners.
- Budget profiles ([ERRORS](ERRORS.md#budget-profiles)): every
  `MapKitWorkingSnapshot` call runs under Authoring. Runtime values (Authoring
  guard): changed samples 1M (8M); working heights, resources, saved payloads
  64 MiB (1 GiB); one resource file 64 MiB (128 MiB); preview work 256 MiB
  (2 GiB); preview payload 16 MiB (128 MiB); preview display 128 MiB (1 GiB).
  `generate_packed`, `estimate` and `validate_cells` take an optional
  `max_triangles` (default 500,000, ≤2,000,000); Save admission uses 2,000,000.
  A generated cell above 500,000 triangles warns `cell_triangles`. A cell
  estimate is an upper bound of the generated output (generation fails closed
  above it): cells without an assembled track (heightmap cells included, tile by
  tile) pre-run their ground arrangement without triangulating and bound
  sidewalks from that ground count
  and the nearby sidewalk segments; trees count unclipped; display bytes for
  road markings cover the roads with faces near the cell and their two-arm
  chain continuations, not the whole document.
- Raw height and road-fitted surface queries are separate; display, flood and
  physics share the fitting operation.
- Water editing floods connected sub-level terrain at a fixed clicked height; flat
  clicks do nothing and open map edges keep level. Terrain changes update
  shorelines/islands, merge/split lakes, and higher overlaps delete lower
  components. Complex lakes split into ordinary v1 bodies (see [WATER](../docs/WATER.md)
  limits) while connected equal-level pieces stay one editor operation. Search and
  union budgets plus cancellation bound the work.

## Generated caches and bounded input

- One MKCELL01 cache layout: fixed header, triangle/object arrays, prism count and
  records, convex count and records, then u32 length + canonical gimmick JSON, then
  u32 length + canonical water-cell JSON. Empty arrays/counts are always present.
- Cache keys include BUILD_FINGERPRINT, generated version, world identity and cell.
  Cargo derives the fingerprint from sorted relative source/schema/dependency paths
  and UTF-8 contents (LF endings, length-delimited). Absolute paths, platform,
  timestamps and build output never enter it. Portable geometry vectors use a fixed
  test key.
- File-backed `read_with_budget(path, allowance)` bounds compressed input before
  allocation and checks the validation peak before retaining inflated payloads.
  Cost inspection streams JSON through a 64 KiB buffer; string tails beyond 256
  encoded bytes are charged at a reduced per-byte rate.
- Godot `open_project(path)` opens the user's project directory in memory under
  Authoring (`Package::from_project`, no ZIP round trip): the same document, files,
  manifest and `world_content_hash` as a stored-preview package, with
  `package_sha256` empty and `package_bytes` 0 (no package exists until
  `export_project`, which stays Runtime/v1-hard). Package calls on that bridge run
  under Authoring; `estimate_chunk_authoring` / `generate_chunk_packed_authoring`
  take `max_triangles`. `validate_document_authoring`, `source_preview*` and
  `preview_document*` (optional `max_triangles`), document-editing calls and
  `MapKitRegionReader.audit_authoring(allowance, generation, overview_bytes)` are
  Authoring too; seeded/suggested course and track candidates are always tested
  under Runtime, so they match the game's. `*_authoring` bridge calls require an
  `open_project` bridge. Reader calls (`open_package*`, `audit`, `load_region`, Runtime
  generation at 500,000 triangles) are unchanged.
- Godot `open_package_budgeted(path, allowance)` and `unpack_source(destination)`:
  restore uses the validated immutable package, keeps every original payload and
  requires a new directory. Failed opens clear prior native state.
- The Godot bridge caches cell cost/archive descriptors on the validated source
  (≤1,024 entries, 4.25 MiB shared with the core estimates); with the prepared
  source's 2 MiB junction-plan cache the 6.25 MiB capacity is in both container
  read allowances. Asset hashes/costs are computed once per source; packed display bytes
  are created lazily on the presentation path only. Headless paths create no
  display bytes or GPU resources.
- `cargo run -p mapkit-cli --example menu_preview -- INPUT.memap OUTPUT.json` writes
  a roads-and-bounds overview tied to the package SHA-256. It grants no spawn
  authority; consumers validate the package and selected surface before entry. The
  `cache_population` example creates disposable cell archives for cache tests.

## Placement validation

- Authored placement uses the convex hull of the complete transformed collision
  footprint. Placement contact is forbidden.
- Bridge/elevated corridors admit a support only when its whole collision top is
  at least 1 cm below every interpolated deck segment over the footprint, and a
  building only when its roof (including a gable rise) is at least 50 cm below the
  deck wherever a segment's corridor and safety strip cover the footprint.
  Ground/tunnel/underpass corridors stay excluded.
- Surface-area overlap uses exact integer triangle separation: touching edges and
  disjoint concave polygons pass; any positive-area overlap fails. Pieces and
  triangles whose bounds only touch (zero-area contact) cannot overlap and are
  neither tested nor charged.
- These checks charge the existing placement work allowance.

## City ground, barriers, building parts and play area

All four are optional; absent/empty values are omitted from canonical JSON, so
earlier documents keep their bytes, world hashes and generated geometry.

- `MapDocument.ground_surface` (`asphalt|concrete|dirt|gravel|grass`, absent =
  grass; normalization drops an explicit `grass`) labels unpainted terrain faces
  (`terrain`, contact class 1): render material, contact surface and distant
  colour. Roads and `surface_areas` keep precedence; the environment
  `ground_color` tint applies to grass only.
- `MapDocument.barriers`: `{id, kind: hedge|fence|wall, line: [[x,y],...],
  height_cm 10..1000, thickness_cm 5..100}`; 2..4096 in-bounds points, consecutive
  points distinct, a repeated first point closes a perimeter. IDs share the object
  namespace; each barrier counts as one object and its points as input vertices.
  - Gates: a road segment cuts a capsule of half width + sidewalk (explicit or
    implicit) + 50 cm, a road end a disc of its full end width + 50 cm, and
    assembled-track paths their lateral half width + 50 cm, each widened by the
    barrier's half thickness. Ground/underpass corridors always cut. Bridge,
    elevated, tunnel and track corridors cut only where their band (surface
    −100 cm to +250 cm, tunnels + clearance; tracks span both banked ribbon edges
    and, inverted, also below) meets the barrier's base-to-top span at the crossing. Kept spans at polyline joints extend half a thickness so
    corners close. Buildings, placements and vegetation ignore barriers.
  - Geometry: each span is cut at cell seams, every 8 m along its source segment
    and, in a heightmap cell, at grid lines and tile diagonals; each piece is a
    convex parallelepiped up to `height_cm` above the source terrain at its
    centreline ends, with a base 25 cm below the lowest terrain under each end's
    corners and lowered below every source grid vertex under the footprint and
    every grid line/diagonal crease its long edges cross. Faces
    are obstacles (hedge `grass`, fence/wall `concrete`, never spawnable), also
    `asset_convexes` and occupancy solids under the barrier ID. The cell holding a
    piece's centre point owns it unclipped (within the 2 m query halo). Godot
    `surface_probe` reports `blocked_by_barrier` from the owner cells within 52 cm.
  - Only corridors near a barrier are built; pieces are visited, never collected,
    and a cell estimate stops counting past its triangle/solid limit. Budgets
    `barrier_pieces` (an upper bound of all cut pieces) and `barrier_work`
    (corridor gating plus structure tests per bounded piece)
    ([ERRORS](ERRORS.md#budget-profiles)). Barriers never disable regional
    pruning; a region keeps the barriers its line bounds reach. Track environment
    export checks test driving space and supports exactly against the composed
    (track-gated) barrier pieces.
  - Display key `barrier:<kind>` (hedge leaf, fence wood, wall stone colours).
- `Building.parts` (≤255): `{footprint, holes?, base_cm?, height_cm?}`; absent values
  inherit the record's and normalization drops values equal to them. The record
  stays one object (ID, material, usage, roof, entrances, object count); the
  footprint is the first solid. Each solid follows the building rules (ring,
  courtyard, gable rectangle, height, base) and its vertices count. Footprint and
  part interiors (outside courtyards) are pairwise disjoint; touching is allowed
  (shared walls and pairs within one solid are neither tested nor charged) and no stacking. Every consumer treats each solid as a building: prisms/roofs/
  `building_prisms`/occupancy, road corridor and deck checks, building/placement/
  vegetation/repetition clearance (indexed per solid), sidewalk exclusion,
  implicit sidewalk widths, regional retention, overview (one entry per solid
  under the record ID), distant faces, shortcut clearance and surface probes.
- `MapDocument.play_area` (optional, absent = the whole `bounds` rectangle): up to
  16 simple outer rings `[[x,y],...]` of 3..512 in-bounds vertices (`E_PLAY_AREA`
  otherwise). It is world metadata: it changes no generated cell, and the regional
  index and `open_index` carry it. Consumers stop vehicles whose footprint leaves
  every ring (as at the rectangle) and draw the edge along the rings.

## Courses and completion references

- `MapDocument.courses`: up to 64 course documents with map ID, name,
  driving-content hash, ordered 3D sphere/upward-hemisphere checkpoints (1–1000 m
  radius), circuit/sprint mode, ground/air start and horizontal direction. Circuit
  finish reuses checkpoint zero. Lap count is a hosting choice.
- A course stores 0–64 checkpoints so incomplete maps stay readable. The effective
  list keeps the first non-overlapping gate in source order; sealed courses adopt it,
  and package reads expose a derived effective course without changing source bytes.
  If gates are removed the course ID is recalculated and prior completion evidence
  is omitted. Racing requires at least two effective gates. Authoring rejects new
  overlaps. `CourseBody.effective_indices`, `Course.effective` and the Godot APIs
  share this rule.
- Continuous capsule entry geometry is MapKit-owned; consumers own authoritative
  progress and resets. Surface labels are hints, not completion constraints.
- Completion references name bounded opaque records by hash, size and
  `course-validation/<sha256>.mevalidation`. MapKit verifies bytes and references but
  never certifies completion. Courses and records join package integrity but not
  driving-content hashes. Stale references are preserved so consumers can show
  "revalidation required".
- `MapDocument.free_roam` is a required boolean: false for manual/seeded documents,
  true for external geographic imports. It enters both content hashes, regional
  metadata and inspection. `reseal_track_document` edits it for generated sources
  after verifying exact current geometry.
- Optional `MapDocument.backdrop` names display-only scenery for a standalone track
  map: `rc_arena` (also the meaning of absence) or one of the seven default-world
  themes (`village`, `neon_harbor`, `deep_forest`, `red_canyon`, `snow_mountain`,
  `machine_factory`, `sky_park`). Ordinary maps and terrain overlays reject it. It
  joins package integrity but neither content hash, so sealed courses stay valid.

## Declarative driving structures

- `MapDocument.gimmicks`: at most 128 stable IDs, each with centimetre position,
  millidegree Euler YXZ rotation, per-mille scale, 1–32 convex parts, surface and
  RGBA colour. Motion is `static`, cosine ping-pong `translate`, continuous
  `rotate`, `boost` or `launch` with period/phase, displacement/impulse, axis and
  per-vehicle cooldown. No executable scripts.
- Target speed, jump height, air rings and parametric loop/cylinder/swept-cylinder
  tracks are owned by [SPECIAL_DRIVING](../docs/SPECIAL_DRIVING.md).
- `curved_faces`: at most 2,048 unique `[part, triangle]` pairs on static convex
  structures, not combined with a parametric track. Those triangles become the
  shared `track_mesh.inner`; other faces form the shell. Original parts stay the
  source and occupancy geometry; 128 bytes per selected face are charged. No name
  or material inference grants a curved role.
- Safety bounds must enclose full transformed motion and landing area and stay in
  the map. Cells include every intersecting definition; consumers deduplicate by ID
  and keep a global motion phase across reloads. `driving_window` extends the 3×3
  set to complete safety rectangles touching it.
- Cost: 16 KiB plus 32 KiB per convex part per intersecting object/cell. Occupancy
  includes static/translation bounds and full rotation sweeps.
- `godot/gimmick_geometry.gd` is pure geometry/pose/contact-velocity math. The
  consumer supplies authority time, reset and activation; MapKit never starts a
  simulation clock.

## Assembled tracks

- Optional `assembled_track` selects the modular track generator; a terrain-
  integrated assembly is an overlay ([ROAD_DESIGN](../docs/ROAD_DESIGN.md)).
- Seeded settings, categories, short/medium/long length, catalogue and saved-package
  verification: [ASSEMBLED_TRACKS](../docs/ASSEMBLED_TRACKS.md).
- `Assembly.supports` and `Source.grounded_supports` are required:
  [grounded supports](../docs/TRACK_GEOMETRY.md#grounded-supports).
- Sprint finish plaza (`Assembly.finish_plaza`):
  [TRACK_GEOMETRY](../docs/TRACK_GEOMETRY.md#sprint-finish-plaza).
- Source/compiler API, drafts and checkpoints: [TRACK_AUTHORING](../docs/TRACK_AUTHORING.md).
- Optional assembled Source `jump_shortcuts`:
  [rings, platforms, openings and recovery](../docs/JUMP_SHORTCUTS.md).
- `grind_lines` and RC attachments:
  [line contract](../docs/TRACK_AUTHORING.md#independent-grind-lines-and-rc-attachments)
  (`E_GRIND_SOURCE`, `E_GRIND_CONNECTION`, `E_GRIND_BUDGET`).
- `manual_flight` and static `Source.structures`:
  [manual airborne links](../docs/TRACK_AUTHORING.md#manual-airborne-links-and-static-authored-structures).
- Pipe minimum radius, bores and control points: [SPECIAL_DRIVING](../docs/SPECIAL_DRIVING.md#pipe-dimensions)
  (`E_PIPE_DIMENSIONS`).

## Rendering inputs

- `update_environment(..., immediate)` applies road wetness/snow without blending
  at a race boundary; it consumes resolved state and never advances time.
- Dynamic lights use consumer-supplied generic groups
  ([ENVIRONMENT](../docs/ENVIRONMENT.md#dynamic-lights)); no vehicle data enters maps.
- Road snow retention and contact classes: [ROAD_CONTACT](../docs/ROAD_CONTACT.md).
- The miniature kit and climate ground palette are renderer/source assets; they
  do not change surface identity or physical material behaviour.
