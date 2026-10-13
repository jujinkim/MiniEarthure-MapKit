# Fixed-scene render diagnostics

`godot/diagnostics/fixed_world_capture.gd` is an opt-in SceneTree base. It opens a
package's authored cameras/environment and builds common near/distant renderers.
Consumers supply window, lighting, cache admission and detail-distance policy.
It does not run streaming, physics, vehicles or races.

Client and Editor retain their project renderer defaults. Every v1 report identifies
scope, renderer/driver, engine/device, VSync/FPS cap, display/internal sizes,
quality/occlusion, disabled automatic adjustment, package hash, preparation and
sample/repeat counts. Consumer performance docs own options. Editor preview
results do not establish Client performance.

## Timing and interpretation

- `frame_interval`: wall-clock intervals between process frames, including waits.
- `engine_process_cpu`: per-frame `EngineProfiler` time; requires `--debug` or the
  superproject's `--engine-profiler`. Delayed monitor values are not frame samples.
- `render_setup_cpu`: RenderingServer frame setup, across viewports.
- `viewport_render_cpu` / `viewport_render_gpu`: measured viewport rendering time,
  enabled before warmup. Missing, zero or partially zero timer series have
  `status: unavailable` and null percentiles. Missing GPU time is not GPU headroom.

These observations overlap and are not additive. They cannot alone establish a
CPU/GPU bottleneck. Godot's [render timing API](https://docs.godotengine.org/en/stable/classes/class_renderingserver.html#class-renderingserver-method-viewport-get-measured-render-time-cpu)
defines the timer boundaries. Launchers must reject fixed FPS before Godot consumes
the option. Movie mode is rejected by engine state. Image readback/encoding occur
after measurement. Nearest-rank p95/p99 have independent criteria and a
`fixed_scene` verdict; `driving` is `not_measured`. Tool completion PASS does not
mean the frame target passed.

`preparation_stages` records per-view counts, totals and single-call peaks in µs,
with peak cell and template asset/mesh/surface context. Near generation, extraction,
batch preparation, begin and attachment phases are separate from far stages.
`cache/*` uses positive preparation-timer deltas after each advance, excluding zero
durations. These subphases overlap attachment; snapshots are outside stage timers
but included in preparation wall time.

Opt-in `mesh/*` timers further isolate LOD input-array retrieval, importer surface
insertion, `generate_lods`, `get_mesh`, material duplication and surface binding.
`material_style` includes the whole styling call; `material_bind` measures binding
and `material_source_release` measures dropping its local source reference.
Opaque GLB sources are captured and retired before upload inside `cache/import`;
their admitted styling step activates the prepared shared-shader binding. Compare
both import and styling costs to detect displaced work. `material_track`
measures lease tracking, and `material_finalize` covers node margin, shadow and
override setup. `occluder_cached_arrays` marks reuse of retained decoded CPU faces;
`occluder_arrays` marks a mesh readback for independently prepared/preview meshes.
They count even zero-duration calls and attach a surface index where applicable.
Private LOD outputs need no material duplication; borrowed meshes still do.
Observers are installed for one advance, then cleared; reports retain primitives.
Callbacks are outside inner timers but inside cache/attachment timers. These totals
overlap. Staging cannot interrupt an engine call. LOD jobs reuse decoded faces for
occlusion; initial input readback remains a separate limit.

`cache_activity` counts this view's imports/hits. `cache` and `attachment` retain
cache/process-lifetime counters (`timing_scopes`); use `preparation_stages` for
visit comparisons. Reused templates produce no new `cache/*` samples. Repeats
recreate scenes with a warm process/driver. Synchronous generation does not measure
consumer worker scheduling or driving frames.

### Readback isolation

On RenderingDevice renderers, `surface_get_arrays` retrieves packed buffers before
restoring CPU arrays. Godot's [buffer readback](https://docs.godotengine.org/en/stable/classes/class_renderingdevice.html#class-renderingdevice-method-buffer-get-data)
blocks GPU work. Pending uploads/rendering can affect the call; its duration is
not solely this mesh's upload or decompression. Repeated reads still synchronize.

- Time `RenderingServer.mesh_get_surface` separately to probe packed retrieval.
  A following `surface_get_arrays` downloads again, so subtracting the two timings
  cannot isolate restoration. The extra read perturbs preparation.
- For CPU isolation, replay captured packed buffers through the same engine's
  headless dummy mesh storage, excluding creation/file I/O. Require every rendered
  array slot to match. This excludes GPU waits; production ownership stays intact.
- Matched Metal warehouse probes point to synchronous retrieval rather than CPU
  restoration. First-use peaks vary across import/readback and fresh processes;
  frame delays retain readback costs. Individual waits need an engine trace.

Raw GLTF arrays precede compression and change geometry/LOD output. Script bindings
expose neither the CPU surface decoder nor ArrayMesh buffer RIDs for async retrieval.

### Attachment upload isolation

Phase names describe renderer branches, not individual GPU operations.
`instance_upload` includes built-in canopy LOD generation/readback as well as GLB
placement. Capture object/asset or surface-material identity with the cell and
attachment generation/frame; a pending object is not the current mesh batch.

Separate mesh creation, array upload, material preparation, resource tracking and
scene registration. For MultiMesh, separate setup from transforms, bounds,
occluders, custom data and visibility. Include callee-local reference retirement.
The bounded Metal fixture exposes long first generated-surface material preparation;
short GLB placement setters do not explain that cost. Its image proxies select the
unbatched mesh path; prepared batches require a separate comparison.

Record wall and caller-thread CPU time around the same operation. Their gap can
include blocking or descheduling; identifying an engine/GPU wait requires aligned
stack evidence. Short sampling can capture no stacks. Nested costs overlap and
instrumentation perturbs timing, so neither moving a peak nor one faster run proves
an improvement. Compare original/instrumented hashes, display counts, cache reuse,
pressure retirement and reservation return before changing material preparation.

## Ownership and failure

Preparation checks its deadline between native/attachment calls; the caller's
process timeout bounds a stuck operation. Invalid input/options/cameras, generation,
admission and cancellation fail explicitly. Partial scenes use normal cancellation
and cache shutdown. Existing reports/images are not overwritten.

Reports include cache bytes, weak scene references, engine monitors and optional
consumer reservations. Poll retirement across engine frames before checking zero;
revisits keep the cache until the final shutdown. Viewport shadow/target reservations
may survive scene teardown. Logical bytes, engine memory and RSS differ; driver
and shader caches need not return to a cold-process baseline.

`release/*` times cancellation/queueing calls only; deferred engine destruction is
covered by the later resource checks, not by those durations. Revisit shutdown is
timed separately in `revisit_release.shutdown_usec`. Instrumentation retains only
bounded phase aggregates and primitive peak context, never scene/resource owners.

`godot/tests/render_measurement_validator.gd` covers missing GPU timing, frame
thresholds, malformed options, missing views, preparation failure/timeout and
partial-scene teardown, phase peak context and revisit isolation without resetting
shared lifetime counters, including inner-operation surface attribution. Consumer graphical checks cover renderer metadata, frame
sampling, cache retirement and isolated startup.
