# Dynamic lighting and road identity

`environment_renderer.update_environment` handles low-frequency weather and street
lamp discovery. A renderer consumer calls `update_dynamic_lights(camera, poses)`
after its vehicle and camera updates each render frame. Each pose record contains
`pose: Transform3D` and optional `primary: bool`. Consumers omit hidden, removed
and unlit vehicles. The fixed4/8-light pool and single shadow caster are retained.

`MapKitBridge.is_road_surface(id)` checks authored road identity in the already
opened package without generation, file I/O or a new retained cache. Spawnable
terrain, sidewalk and painted surfaces are not road identities.

The read-only `chunk_cost_report` CLI example emits all cells sorted by estimated
triangle count. Counts are conservative estimates, not measured work time or RSS.
