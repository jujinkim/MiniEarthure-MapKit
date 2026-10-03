# Category generation and free track authoring — current v1

This 2026-09-30 decision replaces the earlier mandatory-family, obstacle-minimum,
overrun-acceptance and 8m/90° placement rules in ASSEMBLED_TRACKS.md. Historical
files and Git history remain intact. There are no old readers or converters.

Generation settings use `categories: ["driving", "gimmick", "action"]`, all enabled
by default. Empty/unknown selections are rejected. Categories provide candidates;
no piece family or obstacle has a minimum occurrence. Start, finish, tapered width
transitions, required approaches and closure are exceptions. Identical gimmick
families have at most two consecutive occurrences, including a circuit seam.

Both circuit (one lap) and sprint (start to finish) offer 60/90/120 seconds.
The base route's length at 900cm/s must be within ±10%. Alternative route estimates
are separate. Search extends, closes and backtracks within 24 candidates,
512 pieces, 32000 samples and existing cancellation/package/cell/memory budgets.
A rejected search returns `E_TRACK_DURATION` with requested/closest time and a
reason; callers retain their current map. No exceeding candidate is published.
These are reference-speed estimates, not vehicle performance certification.

## Geometry and catalogue

Ordinary width choices are 200/400/600/800/1200cm with eased entry/exit widths.
The public catalogue reports category, supported dimensions and entry/exit ports.
Dedicated loops, halfpipes and the compact overpass use 400cm; authored pipe bores
support 100/200/300/400/600cm. Generated pipes choose 100/200/300cm. Internal
profiles remain catalogue-owned.

For width w in metres, gentle 45°/90° radius is max(16,4w), right-angle 90° radius
max(4,w/2+2), and sharp 135°/180° radius max(3,w/2+1). Width transitions use the
largest supported cross-section when choosing bend radius. Helices offer
90°/180°/360°, either direction and up/down; radius max(8,w/2+2), rises 2/4/8m,
and eased entry/exit grades. Frames, road surfaces, wall meshes, occupancy,
overview and AI path derive from the same quantized samples.

Instances have stable IDs, full YXZ Euler millidegrees, centimetre positions,
widths and optional cubic Bézier control points (3n+1, up to 193). There is no
8m grid or 90° placement restriction. Curve frames parallel-transport through
vertical tangents, with eased endpoint roll. Degenerate tangents are rejected.
Port snapping aligns position, tangent, normal and width, including named pipe
portal drops along the road normal. Other disconnected ports are draft issues.

## Source and compiler API

`assembled_track::authoring::Source` owns instances, port connections, named
paths (base first), common ordered checkpoints, actions and obstacle attachments.
`from_assembly` retains the original normalized seed settings. `compile` creates
an Assembly and reports draft issues. `mapkit_package::assembled_track::compile_source`
creates the document and, when executable, binds a public course to its content.

Seeded assemblies have `authoring: null`; composed seeded graphs also store
`seed_source`. Verification regenerates the exact seed and settings. Manual
assemblies have `authoring: Source` and `seed_source: null`; verification compiles
that source and compares geometry, graph, actions and other derived products.
Original seed provenance does not make edited geometry seed-verified. The source,
compiled graph, frame data, actions and schema enter fingerprints/hashes; all
format and protocol numbers remain 1. Explicit player completion proof stays a
separate course concern and does not participate in source regeneration.

Drafts may contain disconnected roads, missing paths/checkpoints or clearance
issues. They can be saved/recovered as projects. Execution export, including
indexed packages, rejects them with `E_TRACK_DRAFT`. Invalid references, malformed
values and resource overflows are hard errors. Manual authoring has no occurrence
or target-time quota. Structural graph/sample/cell/memory limits remain binding.

Checkpoints must belong to every route in the same order. Alternative paths share
start and finish; branch-only checkpoints are rejected. Jump actions are separate
from gaps: a `jump_panel` can launch over continuous road; `flight_curve` models
a gap to another height. Explicit landings require reachable height/range and at
least 6m of supported runway. Authored flight links require an explicit jump
approach/landing declaration; the ground start checkpoint needs a supported surface. The compiler checks nonlocal road clearance and
ribbon self-intersections. This is bounded geometric validation; player driving
and landing acceptance remain separate.

`shortcut_source()` composes a wide lower zigzag, narrow elevated straight,
jump approach, flight path and common merge from independent road/action records.
The same graph is a seeded candidate. Its 2m elevated lane and 8m base road share
progress sections without forcing shortcut drivers through the lower zigzag.

