# Water fixture repair — 2026-10-03

The bridge triangle in `crates/mapkit-core/tests/water.rs` omitted the current
required `contact_class` and `snow_retention_percent` fields. This was a stale
test fixture, not a water implementation failure. It now supplies structural-road
class `3` and ordinary snow retention `100`; production code and all v1 contracts
are unchanged.

Starting revision: `e563b8d1d4e2bc4278419627d2fe0bcbb73d9c03`. The final test source
hash, compiler versions, command and exit codes are in [sources.json](sources.json).
The commit containing this report contains the tested correction.

```sh
rtk proxy env PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --locked --manifest-path map-kit/Cargo.toml -p mapkit-core --test water
```

| Run | Result |
| --- | --- |
| [Before](water-before/output.log) | Reproduced E0063 at line 22; exit 101 |
| [After](water-after/output.log) | All 3 tests passed; exit 0; incremental test build 16.07 s |

The passing scope retains non-solid water, submerged/dry-island/bridge spawn
queries, bounds, deterministic seams/hash/cost/archive round trips and invalid
water input rejection. No unrelated Rust target or consumer suite was run.
Existing native libraries were reused because native source/dependencies did not
change. Earlier failed discovery records remain historical evidence.
