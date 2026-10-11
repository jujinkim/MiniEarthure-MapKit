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
| `E_ROAD_DESIGN` | Invalid road design alignment: 2..1025 anchors, empty alignment or stored samples differ from controls. |
| `E_ROAD_SURFACE` | Invalid road/sample surface input, or a terrain-following road without an actual collision probe. |
| `E_ROAD_INTERSECTION` | Crossing roads need an explicit junction or a separated bridge/tunnel. |
| `E_ROAD_GRADE` | Road design grade exceeds the allowed maximum. |
| `E_ROAD_AUDIT` | Road has no authored profile or its design/collision ribbon has a gap. |
| `E_ROAD_ENVIRONMENT` | Preserved terrain penetrates a road driving surface. |
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
| `E_TRACK_SOURCE` | Malformed or invalid manual track source, piece preset, width or panel alignment. |
| `E_TRACK_DRAFT` | Track still has unresolved authoring issues; it cannot be compiled, packed or driven. |
| `E_TRACK_GEOMETRY` | Composite track geometry is invalid (overlap, gap or unsupported join). |
| `E_TRACK_SURFACE` | Generated road surface cannot meet the 1 cm diagonal tolerance within its lateral budget. |
| `E_TRACK_BUDGET` | Track curve or compiled output exceeds its sample/resource budget. |
| `E_TRACK_BOUNDS` | A track piece or driving surface leaves the map. |
| `E_TRACK_OWNERSHIP` | An independent object occupies the assembled track's ID or geometry namespace. |
| `E_TRACK_ENVIRONMENT` | Terrain or an environment solid penetrates or crowds the track's driving space. |
| `E_TRACK_SUPPORT` | No clear terrain support site or support that keeps the driving/flight space and 110 cm passage. |
| `E_TRACK_CHECKPOINT_LIMIT` | Final routes need more than 64 shared checkpoints. |
| `E_TRACK_DURATION` | No connected seed layout within ±10% of the requested duration in 24 candidates. |
| `E_TRACK_PANEL_SUPPORT` | Panel has no supporting production road triangles or no representable convex surface. |
| `E_TRACK_PANEL_BUDGET` | Panel exceeds the 32-part limit. |
| `E_SURFACE_REFERENCE` | Surface attachment target track, editable source or instance is missing. |
| `E_SURFACE_CONNECTION` | Invalid ordinary-road port connection or missing connected road/instance. |
| `E_SURFACE_ATTACHMENT` | Invalid attachment settings, unsupported target surface or insufficient obstacle clearance. |
| `E_SURFACE_OWNERSHIP` | Attachment product or interaction line ID collides or is missing. |
| `E_POI` | Facility name/category is not bounded nonempty text, or its coordinate is outside the map. |
| `E_POI_LIMIT` | Too many points of interest. |
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
| `E_BRUSH` | Invalid terrain brush options or timed input. |
| `E_HEIGHT` | Height sample outside the working grid or allowed range. |
| `E_RESOURCE` | Native working-document resource has no bytes. |
| `E_STALE` | Working height history changed since the request; retry from current state. |
| `E_CONFLICT` | Source file changed on disk since it was read; reload before saving. |
