# Shared world materials and display quality

Original MIT models use angular silhouettes, broad painted faces and restrained
street graphics. Architecture keeps window reveals, foundations, roof fascia and
real openings. Near/far models share their palette and structural envelopes;
small roof seams, window mullions and narrow decorative joinery are omitted.
Circular scenery members use at most eight flat sides. Existing packages and
imported models are preserved; new default worlds carry new content-addressed GLBs.

`scripts/richer_assets.py --textures --racing` creates separate shared tiles under
`godot/textures/racing` at 128/256/512px. Broad tonal variation replaces small grain,
dirt and repair noise; grass, earth and leaf keep a faint grain of their own (lawn
blades, soil crumbs, leaf veins) within the same bounds. The original tiles remain
available as source artifacts. Generated grass ground (`lawn`) adds world-space lush
and dry patches (about 10 m) at every distance and clumps, blades and clover within
the effect distance.
Voxel ground grain (`surface_grit`, 0–1, off by default) gives all terrain, near and
distant, the tones of world-lattice cubes: square from above, a column from the side.
The cube edge follows the pixel footprint so a cube never spans fewer than about
3 pixels, from 0.25 m at the camera up to an 8 m rung; three octaves (S0, 2·S0,
4·S0) crossfade along that ladder so nothing pops, and past the 8 m rung the grain
fades out. One integer PCG chain per octave gives each cube its tone and a slight
warm/cool hue; one amplitude (at most ±5.5% brightness) serves near and distant
terrain, so the ring handover keeps its contrast. It lies over the detail tile,
lawn, rock and shore tones and under the wet bank, snow and weather; roads and
sidewalks (the urban shader) never take it. With the grain off the terrain keeps
its exact pixels. The Client owns the switch: `display_quality.profile()` keeps it
0 on every tier and `apply_material` binds the caller's value, so the MapEditor
stays off. Mobile cost is unmeasured.
`scripts/richer_assets.py --textures --facade` creates the facade relief tiles under
`godot/textures/facade` at 128/256px (no 512px tier: 256px already holds 2–5mm per
texel at 1:3): brick in running bond with recessed mortar (6 bricks × 20 courses),
painted plaster/stucco grain, formwork concrete (2 × 4 panels with recessed joints,
six tie holes each and bug holes), stone cladding (five courses of cut slabs) and
trapezoid metal ribs with a panel seam. They are packed like the racing tiles
(luminance mean 0.65, normal XY from a blurred height field) except that green is a
roughness factor centred at 0.5. The context loads them as one `Texture2DArray`
(`facades`, layer order `FACADE_LAYERS`) after the racing tiles; a missing layer
leaves walls on the racing tiles.
The shader bounds variation to 88–112% of the authored colour and reduces relief.
Low retains the same base colours; medium adds roughness/AO and high adds restrained
normals. Detail fades continuously at 32–48m to the authored mean colour.
Tile luminance stays centred at 0.65 and road markings follow base detail.
Imported normal/AO/roughness/metallic maps, puddle ripples and window masks fade
through the last fifth of the existing quality effect distance (48/96/160m).
Far windows retain average emission and the authority-owned lighting schedule.
The shared [Environment LUT](ENVIRONMENT.md) owns daylight sepia, cool nights and
neutral snow across vehicles, sky and near/far scenery. Canvas HUD colours retain
their palette.

The weather wrapper retains imported albedo, normal/scale, AO/channel/UV2,
roughness/channel, metallic/channel and UV1 transforms. Night-window/light roles
and opaque instancing survive. All five bitmap slots stay tracked by the asset
lease. Shared GLB templates capture opaque parameters before scene upload and
activate their shared shader one surface at a time. Source materials retire during
import; pending bindings retain the same bitmap and context leases. Textures load
after admission through Godot resource remapping with mipmaps,
never script preloads. The five facade layers load one per admitted step after the
racing tiles (the last step uploads the array); storefront signs are procedural and
load nothing. Shared context reservations are 6.625/23.5/68.5MiB for
128/256/512px tiers (`display_quality.tile_bytes`: `1MiB + 10 × size² × 24` racing
tiles + `5 × min(size, 256)² × 24` facade layers), including
CPU/GPU/mipmap/retirement overlap. Cache and
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

