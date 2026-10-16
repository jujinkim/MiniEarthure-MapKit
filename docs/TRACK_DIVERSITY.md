# Seed track diversity and road rhythm

MapKit owns generation policy and `generation_summary`. These are generation
goals; saved Assembly, authoring sources, `.memap` and game protocols remain v1.
Consumers display the native counts without reproducing classification or policy.

## Types and goals

`assembled_track/diversity.rs` is the registry for two independent goals: use at
least half of enabled piece types and at least half of enabled gimmick types,
rounding targets upward. The denominator includes every registered generatable
type in selected categories even when space, duration or admission prevents it.
Connection portals, straight recovery curves and finish pieces add no type.

- Left/right, rising/falling and width variants share a type. Straight, curved,
  U-turn, S-rise and spiral cylinders are distinct. Jump panels, gap jumps,
  offset jumps, acceleration panels, boost chains and ring passages are distinct.
- Compound templates expand into actual road/action types. Double jumps add one
  jump type; ring chains contribute their surviving jump and ring actions.
  Expanded large corners are classified from their sampled driving shape.
- Gimmick types include normalized attachments (quarterpipe sides share a type),
  optional grind rails, jump platforms and action panels/rings. Attachment/branch
  types belong to `gimmick`; panel/ring types belong to `action`.
- Every retained route participates once. Omitted shortcuts and rejected
  attachments add nothing. Optional routes never inflate base special distance.

The immutable seed plan orders types before candidate retries. Growth prefers an
unused type in the chosen category, trying other unused types after conflicts;
retained pieces and expanded actions determine usage after rollback. Four to six
sections begin with open driving, then corners/elevation and gimmick challenge,
and end with recovery. Roles influence family/category/width and attachment-site
preferences. Mandatory approaches/landings remain reserved for their modules.
Attachment placement searches an unused kind's suitable sites before repeating a
kind, within the existing seeded density, object cap and continuous bypass rules.

Only candidates satisfying existing safety/composition and reference duration
60/90/120 s ±10% can succeed. Candidate ordering is lexicographic: feasibility,
largest normalized type-goal shortfall, summed shortfalls, absolute time error,
section-plan residual. Both goals permit early exit; unmet goals continue within
the existing maximum 24 candidates. Diversity alone never causes failure.

## Ordinary narrow-road budget

Generated ordinary roads normally use 6/8/12/18 m within catalogue limits. A seeded
attempt may narrow a safe supported straight, preserving its wider eased entry
and exit. Unsafe narrowing is omitted. Required preparation roads are ordinary
roads too; generated grind approach/landing roads use at least 6 m.

At most one connected 4 m ordinary-road run may exist across all retained main and
optional paths, and its actual sampled length must not exceed 5% of base length.
Shared route combinations do not multiply length; circuit joins connect runs.
A single 4 m endpoint has no narrow-road length. Loops, pipes, overpasses and
required portals retain their dedicated dimensions. The final compiled assembly
must satisfy the budget. Manual sources and existing packages keep their widths.

## Generation response

`mapkit_package::assembled_track::save` and Godot `generate_track` add:

| Field | Meaning |
| --- | --- |
| `generation_summary.pieces`, `.gimmicks` | Independent type summaries |
| `used`, `total`, `target` | Actual distinct types, enabled registry size, ceil(total/2) |
| `used_types`, `missing_types` | Stable registry IDs in registry order |
| `narrow_road.sections`, `.length_cm`, `.limit_cm` | Actual connected runs, sampled length and base-length 5% cap |

The summary is computed after final compile and shortcut admission. It is not
serialized into the saved document, Assembly or package. Cancellation, failed
admission and obsolete replies keep the previous consumer result and summary.

Validation uses fixed classification/template/shortfall fixtures, actual path and
branch/seam budget fixtures, representative seed/category/duration cases,
deterministic recompilation, cancellation/resource limits and package roundtrips.
Client/Editor checks cover summary display and basic startup. Driving feel and
detailed controls remain user acceptance.
