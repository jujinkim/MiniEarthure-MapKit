# Remaining implementation and acceptance work

This repository is a working development foundation, not a completed game cutover.

- K01 public metadata/CLI/Schema audit is implemented. Schema success alone does
  not validate a package; calendar, graph, references, bytes and payloads require
  semantic checks. K02 canonical export/container/inventory audit and scoped Mac
  regressions are implemented; locked-exporter native OS byte parity remains
  unverified. K03 bounded container/static-asset defense is implemented; see the
  exact supported subset in `spec/FORMAT.md`. Repacking malformed metadata requires an explicit source
  edit; the toolkit never silently changes saved originals.

- Generator: terrain grids, width/surface road ribbons, simple building shells,
  forest/orchard lattice, explicit asset box proxies and layered surface lookup.
  Tunnel walls/ceiling geometry exists, but terrain cuts, terrain-conforming road
  cross-sections, junction topology/meshes, sidewalks, full-footprint vegetation
  clearance and solid building volume semantics are not complete. Do not use
  prototype tunnels or overlapping geometry as production driving fixtures.
- K03 validates ZIP envelopes/descriptors/ZIP64 and complete PNG/WebP pixels,
  GLB framing/references/accessor bytes/indices/static node graphs/materials and
  embedded PNGs before admission. It accepts a documented static triangle subset,
  not every glTF feature: sparse/matrix accessors, morphs, skins, animation,
  extensions, extras, external resources and non-PNG embedded images are rejected.
  Collision metadata currently supports bounded box primitives; new convex proxy
  authoring, custom asset rendering and display/performance budgets remain K07.
  The 256 MiB decoded-image work cap and conservative validation allowances are
  not complete allocator/RSS accounting; native decoder OS/device checks remain open.
- Common Godot renderer uses simple material colors and basic tree canopies;
  streaming attachment budgeting, LOD/material libraries and incremental preview
  invalidation are not complete. No claim of 4 ms attachment is made.
- Package I/O currently holds complete payloads in memory and validates terrain
  seams up front. Lazy I/O, structural inspection under 3 s and peak memory
  accounting need implementation before game admission uses this adapter.
- Native Windows/Android hash parity, native Windows exports, representative
  10x10 km mixed-use 50 MB benchmark, and real hardware driving are unverified.
- Game transport, collision admission, session memory/cache policy and release
  acceptance belong to consumers and are not certified by this standalone tool's
  tests. This repository does not declare a completed game cutover.
