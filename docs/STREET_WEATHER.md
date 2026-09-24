# Street weather renderer

The common renderer uses a fixed 64-slot splash MultiMesh (24 on low quality),
one shared quad/material and at most two additional surface rays per 10 Hz
update. Covered cameras and covered/steep sampled surfaces suppress splashes.
Clear weather and snow retire the pool contents without reallocating it.

Wet upward surfaces share world-space puddle masks, small ripple normals,
roughness 0.18 and modest specular response to the existing sky/lights. There
are no planar reflection passes or fluid simulation. Surface shaders expand
the shared include before duplicating context-owned shaders; this avoids the
Godot compatibility renderer losing a relative include on Shader.duplicate().

Rain is brighter/longer against darker cloud, direct and ambient light.
Clouds sample a curved noise dome without the old horizon projection clamp.
Sky fog is disabled because the shader owns horizon haze and cloud occlusion;
otherwise exponential fog hid the existing sun and moon completely in Editor.
Environment authority and race freezing are unchanged.

Automated checks on macOS arm64/Godot 4.7.2: rain/snow/clear and roof transitions,
24/64 pool bounds, ray bound, resource retirement, light-group priority/shadow
bounds, rendered day/night surfaces and sun/full/half moon/cloud occlusion pass.
Logs: /tmp/miniearthure-streets/weather-phase-check and weather-render3-check.
Fixed renders were inspected; detailed aesthetics and device performance remain
user verification. Existing atmosphere memory reservations are unchanged.