Paired architectural models retain deep window reveals, lintels, roof fascia and
ridge thickness. Urban cornices/corner piers and factory columns break up flat
walls. Cottages keep dormers, porches and flower boxes; asymmetric crown peaks and
irregular radial facets distinguish columnar, spreading and young trees and rocks.
Canyon strata use irregular edges in both models. Market stalls, open sandstone arches and park
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
Generated building walls (material keys `brick|wood|concrete:usage`) bind one of twelve
base colours (the material base blended towards the use tint, concrete 0.6, brick and
wood 0.3) and the record palettes of their key. `bind_facade` gives those bindings a facade
kind (concrete residential = apartment bands and balcony slabs, brick/wood residential =
house, commercial = a shop storey 1.5 floors tall and a curtain-wall grid of whole
storeys above, industrial = panel ribs with sparse high windows, public = tall windows
with stone courses), the night-window role and a floor pitch of 3 m ÷ the map scale 1:D that
presentation publishes as `facade_floor_cm` (300 when no OSM layer records a scale).
Panes and bands are drawn procedurally from world position on vertical faces within the
quality effect distance. Packed chunks carry `facade_seeds` (seed, 1000 + base metres)
and `facade_tops` (one float, ±(1000 + top metres)) on every generated building solid
(triangles of one record sharing a vertex): the seed's high 12 bits are the record seed
(`sha256(record ID)`, shared with the distant mesh) and its low 12 bits, the solid seed,
hash the record ID and the solid's highest vertex, which both sides of a cell cut share
(mapkit-package `distant::facade_solids`, which the distant mesh runs on the same
triangles), so solids of one record and top share it; the top is the eaves (the lowest vertex of the
solid's non-vertical faces above its base, so a gable ridge does not count), else the
highest vertex. A triangle whose highest vertex rises above the top (a gable end, the
taller part of a merged solid, a roof slope) carries the top negated. Flat roof faces
(horizontal, above the solid's base) carry the solid's roof frame instead: 4000 + the
dominant direction of its facade walls modulo 90° (the length-weighted mean of 4θ of
its flat roof perimeter edges, quantized to 1/4096 rad, so each cell showing a
rectangular building's walls finds the same frame). Roof edges shared by two flat roof
triangles of the solid (triangulation and fan diagonals, whose prisms carry internal
wall pairs) and edges on the cell cut (the chunk's extent) do not count. Facade batches
bind the seed as UV2 instead of UV (their materials are world-space) and the top as
CUSTOM0.x (commercial storeys from its magnitude, and every kind's parapet coping on
walls whose top is positive; roof finishes square to the frame, values above 3500). Storeys start at the solid's
base, so ground floors follow terrain. Commercial walls fit whole storeys: a shop
storey g = 1.5 floors (never taller than the solid), then n = max(1, ⌊(top − base − g)
/ floor⌋) storeys of (top − base − g) / n, so L ≥ 3 whole levels show the shop storey and
L − 2 taller storeys; a remainder under 0.75 floors stays a plain parapet, and floor-pitch
rows continue above the top (gable ends, taller parts of a merged solid). Meshes without
CUSTOM0 keep one-floor storeys, the shop storey included. Each solid has a 12-bit look:
its solid seed for the key's `own_share` of solids (`godot/facade_palette.json`: 0.35,
apartment complexes, concrete:residential, 0.15; `vertex()` compares one integer PCG of
the solid seed with it), otherwise the record seed. The look picks palette entries from
`facade_palette.json`, which the distant mesh (mapkit-package `distant::facade_look`)
reads too: the wall body (bits 0–2; brick and wood keep their own bodies, concrete
follows the use) and the roof (bits 3–5; concrete roofs green-painted or grey) at every
distance, and within the effect distance the pane pattern and the bay width (0.8–1.3×).
The glass tint, wall finish, window frames, signs and, on apartment complexes, an accent
colour (record bits 6–8) on one bay column every 5–8 bays and on the base storey stay
per record. The solid seed varies brightness by ±4 % and sets the night switch-off minute. Meshes without UV2 keep a per-plane seed, four
faded tones and scene-height storeys. The shop storey is split into sign slots of two
bays; in height fractions of g each slot has a darker stall riser (0–0.08), one door
0.3 floors wide with a frame (0.08–0.72; left, centre or right by a PCG hash of the slot),
display windows (0.10–0.68) with two or three mullions per bay, a transom (0.70–0.76), a
canopy shadow line and a fascia (0.78–0.98), a plain charcoal, off-white or darker-body
panel. Shop glass lights at night in a soft warm white (all of it lit).

Roofs keep the look's roof palette colour and, within the effect distance, draw a
finish (`roof_finish` in `atmosphere_surface.gdshader`) whose tint averages 1 over the
roof (patterns are zero-mean or divided by their area share, and settle with the pixel
footprint), so a roof far away shows the distant mesh's colour. Flat roofs run square
to their roof frame, in floor pitches. Green and red-brown entries are urethane
coatings: roller passes 0.12 floors wide along one frame axis (each a little lighter or
darker, a lap ridge), a fine roller stipple, repaired patches (fresher, more saturated)
and sun-faded ones (chalky, greyer) on a 3-floor grid with a darker edge, and broad
weathering. Grey entries are, per record, a grey coating or a concrete slab (expansion
joints every floor with dark sealant, each bay a shade lighter or darker, a fine
aggregate); industrial roofs also take a sheet membrane (1 m sheets with a lighter lap
band and a dark seam), public roofs a membrane or gravel ballast. Every flat roof drains to roof
drains on a 5–8 floor grid (record phase): a dark grate, a ragged stain round it and
dirtier valley lines between drains. Pitched faces run down their slope: red entries
clay tile courses (0.11 floors, staggered barrel tiles with a shadowed lip and a tilted
normal on high), grey ones asphalt shingles (0.05-floor courses, staggered tab slots) or
standing-seam metal, green ones coloured steel sheet; a gutter shadow follows the eaves
and moss creeps up from it. The finish also sets roughness and specular (coatings
glossier, slabs and gravel rough). Low keeps the joints, patches and drains and drops
the noise-based grain. `facade_validator` renders green, grey and red records (flat and
pitched) near and beyond the effect distance and checks that each shows its pattern and
keeps its mean colour.

Storefront signs are procedural (`godot/facade_signs.gdshaderinc`, original, no font,
text, texture or real brand); `bind_signs` enables them on commercial walls and binds
nothing. One sign stands on 85 % of slots, centred on the fascia (centre 0.88 g):
0.16 g high (channel letters 0.18 g), 80–94 % of the slot wide and at most 8:1. Its
design comes from the 12-bit record and the slot (`facade_pcg`, the hash of the wall
finish), so a seed that wobbles by 1e-7 keeps its signs: one of eight categories
(food, cafe, shop, clinic, finance, beauty, academy, store) with four backgrounds, two
inks and a pictogram (bowl, cup, bag, cross, coin, star, book, stripe), a border style
(none, outline, bottom bar, side bar), a pictogram on 70 % and 2–6 glyphs of an
invented, language-neutral script: 3–5 strokes of one weight with rounded caps between
neighbouring points of a 3×4 lattice, a dot one time in five. Three types: a backlit
lightbox panel (45 %), channel letters on a thin dark rail over a dark fascia (30 %;
never the off-white tone, and a body tone no lighter than the charcoal; the letters
show a darker return and cast a soft offset shadow on the fascia) and a
two-line panel (25 %; the name over a row of small glyph groups with dashes, and an
optional divider). Glyphs, pictograms and panel edges are signed distances
antialiased from the pixel footprint (taken in uniform control flow), so they stay
crisp at any distance. Each glyph owns a cell from mid-gap to mid-gap (0.12 glyph
units before its box), so whole strokes, caps and dots fall in it; a raised glyph's
return and shadow reaching into the next cell are added there. As strokes near a
pixel each glyph settles on its own ink mean over its cell, and as glyphs do the run
settles on the expected mean (dashes counted) over the same cells (pictograms on
their filled share), so far signs keep their mean colour and never crawl. The rail
and the two-line divider are box-filtered (exact pulse integrals), so they settle on
their area share once thinner than a pixel. Flat (non-channel) signs and lettering
evaluate each distance once. Panels have acrylic gloss (roughness 0.3, specular 0.5), a soft drop
shadow below-right on the fascia and, on medium/high, a bevelled edge normal near the
camera. At night lightboxes glow, a little brighter in the middle, with dark ink
blocking the light or bright ink glowing on a dimmer dark panel; channel letters glow
while their fascia stays dark. `facade_sign_face(local, bits, aspect, footprint,
vertical)` returns a `FacadeSign` (sRGB face colour, ink share, body coverage, shadow
on the mount, night glow, bevel slope) for horizontal or vertical (upright stacked
glyph) signs, and `facade_glyph_run` draws any glyph run; both need nothing else, so
other shaders can include the file. Signs read left to right from outside.

Window lettering: on commercial walls with signs, about 30 % of slot storeys among the
first three upper storeys (one integer hash of the sign design and storey) carry large
glyphs (0.42 of the glass height, 2–6 across the slot) in one ink, white, yellow, red
or blue, drawn only on the slot's glass. Their storey is lit at night: the vinyl
blocks the room's light and glows in its own ink.

The solid seed, base and height are flat varyings and per-slot, per-window and
per-room choices use an integer (PCG) hash of the seed or record and the integer slot
or cell, so each choice is constant across its pixels. Bottoms and imported assets are
unchanged.

Wall finishes make a record's walls read as one material. `facade_palette.json`
`finishes` lists the finishes (facade tile layer, tile edge in storeys, roughness,
metallic, specular) and per key the weights: concrete residential mostly painted
plaster with exposed concrete, stone and metal panels; commercial adds a glass
curtain wall (commercial keys only); industrial mostly metal panels and concrete;
public stone, concrete and plaster; brick keys stay brick; wood keys have none and
keep the racing wood tile. `bind_facade` binds them as slots (`facade_finish`,
`facade_finish_look`, `facade_finish_count`, at most six), and every brick, wood and
concrete surface carries the shared facade array. `vertex()` picks one finish per
record (a flat varying) with one integer PCG of the 12-bit record seed against the
cumulative weights, so all walls of a building agree at every pixel. Within the
effect distance the finish sets the roughness, metallic and specular of walls, piers
and stall risers; shop frames, canopy shadow and fascia keep the plain response, and
signs their own acrylic gloss. Metallic stays at most 0.1 so a finished wall keeps its diffuse brightness
and matches the distant body colour (metal panels read through roughness 0.42 and
specular 0.5); far walls fade back to the plain response, and neither the albedo
mean nor the distant body colours change. A finished wall samples its tile in
wall-plane metres (u along the wall, v from the solid base, so courses start at the
ground) with analytic gradients and anisotropic filtering, only where relief shows.
Its luminance (75–125%, `facade_luminance`) reaches the effect distance on
medium/high, where mipmaps leave only coarse joints and courses; roughness, AO and
the ×0.12 bump (medium/high) stay within the 32–48m detail range. Low keeps the
luminance within 32–48m only. The racing tile is skipped on finished and curtain
walls. Panes, frames, fascia and signs take no relief.

Pane, mullion and spandrel shares are box-filtered over the pixel footprint (an exact
pulse integral per cell axis, with the fitted commercial storey height), so thin
mullions and spandrels settle on their area share instead of aliasing; every band
below (reveals, frames, mullions, sills, balconies, coping) is a box-filtered share
the same way. Window openings are recessed: a reveal band 0.06 storeys wide (0.015 on
curtain walls) inside each pane edge stays wall, 18% darker, and on medium/high its
normal tilts towards the opening near the camera (the tilt fades out before the band
is narrower than about two pixels). Edges on the cell boundary (ribbon windows) are
continuous glass without a reveal. Punched windows (not curtain walls or shop fronts)
then show, on every tier, a frame band 0.022 storeys wide inside the reveal (white
PVC, silver or bronze on homes; dark or silver aluminium, rarely white, elsewhere; per
record), mullions (a sliding pair on homes, a centre bar and transom on industrial
walls, and per record three panes, a transom, both or a fixed pane on commercial upper
storeys and public walls), a lighter sill ledge 0.035 storeys high and 0.03 wider each
side under the pane with a shadow band below it, and a lintel shadow over the top 18 %
of the glass. Apartments (kind 1) carry balconies on about 45 % of bay columns above
the base storey: a light slab edge, a tinted glass balustrade (0.06–0.33 storeys,
glossy) and a white or dark top rail, the slab above shading the top of each storey
below. Every facade kind with a known top (CUSTOM0) draws a light parapet coping 0.05
storeys high at the solid top and a drip shadow 0.035 below it (not over the fascia),
on walls that end at the top only (none across gable ends or the taller part of a
merged solid). Medium/high add weathering, faded with the effect weight and towards
the mean of its streaks once they near two pixels: soft vertical streaks under sills
and under the coping (value noise along the wall) and darker, mottled grime over the
lowest 0.3 storeys (its 0.4 × 0.25 m mottling settles on its mean as the pixel
footprint reaches 5–12 cm); never on glass or signs.

Behind punched glass on medium/high each window shows its room: a back wall 0.9 storeys
deep seen along the view ray (so the ceiling, floor and side walls frame it as the
camera moves) in one of four wall tones, and a dressing at the glass by an integer hash
of the window: side curtains of two widths in one of six fabrics with folds (mostly
homes), part-drawn slatted blinds (box-filtered 3 cm slats, mostly offices), a sheer or
none. Both settle on the plain glass once a pane nears a few pixels.
Shop windows have their own frames and mullions (shop part 4) instead of reveals.
Glass (punched windows, curtain walls and shop windows alike) has roughness
0.08–0.15 per record and specular 0.55. On medium/high in daylight its tint keeps
45% of its strength and the glass adds what it reflects as emission: the procedural
sky's zenith/horizon gradient and cloud grey along the reflected ray, and the dark
street and opposite walls below the horizon. At a low sun the sky part takes the
sky's golden-hour blend ([Environment](ENVIRONMENT.md)) by the sun altitude in
`environment_data` pixel 1. The reflection is weighted by Schlick Fresnel from a
0.10 base and scaled by the environment's daylight and cloud cover (pixel 0: wet,
snow, cloud cover, daylight), so dusk, overcast and night follow the sky; at
night it is off. Each pane leans up to about ±1.5° and reflects 70–130% as much; both
settle once a pane nears a pixel. Low keeps plain tinted glass. Curtain walls glaze
each cell between 4% mullions and transoms in neutral anodised grey above a dark
coated spandrel (no wall tile), in a reflective tint (blue, green, bronze or silver,
metallic 0.35); at night half as many of their panes are lit (a second PCG draw per
cell), at 60% strength, so a glazed tower does not outshine punched walls; shop
fronts stay fully lit. Lit rooms (every tier) vary per window in warmth (warm, neutral
or cool white; offices cooler) and brightness (60–105 %); on medium/high the room's
depth, curtains (coloured light), blinds (striped light) and sheers shape it. Far
walls keep the warm average.

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

## Park vegetation

City trees (`builtin:city_tree`) and shrubs (`builtin:shrub`) are display-only
procedural meshes (`godot/vegetation_meshes.gd`); no Kenney or external asset is
used. A city tree's crown fits its placement footprint: placed city trees
(document placements) have the 120 cm `CITY_TREE_RADIUS`; a zone tree has its zone's declared
radius (120–200 cm, the city importer uses 200), which the native presentation
lists per cell (`city_tree_radii`: object id → radius, only above 120 cm; display
only, at most one entry per object inside its 64 KiB display share). Without that
list (older native builds, standalone previews) every city tree is 120 cm.
`vegetation_meshes.tree_set` picks the mesh set and size:

- Narrow city trees (footprints below 160 cm): three variants (round, columnar,
  spreading; canopy base 1.45–1.7 m, crown 1.8–2.9 m tall), scaled 0.85–1.15
  times radius/120: about 1.7–2.3 m wide and 3–5 m tall at 120 cm.
- Wide park trees (footprints from 160 cm): three broad variants (round, oval,
  spreading; canopy base 1.6–1.75 m, crown 3.0–3.4 m tall, thirteen lobes: a
  core, rings of seven and four and a top lobe), scaled 0.95–1.1 times
  radius/200: 3.1–3.9 m wide (spreading widest, oval narrowest) and 4.7–5.7 m
  tall at 200 cm, the leafy crown about as tall as it is wide and hanging low
  around a short trunk.

A shrub's variant follows its object identity. City trees grow in stands: a tree
whose identity byte `sha256(id)[6:8]` is below 166 (about 65%) takes the variant
of the 20 map m square (2000 cm, floored) it stands in, the others their own.
`chunk_renderer.gd` and the distant proxies (`distant.rs`) share the
integer-only `city_tree_variant`, so near and far trees match; one golden table
is asserted in `street_detail_lease_validator` and the `mapkit-package` distant
tests. Scaled and turned by the object identity, every crown vertex (hull,
cards, the low-tier hull) plus its largest wind sway (see below; 12 cm times its
sway weight on high, cards 1.1 cm more) reaches at most the footprint radius from
the trunk at the largest scale, so crowns never reach into facades, fences or
lanes the generator keeps clear; shrubs stay within the former 42 cm. The fit
barely squeezes the wide crowns (at least 0.9), keeps their cards square and only
shrinks the few cards at the limit. Each tree or shrub has three meshes:

- `body`: a tapered eight-sided trunk with a root flare whose inscribed radius
  keeps the 14×140×14 cm collision box hidden at every scale (wide trees: a
  thicker trunk, at their smallest scale 0.95 × 160/200), two or three branches
  forking at 1.1–1.4 m (wide trees four from 1.4–1.65 m, a broad fork ending in
  the lower lobe ring over half the crown radius out) and climbing out under the
  crown's lifted underside, and the crown's inner hull (an icosphere pushed out
  to 80 % of the union of the jittered lobes, 320 triangles) in a dark gap colour
  with vertex ambient occlusion, about 480 triangles in all (wide 496).
- `cards`: 150–174 alpha-scissor leaf cards just outside that hull, forming the
  silhouette (300–348 triangles). Wide trees carry 300–324 larger square cards
  on a jittered golden spiral over the whole lobe envelope (underside to top), so
  their many lobes stay evenly covered (600–648 triangles).
- `far`: an 80-triangle hull at the full crown (wide trees: out to the cards'
  mean silhouette, 112 % of the lobe union) and a plain six-sided trunk.

Shrubs have an 80-triangle body hull, 26–38 cards and an 80-triangle far hull.
Cards sample the original 2×2 leaf-cluster atlas
`godot/textures/vegetation/leaf_atlas.png` (`scripts/richer_assets.py --textures
--leaves`, data channels: luminance, yellowing, coverage); the body projects the
same atlas on its hull as a leafy pattern. On low (no cards) the body's hull grows
back to the full crown in the vertex shader and takes the far hull's brightness,
so close trees keep their lobed crowns and visible branches.

`godot/vegetation_leaf.gdshaderinc` has two variants (body, cards), each
a preloaded shader (`vegetation_leaf`, `vegetation_leaf_cards`; the grass field's
`vegetation_grass` is its own shader)
bound once by a warm-up material when the renderer scripts load, so its
compilation never falls into a streaming step. Wind
sways vertices by a weight measured from the canopy base (0 on trunks, 10 cm at the
crown top at full strength) with coherent gusts; a card flutters 8 mm as one piece
with its own phase. The phase and a hue/brightness-varied leaf
tint come from `INSTANCE_CUSTOM`. Leaves back-light warm green, darken when wet and
take snow on upward faces from the environment pixels; sky-facing cards keep their
yellowing and back-light low. Card alpha keeps its coverage with the mip level and
fades cards seen within 27° of edge-on (gone within 12°). The Client renders without MSAA, so
scissor edges stay pixel-stepped; nothing here relies on alpha-to-coverage.

A cell instances trees and shrubs per mesh set, variant and 32 m square: one admitted
step plans each placement, then each group attaches a body, a cards and a far
MultiMesh with custom data. Display quality measures every range to a group's
centre and adds its 23 m half diagonal, so a plant within `vegetation_near`
(24/32/48 m, shrubs 0.6×) always draws its near meshes. Body and cards hand over
to the far mesh at that distance with 6 m of margin, a dithered crossfade on
Forward+ and hysteresis on Mobile (which does not fade). Low draws bodies without
cards. Far trees have no end range, like the canopies they replace, so every tree
of a loaded cell keeps a crown; far shrubs end at the decoration distance. Tree
shadows follow `vegetation_shadows`: none on low, bodies and far crowns on medium,
also cards on high; shrubs and the grass field cast none. The meshes, two
materials and the atlas are one shared set per resource cache (`claim_vegetation`,
2 MiB: about 8,300 vertices and 21,100 indices counted in CPU arrays and GPU
buffers, 1.10 MB,
plus the atlas as image and texture and the materials; built
one part per admitted step: the atlas, each material, each variant's lobes and
hulls, its cards and fit, each mesh; a variant's shape is released after its last
mesh); cells without a
cache (standalone previews) build a process-lifetime set the same way and
instance it in the same groups.

Grass is one camera-local field per resource cache (`godot/grass_field.gd`,
`vegetation_grass.gdshader`) at fixed memory, however many cells are loaded.
Three rings of tiles (1, 2 and 4 m) surround the camera, each one MultiMesh per
quadrant of a shared tile mesh (twelve draws at most). A tile slot is a grass
card: an upright quad (four vertices) whose columns each hold at most one
curved, tapered blade the fragment shader draws analytically (its offset,
height, lean, width and shade from one hash of the card and column; pixels
between blades are discarded, a blade stays at least about a pixel wide). The
vertex shader places every card from integer hashes of its world tile and slot
(root, yaw, width, height), so blades are anchored to the world and never swim;
beyond 2 m a card turns up to half way toward the camera, so none is seen
edge-on, and each ring snaps to its own tile grid, leading the camera by its
last frame's travel. Rings hand over by 3D camera distance (medium 4.5–6,
11–14 and 19–24 m; high 5–6.5, 13–17 and 32–40 m, `grass_distance`) and every
ring thins its cards as 1 / distance past the distance where its full density
meets one projected blade cover (cards/m² × width × mean height × blade share
× distance / 1.5 m: 0.9 on medium, 1.3 on high), so the carpet stays even
across handovers and fills the ground in front of a chase camera. Cards per m²
and columns: medium 64 × 12 (24 cm cards), 18 × 10 (40 cm) and 5.5 × 8 (80 cm),
high 80 × 12, 22 × 12 (48 cm) and 6.25 × 10 (1 m); 15 % of the columns hold no
blade. Roots darken into the lawn, tips lighten, and every blade takes the
lawn's lush/dry patches with a gentle density, height and hue variation by
clumps. Flower slots (stem and five-petal head tilted 50° toward the camera's
side, with a warm centre) bloom in a fifth of the 4 m world squares (0.35/m²
there, 0.012/m² elsewhere) on the 1 and 2 m rings. Inside such a square every
head takes the square's one drift colour (white, yellow, marigold orange,
petunia magenta, pansy violet, salvia red or cosmos pink, from the square's
hash), so parks and lawns show drifts of flowers; scattered heads elsewhere mix
white, yellow and violet. The drift is shader-only: no memory or geometry. With
every quadrant drawn the rings hold 552 tile instances and 169,588 vertices on
medium, 900 and 336,112 on high. Blades stand only on grass ground:
a toroidal `Texture2DArray` of 8 m blocks (32 × 32 texels of 0.25 m, R32F; 8 × 8
blocks on medium, 12 × 12 on high, always reaching `grass_distance` + 4 m) holds
the grass ground height under each texel centre or EMPTY, and a card needs half
the bilinear weight of its four texels on grass, at their height (exact on a
plane, within 3 cm at an 11° crease); each card end stands on the ground under
it and an end over no grass folds back to the root, so blades never reach over
a road or water. Roads, sidewalks, paths, earth basins, water, steep banks and
slivers have no grass ground triangle, so no blade.
Each cell indexes its grass batches by 8 m block after its objects
(`godot/grass_scatter.gd`, 1,024 triangles or 1.5 ms per admitted step, no copy
of the batch arrays) and registers them with its lawn material; the field
rebuilds the blocks of its window nearest first within 1 ms per frame counted
with the window's layer clears (a block too large for one frame resumes the
next, 16 triangles between time checks), uploads each as one 4 KiB layer,
rebuilds the blocks of a cell shown, hidden or released, and hides a quadrant no
grass block reaches, so streets cost nothing. A teleport clears every grass
layer of the old window in one frame (at most 64 / 144 4 KiB uploads). The tier
is the cache texture tier (`grass_level`), which adaptive quality keeps at the
user's ceiling: a GPU-pressure dip lowers the live rings and their reach, and a
raised quality never reaches past the field tier's window; low and polar ground
have none. While it draws, the field darkens its cells' lawn under the blades
(`grass_field_distance`, `grass_field_shade`: 16 % on medium, 18 % on high,
none on banks steeper than its blades allow), fading out where they end, and
clears the shade when it shows no blades; a plain lawn material without the
atmosphere surface shader (no prepared environment) gets no shade. The lawn itself tilts its normal by
the fine blade noise gradient within 30 m, fading wherever a pixel spans a third
of the finest octave's period, so grazing views do not shimmer. Renderer
diagnostics report the shared build as `vegetation_preparation` (cache stages
`vegetation_<part>`), group attachment as `instance_upload` and the grass
index as `grass_index`.

