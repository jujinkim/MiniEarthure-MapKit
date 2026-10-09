# Determinism and boundary audit

Only the current v1 algorithms run. Integer centimetres, exact predicates,
stable ID ordering, bounded work, pinned portable math and source/cell-derived
randomness define generation. There is no clock, platform path, global RNG or
cell arrival order in content identity. Ordered geometry arrays are not sorted
as sets. Display quality never changes collision or generation hashes.

Generated caches bind source/schema/dependency fingerprints and current world,
cell and contract identity. Original packages are preserved. Source changes can
invalidate caches without changing format numbers or adding old decoders.

## Reproducible vectors and checks

`determinism-vectors.json` freezes 13 original synthetic fixtures / 52 cells:
the public map examples, signed/offset placement with maximum exact JSON
seed, four negative-height partial terrain variants and four oblique sloped bridge
variants. Each records input identity (including decoded grid values), generated,
triangle, object, occupied-sidecar and archive digests, counts and integer surface
probe positions/normals. Sidecar JSON is test-only; it creates no public wire format.
The audit archive key uses the complete vector-input digest in the world-hash slot.
Native archive tests separately use the real package world hash.

The initial golden is a regression baseline, not independent proof of correct
geometry. Behavioral checks independently test 9,480 signed terrain seam points,
all centimetres of oblique bridge seams, duplicate faces and anchor ownership,
source/cell permutations, unrelated-object seed isolation, failed budgets,
spawn/query interleaving, optional occupancy and exact archive restoration.
Existing road/placement/convex tests retain domain-specific shape expectations.

From this public repository, with native Rust and Godot 4.7.2 installed:

```sh
rtk cargo test --locked -p mapkit-core
rtk cargo build --locked -p mapkit-cli -p mapkit-godot
rtk proxy python3 scripts/check_architecture.py
rtk proxy python3 scripts/check_determinism.py --output /new/path/debug-vectors.json
rtk proxy python3 scripts/check_determinism.py --release --output /new/path/release-vectors.json
rtk proxy python3 scripts/verify_godot_layout.py --godot /absolute/Godot --probe determinism --probe assets
```

The checker compares two fresh processes with different timezones to the committed
golden; it never updates expectations. Debug and optimized results must be equal.
The native probe compares 16 example cells in reverse order against the same core
goldens through JSON, occupied packed geometry, real package cache keys, restored
archives and presentation, then verifies post-query generation and COW isolation.
The asset probe changes actual mesh LOD/shadow/material filtering and verifies
the immutable physical arrays and generation hash remain exact.

On Windows/Linux run the commands at the same locked revision and compare exported
JSON byte-for-byte, including hashes/normals/archives. On Android provision the NDK
target and matching Godot/device runner, build the `determinism` example with that
target linker, run it on the device and capture stdout as the comparable JSON;
run the native packed/cache/asset probe in a device project too. Cross-compilation
alone, emulation and desktop resource exports do not prove native device parity.
Record OS/CPU/toolchain/revision, build profile, command, stderr and vector digest.
A differing component is a failure to investigate, never a reason to bless new
goldens or rewrite saved maps. Semantic changes require an explicit replacement decision; own formats stay v1.

Reference maps still need continuous rendered/collision traversal across seams,
rotated/partial structures, cancel/retry and cache cold/warm on supported hardware.
These small exact tests do not prove universal absence of cracks, physics-engine
trajectory determinism, renderer pixel identity or sustained frame/memory targets.


## Current unresolved vector expectation

The 2026-10-05 frozen portable input/archive hash check failed although compared
output matched a9e2761; four other determinism checks passed. Inspect current
fixture semantics and expected values before adopting a new golden. This known
failure is separate from scoped geometry/roundtrip successes and native OS parity.
