# In-game defect work (2026-09-15)

Work in progress on main; no release or compatibility-lock claim.
Root `docs/INGAME_BUGFIX_WORK.md` and architecture §44.165 own cross-project
scope/status. Root `docs/validation/ingame-bugs-macos-2026-09-15/README.md`
contains commands and retained evidence. The user requested a checkpoint commit on 2026-09-16; unresolved failures remain active.

## Initial checkpoint, before recipe 9 implementation

No generator change was delivered at the initial checkpoint. The new roads.rs test
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


## 2026-09-16 recipe 9 implementation in progress

The new ground path performs simultaneous integer subdivision per terrain tile,
with exact rational segment clipping at tile bounds before integer graph creation.
It uses pinned i_overlay8.1.1 (MIT option), a bounded512-edge/8192-potential-crossing
profile, charged sweep work, per-face triangulation limits and exact area checks.
All faces share graph intersections before emitting terrain or road identities.
Original terrain height interpolation supplies one height for every graph vertex.
Hole triangulation reuses the courtyard algorithm and restores exact collinear
boundary points; it never snaps nearby mesh edges after emission.

Recipe1–8 generation paths remain. The previously failing curved50m area test now
passes with recipe9. Twelve translated/width variants preserve area and all mesh
edges. Curved adjacent cells with varying height grids share every3D edge; tiny
partial cells preserve the exact outer domain. Road/bridge/tunnel/underpass,
ground portal, crossfall, material/width, order/budget/archive and paving/sidewalk
regressions run both their original recipe and9 and pass. Full mapkit-core tests
passed; newer final focused tests also pass after bounded sweep admission changes.

A separate candidate of the currently bundled recipe8 atmosphere town was packed
from a temporary unpacked copy, changing only recipe to9 and revision2→3.
All432cells generated successfully. Full MapKit workspace tests and native Rust
builds pass. Consumer integration,
render/collision driving checks, performance, dependency pins and final delivery
remain pending. The old package and original source files are untouched.
