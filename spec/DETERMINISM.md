# Determinism and boundary audit

K08, 2026-09-09. The checked scope adds executable regression evidence without
changing recipes 1–4, generated v6, source bytes, cache keys or production code.
Native Windows/Linux/Android parity and reference-map driving remain acceptance
work. Passing on one OS is not cross-platform evidence.

## Audited generation boundary

| Concern | Source and preserved rule |
| --- | --- |
| Math | `generation.rs`, `roads.rs`, `placement.rs`, `lib.rs`: exact pinned `libm =0.2.16` for square roots and rounding; integer cm output. No platform transcendental calls, fused multiply-add or fast-math configuration. Normal queries quantize to millionths after portable normalization. |
| Integer predicates | `spatial.rs`, `roads.rs`, `convex.rs`, `placement.rs`: checked source magnitudes, i128 cross products/interpolation and integer quarter turns. Signed lattice indices use `div_euclid`; clipping orders endpoints lexicographically. Recipe-1 vegetation deliberately retains its historical relative-height quantization. |
| Ordering | `MapDocument::normalize` sorts named collections by ID, heightmaps by cell and attributions by content. `BTreeMap`/`BTreeSet` order graph/group processing. Authored polygon vertices, road points, proxy faces and repetition paths are ordered geometry, not sortable sets. |
| Randomness | The first 16 hexadecimal SHA256 digits of canonical `[document_seed,zone_id,rule_id,lattice_x,lattice_y]` become a u64. Rules are `vegetation-v1` for recipes 1–2 and `vegetation-v3` for 3–4. Density uses `%1000`, jitter fixed shifts/moduli and rotation `%4`; no mutable global RNG, spawn seed or cell arrival index. Repetitions use object ID plus path index. |
| Ownership | Visual anchors have one half-open cell owner (the outside map maximum belongs to the last cell). Faces are clipped as defined by the recipe; whole occupied solids may repeat in neighboring query results by design. Recipe-3/4 vegetation owners emit whole trunks and near-edge queries include owner cells. Shared solids are not duplicate visual instances. |
| Cache and quality | Canonical generated hashes and little-endian archives contain gameplay geometry/instances, not engine meshes/materials. `packed.rs` creates immutable native owners with isolated COW views. `with_presentation` works after cold generation or archive restore. LOD bias, shadows and texture filtering act on rendered nodes only. Full product quality controls/LOD assets are separate work. |
| Dependencies | Pure core has no filesystem, environment, time, threads, engine, network, random-state or hardware-intrinsic dependency. `scripts/check_architecture.py` now gates these accidental additions and the exact math pin. This source scan is a guard, not a formal proof about compiler/transitive behavior. |

## Reproducible vectors and checks

`determinism-vectors.json` freezes 13 original synthetic fixtures / 52 cells:
the four public recipe examples, signed/offset placement with maximum exact JSON
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
goldens or rewrite saved maps. Semantic changes require an explicit strategy and
archive/release compatibility decision.

Reference maps still need continuous rendered/collision traversal across seams,
rotated/partial structures, cancel/retry and cache cold/warm on supported hardware.
These small exact tests do not prove universal absence of cracks, physics-engine
trajectory determinism, renderer pixel identity or sustained frame/memory targets.
