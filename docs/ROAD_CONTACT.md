# Road snow and generated contact identity (current v1)

Road `snow_retention_percent` is an integer 0–100, default 100. The default can
be omitted; authored nondefault values survive document/schema/package round trips
and affect source identity. Marked and unmarked roads use the same retention in
shared weather materials; paint receives the road cover. Terrain keeps full snow.

Every generated triangle has `contact_class` (0 authored obstacle, 1 natural or
painted terrain, 2 imported terrain-following road, 3 structural or independently
designed road) and
`snow_retention_percent`. Generation assigns these from the actual road/terrain
phase before placements are appended. Material names do not classify geometry.
Structural sides, railings, buildings and placements remain obstacles. These
traits survive cell hashes, the bounded disposable archive, packed Godot views
and collision reordering. Authored ordinary roads use their cubic design surface
and analytic tangent frames; their collision is independent of raw terrain bumps.

All own formats remain v1. Build fingerprints invalidate generated caches; no
previous archive reader or automatic source converter is provided. Existing source
packages remain immutable. Runtime adapters reserve the two packed trait bytes
and two retained collision trait bytes per triangle.

Validation covers contact properties, archive corruption/budgets, roads, road
safety, independent designed surfaces, cell seams, surface probes, hashes,
costs, packages and schemas. Structural top winding is normalized for the
shared one-sided Godot collision adapter. Shared shader capture checks road/terrain contrast and
yellow/white markings at snow stages 0, 1 and 2. Application/device art acceptance
is user verification.

Current independent-road delivery checks pass on macOS ARM64: nine core
independent-road regressions, road safety/urban geometry, package/composite,
source/schema/hash/region/cost suites and matching Godot native builds. The public
`mapkit audit-roads PACKAGE` command samples actual generated collision every
0.5 m on five width lines against the designed triangular surface (≤1 cm),
checks junction overlap and duplicate terrain contacts, analytic design grade
(≤12% for default worlds) and per-cell road-paint caps. The seven authored default
worlds pass 17,600 cells and 380,491 collision samples, with maximum 0.5 cm error.
Broad elevation change remains; detailed real driving is user verification.
