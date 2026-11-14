# Regional appearance contract (v1)

`godot/regional_catalog.json` is the shared catalogue for producers and renderers.
It defines ground appearances, material response, building uses and architecture
modules, vegetation, barriers, and independent climate/settlement/terrain axes.
Names are stable identifiers, not file paths or national style assignments.

| Optional field | Identifier |
| --- | --- |
| `ground_style_id`, surface-area `style_id`, road `sidewalk_style_id` | `ground/<ground>` |
| Road `style_id` | `road/<road_type>/<ground>` |
| Building `style_id` | `building/<architecture>/<building_type>/<material>[/<roof_shape>/<roof_material>]` |
| Zone `style_id` | `vegetation/<vegetation_type>` |
| Barrier `style_id` | `barrier/<barrier_type>/<material>` |

Absent/empty values preserve the existing v1 presentation. Values are ASCII,
at most 128 bytes, and must compose valid names for their field's domain.
Unknown names, cross-domain styles, and sidewalk styles on explicitly absent
sidewalks fail with `E_APPEARANCE`. No network lookup or substitution occurs.
Physical `Surface`, building footprints, courtyards, part heights and collision
remain independent. Ground catalogue `surface` is a producer recommendation;
a style never changes authored physics. Building parts inherit the record style.

Styles participate in world identity, canonical serialization, source-region
selection and retained-memory accounting. Native generated collision bytes remain
identical for appearance-only changes. A producer must preserve style boundaries
when grouping building solids or road segments. The shared [regional display](../docs/REGIONAL_RENDERING.md) consumes
these identities separately from the coarse physical/material fields.

The environment profile and its ordered regions accept catalogue architecture,
climate and settlement names, plus optional `terrain`. Empty dimensions keep
concept defaults. Regions retain first-match precedence. Weather remains map-wide.
Object source facts and explicit author selection take precedence over defaults.

The two optional building roof components are supplied together. Omitted roof
components use the architecture catalogue: an architecture's optional `uses` map
gives per-building-type `material`, `roof`, `roof_material` and `modules`, each
present field replacing the architecture default for that type (`modules` replaces
the whole set). Producers take the wall-material default from the same entry.
A use's optional `roof_palette` lists display-only roof materials: a solid whose
roof comes from the catalogue (no explicit components) shows the entry chosen by
a hash of its record ID and solid index, near and far; explicit roofs never vary.
An architecture's optional `wall_palette` maps a wall material to display-only
shades of the same detail family (for example plaster to cream, ochre and rose):
each solid with that wall material shows one shade by a separate hash of the same
identity, and its display key keeps the style ID with only the material replaced.
Shade materials are ordinary catalogue materials; stored styles are unchanged.
Korean modern houses and farmhouses default to hipped roofs over plaster with
tiled eaves, varying over dark, red, blue and clay tile; Japanese modern ones to
gabled roofs, half dark tile and a quarter each brown and navy tile; their
warehouses and barns to gabled metal; farm barns to gambrel roofs. Shapes are
flat, gabled, hipped, pyramidal, skillion, dome, vault, mansard and gambrel. `blue_tile` roofs require a Cycladic
religious building with explicit `dome`; other uses cannot select that combination
(a roof palette may still display the colour).
The Editor records whether roof selections came from author, source, auxiliary
data or the profile. Source roof facts survive regional profile edits.

Ground types include asphalt, concrete, pavers, stone paving, gravel, compacted
earth, sand, rock, managed lawn, grassland, scrub, forest floor, cropland, orchard,
wetland, snow and ice. Snow/ice here are appearances, independent of weather grip.
Optional renderer-only catalogue tables recolour ground and road styles by the
map-wide resolved climate and terrain: `terrain_ground.<terrain>.<ground>` wins
over `climate_ground.<climate>.<ground>` (sRGB), and `terrain_slope_rock.<terrain>`
(`color`, `weight_per_mille`) shows bare rock on steep vegetated cover. The
catalogue `version` stays 1; producers ignore these tables and stored styles are
unchanged. The current tables cover arid natural cover and canyon rock.
Building uses keep homes, apartments/towers, retail/mixed use, office, hotel,
shopping, school, university, hospital, civic/religious facilities, stations,
industrial/storage, farming, utility and research distinct.

Architecture compositions cover shared modern, Korean/Japanese modern and
traditional, Chinese courtyard, South/Southeast Asian, Middle Eastern, European
stone/brick, Nordic timber, Mediterranean, North American suburb, farm, polar
station, Venetian and Cycladic sets, plus East European panel blocks, British
terraces, half-timbered, Alpine chalet, Parisian (mansard), Latin colonial,
Chinese modern and shophouse sets. Existing generic axes remain valid current
catalogue entries. Module declarations describe reusable parts, never permission
to replace source footprints or add religious domes without source support.

`appearance_contract` covers package/region roundtrips, strict identifiers,
unchanged collision and part inheritance. Canonical hash and retained-memory
unit checks include styles. `environment_contract` covers expanded axes and
invalid regional authoring. All fixture geometry is synthetic.

Regional shrub zones use `builtin:shrub` with a footprint radius of 42–200cm;
city trees use `builtin:city_tree` at 120–200cm. The chosen object retains its
physical behavior independently of the vegetation appearance ID.
