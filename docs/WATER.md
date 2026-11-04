# Current v1 water and arcade presentation

`MapDocument.water_bodies` is optional and empty by default. Each body has a unique
document-wide `id`, a simple `polygon`, zero or more dry `islands`, `surface_cm`,
`bottom_cm`, and horizontal map-space `flow_cm_s: [x,y]`. Heights and coordinates
are centimetres; Godot converts map y to negative scene z. The volume includes its
surface and bottom. The shoreline is wet; island boundaries are dry. Overlapping
volumes choose the highest containing surface, then the lexically first ID.

There are at most 1,024 bodies, 512 combined ring vertices per body and 16 islands.
Rings must stay inside map bounds and cannot self-intersect; islands cannot touch,
intersect, nest, or leave their parent. Bottom must be below surface, heights are
within ±1,000,000 cm, and flow components within ±1,000 cm/s. Existing global
document, geometry, cancellation and allocation limits also apply.

The bounds index selects cell/region candidates. Each generated cell retains the
complete body plus clipped, display-only surface triangles (at most 4,096 per
body/cell). Neither water triangles nor volumes enter solid collision geometry.
Actual terrain, shores and bridges retain their ordinary collision surfaces.
Spawn/surface queries reject submerged ground but permit islands and bridges.
The public `godot/water_query.gd` offers a horizontal `column` query and a bounded
`sample` query; atomic cell records deduplicate body IDs across ownership changes.

Source/chunk hashes, regional metadata, indexed audits and generated archives
include water. Packed Godot chunks carry `water_bodies_json`. The current archive
has a required length-prefixed water section following gimmicks. Conservative
`water_bytes` charges 32 KiB + 16 KiB per ring vertex per intersecting cell for
geometry, JSON/Variant copies, query records and renderer resources. Existing
consumer caps are unchanged. `water_renderer.gd` uses the existing bounded renderer
steps and compatibility shader (`water_surface_shared.gdshaderinc`, shared by near
and distant water): broad, restrained ripples; a view-depth tint from silt over a
shallow bed to muted teal (`1 − exp(−2.2 · depth m)`); a soft fresnel rim; soft
specular reflection; and foam only in the narrow band where the surface meets the
bank (`1 − smoothstep(0.01, 0.10, depth m)`). Physics and simulation clocks remain
the consumer's responsibility.

## Real-city earth basins

On real-city OSM documents (an `osm-extract-v1` notice recording a 1:D scale, as
for the 3 cm city curb in [ROAD_CONTACT](ROAD_CONTACT.md); no assembled track)
generated ground sinks into an earth bed under water. Authored
maps keep byte-identical chunks. Nothing in the document changes; this is
generation output only (formats stay v1).

- Levels: bodies with equal `surface_cm` and `bottom_cm` act as one water; an
  edge of one whose midpoint lies in another of the same level is a seam between
  import pieces, not a shore. Touching bodies of different levels keep a bank
  between them.
- Bank width `w = clamp(3 · (surface − bottom), 90, 240)` map cm (180 for the
  60 cm import depth). The bed is the body `bottom_cm`.
- A ground vertex at `p` with terrain height `g` inside water of one level and
  outside every keep-out, `d` its whole-cm distance to the nearest shore or
  keep-out edge and `k = surface − bottom + 8` (the bank fall per bank width),
  moves to `h = g − min(g − bottom, k · min(d, 1200) / w)` where the terrain lies
  above the bottom (integer, truncated): a bank at the flat-map slope (68 cm per
  180 cm, about 21°, so DEM banks stay climbable) that keeps falling past `w`
  until it meets the bottom, at most 1200 cm (the basin reach) from the shore,
  i.e. at most 453 cm under the terrain for the import depth. Terrain exactly
  `k` above the bottom (flat imports at their import levels) meets it at `w`
  with the same bytes as the earlier bank-width rule; terrain between the bottom
  and `k` above it (edited levels, DEM hollows) reaches the bottom before `w`,
  at the same flat-map slope. DEM
  terrain inside the polygon higher above the low water surface keeps falling
  under the water rather than stopping `k` under the terrain as a flat dry bed.
  Two limits remain: past the bank loop the fall rests on the terrain tile
  vertices only (see Plan view), so a face spanning from the loop to the next
  tile corner can stay dry above where the rule would put it under; and a
  narrow body (loop at 2/5 of its mean width, below `w`) never reaches the
  further fall, so a sloped stream whose terrain lies well above its single
  lowest-shore level stays a dirt trough with little or no water showing.
  Terrain at or below the bottom rises to it,
  `h = g − (g − bottom) · min(d, w) / w`. Shore, keep-out, mixed-level and dry
  vertices keep `g`. The shore query stops where the bank meets the bottom, so
  only vertices under terrain high above the water search the full reach. On a terrain tile edge (not a corner) a
  bed vertex instead lies on the edge's piecewise-linear profile through its
  corners and every basin line crossing (each sunk by the rule above on the edge's
  terrain), within about 1 cm of the rule. Heights are thus a function of the
  vertex (and its tile edge) alone: faces sharing a vertex never crack, and a
  vertex only one side of a tile edge has (a tile-local bridge-plane crossing, a
  split part's corner) lies on the other side's straight edge. Where
  terrain lies at or above the bottom the bed stays at or above it, so
  `WaterBody::contains` and spawn rejection of submerged ground hold there; every
  sunk vertex is at least the bottom.
- Keep-outs (ground stays on the terrain): ground, tunnel, underpass and designed
  road segments (half width + sidewalk + 2 cm), their ends and junctions (the
  plan's influence margin of the widest road there + sidewalk), bend mitres,
  barriers (half thickness + 2 cm), building footprints and authored placement
  footprints, each as an integer polygon (octagons widened by 1/12 + 1 cm) and
  only where it overlaps water. Each keep-out edge splits where it crosses a
  multiple of 1024 cm and keeps only the pieces within 240 cm of the bounds of a
  body it overlaps, so a long road wet at one end adds nothing to distant dry
  cells and every rule depends only on geometry a regional source keeps (water
  within 1501 cm of its cells, more than the 1200 cm reach): a keep-out piece can
  be a vertex's nearest edge only where it lies in water (the way from a vertex
  in water to a dry point crosses a shore, which is no farther), and that water
  and its keep-outs lie in the cell's source (a compile-time assertion in
  `region.rs` ties the source margin to the reach and the piece bound). Bridge and elevated decks are not keep-outs; road
  bridges have no piers. Repeated placements and buildings partly in water are
  not lowered.
