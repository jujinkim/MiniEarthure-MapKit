# L01 regional source storage — decision, 2026-09-11

MapKit owns this opt-in, independently indexed source format. It is a separate
`.mkregions` artifact, magic `MKREGN01`; it is not a replacement interpretation of
`.memap` v1. Existing packages, recipes 1–6, generated-v6 bytes, recovery originals
and ENet contracts remain intact. No format is upgraded on read.

## Decision before implementation

Use a bounded front index and independently compressed, SHA-256-bound records.
Storage regions group an explicit integer number of execution cells. Eight or
sixteen 16m cells give the 128m/256m comparison candidates; neither is a mandatory
world grid or a performance guarantee. Coordinates, cell origin, object IDs and
procedural seeds remain world-global. The source grid may cover more than 16,384
cells; a prepared execution region remains bounded. Legacy document validation
and its 16,384-cell limit are retained. The 32MiB authoring document, 200,000 input
objects, 512MiB artifact, 1GiB expanded data and execution budgets are not raised.

The first producer converts a bounded authored source into independently
validated source snapshots. It retains dependencies conservatively; spatial
partitioning must not cut connected road arms, implicit sidewalk dependencies,
vegetation competitors, repetitions, collision proxies or terrain edge samples.
Shared payloads are stored once, with per-region references. The authoring/export
step may read the whole source. Opening the artifact reads only its index;
loading one region reads that region's source and referenced payloads. No reader
implicitly enumerates the world or inflates unrelated regions.

Independent region verification means index/envelope verification followed by
complete validation of each requested region and its referenced assets. It does
not mean that unread bytes have been validated. An explicit whole-artifact audit
is separate. Hashes identify content, not a trusted publisher. A failed,
cancelled or stale candidate must never replace a caller's live source/collision.
Readers return immutable, separately owned snapshots and retain no implicit
region cache. Callers budget all simultaneously held snapshots and worker
lifetimes; cancellation is checked between bounded I/O/decode operations and
before returning. A running decoder/generator is joined before its reservation
can retire. Existing renderer retirement is unchanged.

The index binds topology, region source hashes, exact offsets/lengths, payload
references and total length. Region/cache identity includes the index digest,
region coordinate, source digest and generator contract. Overlap, gaps, duplicate
records, trailing bytes, unknown fields, oversized allocations, decompression
tails and hash/length mismatches reject. Asset paths remain inert portable names.
Existing asset and heightmap validators are reused.

## Alternatives and integration boundary

- Increasing cell size conflates storage with physics/render work and can worsen
  dense-cell cost. Increasing memory/document/cell constants is not selected.
- One compressed document cannot supply bounded partial reads. An ordinary ZIP
  central directory alone does not provide spatial source dependencies.
- Independent bounded records permit seeks and eventual range transport without
  requiring network requests per region now. Per-region asset duplication is
  avoided on disk; independently held snapshots still count their own memory.
- A generated-only cache loses editable source and ties storage to generated
  versions. Generated archives remain disposable, separately keyed accelerators.

Whole-map transport versus region requests requires consumer implementation and
measurement. This format does not silently extend the protocol-8 game offer or
make the existing Client/Host/Editor accept it. L01 delivery must distinguish the
MapKit source/adapter implementation from consumer adoption; L02 representative
area/density support and final platform acceptance are not inferred from it.

This decision is authorized by the current L01 implementation request; it is not
a claim of completed functional cutover.

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
not serialized, and never bypasses legacy `validate` or `cells` bounds.

## Version-1 dependency closure and shared authoring limits

Global road records and explicit graph nodes are retained so junction arms and
width influence remain complete. Zones and repetitions remain global. With
explicit ground-road sidewalk widths and no repetitions, buildings/entrances
and manual proxies are spatially culled against the region plus maximum vegetation
competitor reach, canopy and clearance. All other recipes/cases retain complete
building/manual context. Surface polygons are filtered by their bounds. Heightmap
descriptors retain the immediate neighbor ring; validation checks restored shared
edges that touch the execution region, including implicit-flat neighbors.
Assets retain material-texture closure; display bytes are validated with existing
asset rules and are never treated as executable paths.