## Street furniture

Builtin streetlights and benches are procedural display meshes
(`godot/street_furniture_meshes.gd`), each a detailed `near` and a simple `far`
mesh sharing three materials of the environment surface shader: painted metal
(`metal` tile, roughness 0.55), wood (`wood` tile; the grain is baked into
vertex colours, see below) and the lamp lens (`lighting_role` 2: a pale neutral
diffuser by day, a white glow from sunset like authored bulbs; no extra lights).

- Streetlight (912 triangles; far 100): a square base that fills the 20 cm pole
  box at car height to within 5 mm (plinth plate, skirt, access door with lock,
  number plate, cornice), a tapered transition and collar, a tapered round
  shaft to 4.84 m, a hub and two curved arms, each with a slim lofted luminaire
  and its lens underneath. Every vertex stays inside the unchanged collision
  boxes (pole 20×500×20 cm, head 80×25×40 cm at 5 m); the heads reach 39.5 cm.
  The renderer no longer draws the streetlight's generated proxy faces
  (`render_memory.hidden_objects`: owner objects and, for clipped faces in a
  neighbour cell, the proxy materials); collision, occluders and lamp
  bindings never read them.
- Bench (1,336 triangles; far 48): 0.6 m wide inside the 40 cm footprint, four
  bevelled seat and three backrest slats with gaps, painted cast side frames
  (legs, feet, seat rail, reclined back support, armrest) and a rear stretcher.
  Each slat carries its grain in the mesh, so it reads at any orientation:
  three lengthwise streaks on its wide faces (±10 % tint, slight warmth
  shift), darker last 4 cm and end grain, and a per-slat tint.

