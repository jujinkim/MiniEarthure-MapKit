# Curved corner grind shortcuts

MapKit owns composite main roads, optional branch admission and editable v1 source.
[Seed generation](ASSEMBLED_TRACKS.md) owns category weights, length tiers and search;
[authoring](TRACK_AUTHORING.md) owns Source contracts. Runtime consumes the existing
structures and grind lines without template-specific physics or protocol changes.

## Selection and geometry

The seed generator places the large composite corners (U-turn, S or 360-degree
spiral, mirrored by the motif's turn direction, 6 or 8 m wide) only as shortcut
motifs for a ticket. The independent grind stream keeps the 1/3 rule for the
first two corner ordinals, so a seed has zero, one (4/9) or two (1/9) tickets;
without a ticket no large detour corner is placed. Only enabled `gimmick`
categories admit rails, with at most two installed per map; zero is valid. The
spiral has open, displaced portals and a smooth rise/fall. Corner main roads count
as ordinary distance, and optional branches never increase special distance.
Straight-run and search limits still apply. Growth places the template's base
roads rigidly at the current end, checks the branch roads against the grid and the
rail corridor against nearby roads, and reserves the branch against later pieces;
finish merges the branch without extra compiles and checks `clear` on the single
compiled result. Over 384 generations (seeds 0..64 × tiers × modes), 78 rails were installed.

Each shortcut has a supported jump approach and landing, shared split/merge gates,
and a continuous cubic rail. Its top is 40 cm above the branch runway. The 40 cm
beam's display and solid convexes use the grind line's actual samples and
transported frames, split into structures of at most 32 parts. Including entry,
exit and vertical jump/landing allowance, it saves at least 20% and 5 m against the
same main-route split/merge. After the compile, sampling checks the full rail vehicle corridor against
roads/walls, obstacles, supports, other rails and authored solids. The complete
approach/recovery/merge stays free of generated obstacles. Unsafe placement,
checkpoint or resource limits omit the shortcuts (the compile fallback drops all of
them at once), preserving the main road.
Two branches produce all four complete route combinations with ordered common
checkpoints. Source recompilation and package reopening retain these routes.

Automatic obstacle selection excludes short rails and straight fence grind lines;
explicit attachments and stored v1 products retain their geometry. Template
insertion transforms all roads, actions, structures and lines consistently and
rejects unsafe candidates through the regular compiler and budgets. Ring apertures
reserve clearance above ordinary road envelopes. `append_track_template` expands
a module at the base-route endpoint. No additional settings or physics are required.


## Verification

`mapkit-core` corner-shortcut tests cover all mirrored forms, supported widths,
main-road connectivity, sampled beam tops after placement rotation, savings,
obstacle/other-rail rejection, source budgets, atomic omission and all four route
combinations. Seed tests cover zero/one rails per seed, deterministic recompilation,
category gating and the two-per-map limit. Focused package tests compile,
serialize and reopen every template and two branches with ordered shared gates.
Existing packages are loaded without regeneration or edits.

Runtime's `curve_shortcut_validator` uses real package collision at 60/120 Hz:
ordinary ground jump, capture, full curved/up/down traversal, exit and supported
road merge. Throttle remains held; climbing may use the existing booster. Detailed
handling, unrestricted boost speeds, Host/Join and devices remain user checks.
