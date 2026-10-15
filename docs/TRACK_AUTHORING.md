# Track source, compiler and manual authoring

Owner of the authored track source, compiler API and manual authoring rules.
Seeded generation settings, categories, catalogue and geometry are owned by
[ASSEMBLED_TRACKS](ASSEMBLED_TRACKS.md). Overlays on existing maps follow
[ROAD_DESIGN](ROAD_DESIGN.md).

## Instances and ports

- Instances have stable IDs, YXZ Euler millidegrees, centimetre positions, widths
  and optional cubic Bézier controls (3n+1, up to 193). There is no grid or 90°
  placement restriction.
- Curve frames parallel-transport through vertical tangents with eased endpoint
  roll; degenerate tangents are rejected.
- New roads, loop ribbons and ports require at least 400 cm. Pipe bores use
  400/600 cm with a 400 cm default; the catalogue publishes every minimum, size
  and default for consumers, including `max_port_width_cm` per entry (1800 cm
  for ordinary roads/transitions). Fixed 18 m bodies use the ultra-wide presets.
  Stored v1 validation is separate from compilation.
  Project opening materializes a preview from validated saved geometry; it does not
  recompile or resize old sources. Explicit execution export uses current authoring
  verification and reports narrow instances and fields.
- Port snapping aligns position, tangent, normal and width, including named pipe
  portal drops along the road normal. Other disconnected ports are draft issues.

## Source and compiler API

- `assembled_track::authoring::Source` owns instances, port connections, named
  paths (base first), ordered checkpoints, actions and obstacle attachments.
  `from_assembly` keeps the original normalized seed settings; `compile` creates an
  Assembly and reports draft issues.
- `mapkit_package::assembled_track::compile_source` creates the document and, when
  executable, binds a public course to its content.
- Seeded assemblies have `authoring: null` (composed seeded graphs also store
  `seed_source`); verification regenerates the exact seed. Manual assemblies have
  `authoring: Source`, `seed_source: null`; verification recompiles and compares
  geometry, graph, actions and derived products. Seed provenance never makes edited
  geometry seed-verified. Player completion proof is a separate course concern.
- Source limits (shared with the stored-assembly reader): 128 checkpoints, 32
  paths, 64 actions, 128 attachments, 32 static structures, 1024 connections.
  Effective driving courses separately allow 64 gates.
- `authoring::compile` keeps one successful source/result pair per thread; exact
  source equality reuses it, results are clones, and failed/cancelled work never
  seeds it. `Assembly::validate` and package verification still compare all derived
  fields; there is no trust flag or validation skip.
- Compiled products must not be edited directly.

## Drafts and export

- Drafts may contain disconnected roads, missing paths/checkpoints or clearance
  issues; they save and recover as projects.
- Execution export, including indexed packages, rejects `geometry_issues` with
  `E_TRACK_GEOMETRY`. Race-track export also rejects unresolved route/start/
  checkpoint issues with `E_TRACK_DRAFT`; free-roam maps need no completed route.
- Invalid references, malformed values and resource overflows are hard errors.
  Manual authoring has no occurrence or target-time quota.

## Checkpoints and routes

- Checkpoints must belong to every route in the same order. Alternative paths share
  start and finish; branch-only checkpoints are rejected.
- The ground start checkpoint needs a supported surface.
- Generated routes place an ordinary checkpoint every four pieces (straight,
  gentle corner and simple grade sections count; narrow, obstacle and special
  pieces need both boundaries). Splits/merges use shared samples around the whole
  branch, coincident seams merge, and the exact start and plaza finish are kept.
  More than 64 shared checkpoints retries or fails with `E_TRACK_CHECKPOINT_LIMIT`.
  Manual lists bypass this policy.
- Read-only `suggest_track_course(source_json)` returns ordered `checkpoints`, resolved
  `gates`, `applicable` and located `diagnostics` (`code`, `message`, `blocking`,
  nullable `piece`/`position_cm`). It uses the declared base/alternative paths, with
  manual start/finish endpoints and the same intermediate selection policy. It
  reserves sprint endpoints, omits overlapping intermediates and reports missing
  connections, undeclared branches, geometry errors and the 64-gate limit. Editor
  must explicitly adopt the proposal through its document command; querying never
  replaces existing checkpoints or grants completion proof.
- The shared first-wins checkpoint view feeds generation admission, course
  construction and Runtime progress metadata; raw authoring arrays are unchanged.
- `shortcut_source()` composes a wide lower zigzag, narrow elevated straight, jump
  approach, flight path and common merge; the 4 m elevated lane and 8 m base road
  share progress sections.

## Jumps and panels

- A `jump_panel` launches over continuous road; `flight_curve` models a gap to
  another height. Explicit landings need reachable height/range and at least 6 m
  of supported runway.
- Actions require `panel_width_percent` (25/50/75/100) and `panel_alignment`
  (`left`/`center`/`right`); new Editor and generated panels use 50/center. Width
  uses the anchor's road width after a 25 cm margin per side; chains apply it per
  station. Unsupported flight/special anchors and over-limit geometry fail.
  Panel fields are inert for air rings and manual flight.
