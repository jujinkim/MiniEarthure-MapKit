# MEMAP v1 container contract

All own format, recipe, generated and scene-unit versions are 1. Only the current
schema is supported. [CURRENT_V1](CURRENT_V1.md) owns the map domain; generated
JSON Schemas in this directory and Rust semantic validation define field bounds.
There are no historical readers, automatic converters or recipe selectors.

## ZIP and JSON

A `.memap` is an unencrypted ZIP with `manifest.json` as its first physical and
central-directory entry, no prefix and only regular stored/DEFLATE entries.
Exports use DEFLATE level 9, 1980-01-01 00:00:00, Unix 0644 and sorted payload
paths. The manifest is excluded from its own inventory. `document.json` is the
editable map source; extracted projects use the same relative payload paths.

Canonical JSON is UTF-8 without BOM, whitespace or a final newline; keys use
Rust BTreeMap order and geometry uses bounded integer centimetres. Seeds are
nonnegative integers at most 2^53-1. Unknown fields and duplicate keys fail.
Portable paths are case-sensitive ASCII, at most 240 bytes and unique after
ASCII case folding. Absolute paths, backslashes, parent components, device names,
trailing dots/spaces, symlinks and special files fail. Components allow letters,
digits, spaces, hyphens, underscores and dots.

The manifest mirrors document identity, revision, bounds, cell size, seed, theme,
assets, attributions and supplied provenance. Its inventory binds every payload's
size and SHA-256; missing or extra files fail. Producer fingerprints confer no
trust. Export does not invent timestamps or read environment metadata.

`package_sha256` hashes exact ZIP bytes for transport/cache identity and is
reported outside the ZIP. `world_content_hash` hashes canonical gameplay source
and content payload hashes, excluding provenance, top-level attributions,
document.json and courses/completion records. The course's own geometry identity
is separate. Changing ZIP compression alone need not change driving identity.
Current source/dependency fingerprints invalidate disposable generated caches.

## Coordinates and ownership

Source vectors are `[x_cm,height_cm,y_cm]`; Godot uses `(x,height,-y)*0.01` at
actual metre scale 1.0. Objects are global, geometry is clipped to cells and
instances belong to their anchor cell. The maximum map edge belongs to the last
cell. Explicit graph node identity defines road connectivity; coincident points
and crossing roads do not. Surface queries distinguish stacked roads and exclude
non-spawnable roofs, walls, supports, water and venue floors.

Heightmaps are unsigned grayscale PNG16 with `(cell_size/spacing)+1` samples,
row increasing local y and column increasing local x. Height is
`offset_cm + sample*step_cm`; minimum spacing is 200 cm. Shared edges must agree,
including implicit flat terrain. Partial edge cells retain full source grids.
Independent authored road decks and terrain fitting follow CURRENT_V1.

## Limits and publication

The package limit is 512 MiB, expanded inventory 1 GiB, entry 128 MiB,
manifest 4 MiB, document 32 MiB and inventory 8,192 files. Decoder/workspace and
caller memory reservations apply before inflation/allocation, independently of
these disk limits. Static assets use the bounded GLB/PNG/WebP profiles; code,
external URLs and executable dependencies are rejected. See [errors](ERRORS.md),
[assets](WORLD_ASSETS.md) and [memory](../RENDER_MEMORY.md).

Read validates hashes, references, saved geometry and limits without recompiling
the stored package. Authoring/export verifies current source equality. Output
uses owned temporary files and atomic publication to a new destination; originals
are never silently replaced. Cancellation cannot publish partial packages.
`read_with_budget` bounds file-backed input; `unpack_source` requires a new directory.

CLI `pack`, `inspect`, `validate`, `unpack` and `generate-chunk` are available from
the independent public build. Contract, corruption, path, cancellation and
determinism tests use synthetic fixtures. Desktop checks do not establish device
parity or detailed gameplay acceptance.
