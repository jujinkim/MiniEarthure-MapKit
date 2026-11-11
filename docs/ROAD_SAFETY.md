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
  centimetre rounding. Consecutive corners that round to the same plan position
  share the outgoing corner and owner, including when slope interpolation gives
  their heights different rounded values.
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
  Mark shapes and sizes: [ROAD_DESIGN](ROAD_DESIGN.md#road-paint).
- Source roads support 1..16 painted lanes. Optional `forward_lanes` preserves
  an asymmetric two-way split relative to the road points and the traffic side.
  Optional `median_width_cm` leaves the central median unpainted, outside lane
  space; stop lines and arrows use each direction's own lane count. Omitted
  values preserve the existing split and no-median appearance.
- `RoadMarkings.lane_width_cm` (optional, 0 omitted, else 1..1000 map cm) keeps
  every painted lane that wide, centred on the road (clamped to its width). Lane
  dividers, the center line, stop lines, arrows and wheel-track wear stay inside
  that band; with more than 12 cm left per side the edge lines bound the band and
  the remainder is shoulder. Omitted, the lanes share the road width and edge lines
  follow the paint borders.
- Urban asphalt (`urban_surface.gdshader`) adds broad tonal blotches, light
  aggregate, seeded repair patches and sparse domain-warped cracks from world
  position; marked roads darken and polish two wheel tracks per lane, and paint
  wears as described in [ROAD_DESIGN](ROAD_DESIGN.md#road-paint). Concrete carriageways
  vary per slab; sidewalk paving, curbs and gutters are in
  [ROAD_DESIGN](ROAD_DESIGN.md#sidewalks-and-curbs).
  Small features fade by 24 m and all variation fades with the effect weight;
  colours, geometry and contact surfaces are unchanged.

## Safety solids

- Every elevated/bridge exterior, including an unconnected deck end, gets 80 cm
  safety furniture outside the carriageway. Bridges: 20×20 cm base plus two metal
  rails; elevated roads: two metal rails. 8 cm posts every 200 cm, stationed
  before cell clipping. Tunnel/underpass portals stay open; there is no jump opening.
- Visible rail/post/base pieces are the same convexes used for collision and
  occupancy. IDs end in `:safety:metal` or `:safety:base` and are reserved. They are
  not spawn/recovery surfaces; metal is a renderer material only.
- Manual facilities are preserved; intersections fail with road, facility and
  position diagnostics. Sloping/diagonal pieces use SAT after AABB checks. Placement
  boxes and custom convexes use their full authored yaw and the same rounded
  vertices as generated collision.

## Work and memory

- Spatial/regional influence includes curve mouths and outer furniture. Costs
  include convex/occupied pieces, rendered triangles, IDs and paint uniforms.
  Cancellation, local plan budgets and paint work ticks reject excess work; paint
  over the 128-segment shader cap only drops that road's paint in the cell.
- Placement validation builds one transient road bounds/endpoint index per
  document; courtyard checks compare corridor polygons directly with building
  rings. Buildings above tunnels require 50 cm between their base and every
  overlapping ceiling face; clipping the shared plan retains safe buried
  interiors without ignoring a rising portal or junction fan. The 8 million work limit (enough for about 11,000 dense 1:8 city
  buildings) and exact contact rules stay authoritative.
- Sidewalk candidates from a spatial-index leaf are filtered by their own bounds
  before polygon work. A rejected box costs one bounded comparison; nearby aprons
  keep the same clipping order, terrain-conforming tops and curb boundaries.
- Terrain tiles, surface overlap and face classification query ordered spatial
  indexes; dense arrangements, or faces beyond the 16-hole/512-vertex triangulation
  limit (many disjoint islands in one tile), split into quarters (depth ≤6) only at
  that local limit. Exact predicates, surface order and cumulative budgets are
  unchanged. Touching arrangement holes attach at the free interior sector of
  a shared vertex, including when a visibility bridge duplicates that vertex;
  triangulation preserves their exact integer area.

## Validation

`tests/road_safety.rs` (decks, mouths, dead ends, layered crossings, seams,
archives, cancellation, reserved IDs) and the Godot `road_safety_validator`
(`MAPKIT_ROAD_CAPTURE` optionally writes an image). Renderer-drawn traffic
signals (horizontal ones with their road-name boards) and bus shelters borrow the
shared signal set
([RICHER_MATERIALS](RICHER_MATERIALS.md#traffic-signals-and-bus-shelters),
`street_signal_validator`); road signs and overhead guide signs the shared road-sign
set ([RICHER_MATERIALS](RICHER_MATERIALS.md#road-signs-and-guide-signs),
`road_sign_validator`); shrubs and city trees borrow the
shared park vegetation set ([RICHER_MATERIALS](RICHER_MATERIALS.md#park-vegetation)),
streetlights and benches the shared street furniture set
([RICHER_MATERIALS](RICHER_MATERIALS.md#street-furniture), `street_furniture_validator`),
subway entrances and station signs the shared transit set
([RICHER_MATERIALS](RICHER_MATERIALS.md#transit-entrances-and-station-signs), `transit_detail_validator`),
street trees and flower beds the shared street planting set
([RICHER_MATERIALS](RICHER_MATERIALS.md#street-trees-and-flower-beds), `street_planting_validator`),
and city streetlights the shared city lamp set, with the night lamp map on
road and sidewalk materials
([RICHER_MATERIALS](RICHER_MATERIALS.md#city-streetlights), `city_lamp_validator`).
A cell's display lease tracks only their nodes, MultiMeshes and instance buffers
and the cell's lamp-map texture, so it retires with the cell (`street_detail_lease_validator`). `road_safety_probe` checks
public regional examples. `reseal_courses` copies course definitions onto a new map
identity without completion evidence and never overwrites. Vector status:
[DETERMINISM](../spec/DETERMINISM.md). Vehicle driving and appearance are user checks.
