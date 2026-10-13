extends RefCounted
## Opt-in diagnostics only. Timers overlap; never sum CPU, GPU and frame interval.

class FrameCPU extends EngineProfiler:
	var samples: Array = []
	var remaining := 0
	func _tick(_frame_time: float, process_time: float, _physics_time: float, _physics_frame_time: float) -> void:
		if remaining <= 0: return
		samples.append(process_time*1000.0)
		remaining-=1

static func summary(values: Array, timer := false) -> Dictionary:
	var valid := values.filter(func(v): return is_finite(float(v)) and float(v) >= 0.0)
	var positive := valid.filter(func(v): return float(v) > 0.0).size()
	var available := not valid.is_empty() and valid.size() == values.size() and (not timer or positive == values.size())
	valid.sort()
	return {"status":"available" if available else "unavailable", "samples":values.size(), "positive_samples":positive,
		"p50_ms":percentile(valid,.5) if available else null,
		"p95_ms":percentile(valid,.95) if available else null,
		"p99_ms":percentile(valid,.99) if available else null,
		"max_ms":valid.back() if available else null}

static func percentile(sorted: Array, fraction: float) -> float:
	if sorted.is_empty(): return 0.0
	return float(sorted[clampi(ceili(sorted.size()*fraction)-1,0,sorted.size()-1)])

static func verdict(interval: Dictionary) -> Dictionary:
	var available: bool = interval.status == "available"
	return {"p95_limit_ms":16.7,"p99_limit_ms":33.3,
		"p95_pass":interval.p95_ms <= 16.7 if available else null,
		"p99_pass":interval.p99_ms <= 33.3 if available else null,
		"fixed_scene":"pass" if available and interval.p95_ms <= 16.7 and interval.p99_ms <= 33.3 else ("fail" if available else "unavailable"),
		"driving":"not_measured"}

static func environment(viewport: Window) -> Dictionary:
	var size := viewport.get_visible_rect().size
	var internal := Vector2i(size * viewport.scaling_3d_scale)
	return {"engine":Engine.get_version_info().string,"os":OS.get_name(),"cpu":OS.get_processor_name(),
		"device":RenderingServer.get_video_adapter_name(),"renderer":RenderingServer.get_current_rendering_method(),
		"driver":RenderingServer.get_current_rendering_driver_name(),"display":DisplayServer.get_name(),
		"vsync_mode":DisplayServer.window_get_vsync_mode(),"max_fps":Engine.max_fps,
		"screen_size":vec(DisplayServer.screen_get_size()),"window_size":vec(viewport.size),
		"viewport_size":vec(Vector2i(size)),"internal_render_size":vec(internal),"scale_3d":viewport.scaling_3d_scale,
		"occlusion":viewport.use_occlusion_culling,"automatic_quality":false,
		"directional_shadow_size":ProjectSettings.get_setting("rendering/lights_and_shadows/directional_shadow/size"),
		"positional_shadow_size":viewport.positional_shadow_atlas_size}

static func vec(value: Vector2i) -> Array: return [value.x,value.y]

static func sample(tree: SceneTree, viewport: Window, warmup: int, frames: int) -> Dictionary:
	var rid := viewport.get_viewport_rid()
	RenderingServer.viewport_set_measure_render_time(rid,true)
	var profiler := FrameCPU.new()
	var profiling := EngineDebugger.is_active()
	if profiling:
		EngineDebugger.register_profiler("mapkit_fixed_scene",profiler)
		EngineDebugger.profiler_enable("mapkit_fixed_scene",true)
	for frame in warmup: await tree.process_frame
	profiler.remaining=frames
	var intervals: Array=[];var setup: Array=[];var cpu: Array=[];var gpu: Array=[]
	var last := Time.get_ticks_usec()
	for frame in frames:
		await tree.process_frame
		var now := Time.get_ticks_usec()
		intervals.append(float(now-last)/1000.0);last=now
		setup.append(RenderingServer.get_frame_setup_time_cpu())
		cpu.append(RenderingServer.viewport_get_measured_render_time_cpu(rid))
		gpu.append(RenderingServer.viewport_get_measured_render_time_gpu(rid))
	var interval := summary(intervals)
	if profiling: EngineDebugger.unregister_profiler("mapkit_fixed_scene")
	var process := summary(profiler.samples,true)
	if not profiling or profiler.samples.size()!=frames:
		process={"status":"unavailable","samples":profiler.samples.size(),"p95_ms":null,"p99_ms":null,
			"reason":"requires --engine-profiler and one profiler callback per measured frame"}
	return {"warmup_frames":warmup,"measured_frames":frames,"frame_interval":interval,
		"engine_process_cpu":process,"render_setup_cpu":summary(setup,true),
		"viewport_render_cpu":summary(cpu,true),"viewport_render_gpu":summary(gpu,true),
		"assessment":verdict(interval)}

static func memory() -> Dictionary:
	return {"static_memory_bytes":Performance.get_monitor(Performance.MEMORY_STATIC),
		"video_memory_bytes":Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED),
		"nodes":Performance.get_monitor(Performance.OBJECT_NODE_COUNT),
		"resources":Performance.get_monitor(Performance.OBJECT_RESOURCE_COUNT)}

static func render(viewport: Viewport) -> Dictionary:
	var rid := viewport.get_viewport_rid()
	return {"objects":RenderingServer.viewport_get_render_info(rid,RenderingServer.VIEWPORT_RENDER_INFO_TYPE_VISIBLE,RenderingServer.VIEWPORT_RENDER_INFO_OBJECTS_IN_FRAME),
		"primitives":RenderingServer.viewport_get_render_info(rid,RenderingServer.VIEWPORT_RENDER_INFO_TYPE_VISIBLE,RenderingServer.VIEWPORT_RENDER_INFO_PRIMITIVES_IN_FRAME),
		"draw_calls":RenderingServer.viewport_get_render_info(rid,RenderingServer.VIEWPORT_RENDER_INFO_TYPE_VISIBLE,RenderingServer.VIEWPORT_RENDER_INFO_DRAW_CALLS_IN_FRAME)}
