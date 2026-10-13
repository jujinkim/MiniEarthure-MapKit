extends RefCounted
## Offline fixed review camera; no scheduling, physics, input or race simulation.
const RENDER := preload("../chunk_renderer.gd")
const FAR := preload("../distant_renderer.gd")
const CACHE := preload("../render_resource_cache.gd")
const QUALITY := preload("../display_quality.gd")
const ENVIRONMENT := preload("../environment_renderer.gd")
const PROFILE := preload("../environment_profile.gd")
const METRICS := preload("./render_measurement.gd")

class Lease extends RefCounted:
	var references: Array[WeakRef] = []
	func track(value: Object) -> void: references.append(weakref(value))
	func seal() -> void: pass
	func live() -> int: return references.filter(func(ref: WeakRef): return ref.get_ref()!=null).size()

class StillSky extends RefCounted:
	var profile: Dictionary
	var config: Dictionary
	var seconds := 14.0*3600.0
	var wet := 0.0
	var snow := 0.0
	var seed := 10092026
	var elapsed := 0.0
	var previous_weather := "clear"
	var weather := "clear"
	func blend() -> float: return 1.0
	func celestial() -> Dictionary: return PROFILE.celestial(seconds,config)

var scene: Node3D
var camera: Camera3D
var environment: Node3D
var cache: RefCounted
var jobs: Array[Dictionary] = []
var leases: Array[Lease] = []
var references: Array[WeakRef] = []
var clock: Callable = Time.get_ticks_msec
var deadline: int
var report: Dictionary = {}

static func scene_point(point: Array) -> Vector3: return Vector3(point[0],point[1],-point[2])

static func visible_cell(view_camera: Camera3D, x: int, y: int, heights: Array) -> bool:
	if Vector2(x*32+16,y*32+16).distance_to(Vector2(view_camera.position.x,-view_camera.position.z))<48: return true
	var bounds := AABB(Vector3(x*32-18,float(heights[0]),-(y+1)*32-18),Vector3(68,float(heights[1])-float(heights[0]),68))
	for plane: Plane in view_camera.get_frustum():
		var all_outside := true
		for corner in 8:
			if not plane.is_point_over(bounds.get_endpoint(corner)):
				all_outside=false;break
		if all_outside: return false
	return true

static func default_lighting(parent: Node3D) -> Node3D:
	var atmosphere := WorldEnvironment.new();atmosphere.environment=Environment.new();parent.add_child(atmosphere)
	var sun := DirectionalLight3D.new();parent.add_child(sun)
	var renderer := ENVIRONMENT.new();parent.add_child(renderer)
	renderer.configure(atmosphere.environment,sun,false)
	return renderer

func expired() -> bool: return int(clock.call()) >= deadline

