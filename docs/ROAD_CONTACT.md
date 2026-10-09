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

## Sidewalk boundaries and shared geometry

Sidewalk tops are 12cm above their clipped ground polygons. The generator unions
these footprints in integer XY and emits walls only on exposed boundaries,
excluding internal fragments and cell seams. Source edges retain bottom heights
and road identity. A 1cm support check removes false boundaries from rounded
intersections. Top faces remain spawnable; sides do not.

Convex gimmick faces point outward in map coordinates. Z reflection produces
Godot clockwise front faces, so their order is retained with flat outward normals.
Sidewalk boundary winding keeps the interior on the left. Prepared and direct
rendering both project vertical UVs and retain declared solid materials.

Ramp entries start at zero height with a 5cm buried underside. `fit_to_surface`
samples both entry edges, including crossfall, and bakes one shape for collision
and display. Editor templates come from `godot/driving_templates.json`, generated
by `scripts/driving_structures.py`; saved objects and example packages are preserved.
Junction terrain errors report road ID and map coordinate without weakening the
1cm terrain match requirement.

The exposed-boundary regression checks a two-cell road, absence of walls inside
supported sidewalks and at shared seams, and presence of the exterior step.
Road/cost/package tests and Godot `surface_geometry_validator` cover bend/prop/
slope geometry, clockwise normals and materials. These scoped checks passed;
detailed driving and appearance remain user verification. Vegetation rendering
is described in [VEGETATION](../VEGETATION.md).