- Plan view: water shores outside keep-outs, keep-out pieces and one closed bank
  loop per ring (every ring edge moved `w` inward, joined at crossings, near
  mitres or chords) enter the ground arrangement beside the paint lines. A body
  whose mean width (2 · area / perimeter, seams included, a property of the body
  alone) is under `2w` gets its loop at 2/5 of that width instead, so narrow
  streams still sink in the middle. Past the loop the bank's further fall under
  higher terrain lies on the terrain tile vertices (400 cm on the shipped DEM
  cards); no extra lines are added, so the plan view, the estimates and the basin
  memory do not depend on terrain. Count and Emit arrange identical lines, so the
  ground estimate stays an upper bound on flat and heightmap cells; a face whose
  holes cannot be bridged splits like an over-dense one. In a tile part whose
  bounds meet a body's bounds, every arrangement line (road plan, paint or basin)
  ending on the part boundary also splits that boundary edge.
- Ground faces whose centre lies in a bed are `dirt` and keep their `terrain` or
  paint ID, contact class 1 and spawnability. Roads, bridges, tunnels, sidewalks
  and their heights are unchanged (basin lines may add vertices to road faces).
- `Basins` is built exactly once per prepared document (concurrent first callers
  wait; a failure is kept) and unprepared generation builds it per call:
  placement work allowance per phase; construction peak and retained bytes at
  most `basin_reserve_bytes` (64 KiB + 800 B per non-string document byte, at
  most 8 MiB; 4,000 B per water ring vertex during construction), which both
  package admission paths charge. Basins retain about 450 B per water ring
  vertex, so water beyond roughly 13,000 ring vertices (the shipped cards have
  about 1,600 and 3,100) fails closed: `PreparedMap::new`/`new_region` and
  `pack_source` (whole map and every regional source) return `E_BUDGET` instead
  of failing every cell later.
  Point-in-water tests use an index over ring edges, so per-vertex cost does not
  grow with a body's vertex count; per-cell vertex heights charge the road work
  limit. Distant sources share the same ground.
- Display: the chunk renderer gives each near earth (`dirt`) mesh of a cell with
  water that cell's highest water surface (`shore_level_m` instance uniform), and
  the ground shader darkens earth below about 15 cm above it (×0.56 colour,
  roughness 0.34) as a wet bank; other ground and distant cells are unchanged.
- `mapkit_core::water::basin_report` reports components (the union of all
  bodies), how many sink, how many a keep-out trims, fully kept-out components and
  keep-out, seam and line counts.

Two optional environment fields support authored scenery: `start_minutes` (0–1439)
is an initial-time suggestion for the authority, and `ground_color: [r,g,b]` tints
terrain. Optional `RoadMarkings.color` tints a road's visible material. All colors
are bytes; they do not change collision, friction or weather authority.
`assets/arcade-library` contains original MIT scenery, reproducible with
`scripts/arcade_assets.py`; no game code or external datasets are included.

Validation: `crates/mapkit-core/tests/water.rs` (basins: watertight, unsunk
shores, bed bound, bank past the bank width and the reach cap, earth faces,
roads, levels, regional sources, a bank shaped by a keep-out in another cell),
`crates/mapkit-core/tests/cost.rs` (random basins within estimates),
`crates/mapkit-core/tests/chunk_hash.rs` (authored hashes frozen),
`crates/mapkit-core/tests/determinism.rs` (prepared basins across threads),
`crates/mapkit-package/tests/water_contract.rs` and
`crates/mapkit-package/tests/basin_reserve.rs` (measured allocation under the
admission reserve). Distant water display follows
[DISTANT_RENDERING](DISTANT_RENDERING.md#authored-distant-meshes-and-water).
