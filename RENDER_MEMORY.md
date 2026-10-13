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
Claims precede import; the default cap is 256 entries/128 MiB including shaders.
Each model imports once per cache entry. Texture context, GLB import, each LOD input
surface, LOD generation, output upload, each material binding, occluder preparation,
MultiMesh setup and placement are separate admitted steps. Concurrent claims share
preparation. Diagnostics report phase total/peak and attachment/release costs;
synchronous engine calls cannot be preempted.
With a ready context, GLB import captures opaque parameters into shaderless
bindings before scene upload. Original materials retire within import without
preparing unused shaders. Each unpublished surface activates the shared shader in
its admitted step. Colours, texture channels/UVs, light roles and transparency
survive; pending bindings retain full template/context leases through cancellation.
Private LOD outputs bind materials directly; borrowed meshes/previews keep binding
copies. Placements use complete templates; shutdown rejects partial access.
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

Repeated opaque static models use cell-local MultiMesh instances. Hierarchical
transforms, mesh surfaces, material overrides, original colours and shadow mode
survive. Transparent models, overlays and per-surface overrides use ordinary
instances. ImporterMesh LOD indices are generated once per cached template. Only assets without collision at/above their local origin may enter the small
decoration distance tier (buried root beds remain decorative). The validated
`solid` presentation flag and cache key preserve small fences, walls and props in
near and distant views at every preset. Decorations use 8 m distance hysteresis; structures
remain visible until normal cell/frustum retirement. MultiMesh bounds include only
populated transformed instances, not unused capacity, with an 8 cm margin for leaf
motion. Leaf faces never act as static occluders. Actual upright opaque
triangles (at most 64, inset 1 cm) form occluders; an AABB never fills an opening.
Immutable templates prepare each shape once after LOD/material binding, caching
empty results too. Placements own occluder nodes/transforms/visibility; mutable
standalone meshes derive faces on demand. Preparation is separately admitted within
the template allowance and has its own phase total/peak.

Opaque shared rendering emits each multi-material mesh once. Single-object groups
also use MultiMesh custom data; exceptional material overrides get an owned seed
binding while sharing shaders/textures, so there is no global per-instance shader
slot ceiling. Small-prop distance classification includes authored root scale.

## Lifetimes

The cell lease subtracts only the separately admitted shared asset allowance.
Shared resources have their own lease. Instance nodes/MultiMeshes and private
surface meshes/materials belong to the cell. Worker/native source owners remain
independent. The cache retains the import peak until actual resource retirement.
Fresh generated opaque bindings receive quality when their node is tracked, in
the same admitted call before attachment. Reused cell bindings still receive live
quality on every attachment; direct material factory results are ready for use.
Canopy claims supplement the per-object allowance. Shared meshes do not hold cell
leases: other trees and the warm cache cannot prevent a dead cell's retirement.
Evicted copies stay charged through the last mesh/importer borrower.

Claims pin templates during attachment. Completion/cancel releases pins exactly
once and clears source records. Unpinned templates may be evicted; their meshes,
materials/textures remain charged while scene instances or external consumers
borrow them. An independently retained occluder shape also keeps the shared
template lease charged after its mesh and instances retire. Cache shutdown seals
these leases, including the shared shader.
The lease provider enforces last-borrower retirement. Completed/failed jobs may
retain an invalid root handle for diagnostics.

`MapKitBridge.cell_window` and estimates provide a conservative horizontal visual
anchor margin, including transformed GLB bounds independently of collision proxies.

`scripts/verify_godot_layout.py --godot <Godot> --probe renderer --probe assets
--probe binding` checks relocation, hashes, assets and batching.
Run `display_quality_validator` and `richer_material_validator` with a rendered
engine for MultiMesh transforms, compressed-GLB occlusion, openings and excluded
faces. `mesh_preparation_validator` compares full geometry/LOD/material output,
early source retirement, surface admission, interleaved claims and immutable inputs.
`tree_canopy_validator` covers geometry/LOD, leaf uniforms and same-frame pixels at
each preset, independent placements/bindings, cancellation, pressure and borrowers.
These and caller checks cover admission, cancellation, failed replacement,
shutdown, scratch release and final mesh/context/shape borrowers.

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
