# Road snow and generated contact identity (current v1)

Road `snow_retention_percent` is an integer 0–100, default 100. The default can
be omitted; authored nondefault values survive document/schema/package round trips
and affect source identity. Marked and unmarked roads use the same retention in
shared weather materials; paint receives the road cover. Terrain keeps full snow.

Every generated triangle has `contact_class` (0 authored obstacle, 1 natural or
painted terrain, 2 terrain-following road, 3 structural road) and
`snow_retention_percent`. Generation assigns these from the actual road/terrain
phase before placements are appended. Material names do not classify geometry.
Structural sides, railings, buildings and placements remain obstacles. These
traits survive cell hashes, the bounded disposable archive, packed Godot views
and collision reordering. Cell geometry and heights do not change.

All own formats remain v1. Build fingerprints invalidate generated caches; no
previous archive reader or automatic source converter is provided. Existing source
packages remain immutable. Runtime adapters reserve the two packed trait bytes
and two retained collision trait bytes per triangle.

Validation: contact properties, archive corruption/budgets, roads, road safety,
surface probes, hash, cost, package and schema checks. The current determinism
vectors were reviewed: all input/geometry counts, objects, occupancy and probes
are unchanged. Triangle/generated digests change for the new metadata; each
archive adds two bytes per triangle. The previous golden also lacked the already
implemented six-byte empty water tail; it is now reflected explicitly. Old vectors
remain in Git history. Shared shader capture checks road/terrain contrast and
yellow/white markings at snow stages 0, 1 and 2. Application/device art acceptance
is user verification.
