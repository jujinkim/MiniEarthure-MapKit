# Shared world materials and display quality

Original MIT models use angular silhouettes, broad painted faces and restrained
street graphics. Architecture keeps window reveals, foundations, roof fascia and
real openings. Near/far models share their palette and structural envelopes;
small roof seams, window mullions and narrow decorative joinery are omitted.
Circular scenery members use at most eight flat sides. Existing packages and
imported models are preserved; new default worlds carry new content-addressed GLBs.

`scripts/richer_assets.py --textures --racing` creates separate shared tiles under
`godot/textures/racing` at 128/256/512px. Broad tonal variation replaces small grain,
dirt and repair noise. The original tiles remain available as source artifacts.
The shader bounds variation to 88–112% of the authored colour and reduces relief.
Low retains the same base colours; medium adds roughness/AO and high adds restrained
normals. Detail fades continuously at 32–48m to the authored mean colour.
Tile luminance stays centred at 0.65 and road markings follow base detail.
Imported normal/AO/roughness/metallic maps, puddle ripples and window masks fade
through the last fifth of the existing quality effect distance (48/96/160m).
Far windows retain average emission and the authority-owned lighting schedule.
The shared [Environment LUT](ENVIRONMENT.md) owns daylight sepia, cool nights and
neutral snow across vehicles, sky and near/far scenery. Canvas HUD colours retain
their palette.

The weather wrapper retains imported albedo, normal/scale, AO/channel/UV2,
roughness/channel, metallic/channel and UV1 transforms. Night-window/light roles
and opaque instancing survive. All five bitmap slots stay tracked by the asset
lease. Shared GLB templates capture opaque parameters before scene upload and
activate their shared shader one surface at a time. Source materials retire during
import; pending bindings retain the same bitmap and context leases. Textures load
after admission through Godot resource remapping with mipmaps,
never script preloads. The sign atlas loads as one more admitted step after the
tiles (halved to 512×320 with 128px tiles). Shared context reservations are
8.5/31/76MiB respectively (`1MiB + 10 × size² × 24` plus the atlas pixels × 24),
including CPU/GPU/mipmap/retirement overlap. Cache and
consumer limits still govern admission; borrowers retain charges after shutdown.

## Track and authored scenery

`rc_venue.surface_key` is the single Editor/runtime classifier for road, wall,
outer shell and structural support materials. Panel seams, dark side faces,
formwork bands and shell ribs use the existing meshes; catalogue IDs, ports,
drive surfaces and collision dimensions are unchanged. Pipes, loops, halfpipes
and static attachments share `gimmick_geometry.structure_material`; stored colour
and matte-metal properties remain authoritative. Small seam detail fades with
distance. Speed panels retain orange chevrons, jump panels cyan arrows, and grind
caps cyan/cream bands. No fullscreen pass or texture tier is added.

Paired architectural models retain deep window reveals, lintels, roof fascia and
ridge thickness. Urban cornices/corner piers and factory columns break up flat
walls. Cottages keep dormers, porches and flower boxes; asymmetric crown peaks and
irregular radial facets distinguish columnar, spreading and young trees and rocks.
Canyon strata use irregular edges in both models. Market stalls, open sandstone arches and park
pergolas fill the middle scale between scenery and major landmarks. Far meshes
retain their silhouettes/openings while dropping fine geometry.

## First-use binding

Generated opaque terrain, building and built-in faces use
`chunk_renderer._generated_material` and `environment_materials.generated_material`
to create cell-owned bindings sharing context shaders/textures. Absent bitmap, UV
and lighting overrides use shader defaults. Climate tint and safety-metal metallicity survive.
Prepared and unbatched meshes use the same factory. Shader includes are expanded
before assigning the complete source once, with identical final shader code.
Canopy leaves create independent bindings with leaf colour, roughness 1.0, detail
and wind. The shared shader retains double-sided rendering; borrowed shaders/textures
keep the context charged.
Generated building walls (material keys `brick|wood|concrete:usage`) take one of twelve
colours: the material base blended towards the use tint (concrete 0.6, brick and wood 0.3),
shared byte-for-byte with the distant mesh. `bind_facade` gives those bindings a facade
kind (concrete residential = apartment bands and balcony slabs, brick/wood residential =
house, commercial = shopfront glass on the lowest storey and a curtain-wall grid above,
industrial = panel ribs with sparse high windows, public = tall windows with stone
courses), the night-window role and a floor pitch of 3 m ÷ the map scale 1:D that
presentation publishes as `facade_floor_cm` (300 when no OSM layer records a scale).
Panes and bands are drawn procedurally from world position on vertical faces within the
quality effect distance. Packed chunks carry UV2 (seed, 1000 + base metres) on every
generated building solid (triangles of one record sharing a vertex): the seed hashes
the record ID and the solid's smallest vertex. Storeys start at the solid's base, so
ground floors follow terrain. The seed picks one of four wall tones (±8 % hue/value),
the bay width (0.8–1.3×), one of four pane patterns on apartment, commercial and
public walls (standard, tall, ribbon, square), an apartment balcony-slab band and the
glass tint, all faded with the effect weight so far walls keep the distant colour; it
also varies brightness by ±3 % and sets the night switch-off minute per building.
Meshes without UV2 keep a per-plane seed and scene-height storeys. Commercial walls
draw a storefront sign per two bays (85 %) between the shop window and the first floor
from the shared original atlas `textures/sign_atlas.png` (512 signs of 64×20 px:
8 category rows of colours, a pictogram and glyphs of an invented script, no font,
text or external image; `scripts/sign_atlas.py` reproduces it). Signs read left to
right from outside and glow at night. Roofs, bottoms, imported assets and distant
meshes are unchanged.

