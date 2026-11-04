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
racing tiles (the last step uploads the array) and the sign atlas follows (halved to
512×320 with 128px tiles). Shared context reservations are 10.375/38.5/83.5MiB for
128/256/512px tiers (`display_quality.tile_bytes`: `1MiB + 10 × size² × 24` racing
tiles + `5 × min(size, 256)² × 24` facade layers + the atlas pixels × 24), including
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
and `facade_tops` (one float, 1000 + top metres) on every generated building solid
(triangles of one record sharing a vertex): the seed's high 12 bits are the record seed
(`sha256(record ID)`, shared with the distant mesh) and its low 12 bits hash the record
ID and the solid's smallest vertex; the top is the eaves (the lowest vertex of the
solid's non-vertical faces above its base, so a gable ridge does not count), else the
highest vertex. Facade batches bind the seed as UV2 instead of UV (their materials are
world-space) and commercial ones the top as CUSTOM0.x. Storeys start at the solid's
base, so ground floors follow terrain. Commercial walls fit whole storeys: a shop
storey g = 1.5 floors (never taller than the solid), then n = max(1, ⌊(top − base − g)
/ floor⌋) storeys of (top − base − g) / n, so L ≥ 3 whole levels show the shop storey and
L − 2 taller storeys; a remainder under 0.75 floors stays a plain parapet, and floor-pitch
rows continue above the top (gable ends, taller parts of a merged solid). Meshes without
CUSTOM0 keep one-floor storeys, the shop storey included. The record seed picks palette entries
from `godot/facade_palette.json`, which the distant mesh (mapkit-package `distant.rs`)
reads too: the wall body (bits 0–2; brick and wood keep their own bodies, concrete
follows the use) and the roof (bits 3–5; concrete roofs green-painted or grey) at every
distance, and within the effect distance the pane pattern, the glass tint, the bay
width (0.8–1.3×) and, on apartment complexes, an accent colour (bits 6–8) on one bay
column every 5–8 bays and on the base storey. The solid seed varies brightness by ±4 %
and sets the night switch-off minute. Meshes without UV2 keep a per-plane seed, four
faded tones and scene-height storeys. The shop storey is split into sign slots of two
bays; in height fractions of g each slot has a darker stall riser (0–0.08), one door
0.3 floors wide with a frame (0.08–0.72; left, centre or right by a PCG hash of the slot),
display windows (0.10–0.68) with two or three mullions per bay, a transom (0.70–0.76), a
canopy shadow line and a fascia (0.78–0.98). The fascia holds a storefront sign (85 % of
slots) at the atlas content aspect 56:16, fascia-high, at most 88 % of the slot wide and
centred, and otherwise shows a plain charcoal, off-white or darker-body panel. Shop
glass lights at night. Signs come from the shared original atlas
`textures/sign_atlas.png` (512 tiles of 64×20 px, each sign in the central 56×16 px
with edge-repeat gutters, tiles downsampled one by one:
8 category rows of colours, a pictogram and glyphs of an invented script, no font,
text or external image; `scripts/sign_atlas.py` reproduces it). Signs read left to
right from outside and glow at night. The solid seed, base and height are flat varyings and
per-slot sign and lit-window choices use an integer (PCG) hash of the 24-bit seed and
the integer slot or cell, so each choice is constant across its pixels. Signs sample
with analytic gradients (anisotropic) and fade to one tile colour once a sign texel
covers more than four pixels. Bottoms and imported assets are unchanged.

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
and stall risers; shop frames, canopy shadow, fascia and signs keep the plain
response. Metallic stays at most 0.1 so a finished wall keeps its diffuse brightness
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
mullions and spandrels settle on their area share instead of aliasing. Window
openings are recessed: a reveal band 0.06 storeys wide (0.015 on curtain walls)
inside each pane edge stays wall, 18% darker, and on medium/high its normal tilts
towards the opening; the band fades out before it is narrower than about two pixels.
Edges on the cell boundary (ribbon windows) are continuous glass without a reveal.
Shop windows have their own frames and mullions (shop part 4) instead of reveals.
Glass (punched windows, curtain walls and shop windows alike) has roughness
0.08–0.15 per record and specular 0.55. On medium/high in daylight its tint keeps
45% of its strength and the glass adds what it reflects as emission: the procedural
sky's zenith/horizon gradient and cloud grey along the reflected ray (street and
opposite walls below the horizon), weighted by Schlick Fresnel from a 0.10 base and
scaled by the environment's daylight and cloud cover (`environment_data` pixel 0:
wet, snow, cloud cover, daylight), so dusk, overcast and night follow the sky; at
night it is off. Each pane leans up to about ±1.5° and reflects 70–130% as much; both
settle once a pane nears a pixel. Low keeps plain tinted glass. Curtain walls glaze
each cell between 4% mullions and transoms in neutral anodised grey above a dark
coated spandrel (no wall tile), in a reflective tint (blue, green, bronze or silver,
metallic 0.35); at night half as many of their panes are lit (a second PCG draw per
cell), at 60% strength, so a glazed tower does not outshine punched walls; shop
fronts stay fully lit. Other night window choices are unchanged.

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
used. A city tree's crown fits its placement footprint: street and authored city
trees have the 120 cm `CITY_TREE_RADIUS`; a zone tree has its zone's declared
radius (120–200 cm, the city importer uses 200), which the native presentation
lists per cell (`city_tree_radii`: object id → radius, only above 120 cm; display
only, at most one entry per object inside its 64 KiB display share). Without that
list (older native builds, standalone previews) every city tree is 120 cm.
`vegetation_meshes.tree_set` picks the mesh set and size:

