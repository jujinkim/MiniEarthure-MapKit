# Static asset example

`create.py` reproducibly authors the original tetrahedron GLB, embedded/standalone
checker PNG and recipe-4 document using Python's standard library. All code,
geometry and pixels here are MIT. No external map or image data is included.

From this repository:

```sh
python3 examples/assets/create.py
python3 examples/third_party.py /tmp/new-assets.memap examples/assets/document.json
cargo test --locked -p mapkit-core --test assets
cargo test --locked -p mapkit-package --test assets
cargo build --locked -p mapkit-godot -p mapkit-cli
python3 scripts/verify_godot_layout.py --godot /path/to/godot --probe assets
```

The asymmetric tetrahedron crosses a cell seam. A second instance turns 90° and
uses a declarative texture/material override. A textured box, default tree, fence
and streetlight share the same renderer. GLB local coordinates are metres with
`(x,height,-map_y)` axes; proxies are integer centimetres in `(x,height,map_y)`.
The model and proxy vertices coincide. Assets may intentionally use simpler
collision proxies; visual geometry never defines gameplay collision.

Code's MIT license does not relicense third-party assets. Preserve each imported
asset's original attribution, license and notice in its required descriptor.