CLI: `mapkit compile-track SOURCE.json NEW.memap`, `generate-track SETTINGS.json
NEW.memap`, `track-catalogue`, `verify-track PACKAGE`. Destinations must be new.
Godot exposes `track_authoring_source`, `compile_track_source`, `track_instance`,
`snap_track_instance`, `track_shortcut_source`, and `track_preview`. Preview uses
the production tessellator; `verify_track` separately exposes `seed_verified` and
`progress_checkpoints` for consumers. Compiled products must not be directly edited.

Focused regressions cover all widths, helices, full rotations/portals, vertical
curve frames, category combinations/durations, cancellation, composed branches,
source/action tampering, save roundtrip, draft export and retained memory.
Detailed consumer editing and driving remain user acceptance.

Automated delivery: 49 relevant Rust tests across core geometry/authoring, package,
schema and memory accounting passed on macOS arm64; the category matrix covers
42 mode/target/category combinations at seed 42. CLI and Godot extension builds
passed. Current-source consumer binding checks are recorded by the integration root.


## Compiler and preview preparation — 2026-10-01

Interactive consumers can prepare source edits and preview data in an isolated
worker. `authoring::compile` retains one successful source/result pair per thread,
bounded by the existing source/assembly limits. Exact source equality reuses that
compiler-owned result. Returned values are clones; caller mutation, failed work
and cancelled work cannot seed or modify the cache. Cancellation is checked even
on a hit. Threads never share this mutable cache and it is released on thread exit.

`Assembly::validate` and general document/package verification still compare all
derived fields against deterministic compiler output and check products/courses.
There is no caller-supplied trust flag or validation-skip option. The preparation
unit test verifies one actual compile across document construction and repeated
validation, equality with the uncached compiler (including graph/issues/supports),
tampering rejection, cache isolation, changed input and cancellation. All own
formats remain v1; implementation fingerprints change with source as usual.

`godot/track_authoring_preview.gd` now exposes data-only `prepare` and main-thread
`apply`/`select` paths; `create` remains the convenience entry point. Preparation
uses the same production tessellator and gimmick geometry, with packed vertices,
normals, poses and object signatures. Optional prior preparation is immutable;
unchanged gimmick rows are reused only on exact source and ownership equality.
Scene application replaces changed objects, retains identical nodes/resources,
and changes selection materials without recompiling. Object owner metadata groups
roads, attached actions and obstacles for consumer drag ghosts; floor/supports
are separate final-commit geometry. The worker cancellation token is optional.

Scoped macOS arm64 / Godot 4.7.2 validation: native build, one new compiler-cache
unit test, eight completed track-authoring tests and package tests
`authored_source_roundtrip_draft_export_and_tampering`,
`finish_plaza_and_editable_free_roam_keep_exact_source_validation`, and
`source_mode_and_course_progress_remain_distinct` passed. The long 42-case
category/duration matrix was deliberately stopped, so this is not a full suite
pass. Initial unfiltered test discovery also found an unrelated existing
`tests/water.rs:22` initializer missing `contact_class` and `snow_retention_percent`;
it is outside this patch and remains a known unrelated test compilation failure.
[Native logs](validation/track-preparation-2026-10-01/) retain the actual scope.

