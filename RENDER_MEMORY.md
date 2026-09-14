# Incremental renderer resource ownership

The optional fifth argument to `chunk_renderer.begin(chunk, parent, reserve,
planned_bytes, resources)` is a session `render_resource_cache.gd` context. Existing
four/two-argument calls and synchronous `attach` remain supported for standalone
previews. MapKit contains no game admission, physics or network policy.

## Planning and shared resources

`render_memory.gd` plans validated output without allocating GPU resources.
`supports_batches` excludes image proxies whose alpha/order needs the legacy path.
`prepare_batches` groups opaque triangle surfaces by material into at most512
triangles, preserving coordinates, normals, UVs, winding and hidden-proxy exclusion.
Its packed buffers cost96 bytes per visible triangle plus bounded grouping metadata.
The caller must account for source and preparation buffers before worker execution.
An empty batch list must not replace an unsupported source; use `supports_batches`.

`estimate` includes64KiB per job (another64KiB for urban surfaces),512 bytes per
visible triangle,8KiB per batch,64KiB per object and the validated native asset
allowance. Hidden collision proxies do not allocate visible mesh batches. These are
conservative logical allowances, not allocator measurements. `upper_bound` is a
pre-generation safety bound; the exact plan can be admitted after worker completion
and before renderer allocation. Compressed file length is never a RAM estimate.

Recipe6+ native presentation records add `content_hash` and `memory_bytes` from the
validated package. They are excluded from package serialization/generated hashes.
The cache key includes content, declarative material settings and referenced image
content. Older views without these records retain independent per-job ownership.
The cache claims resources before import and holds at most256 entries/64MiB by
default, including its urban shader. The caller supplies independent admission.
Each model imports once while cached; materials, mesh surfaces and decoded images
are shared. Import, MultiMesh setup, individual placement and mesh upload are
separate `advance` steps. Timings expose cumulative import/attachment/release costs
and peak single-step/import durations; synchronous engine import cannot be preempted.

Repeated opaque static models use cell-local MultiMesh instances. Hierarchical
transforms, mesh surfaces, material overrides, original colours and shadow mode
survive. Transparent models, overlays and per-surface overrides use ordinary
instances. No model reduction or automatic mesh LOD is introduced.

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
