# MEMAP v1 development specification

Status: implementation draft; not yet the active MiniEarthure game contract.
The public package/core/CLI are independent of private game code. The current
renderer and editor are prototypes. See LIMITATIONS.md before production use.

## ZIP and JSON

A `.memap` is an unencrypted standard ZIP, with `manifest.json` as its first
physical and central-directory entry, no prefix, and only regular stored/DEFLATE
entries. Export uses DEFLATE level 9, 1980-01-01 00:00:00, Unix 0644, then
lexicographically sorted payload paths. Producer environment is not read into
exports. The manifest is excluded from its own file inventory and hash.

`document.json` is the map source. Extracted project directories use this same
file plus relative payload paths; editor history/recovery is stored separately.
An unpacked package may be edited by any compliant tool and repacked.

Canonical JSON: UTF-8, no insignificant whitespace/newline/BOM, keys sorted by
Rust BTreeMap string order, integers only, serde_json string escapes. IDs and
paths are case-sensitive; paths must also be unique after ASCII case folding.
Numbers visible in Godot must be exactly representable: seed <= 2^53-1; geometry
uses bounded signed integer centimetres. Unknown fields are rejected for this
version. Duplicate JSON object keys, duplicate ZIP paths, Windows device names,
backslashes, absolute paths, parent components, symlinks and special files fail.
Paths are portable ASCII, at most 240 bytes; allowed component characters are
letters, digits, spaces, hyphens, underscores and dots, without trailing dot/space.

Manifest metadata mirrors document map ID/revision/bounds/cell size/seed/theme,
assets, attributions and tool provenance. The inventory binds every payload size
and SHA-256; extra payloads and missing files fail. Tool fingerprint has no trust
semantics. First-created and last-edited values are supplied metadata, never an
export-time clock read. Third-party tool IDs and fingerprints are accepted.

`package_sha256` hashes exact ZIP bytes and is transport/cache identity, reported
outside the ZIP. `world_content_hash` hashes canonical `[gameplay_document,
payload_hash_map]`; normalize object arrays by stable ID and heightmaps by cell,
remove document provenance and top-level attributions, exclude document.json
from payload_hash_map. Asset attributions currently remain content-bound.
Object generation seeds hash `[map_seed, object_id, rule_id, lattice_x,lattice_y]`;
provenance, participants, spawn, processing order and rendering quality do not
participate. GeneratedChunk v6 is canonical integer JSON; its SHA-256 binds
terrain, triangle surface IDs, spawn eligibility and object placements.

## Spatial model

Local UI x/y metres normalize to integer centimetres. Vertex order is
`[x_cm,height_cm,y_cm]`; Godot vector is `(x,height,-y)*0.01` (actual metres).
Scene-units contract 2 replaces the former global 1:8 display scale. OSM scale,
when desired, is applied once by the importer; custom documents are never rescaled.
Generated format 6 retains integer centimetres and unchanged archive bytes.
Map bounds are inclusive for point validation, with maximum-edge points owned
by the last cell. Cells start at bounds.min and use nonnegative x/y indices,
default 51200 cm. A 3x3 start window contains only existing cells.
Objects are stored once globally. Geometry is clipped into intersecting cells;
manual objects and vegetation instances have one owner cell from their anchor.
Roads connect only through explicit graph nodes, whose position and level are
separate from 2D crossings. Width and surface arrays correspond to segments.
Spawn requests select exact surface ID; building triangles are never spawnable.

Heightmap PNG: unsigned 16-bit grayscale, `(cell_size/spacing)+1` square samples,
row increases local y, column increases local x. Height is offset_cm+sample*step_cm.
Minimum spacing 200 cm. Adjacent grids must currently have identical spacing and
identical restored edge heights, including edges against implicit flat terrain.
Source accuracy metadata is independent of spacing. Partial map-edge cells retain
full grid dimensions and generation clips to map bounds.

### Recipe 2 roads and structures (K05)

Readers support recipes 1 through 5; the current recipe constant is 5. Export writes
**the document's recipe**, and the manifest must agree (`E_MANIFEST`). An unknown
recipe fails `E_VERSION`. Existing recipe-1 source, package bytes, generation-v6
bytes and disposable archive keys are preserved. No file is migrated on read or
save. Choosing recipe 2 is an explicit author edit and changes world identity;
package v1 and the generated-v6 representation do not change. The reference
producer and `examples/roads/document.json` demonstrate this choice without any
private dependency. Editor tool UX remains a separate authoring implementation.

Road graph adjacency is exactly `from`/`to` node identity, including the node's
position and level; `MapDocument.connected_roads(node_id)` returns sorted incident
IDs. XY crossings, coincident but distinct nodes and vertical overlap create no
connection. Levels label graph endpoints; authored elevated/bridge/underpass/tunnel
centrelines supply the physical heights and ramps between levels.

Ground roads follow the restored piecewise terrain, including longitudinal and
crossfall changes inside a segment. Their stored height values remain source
reference elevations; they do not flatten sampled terrain or raise a deck. Use an
explicit structural kind for an independent vertical profile. Widths and the four
road materials (asphalt, concrete, dirt, gravel) apply per segment. Grass is also a
valid authored surface. Ground ribbons and explicit junction aprons partition the
terrain triangles into one road/terrain surface, without an overlapping grass
collider or a separate vertical offset. Overlapping ground paint uses normalized
road-ID/triangle order for deterministic material/ID ownership, not inferred graph
edges. Source IDs and widths are never rewritten.

