# Incremental renderer resource ownership

`chunk_renderer.begin(chunk, parent, reserve, planned_bytes, resources)` accepts a
session `render_resource_cache.gd`. Shorter calls and `attach` support previews;
callers own game admission.

## Planning and shared resources

`render_memory.gd` plans without GPU allocation. `supports_batches` excludes image
proxies needing individual alpha/order. `prepare_batches` groups opaque faces by
material and 32 m bucket, at most 512 triangles, preserving geometry/UVs/winding
and hidden-proxy exclusion. Packed buffers cost 96 bytes/visible triangle plus
bounded metadata; callers admit source/preparation before worker execution.
Check support before using empty batch results.

`estimate` reserves 64 KiB/job (+64 KiB urban), 768 bytes/visible triangle,
12 KiB/batch, 64 KiB/object and validated asset allowances. Hidden proxies have
no visible batches. These are logical bounds, not measured allocations or compressed
sizes. `upper_bound` precedes generation; exact plans follow worker completion
before rendering.

Current v1 presentation records carry validated `content_hash` and `memory_bytes`,
excluded from package serialization/generated hashes. Cache keys cover content,
declarative materials, referenced images and the asset's light binding. Each claim
snapshots that binding and a read-only role lookup, isolating later profile/list
edits. Windows override bulbs; material metadata overrides surface indices.
Unrelated/absent bindings stay independent; previews read current bindings.
Completion, failed import and eviction discard this metadata. The existing
material/clock allowance covers up to 64 window and 16 bulb indices.

Claims precede import; the default cap is 256 entries/128 MiB including shaders.
Concurrent claims share preparation. Context setup, import, each LOD input surface,
LOD generation/upload, each material binding, occluders, part extraction and
MultiMesh setup/placement are separately admitted. Tile decode and mipmap/upload
also yield separately. Synchronous engine calls cannot be preempted;
[diagnostics](docs/RENDER_DIAGNOSTICS.md) report phase total/peak and attachment/release costs.
Observers cannot drain preparation or expose partial contexts/templates.

With a ready context, import captures opaque parameters in shaderless bindings
and retires originals before upload. Each surface activates the shared shader in
its admitted step, preserving colours, texture channels/UVs, roles and transparency.
Pending bindings retain template/context leases through cancellation. Private LOD
outputs bind directly; borrowed meshes/previews copy. Node classification reads
bounds once; material steps share the final surface count until completion/eviction.
Import tracks the hierarchy, overrides, overlays and source bitmaps. LOD output and
shallow copies retain bindings, so replacement tracks only the new mesh. Styling
reports binding replacement: new bindings are tracked; shader activation and
transparent passthrough retain import tracking. The context tracks the shared
shader. Mutable preview tracking remains unconditional.

LOD jobs retain decoded positions/indices through occluder preparation, avoiding
another GPU readback. Raw GLTF arrays precede compression and cannot substitute
without changing geometry/LOD. Scratch uses the doubled asset allowance; eviction
clears it before sealing the full lease. Partial resources stay charged until retirement.

Trees share a material-free canopy per session: 128 KiB covers sphere/importer
scratch, CPU/GPU output and LOD indices. Creation, readback, LOD generation and
upload are separately admitted. Nodes, transforms, visibility and leaf bindings
stay independent. Fresh leaves receive detail/wind and node quality once before
publication; live updates remain unconditional. Cell-owned bindings retain shared
shader/textures through the last borrower. Previews prepare locally.

Opaque static models use cell-local MultiMeshes, preserving hierarchy, surfaces,
overrides, colours and shadows. GLB capacity/group lookup excludes built-ins, image
proxies and previews; shared group lookup follows template preparation. After
LOD/material/occluder work, immutable templates publish read-only part lists,
including empty fallback results. Groups borrow tracked meshes/materials within
the doubled native node allowance. Eviction clears lists; partial/closed caches
expose none. Mutable previews inspect parts at setup and refresh bounds at placement.
Shared parts cache mesh-local AABBs. Composed transforms precede bounds expansion;
only populated instances enter merged bounds, with 8 cm leaf-motion margin.

Each setup shares one quality snapshot. Each placement computes its seed once and
reads occluder quality once if needed; subsequent calls refresh quality. Transparent
models, overlays and surface overrides use ordinary instances with independent
mesh bindings, one hierarchy seed and one quality snapshot for occluders/tree
application. Both paths reuse visual transforms for lamps: rotate authored cm
offsets, convert to metres, then translate. Each multi-material mesh emits once;
single-object groups use MultiMesh custom data. Exceptional overrides use private
seed bindings with shared shaders/textures, avoiding instance shader slot limits.

LOD indices are generated once per template. The validated `solid` flag/cache key
preserves collidable fences, walls and props at every preset. Only assets without
collision at/above their origin may use decoration distance (buried root beds
qualify), with 8 m hysteresis. Classification includes authored root scale.
Structures retire with the cell/frustum. Occluders use actual upright opaque
triangles (at most 64, inset 1 cm), excluding leaves and preserving openings.
Immutable templates prepare shapes, including empty results, once after binding
in a separate admitted phase within their allowance. Placements own occluder
nodes/transforms/visibility; mutable previews derive current faces on demand.

## Lifetimes

Cell leases subtract only separately admitted shared allowances. Instance
nodes/MultiMeshes and private meshes/materials belong to the cell; shared resources
and worker/native sources have independent owners. Import peak stays charged until
retirement. Canopy claims supplement per-object allowances; shared meshes never
hold cell leases. Evicted copies remain charged through the last mesh/importer borrower.

Fresh generated opaque and urban/RC bindings receive quality when their node is
tracked before attachment. Reused bindings receive live quality on every attachment;
direct factory results are ready. A synchronous traversal shares one quality
snapshot, including new occluders, and refreshes next call. Lease callbacks only
record ownership. Node/material updates precede tracking; occluders precede child traversal.

Claims pin templates during attachment. Completion/cancel releases pins once and
clears source records. Unpinned templates can be evicted; independent mesh,
material, texture and occluder borrowers retain charges. Shutdown seals leases,
including the shader; the provider enforces final-borrower retirement. Completed/
failed jobs may retain invalid root handles for diagnostics.

`MapKitBridge.cell_window` and estimates include a conservative horizontal visual
anchor margin from transformed GLB bounds, independently of collision proxies.

`scripts/verify_godot_layout.py --godot <Godot> --probe renderer --probe assets
--probe binding` checks relocation, hashes, assets and batching. Rendered
[material validators](docs/RICHER_MATERIALS.md#display-quality) cover output,
phased preparation and previews. Ownership regressions cover bitmap/overlay
tracking, replacement meshes, shader activation, cancellation and independent
borrowers. Consumers check admission, failed replacement, pressure and shutdown.

## Unshareable presentation resources

Views without validated content hashes and positive memory bounds retain full
cell reservations and job-local importers; caches cannot subtract unproven allowances.

## Shared miniature material tiles

Weather reserves 4.75/16/61 MiB for ten 128/256/512 px tiles and CPU/GPU/mipmap/
retirement overlap, even without admission callbacks. All five source bitmap
slots join asset leases. See [materials](docs/RICHER_MATERIALS.md).

## LOD and occlusion admission

Before import/LOD work, templates reserve twice the native bound plus 128 KiB
for overlap/LOD indices. Batch/object allowances include occluder arrays/nodes
in `upper_bound`. Profile changes reuse textures; retirement grace stays charged.
Display quality never changes physics, AI, scheduling authority or network data.
