# Shared spiral road surfaces — 2026-10-05

Against MapKit `a9e2761`, 120 synthetic packaged surfaces have no unmatched
internal triangle edges. Longitudinal spans remain **5,948→5,948**. Top triangles
increase 22,948→24,476 (6.66%); whole fixture meshes, including connecting roads,
shells and walls, increase 614,864→617,904 (0.49%). [Measurements](summary.json)
separate those scopes. Original files, dimensions, settings and v1 types remain.

The matrix covers 90/180/360°, left/right, ascent/descent, all supported
2/4/6/8/12m widths, uniform widths and 2→12m port transitions, 37° placement,
translation and connected straight roads. Sources are compiled, packed to actual
`.memap` bytes, reopened and tessellated.

## Defects and correction

Previously each span rounded its cross-section with its own lateral denominator.
Opposite-endpoint integer division also gave the underside different vertices.
Subdivision based on only the first triangle's plane introduced a left/right
bias. The matrix contained 6,476 unmatched interior edges: T junctions and
sub-centimetre horizontal gaps/overlaps. These are not 6,476 vertical steps;
upright sections have constant height. Double quantization also produced a
measured 1cm endpoint mismatch at a rotated spiral-to-straight port.

- Mandatory parameter breaks remain fixed. Optional integer-height station
  moves are rejected if either adjacent interval violates the existing sampler
  bounds. No extra longitudinal spans accommodate those moves.
- The analytic spiral is rotated before coordinate quantization. Both pieces
  use the same integer port centre plus rounded edge offsets at their join.
- One section per sample is shared by adjacent spans. Transition triangles
  connect different counts. Final-vertex diagonals minimize ribbon deviation,
  then normal bend. Rendering, collision and grounding use the same faces;
  the underside reverses these faces and translates down 10cm. Outer sides and
  walls retain the common edge vertices.
- The actual bilinear-to-plane error includes quantization and transitions.
  Only failing spans receive local reinforcement. A local reduction then removes
  section vertices whenever both neighbours still satisfy the bound. No entire
  helix is raised to its densest section count.

Maximum measured diagonal error is **0.999827cm**. The existing analytic height
bound (0.6cm plus at most 0.5cm coordinate rounding), 3.5° sample-frame bound,
5⅚° horizontal bound and 23% inner/centre/outer grade remain. Final face angles
are diagnostic: the maximum changes 9.0823→5.0273°, without a 3.5° facet limit.

Uniform 4m top triangles, for ascent and descent:

| Turn | Spans, unchanged | Old left / right | New left / right |
| --- | ---: | ---: | ---: |
| 90° | 20 | 80 / 64 | 74 / 74 |
| 180° | 34 | 140 / 116 | 132 / 132 |
| 360° | 63 | 300 / 228 | 272 / 272 |

Compile/pack/reopen/mesh timing totals 1.891→2.291 seconds over 120 cases on this
Mac. This is one local pipeline observation, not statistical performance
acceptance. Flat roads, loops and pipes retain their sampling density.

## Validation and separate failures

46 affected assembled core tests pass, including grounding, cancellation,
preparation, wall closure, collision budgets and the new surface bounds. The
package matrix asserts exact shared edges, positive nondegenerate faces,
matching undersides and ports. Rotated cell-clipped fixtures additionally match
occupied/render chunks and stay within triangle/solid reservations. 15 assembled
package tests, published schema equality and both panel tests pass. Native bridge
and CLI builds pass. The intended ordered geometry transcript is renewed to
103,638 entries and
`aa6fc0337405887d726ea67c3caa76130b0ab839c03883156427dd94f3e1e20b`.

Two pre-existing failures remain separate: category mask2/sprint120 requests
72 checkpoints over the 64 budget (14/15 authoring tests pass); the frozen
portable-vector test has stale input/archive hashes. The unchanged `a9e2761`
vector exporter produces exactly the candidate's vectors, including geometry
and occupancy. The unrelated frozen file is not renewed. Four other determinism
tests pass.

Reproduce: core `--lib assembled_track`, `--test track_authoring --test
panel_surfaces --test determinism`; package `--test spiral_surface --test
assembled_track --test schema_contract`; build `-p mapkit-godot -p mapkit-cli`
with `cargo test/build --locked`. `SPIRAL_GEOMETRY_OUT` exports package metrics.
`SPIRAL_REFERENCE_DIR` on the internal
`export_unquantized_spiral_reference_when_requested` test exports a 1024-station
unquantized control for collision diagnostics, never production tessellation.

This report covers the measured surface contract. Vehicle dynamics and detailed
user/platform acceptance belong to consumers; it is not a claim of universal
driving stability or release acceptance.
