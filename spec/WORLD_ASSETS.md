# Language-neutral world library and sign surface

`examples/world-library/library.json` identifies 31 original MIT models by ID,
SHA-256, attribution and explicit box/convex proxies. `scripts/world_assets.py`
with `world_geometry.py` reproduces the library in a new directory. It authors
static input in metres; the compiler, collision generator and package contracts
are unchanged. The library contains no language text, font or image dependency.
Regional/climate/settlement selection belongs to consumers' authoring tools.

The library includes utility cabins, podium towers, gabled barns,
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

## Placement yaw

General `Placement` and generated objects accept `yaw_offset_mdeg` in
[-360000, 360000], default 0 (omitted from canonical JSON), added to
`quarter_turns * 90000`. Source axes are (x, height, map-y); the common renderer
reflects map-y. Nonzero offsets turn collision boxes into rotated convexes.
Occupancy, footprint rejection, cell costs, near/distant rendering, lights and
archives share the same pose; integer vertices round once after rotation.

## Authored model libraries

Editor owns environment layout and asset-distribution derivatives; MapKit owns
package validation, cell geometry and shared rendering. Asset sizes and collision
proxies are source centimetres; GLB uses metres. Derivatives get distinct content
hashes; originals are never overwritten.

Each script writes original MIT paired primary/distant models with immutable
SHA-256 names, sharing the existing material tiles (≤512 px). Distant meshes keep
the same frame, roof forms, crowns and real openings while dropping fine detail.

| Script | Contents |
| --- | --- |
| `authored_assets.py` | Dormer/porch houses, shops, market hall and stalls, farm equipment, crops, tree growth stages, shrubs, rocks, bridge piers |
| `harbor_assets.py` | Shop/apartment silhouettes, loading warehouses, container stacks, open gantry cranes, trucks, quays, pipes, piers |
| `forest_assets.py` | Cedar/broadleaf growth stages, saplings, ferns, fallen timber, lodges, tents, picnic shelters, piers |
| `canyon_assets.py` | Eroded sandstone with strata, open natural arch, talus, cactus/scrub, quarry office, loader, crusher bay, conveyor, piers |
| `snow_assets.py` | Snow firs, granite crags, timber lodges, snowbanks, road rails, open shelter, solid frozen tarn |
| `factory_assets.py` | Production halls, process tanks, boilers, stacks, pipe racks, transformers, compressors, offices |
| `park_assets.py` | Open observation wheel, carousel, decorative coaster, kiosks, queue rails, pergolas, flowerbeds, viaduct piers |

Ground-cover patches follow the quality distance limits; trunks and structures
keep primary collision. Physical hulls stay within the 32-vertex/60-face and
32-hull limits. Validate with `scripts/test_authored_assets.py`.

The authored library also provides four-metre timber/metal/snow fences, low stone
walls, safety barriers, park railings and trail signs. Modules use simple box
collision; recipes leave explicit gateway gaps and validate road clearance. Near
and distant models share the same metre origin. Display quality preserves these
solid obstacles regardless of their small dimensions.
