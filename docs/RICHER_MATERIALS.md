# Shared world materials and display quality

Original MIT library assets use metre UVs, framed panes, foundations,
eaves, signs, groves and faceted rocks. Containers have corrugations and door
hardware; lamps have a cap and base, with corresponding collision proxies.
Built-in fences/trunks/lights use wood/metal detail and the canopy uses a bounded
LOD mesh with a leaf material. Existing user assets and generated packages survive.

`scripts/richer_assets.py` generates ten shared tiles (brick, plaster, wood, stone,
asphalt, concrete, earth, grass, metal, leaf) at 128/256/512px. Channels contain
shaded luminance/AO, roughness, and baked normal XY; a normal costs one texture
sample instead of neighboring height fetches. Near detail stops at 48m. Low omits material relief, medium uses packed colour/AO
and roughness, and high also samples the normal detail. Muted
material colours and subtle warm grading preserve vehicle colours, hazards and
weather/night distinctions; Control/HUD rendering is unaffected.

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

| Branch | Operations to isolate |
| --- | --- |
| Generated opaque face | Direct final binding, colour/detail and quality; no source-material conversion or imported identity-cache entry |
| Built-in canopy leaf | Independent direct binding with leaf appearance and quality; the standalone preview retains its source material |
| Urban `ShaderMaterial` | Creation, first shader assignment, source parameters, environment texture and `bind_detail`; the same material is returned |
| RC `ShaderMaterial` | Existing shader and wall parameter, then `bind_detail` |
| Imported opaque `StandardMaterial3D` | Shared-wrapper creation, shader/bitmap/parameter binding, quality application and caller's last source-reference release |

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

`test_richer_assets.py` reproduces all library bytes and three texture tiers.
`generated_material_validator` compares effective GPU uniforms, live quality,
rendered pixels, prepared/unbatched geometry and cell/context borrower retirement
against the source-material path, including climate, urban, RC and preview cases.
It checks generated opaque and urban/RC attachments before corrective updates,
preset changes between admitted batches, reused cell bindings and same-preset
repair of external edits, including partial cancellation and last detail borrowers.
Set `MAPKIT_SKIP_PIXEL_COMPARISON=1` for focused material/attachment regressions;
the validator reports the skipped visual comparison explicitly.
`tree_canopy_validator` also compares leaf GPU uniforms and same-frame pixels at
each preset, live quality, independent bindings and final leaf/context retirement
after partial cancellation or completed attachment. Initial-pixel comparisons run
before any corrective quality update; admission-step preset changes and same-preset
repair of externally edited uniforms are also covered.
`richer_material_validator` covers actual GLB import, retained channels, denied
admission, shared identity and last-borrower retirement. It checks initial surface
quality at each preset for direct/wrapped/pending bindings, GLB activation,
same-preset repair, live updates and the final unpublished material borrower.
`display_quality_validator` covers texture leases, live water tiers, generated LOD indices/cache reuse, spatial
batches, tight MultiMesh bounds, solid-versus-decoration classification, live material/wind
levels and actual-face occluders. Template preparation tests include shared phased
LOD/material setup and final-borrower retirement. Recursive resource tracking
checks per-call quality refresh, same-preset repair, node/occluder state, lease
callback order and final shape retirement. MultiMesh checks cover nested parts,
setup/placement preset changes, per-object seeds, live repair and borrower retirement
after empty, partial and completed placement. Weather appearance and
target-device acceptance are user checks.