At an explicit node or polyline bend, each arm stops at a mouth displaced by the
largest incident half-width, capped at 45% of its segment length. A convex apron
joins these mouths through the exact source node. Segment width is exact outside
this transition; apron triangles interpolate the authored mouth/node elevations
for structures. Stable incident-road IDs/materials label apron faces. At most 32
endpoint arms are accepted (`E_LIMIT`). Structural mouths must remain distinct
perimeter edges of the apron: overlapping/too-short/acute approaches fail
`E_GEOMETRY`; authors add separated approach points instead of receiving ambiguous
or overlapping decks. Joined tunnel clearances must agree. Mixed ground/structure
aprons must match actual local terrain within 1 cm or generation fails
`E_GEOMETRY`; author a terrain-level approach before the grade. This keeps portal
connections explicit and avoids accepting a gap between sampled ground and a ramp.

Elevated roads and bridges keep their own drivable deck above/below other surfaces.
Underpasses cut their entire open corridor out of terrain. Their side walls reach
at least the supplied clearance and extend to higher restored terrain, subdivided
at terrain triangles. They have no ceiling. Tunnels keep the authored floor and
clearance-height ceiling, with side walls and open graph endpoints. Terrain is cut
where it intrudes below the ceiling, including the portal/grade approach, and is
retained above the bore. A crossing deck is never removed by a terrain cut. Floors
are spawnable; walls/ceilings are not. Creator geometry remains responsible for
clearance against independently authored intersecting structures and buildings.
All emitted faces use the same renderer/collider stream and cell clipping.

Plan/cut work is bounded before unbounded accumulation: at most 2048 local
corridor segments, 16384 local arms and combined patches/walls, 16384 live fragments,
65536 live fragment vertices, and 8000000 subdivision operations. Exceeding a limit
fails `E_BUDGET`, with no partial returned chunk. Cost estimates expose an additional
`generation_scratch_bytes` allowance (16 MiB for recipe-2 maps containing roads;
zero for recipe 1), independent of output counts. Consumers must reserve it through
worker retirement. Source-derived output counts are also enforced as a generation
allowance; complex input may fail rather than exceed its estimate. These are
bounded CPU planning allowances, not completed RSS/GPU/performance calibration.
Recipe-2 vegetation skips removed terrain; it never invents support over a cut.

### Recipe 3 buildings and placement (K06)

Recipe 3 is an explicit author choice. It retains recipe-2 roads and adds the
following domain rules. Recipes 1/2 retain their exact source serialization,
generated bytes/hashes and archives. New optional `repetitions` and building
`entrances` collections are omitted when empty; nonempty extensions require
recipe 3. No document is upgraded during load/save. `examples/placement` and the
stdlib producer demonstrate all new features without private dependencies.

Buildings keep authored simple (including concave) footprints, absolute base and
height. Supported `usage` values are `residential`, `commercial`, `industrial`,
`public`; `material` is `concrete`, `brick`, `wood`. The common renderer combines
material color with a use tint. Unsupported labels fail instead of silently
rendering another roof/material. `flat` roofs support any accepted simple polygon.
`gable` requires an axis-aligned rectangle, at least 2 cm on both axes. Its ridge
runs along the longer dimension (local y on ties), halfway across the short side;
rise is one quarter of that side, clamped to 1..1000 cm, above authored wall height.
Integer roof planes are shared by display, physics and occupied-volume queries.
Buildings never become automatic spawn surfaces. Authors supply a base below
local ground: generation does not infer neighbouring terrain or alter source height.

Overlapping building horizontal footprints with overlapping height envelopes and
building/road corridor overlap fail `E_GEOMETRY`. Road clearance is deliberately
conservative in XY, including elevated/underground roads; this authoring profile
does not support placing a building over an independently authored road. Each
building may declare `entrances: [polygon,...]` as explicit access corridors.
All polygons obey ordinary bounds/simplicity limits. They suppress automatic
props/vegetation/sidewalk pieces and reject conflicting manual placements.

Generated v6 has an optional `building_prisms` array, omitted when empty. Each
record has `object_id`, `material`, `usage`, three-point `footprint`, `bottom_cm`
and three `top_cm` heights. It is a convex vertical triangular prism with a planar
sloped top. Parts are clipped to cell bounds; concave footprints remain unions,
not bounding boxes. All six vertices and all eight triangle faces are generated
by MapKit. Consumers attach `ConvexPolygonShape3D` parts as well as the canonical
triangle stream, so interiors are solid. The array participates in canonical
hashing before `cell`; old empty-array omission preserves v1/v2 hashes. Old
executables reject the new source recipe; consumers require synchronized execution
identities before transferring a map. This is an explicitly recipe-gated additive
v6 contract, not permission for old consumers to ignore new solid primitives.

Packed geometry exposes `building_prism_vertices_cm` (18 integers per part),
`building_prism_object_indices`, and per-object-ID `building_materials` and
`building_usages` strings. The shared data adapter reads packed and JSON forms.
Arrays remain immutable-owner/COW views. Archives with parts use `MKCELL02`, adding
a bounded part count and records after objects; empty-part archives retain exact
`MKCELL01` bytes. Trusted source estimates, actual remaining bytes, vertex/material
validation and the complete generated hash guard decoding; a corrupt cache cannot
remove solid interiors from a valid hash.

Manual placements additionally support `builtin:tree`, `builtin:fence` and
`builtin:streetlight`, without custom asset files. Their box dimensions in cm are:
tree trunk 40×400×40 at center (0,200,0); fence panel 200×120×12 at (0,60,0);
light pole 20×500×20 at (0,250,0) plus lamp 80×25×40 at (0,500,0). Existing quarter
turns rotate proxies. The common renderer draws these exact proxy faces; trees
also use its existing canopy. Custom assets still use their declared box proxies,
and recipe-3 manual assets require at least one proxy to define clearance. Custom
GLB appearance/convex proxy authoring remains K07. Manual full footprint conflicts
with map bounds, buildings, access corridors, roads or other manual footprints
fail. Multiple custom boxes use a conservative combined rectangular footprint.

