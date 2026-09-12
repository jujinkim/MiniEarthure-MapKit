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

## L01-D initial preparation diagnostics — 2026-09-12

An opt-in `ReadProfile` separates complete audit/source read, parsing, validation,
dependency derivation, canonical comparison, overview and file hashing. The
`regional_benchmark` example additionally measures estimates, occupied generation
and generated hashes with one scoped source. Normal reader methods do not read a
clock; diagnostics never enter package identity or replace any admission checks.

Same fixed dense 2km artifacts with debug native code identified whole audit as
the initial preparation bottleneck. The internal audit now moves its already
validated authored document into summary construction instead of parsing and
validating the same bytes again. That document is still released before return;
the existing original/summary reservations and complete payload/closure checks
remain. `PreparedMap::new_region` validates before normalizing record order and
does not repeat that unchanged geometry check. Public mutable source topology is never a
validation receipt: edited documents are fully checked on every preparation.
The public `source_overview` validation remains intact.
No format, recipe/generated bytes, source cache, budget or worker lifetime changes.

Repeated full-world placement/road scans during each dependency derivation remain
the largest measured audit phase. A reusable derivation plan would need bounded
allocation and ownership evidence plus frozen version-1/version-2 closure parity;
it is a separate follow-up. Source LRU, sharded authoring, region transport and
larger-area/platform acceptance remain unimplemented or separately pending.

In one sequential macOS arm64 debug comparison with identical dense 2km v2
artifacts, complete standalone audit changed from 15.424s to 14.400s at 128m and
8.156s to 7.070s at 256m. The removed summary parse/validation phase previously
used 1.209s/1.214s. First-region preparation changed from 4.681ms/18.805ms to
2.440ms/9.531ms; complete source load was 78.971ms/111.940ms before and
77.807ms/103.428ms after. Dependency derivation still used 9.086s/2.546s after
the change. All non-timing identity, allowance, generated-hash and occupancy-count
fields matched for the same five cells at both storage sizes. These are scoped
single-run observations, not controlled cold-cache or startup-SLA acceptance.

The 126-test Rust suite passed; the final preservation of pre-normalization input
checks was then rechecked by five core cost/preparation tests and 14 indexed tests.
These include mutated indexed-source rejection, region confinement, complete
audit/geometry/occupied-solid equivalence, frozen v1, corruption, cancellation,
pre-read denial and recovery. Debug/release workspace native, CLI and examples
build; pure-core ownership passes with Python 3.12. Consumer binding, collision,
ownership/join and platform evidence belongs to each consuming application's
validation report.
The final release `regional_parity` example also compared complete generated
geometry and occupied-solid structures in 20 representative cells across the
fixed dense/mixed 2km version-1/version-2 pairs at 128m/256m. All match; these
partial migration comparisons do not replace complete artifact admission.

## L01-E decision before implementation — 2026-09-12

Full audit may build one immutable, document-borrowing `RegionSourcePlan` after
the original source, all payloads, terrain and world hash have passed validation.
It caches placement footprint bounds, whole-road bounds and global selection
constants once. Placement footprint and anchor remain separate predicates;
road bounds only reject distant candidates before the original segment test.
Endpoint stars remain one-hop, and the last tied widest road remains selected.
Version-1 and conservative fallbacks keep their original global context. The
ordinary uncached producer stays available as the canonical comparison oracle.
This changes no file/index/world identity, generation rule or protocol.

The two arrays have exactly one 32-byte Bounds per placement/road, at most
200,000 combined entries under the existing input limit (6,400,000 bytes).
Reserve `min(original record bytes, 6,400,000) + 256` before reading any payload,
in addition to the existing closure-phase original/expected/canonical/scratch
allowances. Before allocation, enforce the actual entry count/byte bound; never
grow these arrays. The 256 bytes cover vector/plan headers and allocation
bookkeeping. Only one placement's existing footprint scratch is live during
construction. The maximum of original validation and this augmented comparison
phase determines admission; no consumer budget is raised.

Check cancellation before construction, every 64 records and after construction,
as well as before each regional derivation and record read. Drop the plan on
success, error or cancellation before audit returns/summary construction; it
cannot escape its borrowed original, survive on the reader, or become a cache.
This is a bounded pure selection transform, not a validation receipt or unchecked
PreparedMap constructor. Its returned ordinary MapDocument still requires the
existing validation before execution. Full source/payload/inventory and every
canonical regional comparison remain required before installation.

Validate cached/uncached canonical equality for v1/v2, all fallback modes,
seams/terrain/assets/convex placement, segment-vs-road bounds, endpoint stars and
widest ties; preserve generated/occupied parity, pre-read budget refusal and
cancellation/current-owner protection. Compare the same fixed L02 artifacts and
consumer initial preparation with separate plan-build/derivation timings.

### Implemented and scoped validation

132 standalone Rust tests pass, including six new plan/budget regressions.
Canonical source equality covers both closure versions, recipes 1–6, implicit
width/repetition fallback, reversed source order, offset/rotated odd convex
footprints with a separate anchor, whole-road AABB false positives, exact halo
touch, one-hop endpoints and last-tied widest roads. Fallback global scans also
observe cancellation every 64 records. An audit allowance one byte below its
reported peak rejects before payload I/O; the exact allowance succeeds.
Existing full audit, unused payload, frozen v1, corruption, terrain seams,
generated/occupied-solid and source independence tests remain passing.
Debug/release workspace native, CLI and examples build; core ownership passes.

