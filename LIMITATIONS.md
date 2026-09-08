# Remaining implementation and acceptance work

This repository is a working development foundation, not a completed game cutover.

- K01 public metadata/CLI/Schema audit is implemented. Schema success alone does
  not validate a package; calendar, graph, references, bytes and payloads require
  semantic checks. K02 canonical export/container/inventory audit and scoped Mac
  regressions are implemented; locked-exporter native OS byte parity remains
  unverified. K03 bounded container/static-asset defense is implemented; see the
  exact supported subset in `spec/FORMAT.md`. Repacking malformed metadata requires an explicit source
  edit; the toolkit never silently changes saved originals.

- K05 recipe-2 road graph aprons, terrain-conforming ground surfaces and explicit
  elevated/bridge/underpass/tunnel geometry are implemented. Recipe 1 remains
  frozen. Authoring constraints (junction arm/approach limits, terrain-level
  transitions, matching tunnel clearances) and bounded generation failures are
  specified in `spec/FORMAT.md`. Scoped native Mac collision tests are not full
  target-platform driving acceptance. Arbitrary intersecting structures still
  require author-supplied clearance; no implicit crossing connection is generated.
- Generator work remaining: sidewalks, full-footprint vegetation clearance and
  solid building volume semantics (K06), user assets/rendering (K07) and native
  cross-platform/reference-map acceptance (K08/P). Recipe-2 road scratch has an
  explicit consumer reservation, but complete S04 allocator/RSS/GPU accounting
  remains separate.
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
