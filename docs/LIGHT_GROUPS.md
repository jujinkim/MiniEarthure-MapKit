# Bounded dynamic light groups

`godot/environment_renderer.gd.update_dynamic_lights(camera_position, groups)`
accepts generic atomic groups. No consumer vehicle geometry or identifiers belong here.
Each group contains `priority` (ascending integer), `position` (world Vector3),
`lights` (array), and optional `distance` (default 45 world units). Equal priorities
sort by squared distance to the camera. Consumers determine semantic priorities.

Each light supplies world `position`, `basis` (-Z emission direction), `color`,
`energy`, `range`, and optional `angle` (default 48 degrees). An entire group is
skipped when it cannot fit; later smaller groups may use remaining slots. Existing
nighttime static lamps fill the remaining capacity. Every update hides unused slots.
The existing fixed pool is eight desktop/four mobile SpotLight3D nodes, with at most
one dynamic shadow light. Directional sun/moon behavior is unchanged.

Focused regression: `godot/tests/light_group_validator.gd` exercises priority,
distance sorting/culling, atomic pairs, smaller-group reuse, streetlight fallback,
platform caps, removal and reentry. Run this SceneTree script in a Godot 4.7.2
consumer project. This changes the current v1 implementation fingerprint, not any
serialized map format or deterministic generated geometry.
