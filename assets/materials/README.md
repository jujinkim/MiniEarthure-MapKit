# Miniature surface tiles

Original 128 × 128 deterministic material tiles, MIT licensed with MapKit.
Regenerate with `python3 scripts/atmosphere_assets.py --help` (the `textures`
function is also importable). No downloaded imagery or third-party textures.

Asphalt, concrete, brick, plaster, wood, metal, earth, gravel, sand, snow, bark,
and leaves share a muted palette. The upgrader writes a new source project,
embeds tiles in GLBs and preserves geometry, collision, normals and existing
attributions. Existing textured signs remain intact. Runtime uses one shared
weather texture, opaque materials and instancing; lighting bindings are explicit.