- Panels clip the production road triangles, including connected seams. The trigger
  top lies on the road; a 4 cm underside prism is trigger data only (no solid or
  collider). Rendering keeps only top faces, offset 0.5 mm along the world normal.
  `gimmick_geometry.part_triangles` serves Editor and Client. Speed panels use orange
  chevrons, jump panels cyan arrows/bars.
- The assembly fingerprint includes the shared `panels.rs` fitter.
- Air rings: [SPECIAL_DRIVING](SPECIAL_DRIVING.md#air-ring-defaults).

## Independent grind lines and RC attachments

- Optional `grind_lines` on map and source. A line has a unique UTF-8 ID (1–32
  bytes), two straight points or 3n+1 cubic controls (4–49), a unit up vector in
  millionths, capture width 5–100 cm and explicit start/end links (`line`, `end`).
  Endpoints must coincide within 2 cm.
- Limits: 256 lines, 16 links per endpoint, 32,000 resolved samples per map.
  Resolved samples carry a transported up frame, ≤35 cm spacing and
  `capture_height_cm=4`; the striped cap and Runtime capture use that plane.
- Lines have no collision and no interaction is inferred from collider names or
  materials. The supporting rail/fence stays ordinary collision.
- Seed generation adds rail and suitable straight-fence lines within existing
  budgets. Conversion to authored source materializes them. `attachment_lines`
  returns a rail placement's line; afterwards editing/deleting the line never
  changes the collider or regenerates the interaction.
- Hashes, indexed audits, archives, cell costs, driving windows, preview and chunk
  rendering include lines. Errors: `E_GRIND_SOURCE`, `E_GRIND_CONNECTION`,
  `E_GRIND_BUDGET`.
- RC attachments: `ramp_low` (25 cm), `ramp_standard` (60 cm), `ramp_triple`
  (three 25 cm ramps), left/right 2 m radius quarterpipes and a rail with a 40 cm
  top. They are static convex geometry with no automatic jump effect.

## Manual airborne links and static authored structures

- `kind: "manual_flight"` declares a supported takeoff (`piece`, `sample`), a
  supported `landing` reference and a `height_cm` envelope. It emits no automatic
  launch/boost effect. Authored flight links need either a jump approach or this
  declaration.
- References, supported starts, ≥6 m landing runway, width, height/range, ordered
  connections, clearance and graph/action/sample budgets still apply. This is
  geometric admission, never a completion claim.
- Optional `Source.structures`: at most 32 static gimmicks with IDs beginning
  `authored-` and no effect. Compilation, rendering, collision, reconstruction and
  both containers include them, charged to existing memory budgets. A solid beam is
  authored here; its `grind_lines` interaction neither creates collision nor
  regenerates a deleted beam.

## Stage progress

`MapKitWorkToken` exposes an atomic snapshot: job ID, revision, stage, completed
count, nullable total and unit. Candidate search has no total; validation counts
checks, compression and atomic writes count bytes, preview counts objects.
Percentages describe only the current stage; saving is not consumer readiness.
`track_job.gd` emits progress with its request ID and drops cancelled, superseded
and duplicate revisions. `work_progress` offers a 72 px stage indicator and a
28 px text-free indeterminate one; reduced motion keeps a static arc. Consumers
translate stage keys.

## Preview

- `godot/track_authoring_preview.gd`: data-only `prepare`, main-thread `apply` and
  `select`; `create` is the convenience entry. Preparation uses the production
  tessellator and gimmick geometry. Unchanged gimmick rows are reused only on exact
  source and ownership equality; scene application keeps identical nodes and
  changes selection materials without recompiling. Owner metadata groups roads,
  actions and obstacles for drag ghosts; floor/supports are final-commit geometry.
- `apply_draft` shows pending edits: rigid changes transform existing meshes,
  additions/shape changes use path lines, changed attachments and grind lines use
  markers/control-polygon guides. Obsolete junction/support/ground geometry hides
  until preparation is ready; `clear_draft`/`apply` restore poses.
- Godot also exposes `track_authoring_source`, `compile_track_source`,
  `track_instance`, `snap_track_instance`, `track_shortcut_source` and
  `track_preview`; `verify_track` reports `seed_verified` and `progress_checkpoints`.

Editor timing targets are owned by the
[MapEditor editing performance document](https://github.com/jujinkim/MiniEarthure-MapEditor/blob/main/docs/TRACK_EDIT_PERFORMANCE.md).

## CLI

`mapkit compile-track SOURCE.json NEW.memap`, `generate-track SETTINGS.json
NEW.memap`, `track-catalogue`, `verify-track PACKAGE`. Destinations must be new.

Validation: focused `mapkit-core` authoring/grind/checkpoint tests and
`mapkit-package` assembled-track roundtrip/tampering tests. Driving and editing
acceptance is user verification.