`repetitions` records contain stable `id`, builtin fence/light `asset_id`, absolute
3D `points`, and `spacing_cm` (200..100000). Fence paths require cardinal segments;
light paths may be diagonal. Portable integer-centimetre arc length uses floored
libm segment lengths. Spacing carries across segment joins; half-open segment ends
never duplicate a joint. Fence anchors start at half-spacing, lights at zero;
there is no forced final endpoint. Orientation chooses the nearer cardinal axis.
IDs are `source_id:repeat:index`, with index taken before exclusions. Conflicting
automatic anchors are skipped in normalized source order; manual placements take
priority. Fence panels may share edges but never positive-area interiors. Curves,
mitred corner panels and arbitrary orientation are outside this declared profile.

Vegetation uses a global signed lattice, SHA-256 rule `vegetation-v3`, density
0..1000, spacing 25..100000 cm, orchard zero jitter and forest ±spacing/3 jitter.
The full 400×400 cm canopy envelope (including the 40 cm trunk) must lie inside its
zone/map and clear exclusions, all building footprints/access corridors, manual
and accepted repeated props, roads plus 100 cm and their planned sidewalk width.
Corner containment alone is insufficient: edge crossings and concave notches are
checked. Source-global local-minimum thinning compares eligible neighbours by
`(rank,zone_id,lattice_x,lattice_y)`. Overlapping canopy envelopes are forbidden;
centre distance is at least the larger spacing. A lower-priority candidate is
discarded even when its competitor is itself discarded, so this rule can yield
less than requested density. Density is candidate probability, not a count quota.
It never fills capacity by inventing fallback positions. Generation order, cell
availability and render quality cannot change selection.

Vegetation instances and their entire small trunk are emitted once by the anchor
owner, including a trunk's possible 20 cm overhang into a neighbour. This explicit
recipe-3 exception to face clipping fixes the former half-trunk seam. The existing
20 cm query-owner halo still applies. Repeated/manual proxy faces and building
parts are clipped into touching cells; display object records have one owner.
`builtin:`, road `:sidewalk` and repetition `:repeat:` namespaces are reserved.

Ground-road sidewalks use `sidewalk_cm`: null/missing selects the theme rule,
zero disables, 20..1000 supplies an explicit width. Structural roads emit none.
Theme `rural` generates none automatically. `urban` roads at least 400 cm wide use
180 cm, or 250 cm when at least three buildings are within 30 m of the corridor.
`default` uses 150 cm for roads at least 500 cm wide with two nearby buildings.
Other cases use zero. Automatic widths shrink in 25 cm steps, down to 50 cm,
to fit each pieces building setback. Explicit widths never shrink. Pieces that
still cannot fit are suppressed, never pushed into a building. Each side is
divided into at most 200 cm pieces, raised 12 cm over the exact restored terrain
with matching risers. Road half-width + sidewalk width + 100 cm is left open at
every endpoint/bend. Whole pieces intersecting another road plus 100 cm, a building,
entrance or manual footprint are suppressed. Sidewalk identity is `road_id:sidewalk`.
This conservative policy can leave larger openings and does not infer entrances.

Limits: 20000 repeated candidates per document, 300000 lattice candidates per
zone/cell and 4000000 bounded placement operations, plus existing input/output
limits. Exhaustion returns `E_BUDGET`, never a partial chunk. `building_prisms`
counts and `generation_scratch_bytes` are exposed before generation: road workspace
plus 16 MiB placement workspace and 512 bytes per manual/repeated candidate.
Consumers reserve convex CPU/packed/physics resources separately from triangles,
retain reservations through cancellation/worker retirement and do not raise world
budgets. Complete allocator/GPU calibration and native-platform/real driving
acceptance remain independent final gates.

### Spatial/terrain validation refinement (K04)

The grid origin is exactly `bounds.min`, including negative and non-grid-aligned
origins. Internal ownership intervals are half-open: a point on an internal
east/north edge belongs to the next cell. The inclusive outer maximum belongs to
the last (possibly partial) cell. Closed geometry queries include every touching
cell; this does not duplicate an anchor-owned object. Clipped triangle references
retain the original object ID. Vegetation uses the global signed lattice and
`zone_id:x:y` identity; it is independent of processing order and spawn position.
`terrain` and a declared zone's generated `zone_id:x:y` namespace (canonical signed
decimal indices, without leading zeros or `+`) are reserved from authored IDs.
Ambiguous documents fail with `E_ID`; readers never rename or repair source IDs.

Topology helpers fail closed on unvalidated invalid/oversized topology: cell
lookup returns no owner, enumeration/window is empty and cell bounds return
`E_CELL`. Validation still rejects the document; an empty result is not acceptance.
Cell size is 200..102400 cm, divisible by 200, with at most 16384 cells.

The standard terrain sampling interval is 200 cm (257×257 for a default cell).
Larger intervals must divide the cell size; adjacent explicit grids use the same
interval. PNG samples are unsigned, big-endian 16-bit grayscale. `offset_cm` is
within ±1000000 and `step_cm` is 1..100; decode is exact integer
`offset_cm + sample * step_cm`, with no normalization or inferred vertical datum.
Neighbors may use different offsets/steps if every restored shared-edge sample
matches. Missing grids represent `terrain_base_cm` and must match explicit
neighbors. Compare the complete padding edges even outside partial map bounds.
Generation clips full-grid triangles; it does not rescale a partial grid.
`source_accuracy_cm` remains optional descriptive source uncertainty, independent
of both horizontal sampling and vertical quantization. It is not an accuracy
guarantee. Changing it alone changes v1 world content identity, not geometry.

