# District-scale asset library

Original MIT architecture and terrain props in actual metres. This library adds
20–35 storey podium towers, attached shop/house parcels, hillside apartment
blocks with deep foundations, warehouse yards, courtyard compounds, crops,
layered forest crowns, kerbside amenities and bridge rails. Map layouts belong
to MapEditor. The original regional-library remains preserved.

Generate into a new directory from the superproject's Python 3.12 `.venv`:

```sh
rtk proxy .venv/bin/python map-kit/scripts/district_assets.py /tmp/new-district-library
rtk proxy .venv/bin/python -m unittest discover -s map-kit/scripts -p test_district_assets.py
```

`library.json` records every embedded GLB, collision proxy, attribution and
SHA-256. No external downloads, textures or installed fonts are required.
