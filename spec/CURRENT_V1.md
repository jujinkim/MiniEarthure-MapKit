# Current map contract

The user-approved 2026-09-26 arcade/water decision replaces the earlier reader-v2
policy. All current own formats and protocols, including `.memap`, are version 1.
Readers accept exactly 1, with no historical loader or automatic converter.
Original files remain preserved; active packages are
exported anew from authored source. Build fingerprints invalidate disposable
geometry caches and world hashes separate changed content.

## In-memory authoring snapshots

`mapkit_package::working::WorkingSnapshot` owns copy-on-write height arrays and a
bounded `Resources` provider (memory bytes or confined project paths). The Godot
`MapKitWorkingSnapshot` exposes configuration/fork, queries, brush lifecycle,
sparse delta history, water operations, memory validation and common packed
cell generation. These operations never encode PNGs or write project/recovery
files. Explicit `materialize` validates and encodes only modified tiles as PNG16;
consumers own atomic publication and saved baselines. Disk-source hashes detect
external changes, and forks isolate concurrent preview/output workers.
Source descriptors remain stable across publication; saved PNG descriptors and
height baselines are separate, so earlier file imports can still be undone/redone
after sculpting and saving. Save As carries the referenced source payloads too.

Brushes follow existing spacing (new terrain 200 cm), accumulate fractional timed
height changes, interpolate paths and update all shared seam owners. Raise/lower,
fixed/numeric flatten and smooth have bounded radius/falloff/strength. Sparse
changes are capped at one million samples, working height arrays/resources at
64 MiB each; Editor also charges its 16 MiB Undo budget. No disk spilling or
format conversion is used. Raw height and common road-fitted surface queries are
separate; display, flood and physics generation share the fitting operation.

Water editing floods connected sub-level terrain triangles at a fixed clicked
height. Flat clicks do nothing; open map edges retain level. Symmetric integer
edge interpolation avoids cracks between incident faces. Terrain changes update
shorelines/islands, merge/split lakes, and higher overlaps delete complete lower
components. Complex lakes split into existing v1 bodies (512 vertices / 16 islands,
1024 bodies) while connected equal-level pieces remain one editor operation.
Search/union budgets and cancellation bound work. This adds no fluid simulation,
new disk layout or version number. Tests cover seams, timed/path editing, memory
versus materialized geometry, cancellation, multi-tile paths and water topology.

## Independent roads and composite worlds

An ordinary `Road` may explicitly opt into `design`: joined cubic
`control_points` (3n+1 source-centimetre points), `terrain_policy`
(`auto_fit`, `preserve`, `elevated`) and `shoulder_cm`. The sampled points,
widths, lane markings, sidewalks and graph node IDs remain part of the road.
Export verifies that the samples match the controls. Heights and shared tangent
frames come from the authored curve; integer terrain samples never pull the
carriageway up or down. Roads without `design` retain the explicit imported
terrain-following path. Opening a source does not add or convert designs.

`road_design::edit` moves shared endpoints, connected authored roads, attached
tools and connected track entries atomically. Same-level crossings require an
explicit shared node; separated bridges/tunnels retain their own surface level.
Original heightmap payloads remain immutable. Ordinary `auto_fit` roads derive
cut/fill and shoulders locally, choosing the nearest road edge, and deleting or
moving a road restores the previous terrain. Redundant terrain contacts beneath
the road are removed. Preserved/elevated roads reject terrain penetration on
execution export. Authored road tops use contact class 3.

`assembled_track.authoring.terrain_integration` makes a track an overlay on the
same document. `assembled_track::composite::apply_source` replaces only its owned
products, retaining terrain, buildings, water, assets and independent gimmicks.
`terrain_policies` provides per-instance fitting choices; `road_connections`
binds an entry to an ordinary road's start/end frame. Special meshes keep their
shape and receive terrain-based supports. Export checks terrain, independent
occupied driving space, road clearance and support interference without moving
or deleting source objects. Composite generation, distant display, occupancy,
costs and content hashes include both the environment and track.

`surface_attachments` bind actions/obstacles/rails to a stable `surface_id` and
`station_cm`; ordinary road IDs and `track:<instance-id>` share the resolver.
Derived physical geometry and rail interaction lines move with the source.
Existing course hashes/proofs become stale after driving-content edits. Free-roam
exports may have no course or disconnected route drafts; unsafe geometry always
blocks export. Race-track export additionally verifies its routes, ground start
and common checkpoints, then binds a course to all current source/assets without
inventing player completion evidence. Execution cells remain 32 m in products;
all current format versions and existing resource caps remain unchanged.

