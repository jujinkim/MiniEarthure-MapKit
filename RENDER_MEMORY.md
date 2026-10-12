# Incremental renderer resource ownership

The optional fifth argument to `chunk_renderer.begin(chunk, parent, reserve,
planned_bytes, resources)` is a session `render_resource_cache.gd` context. Existing
four/two-argument calls and synchronous `attach` remain supported for standalone
previews. MapKit contains no game admission, physics or network policy.

## Planning and shared resources

`render_memory.gd` plans validated output without allocating GPU resources.
`supports_batches` excludes image proxies whose alpha/order needs individual rendering.
`prepare_batches` groups opaque triangle surfaces by material and 32 m spatial bucket into at most 512
triangles, preserving coordinates, normals, UVs, winding and hidden-proxy exclusion.
Its packed buffers cost 96 bytes per visible triangle plus bounded grouping metadata.
The caller must account for source and preparation buffers before worker execution.
An empty batch list must not replace an unsupported source; use `supports_batches`.

`estimate` includes 64 KiB per job (another 64 KiB for urban surfaces), 768 bytes per
visible triangle, 12 KiB per batch, 64 KiB per object and the validated native asset
allowance. Hidden collision proxies do not allocate visible mesh batches. These are
conservative logical allowances, not allocator measurements. `upper_bound` is a
pre-generation safety bound; the exact plan can be admitted after worker completion
and before renderer allocation. Compressed file length is never a RAM estimate.

Current v1 native presentation records add `content_hash` and `memory_bytes` from the
validated package. They are excluded from package serialization/generated hashes.
The cache key includes content, declarative material settings and referenced image
content. Views without shareable records retain independent per-job ownership.
The cache claims resources before import and holds at most 256 entries/128 MiB by
default, including its urban shader. The caller supplies independent admission.
Each model imports once while cached; materials, mesh surfaces and decoded images
are shared. GLB import, shared texture context, per-mesh LOD generation, material binding,
MultiMesh setup, placement and mesh upload are separate admitted steps. Concurrent
claims advance one shared template and reuse its completed phases. Diagnostics
include each phase total/peak and attachment/release costs; individual synchronous
engine operations cannot be preempted and their peaks remain visible.

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

Opaque shared rendering emits each multi-material mesh once. Single-object groups
also use MultiMesh custom data; exceptional material overrides get an owned seed
binding while sharing shaders/textures, so there is no global per-instance shader
slot ceiling. Small-prop distance classification includes authored root scale.

## Lifetimes

The cell lease subtracts only the separately admitted shared asset allowance.
Shared resources have their own lease. Instance nodes/MultiMeshes and private
surface meshes/materials belong to the cell. Worker/native source owners remain
independent. The cache conservatively retains the import peak until actual resource
retirement; it does not claim allocator savings by reducing a multiplier.

Claims pin templates during attachment. Completion/cancel releases pins exactly
once and clears source records. Unpinned templates may be evicted; their meshes,
materials/textures remain charged while scene instances or external consumers
borrow them. Cache shutdown seals these leases, including the shared shader.
The lease provider must retain charges through its last-borrower/retirement rules.
Completed/failed jobs may retain an invalid root handle for diagnostics.

`MapKitBridge.cell_window` and estimates provide a conservative horizontal visual
anchor margin, including transformed GLB bounds independently of collision proxies.
It is presentation metadata, not a new package, generated or transport version.

Validation: `scripts/verify_godot_layout.py --godot <Godot> --probe renderer --probe
assets --probe binding` checks relocatable standalone use, unchanged generated
hashes, GLB/images, opaque grouping, cancellation and transforms. Actual GPU
MultiMesh transform queries require a rendered engine; the dummy headless server
cannot supply them. Caller integration tests cover admission, shared last borrowers,
failed replacement, worker cancellation and delayed retirement.

## Unshareable presentation resources

Views without validated content hashes and positive memory bounds retain their
full per-cell reservation and job-local importer ownership. A supplied session
cache cannot subtract an unproven shared allowance. This is a resource-accounting
rule within current v1.

## Shared miniature material tiles

The current cache remains 128 MiB / 256 entries. The admitted weather context
reserves 4.75/16/61 MiB for ten 128/256/512 px packed tiles, mipmaps and
import/CPU/GPU/retirement overlap.
Its bytes are also included in diagnostics without an admission callback.
Normal, AO, roughness and metallic shader bindings join albedo in asset-lease
tracking. See [material implementation and verification](docs/RICHER_MATERIALS.md).

## LOD and occlusion admission

Each shared template reserves twice the validated native asset bound plus 128 KiB
before import/LOD generation, covering transient source/output meshes and retained
LOD indices. Spatial batches use 768 bytes/triangle and 12 KiB/batch; objects retain
64 KiB for instance/occluder nodes and arrays. `upper_bound` includes the same
worst-case per-triangle batch overhead. Profile changes reuse map textures, and
retiring leases remain charged through final borrowers and the consumer grace.
No physics, AI, scheduling authority or network data depends on display quality.
