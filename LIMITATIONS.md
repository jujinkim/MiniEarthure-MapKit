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
  a flat roof. Hole bridges are the shortest valid ones, tested shortest first.
  Touching/nested holes and arbitrary sloped courtyard roofs reject.
- Barriers are straight prisms of at most 8 m: joints overlap by half a
  thickness, the base follows source terrain (not road-fitted terrain or
  sidewalks), and gates use conservative corridor capsules (structures by a
  height band), not exact road aprons.
  Building parts cannot stack; they share one roof kind.
- Regional files retain one bounded original authoring document. Sharded authoring,
  arbitrary density, source LRU and network region requests are not provided here.
- Logical estimates and decoder caps are not complete allocator/RSS/GPU accounting.
  Engine GLB import and initial mesh readback are not preemptible; see
  [readback isolation](docs/RENDER_DIAGNOSTICS.md#readback-isolation).
  Callers retain leases through real retirement.
- The frozen determinism vector check has a known mismatch; see
  [determinism](spec/DETERMINISM.md#known-mismatch).
- `scripts/check_architecture.py` flags `std::thread` in the test-only cancellation
  module. This known audit issue is separate from production ownership.
- Native Windows/Android parity, detailed application driving, hardware rendering
  and representative large-map performance remain user/consumer verification.

See [current contracts](spec/CURRENT_V1.md), [container](spec/FORMAT.md),
[regional source](spec/REGIONAL_SOURCE.md) and [determinism](spec/DETERMINISM.md).
Passing scoped tests is not a complete game release or cross-platform claim.
