# Environment authoring and presentation

Recipe 8 adds an optional environment profile, version 1. Old recipes retain their
original representation and hashes. Profile changes affect the world hash; they
never change existing archive bytes automatically. See `spec/document.schema.json`.

- Concepts: polar, metropolis, countryside, middle-eastern, desert, jungle,
  southeast-asian. Independent architecture: modern/rural/adobe/timber/tropical;
  climate: temperate/polar/arid/tropical; settlement: urban/village/sparse/wilderness.
  An empty dimension follows the concept. Regions use ordered, bounded polygons
  in centimetres; the first matching region wins. Weather remains map-wide.
- Light bindings refer to the imported GLB material index, independent of the mesh
  surface order. Window/bulb lists may be empty. No lamps or buildings are added
  implicitly. Window randomization uses map ID, object ID and night index; near
  instances and distant proxies use the same seed. Windows switch off at a stable
  minute from 00:00 through 04:00, and at sunrise if earlier.
- Simple solar cycles use authored rise/set times. The astronomical mode uses
  low-cost J2000 orbital approximations, lunar phase and polar day/night. This is
  a visual game sky, not an ephemeris for navigation. Polar-night occupancy uses
  18:00 through the nightly cutoff. Streetlights follow the actual solar horizon.
- Opaque surfaces preserve shared textures and instancing. A session-owned 2 × 1
  float texture carries wetness/snow/time. A bounded light pool (8 or 4) prioritizes
  the local vehicle and uses emissive distant lamps/windows. Precipitation is a
  bounded camera-centred GPU particle volume; an overhead ray shelters the camera.
- Shared imported templates have a 128MiB cache ceiling, with every allocation
  additionally charged to the consumer's overall budget. Cache keys include only
  the current asset's light binding so adjacent cells share textures and meshes.
- Original 128px material tiles and the new-project upgrader are in
  `scripts/atmosphere_assets.py`. Existing image-bearing GLBs retain their original
  image bytes and UVs. Geometry, normals, indices, collision and attribution remain
  intact. Source projects, payloads, generated packages and user data are never
  deleted by the upgrader.

MapKit does not advance game time or simulate grip. Consumers supply resolved
state. The game Runtime owns authority, scheduling, networking and physics. The
standalone Editor supplies an independent preview state.

Validation: `cargo test --workspace --locked`; `environment_contract` covers
recipe gating, canonical identity, roundtrip and invalid authoring. Consumer
render tests additionally exercise actual GLB materials and the bounded light pool.
