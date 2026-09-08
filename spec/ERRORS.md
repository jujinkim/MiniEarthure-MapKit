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
| `E_ID` | Empty, oversized or duplicate object ID. |
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
