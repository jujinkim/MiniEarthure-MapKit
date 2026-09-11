# Language-neutral world library and sign surface

`examples/world-library/library.json` identifies 31 original MIT models by ID,
SHA-256, attribution and explicit box/convex proxies. `scripts/world_assets.py`
with `world_geometry.py` reproduces the library in a new directory. It authors
static input in metres; the compiler, collision generator and package contracts
are unchanged. The library contains no language text, font or image dependency.
Regional/climate/settlement selection belongs to consumers' authoring tools.

The 288,292 raw GLB bytes include utility cabins, podium towers, gabled barns,
courtyard compounds, verandas, faceted landforms, vegetation, landscaping plots,
furniture and a blank board. Compound plots own their ground and vegetation
proxies, so separate manual placement footprints do not overlap. Foliage is a
visual detail; solid roots/trunks/structures have explicit collision. Snow/ice
appearance does not introduce vehicle physics. Each library asset passed native
packaging independently; reproduce with:

```sh
rtk proxy python3 -B -m unittest discover -s scripts -p test_world_assets.py
rtk proxy python3 scripts/world_assets.py /tmp/new-world-library
```

Build the ordinary `mapkit-cli` release binary first if absent. Regeneration tests
also bind every emitted byte to the catalog; no private game/editor imports exist.
The catalog is a source-asset manifest, not a new installed pack or network format.
A map includes its used assets under the existing package inventory and budgets.

`godot/sign_asset.gd.encode(png, width_cm, height_cm)` authors a static GLB with an
embedded PNG and normalized UVs covering exactly one sign. It never loads a font,
URI, script or texture at consumption. It is a pure byte builder: its output MUST
pass the native asset validator before adoption or display. The existing PNG
CRC/full-decode/limits, static GLB validation and image memory reservations apply.
The helper checks bounded input size/dimensions, not the PNG payload's validity.

The origin is bottom center; front points toward source -Y (glTF +Z), with a
2 cm declared proxy centered at half height. Consumers own the text/image/license
metadata and collision descriptor. Each baked GLB is a map-specific sign variant;
common building and blank-board assets remain byte-identical across languages.
No schema, recipe, generated format, protocol, loading policy or budget changes.
