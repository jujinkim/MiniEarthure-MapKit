# Incremental renderer resource ownership

`chunk_renderer.begin(chunk, parent, reserve, planned_bytes, resources)` accepts a
session `render_resource_cache.gd`. Shorter calls and `attach` support previews;
callers own game admission.

## Planning and shared resources

`render_memory.gd` plans without GPU allocation. `supports_batches` excludes image
proxies needing individual alpha/order. `prepare_batches` groups opaque faces by
material and 32 m triangle-bounds-centre bucket, at most 512 triangles, preserving
geometry/UVs/winding
and hidden-proxy exclusion. Packed buffers cost 96 bytes/visible triangle plus
bounded metadata; callers admit source/preparation before worker execution.
Check support before using empty batch results.

`estimate` reserves 64 KiB/job (+64 KiB urban), 768 bytes/visible triangle,
12 KiB/batch, 64 KiB/object and validated asset allowances. Hidden proxies have
no visible batches. These are logical bounds, not measured allocations or compressed
sizes. `upper_bound` precedes generation; exact plans follow worker completion
before rendering.

Presentation-only `content_hash` and `memory_bytes` stay outside v1 serialization/
generated hashes. Keys cover content, declarative materials, images and the asset's
light binding. Claims snapshot bindings/read-only roles against profile/list edits;
unrelated/absent bindings stay independent. Windows override bulbs; material metadata
overrides surface indices. Previews read current bindings. Completion/failure/eviction
discard snapshots; the material/clock allowance covers 64 window and 16 bulb indices.

Claims precede import (256 entries/128 MiB cap including shaders) and share preparation.
Context setup, import, each LOD input surface, LOD generation/upload, each binding,
occluders, parts and MultiMesh setup/placement have separate admission. Tile decode
and mipmap/upload also yield separately. Engine calls cannot be preempted;
[diagnostics](docs/RENDER_DIAGNOSTICS.md) report phase total/peak and attachment/release costs.
Observers cannot drain preparation or expose partial resources.

With a ready context, import captures opaque parameters in shaderless bindings
and retires originals before upload. Each surface activates the shared shader in
its admitted step, preserving colours, texture channels/UVs, roles and transparency.
Pending bindings retain template/context leases through cancellation. Private LOD
outputs bind directly; borrowed meshes/previews copy. Node classification reads
bounds once; material steps share the final surface count until completion/eviction.
Import tracks the hierarchy, overrides, overlays and source bitmaps. A local
visited set scans each shared mesh's materials and each binding's texture slots
once; node-specific overrides remain independent, and each new traversal reads
current resources. LOD/shallow copies retain bindings, so replacement tracks only
the new mesh. Surface and node-override styling assign and track replaced bindings;
activation/transparent passthrough retain their bindings and reuse import tracking.
The context tracks the shared shader. Mutable previews track unconditionally.

Image/declarative lookups reuse initial tracking. GLB import tracks cached overrides
even before hierarchy rejection. Failed creation remains retryable; entries own
their bindings, and independent bitmaps retain the charge.

LOD jobs retain decoded positions/indices through occluder preparation, avoiding
another GPU readback. Raw GLTF arrays precede compression and cannot substitute
without changing geometry/LOD. Scratch uses the doubled asset allowance; eviction
clears it before sealing the full lease. Partial resources stay charged until retirement.

Trees share a material-free canopy: 128 KiB covers sphere/importer scratch, CPU/GPU
output and LODs. Creation, readback, LOD generation and upload have separate admission.
Nodes, transforms, visibility and leaf bindings stay independent. Fresh leaves receive
detail/wind/node quality once before publication; live updates remain unconditional.
Cell bindings retain shared shader/textures through the last borrower. Previews prepare locally.

Opaque static models use cell-local MultiMeshes, preserving hierarchy, surfaces,
overrides, colours and shadows. A node override is classified once while every
per-surface override still triggers the ordinary-instance fallback.
Capacity/group lookup excludes built-ins, image
proxies and previews; shared lookup follows preparation. Final templates publish
read-only parts, including empty fallbacks. Groups borrow tracked meshes/materials
within the doubled native node allowance. Eviction clears lists; partial/closed
caches expose none. Previews inspect parts at setup and refresh bounds at placement.
Shared parts cache mesh-local AABBs. Composed transforms precede bounds expansion;
only populated instances enter merged bounds, with 8 cm leaf-motion margin.

Setup shares one quality snapshot; placement computes one seed and reads occluder
quality if needed. Calls refresh quality. Transparent/overlay/surface-override
fallbacks use independent mesh bindings with one hierarchy seed/quality snapshot.
Both paths reuse visual transforms for lamps: rotate cm offsets, convert to metres,
translate. Multi-material meshes emit once; single-object groups use MultiMesh
custom data. Exceptional overrides use private seed bindings and shared
shaders/textures, avoiding instance shader slot limits.

LOD indices are generated once per template. The validated `solid` flag/cache key
preserves collidable fences, walls and props at every preset. Only assets without
collision at/above their origin may use decoration distance (buried root beds
qualify), with 8 m hysteresis. Classification includes authored root scale.
Structures retire with the cell/frustum. Occluders use actual upright opaque
triangles (at most 64, inset 1 cm), excluding leaves/water/unproven shader opacity
and preserving openings. Bounded samples span surfaces; selection retains direction/
height coverage and favours larger faces rather than the first imported faces.
Transparent node/surface overrides prevent ordinary-instance occluders; mutable
preview overrides also hide existing shapes when quality is reapplied.
Immutable templates prepare shapes, including empty results, once after binding
in a separate admitted phase within their allowance. Placements own occluder
nodes/transforms/visibility; mutable previews derive current faces on demand.

## Lifetimes

Cell leases subtract separately admitted shared allowances. Instance nodes/MultiMeshes
and private meshes/materials belong to cells; shared resources and worker/native sources
have independent owners. Import peak/copies stay charged through the last
mesh/importer borrower. Canopy claims supplement per-object allowances;
shared meshes never hold cell leases.

Fresh generated opaque/urban/RC bindings receive quality at node tracking before
attachment; reused bindings update every attachment. Direct factory results are ready.
Traversal shares one quality snapshot including new occluders, refreshing next call.
Lease callbacks only record ownership. Node/material updates precede tracking;
occluders precede child traversal.

Claims pin templates until completion/cancel releases pins and source records.
Unpinned templates can be evicted; independent mesh/material/texture/occluder borrowers
retain charges. Shutdown seals leases including shaders; the provider enforces
final-borrower retirement. Completed/failed jobs may retain invalid diagnostic roots.

`MapKitBridge.cell_window` and estimates include a conservative horizontal visual
anchor margin from transformed GLB bounds, independently of collision proxies.

`scripts/verify_godot_layout.py --godot <Godot> --probe renderer --probe assets
--probe binding` checks relocation/hashes/assets/batching.
[Material validators](docs/RICHER_MATERIALS.md#display-quality) cover output,
phased preparation, previews, bitmap/overlay ownership, replacements, activation,
cancellation and final borrowers. Consumers check admission/replacement failure,
pressure and shutdown.

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
