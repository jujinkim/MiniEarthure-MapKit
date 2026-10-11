# Seeded track generation and geometry

`mapkit_core::assembled_track` owns the catalogue, dimensions, seeded selection,
connections, corridors and collision geometry. `mapkit_package::assembled_track`
adds the matching public course and package save/verification. Consumers never
reimplement the generator. Authored sources and the compiler API are owned by
[TRACK_AUTHORING](TRACK_AUTHORING.md); pipes and air rings by
[SPECIAL_DRIVING](SPECIAL_DRIVING.md).

## Generation settings

- `categories: ["driving", "gimmick", "action"]`, all enabled by default; empty or
  unknown selections are rejected. Categories supply candidates; no family has a
  minimum occurrence (start, finish, width transitions, required approaches and
  closure excepted). Identical gimmick families repeat at most twice in a row,
  including across a circuit seam. Enabled category tickets are 26:9:5.
- Circuit (one lap) and sprint offer 60/90/120 s. The base route length at
  900 cm/s must be within ±10%; alternative routes are estimated separately. This
  is a reference estimate, not a measured lap time.
- Search: at most 24 candidates, 512 pieces, 32,000 samples, within cancellation,
  package, cell and memory budgets. Failure returns `E_TRACK_DURATION` with the
  requested/closest time and reason; callers keep their current map.
- Difficulty first draws a driving family, then a uniform left/right/up/down
  variant. Family weights (easy/normal/hard): straight 34/16/8, grade 12/10/6,
  gentle corner 34/24/14, right angle 16/18/18, zigzag 4/8/12, sharp135 0/8/12,
  hairpin 0/6/12, spiral90 0/6/8, spiral180 0/3/6, spiral360 0/1/4.
- Width weights for 2/4/6/8/12 m: easy 0/20/40/30/10, normal 10/30/30/20/10, hard
  30/40/20/8/2. Obstacle spacing is 64/32/16 m. API default seed is 1; settings
  have no weather field.

## Catalogue and dimensions

- Ordinary widths: 200/400/600/800/1200 cm with eased entry/exit widths. Loops,
  halfpipes and the compact overpass use 400 cm. The public catalogue reports
  category, supported dimensions and ports.
- Extra-wide presets (fixed 1200 cm body, default 1200 cm ports, ports editable
  200–1200 cm): `straight_extra_wide` (8 m), `gentle90_extra_wide`/`_left` (90°,
  radius 48 m), `right90_extra_wide`/`_left` (90°, radius 8 m). A generated 12 m
  draw of one of those five shapes publishes the extra-wide ID with no extra RNG
  draw. Saved IDs are not rewritten.
- Bend radius for width w (m): gentle 45°/90° max(16, 4w); right-angle 90°
  max(4, w/2+2); sharp 135°/180° max(3, w/2+1). Transitions use the widest section.
- Helices: 90°/180°/360°, either direction, up/down, radius max(8, w/2+7) m, rises
  2/4/8 m, eased grades.
- Loop upper arc advance follows radius `r*(1.1 + 0.6*cos(t) + 0.3*cos(2*t))`
  (minimum 0.65r, crown 0.8r); height is 2r. Quintic feet match position, slope and
  curvature to flat road. The acceleration panel sits on the final straight.
- The overpass lower road uses analytic quarter-circle tangents; empty-control
  `free_curve`/`flight_curve` keep a straight exit tangent.

## Circuit returns

Closure runs a bounded best-first search over straight, right-angle, 45°, grade
and height-return spiral modules, available regardless of category. A final cubic
seam is allowed only between endpoints ≤800 cm apart with control-polygon length
≤1600 cm, forward-facing tangents and ≤23% vertical tangent component. A closure
stores at most 4096 nodes and 64 added modules, is attempted after four more
reference seconds of growth, and stops early within 1% of the target. Equivalent
50 cm pose bins keep the cheapest arrival. Hand-authored curves are not limited.

## Surface geometry

- Curves sample the analytic centre and ribbon, quantizing only final centimetre
  vertices: ≤4° tangent/frame change, ≤1 cm surface error, ≤1.5 m spacing.
