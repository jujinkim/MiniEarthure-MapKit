# Declarative driving library

Original MIT centimetre-scale convex templates: ramps, separated jump/landing
ramps, humps, hollow pipes/logs, an open halfpipe, translating barriers, rotating
crosses, moving platforms, boost pads and launch pads. No game data or executable
scripts are embedded. Pipe inner radius is 100 cm; halfpipe inner radius is 190 cm.
Their compound parts leave the passage open. Surface/color are editable.

`library.json` is the template input for an authoring UI. The Python constructors
in `scripts/driving_structures.py` also support authored positions and rotations.
A map owner supplies placements, destinations and route design. Translation and
impulse vectors use source-world coordinates; rotation uses the selected world
axis. `safety_min_cm`/`safety_max_cm` must include the full motion and intended
landing area. Recompute them after changing scale or a part.

The current v1 schema bounds every field; see `spec/CURRENT_V1.md` and
`crates/mapkit-core/src/gimmick.rs`. Focused `gimmicks` tests cover bounded input,
identity/hash/archive/cost validation, open compound centers and expanded safety
windows. Physics, user driving and platform acceptance belong to consumers.
