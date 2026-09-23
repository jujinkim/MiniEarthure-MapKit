# Sidewalk collision boundary — 2026-09-24

The current v1 sidewalk generator raises each clipped ground polygon by 12 cm.
Previously it emitted four side faces for every polygon. Terrain triangles,
road apron patches and exclusions split one continuous sidewalk into many
polygons, so those faces also stood inside the drivable top. The top triangles
are spawnable; the sides are not. GameRuntime registers these two classes on
different static bodies, so Jolt's internal edge removal within one body cannot
remove the hidden faces.

`roads::sidewalk_boundary_walls` now unions the emitted top footprints in the
integer XY plane and builds walls only on exposed union boundaries. It omits
cell-edge walls that would duplicate a neighbor's seam. Source edges supply
the bottom height and road identity. A one-centimetre top-support check removes
short false boundaries from rounded overlay intersections. The top mesh,
12 cm step, surface and v1 package fields remain the same. The generated
geometry changes, so the existing source fingerprint invalidates disposable
generated caches; no format version changes.

`sidewalk_walls_only_follow_exposed_edges_not_fragment_or_cell_seams` uses a
fixed two-cell ground road. It rejects any wall whose two sides both have
spawnable sidewalk support, checks the shared cell seam, and requires an
exterior step. Existing bend, prop, sloped-terrain, road and package contract
tests also pass. The root [diagnosis and handoff](../../docs/SIDEWALK_LANDING.md)
records the commands and scope. Actual reported driving behavior remains for
user verification.
