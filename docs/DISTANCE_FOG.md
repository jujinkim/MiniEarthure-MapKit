# Optional display-distance fog

`godot/environment_renderer.gd.set_display_distance(metres)` opts a visual consumer
into depth fog. Zero (the default) keeps the existing weather-dependent exponential
fog, including standalone Editor behavior. Negative/non-finite inputs are ignored.

The transition starts at 60% of the supplied display range and reaches full fog at
95%, with curve 0.7. Aerial perspective uses the current sky radiance, including
day/night and weather changes. Sky visibility itself is preserved. Godot samples a
blurred sky radiance for aerial perspective; this is atmospheric color blending,
not exact pixel transparency against the sky.

Consumers supply their actual display range after updating streaming preferences.
No map cells, collision shapes, materials, lights, shadow maps or viewport owners
are created by this option. Near and distant map renderers share the Environment
fog, so their material transitions follow the same rule. Fog does not grant loading
readiness or collision admission. Disabling it restores the current weather fog.

Validated with the standalone renderer relocation probe and a consumer's rendered
day/night/rain depth fixture, actual near/far building materials, range updates and
resource teardown. The implementation uses the engine's
[Environment depth fog](https://docs.godotengine.org/en/latest/classes/class_environment.html)
without an additional render pass.
