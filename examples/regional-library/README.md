# Regional miniature kit

Original reusable MIT geometry, authored in `scripts/regional_assets.py`. No map
layout, game codec, third-party game assets or surveyed data is included here.
`library.json` records every GLB SHA-256, license and box/convex collision proxy.

The kit contains four variants each of towers, shops, stone houses, farmhouses,
polar houses, stilt houses and warehouses; a courtyard; layered sandstone;
trees, field/water tiles; windmill/watermill/waterfall, crane and observatory;
quantized rail orientations and half-metre pier heights. Buildings combine
foundations, floors, openings, roof shapes, canopies and roof equipment.

Regenerate into a **new** directory (the tool refuses an existing destination):

```sh
python scripts/regional_assets.py /tmp/new-regional-library
python -m unittest discover -s scripts -p test_regional_assets.py
```

All coordinates use the existing metre-to-centimetre conversion and v1 asset
contract. Region-specific signs/font licenses belong to the consuming map.
