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
Dedicated loops, halfpipes and the compact overpass use 400cm; pipe bores support
200/400/600cm. Internal profiles remain catalogue-owned.

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