Surface queries interpolate the absolute rational height and truncate once toward
zero to integer cm. Quantizing a delta relative to one triangle vertex is invalid:
it previously gave neighboring triangles different 1 cm answers on the same edge.
This query correction leaves v1 package bytes and generated-v6 triangles/hashes
unchanged: recipe-v1 vegetation keeps its frozen anchor-relative quantization in
the explicit internal `recipe_v1_spawn` strategy. The corrected public query
applies to every queried surface. Native consumers must use matching
MapKit source/release pins; no saved source, archive geometry or recipe is migrated.

## Resource limits and errors

Current tooling profile: package <=512 MiB, expanded entries including manifest <=1 GiB, each file
<=128 MiB, manifest <=4 MiB, document <=32 MiB, <=8192 payload files, <=16384 cells,
<=200000 document objects. Tooling reads payloads into memory; these are admission
limits, not a proof of runtime memory compliance. Generation has an explicit
triangle budget (CLI/bridge default 500000), and zone candidate budget.

Errors use `{code,message}`; CLI errors go to stderr with exit 1. The complete
[error catalog](ERRORS.md) defines codes and caller actions. No partially validated
package is reported as accepted. Every file-producing command preserves existing
destinations; `unpack` requires a new directory and an existing parent directory.

## Public authoring contract audit (K01)

The checked-in Draft 7 [document](document.schema.json) and
[manifest](manifest.schema.json) schemas are generated from the exact public Rust
types with `mapkit schema document|manifest NEW_OUTPUT.json`. Both reject unknown
object fields, including nested cells. Required collections may be empty; optional
typed fields (for example `clearance_cm`) may be omitted or null. A package reader
still performs all semantic and payload checks after deserialization.

Schema success is **structural validation only**, never package acceptance. JSON
Schema cannot detect duplicate JSON keys after a generic parser has discarded
them, nor require integer lexical notation instead of `1.0`/`1e0`. Graph references,
polygon validity, case-folded paths, byte limits, manifest/document equality,
calendar/chronology, payload decode/seams and digest recomputation are semantic
checks performed by MapKit. Schema `maxLength` counts characters; the 128-byte
map/object ID limits are additionally checked by the core. Generated schemas
describe simple fixed-version/theme/seed/size limits as well as field shapes.

### Producer and source metadata

All six provenance fields are required and must mirror the document in the
manifest. `tool_id`, `version`, `build_id` and `fingerprint` are nonblank Unicode
labels without control characters. Values are preserved exactly; there is no tool
registry, known-fingerprint list, signature verification or authentication. Never
use a matching fingerprint as permission to skip package checks. Unknown tools
and arbitrary nonblank fingerprints, including the independent example, work.

