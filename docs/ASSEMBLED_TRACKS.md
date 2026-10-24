# Seeded track generation

`mapkit_core::assembled_track` owns the catalogue, dimensions, seeded selection,
connections, corridors and collision geometry. `mapkit_package::assembled_track`
adds the matching public course and package save/verification. Consumers never
reimplement the generator. Authored sources and the compiler API are owned by
[TRACK_AUTHORING](TRACK_AUTHORING.md); pipes and air rings by
[SPECIAL_DRIVING](SPECIAL_DRIVING.md). Surface dimensions, walls, supports and
finish plazas are owned by [TRACK_GEOMETRY](TRACK_GEOMETRY.md).

New generated source stores gate zero at the end of the third 8 m-wide runway
piece, leaving room for eight vehicle envelopes behind its trigger. Saved source
and package checkpoints retain their explicit positions.

## Generation settings

- `categories: ["driving", "gimmick", "action"]`, all enabled by default; empty or
  unknown selections are rejected. With driving and any special category enabled,
  gimmick/action pieces occupy 40–60% of the completed base-route distance. With
  only special categories, the minimum is 40% without an upper cap; driving-only
  courses contain no special pieces. All-category courses include both gimmicks
  and actions and at least three families. Selection draws a family before its
  variants, so additional pipe variants do not increase the pipe family's weight.
  Mirrored/width variants share a family; identical special families repeat at most
  twice in a row, including across a circuit seam.
- Jump panels, gap/offset jumps and air-ring pieces contain a standalone jump
  panel. Generated acceleration-panel and boost-chain pieces cannot connect
  directly to any of these jump pieces, including across a circuit seam; an
  intervening ordinary driving piece permits the sequence. Manual actions remain
  independent. New generation/recompilation uses this rule; saved packages keep
  their declared panels and geometry.
  Automatic air rings sit 3 m above the approach to clear the unboosted jump
  trajectory at the declared minimum entry speed.
- Settings `length` is `short`, `medium` or `long` (default `short`). Circuits
  race 3/2/2 laps; sprints are one run. Until the generator rewrite lands the
  layout search maps the tiers to 60/90/120 s and requires the base route at
  900 cm/s within ±10%; alternative routes are estimated separately. This is a
  reference estimate, not a measured lap time. The catalogue lists
  `length_options` per mode.
- Course names are `Seed {seed} · Short|Medium|Long`.
- Search: at most 24 candidates, 512 pieces, 32,000 samples, within cancellation,
  package, cell and memory budgets. Failure returns `E_TRACK_LAYOUT` with the
  requested length, closest time and reason; callers keep their current map.
- A seed first plans four to six sections with open driving, corners/elevation,
  gimmick challenge and recovery roles. The plan selects type priorities before
  layout attempts. Growth and attachments prefer unused types; backtracking uses
  actual retained placements. The base-route special-distance and straight limits
  remain hard constraints; preparation/recovery and optional routes add no special
  distance. Attachments preserve footprint clearance and the continuous bypass.
- [Diversity and narrow roads](TRACK_DIVERSITY.md) owns the independent piece/gimmick
  50% goals, taxonomy, candidate ordering and response-only `generation_summary`.
  After time/safety feasibility, candidates compare largest diversity deficit,
  combined deficit, time error and section-plan fit. Both goals permit early exit;
  a shortfall searches up to the existing 24-candidate cap and returns actual counts.
- Ordinary generated roads use 6/8/12/18 m within preset limits. At most one
  smoothly tapered 4 m ordinary-road run may occupy at most 5% of base distance,
  shared across main/optional routes and circuit seams; unsafe narrowing is omitted.
  Dedicated structures and required tube portals retain their own dimensions.
- `track_seed_defaults(seed)` returns `time_minutes` and `weather`, drawn uniformly
  from 06/09/12/17/18/21 hours and clear/cloudy/rain/snow using an independent stream.
  The Godot bridge exposes the same query. Generation stores this seed time in the
  existing v1 time field. `difficulty` is an inert v1 preservation field; new
  generation writes `normal`. Saved geometry retains its stored fields.
- `track_job.gd` captures seed environment defaults with the immutable request and
  returns them only with a successful result. Consumers adopt environment and map
  together after validation. Play/preview overrides stay outside package data.

## Composed modules

