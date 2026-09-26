# Current v1 water and arcade presentation

The 2026-09-26 contract replaces the previous `.memap` reader version 2. All own
format numbers are 1. The current schema and implementation fingerprint identify
the definition; there are no previous readers or automatic conversions.

`MapDocument.water_bodies` is optional and empty by default. Each body has a unique
document-wide `id`, a simple `polygon`, zero or more dry `islands`, `surface_cm`,
`bottom_cm`, and horizontal map-space `flow_cm_s: [x,y]`. Heights and coordinates
are centimetres; Godot converts map y to negative scene z. The volume includes its
surface and bottom. The shoreline is wet; island boundaries are dry. Overlapping
volumes choose the highest containing surface, then the lexically first ID.

There are at most 1,024 bodies, 512 combined ring vertices per body and 16 islands.
Rings must stay inside map bounds and cannot self-intersect; islands cannot touch,
intersect, nest, or leave their parent. Bottom must be below surface, heights are
within ±1,000,000 cm, and flow components within ±1,000 cm/s. Existing global
document, geometry, cancellation and allocation limits also apply.

The bounds index selects cell/region candidates. Each generated cell retains the
complete body plus clipped, display-only surface triangles (at most 4,096 per
body/cell). Neither water triangles nor volumes enter solid collision geometry.
Actual terrain, shores and bridges retain their ordinary collision surfaces.
Spawn/surface queries reject submerged ground but permit islands and bridges.
The public `godot/water_query.gd` offers a horizontal `column` query and a bounded
`sample` query; atomic cell records deduplicate body IDs across ownership changes.

Source/chunk hashes, regional metadata, indexed audits and generated archives
include water. Packed Godot chunks carry `water_bodies_json`. The current archive
has a required length-prefixed water section following gimmicks. Conservative
`water_bytes` charges 32 KiB + 16 KiB per ring vertex per intersecting cell for
geometry, JSON/Variant copies, query records and renderer resources. Existing
consumer caps are unchanged. `water_renderer.gd` uses the existing bounded renderer
steps and compatibility shader: animated ripples, depth tint, weak reflection and
shoreline foam. Physics and simulation clocks remain the consumer's responsibility.

Two optional environment fields support authored scenery: `start_minutes` (0–1439)
is an initial-time suggestion for the authority, and `ground_color: [r,g,b]` tints
terrain. Optional `RoadMarkings.color` tints a road's visible material. All colors
are bytes; they do not change collision, friction or weather authority.
`assets/arcade-library` contains original MIT scenery, reproducible with
`scripts/arcade_assets.py`; no game code or external datasets are included.

Focused core water, cost, query, surface-probe and archive tests, package/schema/
indexed audits, and the Godot consumer's compatibility-renderer load passed.
See `crates/mapkit-core/tests/water.rs` and
`crates/mapkit-package/tests/water_contract.rs` for the contract fixtures.