The first exporter still accepts one bounded authoring document. Sharded authoring
above 32MiB/200,000 objects and partitioning long global road/rule dependencies
are **not implemented**. Large area does not imply arbitrary density. These limits
may cause an explicit budget/format rejection; the writer never drops geometry,
raises budgets or silently freezes derived sidewalk widths to make a map fit.

`region_cost` includes the live index, source and payload vectors, structured
parsing, the bounded estimate cache and sequential decoder workspace. It uses a
conservative 256MiB decoder allowance whenever payloads are present; unlike the
legacy reader it does not inspect asset prefixes to refine that allowance.
Consequently a smaller retained source can have a higher validation reservation.
Independent concurrently held snapshots each require their own reservation.
Reported bytes are logical planning allowances, not measured RSS or GPU usage.
Generation/output/physics/presentation reservations are additional.

Cancellation is observed every 64KiB input block, every 8KiB inflate step, between
validation stages, and before returning. A native asset decoder or geometry
validator is not preempted mid-call. `ReadEpoch`/`ReadTicket` and the Godot request
generation reject cancelled and late candidates; applications must check again
at serialized commit and join before releasing the worker's peak reservation.
Committed snapshots do not become unusable when another request starts.

## Validation and handoff

Standalone Rust tests include legacy frozen vectors, 10km/16m topology with 6241
128m storage regions, exact read spans, pre-read budgets, malformed envelopes,
duplicate JSON, decompression tails, mid-read cancellation, late results,
heightmap corruption, shared payloads, recovery/no-overwrite, and source/geometry/
occupied-solid equivalence across public fixtures. The native `regional` probe
loads on a worker, joins/cancels/retries, verifies packed/cache identity and actual
bridge collision support while retaining the current collider.

For the unchanged public v6 driving-school package, `regional_compare` checked all
432 cells and occupied solids for both storage sizes. The original package is
262,980B; 128m/256m indexed artifacts are 534,527B/370,219B with 10/3 regions.
All 33 original payload files (1,237,273B expanded) occur once. Source retained
allowances are 15,561,226–43,850,274B / 40,632,656–58,422,412B, versus the old
70,943,750B. Highest source validation allowances are 419,920,590B/475,877,998B,
versus the old 332,374,070B. This is a functional storage comparison, not gameplay
or density/performance acceptance. Neither grouping becomes a universal default.

The L01-A storage/CLI/native evidence above describes the earlier standalone
unit. L01-B adds `audit_summary` and native complete-audit admission, immutable
world queries outside the reader I/O lock, source-topology overview, and explicit
project export/recovery bindings. Applications own the adoption and its tests.

## L01-B consumer interface — 2026-09-11

`audit_summary` returns the SHA-256 of the complete original open handle, index
identity, world identity, complete expanded record bytes (including source
copies), shared user-asset compressed/expanded bytes, a maximum 4MiB overview
and its allocation cost. The original document/payloads are released at return.
The summary has an additional 16MiB native allowance. The native reader also
reserves a second index allowance for immutable query metadata; `cell_window`,
`query_cells` and `map_bounds` never wait on an active decoder's lock. Native
`audit` reports both audit peak and the largest region validation bound.

The private Runtime consumer chooses whole-file protocol9 transport with explicit
`memap`/`mkregions` support negotiation. File/index/world hashes have separate
roles. A cache installation runs a complete audit and verifies the offered
identity; region records are verified again on every source read. There is no
region-request network protocol. Runtime uses one scoped source snapshot at a
time on its existing serial worker. A source is dropped before reading another
and on job return; generated/presentation data are independent copies. Runtime
reserves the conservative largest source validation allowance together with
generation and existing outputs, keeps live work charged through cancellation
and join, and binds disk archive namespaces to the regional identity. The
largest-region allowance is conservative even for lighter regions; safe denial
is possible where more precise planning could fit. No budget is increased.

Editor selects `.mkregions` explicitly at export, with 1–128 execution cells per
storage side; its initial UI value of 8 is an editing convenience, not a universal
performance profile. Reopen restores into a new adjacent `.source` directory and
uses ordinary dirty-document protection before adopting it. Existing files and
unused original payloads remain intact. The legacy bounded authoring document
limit still applies; source-topology overview does not enable unbounded editing.
Functional consumer evidence and remaining platform/performance gates belong to
the owning application reports. This interface does not declare final cutover.