Fresh generated-face and canopy attachments defer detail/wind quality until their
node is tracked in the same admitted call, before publication. Fresh urban/RC
bindings likewise defer detail quality at `_environment_surface`. This applies the
active preset once with LOD and decoration distance. Direct generated bindings,
opaque source wrappers and shaderless GLB snapshots bind detail textures and roles,
then apply detail/wind quality once before returning. Standalone `bind_detail`
applies quality itself. Reused cell bindings and live updates always reapply
quality after external edits; no remembered preset replaces current material state.

Urban/RC surfaces use dedicated factories and `_environment_surface`;
standalone previews can use source materials. Distinguish these from GLB import timing.

The first urban shader assignment can stall on Metal before `_environment_surface`.
`ShaderMaterial.set_shader` lazily requests the shader RID; the internal cause needs
matching engine symbols and aligned stacks. Parameter reads use a local cache,
while texture setters request texture RIDs. Transparent sources pass through;
deferred GLB bindings activate later. Keep source retirement and wall/thread-CPU
timing in the same call, compare output/lifetimes, and distinguish settled from
completion-time reservations. See [timing controls](RENDER_DIAGNOSTICS.md#attachment-upload-isolation).

## Display quality

`godot/display_quality.gd` provides `profile(level)`, `active()`, and
`apply(viewport, profile)`. Low/medium/high use 75/100/100% 3D scale, 256/512/768
combined vehicle particle slots, 48/96/160m directional shadows, 4/6/8 selected
local lights, and progressively stronger water. Sky quality selects one, two or three cloud
noise layers with a high cloud veil and sun halo above low. Leaf-only wind is
disabled on low and uses bounded 7 cm motion on medium/high; trunks and collision
remain fixed. These fields update live along with surface material detail. Client owns preferences and
automatic adaptation. Profiles never enter map files or authoritative race state.
Profiles also expose a 96/128/160 m full-detail distance for consumers; Runtime
owns its optional-cell transitions, margins and replacement lifetime.
The texture tier is captured by each new map resource cache; live changes update
resolution, water, effects, shadows, LOD and optional low-tier occlusion.

Water preserves the authoritative flat surface. Low uses one flowing wave and
simple foam; medium adds a second normal, depth colour/alpha and Fresnel sky tint;
high adds a guarded screen refraction. Shore foam softens shallow boundaries.
Depth reconstruction supports Mobile and Compatibility NDC conventions. No
cosmetic ripple alters buoyancy, water entry or surface height.

`surface_style_validator` compares generated/imported/distant swatches at every
quality tier, including daylight, night and snow, Low tile visibility and mean
luminance at detail/distance transitions. `test_richer_assets.py` reproduces
library bytes and texture tiers. Render `generated_material_validator` to compare
GPU uniforms and pixels before corrective quality updates, with prepared/unbatched
geometry, cancellation and last-borrower retirement. Its optional
`MAPKIT_SKIP_PIXEL_COMPARISON=1` explicitly skips only pixel comparison.
`tree_canopy_validator`, `richer_material_validator` and
`display_quality_validator` cover leaf/imported bindings, bitmap retention,
independent materials, denied admission, quality changes during preparation,
MultiMesh bounds, occluders and final resource retirement. Track checks pair
`rc_venue_validator`, `pipe_material_validator` and `panel_geometry_validator`
with the rendered `rc_piece_gallery`; `RC_PIECE_KINDS`, `RC_PIECE_WIDTH` and
`RC_PIECE_CAPTURE` select supported catalogue variants and output images.
Weather appearance and target-device acceptance are user checks.
