# Modest miniature material detail

Original MIT assets in `assets/richer-library` keep simplified miniature forms.
`scripts/richer_assets.py` authors 256px packed tiles (luminance, roughness,
height, occlusion), metre UVs, framed recessed panes, foundations, eaves,
solid signboards with abstract marks, roofs, groves, rocks and water geometry.
No fonts, photographs or duplicate embedded bitmap payloads are required.
Every projecting canopy, sill, signboard and roof has a box/convex proxy.

Eight shared tiles are owned by the admitted environment context, loaded through
Godot resource remapping and given mipmaps. They are never script preloads.
The context reserves `1 MiB + 8 × 256 × 256 × 24 = 13 MiB` before loading.
The existing 128 MiB/256-entry shared cache and consumer limits remain unchanged.
Missing/denied tile contexts fail rendering admission. Shutdown/cancellation
seals the charge; borrowed shaders or textures keep it until their last release.

The weather wrapper retains imported GLB albedo, tangent normal/scale, AO/channel/
UV2, roughness/channel and metallic/channel textures and the source UV1 transform.
All five bitmap slots are tracked by the original asset lease. Surface weather,
night window roles and opaque instancing remain shared. Broad ground tint and
subtle packed relief complement the texture without changing collision physics.

Water uses continuous cell-sized authored triangles, depth vertex tint, mild
flowing/still ripples and highlights. Water meshes do not cast opaque shadows.
The static 2cm proxies are inset 3cm because current placement validation excludes
touching neighbours; visible surfaces stay continuous. This is decorative water,
without fluid simulation or buoyancy. Existing format 2/v1 contracts remain.

Validation: `scripts/test_richer_assets.py` reproduces every GLB and tile;
`godot/tests/richer_material_validator.gd` covers channel forwarding, real GLB
import, real shader compilation, sharing, denial and last-borrower retirement.
The consuming Client also runs display/cache regressions and actual-map budgets.
