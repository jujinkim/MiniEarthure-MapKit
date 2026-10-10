# Prepared generation and job cancellation

The current v1 generator prepares source-global placement decisions once per
immutable `PreparedMap`. Authored proxy footprints, accepted repetitions and the
complete occupied sequence retain normalized source order. A deterministic bounds
index limits cell emission; separate road/building/occupied indices accelerate
vegetation eligibility while retaining exact predicates and cross-cell competitors.
Vegetation visits the cell/zone envelope intersection with its lattice halo, so a
small 1:8 zone does not scan the empty remainder of a large cell. Global candidate
seeds, ordering, jitter, eligibility, thinning and all work/output limits remain.
An anchor is included in placement bounds even when its collision proxy is offset.
Asset and heightmap descriptor lookups have immutable indices. No complete map
geometry or quality-reduced substitute is cached.

Prepared indices are charged by package read planning: an additional eight times
structured source bytes and 32 MiB for bounded repetition/footprint/index storage.
The existing generated-cell estimate cache, output limits and work budgets remain.

`cancellation::CancellationToken::run(|| prepared.generate(...))` provides a
job-local cooperative generation path. Existing Editor calls remain valid. Tokens
are never reset; a replacement gets its own token. Checkpoints cover placement
predicates, triangle emission, source read blocks and regional read tickets,
archive encode/decode and native presentation/packing loops. They do not interrupt
one OS read, image decode, sort or polygon operation in the middle.

The Godot `MapKitWorkToken` binds that scope to one worker thread with `enter` /
`leave`; `cancel` is atomic. The caller must pair entry and exit and treat
`E_CANCELLED` as obsolete work, not corrupt input or a failed required map.
`render_memory.prepare_batches` accepts an optional cancellation Callable; ordinary
Editor callers omit it. Runtime ownership, ACKs, retirement and scheduling are
outside MapKit.

Validation: affected core placement/occupancy/road/cost/zone and archive regressions,
prepared/raw boundary parity, independent tokens and controlled running-worker
cancellation; package indexed closure/input defense/contract tests. Existing v1
format numbers and generated geometry/hash ordering are unchanged.
The envelope optimization passes placement10/spatial4/zone-assets4/obstacles1/
cost6 checks and the four behavioral determinism checks. All 28 current vector
cells are byte-identical to pre-optimization `ffe7768`; the separate stale frozen
vector failure is recorded in [DETERMINISM](../spec/DETERMINISM.md).