- Street trees (footprints below 160 cm): three variants (round, columnar,
  spreading; canopy base 1.45–1.7 m, crown 1.8–2.9 m tall), scaled 0.85–1.15
  times radius/120: about 1.7–2.3 m wide and 3–5 m tall at 120 cm.
- Wide park trees (footprints from 160 cm): three broad variants (round, oval,
  spreading; canopy base 1.6–1.75 m, crown 3.0–3.4 m tall, thirteen lobes: a
  core, rings of seven and four and a top lobe), scaled 0.95–1.1 times
  radius/200: 3.1–3.9 m wide (spreading widest, oval narrowest) and 4.7–5.7 m
  tall at 200 cm, the leafy crown about as tall as it is wide and hanging low
  around a short trunk.

Chosen, scaled and turned by the object identity, every crown vertex (hull,
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
side, white, yellow or violet with a warm centre) bloom in a fifth of the 4 m
world squares (0.35/m² there, 0.012/m² elsewhere) on the 1 and 2 m rings. With
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
clears the shade when it shows no blades. The lawn itself tilts its normal by
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

- Signals (`signal_h` 1,484 triangles, far 112; `signal_v` 1,284, far 124): a
  bolted square base plate, flange and tapered round pole with a cap and a
  yellow push-button box, all inside the unchanged 8×220×8 cm pole box; a
  pedestrian signal (stop and walk figures under hoods) facing across the road
  at 0.85–1.0 m. `signal_h` adds a tapered mast arm with a collar, tie rod and
  end cap, and under it at 1.32 m a horizontal head; `signal_v` a vertical head at 1.85 m held by brackets above
  and below it. A head is a bevelled housing with three domed lenses and a
  hood over each lens on both faces (the hoods toward the pole of a vertical
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
near and far MultiMeshes after its objects with the street furniture range
policy for solid kinds (`furniture_near`; low draws only the far mesh, the far
mesh stays drawn like the faces it replaces, shadows on every tier); shelter
glass is one node per group, drawn at every distance on every tier, and never
casts shadows (`mapkit_furniture_glass`, tagged far for editor picking). The
set is one claim per resource cache (`render_resource_cache.claim_mesh_set`,
960 KiB: about 7,100 vertices and 13,500 indices in CPU arrays and GPU
buffers, 0.90 MB, plus materials, the lens shader and build arrays), claimed
as the cell begins (`chunk_renderer.begin`, before any upload) and built one
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
the weights over all 4096 records); rendered, it compares the shader's own pick on a
wall quad, daylight glass reflection, each finish's wall brightness near and beyond
the effect distance (within 15%) and identical far walls. Render `generated_material_validator` to compare
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
MultiMesh bounds, occluders and final resource retirement. Track checks pair
`rc_venue_validator`, `pipe_material_validator` and `panel_geometry_validator`
with the rendered `rc_piece_gallery`; `RC_PIECE_KINDS`, `RC_PIECE_WIDTH` and
`RC_PIECE_CAPTURE` select supported catalogue variants and output images.
Weather appearance and target-device acceptance are user checks.
