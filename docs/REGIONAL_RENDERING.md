# Regional materials and architecture

The optional [v1 appearance IDs](../spec/APPEARANCE.md) select a shared catalogue.
MapKit owns the composition and rendering; the Editor owns source classification,
regional author choices and evidence. No renderer requests external data.

## Geometry and source preservation

`mapkit-core::regional_display` produces deterministic display faces for classified
building solids. The same faces and mean sRGB colours feed the Godot near renderer
and package distant products. Physical prisms, source footprints, courtyards, part
heights and generated collision archives remain independent. Source building faces
are hidden only in display when their regional replacement is present.

Flat, gabled, hipped, pyramidal, skillion, dome and barrel-vault roofs are sampled
within the original footprint. Courtyard buildings retain their open roof rings.
The physical source envelope bounds roof heights: a native gable retains its rise;
a regional roof over a flat source fits inside the declared height. Body and roof
faces use the same cell clipping planes on both sides of a seam. Decorative edge
pieces participate in neighbouring cells within their bounded reach.

Architecture and use combine reusable eaves, cornices, plinths, timber framing,
arches, shutters, balconies, rails, entrance panels, canopies, columns, vents and
steps. Buildings keep their detailed style in every batch. Residential, retail,
office, hospitality, school, university, hospital, civic, religious, station,
industrial, farm, utility and research uses remain distinguishable inputs.
Venetian compositions use stone waterfront bases and arches; Cycladic compositions
use whitewash, blue shutters and explicit vault/dome forms. A blue dome requires
the appropriate building and roof selection. Seeded variation never chooses a use
or wall material for a classified building.

Source wall/roof materials use catalogue colours and physical-looking roughness
and metallic response. Existing window and night-lighting patterns retain scaled
floor pitch. Classified bodies do not enter the random facade-finish palette or
unrelated rooftop-fixture selection. A region profile change creates new appearance
identities and invalidates draft preview identity even when collision is unchanged.

## Ground, roads and barriers

Ground and carriageway styling follows semantic record ownership. Surface-area
paint and default terrain retain their styles; sidewalk identities resolve through
the owning road. Markings, stop lines, directional lanes and sidewalk geometry stay
on their existing paths. Concrete, paving, stone and natural ground use their own
material detail rather than the physical surface's default visual.

Ground materials include aggregate, joints, ripples, strata, lawn grain, crop and
orchard rows, forest/scrub patches, wetland, snow and ice. Their fine variation fades
to the shared distant mean colour. Explicit cover keeps its palette at the local
height origin and on slopes; the same distant role preserves that choice.
Appearance alone does not change traction.

Explicit barrier type and material fix the fence/wall family and paint. Both near
barrier detail and distant opacity use that selection; unspecified barriers retain
their source-identity design. Barrier pieces remain inside their physical prisms.

Regional conifer, palm and orchard trees use compact original templates; scrub and
polar scrub templates serve shrub objects. Native distant and Godot instanced meshes
share the same vertices, deterministic scale/yaw and mean palette. Float vertex
attributes carry bark/AO and the leaf mask independently of instance tint. The
leaf shader converts its linear palette for the active renderer, matching distant
colour handling. Crown reach plus
wind stays inside the declared placement footprint. Source zone types retain their
physical tree or shrub identity. Unclassified deciduous vegetation uses the common
lobed canopy and leaf-card sets. Regenerate the shared template file with
`scripts/regional_plants.py`.

## Resources and admission

Original MIT procedural tiles under `godot/textures/regional` add sand, rock, roof
tile, glass, pavers, snow and ice to the shared racing and facade materials. Generate
them with `scripts/regional_textures.py godot/textures/regional`; 128/256/512px tiers share one periodic
master. RGBA stores mean-normalized luminance, roughness and normal XY. No external
imagery or embedded per-building texture copies are used.

The renderer requests only materials needed by a cell and its assets. A session
cache reserves each additional group before appending its load/upload queue.
Existing materials retain their texture owners while another cell prepares new
ones. Failed admission leaves the prior queue and bindings intact. Borrowed images,
textures and materials keep their leases after shutdown until the last reference
is released. Texture tiers and the existing cache/consumer limits govern memory;
no global texture preload is introduced.

Regional output has an explicit per-cell face budget. Critical body/roof overflow
fails generation. Decoration has a separate bounded allowance and reports omitted
module faces; it cannot drop source building bodies. Native estimates include
clipping, retained arrays and spatial material batches. Worker batch construction
checks cancellation; normal renderer cancellation and retirement own all meshes.

## Focused checks

Package `regional_display` checks catalogue compositions, repeatability, heights,
courtyards, seam geometry, native/distant vertex and colour agreement, bounds and
cancellation. `appearance_contract` checks strict identifiers and unchanged physics.
The rendered `regional_appearance_validator` covers selective texture tiers, append
and denied admission, last-borrower retirement, all architecture sets and separate
Venetian, Cycladic, polar, farm and urban compositions. `MAPKIT_REGIONAL_CAPTURE`
selects its local capture directory. `MAPKIT_REGIONAL_SCENE` filters comma-separated
scene names and `MAPKIT_REGIONAL_FAR=1` captures the matching distant products. Generated-material and barrier validators cover
the related established render paths. Detailed driving and device acceptance remain
user checks.
