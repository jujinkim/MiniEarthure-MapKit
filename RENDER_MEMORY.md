# Incremental renderer memory ownership

`render_memory.gd` is pure worker-side planning over a validated chunk view.
It shares the renderer material grouping and 512-triangle batch limit. The caller
supplies native `estimate_chunk.presentation_bytes`, including decoded pixels,
importer scratch and custom instances. Compressed byte length is not a substitute.
No GPU resources are made by planning.

`chunk_renderer.begin(chunk, parent, reserve, planned_bytes)` optionally asks the
caller for a lease before creating the root, meshes or decoding assets. A denied
or missing positive bound returns a terminal `E_MEMORY_BUDGET` job. A lease has
`track(Object)` and `seal()` methods. Admission, process caps and retirement policy
belong to the caller; MapKit has no private game dependency. Existing two-argument
`begin` and `attach` remain compatible for standalone previews and do not claim a
process memory budget.

The plan includes 64 KiB per job, 512 bytes per triangle, 8 KiB per mesh batch,
64 KiB per object and the native custom-asset peak. These conservative logical
allowances are not measured allocator/GPU bounds. Templates/material caches share
resources within one job; separate cells/candidates own separate leases. There is
no global renderer asset cache. The full peak stays charged through resource
lifetime rather than speculatively crediting importer scratch.

The renderer tracks roots, meshes, surface/override materials and texture slots.
GLB templates are tracked before duplication. Completion destroys templates,
clears source/material caches and seals the lease. Cancellation also removes and
queues the root for destruction; it is idempotent. The lease owner must count until
tracked resources are gone, including references held by another trusted consumer.
Callers must cancel jobs on shutdown and must not mutate validated chunks or make
unaccounted copies. A closed job may retain an invalid root handle for diagnostics.

Standalone checks: `scripts/verify_godot_layout.py --godot <Godot> --probe renderer
--probe assets` covers relocation, actual GLB/images and incremental cancellation.
The pure planning script is included in the independent project. Rust, package
format, generated hashes and native ABI are unchanged. Game integration also tests
budget denial/retry, reference lifetime and delayed retirement as caller policy.