The explicit CLI `design-roads INPUT.json NEW.json` authors a new document.
`audit-roads PACKAGE` examines generated collision at 0.5 m intervals on five
width lines, checks junction fans, duplicate terrain contacts and road-paint
budgets, and enforces the default-world 12% design grade / 1 cm deviation limits.
Dense road paint is compacted within 1 cm for display; collision keeps its full
curve sampling and shader path/exterior arrays remain capped at 128 segments.

Ordinary map generation uses integer road arrangement, connected sidewalks, courtyard
buildings, indexed placement validation, authored tree assets and environment
profiles. The latest road partition and three-dimensional seam rules remain.
Regional source uses the local dependency closure, retaining complete context only
where implicit widths or repetitions require it. Decoder peak declarations are
mandatory and verified against payload bytes. Size, cancellation, allocation,
content hash and audit checks remain binding.

Generated caches have one MKCELL01 layout: fixed header, triangle/object arrays,
prism count and records, convex count and records, then a required u32 byte length and canonical gimmick JSON, then a required u32 byte length and canonical water-cell JSON,
including empty arrays/counts. Their key
includes BUILD_FINGERPRINT, generated version, world identity and cell. Cargo derives
the fingerprint from sorted relative source/schema/dependency paths and UTF-8 contents
with LF line endings and length delimiters. Absolute paths, platform, timestamps and
build output do not enter the digest. Archive roundtrip and corruption tests remain;
portable geometry vectors use a fixed test key independent of source edits.

The renderer's update_environment accepts `immediate` to apply road wetness/snow
without blending at a race boundary. It consumes resolved state and never advances
simulation time; precipitation animation remains live.


Authored placement validation uses the convex hull of the complete transformed
collision footprint, rather than its axis-aligned bounding rectangle. Placement
contact remains forbidden. Bridge/elevated corridors admit a support only when
its complete collision top is at least one centimetre below every interpolated
deck segment across the footprint. Ground/tunnel/underpass corridors remain
excluded. These checks charge the existing bounded placement work allowance.
The miniature kit and climate ground palette are renderer/source assets; surface
identities and physical material behavior do not change.

File-backed `read_with_budget(path, allowance)` bounds compressed input before
allocation and checks the existing validation peak before inflation. Godot exposes
`open_package_budgeted(path, allowance)` and `unpack_source(destination)`. Restore
uses the already validated immutable package, retains every original payload and
requires a new destination directory. Failed opens clear prior native state.
These APIs do not introduce another format or a migration path.


### Immutable source preparation and menu preview

The Godot bridge caches cell cost/archive descriptors on the validated source
(maximum 1,024 entries; the shared 4.25 MiB capacity constant is included in
both container read allowances). Authored
placement candidates use the existing spatial index. Asset hashes and costs are
computed once per source; packed display bytes are created lazily only on the
presentation path and shared across chunks. Generated geometry and canonical
hashes are unchanged. No headless path creates display bytes or GPU resources.

`cargo run -p mapkit-cli --example menu_preview -- INPUT.memap OUTPUT.json`
creates a v1 roads-and-bounds overview tied to the original package SHA-256.
It does not alter the package or grant spawn authority. Consumers must validate
the package and selected surface before entry. The `cache_population` example
creates valid disposable cell archives from a synthetic package for cache tests.

## Course authoring and completion references

`MapDocument.courses` is an optional list of up to 64 public current-v1 course documents.
Definitions include map ID, name, driving-content hash, ordered 3D sphere/upward-hemisphere
checkpoints (1–1000 m radius), circuit/sprint mode, ground/air start and explicit horizontal
direction. Circuit finish reuses checkpoint zero. Lap count is a hosting choice, not geometry.

A course may retain 0–64 stored checkpoints so incomplete maps remain readable.
The stable effective list keeps the first non-overlapping sphere/hemisphere in
source array order. Generated/sealed courses adopt that list; package reads keep
source bytes and hashes unchanged and expose a derived effective course. If gates
are removed its course ID is recalculated and prior completion evidence is omitted.
Racing requires at least two effective gates. `CourseBody.effective_indices`,
`Course.effective`, and the Godot course/edit APIs share this contract. Authoring
rejects new overlaps before modifying a draft. Continuous capsule entry geometry
is MapKit-owned; consumers own authoritative progress and resets.
Stored overlaps use the effective-list rule above. Surface labels are authoring hints, not completion constraints.

