# Current v1 contract

The 2026-09-20 replacement removes versioned implementations. Package, recipe,
generated geometry, scene units, regional index and generated archive use v1.
Changing implementation updates this contract in place; version increments need
explicit user approval. Existing data is neither searched for conversion nor deleted.

Generation always uses integer road arrangement, connected sidewalks, courtyard
buildings, indexed placement validation, authored tree assets and environment
profiles. The latest road partition and three-dimensional seam rules remain.
Regional source uses the local dependency closure, retaining complete context only
where implicit widths or repetitions require it. Decoder peak declarations are
mandatory and verified against payload bytes. Size, cancellation, allocation,
content hash and audit checks remain binding.

Generated caches have one MKCELL01 layout: fixed header, triangle/object arrays,
prism count and records, convex count and records, including zero counts. Their key
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
