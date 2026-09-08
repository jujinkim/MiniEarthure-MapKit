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
`[x_cm,height_cm,y_cm]`; Godot vector is `(x,height,-y)*0.00125` (1:8 scale).
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
`buildings` (`id`, `footprint`), `attributions` (`source`, `license`) and
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

## Optional native occupied-volume view v1

This is an in-memory Godot adapter contract, outside `.memap` and generated v6
serialization/hash. `generate_chunk_occupied_packed(x, y, max_solids)` returns the
normal `{ok,data}` envelope. `data` contains `chunk`, `generated_sha256` and an
immutable `MapKitPackedOccupancy` RefCounted owner. `occupancy.view()` returns:

| Field | Type | Meaning |
| --- | --- | --- |
| `occupancy_version` | int | Exactly 1 |
| `shape_kinds` | PackedByteArray | One tag per solid: 0 box, 1 triangular prism |
| `shape_values_cm` | PackedInt64Array | Exactly eight integers per solid |
| `object_indices` | PackedInt32Array | One valid index into object_ids per solid |
| `object_ids` | PackedStringArray | Stable source/generated object IDs |

Box record: `[min_x,min_height,min_y,max_x,max_height,max_y,0,0]`.
Prism record: `[x0,y0,x1,y1,x2,y2,bottom_height,top_height]`.
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