Optional completion references identify bounded opaque consumer records by hash, size and
`course-validation/<sha256>.mevalidation`. MapKit verifies bytes and references, never
certifies player completion. Courses and their records participate in package integrity but
are excluded from driving-content hashes in both containers. Projects preserve stale
references so consumers can display revalidation required after geometry/map changes.

Verification: package unit tests 15, course tests 2, indexed tests 16 and package contract
tests 20 passed locally. Runtime certification and application acceptance are separate.

## Dynamic light inputs

Consumers supply atomic generic light groups instead of inferred vehicle poses.
See [light group contract](../docs/ENVIRONMENT.md). Pool and shadow caps remain
unchanged; no vehicle design data or serialized map contract is added.

## Declarative driving structures

`MapDocument.gimmicks` declares at most 128 stable IDs. Each record has centimetre
position, millidegree Euler YXZ rotation, per-mille scale, 1–32 validated convex
parts, a surface and RGBA color. Motion is `static`, cosine ping-pong `translate`,
continuous `rotate`, `boost` or `launch`. Motion stores period/phase milliseconds,
world-space displacement/impulse, rotation axis and per-vehicle cooldown. No
executable user scripts are accepted. See [the reusable library](../examples/driving-library/README.md).

The 2026-09-26 extension adds target speed, launch height and directional air-ring
effects, plus parametric loop/cylinder tracks with separate inner/shell roles.
Tracks replace ordinary convex parts with a bounded shared mesh; all other
definitions retain the 32-part cap. See [special driving](../docs/SPECIAL_DRIVING.md)
for dimensions, deterministic quantization, cost and spawn exclusions.

Current v1 also permits `curved_faces`, at most 2,048 unique `[part, triangle]`
pairs on static convex structures. Indices must reference existing faces and
cannot accompany a parametric track. Quarterpipe presets mark only their inner
arc. Resolved rendering/collision use those exact triangles as a shared
`track_mesh.inner`, with the other faces in the blocking shell; original parts
remain the source and occupancy geometry. Resolution bakes scale once and adds
128 bytes per selected face to the conservative memory estimate. No name or
material inference grants a curved driving role.

Required safety bounds enclose the full transformed motion and authored landing
area. Validation uses a conservative integer L1 radius, bounded displacement,
period, scale, impulse and total extent; bounds must remain inside the map.
Generated cells include each intersecting definition. Consumers deduplicate by ID
within their world generation and retain a global motion phase through cell reloads.
`driving_window` extends the ordinary 3×3 set to include complete safety rectangles
for definitions touching that set. Consumers retain their own cell and memory caps.

Canonical source/chunk hashes, regional metadata, the packed `gimmicks_json` field
and archive audits include these definitions. Cost declares 16 KiB plus 32 KiB per
convex part per intersecting object/cell; occupancy includes per-part static or
translation bounds and full rotation sweeps. Hollow passages retain separate parts.
The generated archive layout changes within v1; build fingerprints invalidate
old disposable caches. There is no fallback decoder or original-package conversion.

`godot/gimmick_geometry.gd` is pure shared geometry/pose/contact-velocity math.
The consumer supplies authority time, generation, reset and activation policy.
MapKit never starts a simulation clock. Geometry is transformed once into scene
coordinates; scale is baked into physics shapes by the physics consumer.

## Non-solid water

`water_bodies` declares bounded polygon volumes with dry islands, surface/bottom
heights and horizontal flow. Shared queries exclude submerged ground from spawn
and recovery admission, retaining dry islands and bridges. Each generated cell
owns clipped display triangles and a complete volume record. Water never enters
solid collision triangles. Cost, archive audit, regional closure and content hash
include water; memory and cell caps are unchanged. Godot queries deduplicate IDs
across atomic cell records. Rendering uses the same outlines/heights and the
existing compatibility renderer, with depth tint, ripples and shoreline foam.

Road snow retention and generated face traits follow [ROAD_CONTACT](../docs/ROAD_CONTACT.md).

## Seeded assembly

