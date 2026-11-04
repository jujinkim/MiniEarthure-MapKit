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
used. Three tree variants (round, columnar, spreading; canopy base 1.45–1.7 m,
crown 1.8–2.9 m tall) are chosen, scaled 0.85–1.15 and turned by the object
identity. Every crown vertex (hull, cards, the low-tier hull) reaches at most
1.2 m from the trunk at the largest scale, inside the 120 cm placement footprint
(`CITY_TREE_RADIUS`), so crowns never reach into facades, fences or lanes the
generator keeps clear; shrubs stay within the former 42 cm. Each tree or shrub has
three meshes:

- `body`: a tapered eight-sided trunk with a root flare whose inscribed radius
  keeps the 14×140×14 cm collision box hidden at every scale, two or three
  branches forking at 1.1–1.4 m and climbing out under the crown's lifted
  underside, and the crown's inner hull (an icosphere pushed out to 80 % of the
  union of five or six jittered lobes, 320 triangles) in a dark gap colour with
  vertex ambient occlusion, about 480 triangles in all.
- `cards`: 150–174 alpha-scissor leaf cards just outside that hull, forming the
  silhouette (300–348 triangles).
- `far`: an 80-triangle hull at the full crown and a plain six-sided trunk.

Shrubs have an 80-triangle body hull, 26–38 cards and an 80-triangle far hull.
Cards sample the original 2×2 leaf-cluster atlas
`godot/textures/vegetation/leaf_atlas.png` (`scripts/richer_assets.py --textures
--leaves`, data channels: luminance, yellowing, coverage); the body projects the
same atlas on its hull as a leafy pattern. On low (no cards) the body's hull grows
back to the full crown in the vertex shader and takes the far hull's brightness,
so close trees keep their lobed crowns and visible branches.

`godot/vegetation_leaf.gdshaderinc` has three variants (body, cards, grass), each
a preloaded shader (`vegetation_leaf`, `vegetation_leaf_cards`, `vegetation_grass`)
bound once by a warm-up material when the renderer scripts load, so its
compilation never falls into a streaming step. Wind
sways vertices by a weight measured from the canopy base (0 on trunks, 10 cm at the
crown top at full strength) with coherent gusts; a card flutters 8 mm as one piece
with its own phase, blade tips 12 mm. The phase and a hue/brightness-varied leaf
tint come from `INSTANCE_CUSTOM`. Leaves back-light warm green, darken when wet and
take snow on upward faces from the environment pixels; sky-facing cards keep their
yellowing and back-light low. Card alpha keeps its coverage with the mip level and
fades cards seen within 27° of edge-on (gone within 12°). The Client renders without MSAA, so
scissor edges stay pixel-stepped; nothing here relies on alpha-to-coverage.

A cell instances trees and shrubs per kind, variant and 32 m square: one admitted
step plans each placement, then each group attaches a body, a cards and a far
MultiMesh with custom data. Display quality measures every range to a group's
centre and adds its 23 m half diagonal, so a plant within `vegetation_near`
(24/32/48 m, shrubs 0.6×) always draws its near meshes. Body and cards hand over
to the far mesh at that distance with 6 m of margin, a dithered crossfade on
Forward+ and hysteresis on Mobile (which does not fade). Low draws bodies without
cards. Far trees have no end range, like the canopies they replace, so every tree
of a loaded cell keeps a crown; far shrubs end at the decoration distance. Tree
shadows follow `vegetation_shadows`: none on low, bodies and far crowns on medium,
also cards on high; shrubs and grass cast none. The meshes, three materials and
the atlas are one shared set per resource cache (`claim_vegetation`, 2 MiB, built
one part per admitted step: the atlas, each material, each mesh); cells without a
cache build a process-lifetime set the same way, and standalone previews draw
single body and cards nodes.

Grass tufts (24 curved blades in a 0.2 m clump) are scattered by
`godot/grass_scatter.gd` on a cell's grass ground triangles (steep banks excluded)
after its objects: density is the lower of 0.9/m² and the cap over the grass area,
and every position, size, yaw, tint and flower is an integer hash of the cell and
the triangle's centimetre vertices, so batching and step boundaries never move a
tuft. Flowers (white, yellow or violet heads tilted 50° to face eye height) cluster
in a fifth of the 4 m world squares (40 % of tufts there, 2 % elsewhere). Each tuft
joins the group of its own 32 m square. The cap follows the tier of the cache
textures (`grass_cap`: 0/1,500/4,000), which adaptive quality keeps at the user's
ceiling, so a GPU-pressure dip only hides grass (`grass_distance` 0/24/40 m) and
never leaves cells loaded meanwhile bare. Area and scatter steps stop at 1,024
triangles, or 256 triangles or 256 tufts, or 1.5 ms of wall time; each group
attaches in its own step. Tufts follow the lawn's lush/dry patches, shrink into the
ground over the last 10 m and are released with the cell; polar ground and cells
with under 32 m² of grass have none. The lawn itself tilts its normal by the fine
blade noise gradient within 30 m, fading wherever a pixel spans a third of the
finest octave's period, so grazing views do not shimmer. Renderer diagnostics
report the shared build as `vegetation_preparation` (cache stages
`vegetation_<part>`), scatter as `grass_scatter` and group attachment as
`instance_upload`.

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
`vegetation_scatter_validator` checks grass determinism across batch splits, the
per-tier caps (none on low or polar ground) and their reservations, bounded steps,
the texture-tier grass cap, tree/shrub body, card and far ranges and shadows per
tier, trunk coverage of the collision box, crown reach inside the placement
footprint, visible branches, a refused shared set (fails cells with trees, skips
grass) and the release of every lease; `street_detail_lease_validator` covers
single-node trees/shrubs and the group MultiMeshes.
`tree_canopy_validator`, `richer_material_validator` and
`display_quality_validator` cover leaf/imported bindings, bitmap retention,
independent materials, denied admission, quality changes during preparation,
MultiMesh bounds, occluders and final resource retirement. Track checks pair
`rc_venue_validator`, `pipe_material_validator` and `panel_geometry_validator`
with the rendered `rc_piece_gallery`; `RC_PIECE_KINDS`, `RC_PIECE_WIDTH` and
`RC_PIECE_CAPTURE` select supported catalogue variants and output images.
Weather appearance and target-device acceptance are user checks.
