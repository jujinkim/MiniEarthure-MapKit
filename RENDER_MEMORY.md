# Incremental renderer resource ownership

The fifth argument to `chunk_renderer.begin(chunk, parent, reserve, planned_bytes,
resources)` is a session `render_resource_cache.gd` context. Shorter calls
and synchronous `attach` support standalone previews. Callers own game admission.

## Planning and shared resources

`render_memory.gd` plans validated output without allocating GPU resources.
`supports_batches` excludes image proxies whose alpha/order needs individual rendering.
`prepare_batches` groups opaque triangle surfaces by material and 32 m spatial bucket into at most 512
triangles, preserving coordinates, normals, UVs, winding and hidden-proxy exclusion.
Its packed buffers cost 96 bytes per visible triangle plus bounded grouping metadata.
The caller must account for source and preparation buffers before worker execution.
An empty batch list must not replace an unsupported source; use `supports_batches`.

`estimate` reserves 64 KiB per job (another 64 KiB for urban surfaces), 768 bytes per
visible triangle, 12 KiB per batch, 64 KiB per object and the validated asset allowance.
Hidden proxies allocate no visible batches. These are logical bounds, not measured
allocations. `upper_bound` applies before generation; exact plans are admitted after
worker completion and before rendering. Compressed length is not a RAM estimate.

Current v1 native presentation records add `content_hash` and `memory_bytes` from the
validated package. They are excluded from package serialization/generated hashes.
Cache keys include content, declarative materials and referenced image content.
Each new asset claim snapshots the light binding used by its key. Import and all
mesh material steps reuse that snapshot even when another cell replaces the current
profile or authored lists change. Unrelated assets and absent bindings remain
independent. Completion, failed import and eviction discard this preparation-only
metadata within the existing asset allowance.
Claims precede import; the default cap is 256 entries/128 MiB including shaders.
Concurrent claims share one import/preparation per entry. Texture context, import,
each LOD input surface, LOD generation/upload, each material binding, occluder
preparation, instance-part extraction and MultiMesh setup/placement are separate admitted steps.
Synchronous engine calls cannot be preempted; [diagnostics](docs/RENDER_DIAGNOSTICS.md)
report phase total/peak and attachment/release costs.
With a ready context, GLB import captures opaque parameters into shaderless
bindings before upload and retires original materials. Each unpublished surface
activates the shared shader in its admitted step. Colours, texture channels/UVs,
light roles and transparency survive. Pending bindings retain full template/context
leases through cancellation. Private LOD outputs bind directly; borrowed meshes
and previews copy bindings. Placements require complete templates.
LOD jobs retain decoded positions/indices until occluder preparation, avoiding a
second GPU readback with identical faces. Raw GLTF arrays precede compression and
cannot replace decoded inputs without changing geometry/LOD. Scratch uses the
doubled asset allowance; eviction clears it before sealing the full template lease.
Tile decode and mipmap/upload use separate steps. Observers cannot drain preparation
or read a partial context. Partial resources retain the full lease until retirement.

Built-in trees share an immutable material-free canopy per session cache. Its
128 KiB claim covers sphere/importer scratch, CPU/GPU output and LOD indices.
Creation, readback, LOD generation and upload are separately admitted. Nodes,
transforms, visibility and final leaf bindings remain independent. Fresh leaves
receive detail/wind quality once with node LOD/distance before publication in the
same call; live updates remain unconditional. Bindings use the cell allowance;
their last borrower retains shared shader/textures. Previews prepare locally.

