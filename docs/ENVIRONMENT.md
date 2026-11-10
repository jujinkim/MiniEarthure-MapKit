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
  object ID and night index seed both near and distant windows; generated building
  walls use a per-face seed near and the object seed far. Their stable
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

Golden hour (`sky_hour.gd`) warms a low sun. The weight
`(1-smoothstep(6,24,deg))·smoothstep(-6,0,deg)·(1-smoothstep(0.45,0.80,cover))`
is 1 from 0° to 6° of sun elevation under clear cover and 0 from cloudy cover up.
Each output lerps its current value towards a low-sun table by that weight:

| Output | Low-sun target |
| --- | --- |
| Sun and ambient colour (energies unchanged) | `LOW_SUN`, `LOW_AMBIENT` |
| Sky zenith, horizon, dusk glow, halo, lit cloud (sky uniforms) | `ZENITH`, `HORIZON`, `DUSK`, `HALO`, `CLOUD_LIT` |
| Daylight fog | `HORIZON` in the day fog-to-horizon ratio, so far geometry meets the sky |
| Grade | `golden_stop`: day × (1+0.07w, 1, 1−0.10w), w = smoothstep(0, 0.6, stop), clamped to 1 |
| Saturation | +0.05 × weight |
| Glass reflection | zenith/horizon/cloud through `sky_hour.gdshaderinc` |

So every output equals the plain formulas from 24° up, below -6° (the `DUSK` -6°
key is the dusk constant, so twilight is unchanged) and under cloud, rain or snow.
With the default 06:00/18:00 simple sun, Morning 09 and Noon 12 are unaffected,
Dawn 06 and Sunset 18 sit on the plateau and Evening 17 is partly warmed. The
grade's black and olive stops keep their colour; stops from 0.25 up warm. In
astronomical mode at high latitude a winter sun can stay below 24° all day, so the
layer can stay on from sunrise to sunset. The include mirrors the GDScript tables
as constants; edit both together.

Precipitation uses a bounded camera-centred GPU volume and an overhead shelter ray.
Splashes use one shared quad/material and a fixed 64-slot MultiMesh (24 on low),
with at most two extra surface rays per 10Hz update. Covered/steep surfaces suppress
splashes. Clear weather and snow retire pool contents without reallocating.

Sky radiance (the only reflection source; ambient light is a flat colour) follows
the display tier: 32/64/128px on low/medium/high (`display_quality` `sky_radiance`),
set at `configure` and on live quality changes. `display_quality.radiance_bytes`
estimates its cubemap from the engine's texture-array allocation (six RGBA16F faces
with mipmaps for each of eight roughness layers): 0.5/2/8MiB. The estimate is not
measured and excludes the engine's small downsampled copy; every sky owner allocates
its own. Glass and wet surfaces read the sharper reflection on higher tiers. Surface
materials also read the renderer's cloud cover and daylight from `environment_data`
(pixel 0: wet, snow, cloud cover, daylight) and the sun altitude (pixel 1) for the
daylight glass reflection and its golden-hour blend.

Wet upward surfaces share world-space puddle masks, small ripple normals,
roughness 0.22 and modest specular response. There is no planar reflection pass
or fluid simulation. Shader includes expand before context-owned duplication for
compatibility rendering. Rain contrasts with cloud/direct/ambient lighting;
a curved noise dome handles clouds. The sky shader owns horizon haze and cloud
occlusion, preserving sun and moon visibility; the horizon haze blends
continuously into the lower hemisphere (`sky_horizon_validator`: a known stale
reference, no such validator exists). Well below the
horizon the sky blends toward `ground_tint` by `ground_tint_weight`
(`set_ground_tint(color, weight)`), so the far ground matches the map's ground
colour and edge-stage skirt; the default keeps the dark green-grey.

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
HUD exclusion, bounded LUT reuse and the golden curve (warmer highlights, kept
shadow stops, exact day restore). `sky_hour_validator` checks per-table keys, the
golden plateau and cut-offs, continuity, the include mirroring the tables, and
renderer outputs equal to the plain formulas wherever the weight is 0. Weather,
splash pool, celestial occlusion and depth-fog checks run in Godot 4.7.2.
Aesthetics and device performance are user checks.