On the same fixed synthetic dense 2km debug artifacts, 128m/256m standalone audit
changed from 14.199s/6.955s to 8.025s/5.399s. Derivation changed from
8.965s/2.481s to 2.763s/0.928s, plus one plan build of 27.965ms/28.438ms.
The 22,500 placements and 10,891 roads use 1,068,768 logical plan bytes including
overhead, within their 5,973,705-byte pre-read reservation. The original validation
phase still dominates the phase maximum, so these artifacts' audit peaks remain
unchanged. Mixed 256m audit changed from 2.552s to 2.031s. File/index/world identity,
source allowances and selected cell results are identical; all four fixed
mixed/dense 128m/256m artifacts pass complete audit, and 20 selected v1/v2 cells
match complete generated geometry and occupied solids.

These are single sequential macOS arm64/M1 debug observations, not controlled
cold-cache, RSS/GPU, consumer-budget or startup-SLA acceptance. Full original
validation/retention still bounds audit memory. The plan is released before
summary/return and does not solve sharded authoring, resident source LRU,
region-request transport or larger-area/platform acceptance.

## L01-F decision before implementation — 2026-09-12

Replace the complete audit's whole-source JSON trees with a strict, discarding
JSON pass followed by direct typed decoding, and a borrowed streaming world hash.
The strict pass still rejects duplicate keys (including escaped aliases and
nested metadata), noninteger numbers, invalid/trailing JSON and excessive depth.
It retains only the keys of currently open objects; arrays retain no elements.
The typed decoder runs after that scratch is gone. Regional reads and the legacy
package reader keep their existing implementation and reservations.

Admission becomes staged inside the caller's already reserved worker allowance.
Before any original record I/O reserve index + twice original expanded bytes +
the existing 32-times-source typed/parse allowance + twice compressed bytes +
8MiB scratch. Check cancellation during strict traversal and before typed decode.
After decoding, walk every owned String/Vec capacity, inline structure and nested
allocation without allocating. Include 64 bytes per allocation and checked
arithmetic; reject if this ownership exceeds the pre-reserved typed envelope.
This ownership calculation is a planning charge, not RSS or allocator telemetry.

Before semantic validation or reading any payload, replace only the original
typed 32-times-source resident allowance with that checked owned capacity. Keep
all original files, payload/GLB allowances, decoder declarations and verification,
the existing 32-times-source clone/scratch allowance, per-region 128-times-source
canonical/expected workspace, plan bytes, index/query/summary and fixed scratch.
The original-validation phase needs one source scratch allowance after removing
whole-source parse/canonical trees; it does not overlap a typed decoder with the
strict pass. Full original validation, all unused payloads, terrain, world identity
and every v1/v2 canonical closure/inventory comparison remain mandatory.

The reported audit peak is the maximum of preflight and the checked later phases.
`audit_peak_bytes` remains an index-only conservative sufficient bound; its
minus-one value no longer means guaranteed refusal. A separate preflight bound
rejects before source I/O. A later budget refusal may have read/parsed the source
but must precede semantic/payload/closure allocations. No receipt is stored on the
reader and no source survives audit. Errors, cancellation and success drop all
local ownership, and consumer reservations remain held until worker join.

Before creating each expected regional canonical tree, count its serialized
length without allocating and require equality with the indexed record size.
Check cancellation every 64 serializer writes. A forged tiny regional record
therefore rejects while still in the original-source clone allowance, before
record I/O or record-sized canonical workspace. Length equality never replaces
the subsequent full record length/hash and canonical byte comparison.

Hashing sorts borrowed record indices one collection at a time, with the original
index as the tie breaker to preserve stable normalization. Canonical serializers
preserve the existing field ordering, omissions, integers and nested-array order;
provenance/attributions remain excluded only from world identity. Hash equality
against the unchanged legacy oracle and exact-capacity, staged-denial, duplicate,
corruption/cancellation and fixed-artifact admission regressions are required.
No index/protocol/recipe/generated format, limit, map quality, transport, renderer
reservation or source-cache policy changes. The same fixed 2km and small Client
controls decide the supported admission results; larger areas and final platform,
cold/50MB, human/internet and installation acceptance remain separate.

### Implemented and scoped validation

145 Rust tests are covered by the passing 144-test suite followed by the final
16 indexed regressions after adding the forged-size guard. Twelve new unit tests
cover strict/cancelled parsing, all owned capacities and schema fields, exact
legacy canonical hash parity for recipes 1–6/options/order and hash cancellation.
The staged-boundary regression refuses before source I/O or before semantic/
payload work, accepts the exact reported peak and preserves the index upper bound.
The additional forged tiny closure refuses before region I/O/canonical allocation.
Debug/release workspace native, CLI and examples build; core purity passes.

All four fixed mixed/dense 2km 128/256m artifacts audit within the Client worker's
383,254,528-byte allowance after subtracting its additional index query metadata.
Core audit summaries (including 16MiB summary but excluding that extra native
metadata) require mixed 272,991,727/270,747,343B and dense
323,057,523/318,900,883B. Dense source owned capacity is 16,481,529B, versus its
unchanged 191,150,368B pre-parse typed envelope. Mixed ownership is 6,617,271B.
The dense256 exact summary peak succeeds; one byte less refuses after preflight,
and preflight+summary minus one refuses before original I/O. Frozen v1 still audits.
Twenty selected v1/v2 cells match complete generated geometry and occupied solids.

The standalone diagnostic observes 11,315,093–26,233,434 requested Rust heap bytes
at audit peak across these four cases. Every success/refusal returns exactly to
the pre-audit live allocation baseline after the result is dropped; dropping the
reader releases its index. These observations exclude allocator overhead, external
allocations, stack and RSS and do not replace the conservative logical bounds.
Consumer safety/initial admission and platform evidence remain application-owned.