MapEditor verifies fresh/reused preview equivalence, zero whole-preview/mesh builds
for selection and existing-piece drag, async history/save/recovery and cancellation.
Its public [editing report](https://github.com/jujinkim/MiniEarthure-MapEditor/blob/main/docs/TRACK_EDIT_PERFORMANCE.md)
records measurements and the 49-piece seed-derived 500 ms commit target miss.
Detailed interactive/platform acceptance remains user verification.


## Piece-local wall preparation — 2026-10-03

Connected-road triangles and their five clipping planes are now prepared once per
piece traversal. The opposite branch is prepared once per path. Quantized ribbon
edges, lazy outward offsets and the rotation basis are reused by adjacent wall
sections. These are private temporary values, discarded after the piece/path;
there is no new request-spanning cache or public API. Rendering, collision,
grounding and cost estimation still call the same tessellator.

Triangle traversal, support arithmetic, plane order, interval sorting, tolerances,
rounding and emitted triangle/solid order are unchanged. Cancellation is checked
during preparation and every 64 clipping triangles, in addition to existing
segment checks. Generation workspace now reserves the plane capacity (including
skipped triangles), interval/sort scratch and prepared edge/offset arrays. No
budget limit or validation requirement is relaxed; all own formats remain v1.

Scoped macOS arm64 validation: 20 core library tests (walls, junctions, grounding,
compiler-cache equivalence/tamper/cancellation), ten `track_authoring` tests
(excluding duration/category searches), five affected package tests and optimized
debug CLI/Godot native builds pass. The new pre-change golden covers 40 synthetic
shape/orientation/neighbor combinations and **115,902** ordered surface/solid
records, SHA256 `1a847ea6ef087f15ebb75dd7d72d225874bede0851b4ad4c8664c7ff3b8b617b`.
It matches exactly after preparation. Added allocation-capacity and cancellation
regressions pass, as do existing curved/tapered/grade-separated joins, loops,
branch walls, support caps/clearance, occupancy and generation budgets.

The affected package cases are `generated_walls_have_no_reverse_coplanar_duplicates`,
`cancellation_and_invalid_requests`, `venue_floor_is_collision_only_and_budgeted`,
`authored_source_roundtrip_draft_export_and_tampering` and
`manual_flight_and_static_shapes_survive_both_containers`. No full suite, category
matrix or platform acceptance was run. Consumer timing and native-load results
are in the public [Editor report](https://github.com/jujinkim/MiniEarthure-MapEditor/blob/main/docs/TRACK_EDIT_PERFORMANCE.md).
Build fingerprints change, so active bundled packages must be re-exported from
their preserved sources; no old loader or automatic conversion is introduced.

## Independent grind lines and RC attachments (2026-10-01)

Current v1 adds optional `grind_lines` to both the map and authored source. A line
has a unique UTF-8 ID (1–32 bytes), two straight points or 3n+1 cubic control points
(4–49), a unit up vector in millionths, capture width (5–100 cm), and explicit
start/end links (`line`, `end`). Endpoints must coincide within 2 cm. Maximums
are 256 lines, 16 links per endpoint and 32,000 resolved samples per map. Source
coordinates remain integer centimetres. Resolved samples carry a transported up
frame, at most 35 cm spacing, and `capture_height_cm=4`; the visible striped cap
and Runtime capture use that plane. Lines have no supporting collision of their
own and do not infer interaction from collider names or materials.

Source validation, canonical hashes, indexed audit ownership/scratch accounting,
generated archive payloads, cell costs, driving windows, shared preview and chunk
rendering include lines. The supporting rail/fence remains ordinary collision.
Seed generation explicitly adds rail and suitable straight-fence lines, subject
to the existing category/duration/attachment budgets. Conversion to authored
source materializes the interactions. `attachment_lines` returns the explicit
line for one rail placement; thereafter deleting or editing the line does not
change its supporting collider or silently regenerate the interaction.

RC attachments are `ramp_low` (25 cm), `ramp_standard` (60 cm), `ramp_triple`
(three 25 cm ramps), left/right 2 m radius quarterpipes, and a 6 m rail with a
60 cm top. They are static convex geometry with no automatic jump effect.
Ordinary road paths, including cubic links, share curvature/width/twist
refinement. Connected junctions clip only wall intervals inside the neighbor's
road ribbon at the same height. Rendering, collision, occupancy and preview use
the resulting common triangles; outside and grade-separated walls remain.

Focused checks: four `mapkit-core` grind tests, package grind roundtrip, schema
contract, the 12 assembled-track unit tests, authored source regressions and
indexed ownership/hash tests. The earlier frame-orthogonality regression was
fixed and the affected width/helix test passed. Full application driving and
platform acceptance remain user verification. All own versions remain 1;
source/schema fingerprints invalidate disposable caches, never user artifacts.


## Manual airborne links and static authored structures (2026-10-01)

A source action with `kind: "manual_flight"` declares a supported takeoff
(`piece`, `sample`), supported `landing` reference and `height_cm` envelope.
It emits no automatic launch/boost effect. This explicitly replaces the rule
that every flight link must have a `jump_panel`. Missing references, unsupported
starts, landing runway (at least 6 m), width, height/range, ordered connections,
clearance and existing graph/action/sample budgets still apply. This is geometric
admission, never a claim that a player has completed a course.

Optional `Source.structures` contains at most 32 explicit static Gimmicks whose
IDs begin `authored-`, with no effect. Shared compilation, rendering, collision,
exact reconstruction and both package containers include them. The complete
convex/mesh source and canonical hashing scratch are charged to existing memory
budgets. A solid beam is authored here; its independent `grind_lines` interaction
does not create collision or regenerate a deleted beam. No output editing is
needed. These additions retain current v1 and change source/schema fingerprints.

Scoped validation: manual-link/landing/budget/exact-shape unit test, retained
automatic-flight regression, manual source package tampering test, both-container
static-shape audit and published schema test. Consumer physics/UI acceptance is
separate.

## Modular circuit returns — 2026-10-02 replacement

Circuit closure uses bounded deterministic best-first search over existing
straight, right-angle, 45-degree, grade and height-return spiral modules.
These basic return connectors remain available regardless of random content
category, just as the previous free-curve closure was. Arbitrarily long cubic
returns are removed. A final cubic seam is allowed only between endpoints at
most 800cm apart, with control-polygon length at most 1600cm, forward-facing
end tangents and at most 23% vertical tangent component. Larger returns must
consist of catalogue pieces. Hand-authored free curves remain unrestricted by
this generator-only seam policy.

A closure stores at most 4096 nodes and 64 added modules, charges existing
piece/sample/time budgets, rejects self/other-road collisions, and checks
cancellation while expanding. Rejected/ cancelled searches do not mutate the
live candidate. Templates reuse only immutable local piece geometry; placement
and collision still use each candidate's actual frame and source. Equivalent
50cm pose bins retain the cheapest arrival with stable ordering.

The 24-candidate outer limit and requested-time ±10% rule remain. Closure is
attempted after another four reference seconds of growth. A candidate within
one percent of the target stops the deterministic search early. No wall-clock
cutoff, long-curve fallback, reduced category or manufactured success is used.
All own formats stay v1; current source fingerprints identify regenerated data.

## Stage progress — 2026-10-02

`MapKitWorkToken` exposes an atomic snapshot with job ID, revision, stage,
completed count, nullable total and unit. Candidate search has no total; attempts
never stand in for completion. Validation counts completed checks, compression
and atomic new-file writes count bytes, and authoring preview counts objects.
Percentages describe only the current stage. Saving a package is not consumer
readiness. `track_job.gd` emits progress with its request ID and drops cancelled,
superseded and duplicate revisions. The shared progress ring supports determinate,
indeterminate and reduced-motion display; consumers translate stage labels.

Validation: four Rust cancellation/progress units, generated-request cancellation,
written-byte/source-preservation unit, native/CLI build and isolated Godot
`work_progress_validator` pass. An initial unfiltered test invocation exposed the
existing unrelated `tests/water.rs` Triangle initializer missing contact fields;
focused units use `--lib` or an explicit integration target. Detailed consumer
interaction remains user verification; consumer wiring is delivered separately.

## Automatic checkpoint spacing — 2026-10-02 replacement

Final generated routes select an ordinary checkpoint every four pieces. Straight,
gentle corner and simple grade sections count toward spacing; narrow sections,
obstacles and other special pieces require both boundaries. Splits and merges
use shared samples before and after the entire branch. Selection is ordered on
every route, merges coincident seams, and preserves the exact start and plaza
finish. Manual authored lists bypass this automatic policy. Candidates needing
more than 64 shared checkpoints are retried within the existing search bound or
fail explicitly with E_TRACK_CHECKPOINT_LIMIT.

Three focused units cover spacing, hazards, deduplication, limits, branch order
and manual preservation; the generated alternate-route regression also passes.

Generation consumers now keep the same work token through optional prepared
authoring preview output. Geometry reports completed pieces; the common ring
also exposes stage keys for consumer localization. Editor's worker test observed
unknown search, counted work and preview stages before adoption. Client progress
filter units and Editor preview adoption pass with the current native build.


## Continuous draft preview — 2026-10-03

`track_authoring_preview.apply_draft` consumes current `track_instance` paths plus
the validated source represented by its existing nodes. Stable piece IDs map old
owners to current indices. Rigid changes retain meshes and transform their owned
visuals; additions/shape changes use lightweight path lines. Changed attachments
use path-based markers and changed independent grind lines use authored control
polygon guides. These are explicitly pending guides, not validated surfaces.
Obsolete junction/support/ground geometry is hidden until preparation is ready,
so previous and draft geometry do not overlap. `clear_draft`/`apply` restore poses
and retire guides before adopting prepared data; unchanged mesh nodes are reused.
The consumer defers final replacement during a drag or active text input.

`work_progress` retains its default 72 px stage indicator and also supports a
28 px, text-free indeterminate indicator. Reduced motion keeps a static arc.
The Editor shows it after 500 ms of continuous unconfirmed work without dim;
explicit Save/export uses its workspace dim and input lock.

No native source, dependency or ABI changed. Shared progress plus Editor held-worker,
real track/workbench/history/recovery/export/grind regressions pass; the rendered
Editor initial screen was inspected. [Consumer evidence](https://github.com/jujinkim/MiniEarthure-MapEditor/blob/main/docs/validation/continuous-edit-2026-10-03/README.md)
records the scope and fixed 49-piece measurement. Completion p95 532.745 ms still
misses the 500 ms goal. Detailed interactions/platform acceptance remain user work.

## Surface-conforming panels (02, 2026-10-04)

Seed and source actions now use one panel fitter over the production road
triangles, including connected seams. The clipped triangular prisms lift the
driving face 3cm along its actual normal (millimetre local quantization), with
4cm vertical thickness. Unsupported areas are clipped; no supporting triangles
or more than the existing 32 convex parts is an explicit error. The existing
per-part memory/occupancy accounting and all v1 numbers remain unchanged.
Panels use one shared procedural material: orange forward chevrons for speed,
cyan launch arrow/bars for jump. The pattern scales over the whole footprint
and follows its top faces in both prepared Editor meshes and Client visuals.
Air rings retain their existing rendering and geometry.

Three focused Rust units pass: flat lift, slope/curve/helix deterministic convex
geometry and limits, connected/tapered seam and stacked-road exclusion. Final
consumer native/load/render checks are grouped with the width/trigger delivery;
detailed editing, driving and device readability remain user verification.

## Partial-width panel contract (07, 2026-10-04)

Current-v1 `Action` requires `panel_width_percent` (25, 50, 75 or 100) and
`panel_alignment` (`left`, `center`, `right`). New Editor actions and generated
panels use 50/center. Width uses the anchor's available road width after the
existing 25cm margin on each side; chains apply the same policy at each station.
Actual supported triangles clip the footprint at road ends. Unsupported flight/
special-surface anchors and over-limit geometry fail explicitly. Air rings and
manual-flight actions retain their geometry/effects; their panel fields are inert.
No missing-field fallback, historical reader or converter is added. Schema and
source fingerprints change while every format number remains v1.

[Scoped tests and synthetic render](validation/panels-2026-10-04/README.md) cover
all layout combinations, exact road-top exposure, chains, bounds, deterministic
generation, serialization and source/derived-product tampering. The marking is
loaded only by display consumers, not the headless geometry path.

The assembly fingerprint explicitly includes the shared `panels.rs` fitter,
so future fitter edits invalidate compiled assembly products as well as cell
cache fingerprints. Preserved packages must be recompiled by their authoring
workflow; this is not a loader fallback.

## Pipe bore reduction (08, 2026-10-04 replacement)

This explicitly replaces the earlier generated 2/4/6m bores with 1/2/3m. The
existing pipe draw is uniform at each difficulty: the identical seeded draw now
selects half its previous bore. Difficulty-dependent ordinary-road weights and
road width choices are unchanged. Authoring keeps 1/2/3/4/6m, including existing
4/6m sources without dimensional conversion. General cylinder catalogue defaults
are 2m; explicitly named wide presets retain their 4m defaults. Catalogue entries
expose `default_width_cm` and `min_port_width_cm` for consumers.

The cylinder body keeps its centreline bends, end poses and length. Only bore
radius and its dependent tessellation/clearances change. Outer ramps retain the
diameter/3 portal drop and maximum 12% analytic grade. Generated exit ramps taper
back to the former road-side width, so no 1m ordinary road/port is introduced.
Tube/ramp authoring admits 1m ports; ordinary-road ports still start at 2m.
Internal connected pipes retain matching open rings without internal funnels.

Straight-cylinder authoring now defaults to radius 125cm (2.5m bore), length
1600cm. Cylinder and swept-cylinder minimum radii are 50cm. Loop minimum radius
150cm and swept-halfpipe minimum 100cm are unchanged. Loop, halfpipe, panel and
ordinary-road dimensions stay unchanged. Current v1 and explicit stored values
remain; source fingerprints change, with no historical loader or converter.

Scoped macOS arm64 tests passed: three `pipe_dimensions` tests, five
`special_tracks` tests, seeded pipe-draw/budget and retained road-distribution
units, package tests `small_and_existing_authored_pipe_sizes_roundtrip_without_conversion`
and `reproducible_roundtrip_and_modified_source_rejected`, plus native/CLI build.
These cover all 12 cylinder presets at all five authored sizes, unchanged body
endpoints/length (under 3cm sampling difference), actual circular/hollow geometry,
portal frames/grade, internal open sections, shape/source tampering, deterministic
bytes, old 4/6m roundtrip and unchanged rejection budgets. Shared Godot material/
preview and Editor history verification are reported by the consumers. Runtime
passage observations are separate from geometric admission and user acceptance.
