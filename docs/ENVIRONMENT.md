# Environment, weather and lighting

The current v1 environment profile is optional. Profile changes affect world
identity; loading never converts or rewrites source packages. The schema is
`spec/document.schema.json`. MapKit renders consumer-supplied state and does not
advance game time or simulate grip. Game authority belongs to the consumer;
standalone Editor previews use their own state.

## Authoring and shared resources

- Concepts: polar, metropolis, countryside, middle-eastern, desert, jungle and
  southeast-asian. Architecture, climate and settlement are independent axes;
  an empty dimension follows the concept. Bounded region polygons use centimetres,
  first match wins, and weather remains map-wide.
- GLB light bindings refer to material indices rather than mesh surface order.
  Empty window/bulb lists are valid. No objects are added implicitly. Map ID,
  object ID and night index seed both near and distant windows. Their stable
  switch-off minute lies between midnight and 04:00, or sunrise if earlier.
- Simple solar cycles use authored rise/set times. Astronomical mode uses J2000
  approximations, lunar phase and polar day/night for a game sky. Polar-night
  occupancy starts at 18:00; streetlights follow the solar horizon.
- A session-owned 2×1 float texture carries wetness, snow and time. Opaque surfaces
  retain shared textures and instancing. Template caching is bounded to 128MiB
  and charged to the consumer budget; each key includes the asset's light binding.
- Each cell prepares lamp descriptors once for its placed assets with bulbs.
  Placements reuse mounting offsets, colour, range and downward basis, then own
  independent transformed records. Other cells' profile changes cannot alter them.
  Completion and cancellation discard descriptors within the per-object allowance;
  the cell's final lamp metadata follows its scene lifetime.
- `scripts/atmosphere_assets.py` creates separate material assets. Original GLB
  images, UVs, geometry, collision, attribution, sources and generated packages
  remain intact. [Renderer memory](../RENDER_MEMORY.md) owns resource lifetimes.

The shared selector offers Morning 09, Noon 12, Evening 17, Sunset 18, Night 21
and Dawn 06. Numeric hour/minute values remain v1. Nonpreset authored times
survive loading unchanged; applications own translations.

## Dynamic lights

`environment_renderer.update_environment` handles low-frequency weather and lamp
discovery. Consumers call `update_dynamic_lights(camera_position, groups)` after
camera and presentation updates. This API accepts generic atomic groups; MapKit
contains no vehicle geometry or identifiers.

A group has integer `priority` (ascending), world `position`, a `lights` array
and optional `distance` (45 world units by default). Equal priorities sort by
squared camera distance. Each light supplies `position`, `basis` (-Z emission),
`color`, `energy`, `range`, optional `angle` (48°), `attenuation` (1) and
`angle_attenuation` (1). Both falloffs reset whenever a pool slot is assigned.

Groups that cannot fit are skipped as a whole; later smaller groups may fit.
Nighttime static lamps fill remaining slots. Every update hides unused slots.
The fixed pool is eight desktop/four mobile SpotLight3D nodes, with at most one
dynamic shadow caster. Sun/moon behavior is independent.

Authored streetlights use `atan(3 * tan(48°))` (about 73.3°) and triple range.
Mounting, color and energy 2 remain authored; distance falloff is 0 and cone
falloff 0.25. Existing packages gain this display behavior without regeneration.

`MapKitBridge.is_road_surface(id)` checks identity in the open package without
new generation, I/O or retained caching. Spawnable terrain, sidewalk and painted
surfaces are not road identities. The `chunk_cost_report` example emits conservative
cell estimates, not measured work time or RSS.

## Weather and distance fog

`color_grade.gd` supplies a scene-owned 256px HDR channel LUT to Godot's built-in
`Environment.adjustment_color_correction`. Daylight balances a bright casual palette with sepia: warm cream highlights, olive
shadows and lifted midtones; night keeps cool shadows and true black. Accumulated
snow blends the daylight curves towards neutral. Updates reuse one gradient and
texture, within the existing atmosphere allowance; retained LUT resources keep
that lease alive. The grade covers sky, vehicles and scenery after tonemapping,
while Canvas HUD colours stay unchanged. Materials retain their authored colours.
Direct light follows solar elevation, with cooler snow illumination and brighter
olive-neutral ambient fill for readable shaded faces. Existing
light/shadow pools, authority and display-distance limits are unchanged.

Precipitation uses a bounded camera-centred GPU volume and an overhead shelter ray.
Splashes use one shared quad/material and a fixed 64-slot MultiMesh (24 on low),
with at most two extra surface rays per 10Hz update. Covered/steep surfaces suppress
splashes. Clear weather and snow retire pool contents without reallocating.

Wet upward surfaces share world-space puddle masks, small ripple normals,
roughness 0.22 and modest specular response. There is no planar reflection pass
or fluid simulation. Shader includes expand before context-owned duplication for
compatibility rendering. Rain contrasts with cloud/direct/ambient lighting;
a curved noise dome handles clouds. The sky shader owns horizon haze and cloud
occlusion, preserving sun and moon visibility; the horizon haze blends
continuously into the lower hemisphere (`sky_horizon_validator`).

`set_display_distance(metres)` opts into depth fog. Zero restores weather fog;
negative/nonfinite values are ignored. Fog starts at 60% of range and reaches full
strength at 95%, with curve 0.7 and current sky radiance. Near/far renderers share
this Environment. It allocates no extra cells, collision, lights or viewports and
grants no loading readiness. Standalone Editor defaults to weather fog.

## Essential validation

`environment_contract` covers authoring, canonical identity, roundtrip and invalid
inputs. `light_group_validator` covers ordering, atomic groups, culling, reentry,
static fallback, falloff reset and pool bounds. It also checks interleaved cell
profiles, mounted offsets/yaw, opaque/transparent placements and cancellation/retirement.
`color_grade_validator` checks actual Mobile framebuffer colours, night black, snow,
HUD exclusion and bounded LUT reuse. Weather, splash pool, celestial occlusion and
depth-fog checks run in Godot 4.7.2. Aesthetics and device
performance are user checks.