- Spirals are validated against a 23% grade on inside/centre/outside edges. Ramps
  ease over their first/last eighth. Loops and pipes use adaptive sampling without
  the ordinary grade cap.
- Snapped pieces rebuild end ribbons from the final integer frame; saved endpoint
  ribbons are canonicalized on load without rewriting.
- Faces are published with the same winding as ordinary roads and terrain; the
  Godot adapter produces outward clockwise faces. `track_winding` checks
  orientation, join vertices and closed walls.
- Regenerate or recompile to receive geometry corrections; loading or sharing a
  saved `.memap` keeps its stored geometry.
- The opt-in catalogue audit:
  `cargo test -p mapkit-core --test track_winding catalogue_geometry_audit_includes_tapers_editor_curves_and_special_surfaces -- --ignored`.
  Arbitrary user control points are outside it.

## Walls and clearance

- Ordinary boundaries are closed 50 cm outward volumes emitted as deterministic
  tetrahedra; shared caps cancel across seams. Rendering, collision and occupancy
  share the quantized parts. Wall tops support vehicles but never spawn/recovery.
- Connected-road clipping uses actual road triangles (2 cm inset, 5 cm height
  tolerance); outside and grade-separated walls stay. Cost estimation traces the
  same clipping plan and reserves its workspace.
- Clearance: nonlocal road separation and ribbon self-intersection are checked.
  Level constant-width straight ribbons without a shared port use their finite
  footprint plus 50 cm wall and 30 cm vehicle margin; boundary contact counts as
  overlap. Other pairs use sampled volumes. Failures are draft issues
  (`E_TRACK_DRAFT` on export).
- RC ramps are 160 cm wide; rail tops are 40 cm with an explicit grind line. Venue
  decoration has no driving collision.
- Up to 64 display-only direction chevrons per stage at 24 m intervals on flat road
  (no collision, occupancy, spawn role or shadow).

## Grounded supports

- Seed assemblies ground the venue at the minimum of the actual slab, finish plaza,
  branches and special-track inner/shell mesh. Clearance envelopes, flight paths and
  decorations do not lower it. Ribbons/plazas expose a 10 cm underside and sides.
- `Assembly.supports` records one 20×20 cm vertical convex column per piece with an
  airborne underside; its flat cap uses the minimum clipped underside over the
  square. Candidates are deterministic, centre-first, longitudinal and lateral.
  Pure flight has no support.
- Columns may cross lower roads only if a continuous 110 cm lane remains with
  nearby columns and obstacle sweeps. Start runways, bores, flight, launch and
  landing space are protected. No valid placement yields `E_TRACK_SUPPORT`; the
  generator rejects that candidate.
- Columns (`assembled-support-<piece_index>`) and shells
  (`assembled-shell-<piece_index>`) are non-spawnable structures. Costs, convex
  capacity and mesh workspace are charged; grounding keeps at most one
  300,000-face piece mesh.
- `Source.grounded_supports` is false for independent manual sources and true when
  converted from a seed. External imports are unchanged.

## Sprint finish plaza

Sprint assemblies end in one `finish_plaza`: 8 m entrance, 16 m circular floor and
1.2 m wall open only at the entrance. `Assembly.finish_plaza` gives consumers the
checkpoint, centre, recovery direction and bounds. Only the first 4 m of the
entrance counts toward race distance. The finish candidate is a path sample in the
terminal entry; if it overlaps an earlier gate, first-wins removal makes that gate
the finish. Circuits have no terminal piece.

## Saved-package verification

`MapDocument::validate` checks saved assembly coordinates, frames, paths,
connections, support convexes and references without recompiling. Package reads
also check payload hashes, manifest, world identity and resource limits.
`verify_track` binds the candidate to the saved course; a changed generator or
catalogue fingerprint does not invalidate a sound package. `pack_bytes`,
`pack_source` and authoring `verify_document` require current-source equality.
Loading never edits a file.

Validation: focused `mapkit-core` assembled/track_winding/wall/special-track tests
and `mapkit-package` assembled-track tests. Detailed driving is user verification.
