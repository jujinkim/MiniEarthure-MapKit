# Recipe 9 integer geometry dependencies

The MapKit source remains MIT. The following linked libraries use the MIT license
option; retain these notices in binary distributions that include them.

| Library | Locked version | License notice | Source |
|---|---|---|---|
| i_overlay | 8.1.1 | [MIT2023](licenses/ishape-2023-MIT.txt) | https://github.com/iShape-Rust/iOverlay |
| i_float | 4.1.0 | [MIT2023](licenses/ishape-2023-MIT.txt) | https://github.com/iShape-Rust/iFloat |
| i_shape | 4.0.0 | [MIT2023](licenses/ishape-2023-MIT.txt) | https://github.com/iShape-Rust/iShape |
| i_tree | 0.19.0 | [MIT2024](licenses/ishape-2024-MIT.txt) | https://github.com/iShape-Rust/iTree |
| i_key_sort | 0.11.0 | [MIT2024](licenses/ishape-2024-MIT.txt) | https://github.com/iShape-Rust/iKeySort |

Cargo.lock records exact dependency revisions. The integer overlay does not use
its optional threading, rendering, or floating-point adapter features.
