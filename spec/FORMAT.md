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

Current tooling profile: package <=512 MiB, expanded payload <=1 GiB, each file
<=128 MiB, manifest <=4 MiB, document <=32 MiB, <=8192 payload files, <=16384 cells,
<=200000 document objects. Tooling reads payloads into memory; these are admission
limits, not a proof of runtime memory compliance. Generation has an explicit
triangle budget (CLI/bridge default 500000), and zone candidate budget.

Errors use `{code,message}`; CLI errors go to stderr with exit 1. Codes:
E_JSON, E_DOCUMENT, E_VERSION, E_ID, E_GEOMETRY, E_PATH, E_REFERENCE, E_HASH,
E_ZIP, E_MANIFEST, E_HEIGHTMAP, E_SEAM, E_ASSET, E_LIMIT, E_BUDGET, E_CELL,
E_SPAWN, E_RELEASE, E_IO, E_STATE and E_USAGE. No partially validated package is
reported as accepted. `pack` and `unpack` never overwrite existing destinations.

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
