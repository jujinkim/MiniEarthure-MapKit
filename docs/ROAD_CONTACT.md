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

Runtime adapters reserve the two packed trait bytes
and two retained collision trait bytes per triangle.

Structural top winding is normalized for the shared one-sided Godot collision
adapter. Validation covers contact properties, archive corruption/budgets, seams,
surface probes, hashes and costs; a shader capture checks road/terrain contrast and
markings at snow stages 0–2. Designed-road collision is audited by
`mapkit audit-roads` ([ROAD_DESIGN](ROAD_DESIGN.md#cli-and-audit)). Device art
acceptance is user verification.

## Sidewalk boundaries and shared geometry

Sidewalk tops are 12cm above their clipped ground polygons. The generator unions
these footprints in integer XY and emits walls only on exposed boundaries,
excluding internal fragments and cell seams. Source edges retain bottom heights
and road identity. A 1cm support check removes false boundaries from rounded
intersections. Top faces remain spawnable; sides do not.

Each cell clips with one reused integer clipper. An apron that shares no point
with the remaining ground piece skips its intersection and difference but is
charged the same work, so output and budgets match clipping every bounds-overlap
pair. Building exclusions are indexed once per prepared map.

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

Validation: the exposed-boundary regression and Godot `surface_geometry_validator`.
Vegetation rendering is described in [VEGETATION](../VEGETATION.md).