Optional `assembled_track` metadata selects the modular track generator. A
standalone assembly uses its venue; a terrain-integrated authored assembly is
composed with the existing map as described above. Catalogue, generation, saved-package verification,
package/Godot/CLI interfaces and cancellation follow
[ASSEMBLED_TRACKS](../docs/ASSEMBLED_TRACKS.md). This extends the current v1 schema;
it introduces no old reader, automatic conversion or synthetic completion proof.

The current v1 special-track contract also accepts `swept_cylinder` with 2–512
`centerline` floor/normal/forward frames. See [assembled tracks](../docs/ASSEMBLED_TRACKS.md).
The canonical schema and source fingerprints identify this definition; no
version increment or migration is introduced.

## Race finish policy

`MapDocument.free_roam` is a required boolean in current v1. New manual and seeded
documents use false; external geographic/terrain imports set true. It participates
in both container content hashes, regional metadata and inspection responses.
Generated sources allow this policy edit through `reseal_track_document`, which
verifies exact current geometry for authoring/export and recomputes the generated course hash. Loading independently validates bounded saved geometry, paths/references and hashes; generator/catalogue fingerprints identify caches, never mandatory regeneration.

Sprint assemblies end in one `finish_plaza`: an 8m entrance, 16m circular floor
and 1.2m wall, open only at the entrance. `Assembly.finish_plaza` supplies the
checkpoint, center, recovery pose direction and bounds to consumers. Only the
first 4m of the entrance contributes race distance; no checkpoint or race distance
is added inside the plaza. Circuits have no terminal piece. Rendering, collision,
occupancy and budgets consume MapKit geometry.

Focused package assembly, both containers, strict schema, audit accounting and
course tests passed locally. Consumer driving acceptance is separate.

The sprint finish candidate is an explicit path sample in the terminal entry; if it overlaps an earlier gate, stable first-wins removal makes that earlier effective gate the finish. AI and progression use the same effective course. Focused `finish_plaza_and_editable_free_roam_keep_exact_source_validation` and Runtime `assembled_track_validator` verify the source/route boundary.


## Category generation and authored track graphs

[TRACK_AUTHORING.md](../docs/TRACK_AUTHORING.md) is the current track definition.
Settings use three candidate categories and 60/90/120s ±10% base-route time;
there are no mandatory family/obstacle occurrences. Full 3D instances, ports,
Bézier links, shared progress, branches and independent actions replace the
linear/grid-only source. Seed exact reproduction and manual source compilation
are distinct verification modes. Disconnected drafts save as projects but cannot
export execution packages; manual courses retain player completion requirements.
The document schema, generator and catalogue fingerprints change within v1.


## Grounded seed support policy