Opaque static models use cell-local MultiMesh instances, preserving hierarchy,
surfaces, overrides, colours and shadows. Capacity counts cover GLB placements;
spatial group lookup occurs after template preparation in shared attachment.
Built-in objects, image proxies and standalone previews need no group keys.
Immutable templates extract a read-only
part list after all LOD/material/occluder work, including empty fallback results.
Spatial groups borrow that list without traversing the hierarchy again. Records
borrow tracked meshes/materials within the doubled native node allowance and are
cleared on eviction; partial or closed caches expose no parts. Mutable standalone
templates are inspected on every setup. Shared parts retain mesh-local AABBs for
setup/placement; composed transforms still precede bounds expansion and only placed
instances enter the merged bounds. Mutable preview meshes refresh bounds at placement.
Each setup shares one quality snapshot; each
placement computes its object seed once and reads occluder quality once if needed.
The next call reads current quality again. Transparent models, overlays and
per-surface overrides use ordinary instances. Their hierarchy shares one object
seed calculation while each mesh keeps its independent material bindings.
Each ordinary placement shares one current quality snapshot between occluder
creation and final tree application, refreshing it on the next admitted call.
Mounted lamps reuse the visual placement transform in both paths; authored
centimetre offsets are still rotated before metre conversion and translation.
LOD indices are generated once per
template. The validated `solid` flag/cache key preserves collidable fences, walls
and props at every preset. Only assets without collision at/above their origin
may use decoration distance (buried root beds qualify), with 8 m hysteresis.
Structures retire with the cell/frustum. MultiMesh bounds include only
populated transformed instances, not unused capacity, with an 8 cm margin for leaf
motion. Leaf faces never act as static occluders. Actual upright opaque
triangles (at most 64, inset 1 cm) form occluders; an AABB never fills an opening.
Immutable templates prepare each shape once after LOD/material binding, caching
empty results too. Placements own occluder nodes/transforms/visibility; mutable
standalone meshes derive faces on demand. Preparation is separately admitted within
the template allowance and has its own phase total/peak.

Each multi-material mesh is emitted once. Single-object groups also use MultiMesh
custom data; exceptional overrides get a private seed binding with shared
shaders/textures, avoiding per-instance shader slot limits. Distance classification
includes authored root scale.

## Lifetimes

The cell lease subtracts only the separately admitted shared asset allowance.
Shared resources have their own lease. Instance nodes/MultiMeshes and private
surface meshes/materials belong to the cell. Worker/native source owners remain
independent. The cache retains the import peak until actual resource retirement.
Fresh generated opaque and urban/RC bindings receive quality when their node is
tracked, in the same admitted call before attachment. Reused cell bindings still
receive live quality on every attachment; direct material factory results are ready for use.
Resource tracking takes one quality snapshot for its synchronous node traversal,
including newly created occluders, and reads it again on the next call. Lease
callbacks only record ownership; they must not change display policy. Nodes and
materials are still updated before tracking, with occluders created before child traversal.
Canopy claims supplement the per-object allowance. Shared meshes do not hold cell
leases: other trees and the warm cache cannot prevent a dead cell's retirement.
Evicted copies stay charged through the last mesh/importer borrower.

Claims pin templates during attachment. Completion/cancel releases pins once and
clears source records. Unpinned templates may be evicted; meshes, materials,
textures and independently retained occluder shapes keep leases charged through
their last borrower. Shutdown seals these leases, including the shared shader.
The lease provider enforces last-borrower retirement. Completed/failed jobs may
retain an invalid root handle for diagnostics.

`MapKitBridge.cell_window` and estimates provide a conservative horizontal visual
anchor margin, including transformed GLB bounds independently of collision proxies.

`scripts/verify_godot_layout.py --godot <Godot> --probe renderer --probe assets
--probe binding` checks relocation, hashes, assets and batching. The rendered
[material validators](docs/RICHER_MATERIALS.md#display-quality) cover node quality,
geometry/LOD/material output, phased preparation and shared borrowers. Caller checks
also cover part order, nested transforms, multi-surface/override fallback, empty
result reuse, composed/merged bounds, mutable previews, claimed light roles across
profile changes and final mesh/material retirement. Consumer checks
cover admission, cancellation, failed replacement, pressure, shutdown and final
mesh/context/shape retirement.

## Unshareable presentation resources

Views without validated content hashes and positive memory bounds retain their
full per-cell reservation and job-local importer ownership. A supplied session
cache cannot subtract an unproven shared allowance. This is a resource-accounting
rule within current v1.

## Shared miniature material tiles

The weather context reserves 4.75/16/61 MiB for ten 128/256/512 px tiles, mipmaps
and CPU/GPU/retirement overlap, including diagnostics without admission callbacks.
All five bitmap slots join asset-lease tracking. See [materials](docs/RICHER_MATERIALS.md).

## LOD and occlusion admission

Before import/LOD generation, each template reserves twice the native asset bound
plus 128 KiB for source/output overlap and LOD indices. Batch/object allowances
above include occluder arrays/nodes and apply to `upper_bound`. Profile changes reuse
map textures. Consumer retirement grace remains charged; display quality does not
affect physics, AI, scheduling authority or network data.