## L01-C decision before implementation — 2026-09-12

Write index version **2** inside the existing `MKREGN01` envelope. Read version 1
with its frozen dependency derivation; never reinterpret or rewrite old artifacts.
Version 2 uses a separately versioned local closure and per-payload decoder
allowances. Older readers reject version 2. The private consumer must explicitly
advance its package compatibility contract before offering these files.
Recipe/generated bytes, world identity, authoring and execution limits stay fixed.

For recipe 6 with explicit ground-road sidewalk widths and no repetitions, retain
whole roads whose segment bounds touch the execution rectangle expanded by the
maximum vegetation competition reach, canopy/clearance, global road width and
sidewalk reach. Also retain every arm at the selected roads' authored endpoints,
their referenced graph nodes, and one widest road to preserve the generator's
global influence bound. Do not clip polylines, renumber segments, freeze inferred
widths or merge geometric crossings. Zones keep their original polygons, seeds and
IDs when their bounds intersect the competition halo. Building/manual/surface and
terrain-neighbor closures retain their existing safety margins. Other recipes,
implicit widths and repetitions use the frozen conservative closure. Construct
the local snapshot from selected records without cloning the entire world first.

Version 2 index `decoder_peaks` covers every shared payload path. Values come from
the existing bounded PNG/GLB prefix estimator (unknown/WebP/embedded images retain
256MiB); these are untrusted planning declarations, never validation receipts.
After each record's complete length/hash verification, recompute its allowance
from those exact bytes and reject a mismatch **before any asset/pixel decoder**.
Index-only open still reads no payload. All full asset validation and malformed
prefix defenses remain. Version 1 retains its conservative decoder allowance.

Audit reservations use the maximum of sequential original-validation and closure
comparison phases, retaining the original document/files throughout both. The
closure phase still budgets original cloning for version-1/fallback derivation,
one regional canonical comparison, compressed input and scratch. No full source
survives audit. All original and unused payloads, world identity, every regional
derivation and inventory are audited before admission. Cancellation/read tickets,
file/index identity, source ownership, worker join and current collision are unchanged.

Required evidence: frozen version-1 admission; complete generated/occupied-solid
parity on public fixtures and targeted seam/junction/competition/fallback cases;
forged decoder allowances, correctly rehashed wrong closures, corruption,
pre-read budget denial and cancellation; fixed L02 source/package/native/consumer
cost comparisons. Platform, long-driving and larger-area acceptance stay separate.

### Implemented and scoped validation

Standalone Rust: 124 tests pass, including 13 indexed-source tests. The frozen
5,690B MIT roads v1 fixture audits and generates identically. The local closure
test compares every one of 400 cells, complete geometry and occupied solids,
including endpoint stars, a distant widest road, competing zones, implicit-width
and repetition fallbacks. Public fixtures retain terrain seams, corruption,
pre-read denial, cancellation, source independence and recovery/no-overwrite.
Hash-correct source omissions still fail full audit; forged decoder declarations
fail before decoding. Debug/release native and CLI build; pure-core ownership passes.

Fixed public L02 mixed/dense 2km source and asset hashes are unchanged. At 128/256m
storage their v2 files are 1,481,432/781,915B and 2,293,070/1,375,142B. Complete
native audit peaks are 489,334,936/484,846,168B and 886,776,154/878,462,874B.
Mixed passes standalone 512MiB; dense requires 1GiB. This does not include a
consumer's screen, other owners or generation reservations. Source peaks are
179,915,638/189,952,614B mixed and 95,924,982/106,420,642B dense. No limit changed.
The 288m controls and 1,056m dense control also pass native 512MiB audit.
Across 12 fixed L02 artifacts, 126 selected cells match their v1 geometry and
occupied solids. The unchanged public driving-school v6 source additionally
matches all 432 cells at both 128/256m, with files 393,020/336,420B.

These are logical allowances and scoped source/generation observations, not RSS,
GPU, arbitrary-density, long-driving or platform acceptance. The authoring source
still bounds full audit; it is released on return. Larger-area experiments and
consumer acceptance must report their actual simultaneous resource owners.
