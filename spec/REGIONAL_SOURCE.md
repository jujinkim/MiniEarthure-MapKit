# Current v1 regional source storage

`.mkregions` uses magic `MKREGN01`, a bounded front index and independently
compressed/hash-bound source records. All own versions are 1; old readers,
conversion and alternate version paths do not exist. Storage regions group
execution cells without changing world coordinates, cell geometry or source IDs.
Source/assets and the canonical original document are preserved.

## Implemented envelope and verification levels

The 48-byte header is eight-byte magic, little-endian u64 index length and 32
raw SHA-256 bytes of canonical index JSON. The index is at most 4MiB. Record
offsets are relative to the payload start; each is a complete raw-DEFLATE stream.
Spans are contiguous and ordered, without gaps, overlap, prefix or tail. Each
record binds exact compressed and expanded lengths and SHA-256 of expanded bytes.
The file's captured length is rechecked before each record. Reads use the original
open handle and independently verify content; a changed path cannot redirect it.

Index fields are defined by `indexed::Index` with unknown fields and duplicate
JSON keys rejected. `world` is a metadata-only MapDocument, `side_cells` is 1–128,
`regions` are row-major exact non-overlapping coverage of the global execution
grid, and each `cells.min/end` is an inclusive/exclusive cell range. There are at
most 8192 storage regions, 8191 shared payloads and 16384 total records. Sources
remain at most 32MiB each; other records at most 128MiB. All source duplication is
included in the existing 1GiB expanded and 512MiB compressed ceilings.

The original canonical authoring document is a separate record. All original
declared assets/terrain payloads, including unused assets, are stored once and
preserved by `unpack-regions`; region snapshots reference only their dependencies.
No generated geometry is bundled. Original world content identity is retained;
the index digest and region identity distinguish the new storage/source snapshot.
The embedded regional manifest is labeled `mkregions-source`, never `memap`.
Its `package_sha256/package_bytes` describe region identity/read bytes, not a ZIP
transport package. Existing generated archives additionally retain their ordinary
world/recipe/generated/cell binding; an application's storage cache must bind the
region/index identity when caching regional source.

1. `open` verifies the index, topology, spans and captured file length only.
2. `load_region` verifies that source and all referenced payloads, complete source
   geometry/IDs/rules/assets, and terrain edges touching the execution region.
   It does not read or certify unrelated source or prove the global dependency
   derivation. A changed but individually valid regional document may require the
   complete audit to detect its disagreement with the authored whole.
3. `audit` reads the bounded original source and all assets, checks world identity
   and every regional source against `region_source(original, cells)`. **An
   untrusted artifact needs this complete audit before consumer installation or
   cross-region game admission.** Persist that result by the index digest and
   reverify every region record on access; the digest is not publisher identity.
   `unpack_source` performs this audit and only creates a new destination.

The reader has no implicit cache. Every returned `RegionSnapshot` is independent;
its prepared generator rejects cells outside `cells`. Metadata-only world queries
are constant-space. Ordinary deserialization cannot enable source topology: the
in-memory capability comes from explicit `into_indexed_source`/`new_region`, is
not serialized, and never bypasses ordinary `validate` or `cells` bounds.


## Dependency closure and limits

Region source derives the complete local dependency closure, keeping full context
only where implicit widths or repetition rules need it. Terrain neighbor edges,
connected road arms, vegetation competitors and material/texture dependencies
remain available. Decoder peak declarations are checked against payload bytes.
Opening does not enumerate/inflate unrelated regions; source snapshots are
immutable, separately owned and budgeted through cancellation/worker retirement.

Authoring remains one document at most 32 MiB/200,000 input objects, package
512 MiB/expanded 1 GiB. There are at most 8,192 regions and 16,384 records;
`side_cells` is 1–128. Large extent does not imply arbitrary density or relaxed
work limits. Sharded authoring, source LRU and network region transfer are separate
consumer work. Game/Editor adapters exist but detailed platform acceptance is
not implied by format tests.

Scoped tests cover full dependency audit, canonical geometry identity, bounded
reads, changed files, corrupt references/offsets/hashes, cancellation, preserved
source/payloads and new-destination restoration. Native device parity and large
area/density performance remain separate verification.
