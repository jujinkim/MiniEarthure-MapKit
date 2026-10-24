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
  unknown selections are rejected. Disabled categories contribute no pieces. Without
  `driving`, only connector roads (straight, sprint lane, right-angle, 45°, grade,
  90° spiral, portals, the closure seam and the finish) join the special motifs.
  Without `gimmick` there are no automatic obstacles.
- Generation grows a sequence of motifs (`assembled_track/motifs.rs`): sweeps,
  corners, alternating hairpin chains (2–4, ≥8 m straight between, banked at 12 m),
  S-bends, esses, chicanes, spirals, rises/falls and straights; pipe, halfpipe, loop
  (≤2) and overpass (≤1) gimmicks; gap jumps and boost-before-rise actions. Each
  motif draws its own turn direction; past ±540° of accumulated yaw it leans the
  other way. A per-seed style scales family weights by 0.5–1.5 and the
  600/800/1200/1800 cm width weights (35/35/20/10); widths change only between
  motifs. No family runs three motifs in a row; pipes are ≥4 motifs apart, gap
  jumps ≥3.
- Gap jumps (`jump`, `offset_jump`, `air_ring`) have a 16 m approach and landing
  straight. The `jump_panel` piece is never generated. Acceleration panels and
  boost chains follow a hairpin chain or pipe exit, or precede a rise, and never
  connect directly to a jump piece, including across a circuit seam. Manual actions
  remain independent; saved packages keep their declared panels and geometry.
  Automatic air rings sit 3 m above the approach to clear the unboosted jump
  trajectory at the declared minimum entry speed.
- Optional straights (horizontal heading, so grades count) never exceed 32 m.
  Sharp corners and 8 m grades are shorter than the clearance between their
  neighbours, so motifs follow them with a level link or keep them at 6 m.
- Settings `length` is `short`, `medium` or `long` (default `short`). Circuits
  race 3/2/2 laps; sprints are one run of exactly 30/45/60 counted pieces (the
  runway, finish and pipe portals are not counted). Circuits may close after
  300/500/750 m. Past 150/300/500 m, motif weights are multiplied by
  exp(β·(distance gain + ½·heading agreement + height return)) toward the pose
  40 m behind the start, β = 3.0/1.8/1.0 · ramp^1.5 up to 700/1100/1600 m and 4
  after it; 40 pieces later the attempt is abandoned. Circuit growth keeps a 40 m
  by 50 m corridor behind the start free for the return. `estimated_msec` is the
  base route at 900 cm/s, a reference estimate, not a measured lap time. The
  catalogue lists `length_options` per mode.
- Course names are `Seed {seed} · Short|Medium|Long`.
- Search: at most 4 attempts, each with ≤8 weighted draws per motif and up to 3
  consecutive / 48 total motif pops; every draw uses a stream derived from seed,
  attempt, step and retry. Placement clearance uses the padded piece grid and the
  shared `conflict` predicate; counters update per piece. Limits stay 512 pieces
  and 32,000 samples, within cancellation, package, cell and memory budgets.
  Failure returns `E_TRACK_LAYOUT` with the requested length and reason; callers
  keep their current map. Progress reports `searching` (pieces), `closing`,
  `finishing` and `validating`.
- A finished layout gets obstacles (seeded spacing; the final recovery share of
  the route and gap-jump roads stay clear), shared checkpoints from the common
  policy (thinned evenly to 64 effective gates), and exactly one compile;
  validation reuses that compile.
- [Diversity and narrow roads](TRACK_DIVERSITY.md) owns the response-only
  `generation_summary` taxonomy and the narrow-road budget.
- Ordinary generated roads use 6/8/12/18 m within preset limits. Generated seed
  tracks currently contain no 4 m ordinary road; dedicated structures and required
  tube portals retain their own dimensions.
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
and extended landing. They are editor templates; the seed generator does not place
them.

Seeded grind-rail corner shortcuts and jump-shortcut routes are currently not
installed: the motif generator places no shortcut tickets, and seed tracks have one
route. The shortcut sources, admission and stored-package support are unchanged.
[Corner shortcuts](CORNER_SHORTCUTS.md) owns their geometry and admission;
[Jump shortcuts](JUMP_SHORTCUTS.md) owns the source and geometry contract.

## Circuit returns

Once a circuit may close, every step first tries an analytic cubic `free_curve`
seam into the start: ≤60 m, horizontal radius ≥ w/2+400 cm, ≤23% vertical tangent,
no self intersection, clear of the grid and within the straight limit. Near the
approach (≤90 m, behind the start, not facing away) a bounded A* over straight,
45°, right-angle, grade and 90° spiral pieces (6 m) retries the seam after every
expansion. It stores at most 1024 nodes (a quarter for later searches) and 16
pieces, checks clearance when a node is expanded, and runs at most six times per
attempt. No filler or action pieces are added.

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

Validation: `seed_track_generator` covers determinism and `seed_source`
recompilation (seeds 0..16 per tier and mode), exact sprint sizes, closure of every
circuit for seeds 0..64 per tier, tier length order, turn balance, hairpin-chain and
S/chicane frequency, the straight, jump and boost rules, category masks and template
geometry; `seed_generator_counters` the single compile and clearance-call bound;
ignored `track_generation_speed` the release timing of long tracks.
`corner_shortcuts` covers zero/one/two placements, all route combinations, curved
solids and atomic omission (its seeded case is paused with the shortcut tickets);
package tests cover every new template and two-branch save/reopen. Obstacle unit tests cover quarterpipe
handedness, actual wall distance, curved/tapered rejection and the continuous bypass; focused
`mapkit-core` assembled/track_winding/wall/special-track tests
and `mapkit-package` assembled-track tests. Detailed driving is user verification.
