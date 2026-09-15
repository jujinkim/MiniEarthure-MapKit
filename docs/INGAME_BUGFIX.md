# In-game defect work (2026-09-15)

Work in progress on main; no release or compatibility-lock claim.
Root `docs/INGAME_BUGFIX_WORK.md` and architecture §44.165 own cross-project
scope/status. Root `docs/validation/ingame-bugs-macos-2026-09-15/README.md`
contains commands and retained evidence. The user requested a checkpoint commit on 2026-09-16; unresolved failures remain active.

No generator change is delivered. The new roads.rs test
curved_ground_subdivision_covers_cell_without_gaps_or_overlaps reproduces a
50m-cell curved/width-changing ground subdivision with doubled triangle area
49,999,619 rather than 50,000,000 cm² (190.5cm² area deficit).

The failure remains an active regression, not ignored or tolerance-weakened.
Endpoint rounding alone does not ensure a conforming triangulation: later cuts
can subdivide previously emitted edges. Experimental stitching produced invalid
thin polygon rings and was removed. A topology-preserving generation solution
and recipe9 routing, synthetic slopes/diagonal/cell seam tests, native render /
collision validation and a separate new default package remain unimplemented.
Existing source datasets, generated packages and recipe1–8 behavior are preserved.
