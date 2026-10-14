# Shared world materials and display quality

Original MIT library assets use metre UVs, framed panes, foundations,
eaves, signs, groves and faceted rocks. Containers have corrugations and door
hardware; lamps have a cap and base, with corresponding collision proxies.
Built-in fences/trunks/lights use wood/metal detail and the canopy uses a bounded
LOD mesh with a leaf material. Existing user assets and generated packages survive.

`scripts/richer_assets.py` generates ten shared tiles (brick, plaster, wood, stone,
asphalt, concrete, earth, grass, metal, leaf) at 128/256/512px. Channels contain
shaded luminance/AO, roughness, and baked normal XY; a normal costs one texture
sample instead of neighboring height fetches. Detail tiles stop at 48m. Imported normal/AO/roughness/metallic maps, grain,
puddle ripples and procedural window masks fade out over the last fifth of the
quality effect distance (48/96/160m). Far windows retain average emission and the
authoritative lighting schedule; albedo/sign colours remain sampled. Dry surfaces
and disabled metallic maps skip their texture/noise work. Road markings remain intact. Low omits material relief, medium uses packed colour/AO
and roughness, and high also samples the normal detail. Muted
material colours use one shared golden-daylight grade for generated, imported,
urban/RC and distant bindings. Authored hues and bitmap channels remain intact;
warmth fades out at night and under snow. Cream, brick and olive backgrounds frame
dark asphalt and distinct lane/hazard colours. Packed luminance is centred to
avoid a dark ring at detail/quality transitions and fades continuously at 32–48m.
Leaf relief favours broad canopy colour over fine noise. Control/HUD rendering is
unaffected.

The weather wrapper retains imported albedo, normal/scale, AO/channel/UV2,
roughness/channel, metallic/channel and UV1 transforms. Night-window/light roles
and opaque instancing survive. All five bitmap slots stay tracked by the asset
lease. Shared GLB templates capture opaque parameters before scene upload and
activate their shared shader one surface at a time. Source materials retire during
import; pending bindings retain the same bitmap and context leases. Textures load
after admission through Godot resource remapping with mipmaps,
never script preloads. Shared context reservations are 4.75/16/61MiB respectively
(`1MiB + 10 × size² × 24`), including CPU/GPU/mipmap/retirement overlap. Cache and
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

Paired cottage models keep dormers, porches and flower boxes; crown forms distinguish
columnar, spreading and young trees. Market stalls, open sandstone arches and park
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
quality tier, including daylight, night and snow. `test_richer_assets.py` reproduces
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