Current v1 requires `Assembly.supports` and `Source.grounded_supports`.
See [grounded seed structures](../docs/ASSEMBLED_TRACKS.md#grounded-supports).
The shared road slab, grounded floor and structural columns participate in exact
source reconstruction, hash, cell costs and memory accounting. Generated
sources retain this policy when converted to manual authoring. Independent
manual sources and external imports do not enable it automatically.


## Independent grind contract

Current v1 includes explicit `grind_lines` source, endpoint connections and
resolved cap geometry, plus RC attachment presets and common ordinary-road
refinement. See [the current line contract](../docs/TRACK_AUTHORING.md#independent-grind-lines-and-rc-attachments).
`E_GRIND_SOURCE`, `E_GRIND_CONNECTION` and `E_GRIND_BUDGET` reject malformed
geometry, unresolved endpoints and excessive interaction work respectively.


Current v1 permits `manual_flight` approach/landing declarations and bounded
explicit static `Source.structures`. See the [manual airborne link
contract](../docs/TRACK_AUTHORING.md#manual-airborne-links-and-static-authored-structures-2026-10-01).
No automatic action or player completion proof is implied.

2026-10-05 pipe minimum replacement: cylinder/swept-cylinder radius is at least
100cm (2m bore); assembled sizes are 200/300/400/600cm. Generated pipes uniformly
select 200/300/400cm, with the same one RNG draw at all difficulties. Tube ports
are at least 200cm; road/loop/halfpipe domains are unchanged. The catalogue exposes
`pipe_min_radius_cm` for Editor controls. Undersize source data is rejected with
`E_PIPE_DIMENSIONS`; there is no conversion or source mutation. Standalone default
radius remains 125cm. These are current v1 constraints, not a format increment.
## Placement yaw and distribution assets

General `Placement` and generated objects accept `yaw_offset_mdeg` in
[-360000,360000], default zero, added to `quarter_turns * 90000`. Source axes are
(x,height,map-y); the common renderer reflects map-y. Collision boxes become
rotated convexes for nonzero offsets. Occupancy, footprint rejection, cell costs,
near/distant rendering, lights and disposable archives use the same pose. Integer
vertices round once after rotation. Zero is omitted from canonical JSON. Own
format versions remain 1; build fingerprints invalidate generated caches.

Editor owns environment layout and asset distribution derivatives. MapKit owns
package validation, cell geometry and shared rendering. Authored asset sizes and
collision proxies use metres via source centimetres; GLB uses metres. Derivative
bytes have distinct content hashes; originals are never overwritten.

Scoped macOS arm64 validation: placement/assets/occupancy/cost/hash regressions,
two additive-yaw/convex/archive tests, indexed/region-plan regressions and schema
agreement pass. Native CLI/binding build and Editor document/asset loading pass.

Opaque shared rendering emits each multi-material mesh once. Single-object groups
also use MultiMesh custom data; exceptional material overrides receive an owned
seed binding while sharing shaders/textures. This removes the global per-instance
shader-slot ceiling without changing cell or memory budgets. Display-quality and
richer-material regressions pass, including yaw, both material surfaces, stable
lighting identity and immutable template materials.
Small-prop distance classification includes authored root scale; metre-scale
buildings are not classified from their unscaled source mesh dimensions.

Surface-area overlap validation uses exact integer triangle separation. Touching
edges and disjoint concave frontage/yard polygons are accepted; positive-area
overlap, including one centimetre, remains rejected. Emitted geometry and existing
work budgets are unchanged. Eight urban paving/sidewalk/occupancy regressions pass,
including the rounded-clipping false-positive fixture and reversed winding.

## Authored distant meshes and shared models

An `Asset` may declare `distant_path`, a second safe package-relative `.glb` path.
It uses the primary model's local origin, metre scale and placement transform.
Both files participate in package references, manifest hashes, source/asset byte
counts, validation workspaces and memory estimates. The optional field remains
part of the current v1 contract. No loader conversion or format increment exists.
The primary asset alone defines collision; changing display LOD never changes
surface probes, collision triangles, execution cells or admission budgets.

The distant path emits its authored triangles, node hierarchy, materials and
vertex tint. Quarter turns, continuous yaw and reflected node transforms apply
once. Open arcades remain open. Assets without a distant path retain the generic
display proxy. Packed distant colours are sRGB: linear GLTF PBR/vertex colours
are converted once, while explicit sRGB overrides retain their values. The common
shader converts vertex colours for Forward/Mobile as it does colour uniforms;
Compatibility consumes sRGB directly. Emissive signs preserve their authored hue
in both LODs and retain the existing daylight schedule. Water retains the clipped shoreline and islands without physical
triangles. Terrain uses the same broad variation in near and distant shaders.
Small authored props form a contiguous batch tail and follow the existing quality
distance limit. Batches keep the existing vertex cap; cancellation releases their
native geometry and GPU mesh owners. Parsing workspace and emitted triangle bounds
are charged before allocation; the 32 m execution cell and Runtime budgets remain
consumer policy.

`scripts/authored_assets.py` supplies original MIT models for houses, shops,
market hall, farm equipment, crops, tree growth stages, shrubs, rocks and bridge
piers. Paired files have immutable SHA-256 names and share the existing material
tiles (at most 512 px). Editor owns their world placement and recipes. Fine joinery
is omitted at distance while roof forms, tree crowns and architectural openings
remain. Near/middle distance still uses the common mesh LOD and instancing path.
`scripts/harbor_assets.py` adds six shop/apartment silhouettes with rear access,
roof equipment, three loading warehouses, container stacks, open gantry cranes,
trucks, quays, pipes and supported bridge piers. Urban night ambient light and
contrast preserve facade detail within the existing light-pool/quality budgets.

Scoped validation on macOS ARM64 / Godot 4.7.2: package library 26,
input-defense 12, package-contract 24 and authored-distance 3 Rust tests pass;
schema/public error agreement passes. Two authored-model tests cover reproducible
hashes, bounds, real openings and reduced triangle counts (for example, shop
2380→446, oak 840→236, cabbage patch 4320→72). CLI/native binding builds and Editor
save/history/paired derivation/resource-release regressions pass. A fixed-camera
common-render terrain comparison differs by at most 1/255 RGB. These checks do
not constitute map art or device performance acceptance.

The Harbor changes pass four model tests (paired bounds, openings and reduction),
the distant/display Rust regression and three authored-distance regressions,
including linear GLTF colour and unchanged explicit override values. CLI/native
builds pass. An actual-pixel night/day test passes on Compatibility and Forward
Mobile on the same macOS M1: near/far cyan/pink keep their hue and daylight disables
emission. Light-pool regression uses the current quality limits rather than the
retired platform-size assumption. The terrain colour comparison remains within
1/255 on Compatibility. No device/FPS or full-platform acceptance is claimed.

Forest authoring adds original cedar/broadleaf growth stages, saplings, fern
patches, fallen timber, ranger lodges, tents, open picnic shelters and piers.
Six total authored-model regressions cover hashes, reduced silhouettes and real
openings. Ground-cover patches are small spatial units using the existing quality
distance limits; static trunks and structures retain primary collision.

Distant water uses the same quality-selected water shader as nearby water,
including depth, refraction and map-to-scene flow. The packed display kind byte
is 0 for solids, 1 for the small-prop tail and 2 for water; the existing two UV
channels carry flow for water and light identity/role for solids. This is a
display payload change under v1, not a physical or source-water change. Batches
split on kind and share at most two job materials. The existing 64 KiB fixed
far overhead covers these materials and the additional two run boundaries;
per-triangle/cell scheduling limits are unchanged. Materials and geometry retire
with the original far lease. Colour/quality, flow, collision invariance and
last-owner tests cover the change; renderer fingerprints invalidate old caches.

On macOS M1 / Godot 4.7.2, the water correction passes the distant, authored-distance
and water-contract Rust checks (1+3+1), native binding/CLI builds, and actual near/
far water pixel checks at low/high quality on Compatibility and Forward Mobile.
Mean RGB delta is below 1/255 in both backends; far water owners release. The
existing terrain colour check also passes. This is bounded display verification,
not a device or whole-map performance acceptance.

The original `canyon_assets.py` library adds four eroded sandstone profiles with
visible strata, talus, cactus/scrub, a quarry office, loader, open crusher bay,
conveyor and open bridge piers. Primary/far meshes keep the same frame and major
openings. Two focused model tests pass hashes, bounds, reduced rock triangles
and open machinery/bridge rays; native source validation checks closed collision
geometry. No new material/format version or runtime cap is introduced.

The common sky now blends its horizon haze continuously into the lower
hemisphere. A hard colour step exposed a dark band behind bounded canyon terrain.
The day/night `sky_horizon_validator` actual-pixel test passes Compatibility and
Metal Forward Mobile on macOS M1/Godot 4.7.2; no display radius or geometry
budget was changed.

`snow_assets.py` adds three snow-covered fir ages, granite crags, two timber
lodges, snowbanks, road rails, an open shelter and a solid frozen tarn. The lake's
12-sided collision hull stays within the current 32-vertex/60-face limits; its
near model adds surface cracks. Two model tests pass matching bounds/hashes,
reduced distant meshes, the physical ice top and open shelter bays.

`factory_assets.py` adds three multi-bay production halls, process tank shapes,
boiler banks, 62 m stacks, supported pipe racks and feed branches, transformer
busbars, compressors and administration buildings. The library shares original
Harbor warehouse/crane/truck components. Two model tests pass reproducible hashes,
paired envelopes, reduced details and preserved rack openings/connection spans.
Physical declarations and all v1 budgets are unchanged.

`park_assets.py` adds an open observation wheel with twelve gondolas, a static
carousel, decorative coaster, three kiosks, queue rails, flowerbeds and two
heights of open viaduct piers. Wheel and coaster legs have explicit physical
hulls; the coaster's thirty legs plus its station roof fit the current 32-hull
limit. Far meshes preserve the wheel spokes, gondolas, coaster rails and station
access ramp while reducing detail. Two focused model tests pass deterministic
hashes, paired bounds, reduced landmark triangles, hull limits and open wheel/
pier rays. No format, collision policy or render budget changed.

Selected-course `.memap` sharing and the optional integrity-bound, non-driving
`preview.png` use the current v1 [container contract](FORMAT.md). Sharing preserves
already validated stored geometry and never fabricates completion evidence.
