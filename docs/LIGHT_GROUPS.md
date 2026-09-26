# Bounded dynamic light groups

`godot/environment_renderer.gd.update_dynamic_lights(camera_position, groups)`
accepts generic atomic groups. No consumer vehicle geometry or identifiers belong here.
Each group contains `priority` (ascending integer), `position` (world Vector3),
`lights` (array), and optional `distance` (default 45 world units). Equal priorities
sort by squared distance to the camera. Consumers determine semantic priorities.

Each light supplies world `position`, `basis` (-Z emission direction), `color`,
`energy`, `range`, and optional `angle` (default 48 degrees), `attenuation`
(distance falloff, default 1), and `angle_attenuation` (cone falloff, default 1).
Both falloffs reset on every slot assignment, including reuse by static lights.
An entire group is
skipped when it cannot fit; later smaller groups may use remaining slots. Existing
nighttime static lamps fill the remaining capacity. Every update hides unused slots.
The existing fixed pool is eight desktop/four mobile SpotLight3D nodes, with at most
one dynamic shadow light. Directional sun/moon behavior is unchanged.

Focused regression: `godot/tests/light_group_validator.gd` exercises priority,
distance sorting/culling, atomic pairs, smaller-group reuse, streetlight fallback,
platform caps, removal and reentry. Run this SceneTree script in a Godot 4.7.2
consumer project. This changes the current v1 implementation fingerprint, not any
serialized map format or deterministic generated geometry.

## Wider streetlights — 2026-09-27

The shared chunk renderer expands authored bulb lights to three times the original
ground width: `atan(3 * tan(48°))`, approximately 73.3 degrees. Range is multiplied
by three so the enlarged cone reaches the ground. Mounting positions, color and
energy (2) stay unchanged; distance falloff is 0 and cone falloff is 0.25 so the
expanded footprint stays visible. Range still provides the outer cutoff. Existing
packages gain the presentation change on load without regeneration or conversion.

The focused light-group regression also checks the threefold footprint, range,
mounting, per-light falloffs and reset on pool reuse for both platform capacities.
