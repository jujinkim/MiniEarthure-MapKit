# Recipe 7 vegetation assets

Recipe 7 lets a Forest or Orchard zone select a shared GLB instead of the builtin
tree. The zone polygon, exclusions, seed, spacing and density remain the inputs
to MapKit's deterministic generator. There is no target count, archived point
list, relocation search or fill quota. Ineligible candidates are omitted.

```json
{
  "id": "garden",
  "polygon": [[0,0],[1000,0],[1000,1000],[0,1000]],
  "kind": "orchard",
  "spacing_cm": 300,
  "density_per_mille": 700,
  "exclusions": [],
  "tree": {"asset_id":"city-tree","radius_cm":58,"clearance_cm":5}
}
```

`asset_id` references an existing custom GLB with box and/or convex collision.
`radius_cm` declares half the square canopy footprint; it must enclose every
horizontal collision vertex and every quarter turn. Authors also enclose the
visible model in this footprint. Generated custom trees use the existing small
tree envelope: 1–200cm radius. Larger scenery remains an ordinary placement.
`clearance_cm` adds space from roads, buildings, authored objects and competing
trees. The full canopy stays inside its own zone and outside exclusion polygons.
Density zero generates none; narrow or occupied zones can also generate none.

Existing candidate hashing, forest jitter, source-global thinning, terrain
anchoring and centre ownership are reused. Custom trees emit their shared asset
ID and complete declared collision at one owner cell. Recipe 7 occupancy queries
include a conservative 200cm owner halo, including metadata-only indexed queries;
cell allowance checks still apply before allocation. Source-region derivation
keeps zone-referenced assets and material dependencies. Cost planning counts all
box/convex shapes, and the source ownership audit accounts for the new asset ID.

The `tree` field requires explicit recipe 7. Omission retains builtin behavior;
recipes 1–6 preserve their serialization and generated output. Package v1,
indexed v2 and generated v6 formats remain unchanged. Core `zone_assets` and
`zone_obstacles` tests cover opt-in, rejected references/footprints, zero density,
spacing/exclusions, no relocation, road/obstacle clearance, seam ownership,
regular/indexed metadata queries, regional asset retention and output budgets.
