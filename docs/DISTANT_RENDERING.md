# Distant display products

The package's generated/collision archive, recipe and hash remain unchanged.
`Package::generate_distant(Cell)` generates the ordinary deterministic chunk and
converts it to a visual-only `DistantMesh`; `distant_from_generated` also accepts
an existing generated chunk. Ground, roads and native building geometry retain
original coordinates and boundaries. GLB collision proxies are omitted and
replaced by coloured bounds grouped per scene mesh/material, including node and
placement transforms. Built-in trees use a canopy volume. No collision or full
GLB/texture instance is produced. The traversal is iterative.

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

Validation: `cargo test -p mapkit-package distant` covers synthetic packages,
deterministic output, conservative bounds, preserved ground and unchanged ordinary
generated hashes. The existing asset tests cover supported archive inputs.

The bridge also reports `visual_margin_cm` from validated transformed model bounds
in `cell_window` and cost metadata. Consumers can choose full detail using the
nearest visual extent instead of the cell centre. Full-detail caching and worker
batch planning are documented in [RENDER_MEMORY.md](../RENDER_MEMORY.md). No
specific distance, physics interest, admission cap or gameplay ACK is imposed by
MapKit. Renderer roots carry diagnostic `mapkit_render_root` metadata so a caller
can count cell resources independently of scene-generated node names.
