# Distant display products

The package's generated/collision archive, recipe and hash remain unchanged.
`Package::generate_distant(Cell)` uses `PreparedMap::generate_distant_source` and
converts its transient `DistantSource` to a visual-only `DistantMesh`.
`distant_from_generated` also accepts an existing ordinary generated chunk. Both
paths produce identical ordered vertices, colours, light data and display kinds.
The source reuses the same geometry and clipping but omits building-prism and
convex sidecars, contact classification and proxies discarded by the conversion.
Water and gimmick exclusions still govern vegetation placement. Authored GLBs,
generated objects and neighbouring-cell proxies keep their existing ownership rules.
`DistantSource` exposes only triangles, objects and water; it has no serialization,
hash, archive or collision-query API. Ground, roads and native building geometry retain
original coordinates and boundaries. GLB collision proxies are omitted and
replaced by coloured bounds grouped per scene mesh/material, including node and
placement transforms. Built-in trees use a canopy volume. No collision or full
GLB/texture instance is produced. The traversal is iterative. Building material lookup uses normalized ID order;
asset lookup uses the prepared index. Authored silhouette bounds and small-prop
classification are computed once per asset template within each conversion.

Native buildings emit closed triangular prisms. The distant conversion removes
exactly paired, opposite internal vertical quads, even when their two triangles
use different diagonals. It keeps source order and coordinates for every surviving
face. Roofs, exterior walls, courtyard openings and cell-cut faces remain intact;
ambiguous coincident groups remain visible. Authored landmark silhouettes and
collision geometry are independent of this reduction. No cross-cell dependency
is introduced. Native face matching reserves 128 scratch bytes per estimated
source triangle before generation; documents without native buildings require
no matching workspace. Triangle/output bounds remain conservative upper limits.

`distant_triangle_bound` supplies a conservative pre-generation triangle bound.
The Godot bridge exposes `estimate_far_chunk(x,y)` (ordinary generation and asset
workspace estimates plus `distant_triangles`) and `generate_far_chunk(x,y)`.
The latter returns an immutable `MapKitFarGeometry` owner with a COW `view()` of
packed vertices/normals/colours, actual triangle count and retained/display byte
allowances. Callers must retain/track the owner while using its packed arrays.
They reserve source, generation, output and display space before generation.

`godot/distant_renderer.gd` uses incremental 512-triangle ArrayMesh batches,
vertex-colour materials and no shadows. `begin` receives a caller-owned display
lease with `track(Object)`; the root stays hidden until the caller observes
completion and authorizes display. The renderer tracks the native owner, root,
materials and meshes. `cancel` frees its references and root; the caller seals the
lease and retires it after the actual last borrower. Camera range, priority,
retention, memory policy and gameplay ownership belong to the consumer.

The ordinary chunk renderer also exposes `display_lease` after completion so a
consumer can transfer an already visible root without allocating new resources.
No public package format or generated-data identity changes are required.

Validation: for distant conversion changes, run package targets `distant`,
`distant_assets`, `distant_source` and the affected regional `indexed` checks.
Core source-generation changes also require core `distant_source` and affected
generation/placement/occupancy targets. Compare every display array in order, frozen ordinary and display
digests, roof/courtyard and cell boundaries, reflected/rotated GLBs and openings,
repeated placement, water/islands, vegetation exclusions, output limits and cancellation.
The ignored `short_distant_measurement` test provides bounded synthetic dense-building,
reused-GLB and open-terrain timings, separating load, estimate, generation and
conversion with first/repeated passes. It is an explicit local measurement, not a
wall-clock test threshold. Consumer checks cover far/detail streaming, failed or
cancelled replacements, stale ACKs, pressure and last-borrower retirement.
Native internal-face removal reduces distant display triangles without changing
the exterior; its effect on renderer batches depends on the cell's total output.

The bridge also reports `visual_margin_cm` from validated transformed model bounds
in `cell_window` and cost metadata. The regional reader publishes the same extent
from its complete audit before display use; position queries do not wait on region I/O. Consumers can choose full detail using the
nearest visual extent instead of the cell centre. Full-detail caching and worker
batch planning are documented in [RENDER_MEMORY.md](../RENDER_MEMORY.md). No
specific distance, physics interest, admission cap or gameplay ACK is imposed by
MapKit. Renderer roots carry diagnostic `mapkit_render_root` metadata so a caller
can count cell resources independently of scene-generated node names.

## Authored distant meshes and water

- An `Asset` may declare `distant_path`, a second safe package-relative `.glb` using
  the primary model's origin, metre scale and placement transform. Both files join
  package references, manifest hashes, byte counts, validation workspace and memory
  estimates. Only the primary asset defines collision; display LOD never changes
  surface probes, collision, execution cells or admission budgets.
- The distant model emits its authored triangles, node hierarchy, materials and
  vertex tint; quarter turns, yaw and reflected node transforms apply once. Assets
  without one keep the generic proxy.
- Packed distant colours are sRGB: linear GLTF PBR/vertex colours convert once;
  explicit sRGB overrides keep their values. Emissive signs keep their hue in both
  LODs and follow the daylight schedule.
- Small authored props form a contiguous batch tail under the quality distance
  limit. Workspace and triangle bounds are charged before allocation.
- Packed display kind byte: 0 solids, 1 small-prop tail, 2 water. The two UV
  channels carry flow for water and light identity/role for solids. Distant water
  uses the same quality-selected shader as near water (depth, refraction, flow) and
  keeps clipped shorelines/islands without physical triangles. Batches split on kind
  and share at most two job materials within the fixed 64 KiB far overhead.
- Terrain uses the same broad colour variation in near and distant shaders.
