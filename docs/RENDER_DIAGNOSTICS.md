# Fixed-scene render diagnostics

`godot/diagnostics/fixed_world_capture.gd` is an opt-in SceneTree base for consumer
diagnostic scripts. It opens an existing package and its authored `region.json`
review cameras / `document.json` environment, then builds a fixed scene with the
common near/distant renderers. Consumers supply window, lighting, cache admission
and detail-distance policy. This does not run streaming, physics, vehicles or races.

Client and Editor adapters retain their own project renderer defaults. Every v1
report row identifies `scope`, actual renderer/driver, engine/device, VSync/FPS cap,
screen/window/viewport/internal 3D sizes, quality/occlusion, disabled automatic
adjustment, package SHA-256, preparation, repeat and measured-frame count. Never
apply an Editor preview result to Client performance. Common inputs and options
are described by each consumer's owning performance document.

## Timing and interpretation

- `frame_interval`: wall-clock intervals between process frames, including waits.
- `engine_process_cpu`: per-frame `EngineProfiler` process time; requires the local
  debugger (`--debug`, or the superproject's `--engine-profiler`). Delayed
  `Performance.TIME_PROCESS` monitor values are not used as frame samples.
- `render_setup_cpu`: RenderingServer frame setup, across viewports.
- `viewport_render_cpu` / `viewport_render_gpu`: measured viewport rendering time,
  enabled before warmup. Missing, zero or partially zero timer series have
  `status: unavailable` and null percentiles. Missing GPU time is not GPU headroom.

These observations overlap and are not additive. They cannot alone establish a
CPU/GPU bottleneck. Godot's [render timing API](https://docs.godotengine.org/en/stable/classes/class_renderingserver.html#class-renderingserver-method-viewport-get-measured-render-time-cpu)
defines the rendering timer boundaries. The superproject launcher rejects fixed
FPS before engine startup; Godot consumes that option before scripts can inspect
it. Other launchers must enforce the same rule. Movie mode is rejected by its
engine state;
optional image readback and encoding occur after measurement. Percentiles use
nearest rank. Each run reports independent p95/p99 criteria and a `fixed_scene`
verdict; `driving` remains `not_measured`. Completion PASS means the tool completed,
not that the frame target passed.

## Ownership and failure

Preparation has a bounded deadline checked between native generation and render
attachment steps; a caller's process timeout also bounds a stuck native operation.
Missing input, invalid options, unknown cameras, failed generation/admission and
cancelled render jobs fail explicitly. Partial scenes follow the same cancellation
and cache shutdown path as successful scenes. Existing reports/images are not
overwritten.

Reports include cache bytes, weakly tracked scene resources, engine memory monitors
and optional consumer reservations after release. Retirement is polled across
engine frames before checking zero. Revisit mode keeps the shared cache until the
last view, then checks its shutdown. Viewport-owned shadow/target reservations may
survive scene teardown; logical cache/lease bytes, engine memory and process RSS
are distinct. Shader/preload and driver caches are not required to return to a
cold process baseline.

`godot/tests/render_measurement_validator.gd` covers missing GPU timing, frame
thresholds, malformed options, missing views, preparation failure/timeout and
partial-scene teardown. Consumer graphical checks cover renderer metadata, frame
sampling, cache retirement and isolated startup.
