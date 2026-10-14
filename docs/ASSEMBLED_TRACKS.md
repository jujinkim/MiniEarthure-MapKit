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
  unknown selections are rejected. Categories supply candidates; no family has a
  minimum occurrence (start, finish, width transitions, required approaches and
  closure excepted). Identical gimmick families repeat at most twice in a row,
  including across a circuit seam. Category weights and section order come from the seed plan.
- Jump panels, gap/offset jumps and air-ring pieces contain a standalone jump
  panel. Generated acceleration-panel and boost-chain pieces cannot connect
  directly to any of these jump pieces, including across a circuit seam; an
  intervening ordinary driving piece permits the sequence. Manual actions remain
  independent. New generation/recompilation uses this rule; saved packages keep
  their declared panels and geometry.
  Automatic air rings sit 3 m above the approach to clear the unboosted jump
  trajectory at the declared minimum entry speed.
- Circuit (one lap) and sprint offer 60/90/120 s. The base route length at
  900 cm/s must be within ±10%; alternative routes are estimated separately. This
  is a reference estimate, not a measured lap time.
- Search: at most 24 candidates, 512 pieces, 32,000 samples, within cancellation,
  package, cell and memory budgets. Failure returns `E_TRACK_DURATION` with the
  requested/closest time and reason; callers keep their current map.
- A seed first plans the whole course as four to six ordered sections. Independent
  draws choose corner families, elevation, road widths, obstacle spacing and
  gimmick/action shares; sections arrange straights, bends and action sequences.
  Flat ordinary courses and courses with substantial elevation/actions are valid.
  Retry randomness changes placement candidates, while the section plan stays fixed.
  Candidate ranking first requires time feasibility, then compares duration error
  and actual section composition with that seed's plan.
- Width choices stay 4/6/8/12/18 m, restricted to each preset's supported dimensions.
  Runways, merges and returns use 8 m. Pipe bores use the catalogue's 4/6 m choices.
  Obstacles use independent seeded spacing and must retain the continuous bypass.
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
`double_jump`, `air_ring_chain` and `grind_shortcut`. Double jumps have separate
supported recovery and reentry space; ring chains have one launch, three rings and
an extended landing. Both can enter the generated base route. Grind is an optional
branch with shared split/merge checkpoints, a supporting rail and an explicit grind
line. Template insertion transforms all roads, actions, structures and lines
consistently and rejects unsafe candidates through the regular compiler/budgets.
The complete approach/recovery/merge stays free of generated obstacles; ring
apertures reserve clearance above ordinary road envelopes.
`append_track_template` expands a module at the base-route endpoint for authoring.

## Circuit returns

Closure runs a bounded best-first search over straight, right-angle, 45°, grade
and height-return spiral modules, available regardless of category. A final cubic
seam is allowed only between endpoints ≤800 cm apart with control-polygon length
≤1600 cm, forward-facing tangents and ≤23% vertical tangent component. A closure
stores at most 4096 nodes and 64 added modules, is attempted after four more
reference seconds of growth, and stops early within 1% of the target. Equivalent
50 cm pose bins keep the cheapest arrival. Hand-authored curves are not limited.

## Saved-package verification

`MapDocument::validate` checks saved assembly coordinates, frames, paths,
connections, support convexes and references without recompiling. Package reads
also check payload hashes, manifest, world identity and resource limits.
`verify_track` binds the candidate to the saved course; a changed generator or
catalogue fingerprint does not invalidate a sound package. `pack_bytes`,
`pack_source` and authoring `verify_document` require current-source equality.
Loading never edits a file. Existing narrow v1 roads, ports, loops and pipes retain
their stored dimensions. Recompilation applies the new catalogue minima and reports
the instance ID and width fields; it never widens source data automatically.

Validation: `seed_track_plan` covers deterministic environment, preserved fields,
completed-course diversity, category/time bounds and template geometry; focused
`mapkit-core` assembled/track_winding/wall/special-track tests
and `mapkit-package` assembled-track tests. Detailed driving is user verification.
