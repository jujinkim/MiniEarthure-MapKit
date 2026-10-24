# Jump acceleration shortcuts

MapKit owns the optional route, explicit v1 source and compiled geometry. Runtime
owns vehicle motion and authority; the Client displays the received active state.

## Source and geometry

`Source.jump_shortcuts` is an optional v1 array retained by saved Assembly source.
Each record links start/end ring poses, independent platform poses/dimensions,
a flight-only path, sampled side-fence openings, and ordinary-road recovery/merge
references. Empty arrays are omitted. Existing saved maps are opened without
regeneration. The compiler generates ring, pad and solid identities from the
shortcut ID (`jump-<id>-start`, `-end`, `-pad-<index>`, `-solid-<index>`).

Five to eight independent platforms provide an optional route with at least 30%
saving including entry and merge. There is no road or connecting floor between
platforms. Solid supports and jump triggers are separate; the same authored
convexes drive display, collision and occupancy. Both wall rendering and solids
use the explicit openings. The venue floor leaves room below the platforms for
failed jumps and ordinary recovery.

Seed generation draws one ticket per seed from the jump stream: one shortcut with
probability 1/3, two with 1/12, otherwise none (at most two, independently of the
two-rail limit; only with the `gimmick` category). Each ticket fixes its 5–8
platforms. A ticket becomes a shortcut motif that places the long main detour
(`split`, `entry`, `corner`, `merge`) rigidly at the current end; without a ticket
that detour is never placed. Growth reserves the flight branch against later
pieces, finish samples the platform corridor against all roads before merging the
branch, and the single compile runs the full `clear` check. Failed geometry,
clearance, shared checkpoint or resource admission omits the whole optional
branch (the compile fallback drops all shortcuts). Main roads stay available.
Many tickets find no room: over 384 generations (seeds 0..64 × tiers × modes), 55 jump shortcuts were
installed. All route combinations retain
ordered common checkpoints; platform order is not a completion requirement.

## Runtime contract and checks

A forward start-ring crossing enables dedicated controls. Each real platform
contact sets travel-direction speed to exactly 80 km/h and computes vertical
launch speed separately using the next landing height, common Runtime drag and
a bounded 1.25 m apex. Platforms are 6×4 m, with 40 cm solids and 16 m centre
spacing; small alternating lateral offsets allow steering correction. Rings carry no ordinary air-ring acceleration in these
routes. Acceleration/braking change flight distance; steering moves sideways in
the section frame. Glide, boost and Shift air turn remain blocked until exit or
recovery, including input loss. The fixed spacing is verified against Runtime
with held throttle and maximum acceleration; skipping one platform must fail.

Focused core/package checks cover source and saved Assembly round trips,
recompilation, zero/one/two branches, deterministic seed tickets, wall geometry,
route combinations, rail coexistence and atomic rejection. Runtime's
`jump_shortcut_validator` uses package collision for vehicle and 60/120 Hz checks.
Detailed handling, full races, multiplayer and device acceptance are user checks.
