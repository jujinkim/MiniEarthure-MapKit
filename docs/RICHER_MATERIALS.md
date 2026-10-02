# Shared world materials and display quality

The 2026-10-02 implementation (root §44.262) replaces the previous eight 256px
luminance/roughness/height/AO tiles and single water shader. All current formats
remain v1. Original MIT library assets use metre UVs, framed panes, foundations,
eaves, signs, groves and faceted rocks. Containers now have corrugations and door
hardware; lamps have a cap and base, with corresponding collision proxies.
Built-in fences/trunks/lights use wood/metal detail and the canopy uses a bounded
LOD mesh with a leaf material. Existing user assets and generated packages survive.

`scripts/richer_assets.py` generates ten shared tiles (brick, plaster, wood, stone,
asphalt, concrete, earth, grass, metal, leaf) at 128/256/512px. Channels contain
shaded luminance/AO, roughness, and baked normal XY; a normal costs one texture
sample instead of neighboring height fetches. Near detail stops at 48m. Muted
material colours and subtle warm grading preserve vehicle colours, hazards and
weather/night distinctions; Control/HUD rendering is unaffected.

The weather wrapper retains imported albedo, normal/scale, AO/channel/UV2,
roughness/channel, metallic/channel and UV1 transforms. Night-window/light roles
and opaque instancing survive. All five bitmap slots stay tracked by the asset
lease. Textures load after admission through Godot resource remapping with mipmaps,
never script preloads. Shared context reservations are 4.75/16/61MiB respectively
(`1MiB + 10 × size² × 24`), including CPU/GPU/mipmap/retirement overlap. Cache and
consumer limits still govern admission; borrowers retain charges after shutdown.

`godot/display_quality.gd` provides `profile(level)`, `active()`, and
`apply(viewport, profile)`. Low/medium/high use 75/100/100% 3D scale, 256/512/768
combined vehicle particle slots, 48/96/160m directional shadows, 4/6/8 selected
local lights, and progressively stronger water. Client owns preferences and
automatic adaptation. Profiles never enter map files or authoritative race state.
The texture tier is captured by each new map resource cache; live changes update
resolution, water, effects, shadows, LOD and optional low-tier occlusion.

Water preserves the authoritative flat surface. Low uses one flowing wave and
simple foam; medium adds a second normal, depth colour/alpha and Fresnel sky tint;
high adds a guarded screen refraction. Shore foam softens shallow boundaries.
Depth reconstruction supports Mobile and Compatibility NDC conventions. No
cosmetic ripple alters buoyancy, water entry or surface height.

`test_richer_assets.py` reproduces all library bytes and three texture tiers.
`richer_material_validator` covers actual GLB import, retained channels, denied
admission, shared identity and last-borrower retirement. `display_quality_validator`
covers texture leases, live water tiers, generated LOD indices/cache reuse, spatial
batches, tight MultiMesh bounds and actual-face occluders. Real Mobile and
Compatibility shader/preview checks passed. Detailed Editor interaction,
weather appearance and target-device acceptance remain user checks.
