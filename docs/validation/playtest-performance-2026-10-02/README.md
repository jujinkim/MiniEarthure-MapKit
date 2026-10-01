# T14 immutable track verification — 2026-10-02

`verify_track` regenerated the exact deterministic document on every call. On the
same seed42 package, first/repeated calls were1,012,586/995,314µs. A successful
verification now belongs to the immutable bridge package; repeated candidate
association remains mandatory. Afterward first/repeated calls were1,023,854/15,036µs.
No format, generator, geometry or collision result changes. The cache is one boolean;
all byte/file/budgeted/project opens, failures and regional bridge initialization
reset it. Errors never mark the document verified. CLI `assembled_track::verify`
continues to perform both geometry and course checks every time.

`verify-before/` records the intentional timing regression failure on unchanged
native behavior; candidate/reopen rejection already passed. `verify-after/` passes
strict diagnostics, changed candidate rejection, structurally valid geometry
modification rejected after all five reopen routes, and failed-open invalidation.
The fixture hash is `58978474680c6fbdd1d2631c6478fd070119069d8bd1cfea047f7dcd5dc0acf1`.
The modified isolated fixture changes terrain_base_cm by−1 and clears courses,
then packs current v1; it must fail E_TRACK_MODIFIED rather than inherit success.

Root command uses `TRACK_MODIFIED_MEMAP=/absolute/modified.memap` and
`scripts/run_godot_checks.py --project client --strict-diagnostics --fixture
/absolute/seed42.memap --script
res://addons/miniearthure_runtime/mapkit/godot/tests/track_verification_cache_validator.gd`.
macOS Godot4.7.2, headless, optimized dev native. `cargo test -p mapkit-package
--test assembled_track reproducible_roundtrip_and_modified_source_rejected` passes
(1 test,53.87s); `cargo build -p mapkit-godot` passes. Initial build caught the separate
regional constructor missing the new flag; it was added before successful build.
Client records whole-load first/reuse costs and user limitations in its
`docs/validation/playtest-performance-2026-10-02/T14.md`.