A cell instances them per kind and 16 m square as near and far MultiMeshes after
its objects; each node lists its instances' object ids (`mapkit_object_ids`),
which the map editor's 3D pick tests. The near mesh draws only while its small
parts stay resolvable: `streetlight_near` none/20/32 m and `bench_near`
none/12/20 m plus the 11.5 m group reach (`FURNITURE_REACH`); low draws only
the far mesh. Near and far cross-fade on Forward+ and hand over by hysteresis on
Mobile and Compatibility. Far benches end at the decoration distance, far
streetlights stay drawn like the faces they replace; streetlights cast shadows
from medium (none on low, like park trees), near benches on high. The set is
one claim per resource cache (`render_resource_cache.claim_mesh_set`, 640 KiB:
about 4,100 vertices and 7,200 indices in CPU arrays and GPU buffers, 0.51 MB,
plus materials and build arrays), taken when the cell begins (before any
upload) and built one material or mesh per admitted step
(`furniture_preparation`, cache stages `furniture_<part>`). Refused, it fails a cell with streetlights at once, as they
would lose their only visible body; a cell with only benches skips them. Each
placement adds 48 B to two instance buffers and its id to the group's id list
inside its 64 KiB object share. Cells without a cache build a process-lifetime
set. `street_furniture_validator` checks the envelopes, the base fill, slats,
gaps and grain, tier variants, ranges, shadows and fade modes, hidden faces
with untouched collision triangles (also packed chunks), instance placement
against the proxy boxes and the glTF anchor convention, object ids, memory,
refusal, release and (rendered) the lens by night and day.

## Transit entrances and station signs

Builtin subway entrances and station signs are procedural display meshes
(`godot/transit_meshes.gd`), each a detailed `near` and a simple `far` mesh
sharing four materials: painted steel (`metal` tile, roughness 0.5), stone
cladding (`stone` tile, vertex-tinted courses and joints), a vertex-tinted glow
(`lighting_role` 2: lit sign inlays, light strips and the stair tunnel's
interior, emissive from sunset at 0.9 × their colour; no extra lights) and one
translucent standard glass material. The pictogram (a train front on a
line-colour badge), the name bars and the teal line colour are original
geometry and colours, not any operator's logo, typeface or line scheme; no
texture is added. Both face local −z, the side the city importer turns toward
the nearest road (`osm_furniture.py`).

- Subway entrance (1,754 triangles; far 182) inside the unchanged 80×110×120 cm
  collision box: stone-clad side and back parapets with a dark base course,
  joints and coping on a closed stone slab 7 cm high (`FLOOR`: nothing inside
  the parapets lies lower, so ground relief under the footprint — at most
  6.5–7.1 cm on the Seoul cards — stays hidden; walls, the front riser and
  the plinths reach 6 cm below the foot so a downhill side shows no gap); two
  5.3 cm flood-guard steps up to a landing and three shallow steps down to
  the well floor, all with yellow nosings; then the stair's fake depth above
  the unbroken ground: the well floor and inner walls darkening from the
  landing (about 0.62 × tread) to at most 0.05 linear before the tunnel
  mouth, four receding nosings fading out, a dark stone hood over the rear
  block and a tunnel whose ceiling slopes down into black. Six steel posts,
  beams, gutters and five arch ribs carry a translucent barrel-vault roof and
  side and back glass; stainless side and centre handrails pitch down toward
  the mouth. The front sign band carries the lit pictogram, name bars, a line
  badge and a line-colour stripe. By night the landing, steps and nosings
  glow softly (glow surface, so the same tints by day), lamp strips run under
  both copings and a luminaire under the ridge, and a lamp inside the mouth
  lights the tunnel faintly (0.07 linear at the mouth, black 25 cm inside).
  Lit inlays, joints and nosings stand at least 2.5 mm (`LAYER`) off the
  face behind them. The far mesh keeps the slab, steps, darkening well, hood,
  mouth, band and the translucent glass.
- Station sign (516 triangles; far 60) inside the 24×180×24 cm box: a stone
  plinth, a bevelled steel pylon with a lit line stripe and front and back
  name plates of abstract text bars, and a four-sided light box (pictogram,
  dark name bars and line stripe on a backlit panel at 0.62 × white, so it
  glows without clipping) under a cap.

The renderer no longer draws their generated proxy faces
(`render_memory.hidden_objects`, the street furniture mechanism); collision and
occluders never read them. A cell instances them per kind and 32 m square as
near and far MultiMeshes after its objects with the street furniture range
policy for solid kinds (`furniture_near`; low draws only the far mesh, the far
mesh stays drawn like the faces it replaces, shadows on every tier). The set is
one claim per resource cache (`render_resource_cache.claim_mesh_set`, 768 KiB:
about 4,500 vertices and 7,500 indices in CPU arrays and GPU buffers, 0.57 MB,
plus materials and build arrays), taken when the cell begins and built one
material or mesh per admitted step (`transit_preparation`, cache stages
`transit_<part>`); a refused set fails the cell before it uploads anything, as
its entrances and signs would lose their only visible body. Each node lists
its instances' object ids (`mapkit_object_ids`, editor picking); each
placement adds 48 B to two instance buffers and its id to the group's id list
inside its 64 KiB object share. Cells without a cache build a process-lifetime
set.
`transit_detail_validator` checks the envelopes, the steps, the slab floor,
the well and tunnel shading, the lit faces, tier variants and ranges, hidden
faces with untouched collision triangles, memory, refusal before upload,
release and (rendered) the sign panel, landing, well and tunnel by night and day and 12 %
sloping ground never showing inside the well. Distant cells keep the native
proxy boxes in the distant concrete colour (`distant.rs`, `Surface::Concrete`),
close to the far mesh's stone.

## Traffic signals and bus shelters

Builtin traffic signals (`signal_h`, `signal_v`) and bus shelters are
procedural display meshes (`godot/street_signal_meshes.gd`, built with the
street furniture builder and shapes), each a detailed `near` and a simple `far`
mesh sharing four materials: painted metal (the street furniture paint),
vertex-tinted signs (`lighting_role` 2: posters, the timetable, the fascia name
strip and the route plate glow at 0.9 × their colour from sunset), the signal
lens shader and one translucent standard glass material. Local +x points over
the road. The importer turns local −z toward the approaching traffic under
right-hand driving and toward the junction under left-hand driving, so every
head is lit on both faces with red at the arm-tip (road-centre) end or on top:
drivers see a lit face, red toward the road centre, under either convention.

