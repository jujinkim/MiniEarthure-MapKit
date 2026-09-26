# Rounded streets and automatic deck safety

Current v1 generation decision, 2026-09-26. Source coordinates, road/node IDs,
width settings and the 12cm curb height remain authored as before. `.memap`
format 2 and generated/recipe/scene versions 1 are unchanged. The source build
fingerprint invalidates disposable caches; there is no legacy generator branch.

## Shared plan

`road_plan.rs` supplies the road surface, exterior, curb, tunnel/underpass walls,
safety furniture, placement intersection and regional influence margin. Complete
endpoint stars are included before cell clipping. Offset boundary intersections
preserve the outside lane at bends; internal mouth cross-sections never become
exterior walls. Separate node/level identities do not connect merely because
roads cross in plan view. Ground follows the terrain subdivision; structures
retain authored heights and the existing strict ground-structure join check.

Exterior corner radii target 15% of width, clamped to 20–100cm; tangent lengths
are limited to 45% of each neighboring edge. Portable `libm` arcs use at most
10-degree steps and 1cm sagitta, then canonical centimetre rounding. Quantization
can merge samples. Overlapping ground approaches use an integer winding union with retained
edge ownership; centimetre-quantization slivers are collapsed within their rounding
band. Genuine disconnected regions or holes remain errors. Polygon self-intersections and unresolved structural mouths
are contextual errors, never silent sharp-corner fallback. Authored centerline
self-crossings at the same level also report the road and intersection location.

The sidewalk ring is built from the shared exterior and unioned before terrain
subdivision, avoiding walls inside touching sidewalk patches. Road paint uses
the same fillet operation and exterior edges. Centerline radius includes half
the carriageway, preserving lane ordering through bends. Degree-two roads share
a half-curve endpoint; larger intersections stop paint at the mouth. Dash periods
fit complete cycles between graph nodes, with symmetric endpoint phases and
cell-independent distance. Crosswalks retain their authored flags.

## Safety solids

Every elevated/bridge exterior, including an unconnected deck end, receives
80cm-high safety furniture outside the carriageway. Bridge bases are 20cm high
and 20cm wide with two metal rails above; elevated roads receive two metal rails.
Eight-centimetre posts are stationed every 200cm before cell clipping. Curve
samples, slopes and adjoining cells use the same uncut vertices and station.
There is no jump opening flag. Tunnel/underpass portals remain open.

Visible rail, post and base pieces are the same convex shapes used for collision
and occupied space. Generated IDs end in `:safety:metal` or `:safety:base` and are
reserved against authored IDs. They are not spawn/recovery surfaces. Metal is
only a renderer material; physical materials and serialized shape types do not
change. Existing manual facilities are preserved; intersections fail with road,
facility and position diagnostics. Sloping/diagonal pieces use SAT after broad
AABB checks to avoid false overlaps in empty bounds corners.

## Work and memory

Spatial and regional influence includes curve mouths and the outer furniture.
Cost estimates include convex/occupied pieces, rendered triangles, longer IDs and
bounded paint uniforms. The existing cell/memory caps are unchanged. Post queries
clip the station range before iteration. Cancellation checkpoints and local
plan/paint budgets reject work rather than permitting unbounded tessellation.

## Verification and artifacts

Focused core suites cover geometry, graph/order determinism, placement, occupancy,
curb seams and costs. `tests/road_safety.rs` adds straight/sloping/curved decks,
T/X mouths, dead ends, layered crossings, variable-width passage support, adjacent
cell post/paint continuity, archive round trips, cancellation, reserved IDs and
manual overlap. Package/region tests preserve cache, dependency, budget and
partition behavior. The native `godot/tests/road_safety_validator.gd` fixture
checks packed presentation, metal rendering, convex contact and non-recovery;
`MAPKIT_ROAD_CAPTURE` optionally writes a fixed-view image.

The 2026-09-26 golden vectors intentionally replace the old generated geometry:
all seven fixture input hashes and instance hashes stay unchanged, while rounded
roads, curbs and guard solids change triangle/collision/archive hashes and some
surface probes. This is an approved current-algorithm change, not a new format
or permission to reuse old completion evidence. See [DETERMINISM](../spec/DETERMINISM.md).
The independent permutation, geometric coverage and archive tests remain active.
The old golden also predates `0cb0062`'s existing gimmick archive trailer: otherwise
unchanged cells gain exactly six bytes (u32 length + `[]`). The archive encoder is
unchanged by this work; these stale expectations are explicitly synchronized.

`road_safety_probe` checks affected cells and authored starts in public synthetic
regional examples. `reseal_courses` copies definitions onto a new map identity
without completion evidence and refuses to overwrite destination files. Actual
vehicle driving, course completion, device appearance and sustained performance
remain consumer/user verification.

Known tooling limitation: `scripts/check_architecture.py` currently rejects the
pre-existing `std::thread` in `cancellation.rs`'s `#[cfg(test)]` module. This audit
failure predates this change; production ownership/dependencies remain unchanged.
