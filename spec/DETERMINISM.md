# Determinism and boundary audit

Integer centimetres, exact predicates, stable ID ordering, bounded work, pinned
portable math and source/cell-derived randomness define generation. No clock,
platform path, global RNG or cell arrival order enters content identity. Ordered
geometry arrays are not sorted as sets. Display quality never changes collision or
generation hashes. Generated caches bind source/schema/dependency fingerprints and
world/cell/contract identity.

## Vectors

`determinism-vectors.json` records 7 synthetic fixtures / 28 cells (public
examples, signed/offset placement with maximum exact seed, negative-height partial
terrain, an oblique sloped bridge). Each records input identity, generated,
triangle, object, occupied-sidecar and archive digests, counts and surface probes.
Sidecar JSON is test-only. The golden is a regression baseline, not proof of
correct geometry; behavioural checks separately cover terrain/bridge seams,
duplicate faces, permutations, seed isolation, failed budgets, query interleaving
and archive restoration.

## Commands

```sh
rtk cargo test --locked -p mapkit-core
rtk cargo build --locked -p mapkit-cli -p mapkit-godot
rtk proxy python3 scripts/check_architecture.py
rtk proxy python3 scripts/check_determinism.py --output /new/path/debug-vectors.json
rtk proxy python3 scripts/check_determinism.py --release --output /new/path/release-vectors.json
rtk proxy python3 scripts/verify_godot_layout.py --godot /absolute/Godot --probe determinism --probe assets
```

The checker runs two fresh processes with different timezones against the golden
and never updates it; debug and release must match. The native probe compares
example cells in reverse order through JSON, packed geometry, cache keys, archives
and presentation. The asset probe changes LOD/shadow/material filtering and checks
that physical arrays and generation hashes stay exact.

Cross-platform: run the same commands at the same revision on Windows/Linux and
compare JSON byte-for-byte. On Android build the `determinism` example for the
device target, run it on the device and run the native probe in a device project.
Cross-compilation or emulation is not device parity. Record OS/CPU/toolchain,
revision, profile, command and vector digest. A differing component is a failure
to investigate, never a reason to bless new goldens.

## Known mismatch

The behavioural reproducibility checks pass, but the frozen-vector equality check
fails: input identities/archives and some road triangle/surface-probe fields
differ from the committed vectors, while object and occupancy digests match. The
baseline has not been replaced. Reconcile earlier road/source contract changes
with the frozen expectations before changing them.

These tests do not prove absence of cracks on all maps, physics trajectory
determinism, renderer pixel identity or frame/memory targets.
