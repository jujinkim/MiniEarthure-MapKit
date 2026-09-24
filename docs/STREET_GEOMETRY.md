# Street geometry

Convex gimmick faces are outward in map coordinates. Z reflection already
produces clockwise Godot front faces; keep their order and supply flat outward
normals. All parts retain the declared solid material. Sidewalk union boundary
winding now explicitly keeps the interior to the left; interior and cell seams
remain excluded. Both prepared and direct rendering project vertical UVs.

`driving_structures.ramp` starts at zero height, with its underside buried 5cm.
`fit_to_surface` samples both entry edges including crossfall and bakes a convex
shape shared by collision/rendering. No format change or converter is involved.
New authoring uses `street_tree`: tapered trunk, attached roots and overlapping
crowns with vertex tint. Canopy 78/84 and palm 44/44 triangles, two materials.
Existing library/source artifacts and their reproducibility remain unchanged.

Validation: core urban 6, roads 9, cost 6; Python test_street_geometry 3;
Godot surface_geometry_validator (11 structure kinds, clockwise normals/material)
and fixed-view render passed on Godot 4.7.2/macOS. New map adoption is performed
by the Editor's miniature-streets authoring step. Detailed appearance is user review.

Junction terrain failures identify the road and offending map coordinate so
smaller authored bridge/portal aprons can be corrected without weakening the
1 cm terrain match check. The roads regression set (9) passes.

The new foliage exposed a compatibility-renderer MultiMesh color default: mesh
vertex colors were multiplied by black on instances. Explicit white instance
colors preserve the shared mesh tint (16 bytes/instance within existing object
reservations). The rendered street_tree_validator compares green coverage on
the original tree and both instances; all three pass. No triangle/material or
cell/memory cap changes.

Editor new-object templates come from `godot/driving_templates.json`, regenerated
by `python3 scripts/driving_structures.py`. This current library uses zero-height
ramp entries; the old `examples/driving-library` and saved user objects remain
unchanged. Sloped miniature-streets ramps bake both surface edges through the
shared `fit_to_surface` authoring helper.
