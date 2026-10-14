# Track catalogue and surface geometry

[Seeded generation](ASSEMBLED_TRACKS.md) and [authoring](TRACK_AUTHORING.md)
use these shared MapKit meshes, ports and clearance rules.

## Catalogue and dimensions

- New ordinary widths: 400/600/800/1200 cm, plus five 1800 cm presets (manual default 400 cm) with eased entry/exit widths. Loops,
  halfpipes and the compact overpass use 400 cm. The public catalogue reports
  category, supported dimensions and minimum/maximum port widths. Ordinary roads
  and transitions accept 400–1800 cm ports; dedicated loop/pipe/overpass ports
  retain their existing limits.
- Extra-wide presets (fixed 1200 cm body, default 1200 cm ports, ports editable
  400–1800 cm): `straight_extra_wide` (8 m), `gentle90_extra_wide`/`_left` (90°,
  radius 48 m), `right90_extra_wide`/`_left` (90°, radius 8 m). A generated 12 m
  draw of one of those five shapes publishes the extra-wide ID with no extra RNG
  draw. Saved IDs are not rewritten.
- Ultra-wide presets (fixed 1800 cm body and default ports, editable 400–1800 cm
  ports): `straight_ultra_wide` (8 m), `gentle90_ultra_wide`/`_left` (radius 72 m),
  `right90_ultra_wide`/`_left` (radius 11 m). An 18 m draw publishes these IDs
  without another RNG draw; manual and generated geometry share the same rules.
- Bend radius for width w (m): gentle 45°/90° max(16, 4w); right-angle 90°
  max(4, w/2+2); sharp 135°/180° max(3, w/2+1). Transitions use the widest section.
- Helices: 90°/180°/360°, either direction, up/down, radius max(8, w/2+7) m, rises
  2/4/8 m, eased grades.
- Newly compiled vertical loops have radius 5.25 m and height 10.5 m, with a
  4 m ribbon. Helices retain their dimensions. Loop upper arc advance follows radius `r*(1.1 + 0.6*cos(t) + 0.3*cos(2*t))`
  (minimum 0.65r, crown 0.8r); height is 2r. Quintic feet match position, slope and
  curvature to flat road. The acceleration panel sits on the final straight.
- The overpass lower road uses analytic quarter-circle tangents; empty-control
  `free_curve`/`flight_curve` keep a straight exit tangent. The 4 m upper ribbon
  uses smooth lateral curvature at both ports while retaining its span and height.

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
- RC ramps are 320 cm wide, retaining their heights and lengths; rail tops are 40 cm with an explicit grind line. Venue
  decoration has no driving collision.
- Automatic attachments are skipped unless their swept footprint and a continuous
  220 cm bypass fit the actual ribbon. Manual placement reports the piece and
  station in `E_TRACK_CLEARANCE`. Grind rails and air-ring dimensions are unchanged.
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
- Columns may cross lower roads only if a continuous 220 cm lane remains with
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