`first_created` and `last_edited` describe supplied creation and edit instants.
K01 replaces the previous unchecked-string behavior with a bounded
[RFC 3339](https://www.rfc-editor.org/rfc/rfc3339#section-5.6) profile: Gregorian
years 0001–9999, `T`/`t`, hour 00–23, minute/second 00–59, optional 1–9 fractional
digits, and `Z`/`z` or a numeric offset through ±23:59. Leap seconds and unknown
offset `-00:00` are unsupported. Calendar validity and last-edit ≥ creation are
checked using integer UTC instants, including fractions and offset date rollover.
Future timestamps are allowed: validation/packing does not read the current clock.
Editors preserve first creation when editing and explicitly supply last edit.
The core neither repairs nor rewrites malformed metadata or original files.

Top-level `attributions` may be empty for wholly original work. Every listed
record and every asset's required `attribution` needs nonblank source and license
labels without control characters. License is a supplied label, not an SPDX-only
allowlist or a claim of verified legal rights. `notice` is required but may be
empty; Unicode and tab/CR/LF text are preserved, other control characters fail.
Source URLs are descriptive text, never fetched. Asset paths remain local-only.
Existing total document/manifest byte limits bound metadata; no clock-derived
defaults or new notice-size cap is introduced.

These metadata checks refine v1 validation; supported geometry and its generated
v6/world hashes do not change. Provenance and top-level attribution edits preserve
world/generated identity. Asset attribution remains content-bound as above.
Previously accepted malformed metadata now fails with `E_PROVENANCE` or
`E_ATTRIBUTION`; saved originals are never automatically migrated.

### CLI contract

| Command | Success (exit 0) |
| --- | --- |
| `inspect PACKAGE` | One JSON object: existing inspection fields plus `manifest`, including inventory, versions, producer metadata and attribution. Full validation precedes output. |
| `validate PACKAGE` | One JSON inspection object after the same full validation. |
| `pack PROJECT NEW_PACKAGE` | Validated package, atomically installed without replacement; one JSON inspection object. Reads referenced files from `PROJECT/document.json`; unrelated project files are not exported. |
| `unpack PACKAGE NEW_DIRECTORY` | Source document and referenced payloads; empty stdout. Manifest is regenerated when repacking. |
| `generate-chunk PACKAGE X Y NEW_JSON` | Canonical generated v6 JSON at the new output; one lowercase SHA-256 line on stdout. X/Y are signed 32-bit integer cell indices; invalid/outside cells fail. |
| `schema document\|manifest NEW_JSON` | Generated Draft 7 Schema file; empty stdout. |

Inspection fields are `package_sha256`, `world_content_hash`, `package_bytes`,
`expanded_bytes`, `base_data_bytes`, `user_asset_bytes`, `cell_count`,
`retained_memory_bytes` and `validation_peak_bytes`. Base/user bytes count expanded
data (base includes manifest), not shares of compressed ZIP bytes. Working-set
values are conservative estimates. `inspect` is currently eager, not a cheap
manifest-only check and not evidence of a 3-second/50-MB target.

The stdlib-only `examples/third_party.py NEW_PACKAGE [DOCUMENT.json]` writes the
same synthetic example without MapKit imports or a binary. Use the CLI to validate
the result. `scripts/check_contract.py` tests Schema generation, all CLI commands,
independent creation, unpack/edit/repack/generation and metadata failure paths.
The independent producer also reads referenced payloads relative to its supplied
document, and fills omitted optional fields with their canonical null values.
It is an authoring example, not a substitute for the reader's complete validation.

## Reproducible authoring and container audit (K02)

Canonical export normalizes the typed document before writing or hashing it:
`nodes`, `roads`, `buildings`, `zones`, `assets`, `placements` sort by ID;
heightmaps sort by `(cell.x, cell.y)`; top-level attributions sort by
`(source, license, notice)`. Ordered geometry arrays (points, polygon vertices,
segment widths/surfaces, exclusions and collision records) keep their input order.
Omitted nullable road/heightmap fields serialize as null. Unicode strings retain
their codepoints (no Unicode normalization). All six provenance strings and
attribution notices retain their supplied text; timestamps are never regenerated.

The file inventory contains each referenced payload path exactly once, even when
several asset descriptors use that file. It includes the canonical document and
excludes the manifest. Hashes are lowercase SHA-256 of the exact expanded bytes.
The writer checks the 32 MiB document, 128 MiB other-entry, 4 MiB manifest, 8192
payload-count and 1 GiB total limits **including the manifest**, then the compressed
512 MiB limit. Limits are inclusive. Reader and exporter apply the same limits;
failure returns no accepted package. User-asset size counts unique asset paths.

Local ZIP filenames, flags and compression methods must agree with their central
records; non-descriptor CRC/sizes must agree as well. These checks run in the
non-inflating admission pass. Streaming data descriptors and ZIP64 size sentinels
remain supported by the ZIP adapter. This is a container consistency check, not
completion of the deeper hostile-container/asset-decoder audit (K03).

Readers accept valid noncanonical JSON spacing/key/object order and legal foreign
ZIP payload ordering, timestamps, permissions, stored/DEFLATE choices and comments.
The manifest must still be first in both physical and central order at offset zero,
all inventory/metadata/hash relations must hold, and every semantic check applies.
`unpack` preserves expanded source/payload bytes. Explicit re-export regenerates
normalized document and manifest bytes and the fixed container profile above.
No source file or existing output is overwritten or silently migrated.

Same normalized source plus identical referenced payload bytes produces identical
ZIP bytes with the locked MapKit exporter. Source path, file modification time,
permissions, process, locale and timezone do not enter the output. Different ZIP
implementations/compressor versions may emit different DEFLATE streams/container
headers while representing the same world: byte identity across unrelated writers
is not required for acceptance. The independent Python producer agrees on canonical
entry bytes/world identity, and each producer is tested for repeated byte equality.
Native OS parity of the locked exporter remains part of final platform acceptance.

Provenance and top-level attribution edits change package identity but preserve
world identity and generated geometry. `map_id`, `revision`, per-asset attribution,
source-accuracy and other document fields remain world-content-bound under v1;
this audit does not broaden the metadata exclusions. A referenced asset or terrain
byte change changes world identity even when a visual asset has the same collision
proxy. Geometry/terrain changes are separately checked against generated output.

`scripts/check_reproducibility.py` runs a small four-cell multi-payload fixture
through real CLI pack/unpack/generate operations, an independent Python producer,
stored/streaming/ZIP64 containers and metadata/geometry/payload edits. Rust tests
retain the existing v1 golden package/world hashes, exact inventory rejection,
local-header inconsistencies and inclusive size/count limits without GiB allocations.
Cross-platform generation, detailed asset defense and full geometry acceptance
remain separate gates in [LIMITATIONS](../LIMITATIONS.md).

## Input defense refinement (K03)

Admission has two gates. `inspect_read_cost` checks the complete container envelope
without inflating payloads or accepting the map. The reader then enforces the
working-set allowance, fully inflates bounded entries, validates JSON/inventory/
hashes/document geometry and decodes every referenced asset and terrain image.
Only the complete result is usable for generation. Container success, a trusted
producer label or valid checksums never skips detailed validation. Failure returns
no partial package; native open clears its previous source, and retry requires a
new successful open. Consumer committed collision belongs to separate owners and
must survive a failed candidate; failure cannot authorize entering its cells.

### Container profile

The non-inflating pass bounds the EOCD count before allocating the ZIP adapter's
index. It checks single-disk EOCD and ZIP64 end/locator/central/local fields,
local-central names/flags/method/CRC/sizes, unique regular portable paths, complete
extra-field TLVs and exact compressed-data ranges. Entries cover the physical data
region once: no gaps, hidden entries, overlap, prefix or unaccounted trailing bytes.
Central records exactly cover their declared region. Standard ZIP comments remain
allowed. Signed or unsigned 32-bit/ZIP64 streaming descriptors must match central
CRC and sizes. ZIP64 sentinels require matching extra values. Unicode path override,
encryption and unsupported flag bits are rejected. Extra-field IDs cannot repeat.
ZIP64 extensible end data is outside this bounded profile (fixed 44-byte end body).

DEFLATE must reach stream end at exactly its declared compressed boundary, emit
exactly its declared expanded size and match CRC. A false small expanded size
fails before appending excess output. The existing count/byte limits, inventory,
world hash and normalized exporter bytes remain unchanged. Earlier header-only
acceptance of ambiguous or incomplete containers is explicitly replaced.
The envelope follows [PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT).

### Static asset profile

Asset paths are `.glb`, `.png` or `.webp`. No URLs, data URIs, scripts, PCKs or
engine scene/material programs are loaded. Attribution URLs remain inert text.
Collision metadata remains the existing bounded `CollisionBox` primitive list;
unknown proxy fields/types are rejected by the document contract. New convex
proxy authoring and renderer support are K07, not introduced by this audit.

- PNG: verify every chunk's CRC, complete IEND with no trailing bytes and complete
  pixel decompression. APNG chunks fail. Ancillary text/ICC is not decompressed or
  interpreted. Images have at most 8192 pixels per dimension and 64 MiB decoded
  pixels. Terrain also requires complete static PNG and its existing 16-bit grid,
  descriptor and seam checks; direct decode rejects invalid divisors/height ranges.
- WebP: exact RIFF/chunk lengths, padding, unique supported chunks and static image,
  feature flags/order and full lossy/lossless pixel decode. Animation is rejected.
  The same dimension/64 MiB output caps apply before pixel allocation.
- Referenced asset paths decode once. Embedded PNG buffer ranges decode once per
  GLB. Cumulative decoded asset-image work is at most 256 MiB per package, inclusive;
  this is checked before each pixel allocation. Terrain keeps its separate existing
  grid limits. PNG/GLB/WebP packages reserve a conservative 256 MiB transient image
  allowance in addition to payload/structured/index costs before inflation.
- GLB v2: exactly aligned JSON and optional single BIN, declared lengths and zero
  BIN padding. JSON permits finite floating-point parameters but rejects duplicate
  keys (also applied before native editor integer normalization). Typed glTF
  validation runs after guarding unsafe POSITION references in gltf-json 1.4.1.
  All views/accessors must stay in the embedded buffer with valid component types,
  alignment/stride/count, finite float values and valid vertex/index relationships.
  Triangle primitives require float VEC3 POSITION; static normal/tangent/color/UV
  attributes follow their typed representations. Indices stay within vertices.
- GLB has at most 8192 records per collection and primitives, one million total
  accessor elements and one million draw elements. Nodes form acyclic forests with
  unique parents and valid scene roots, finite transforms and unit rotations;
  matrix and TRS cannot coexist. Material factors obey declarative glTF ranges.
  Embedded images must be PNG and pass the same complete decoder. These limits
  also apply to unused declared records/resources.

Sparse/matrix accessors, non-triangle modes, morph targets, skins, animation,
cameras, external buffers/images, extensions, extras and script fields are outside
this static subset and fail explicitly. Header-only acceptance of these unsupported
or malformed assets is replaced; saved originals are never automatically edited.
The parser uses [glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html),
locked `gltf` 1.4.1 without import/network/image features, `png` 0.17 and
[`image-webp` 0.2.4](https://docs.rs/image-webp/0.2.4/image_webp/struct.WebPDecoder.html).

Decoded pixels and parsed GLB validation data are temporary; Host does not create
textures, renderer nodes or GPU resources. Working-set allowances and decoder
limits are planning controls, not a proof of complete allocator/RSS or display
memory accounting (some WebP internal allocations are not governed by its memory
limit). S04/K07/platform performance acceptance remains separate. This changes
admission, not generation/recipe/world hashing or physics. Valid existing v1
exports and generated v6 hashes are preserved.

`scripts/check_input_defense.py` independently builds normal, descriptor and ZIP64
containers, corrupts CRC/size/extra fields, and creates an honestly hashed invalid
asset. Rust tests add complete/truncated/corrupt/animated image and GLB graph/byte
regressions, cumulative decode limits, arbitrary truncations/bit mutations and
container/inflation boundaries. Native probes verify failed-source clearing and
retry. These deterministic regressions are not exhaustive fuzzing or native OS
acceptance.

### Read-only overview API (version 1)

`MapOverview` is an optional derived read API, not a package entry or editable
source. Fields are `version`, `map_id`, `bounds`, `roads` (`id`, `kind`, `points`),
`buildings` (`id`, `footprint`, optional `holes`), `attributions` (`source`, `license`) and
`has_custom_assets`. Coordinates retain the document's integer-centimetre axes.
The original package remains authoritative for provenance, complete attribution
notices, asset descriptors and all generation data. Overview output cannot be used
as a replacement MapDocument or change `world_content_hash`.


### Godot packed generated view v1

`generate_chunk_packed(x,y)` returns a native Dictionary response with the same
`generated_sha256`, generated v6 `format_version`, cell and object records as the
JSON API. `chunk.geometry` is an immutable native owner. `geometry.view()` returns
consumer-local packed array wrappers: `vertices_cm` (Int64, nine coordinates per
triangle, original vertex order), `surface_indices` (bytes: asphalt=0, concrete=1,
dirt=2, gravel=3, grass=4), `object_indices` (Int32), `object_ids` (strings) and
`spawnable` (bytes 0/1). Arrays preserve triangle order and integer centimetres.
Different views detach on mutation; retain the owner, never share mutable views
between collision and presentation. `godot/chunk_data.gd` obtains and reads views;
the common renderer also accepts the existing JSON form for editor callers.
This view is an engine adapter, not a new package or generated hash contract.
Generated hashing streams canonical members without retaining a whole JSON tree.
The native owner also prepares `scene_vertices`, `scene_normals`, `ground_uv` and
`wall_uv` arrays in scene-units contract 2. Consumers can submit contiguous mesh
batches without converting every triangle on the main thread. These arrays use
copy-on-write wrappers; the integer geometry remains the source of hashes.
The response includes `generated_counts` (`triangles`, `objects`,
`building_prisms`, `asset_convexes`) measured from the immutable local chunk,
including restored archives. A consumer may retire unused generation allowances
after worker join, while retaining conservative per-element memory costs. These
counts are local adapter metadata, never peer-provided admission authority.
`presentation_bytes` covers one import/template per used asset in a cell and
per-instance scene nodes. The common renderer's `duplicate(0)` instances share
mesh/material/texture resources. Consumers making independent resource copies
must reserve those copies separately; cell leases retain shared resources.

## Optional native occupied-volume view v1

This is an in-memory Godot adapter contract, outside `.memap` and generated v6
serialization/hash. `generate_chunk_occupied_packed(x, y, max_solids)` returns the
normal `{ok,data}` envelope. `data` contains `chunk`, `generated_sha256` and an
immutable `MapKitPackedOccupancy` RefCounted owner. `occupancy.view()` returns:

| Field | Type | Meaning |
| --- | --- | --- |
| `occupancy_version` | int | Exactly 1 |
| `shape_kinds` | PackedByteArray | One tag per solid: 0 box, 1 triangular prism, 2 sloped prism (recipe 3) |
| `shape_values_cm` | PackedInt64Array | Exactly eight integers per solid |
| `object_indices` | PackedInt32Array | One valid index into object_ids per solid |
| `object_ids` | PackedStringArray | Stable source/generated object IDs |
| `slope_tops_cm` | PackedInt64Array | Three top heights per tag-2 record; empty for old shapes |

Box record: `[min_x,min_height,min_y,max_x,max_height,max_y,0,0]`.
Prism record: `[x0,y0,x1,y1,x2,y2,bottom_height,top_height]`.
Sloped record: `[x0,y0,x1,y1,x2,y2,bottom_height,slope_tops_offset]`;
the offset is nonnegative, divisible by three and addresses exactly three valid
heights above bottom. Consumers reject malformed offsets, heights and degenerate
footprints; slope planes use exact integer predicates, not a roof bounding box.
Units are exact local centimeters. Boxes/prisms are full, unclipped volumes;
prism winding does not change occupancy. Concave buildings are unions of prisms.
Unknown versions/tags, nonzero reserved fields, malformed lengths/indices and
invalid extents must be rejected. Consumer views have separate COW array wrappers.
A shape tag describes geometry, not ownership or spawnability.

The generation allowance is 0 through `MAX_OCCUPIED_SOLIDS` (200000); limits fail
without partial output. `estimate_chunk.occupied_solids` bounds optional records
before generation, independent of its triangle cap. Application reservations must
include original geometry, sidecar, temporary packing arrays and retained native
arrays; the count API alone does not enforce memory limits. Tree-owner neighbor
queries, full target-cell assembly and collision readiness are caller obligations.

### Bounded query-cell planning

`MapDocument::query_cells(bounds,max_cells)` and Godot
`query_cells(min_x,min_y,max_x,max_y,max_cells)` return `geometry_cells` and
`occupancy_cells`, each a unique row-major array of `{x,y}`. Bounds are closed
local-centimetre rectangles, including zero-area queries. Exact seams include both
adjacent cells; results are clipped to existing map cells. Entirely outside
queries may have empty geometry results but still require nearby tree owners.
`occupancy_cells` includes geometry cells and the 20 cm horizontal halo required
by recipe-v1 centre-owned tree collision proxies. This halo shares the generator's
proxy size. Buildings and manual proxies already retain full overlapping volumes.

The caller supplies a union cap from 1 through 16384. Invalid query bounds return
`E_QUERY`; invalid topology returns `E_DOCUMENT`/`E_LIMIT`; invalid allowances or
excess cell counts return `E_BUDGET` before result allocation. No partial plans.
Generation hashes and package versions are unchanged. Callers must estimate and
reserve all required cells before generation, then check actual support and
clearance; this broad-phase plan grants no collision readiness or vehicle admission.


### Recipe 4: common static assets and explicit convex proxies

Recipe 4 is an explicit strategy layered on recipe 3. Existing recipe 1–3 source,
canonical generated hashes and `MKCELL01`/`MKCELL02` archives are preserved. Readers
never migrate originals. `Asset` adds optional `convex_collision` (default empty,
omitted) and `material` (default absent, omitted); these fields require recipe 4.
Each asset keeps its required original attribution/license/notice. New code and
original procedural fixtures are MIT; imported asset licenses are independent.

A convex proxy is `{vertices,faces}`. Vertices are 4–32 unique local integer
centimetre `(x,height,y)` points, each component within ±100000. Faces are 4–60
outward-oriented triangles indexing those vertices with integers 0–31. Every
vertex must be used. Duplicate/degenerate faces, open or inconsistently oriented
edges, nonconvexity, zero volume and invalid Euler topology fail `E_ASSET`. Every
face must support all vertices on its interior side. Validation and quarter-turn
placement use exact integer predicates; no engine hull generation defines source
semantics. Up to 32 convexes plus 1024 legacy boxes per recipe-4 asset form a union.
Existing recipe-3 clearance, bounds/building/road/access rules also cover convex
extents. A placement still requires a footprint proxy in this authoring profile.

Generated v6 gains optional `asset_convexes` records `{object_id,shape}`, omitted
when empty and hashed canonically before `building_prisms`. Each overlapping cell
retains the full convex to preserve solid interiors even when no face enters a
cell; clipped collision faces remain ordinary nonspawnable concrete triangles.
Objects retain one anchor owner, so visuals are instantiated once. The geometry
adapter exposes flat `asset_convex_vertices_cm`, `asset_convex_offsets` (including
terminal offset) and `asset_convex_ids`; separate views detach on mutation.
Collision consumers attach the exact convex points alongside existing triangles
and buildings, under their owning cell's readiness/cancellation budget.

Occupied view tag 3 stores `[vertex_offset,vertex_count,face_offset,face_count,0,0,0,0]`
in `shape_values_cm`; offsets address `convex_vertices_cm` (Int64 coordinates) and
`convex_faces` (byte indices). Offsets are nonnegative multiples of three; counts
and extents must satisfy the same convex profile. Consumers validate lengths,
indices, topology and all coordinates before spatial rejection. Full integer SAT
and wheel half-space intersection must use the real convex, never its AABB.
`MKCELL03` adds a bounded convex section after a possibly empty building section:
u32 count, then object-ID string, u32 vertex/face counts, little-endian i64 triples
and u8 index triples. Counts are checked against trusted source estimates and
remaining bytes before allocation. Old magic/bytes remain unchanged.

`AssetMaterial` is `{albedo_rgba:[u8;4], metallic_per_mille:u16,
roughness_per_mille:u16,double_sided:bool,albedo_texture?:asset_id}`. Per-mille factors
are 0–1000. Texture IDs resolve only to declared PNG/WebP assets in the same
package; no path/URL or shader is evaluated. GLB uses its embedded static glTF
materials unless this override is present. PNG/WebP placements texture their
explicit proxy faces (dominant-plane scene-metre UVs; vertical faces include height). GLB coordinates use metres,
right-handed x-right/y-up/z-back, corresponding to `(map_x,height,-map_y)/100`.
The common renderer applies placement quarter turns in actual metres (scale 1.0).
Builtin tree canopy/trunk, fence panel and streetlight post/head use original
procedural shapes and fixed shared material colors.

`with_presentation(generated_data)` explicitly decorates CPU output with validated
in-memory bytes, material descriptors and proxy-display masks. It does not change
generation/hash or create GPU resources. Both fresh generation and restored caches
can be decorated. Callers first reserve `estimate_chunk.presentation_bytes`, which
includes importer/template/instance/texture planning allowances for local or
proxy-overlapping assets and their texture dependencies. Host never requests this
view or loads the renderer, textures or meshes. The core knows no engine or files.

The shared Godot renderer imports validated GLB buffers without filesystem
extraction or resource-path loading. It accepts only plain Node3D/MeshInstance3D
results with no scripts, imports image textures in memory, and batches geometry
and object attachment. Imported templates are released at completion/cancellation;
instances retain only their shared mesh/material resources. Missing/backend-rejected
assets produce `job.error` (`E_RENDER_ASSET`) and never `job.done`; consumers must
reject that generation without acknowledging readiness. Repeated cancel and parent
removal cannot attach late results. A single engine GLB import cannot be preempted;
its latency and complete driver/RSS accounting remain reference-platform gates,
not a promise derived from the batch count or estimate.

The standalone `examples/assets` project and `--probe assets` exercise materials,
PNG/WebP, actual transforms/solid interiors/seams, mutation isolation and cleanup.
Editor gestures, full LOD controls and native OS/performance acceptance remain
separate from this supported static rendering contract.


## Recipe 5: building courtyards

Explicit recipe 5 adds optional `Building.holes: [ring, ...]` (default empty,
omitted when empty). Each record still represents one building with one outer
`footprint`, material, use, absolute base and height. Holes are open through the
whole solid, floor and flat roof; inner boundaries are solid walls. Boundaries
belong to the building for point/clearance queries. No roof or building surface
is spawnable. Ground and separately authored surfaces remain usable in the void.

The bounded profile accepts simple concave outers/holes, up to 16 strictly
interior, pairwise disjoint holes and 512 vertices total per courtyard building.
Touching/crossing rings, nested holes, invalid bounds and non-flat roofs reject.
Independent island buildings inside a hole are separate source objects, retaining
their own identity. Existing conservative XY road/building height-clearance rules
remain; an entire road corridor, manual proxy or vegetation footprint may occupy
a courtyard only when it clears its walls. Source access-corridor exclusions still
apply. General intersecting building shells and structural vertical semantics are
not inferred.

Triangulation uses i128 integer predicates, canonical winding/start/hole order,
visible original-vertex bridges selected by squared length and lexicographic tie
break, and ear clipping with a positive exact area check. Redundant collinear
vertices are removed only on the new hole-bearing path. No floating point,
new vertex rounding, geometry repair or per-building decomposition is introduced.
A shared 4,000,000-step document courtyard validation budget rejects excessive
work; generation uses the same bounded algorithm. Existing 16 MiB placement
scratch covers bounded ring/triangle vectors. Output estimates include inner
vertices and bridge pairs (`n + 2h - 2` maximum triangles before cell clipping).

Existing v6 triangular prisms, occupied sidecars, packed views, archive bytes and
shared renderer represent the material between outer and inner rings. Inner walls
are prism faces, matching physics. Cell clipping follows the existing integer
clip contract. Overview v1 adds omitted-when-empty `holes`, counts their points
and ring records in allocation estimates, and consumers must draw inner outlines.

Readers reject nonempty holes under recipes 1–4 with `E_VERSION`; older readers
reject recipe 5. `.memap` remains version 1 and generated/archive layouts remain
v6. Empty holes serialize identically to existing documents; recipes 1–4 retain
frozen generation vectors. World content identity includes authored holes and
recipe; old packages/files are never silently rewritten or upgraded. The public
synthetic `examples/courtyard` and core `courtyard`/package contract tests cover
this extension. Native OS parity and representative performance remain separate.
