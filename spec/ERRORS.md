# Public error catalog

CLI failure: exit 1, one JSON `{code,message}` object on stderr, no success output
on stdout. Match `code`, not message wording. If input violates several rules,
the first observed failure wins; error ordering is not a public guarantee. Native
Godot methods put the same object in `{ok:false,error:{code,message}}`.

| Code | Meaning / corrective action |
| --- | --- |
| `E_JSON` | Malformed JSON, duplicate keys, unknown/missing fields or wrong typed representation; fix source syntax/shape. |
| `E_DOCUMENT` | Unsupported document identity/theme/seed or topology; inspect document constraints. |
| `E_VERSION` | Unsupported package, recipe or generated contract; use a supported producer/version. |
| `E_PROVENANCE` | Missing/blank/control-character producer label, invalid supplied timestamp or reversed edit chronology. |
| `E_ATTRIBUTION` | Invalid source/license label or notice text, including per-asset attribution. |
| `E_ID` | Empty, oversized, duplicate or reserved generated identity (`terrain` / a declared zone's `zone_id:x:y`). |
| `E_GEOMETRY` | Invalid coordinates, graph/segment relationship, polygon, dimension or placement. |
| `E_PATH` | Unsafe/reserved/duplicate/case-colliding path or disallowed file type/link. |
| `E_REFERENCE` | Missing/extra payload, duplicate inventory record or unmatched document reference. |
| `E_HASH` | Payload size/digest or world content digest mismatch. |
| `E_ZIP` | Invalid/ambiguous ZIP envelope, disagreeing headers/descriptors/ZIP64, gaps/overlaps/trailing bytes, unsupported compression/encryption or incomplete/extra DEFLATE/CRC failure. |
| `E_MANIFEST` | Wrong manifest location, document path or disagreement with source metadata. |
| `E_HEIGHTMAP` | Invalid PNG terrain encoding, dimensions or height samples. |
| `E_SEAM` | Restored terrain/road boundary contract mismatch. |
| `E_ASSET` | Invalid/unsupported static asset, image decode, GLB graph/accessor/material or collision proxy. |
| `E_LIMIT` | Hard package/document/file/count/expanded-data, image dimensions/decoded-work or GLB record/element profile exceeded. |
| `E_BUDGET` | Requested generation, query, overview or occupied-solid allowance exhausted/invalid. |
| `E_MEMORY_BUDGET` | Package validation or overview working-set allowance exceeded before use. |
| `E_CELL` | Requested cell or point is outside the available map. |
| `E_QUERY` | Invalid bounded spatial query. |
| `E_ARCHIVE` | Disposable generated-cell archive identity, encoding or digest invalid; regenerate from source. |
| `E_SURFACE_LIMIT` | Selection exceeds 64 intersecting surfaces or 256 UTF-8 bytes per intersecting identity; the entire selection is refused before output serialization. Choose another position. Package geometry remains valid. |
| `E_SPAWN` | No valid requested spawn surface or invalid spawn input. |
| `E_RELEASE` | Exact release identity mismatch. |
| `E_IO` | Filesystem failure, including an existing output destination; preserve source and select a new output. |
| `E_STATE` | Native adapter called without the required open package/state. |
| `E_USAGE` | Unknown CLI command, wrong arguments or invalid cell integer syntax. |

`scripts/check_contract.py` compares this catalog with all four public crates,
executes failure cases through the CLI, and checks that invalid input does not
create accepted outputs. `scripts/check_input_defense.py` and the Rust
`input_defense` tests exercise the bounded K03 asset/container profile.
Platform/renderer/allocator acceptance remains in [LIMITATIONS](../LIMITATIONS.md).

Current v1 also uses `E_GEOMETRY` for overlapping structural junction mouths,
mismatched joined tunnel clearances or a ground/structure apron that disagrees
with restored terrain. Author explicit separated, terrain-level approaches;
readers never repair original source. `E_LIMIT` bounds endpoint degree (32).
`E_BUDGET` includes local road planning, live fragment/vertex, subdivision work and
source-derived output allowances. `E_MANIFEST` includes document/manifest recipe
mismatch; the current recipe retains source and generated identity.


Common renderer jobs use `E_RENDER_ASSET` for missing validated presentation bytes,
failed image/GLB backend decode, unknown builtins or non-static engine import
results. This is a presentation-job error, not a CLI package-acceptance result.
Consumers must reject readiness and retain any already committed driving region.

Current v1 also reports `E_INDEX` for malformed regional directories or identities,
`E_CANCELLED` when an owning request is retired, and `E_ENVIRONMENT` for invalid map environment profiles.

Panel authoring errors: `E_TRACK_PANEL_SUPPORT` means no supported production road
triangles (or an unrepresentable convex surface). Invalid width/alignment is
`E_TRACK_SOURCE`. No partial panel replaces the current document on failure.

Additional current v1 authoring/runtime input diagnostics:

| Code | Meaning / corrective action |
| --- | --- |
| `E_ROAD` | Invalid road profile, connection or surface input. |
| `E_PLACEMENT` | Invalid placement transform or asset use. |
| `E_WATER` | Invalid water geometry/profile. |
| `E_GIMMICK` | Invalid driving structure/effect declaration. |
| `E_GRIND_SOURCE` | Invalid authored grind geometry. |
| `E_GRIND_CONNECTION` | Disconnected grind endpoint or transition. |
| `E_GRIND_BUDGET` | Authored grind exceeds its bounded resources. |
| `E_PIPE_DIMENSIONS` | Unsupported pipe dimension/clearance. |
| `E_TRACK_SETTINGS` | Invalid procedural track settings. |
| `E_TRACK_REQUIRED` | Operation requires an authored track. |
| `E_TRACK_ASSEMBLY` | Invalid track assembly or connectivity. |
| `E_TRACK_MODIFIED` | Stored/generated track no longer matches its authoring source. |
| `E_TRACK_COURSE` | Track course cannot be built from the selected route. |
| `E_CHECKPOINT` | Invalid or unsupported authored checkpoint. |
| `E_RACE_SIZE` | Unsupported race participant count. |
| `E_COURSE` | Invalid course definition or requested operation. |
| `E_COURSE_JSON` | Malformed course JSON. |
| `E_COURSE_VERSION` | Unsupported course format. |
| `E_COURSE_LIMIT` | Course count/size limit exceeded. |
| `E_COURSE_HASH` | Course content digest mismatch. |
| `E_COURSE_MAP` | Course map identity mismatch. |
| `E_COURSE_WORLD` | Course world content identity mismatch. |
| `E_COURSE_BOUNDS` | Course checkpoint is outside map bounds. |
| `E_COURSE_VALIDATION` | Invalid course completion reference or evidence. |
| `E_PREVIEW` | Invalid menu preview input or identity. |
| `E_THREAD` | Native background task could not be created or joined. |
