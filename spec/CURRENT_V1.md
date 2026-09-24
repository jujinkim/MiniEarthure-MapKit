# Current map contract

The 2026-09-24 reader-compatibility decision replaces the package-v1 portion of
the 2026-09-20 single-v1 decision. `.memap` `manifest.json.format_version` is the
required MapKit reader contract and is now 2. A reader accepts exactly its current
value and reports `E_VERSION` before parsing `document.json` when it differs.
The existing field makes pre-change DLLs reject new packages at their version gate
without encountering an unknown manifest field. This value changes only when a
package requires a newer MapKit reader, never for ordinary map content edits.
Recipe, generated geometry, scene units, regional index and generated archive remain
v1. Earlier packages and user files are preserved, without an old reader or an
automatic converter; they require explicit re-export under the current contract.

Generation always uses integer road arrangement, connected sidewalks, courtyard
buildings, indexed placement validation, authored tree assets and environment
profiles. The latest road partition and three-dimensional seam rules remain.
Regional source uses the local dependency closure, retaining complete context only
where implicit widths or repetitions require it. Decoder peak declarations are
mandatory and verified against payload bytes. Size, cancellation, allocation,
content hash and audit checks remain binding.

Generated caches have one MKCELL01 layout: fixed header, triangle/object arrays,
prism count and records, convex count and records, then a required u32 byte length and canonical gimmick JSON,
including empty arrays/counts. Their key
includes BUILD_FINGERPRINT, generated version, world identity and cell. Cargo derives
the fingerprint from sorted relative source/schema/dependency paths and UTF-8 contents
with LF line endings and length delimiters. Absolute paths, platform, timestamps and
build output do not enter the digest. Archive roundtrip and corruption tests remain;
portable geometry vectors use a fixed test key independent of source edits.

The renderer's update_environment accepts `immediate` to apply road wetness/snow
without blending at a race boundary. It consumes resolved state and never advances
simulation time; precipitation animation remains live.

Earlier dated design and validation documents preserve evidence of prior formats.
Their old-reader, version-selection and frozen legacy-output requirements are superseded.

Authored placement validation uses the convex hull of the complete transformed
collision footprint, rather than its axis-aligned bounding rectangle. Placement
contact remains forbidden. Bridge/elevated corridors admit a support only when
its complete collision top is at least one centimetre below every interpolated
deck segment across the footprint. Ground/tunnel/underpass corridors remain
excluded. These checks charge the existing bounded placement work allowance.
The miniature kit and climate ground palette are renderer/source assets; surface
identities and physical material behavior do not change.

File-backed `read_with_budget(path, allowance)` bounds compressed input before
allocation and checks the existing validation peak before inflation. Godot exposes
`open_package_budgeted(path, allowance)` and `unpack_source(destination)`. Restore
uses the already validated immutable package, retains every original payload and
requires a new destination directory. Failed opens clear prior native state.
These APIs do not introduce another format or a migration path.


### Immutable source preparation and menu preview

The Godot bridge caches cell cost/archive descriptors on the validated source
(maximum 16,384 entries, included in the source read allowance). Authored
placement candidates use the existing spatial index. Asset hashes and costs are
computed once per source; packed display bytes are created lazily only on the
presentation path and shared across chunks. Generated geometry and canonical
hashes are unchanged. No headless path creates display bytes or GPU resources.

`cargo run -p mapkit-cli --example menu_preview -- INPUT.memap OUTPUT.json`
creates a v1 roads-and-bounds overview tied to the original package SHA-256.
It does not alter the package or grant spawn authority. Consumers must validate
the package and selected surface before entry. The `cache_population` example
creates valid disposable cell archives from a synthetic package for cache tests.

## Course authoring and completion references (2026-09-22)

`MapDocument.courses` is an optional list of up to 64 public current-v1 course documents.
Definitions include map ID, name, driving-content hash, ordered 3D sphere/upward-hemisphere
checkpoints (1–1000 m radius), circuit/sprint mode, ground/air start and explicit horizontal
direction. Circuit finish reuses checkpoint zero. Lap count is a hosting choice, not geometry.
Overlaps are legal. Surface labels are authoring hints, not completion constraints.

Optional completion references identify bounded opaque consumer records by hash, size and
`course-validation/<sha256>.mevalidation`. MapKit verifies bytes and references, never
certifies player completion. Courses and their records participate in package integrity but
are excluded from driving-content hashes in both containers. Projects preserve stale
references so consumers can display revalidation required after geometry/map changes.

Verification: package unit tests 15, course tests 2, indexed tests 16 and package contract
tests 20 passed locally. Runtime certification and application acceptance are separate.

## Dynamic light inputs (2026-09-22)

Consumers supply atomic generic light groups instead of inferred vehicle poses.
See [light group contract](../docs/LIGHT_GROUPS.md). Pool and shadow caps remain
unchanged; no vehicle design data or serialized map contract is added.

## Declarative driving structures (2026-09-24)

`MapDocument.gimmicks` declares at most 128 stable IDs. Each record has centimetre
position, millidegree Euler YXZ rotation, per-mille scale, 1–32 validated convex
parts, a surface and RGBA color. Motion is `static`, cosine ping-pong `translate`,
continuous `rotate`, `boost` or `launch`. Motion stores period/phase milliseconds,
world-space displacement/impulse, rotation axis and per-vehicle cooldown. No
executable user scripts are accepted. See [the reusable library](../examples/driving-library/README.md).

Required safety bounds enclose the full transformed motion and authored landing
area. Validation uses a conservative integer L1 radius, bounded displacement,
period, scale, impulse and total extent; bounds must remain inside the map.
Generated cells include each intersecting definition. Consumers deduplicate by ID
within their world generation and retain a global motion phase through cell reloads.
`driving_window` extends the ordinary 3×3 set to include complete safety rectangles
for definitions touching that set. Consumers retain their own cell and memory caps.

Canonical source/chunk hashes, regional metadata, the packed `gimmicks_json` field
and archive audits include these definitions. Cost declares 16 KiB plus 32 KiB per
convex part per intersecting object/cell; occupancy includes per-part static or
translation bounds and full rotation sweeps. Hollow passages retain separate parts.
The generated archive layout changes within v1; build fingerprints invalidate
old disposable caches. There is no fallback decoder or original-package conversion.

`godot/gimmick_geometry.gd` is pure shared geometry/pose/contact-velocity math.
The consumer supplies authority time, generation, reset and activation policy.
MapKit never starts a simulation clock. Geometry is transformed once into scene
coordinates; scale is baked into physics shapes by the physics consumer.
