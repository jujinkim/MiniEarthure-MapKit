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
var preparation_stages: Dictionary = {}

func record_cost(stage: String, elapsed_usec: int, context: Dictionary) -> void:
	if not preparation_stages.has(stage):
		preparation_stages[stage]={"samples":0,"total_usec":0,"peak_usec":0,"peak_context":{}}
	var value: Dictionary=preparation_stages[stage]
	value.samples+=1;value.total_usec+=elapsed_usec
	if value.samples==1 or elapsed_usec>int(value.peak_usec):
		value.peak_usec=elapsed_usec;value.peak_context=context.duplicate(true)

func record_cache(before: Dictionary, context: Dictionary) -> void:
	# Every advance admits at most one shared preparation operation. Read its
	# existing inner timer; lifetime peaks cannot be subtracted across revisits.
	for phase: String in cache.preparation_usec:
		var elapsed := int(cache.preparation_usec[phase])-int(before.get(phase,0))
		if elapsed>0: record_cost("cache/"+phase,elapsed,context)

func record_mesh_cost(phase: String, elapsed_usec: int, surface: int, context: Dictionary) -> void:
	var detail := context.duplicate(true)
	if surface>=0: detail.surface=surface
	record_cost("mesh/"+phase,elapsed_usec,detail)

static func template_context(job: Dictionary, cell: Dictionary) -> Dictionary:
	var context := cell.duplicate(true)
	var objects: Array=job.get("chunk",{}).get("objects",[])
	var index := int(job.get("object",0))
	if index>=objects.size(): return context
	var id := str(objects[index].asset_id)
	context.asset_id=id
	var resources: RefCounted=job.get("resources")
	if resources==null: return context
	var item: Dictionary=resources.entries.get(job.get("claims",{}).get(id,""),{})
	var node: Node=item.get("styling")
	if node==null and not item.get("pending",[]).is_empty(): node=item.pending.back()
	if node is MeshInstance3D:
		context.mesh=node.name;context.surfaces=node.mesh.get_surface_count()
		if item.has("lod_job") and item.lod_job.stage=="surface": context.surface=item.lod_job.surface
		elif item.stage=="material_surface": context.surface=item.material_surface
	return context

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
	preparation_stages={}
	var cache_before := {"imports":cache.imports,"hits":cache.hits}
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
	var environment_started := Time.get_ticks_usec()
	environment.update_environment(state,camera.position,[],cache,true)
	environment.update_dynamic_lights(camera.position,[])
	record_cost("environment_update",Time.get_ticks_usec()-environment_started,{})
	cell_times.sort()
	report.merge({"preparation_ms":int(clock.call())-started,"cell_prepare_p95_ms":METRICS.percentile(cell_times,.95),
		"cell_prepare_p99_ms":METRICS.percentile(cell_times,.99),"shared_cache_bytes":cache.bytes(),
		"cache":cache.diagnostics(),"cache_limit_bytes":cache.limit_bytes,"attachment":RENDER.diagnostics(),
		"preparation_stages":preparation_stages,"cache_activity":{"imports":cache.imports-cache_before.imports,"hits":cache.hits-cache_before.hits},
		"timing_scopes":{"preparation_stages":"this_view","cache":"cache_lifetime","attachment":"process_lifetime"}})
	return {"ok":true}

func _cell(bridge: RefCounted, x: int, y: int, near: bool) -> Dictionary:
	var job: Dictionary
	var lease := Lease.new();leases.append(lease)
	var context := {"cell":[x,y],"representation":"near" if near else "far"}
	var before := Time.get_ticks_usec()
	if near:
		var generated: Dictionary=bridge.generate_chunk_packed(x,y)
		record_cost("near/generation",Time.get_ticks_usec()-before,context)
		if not generated.get("ok",false): return generated
		before=Time.get_ticks_usec()
		var decorated: Dictionary=bridge.with_presentation(generated.data)
		record_cost("near/presentation",Time.get_ticks_usec()-before,context)
		if not decorated.get("ok",false): return decorated
		var data: Dictionary=decorated.data.chunk
		var render_view := RENDER.DATA.view(data)
		report.near_collision_triangles+=RENDER.DATA.count(render_view)
		before=Time.get_ticks_usec()
		data.render_batches=RENDER.PLAN.prepare_batches(render_view)
		record_cost("near/batch_preparation",Time.get_ticks_usec()-before,context)
		# The observer lease detects resource release without imposing a second
		# consumer budget. Shared cache admission remains consumer-owned.
		before=Time.get_ticks_usec()
		job=RENDER.begin(data,scene,func(_bytes): return lease,1+RENDER.PLAN.shared_bytes(render_view),cache)
		record_cost("near/begin",Time.get_ticks_usec()-before,context)
		report.near_cells+=1
	else:
		var generated: Dictionary=bridge.generate_far_chunk(x,y)
		record_cost("far/generation",Time.get_ticks_usec()-before,context)
		if not generated.get("ok",false): return generated
		before=Time.get_ticks_usec()
		job=FAR.begin(generated.data,scene,lease,cache);job["distant"]=true
		record_cost("far/begin",Time.get_ticks_usec()-before,context)
		report.far_cells+=1;report.far_display_triangles+=int(generated.data.triangles)
		report.largest_far_cell_bytes=maxi(report.largest_far_cell_bytes,int(generated.data.retained_bytes)+int(generated.data.display_bytes))
	jobs.append(job)
	var renderer: GDScript=RENDER if near else FAR
	while true:
		var cache_before: Dictionary=cache.preparation_usec.duplicate()
		var detail := template_context(job,context) if near else context
		cache.mesh_observer=record_mesh_cost.bind(detail)
		before=Time.get_ticks_usec()
		var done: bool=renderer.advance(job)
		var elapsed := Time.get_ticks_usec()-before
		cache.mesh_observer=Callable()
		var phase := "near/"+str(job.last_phase) if near else "far/advance"
		record_cost(phase,elapsed,detail if phase=="near/template_preparation" else context)
		record_cache(cache_before,detail)
		if done: break
		if expired(): return {"ok":false,"error":"preparation timeout"}
	if job.get("cancelled",false) or not job.get("error",{}).is_empty(): return {"ok":false,"error":job.get("error","render cancelled")}
	job.root.visible=true
	return {"ok":true}

func release(keep_cache := false) -> void:
	for job: Dictionary in jobs:
		var before := Time.get_ticks_usec()
		if job.get("distant",false): FAR.cancel(job)
		else: RENDER.cancel(job)
		record_cost("release/far" if job.get("distant",false) else "release/near",Time.get_ticks_usec()-before,{})
	jobs.clear()
	var shutdown_started := Time.get_ticks_usec()
	if is_instance_valid(scene): scene.queue_free()
	if cache!=null and not keep_cache: cache.shutdown()
	record_cost("release/scene_cache",Time.get_ticks_usec()-shutdown_started,{"keep_cache":keep_cache})
	scene=null;camera=null;environment=null;cache=null

func live_resources() -> int:
	var count := 0
	for lease: Lease in leases: count+=lease.live()
	for ref: WeakRef in references:
		if ref.get_ref()!=null: count+=1
	return count