- Signals (`signal_h` 1,648 triangles, far 124; `signal_v` 1,284, far 124): a
  bolted square base plate, flange and tapered round pole with a cap and a
  yellow push-button box, all inside the unchanged 8×220×8 cm pole box; a
  pedestrian signal (stop and walk figures under hoods) facing across the road
  at 0.85–1.0 m. `signal_h` adds a tapered mast arm with a collar, tie rod and
  end cap, and under it at 1.32 m a horizontal head; `signal_v` a vertical head at 1.85 m held by brackets above
  and below it. Under the arm (x 0.50–1.02 m, y 1.97–2.11 m) `signal_h` hangs a
  double-faced road-name board on two strap clamps: the blank board (aluminium
  rim, blue field, white border on both faces) is part of `signal_h_near`; its
  print is a separate near part per layout (`name_board_0..3`: a glyph run, two
  small numbers and an arrow at each viewer's right end), chosen per 32 m
  square and axis (`name_layout`) so the two crossing roads of a junction
  differ; the far `signal_h` carries a plain blue board on the printed
  surface, lit at night like the near one. A head is a bevelled housing with
  three domed lenses and a hood over each lens on both faces (the hoods toward the pole of a vertical
  head are half as long and stay 7 mm clear of it) and a black mid-plane
  backplate whose light border is a ring of bars beside it. Far heads draw
  8-sided lenses. The lens shader keeps the cycle (green 6 s, amber 2 s, red
  8 s, local and never synced); odd quarter turns run half a cycle apart: the
  nearest quarter turn of the instance yaw, a yaw exactly halfway going to the
  next one (the threshold lies 0.0005° early, between whole millidegrees), so
  the quarter turns decide within the importer's ±45° offsets and the heading
  decides for larger editor offsets. Instance custom data would carry the
  phase exactly but drops instanced vertex tints on Compatibility. A lit lens
  glows at 0.75–0.9 × a hue pre-compensated for ACES without glow (green
  0, 0.55, 0.30; amber 1, 0.25, 0; red 1, 0, 0) with little albedo, so
  sunlight does not wash it toward white; an unlit one is dark tinted glass;
  the walk figure shows while the approach is red, blinking for its last 2 s.
  Lens roles travel in UV and UV2: vertex colours read by a custom shader were
  unreliable on instanced meshes on Forward+ and Compatibility.
- Bus shelter (1,306 triangles plus 16 glass; far 172): steel front, rear and
  bay posts on base plates, beams and rafters, a roof deck sloping from 0.96 m
  at the front to 0.94 m at a rear gutter behind a blue fascia with a white
  name strip and logo, glass in two back bays and two half-depth side screens
  crossed by a frosted band at 0.47–0.50 m (the panes stop at it), an
  advertising lightbox (poster on both faces) in the third bay, a timetable
  panel on rails to its bay posts, a route plate hanging at the front corner,
  and a four-slat perforated steel bench cantilevered from two legs (the far
  mesh: a seat slab on two legs). Every vertex stays inside the proxies'
  160×97×50 cm envelope, and posts, roof and back wall stand where their
  collision boxes are. The bench, the side screens and their mid posts have no
  collision box: a car driven into the shelter can overlap the bench before it
  meets the back wall, and a car corner can reach into the 40 cm side gaps.
- Printed faces (posters, the timetable, the route plate, the fascia strip,
  the pedestrian signal front) are tiled coplanar faces closing open boxes or
  framed by coplanar rings (`compose`, `framed`), never layers stacked a
  fraction of a millimetre over another face, which flicker in a 24-bit depth
  buffer at street distances. Lens rims stand 1.5 mm off their housings.

The renderer no longer draws their generated proxy faces
(`render_memory.hidden_objects`, the street furniture mechanism); collision and
occluders never read them. A cell instances them per kind and 32 m square as
near and far MultiMeshes after its objects (horizontal signals also join
their name-board group, near only) with the street furniture range
policy for solid kinds (`furniture_near`; low draws only the far mesh, the far
mesh stays drawn like the faces it replaces, shadows on every tier); shelter
glass is one node per group, drawn at every distance on every tier, and never
casts shadows (`mapkit_furniture_glass`, tagged far for editor picking). The
set is one claim per resource cache (`render_resource_cache.claim_mesh_set`,
1,152 KiB: about 8,600 vertices and 15,800 indices in CPU arrays and GPU
buffers, 1.09 MB with the road-name boards, plus materials, the lens shader
and build arrays), claimed as the cell begins (`chunk_renderer.begin`, before any upload) and built one
material or mesh per admitted step (`signal_preparation`, cache stages
`signals_<part>`); a refused set fails the cell before it uploads anything.
Each placement adds 48 B to each of its instance buffers and its id to the
group's id list (`mapkit_object_ids`, editor picking) inside its 64 KiB object
share. Cells without a cache build a process-lifetime set.
`street_signal_validator` checks the pole box (everything below 0.6 m), head
positions, lens layout on both faces and visors clear of the pole, the shelter
envelope and boxes, tier variants, ranges and glass, the phase rule for
quarter turns and offsets, hidden faces with untouched collision triangles,
memory, refusal before upload, release and (rendered) two planned heads at
44° and 45° in one frame, instanced paint tints and the poster by night and
day. Distant cells keep the native proxy boxes.

## Road signs and guide signs

Builtin road signs (`sign_speed_30/40/50/60/80/100`, `sign_crossing`,
`sign_stop`, `sign_yield`, `sign_children`) and overhead guide signs
(`guide_sign`) are procedural display meshes (`godot/road_sign_meshes.gd`, built
with the street furniture builder and shapes), each a detailed `near` and a
simple `far` mesh sharing two materials: painted metal (the street furniture
paint: posts, brackets, aluminium backs and edges) and vertex-tinted printed
faces (`lighting_role` 2: they glow at 0.9 × their colour from sunset, standing
in for retroreflection; printed colours stay at 0.8 so the glow never clips).
Shapes are generic traffic-control forms; digits are seven-segment bars and
names glyphs of the invented storefront script (3–5 strokes on a 3×4 lattice,
fixed bits per sign) drawn as geometry: no fonts, real text or logos. The
importer turns the printed face (local −z) against the traffic and local +x
over the road under right-hand driving.

- Pole signs (near 432–592 triangles, far 26–58): a foot sleeve
  reaching 4 cm below the foot, a slightly tapered round post with a domed cap,
  a back channel and two band clamps, all inside the unchanged 6×104×6 cm post
  box below 0.66 m, holding an aluminium plate (rolled edge, plain back with a
  stiffener rail) within 16 cm of the post across x. Speed limits: a white rim,
  red ring and white disc with black digits ("100" in narrower cells); the
  crossing sign a blue chamfered square, white border, walking figure and zebra
  bars; stop a red inverted triangle with a white border and two white glyphs;
  yield a wide red band, white field and two red glyphs; children protection a
  yellow triangle with a red band, two black child figures holding hands and a
  white supplementary plate (dark border, four glyphs) below it. Far meshes: a
  square post, the outline with one coplanar inner field and a flat back (speed
  signs one dark bar). Speed limits share one body (near and far) under their
  own digit parts.
- Guide sign (near 1,418–1,466 triangles, far 100): a concrete collar,
  bolted base plate and tapered round pole with a cap inside the unchanged
  10×250×10 cm pole box; two cantilever pipes at 2.44 m and 1.725 m with end
  caps, collars, a diagonal web and gussets; a 1.70×0.60 m board from 1.78 m up
  in front of them with Z-bar stiffeners and brackets with U-bolts on its back,
  printed with a blue field, a white border, white arrows, glyph runs and white
  route shields with two blue digits. Four layouts chosen by the object
  identity: a junction diagram, straight with a right branch, a left branch
  with straight, and an overhead lane board; the frame and blank board are one
  part, each layout's print another. The far mesh keeps the open truss: two
  arm bars and one diagonal behind a board with a white border and two bars.
  Only the pole stands below 1.70 m; the arms and board reach 2.58 m along +x
  (the importer accepts an approach whose half carriageway spans 2.6 m), clear
  of vehicles. The arm reaches one way only, so the importer places guide signs
  under right-hand traffic.
- Printed faces are tiled coplanar regions (a ring and its disc, a band and its
  field); symbols stand 1.5 mm in front, as lens rims do.

The renderer draws them instead of their generated proxy faces
(`render_memory.DRAWN_OVER`); collision and occluders never read those faces. A
cell instances them per kind, guide layout and 32 m square as near and far
MultiMeshes after its objects with the street furniture range policy for solid
kinds (`furniture_near`; low draws only the far mesh, the far mesh stays drawn
like the faces it replaces, shadows on every tier). The set is one claim per
resource cache (`render_resource_cache.claim_mesh_set`, 960 KiB: about
6,650 vertices and 14,640 indices in CPU arrays and GPU buffers,
0.86 MB, plus the two materials and build arrays), claimed as the cell
begins and built one material or mesh per admitted step
(`road_sign_preparation`, cache stages `road_signs_<part>`); a refused set fails
the cell before it uploads anything. Each placement adds 48 B to each of its
instance buffers and its id to the group's id list inside its 64 KiB object
share (speed and guide signs: two near parts and a far one, so three
buffers). Cells without a cache build a process-lifetime set.
`road_sign_validator` checks the post and pole boxes, footprint and vehicle
clearance, printed colours and fronts, the open far truss, board prints inside
the blue field and laid out for the driver's right, layouts and digits, tier variants and
ranges, planned groups, hidden faces with untouched collision triangles,
memory, refusal before upload, release and (rendered) a speed sign and a guide
board by day and night. Distant cells keep the native proxy boxes.

## Street trees and flower beds

Street trees in tree pits (`builtin:street_tree_ginkgo`, `_zelkova`, `_plane`,
`_cherry`, `_pine`) and flower beds (`builtin:planter`, `planter_square`,
`median_bed`, `median_bed_pine`) are procedural display meshes
(`godot/street_planting_meshes.gd`) on the park vegetation's shaders, wind,
leaf tint, low-tier hull regrowth and weather, with their own 2×2 leaf atlas
`godot/textures/vegetation/street_leaf_atlas.png` (`scripts/richer_assets.py
--textures --leaves`, same data channels: ginkgo fans, pine needle tufts, small
serrate leaves for zelkova and cherry, large lobed plane leaves). No external
asset is used. Local +x runs along the road; the importer places them
(`map-editor/docs/IMPORT_CITY.md`).

Each species has a mature variant (instance scale 0.92–1.08) and a young
replacement (12 % by identity, scale 1, crown × 0.65), the pine also a mature
variant without a pit for median beds. A variant has a `body` (trunk with the
species bark baked into vertex colours, branches, the crown's dark inner hull
and the pit fixtures, 690–950 triangles), `cards` (77–200 alpha-scissor leaf
cards) and `far` (a coarse hull, a plain trunk and the pit as one plate, 106
triangles):

- Ginkgo: a straight leader in a narrow cone with three whorls of short upswept
  branches, grey-brown bark with vertical fissures, light yellow-green fans; a
  cast-iron grate with 24 radial slots in a granite frame. Crown within 60 cm.
- Zelkova: the trunk forks at 0.95 m into five ascending limbs under a broad
  dome, smooth grey bark with orange-brown flakes; the grate. Within 125 cm.
- Plane: a tall straight trunk under a broad ovoid crown, camouflage bark
  (cream, olive and grey patches), large lobed leaves; a granite frame filled
  with bark mulch and gravel. Within 115 cm.
- Cherry: a low fork at 0.85 m into four spreading limbs under an umbrella
  crown, dark bark with pale lenticel bands; a frame with a ring of liriope
  tufts. Within 120 cm.
- Pine: a gently curved trunk (6 cm at 1.4 m), grey and plated below turning
  red-orange above, four or five flat needle pads in tiers (one hull per pad, so
  the tiers stay apart); a soil mound in a ring of ten stones. Within 85 cm.
- Young trees: a burlap trunk wrap to 0.60 m, a thin stem, and a wooden tripod
  of three stakes with two rails and rubber ties to the stem.

Every crown vertex plus its largest sway (and the low-tier hull grown back)
stays within the species' crown reach (mapkit-core `STREET_TREES`) at the largest
scale; at the smallest scale every vertex beyond 30 cm of the axis stands at
least 1.20 m up (`STREET_CANOPY_MIN_CM`), so crowns may overhang the
carriageway above vehicles. The twelve-sided trunk (young: the wrap) encloses
the 14×140×14 cm trunk box to 0.60 m, above car height. Pit fixtures stay inside
a 36 cm square (38.9 cm at 1.08, inside the 40 cm pit footprint) and rise to
4.5 cm, so the frame shows above the 3 cm city sidewalk. A crown turns with the
placement plus a quarter turn from the identity, so the pit stays square to the
curb.

Flower beds are two instances per piece, a rim with its soil and a planting
(about 120–790 triangles each). Rims, chosen per 48 m world block so neighbours
along a street match: planters take granite blocks with chamfered tops, red
brick with stretcher courses and a soldier-course cap, exposed-aggregate
concrete, or a dark green painted box with seams, a top lip and feet on a dark
plinth; median beds take only granite (30 cm curb stones) or exposed-aggregate
concrete. Plantings, by identity: bedding
annuals (leafy mounds with petunia, marigold, pansy and begonia heads, one
colour per clump), a clipped boxwood block, royal azaleas (lumpy mounds in
magenta bloom), or ornamental grass with a liriope edge. Median beds hold a
clipped azalea or boxwood hedge with rounded ends (per block), with or without
an edge of annuals (by identity); `median_bed_pine` adds the bed pine at its
centre. Rims enclose their proxy boxes by 5 mm, soil lies 2 cm under the rim
top, and everything stays within the box plus 3 cm and below 45 cm (planter),
60 cm (plaza planter) or 25 cm (median bed). A `far` box in granite with a
planted top (28 triangles) replaces both from the near range on.

The renderer draws them instead of their generated proxy faces
(`render_memory.DRAWN_OVER`); collision and occluders never read those faces. A
cell instances each mesh per 32 m square as MultiMeshes with leaf tint and wind
phase and the instances' object ids (`apply_vegetation`): street trees as
trees (near within `vegetation_near`, cards from medium, shadows by
`vegetation_shadows`, far crowns never ending), flower beds as `planter` (near
within 0.6 × `vegetation_near`, shadows from medium, the far box never ending
since beds are solid); low draws no cards and no shadows. The set is one claim
per resource cache (`render_resource_cache.claim_mesh_set`, 4 MiB: about
21,900 vertices and 62,000 indices in CPU arrays and GPU buffers, 2.94 MB, plus
the atlas as image and texture, the two materials and build arrays), claimed as
the cell begins and built one atlas, material, crown step or mesh per admitted
step (`street_planting_preparation`, cache stages `street_planting_<part>`); a
refused set fails the cell before it uploads anything. Each instance adds 64 B
to each of its group's buffers inside its 64 KiB object share. Cells without a
cache build a process-lifetime set. `street_planting_validator` checks the
build, memory, crown reach, canopy floor, trunk enclosure, pit fixtures, rim
enclosure and planting caps, variant choice, hidden faces with untouched
collision triangles, ranges and shadows per tier, refusal before upload and
release; rendered runs write a capture of every species and bed piece
(`MAPKIT_STREET_PLANTING_CAPTURE`). Distant cells draw far street trees and
coloured bed boxes (`DISTANT_RENDERING.md`).

## City streetlights

The real-city lamps (`builtin:city_streetlight_75/135/195/255/315/375` and
`_twin_75/135`, mapkit-core `CITY_STREETLIGHTS`) are procedural display meshes
(`godot/city_lamp_meshes.gd`) over their only collision, the solid 12 cm pole
box 330 cm tall. They follow the contract's frame: the foot at the origin,
local +x over the road, the lens (luminaire underside) at 310 cm, and every
display vertex inside the envelope (x from -25 cm, or -(reach + 20) cm on
twins, to reach + 20 cm; |z| ≤ 8 cm; y from -8 cm to 345 cm), with nothing
beyond x = 25 cm lower than 2.5 m.

- Near mesh (774 triangles for a plain single lamp, 1,172 for a plain twin;
  optional parts add up to about 400):
  - footing and base: an octagonal concrete footing skirt down to -8 cm, which
    hides DEM cut and fill at the foot; a base plate with four anchor nuts; a
    square base cover over the whole 12 cm pole box up to 0.6 m (above the
    car's 0.57 m), so a car always stops against a visible face; and an access
    door with a lock and screws on the road side;
  - shaft: a tapered 12-sided round or octagonal shaft (r 4.6 → 2.7 cm) with a
    section collar at 1.6 m, a ribbed anti-poster sheet from 0.64 to 0.92 m,
    and the white management-number plate with a yellow band and digit blocks;
  - arm: either a bent mast, curving out of the shaft top through a 24 cm bend
    and running level to the head, or a straight arm with a pole cap, two
    clamp collars and a diagonal brace;
  - LED head: a die-cast housing lofted from the spigot sleeve to a rounded
    nose, five heat-sink fins and a photocell on top, and the flat lens with
    twelve LED modules (two rows of six) and dark gaps;
  - optional parts: banner brackets with a bowed, double-sided street banner
    (four palettes: field, header stripe, pale band with a bar, border); a
    pedestrian arm with a small head and lens at 1.45 m toward the pole's back
    (single lamps only); a control box with a door seam and a conduit above
    car height;
  - twins: a second arm and head at -x.
- Far mesh (32 triangles; 52 on twins): a hexagonal pole, an arm bar, a head
  box and its lens.
- Halo mesh: one quad per head.

All three meshes serve every id. The arm reach is a per-instance value
(`INSTANCE_CUSTOM.x`, cm, from the id). The vertex shader
(`city_lamp.gdshader`) moves the head vertices by reach − 1.35 m and stretches
the arm span between its fixed root and the head (`UV2.x` is the share of the
stretch, negative on the twin side). Parts of other designs collapse to a
point (`UV.y` is the part).

Design choices are integer hashes:
- From the lamp's 256 m world square and its road axis (the yaw modulo 180°
  in 30° steps), so a street's lamps inside one square match on both sides.
  The square is a fixed grid, not the road: a row changes design where it
  crosses a square edge, the two sides of a street whose centreline runs
  within half its width of an edge fall in different squares, and yaws near
  a 15° + 30° k bucket edge can differ. The choices are the arm style
  (22 % straight), the shaft (35 % octagonal), the paint (galvanised 45 %, dark
  grey 30 %, bronze 15 %, warm white 10 %; heads die-cast light grey on pale
  poles, in the pole colour on dark ones), banner streets (30 %, with their
  palette) and pedestrian arms (30 %).
- From the object identity: control boxes (40 %), and the 15 % of a banner
  street's lamps that carry no banner.

Weather wets and snows the lamps like walls.

The lens and halos switch on while the sun is below the horizon, like
`lighting_role` 2:
- The lens is a frosted diffuser by day. At night it shows LED modules glowing
  white-hot over the 4000 K light colour (sRGB FFEDD6).
- The halo (`city_lamp_halo.gdshader`) is an additive, depth-tested quad that is
  never written to depth. It sits just under each lens, faces the camera and is
  pulled 0.2 m toward it. Its size grows with distance from 0.18 m and its
  strength falls in proportion, so far heads stay steady points of about three
  pixels. Its profile falls smoothly to zero at the edge. By day the halo
  nodes are hidden, and every halo vertex collapses where no environment
  reports the sun.

The ground pools come from a per-cell night light map (presentation
`lamp_map`, `lamp_map_rect` and `lamp_map_height`, mapkit-godot
`lamp_map.rs`):
- One RG8 texel every 50 cm (257 × 257 on a 128 m cell). Texel centres run
  from one cell edge to the other, so two neighbouring cells meet on identical
  border texels.
- R sums the pools of every head whose pool reaches the cell, neighbours'
  lamps included. A pool is an ellipse round the point under the head: 4.6 m
  along the road, 3.0 m toward the road and 1.9 m back toward the pole, with a
  bright (1 − q)² core and a (1 − q/4)⁴ tail that ends at twice those radii.
  Only cells that some pool's bounding box reaches get a map.
- G holds the light-weighted foot height of the lamps lighting the texel, in
  16 cm steps above the cell's `lamp_map_height` base; unlit texels repeat a
  lit neighbour's height so filtering never blends in a false one.
- The renderer turns the map into one texture per cell without mipmaps (the
  first road or sidewalk material makes it), so border texels stay identical
  at every distance, and binds it on the cell's road and sidewalk materials
  (`city_lamp_meshes.bind_light_map`).
- After sunset, `urban_surface.gdshader` adds the surface's own albedo × 1.5
  under the light, plus a faint sheen that grows on wet ground. Paint reads
  brighter than asphalt, and snow brightest. The light fades out on surfaces
  more than 2.4–3.0 m above the lit feet (a deck over the road, at the heads)
  or 1.6–2.6 m below them (a road under a lit one), so pools stay on the level
  their lamps stand on.
- One texture fetch per road pixel at night, on every renderer (Mobile and
  Compatibility included). No light nodes, decals or per-lamp loops.

The light map was chosen over per-lamp SpotLights, decals, additive ground
quads and a per-pixel lamp loop:
- The SpotLight pool holds 4–8 lights shared with vehicles, so pools would pop
  as lamps move in and out of it.
- Mobile caps decals per mesh instance, and Compatibility has none.
- Additive ground quads cut into crowned or sloped DEM roads.
- A per-pixel lamp loop costs ALU per lamp.

Grass verges, walls and vehicles are not lit by the lamps.

A cell instances every lamp of a 16 m square into one group of near, far and
halo MultiMeshes after its objects:
- Near: within `streetlight_near` (none/20/32 m) + 11.5 m.
- Far: beyond that, to the decoration distance + 11.5 m (64/112/192 m), from
  0 m on low. A 12 cm pole is under a pixel farther away, and loaded cells then
  pay no far draw calls; distant cells keep the pole proxy faces.
- Halos: from medium, as far as the far mesh, and only at night: as the sun
  crosses the horizon `display_quality.set_night_lights` hides or shows every
  halo node, so halos issue no draws by day.
- Near and far cast shadows from medium; halos never do.

The far node lists its object ids for the editor's pick. The proxy faces are
hidden (`render_memory.DRAWN_OVER`).

The set is one claim per resource cache (`claim_mesh_set`, 512 KiB):
- About 2,050 vertices and 5,920 indices in CPU arrays and GPU buffers
  (0.31 MB), plus two materials with their shaders.
- It is built one material or mesh per admitted step (`city_lamp_preparation`,
  cache stages `city_lamp_<part>`).
- Refused, it fails the cell before any upload.
- Cells without a cache build a process-lifetime set.

`city_lamp_validator` checks:
- every id and design against the envelope, the 2.5 m rule and the pole box;
- the lens under the head centre;
- the far and halo meshes;
- the street designs;
- the tier ranges and shadows;
- hidden faces, the groups and their instance data, and the lamp-map binding;
- memory and refusal;
- (rendered) the lens, halo and a lit road by night and day.

## Facade fixtures

Generated buildings carry display-only 3D fixtures
(`godot/facade_fixture_meshes.gd`, original, no external asset or brand): on
walls, projecting blade signs on commercial walls (Korean 돌출간판: vertical
lightboxes standing perpendicular to the wall) and air-conditioner outdoor
units under upper-storey windows of house, commercial and public walls; on flat
roofs, stair penthouses, water tanks, condenser units, parapet rims with guard
railings on part of the roofs, solar panel rows and antenna masts or satellite
dishes. Apartment and industrial walls and non-building geometry carry no wall
fixtures. Nothing generated, collided or sent over the network changes;
formats stay v1.

- Placement is native (`crates/mapkit-godot/src/facade_fixtures.rs`, rooftop
  candidates from `roof_fixtures.rs`) when a cell is decorated
  (`with_presentation`, `preview_document_packed`, `source_preview_packed`),
  because it needs the floor pitch (`facade_floor_cm`). It reads only the
  packed display arrays: vertical triangles of each generated solid (same
  object and facade seed) are grouped into wall planes (outward normal from the
  Godot winding, extent along the wall, base and eaves top from the facade
  seeds and tops), flat roof faces into roof planes (one height of one solid,
  with its packed roof frame). Coincident walls facing the other way (internal
  walls between a footprint's prisms, party walls of touching buildings) cover
  no facade, so nothing mounts there.
- Every choice is an integer PCG hash (the shader's `facade_pcg`) of the 12-bit
  record (rooftops: the 24-bit solid seed and roof height) and integer slot,
  column, row or attempt indices. Wall positions mirror the facade shader's
  layout (`atmosphere_surface.gdshader`, kept in step by hand): the wall frame
  u = dot(p, normalize(-n.z, 0, n.x)), bays of floor pitch × base (house 1.8,
  commercial 0.8, public 1.5) × (0.8 + 0.5 × look bits 9–11 / 7), two-bay
  sign slots read along -u, `facade_storeys` for commercial walls, the window
  rect per kind and look variant and the 0.035-floor sill ledge. Curtain
  walls (the record's finish pick against the ordered `facade_palette.json`
  weights) get no units.
- Blade sign: on about 45 % of sign-slot boundaries, 0.12 floors above the
  shop storey, 0.9–1.3 floors tall (clipped below the top and to the first
  three upper storeys, at least 0.7), 0.2 floors out of the wall from a
  0.04-floor standoff, 0.06 floors thick. Sizes are in floor pitches, so they
  are metres on 1:3 cards and scale with any other recorded map scale.
- Air-conditioner unit: on about 14 % of upper-storey windows (commercial from
  the second upper storey), at most three per wall, 0.27 × 0.2 × 0.1 floors on
  a 0.03-floor standoff and rack, hanging under the window's sill shadow and
  above the window below, centred or to one side, only where that band is
  tall enough.
- Rooftop fixtures stand square to the roof frame. Each slot (one penthouse,
  tank, condenser group, solar array or mast) tries up to 24 hashed positions
  in the roof's frame extent and keeps up to four whose footprint lies on the
  roof 0.2 floors inside its outline; the first of them clear of other
  triangles and of fixtures kept before it wins. Stair penthouse (a plastered
  box in the building's body colour, an overhanging slab in the roof coating,
  a steel door with a canopy and lamp, a louvred vent): roofs 2.9 floors up or
  more, one per roof (apartments up to three along their length), 0.95–1.25 ×
  1.2–1.6 floors, 0.88–1 floor tall. Water tank: apartments, offices and public
  roofs four storeys up (two on large ones), a small round one on 40 % of
  houses three storeys up and 30 % of industrial roofs; a square FRP panel tank
  (2 or 3 one-metre panels a side, on two steel beams, with a ladder and a
  manhole) or a round stainless one (35 %; hoops, a conical crown, four legs).
  Condenser units (0.3 × 0.3 × 0.22 floors, side-blow or top-blow, on steel
  sleepers) in rows of 2–5 on 70 % of commercial, 60 % of public and 50 % of
  industrial roofs, one or two on 15 % of apartments, one on 25 % of houses.
  Solar rows (1.2–3 floors long, panels tilted about 27° toward the frame
  direction nearest south, 1–3 rows 0.8 floors apart) on roofs up to 5.5 floors
  high (houses, industrial and public 35 %, offices 12 %, apartments 8 %). An
  antenna mast with two Yagi booms or a satellite dish on 15 % of roofs.
- Parapet rims stand on the roof edges with a facade wall of the same solid
  under them (never on internal or party walls or on the faces closing a solid
  at the cell edge), collinear edges merged into one run and runs under 0.5
  floors dropped: 0.12 floors high and 0.06 deep, 0.015 inside the wall plane,
  in the building's body colour under a light coping, in pieces of at most 24
  floors. At square corners (turns within about 26° of 90°) rims meet without
  a gap or an overlap: at a convex corner a run ends at the next run's inner
  face and the next starts at its outer face; at a reflex (inner) corner a run
  carries on across the next run's strip, which starts at its outer face. Other
  corners trim the run a thickness short, and a 0–2 % height step between runs
  keeps their tops from sharing a plane. On
  35 % of residential and commercial roofs a guard railing (posts about every
  0.6 floors, mid and top rails; galvanised, green, white or black) stands on
  the rim.
- A fixture is skipped when its box plus a 0.03-floor margin would meet any
  other triangle of the cell (neighbouring buildings, roofs, tree trunks,
  terrain, other walls of its own building, a separating-axis test against a
  4 m grid of the candidates), would leave the cell rect, or would not mount
  on its own wall; units keep clear of accepted blade signs, rooftop fixtures
  of other kept fixtures (a footprint separating-axis test; a condenser row's
  or solar array's own units may touch). A cell keeps at most 1,024 fixtures
  in at most 16 squares of 32 m. Measured on the 1:3 real-city cards (128 m
  cells, so every square of a cell is kept), Yeongtong cells ask for at most
  633 fixtures in nine of ten cells (2,054 at most) and Jongno's dense blocks
  1,298 (5,325 at most); about three quarters of rooftop fixtures are rim
  runs. Caps never follow generation order: clear candidates go blade signs
  first, then penthouses, tanks, rims (whole roofs by a hash of the roof), then the rest by
  a hash rank (of the design bits on walls, so a wall's three units spread
  over its storeys and bays; of the slot on roofs), and the 16 squares are
  those showing the most fixtures (ties by a hash of the square), the busiest
  blocks of a larger cell; presentation `facade_fixtures` lists them (10
  floats each: mount point, yaw, kind, scale, height, 24-bit design bits,
  width and depth) and `facade_fixtures_capped` counts those the cap dropped.
  Rooftop design bits carry the building's palette key and its solid look's
  body and roof entries (bits 0–8) and the mesh variant (bits 9–10).

The meshes (blade near 168 triangles, far 24; unit near 140, far 32; penthouse
94/22; tank 348/100; condenser 104/30; rim 354/10; solar row 96/12; mast 232/56)
share one material (`godot/facade_fixture.gdshader`): surface roles in UV.x, a
baked shade plus twice the mesh variant of the part in UV.y (the vertex shader
collapses parts of other variants: square or round tanks, side- or top-blow
condensers, railing or none, antenna or dish), face coordinates in UV2, the
design bits in instance custom data (three 8-bit channels, exact on every
renderer) and the blade's aspect or a rooftop fixture's floor pitch. Rims and
solar rows stretch along their length; rail posts and solar legs carry their
anchor in UV2 and keep their width (posts spread over the instance's length,
at most 31 intervals). The material binds the record palettes, so penthouse
walls and rims take the building's body colour and the penthouse slab its roof
coating. Blade faces are both broad faces of the lightbox, drawn by
`facade_sign_face` in vertical mode (the storefront signs' designs, palettes
and far settling), each read left-to-right from its own side; the housing is
charcoal, silver or white enamel with a raised rim and top cap, on two bracket
arms with wall plates. Units are ivory, light grey or beige with a
box-filtered fan grille (bezel, wire rings and cross over three blades and a
hub), side louvres, insulated pipes and a rack. FRP tanks show one-metre
panels with flanges and domed centres; solar panels one-third-floor modules of
6 × 10 cells in silver frames; doors a painted steel leaf with a frame
rebate, handle and louvre. By night blade faces glow like the storefront
signs (lit all night, × 0.55, fading with the surface effect distance), 70 %
of penthouse door lamps burn faintly and wash their door; rain darkens and
snow covers upward faces as on walls.

A cell instances each kind per 32 m square as near and far MultiMeshes after
its objects (`display_quality.apply_fixture`): the near mesh within
`fixture_near` none/24/40 m plus the group reach, the far mesh beyond to the
decoration distance (blade signs, penthouses, tanks; rims three quarters of
it, units, condensers, solar rows and masts half) plus the group reach; low
draws the far meshes of blade signs, penthouses and tanks only, medium and
high every kind. Blade signs, penthouses and tanks cast shadows from medium,
the other kinds' near meshes on high. The set is one claim per resource cache
(`render_resource_cache.claim_mesh_set`, 640 KiB: about 3,700 vertices and
5,500 indices in CPU arrays and GPU buffers, 0.46 MB, plus the material, its
shader and build arrays), claimed as a cell with fixtures begins and built one
part per admitted step (`fixture_preparation`, cache stages
`fixture_<part>`); refused, the cell skips its fixtures. The cell reserves
`render_memory.fixture_bytes` (384 B per fixture, 16 KiB per kind and square)
in its estimate and, before generation, the cap (`FIXTURE_BOUND_BYTES`,
2.4 MiB: 1,024 fixtures and eight kinds in 16 squares) when it has building
prisms; its lease carries the nodes and instance buffers. Cells without a cache
build a process-lifetime set. `cargo test -p mapkit-godot -- facade roof packed`
checks the hash, the ordered finish weights, the roof frame, outward blade
signs on slot boundaries and only on commercial walls, units under windows,
internal walls, clearance and cell-edge skips, rooftop fixtures inside their
roof's outline and clear of each other, rims along the outer walls meeting at
corners and skipping internal walls and cell cuts, the palette bits, the cap
and determinism; `facade_fixture_validator` checks the set (one part per step,
identical builds, clearance boxes with anchored posts and legs, sign faces,
palettes, memory), native placement at 1:3 on walls and roofs, instance
counts, transforms (scaled by kind) and design bits against the packed list,
visibility per tier, reservation, lease release, refusal, standalone previews
and (rendered) a blade face and a penthouse door lamp by night and day.

## City barriers

Generated barrier prisms (`MapDocument.barriers`: OSM fences, walls and
hedges) draw as display-only detail inside their unchanged collision prisms;
collision, surface probes, generation, the generated hash and the documents
are untouched, and the prism faces leave the cell's batches
(`render_memory.hidden_objects`) whenever the presentation lists the detail.
At presentation time `mapkit-godot/src/barrier_details.rs` matches each
generated prism (`asset_convexes` under the barrier ID) to its source segment
and classifies its ends by rebuilding the owner cell's cuts (cell rect, the
800 cm length lattice, heightmap grid lines and diagonals): an end on a cut
continues, an extension past a polyline vertex is a joint, any other end is a
span end (a line end, a road or structure gate). Runs follow the prism's own
centreline and side vector (the side vector is rounded and can be skewed up
to 15°; frames are right-handed); at joints turning 15° or more they stop at
the vertex, and the segment starting there adds one corner post or pier and,
for walls and hedges, a corner run over the outer wedge (centred on the
vertex along that segment, 98 % of the prisms' half width, length and
height, so it stays inside them and shares no plane with a run; hedges, giwa
caps and every far solid box draw it, other walls' near corner piers cover
the wedge); gentler joints reach a mitre's length past it, so neighbouring
runs never overlap coplanar faces. Posts and piers stand on the
barrier's arc-length lattice (one bay of the design), each owned by the piece
whose clamped half-open range holds it and kept inside its piece (no post
straddles a joint or a terrain crease), plus an end or 1.25 × gate post just
inside each span end (slimmer on a span-end piece too short for it); lattice posts within 0.3 bay of a hard joint, line end,
any gate of the barrier in the cell (a gate across a cell seam is not seen)
or, on a closed line with a gentle closing turn, the post at arc 0 give way. Sizes stay inside the prism at every map
scale (a size factor shrinks posts of thin authored fences). The cell's
presentation carries `barrier_runs` (13 floats per run: ground point and
ground vector, side vector, height, design bits, phase, end flags, ID index),
`barrier_posts` (13 floats per post: ground point, x vector per real metre
with the rise, side vector, height, bits, post kind, size factor, ID index),
`barrier_ids` and `barrier_capped`; over 8,192 runs or 16,384 posts a cell lists
nothing and keeps drawing its prisms. The 1:3 cards list at most 517 runs and
707 posts (Jongno) or 417 runs and 841 posts (Yeongtong) in a cell, natively in
12–39 ms on a development machine, every prism matched to its segment.

Designs (`mapkit-package/src/barrier_design.rs`, shared with the distant mesh)
hash the barrier ID and the real height (height × D of the map scale 1:D).
Fences under 0.95 m are pipe rails or wooden; 0.95–1.6 m steel bar panels
(plain, spear tips or a low arch per bay), welded wire mesh, wooden boards or
pickets, or chain-link; taller ones welded mesh, chain-link or steel bars.
Walls are red brick (a concrete cap or a soldier course), painted block,
precast panels between H-posts or ashlar stone (40 % with a giwa tile cap);
hedges take three leaf tones with a flat or rounded top. Paints, bays (fence
1.8–3.0 m, walls 2.0–4.0 m) and far opacities come from
`godot/barrier_palette.json`, which the shader binds and the design reads.

`godot/barrier_detail_meshes.gd` builds the procedural set from one shader
(`barrier_detail.gdshader`) in three materials (opaque, dithered infill and
blended far): mesh x runs 0..1 along a run (posts in real
metres), y 0..1 up and z −0.5..0.5 across the prism, with real-metre offsets
in UV2 converted by the vertex shader from the instance's height,
perpendicular width and map scale, so rails, tubes and posts keep real sizes
while wall and hedge bodies fill the prism; normals are authored in that frame
and corrected through the sheared, non-uniform instance basis. Parts collapse
unless their variant matches the design bits. Fence runs carry rails, frames,
tubes, boards and back rails; the infill (bars on a 16-per-bay pitch, 5 mm
mesh wires with V-folds, a 3 mm diamond lattice, pointed pickets) is a
box-filtered pattern on two faces 8 mm apart, alpha-tested near with wires at
least most of a pixel wide and an ordered dither as the pattern goes
unresolved (fewer than six pixels a mesh), where it settles to the fence's far
opacity. Walls show an inset body (5 cm real) standing on a plinth, under a
frieze and coping or the giwa eaves, slopes and ridge (the body meets the
plinth top and the eave band instead of overlapping their end planes, so an
exposed run end shows no coplanar faces), with brick, block, stone and panel
courses on world height (level on slopes) and joints on the bay frame;
patterns repeat whole per bay and noise follows world positions, so pieces
join seamlessly. Piers (brick, block pilasters, precast H-posts; end, gate and
corner piers) stand full width under the coping. Hedges draw with a bumped leaf
clump field, darker gaps and a fresher clipped top. Rain darkens and snow
settles on upward faces.

A cell instances per 32 m square after its objects: near fence runs, solid runs
(walls and hedges), fence infill, fence posts and wall piers, and far fence
faces (alpha blended, never discarded: posts, rails and infill painted at the
far opacity) and far solid boxes (piers, plinth and coping painted). Runs join
the square of their midpoint, posts and piers the square of the run they stand
on, and every group of a square carries the same custom AABB (the union of its
roles' instances), so Godot measures every role's range to the same centre and
the near-to-far handover never draws a part twice or not at all. Near groups
show within `barrier_near` none/24/40 m plus the group reach, far groups beyond
with a crossfade on Forward+ and hysteresis elsewhere, and never end
(`display_quality.apply_barrier`). A cell planned on low, which draws no near
meshes, plans one far fence and one far solid group for the whole cell
instead (two draws, one shadow caster, per barrier cell rather than one or two
per 32 m square); those groups draw from 0 on every tier until the cell
reloads after a tier change. Walls and hedges cast
shadows on every tier like the prisms, fence runs and posts from medium, the
infill on high, the blended far faces never. Planning walks 256 entries per
step. The set is one claim per resource cache (256 KiB: about 1,070 vertices
and 1,530 indices in CPU arrays and GPU buffers, 0.13 MB, plus three materials,
their shaders and build arrays), claimed as a cell with barrier detail begins
and built one part per admitted step (`barrier_preparation`, cache stages
`barrier_<part>`); refused, the cell fails before any upload, since its prism
faces are hidden. The cell estimate adds `render_memory.barrier_bytes` (640 B
per run, 192 B per post, 8 KiB per group); its lease carries the nodes and
instance buffers. Cells without a resource cache (standalone scenes) build a
process-lifetime set. Cells without a lease (the Editor's previews, which claim
the Editor's cached set, and standalone scenes) keep each run's top on the far
groups (`mapkit_pick_faces`) for surface picking. The distant mesh colours a
wall or hedge with its paint and a fence with its paint over the far backdrop
by its far opacity, the haze the near far faces settle to
([distant rendering](DISTANT_RENDERING.md)). `cargo test -p mapkit-godot
barrier_details` checks span ends only at line ends and the road gate, posts
once per bay across pieces and cells, phase continuity, hard joints with one
corner post, mitred gentle joints, corner piers on a closed wall and none on
hedges, corner runs inside the prisms at hard wall and hedge joints, lattice
posts once round a gentle closed ring, and determinism; `cargo test -p
mapkit-package barrier_design` the
designs per height band, whole patterns per bay and the distant haze;
`barrier_detail_validator` the set (one part per step, identical builds,
memory, the three materials, discard only in the near infill), native lists at
1:3 for every family (designs, corner, end and gate posts), every placed vertex
inside its barrier's prisms on flat and 12 %/6 % sloped ground, hidden faces
only with a list, groups, shared square AABBs (posts with the far group that
paints them), the whole-cell low plan, metadata, ranges and shadows per tier, the
estimate, lease release, a refused set and standalone pick faces.

## Display quality

`godot/display_quality.gd` provides `profile(level)`, `active()`, and
`apply(viewport, profile)`. Low/medium/high use 75/100/100% 3D scale, 256/512/768
combined vehicle particle slots, 48/96/160m directional shadows, 4/6/8 selected
local lights, and progressively stronger water. Sky quality selects one, two or three cloud
noise layers with a high cloud veil and sun halo above low. Leaf-only wind is
disabled on low and uses bounded 7 cm motion on medium/high (park vegetation: see
above, half strength on medium); trunks and collision
remain fixed. These fields update live along with surface material detail. Client owns preferences and
automatic adaptation. Profiles never enter map files or authoritative race state.
Profiles also expose a 96/128/160 m full-detail distance for consumers; Runtime
owns its optional-cell transitions, margins and replacement lifetime.
`surface_grit` (voxel ground grain, above) is 0 on every tier; only a consumer that
sets it on the profile it applies turns it on, and distant terrain follows that
profile through the quality targets.
The texture tier is captured by each new map resource cache; live changes update
resolution, water, effects, shadows, LOD and optional low-tier occlusion.

Water preserves the authoritative flat surface. Low uses one flowing wave and
simple foam; medium adds a second normal, depth colour/alpha and Fresnel sky tint;
high adds a guarded screen refraction. Shore foam softens shallow boundaries.
Depth reconstruction supports Mobile and Compatibility NDC conventions. No
cosmetic ripple alters buoyancy, water entry or surface height.

`surface_style_validator` compares generated/imported/distant swatches at every
quality tier, including daylight, night and snow, Low tile visibility and mean
luminance at detail/distance transitions. `test_richer_assets.py` reproduces
library bytes and the racing, original, facade and leaf texture tiers. `facade_validator`
checks the finish slots, the curtain-wall rule, the metallic cap, known PCG answers
and the per-record pick against a GDScript mirror of the hash (shares within 3% of
the weights over all 4096 records), and the own-look pick, its bindings and its
shares (within 2%) against a mirror whose known answers the Rust `facade_look` test
shares; rendered, it compares the shader's own pick on a
wall quad, daylight glass reflection, each finish's wall brightness near and beyond
the effect distance (within 15%), identical far walls and an own-look solid's own far
body. The mapkit-godot test `near_and_distant_solids_share_their_tops_and_looks_across_a_cell_cut`
checks one solid seed per top on both sides of a cell cut and that every matched
distant wall and roof takes the near look's colour. Render `generated_material_validator` to compare
GPU uniforms and pixels before corrective quality updates, with prepared/unbatched
geometry, cancellation and last-borrower retirement. Its optional
`MAPKIT_SKIP_PIXEL_COMPARISON=1` explicitly skips only pixel comparison.
`grass_field_validator` checks bounded index steps identical across batch
splits, exact plane heights and no grass share over a road strip, steep bank or
sliver, crease error, ring coverage, vertex bounds and an even blade cover
across handovers per tier, a medium field under high quality keeping its
medium reach, the field inside its fixed claim, bounded steady frames while
crossing blocks, a teleport leaving no other block's grass, reservations equal
to the estimate less unused grass on low and polar ground, the lawn shade only
while blades draw, a GPU-pressure dip, hidden and released cells, a refused
field and, rendered, no blade over a road strip and blades standing on a 19°
slope.
`vegetation_scatter_validator` checks reservations, tree/shrub body, card and far ranges and shadows per
tier, trunk coverage of the collision box, crown reach inside the placement
footprint, visible branches, a refused shared set (fails cells with trees,
leaves cells without plants untouched) and the release of every lease; `street_detail_lease_validator` covers
single-node trees/shrubs and the group MultiMeshes.
`tree_canopy_validator`, `richer_material_validator` and
`display_quality_validator` cover leaf/imported bindings, bitmap retention,
independent materials, denied admission, quality changes during preparation,
MultiMesh bounds, occluders and final resource retirement. `richer_material_validator`
also checks the voxel grain off on every tier, its clamped binding on surface
materials only, and distant terrain taking it both at job creation and through the
quality targets; rendered, `surface_grit_validator` checks that turning the grain on
and back off restores a flat earth quad's untouched pixels exactly and that, on, it
keeps the mean luminance within 2% while the cubes add tone variance, overhead and at
a grazing car-camera view. Track checks pair
`rc_venue_validator`, `pipe_material_validator` and `panel_geometry_validator`
with the rendered `rc_piece_gallery`; `RC_PIECE_KINDS`, `RC_PIECE_WIDTH` and
`RC_PIECE_CAPTURE` select supported catalogue variants and output images.
Weather appearance and target-device acceptance are user checks.
