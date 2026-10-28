# Rounded streets and automatic deck safety

Source coordinates, road/node IDs, widths and the 12 cm curb height (3 cm on real-city OSM maps, [ROAD_CONTACT](ROAD_CONTACT.md)) are authored
input. Designed road curves are owned by [ROAD_DESIGN](ROAD_DESIGN.md).

## Shared plan

`road_plan.rs` supplies the road surface, exterior, curb, tunnel/underpass walls,
safety furniture, placement intersection and regional influence margin.

- Complete endpoint stars are included before cell clipping. Offset boundary
  intersections keep the outside lane at bends; internal mouth sections never
  become exterior walls. Crossing in plan view never connects separate node/level
  identities.
- Ground follows terrain subdivision; structures keep authored heights and the
  strict ground-structure join check.
- Exterior corner radius is 15% of width, clamped to 20–100 cm; tangents ≤45% of
  each neighbouring edge. `libm` arcs use ≤10° steps and 1 cm sagitta, then
  centimetre rounding.
- Overlapping ground approaches use an integer winding union; quantization slivers
  collapse within their rounding band. Disconnected regions, holes, polygon
  self-intersections, unresolved mouths and same-level centerline self-crossings
  are contextual errors, never sharp-corner fallbacks.
- Nearly retraced ground approaches share a terminal cap so both keep a nonzero
  surface. Bridge/tunnel mouths need explicitly separated approaches.
- The sidewalk ring is built from the shared exterior and unioned before terrain
  subdivision. Road paint uses the same fillets; centerline radius includes half
  the carriageway. Degree-two roads share a half-curve endpoint; larger
  intersections stop paint at the mouth. Dashes fit complete cycles between nodes
  with cell-independent phase. Crosswalks keep their authored flags.
- `RoadMarkings.stop_lines` (optional, false) paints a stop line and lane arrows
  on the inbound half at each road end meeting at least two other roads, before
  the crosswalk when present; `one_way` (1 along the points, -1 against, 0 two-way)
  makes the whole width inbound at the travel end only. Each inbound lane gets
  arrows from the other arms' angles (straight within 35°, left/right to 150°):
  the leftmost lane its left turn, the rightmost its right turn, others straight.
  `MapDocument.traffic.driving_side` picks the inbound half (absent = right). All of
  it is shader paint from the road style; geometry and hashes of cells are unchanged.

## Safety solids

- Every elevated/bridge exterior, including an unconnected deck end, gets 80 cm
  safety furniture outside the carriageway. Bridges: 20×20 cm base plus two metal
  rails; elevated roads: two metal rails. 8 cm posts every 200 cm, stationed
  before cell clipping. Tunnel/underpass portals stay open; there is no jump opening.
- Visible rail/post/base pieces are the same convexes used for collision and
  occupancy. IDs end in `:safety:metal` or `:safety:base` and are reserved. They are
  not spawn/recovery surfaces; metal is a renderer material only.
- Manual facilities are preserved; intersections fail with road, facility and
  position diagnostics. Sloping/diagonal pieces use SAT after AABB checks.

## Work and memory

- Spatial/regional influence includes curve mouths and outer furniture. Costs
  include convex/occupied pieces, rendered triangles, IDs and paint uniforms.
  Cancellation and local plan/paint budgets reject excess work.
- Placement validation builds one transient road bounds/endpoint index per
  document; courtyard checks compare corridor polygons directly with building
  rings. The 8 million work limit (enough for about 11,000 dense 1:8 city
  buildings) and exact contact rules stay authoritative.
- Terrain tiles, surface overlap and face classification query ordered spatial
  indexes; dense arrangements, or faces beyond the 16-hole/512-vertex triangulation
  limit (many disjoint islands in one tile), split into quarters (depth ≤6) only at
  that local limit. Exact predicates, surface order and cumulative budgets are
  unchanged.

## Validation

`tests/road_safety.rs` (decks, mouths, dead ends, layered crossings, seams,
archives, cancellation, reserved IDs) and the Godot `road_safety_validator`
(`MAPKIT_ROAD_CAPTURE` optionally writes an image). `road_safety_probe` checks
public regional examples. `reseal_courses` copies course definitions onto a new map
identity without completion evidence and never overwrites. Vector status:
[DETERMINISM](../spec/DETERMINISM.md). Vehicle driving and appearance are user checks.
