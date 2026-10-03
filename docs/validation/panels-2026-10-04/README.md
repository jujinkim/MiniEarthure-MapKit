# Panels 02/07 — scoped macOS arm64 validation, 2026-10-04

All own versions remain 1. Shared surface fitting and marking (02) is commit
8eb46c9; the following width contract is the commit containing this report.

Passed:
- `cargo test -p mapkit-core --lib panels::`: 4 units, all 12 width/alignment
  combinations, invalid input, default generated chain, authored chain, convex
  limits, deterministic fitting, flat lift, connected/tapered/stacked support.
- `cargo test -p mapkit-core --test panel_surfaces`: 2 units, actual production
  facets at full Euler rotations on flat/graded/curved/tapered/helix roads.
  Every emitted top is 3cm above a supporting triangle within 0.09cm rounding;
  unsupported flight anchors are rejected.
- Existing `track_authoring::widths_curves_helices_and_continuous_jump` passed.
- Package `authored_source_roundtrip_draft_export_and_tampering` (including width,
  right alignment and derived shape tampering) and
  `reproducible_roundtrip_and_modified_source_rejected` passed.
- `cargo test -p mapkit-package --test schema_contract` passed.
- Optimized debug CLI and Godot native builds passed.
- Godot 4.7.2 `panel_geometry_validator` passed with strict diagnostics: shared
  Editor/Client vertices/material parameters and no extra marking mesh. The
  [synthetic render](panels.png) shows the two raised motifs at 50% width.

Initial development failures: missing Rust basis type annotation and test-only
Gimmick module qualification were corrected. A generated boost-chain road marks
its samples unsafe for spawning; support now uses the actual road mode instead
of the spawn flag. The initial render fixture used an out-of-range sample;
its valid station passed. No full suite or device/real driving acceptance was run.
Detailed editing, high-speed driving and device readability remain user checks.
