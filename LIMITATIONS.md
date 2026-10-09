# Current boundaries and open validation

MapKit implements current-v1 map/asset validation, deterministic geometry,
assembled generation and authoring, memory terrain/water editing, package and
regional I/O, incremental common rendering and cost planning. It is independent
of game transport, physics, progress and release acceptance.

- Schema validation is only one layer: graph, geometry, hashes, decoder bounds,
  inventory and semantic references are also required. Invalid sources are not repaired.
- Static GLB/PNG/WebP support is bounded; morphs, skins, animations, executable
  extensions and external resources are unsupported. General intersecting shells,
  arbitrary roofs and inferred structure connectivity require explicit authoring.
- Courtyards allow at most 16 disjoint interior rings and 512 total vertices with
  a flat roof. Touching/nested holes and arbitrary sloped courtyard roofs reject.
- Regional files retain one bounded original authoring document. Sharded authoring,
  arbitrary density, source LRU and network region requests are not provided here.
- Logical estimates and decoder caps are not complete allocator/RSS/GPU accounting.
  Engine GLB import is not preemptible. Callers retain leases through real retirement.
- Portable frozen input/archive vectors had a known expectation mismatch in the
  2026-10-05 check; compared output matched the prior revision. Other determinism
  checks passed. Investigate the fixture contract before changing expectations.
- Native Windows/Android parity, detailed application driving, hardware rendering
  and representative large-map performance remain user/consumer verification.

See [current contracts](spec/CURRENT_V1.md), [container](spec/FORMAT.md),
[regional source](spec/REGIONAL_SOURCE.md) and [determinism](spec/DETERMINISM.md).
Passing scoped tests is not a complete game release or cross-platform claim.