The catalogue includes banked hairpins (left/right, eased bank and flat leads),
spatial S roads (mirrored bends and smooth rise/fall), and swept spiral cylinders
(left/right, up/down, with flat portals). They use the common sampled road/tube
meshes, collision, clearance and width rules.

`templates::source` / `track_template_source` return editable v1 graphs for
`double_jump`, `air_ring_chain`, `grind_shortcut`, `grind_uturn_{left,right}`,
`grind_s_{left,right}`, `grind_spiral_{left,right}_{up,down}` and
`jump_shortcut_{5,6,7,8}`. Double jumps have
separate supported recovery and reentry; ring chains have one launch, three rings
and extended landing. Both can enter the generated base route.

Large hairpin, S and open rising/falling spiral selections expand into ordinary
composite corners. With `gimmick` enabled, an independent seed ticket attempts a
shortcut at 1/3 of these corners, with at most two installed per map and zero
allowed. The 40 cm curved beam uses the grind line's samples/frames and requires
jump entry, safe supported merge and at least 20% / 5 m saving including entry/exit.
Unsafe branches are omitted atomically. Two shortcuts have all four route
combinations and common checkpoints. [Corner shortcuts](CORNER_SHORTCUTS.md) owns
geometry, admission, template insertion and focused verification details.

Spatial-S selections mix the existing rail shape with long ordinary detours for
disconnected jump routes.
With `gimmick` enabled, an independent 1/3 ticket attempts a five-to-eight-platform
branch with at least 30% saving, explicit fence openings and entry recovery.
The limit is two jump routes separately from rails. Failed admission omits the
entire branch; all retained branch combinations share checkpoints.
[Jump shortcuts](JUMP_SHORTCUTS.md) owns the source and geometry contract.

## Circuit returns

Closure runs a bounded best-first search over straight, right-angle, 45°, grade
and height-return spiral modules, available regardless of category, plus selected action modules and loop/overpass returns with their
ordinary width preparation. It tracks distance deficits and remaining straight length;
completed returns must satisfy the same composition and sequence rules as growth. A final cubic
seam is allowed only between endpoints ≤800 cm apart with control-polygon length
≤1600 cm, forward-facing tangents and ≤23% vertical tangent component. A closure
stores at most 4096 nodes and 64 added modules, is attempted after four more
reference seconds of growth, and stops once time (±10%) and composition are feasible. Equivalent
50 cm pose bins distinguish straight allowance, special-distance share and the
last family and portal width before keeping the cheapest arrival. Hand-authored curves are not limited.

## Saved-package verification

`MapDocument::validate` checks saved assembly coordinates, frames, paths,
connections, support convexes and references without recompiling. Package reads
also check payload hashes, manifest, world identity and resource limits.
`verify_track` binds the candidate to the saved course; a changed generator or
catalogue fingerprint does not invalidate a sound package. `pack_bytes`,
`pack_source` and authoring `verify_document` require current-source equality.
For seed tracks `verify_document` rebuilds the document from the stored assembly,
whose `validate` already requires equality with the compiled `seed_source`; it does
not re-run the generator, so saving generates once. This proves the geometry
matches its seed graph, not that the current generator reproduces the settings;
`verify_seed_reproduction` keeps that regeneration check for tests and tools.
Package hashes and content binding are unchanged. A seed package whose generator
fingerprint is stale fails export; `reseal_seed_document` (package `reseal_seed`,
CLI `reseal-track PACKAGE NEW.memap`, Godot `reseal_seed_track_document`)
recompiles its `seed_source` under the current compiler and rebinds the course.
Seed sources must still deserialize as current v1 `Settings`.
Loading never edits a file. Existing narrow v1 roads, ports, loops and pipes retain
their stored dimensions. Recompilation applies the new catalogue minima and reports
the instance ID and width fields; it never widens source data automatically.

Validation: `seed_track_plan` covers deterministic environment, preserved fields,
completed-course distance density/diversity, category/time/resource bounds,
source recompilation and template geometry. `corner_shortcuts` covers seed tickets,
zero/one/two placements, all route combinations, curved solids and atomic omission;
package tests cover every new template and two-branch save/reopen. Obstacle unit tests cover quarterpipe
handedness, actual wall distance, curved/tapered rejection and the continuous bypass; focused
`mapkit-core` assembled/track_winding/wall/special-track tests
and `mapkit-package` assembled-track tests. Detailed driving is user verification.