func prepare(tree: SceneTree, bridge: RefCounted, meta: Dictionary, doc: Dictionary, view: Dictionary,
		resources: RefCounted, lighting: Callable, timeout_ms: int, detail_radius := 128.0, prefetch := 16.0) -> Dictionary:
	var started: int = clock.call();deadline=started+timeout_ms
	cache=resources
	scene=Node3D.new();tree.root.add_child(scene);references.append(weakref(scene))
	environment=lighting.call(scene)
	if environment == null: return {"ok":false,"error":"lighting admission failed"}
	environment.set_display_distance(float(view.view_distance_m))
	environment.apply_display_quality(QUALITY.active())
	camera=Camera3D.new();scene.add_child(camera);camera.current=true;camera.fov=68
	camera.near=.1;camera.far=float(view.view_distance_m)+80
	if view.get("projection","")=="orthogonal":
		camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=float(view.size_m)
	camera.position=scene_point(view.position_m);camera.look_at(scene_point(view.target_m))
	var state := StillSky.new();state.profile=doc.environment;state.config=doc.environment.duplicate(true)
	state.config.merge({"intensity":.3,"aurora_probability":0.0,"celestial_mode":"simple"})
	state.seconds=float(view.get("time_minutes",doc.environment.start_minutes))*60
	cache.environment_profile=state.profile;cache.quality_profile=QUALITY.active()
	var window: Dictionary=JSON.parse_string(bridge.cell_window(roundi(camera.position.x*100),roundi(-camera.position.z*100)))
	if not window.get("ok",false): return window
	var margin := float(window.data.visual_margin_cm)*.01
	var cell_times: Array=[]
	report={"near_cells":0,"far_cells":0,"near_collision_triangles":0,"far_display_triangles":0,"largest_far_cell_bytes":0}
	for y in ceili(float(meta.size[1])/32.0):
		for x in ceili(float(meta.size[0])/32.0):
			if expired(): return {"ok":false,"error":"preparation timeout"}
			var distance := Vector2(x*32+16,y*32+16).distance_to(Vector2(camera.position.x,-camera.position.z))
			if distance-32*.707107>float(view.view_distance_m): continue
			if not visible_cell(camera,x,y,meta.get("display_height_range_m",[-6,80])): continue
			var before := Time.get_ticks_usec()
			var result := _cell(bridge,x,y,not view.get("force_distant",false) and maxf(0,distance-32*.707107-margin)<=detail_radius+prefetch)
			if not result.get("ok",false): return result
			cell_times.append(float(Time.get_ticks_usec()-before)/1000.0)
			if jobs.size()%40==0: await tree.process_frame
	if expired(): return {"ok":false,"error":"preparation timeout"}
	if jobs.is_empty(): return {"ok":false,"error":"no visible cells"}
	environment.update_environment(state,camera.position,[],cache,true)
	environment.update_dynamic_lights(camera.position,[])
	cell_times.sort()
	report.merge({"preparation_ms":int(clock.call())-started,"cell_prepare_p95_ms":METRICS.percentile(cell_times,.95),
		"cell_prepare_p99_ms":METRICS.percentile(cell_times,.99),"shared_cache_bytes":cache.bytes(),
		"cache":cache.diagnostics(),"cache_limit_bytes":cache.limit_bytes,"attachment":RENDER.diagnostics()})
	return {"ok":true}

func _cell(bridge: RefCounted, x: int, y: int, near: bool) -> Dictionary:
	var job: Dictionary
	var lease := Lease.new();leases.append(lease)
	if near:
		var generated: Dictionary=bridge.generate_chunk_packed(x,y)
		if not generated.get("ok",false): return generated
		var decorated: Dictionary=bridge.with_presentation(generated.data)
		if not decorated.get("ok",false): return decorated
		var data: Dictionary=decorated.data.chunk
		var render_view := RENDER.DATA.view(data)
		report.near_collision_triangles+=RENDER.DATA.count(render_view)
		data.render_batches=RENDER.PLAN.prepare_batches(render_view)
		# The observer lease detects resource release without imposing a second
		# consumer budget. Shared cache admission remains consumer-owned.
		job=RENDER.begin(data,scene,func(_bytes): return lease,1+RENDER.PLAN.shared_bytes(render_view),cache)
		report.near_cells+=1
	else:
		var generated: Dictionary=bridge.generate_far_chunk(x,y)
		if not generated.get("ok",false): return generated
		job=FAR.begin(generated.data,scene,lease,cache);job["distant"]=true
		report.far_cells+=1;report.far_display_triangles+=int(generated.data.triangles)
		report.largest_far_cell_bytes=maxi(report.largest_far_cell_bytes,int(generated.data.retained_bytes)+int(generated.data.display_bytes))
	jobs.append(job)
	var renderer: GDScript=RENDER if near else FAR
	while not renderer.advance(job):
		if expired(): return {"ok":false,"error":"preparation timeout"}
	if job.get("cancelled",false) or not job.get("error",{}).is_empty(): return {"ok":false,"error":job.get("error","render cancelled")}
	job.root.visible=true
	return {"ok":true}

func release(keep_cache := false) -> void:
	for job: Dictionary in jobs:
		if job.get("distant",false): FAR.cancel(job)
		else: RENDER.cancel(job)
	jobs.clear()
	if is_instance_valid(scene): scene.queue_free()
	if cache!=null and not keep_cache: cache.shutdown()
	scene=null;camera=null;environment=null;cache=null

func live_resources() -> int:
	var count := 0
	for lease: Lease in leases: count+=lease.live()
	for ref: WeakRef in references:
		if ref.get_ref()!=null: count+=1
	return count
