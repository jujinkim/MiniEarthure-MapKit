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
components use the architecture catalogue. Shapes are flat, gabled, hipped,
pyramidal, skillion, dome and vault. `blue_tile` roofs require a Cycladic
religious building with explicit `dome`; other uses cannot select that combination.
The Editor records whether roof selections came from author, source, auxiliary
data or the profile. Source roof facts survive regional profile edits.

Ground types include asphalt, concrete, pavers, stone paving, gravel, compacted
earth, sand, rock, managed lawn, grassland, scrub, forest floor, cropland, orchard,
wetland, snow and ice. Snow/ice here are appearances, independent of weather grip.
Building uses keep homes, apartments/towers, retail/mixed use, office, hotel,
shopping, school, university, hospital, civic/religious facilities, stations,
industrial/storage, farming, utility and research distinct.

Architecture compositions cover shared modern, Korean/Japanese modern and
traditional, Chinese courtyard, South/Southeast Asian, Middle Eastern, European
stone/brick, Nordic timber, Mediterranean, North American suburb, farm, polar
station, Venetian and Cycladic sets. Existing generic axes remain valid current
catalogue entries. Module declarations describe reusable parts, never permission
to replace source footprints or add religious domes without source support.

`appearance_contract` covers package/region roundtrips, strict identifiers,
unchanged collision and part inheritance. Canonical hash and retained-memory
unit checks include styles. `environment_contract` covers expanded axes and
invalid regional authoring. All fixture geometry is synthetic.

Regional shrub zones use `builtin:shrub` with a footprint radius of 42–200cm;
city trees use `builtin:city_tree` at 120–200cm. The chosen object retains its
physical behavior independently of the vegetation appearance ID.
