extends SceneTree
## Consumer adapters override configuration; public tools never depend on Client.
const SCENE := preload("./fixed_world_scene.gd")
const METRICS := preload("./render_measurement.gd")
const QUALITY := preload("../display_quality.gd")
var failure := ""

func _initialize() -> void: run.call_deferred()
func defaults() -> Dictionary:
	return {"scope":"editor_preview","width":1440,"height":900,"quality":-1,"warmup":12,"frames":120,"repeats":1,"capture":true,"first_views":false}
func configure_window(dimensions: Vector2i) -> bool:
	root.size=dimensions
	return true
func lighting(parent: Node3D) -> Node3D: return SCENE.default_lighting(parent)
func new_cache() -> RefCounted: return SCENE.CACHE.new()
func detail_radius() -> float: return 128.0
func detail_prefetch() -> float: return 16.0
func consumer_memory() -> Dictionary: return {}

static func integer_option(options: Dictionary, name: String, key: String, minimum: int, maximum: int) -> String:
	var value := OS.get_environment("DEFAULT_WORLD_"+name)
	if value.is_empty(): return ""
	if not value.is_valid_int() or int(value)<minimum or int(value)>maximum: return "invalid DEFAULT_WORLD_"+name
	options[key]=int(value)
	return ""

func options() -> Dictionary:
	var value := defaults()
	value.merge({"source":OS.get_environment("DEFAULT_WORLD_SOURCE"),"output":OS.get_environment("DEFAULT_WORLD_OUTPUT"),
		"theme":OS.get_environment("DEFAULT_WORLD_THEME"),"wanted":OS.get_environment("DEFAULT_WORLD_VIEWS").split(",",false),
		"revisit":false,"lod_comparison":false,"occlusion":-1,"timeout_ms":120000})
	if value.theme.is_empty(): value.theme="village"
	for entry in [["WIDTH","width",64,7680],["HEIGHT","height",64,4320],["QUALITY","quality",0,2],
		["REPEATS","repeats",1,10],["WARMUP","warmup",1,600],["FRAMES","frames",1,3600],
		["OCCLUSION","occlusion",0,1],["TIMEOUT_MS","timeout_ms",1,600000]]:
		var error := integer_option(value,entry[0],entry[1],entry[2],entry[3])
		if not error.is_empty(): return {"error":error}
	for entry in [["CAPTURE","capture"],["REVISIT","revisit"],["LOD_COMPARISON","lod_comparison"]]:
		var setting := OS.get_environment("DEFAULT_WORLD_"+entry[0])
		if setting.is_empty(): continue
		if setting not in ["0","1"]: return {"error":"invalid DEFAULT_WORLD_"+entry[0]}
		value[entry[1]]=setting=="1"
	if value.source.is_empty() or value.output.is_empty(): return {"error":"DEFAULT_WORLD_SOURCE and DEFAULT_WORLD_OUTPUT are required"}
	if not value.source.is_absolute_path() or not value.output.is_absolute_path(): return {"error":"source/output must be absolute paths"}
	if value.theme not in ["village","neon-harbor","deep-forest","red-canyon","snow-mountain","machine-factory","sky-park"]: return {"error":"unknown theme"}
	return value

static func selected_views(meta: Dictionary, config: Dictionary) -> Array:
	var views: Array=meta.get("review_views",[]).duplicate(true)
	if not config.wanted.is_empty():
		for name: String in config.wanted:
			if not views.any(func(v): return v.name==name): return []
		views=views.filter(func(v): return v.name in config.wanted)
	elif config.first_views:
		views=views.filter(func(v): return v.get("projection","")!="orthogonal").slice(0,1)
	if config.lod_comparison and not views.is_empty():
		var dense: Dictionary=views[0]
		for view: Dictionary in views:
			if view.name=="day-street": dense=view
		views=[dense,dense.duplicate(true)];views[1].name+="-authored-far-comparison";views[1].force_distant=true
	if config.revisit:
		var repeated: Array=[]
		for view: Dictionary in views:
			repeated.append(view)
			var second:=view.duplicate(true);second.name+="-revisit";repeated.append(second)
		views=repeated
	for view: Dictionary in views:
		if config.quality>=0: view.quality=config.quality
	return views

func run() -> void:
	var config := options()
	if config.has("error"): finish_error(config.error);return
	if DisplayServer.get_name()=="headless": finish_error("fixed scene measurements require --rendered");return
	# Godot consumes engine CLI options before exposing script arguments. The
	# launcher must reject --fixed-fps; movie mode has a queryable engine state.
	if not Engine.get_write_movie_path().is_empty():
		finish_error("movie timing cannot measure wall-clock performance");return
	if not configure_window(Vector2i(config.width,config.height)): finish_error("window admission failed");return
	# Let Client window owners apply their internal cap before scene construction.
	for frame in 2: await process_frame
	var report_path: String=config.output.path_join(config.theme+"-render-report.json")
	if FileAccess.file_exists(report_path): finish_error("report already exists: "+report_path);return
	if DirAccess.make_dir_recursive_absolute(config.output)!=OK: finish_error("output directory unavailable");return
	var meta := read_json(config.source.path_join(config.theme+"/region.json"))
	var doc := read_json(config.source.path_join(config.theme+"/document.json"))
	if meta.is_empty() or not doc.has("environment") or not meta.has("size"):
		finish_error("missing or invalid world review metadata");return
	var views := selected_views(meta,config)
	if views.is_empty(): finish_error("no matching review views");return
	var path: String=config.source.path_join(config.theme+".memap")
	if not FileAccess.file_exists(path): finish_error("package does not exist: "+path);return
	var bridge: RefCounted=ClassDB.instantiate("MapKitBridge")
	var opened: Dictionary=JSON.parse_string(bridge.open_package(path))
	if not opened.get("ok",false): finish_error("package open failed: "+str(opened));return
	var hash_value := FileAccess.get_sha256(path)
	var results: Array=[]
	for repeat_index in config.repeats:
		var shared_cache: RefCounted
		var retired_scenes: Array=[]
		for view: Dictionary in views:
			var quality := QUALITY.profile(int(view.quality))
			if config.occlusion>=0: quality.occlusion=config.occlusion==1
			QUALITY.apply(root,quality)
			var cache: RefCounted=shared_cache if shared_cache!=null else new_cache()
			if config.revisit: shared_cache=cache
			var scene := SCENE.new()
			var baseline := METRICS.memory()
			var prepared: Dictionary=await scene.prepare(self,bridge,meta,doc,view,cache,lighting,config.timeout_ms,detail_radius(),detail_prefetch())
			var report: Dictionary={"schema_version":1,"scope":config.scope,"package_sha256":hash_value,
				"view":view,"repeat":repeat_index+1,"quality":quality,"environment":METRICS.environment(root),
				"device":RenderingServer.get_video_adapter_name(),"resolution":METRICS.vec(Vector2i(root.get_visible_rect().size)),
				"cache_policy":"revisit" if config.revisit else "cold_scene","memory_before":baseline,
				"preparation":"ready" if prepared.get("ok",false) else "failed","image":null}
			if prepared.get("ok",false):
				report.merge(scene.report)
				var measured := await METRICS.sample(self,root,config.warmup,config.frames)
				report.merge(measured)
				report["frame_p95_ms"]=measured.frame_interval.p95_ms;report["frame_p99_ms"]=measured.frame_interval.p99_ms
				report["render"]=METRICS.render(root)
				report.merge(METRICS.memory());report["consumer_memory"]=consumer_memory()
				if config.capture:
					# GPU readback and PNG encoding happen strictly after the sample.
					await RenderingServer.frame_post_draw
					var suffix := "" if config.repeats==1 else "-%d"%(repeat_index+1)
					var image_path: String=config.output.path_join(config.theme+"-"+view.name+suffix+".png")
					if FileAccess.file_exists(image_path) or root.get_texture().get_image().save_png(image_path)!=OK: failure="capture failed or destination exists"
					else: report.image=image_path
			else:
				failure=str(prepared);report["error"]=prepared;report["assessment"]={"fixed_scene":"unavailable","driving":"not_measured"}
			scene.release(config.revisit and failure.is_empty())
			if config.revisit: retired_scenes.append(scene)
			# No early credit: settle queue_free and engine retirement before sampling.
			for frame in 8:
				await process_frame
				cache.bytes();consumer_memory()
			report["release"]={"live_scene_resources":scene.live_resources(),"cache_bytes":cache.bytes(),
				"cache_retained_for_revisit":config.revisit and failure.is_empty(),"memory":METRICS.memory(),"consumer_memory":consumer_memory()}
			if not config.revisit and (scene.live_resources()!=0 or cache.bytes()!=0): failure="resources retained after scene/cache release"
			results.append(report)
			print("REVIEW_CAPTURE ",JSON.stringify(report))
			if not failure.is_empty(): break
		if shared_cache!=null:
			var shutdown_started := Time.get_ticks_usec()
			shared_cache.shutdown()
			var shutdown_usec := Time.get_ticks_usec()-shutdown_started
			for frame in 8:
				await process_frame
				shared_cache.bytes();consumer_memory()
			results.back()["revisit_release"]={"cache_bytes":shared_cache.bytes(),"consumer_memory":consumer_memory(),"shutdown_usec":shutdown_usec}
			if shared_cache.bytes()!=0: failure="revisit cache retained after shutdown"
			for scene: RefCounted in retired_scenes:
				if scene.live_resources()!=0: failure="scene resources retained after revisit shutdown"
		if not failure.is_empty(): break
	var report_file := FileAccess.open(report_path,FileAccess.WRITE)
	if report_file==null: finish_error("report write failed");return
	report_file.store_string(JSON.stringify(results,"\t"));report_file.close()
	RenderingServer.viewport_set_measure_render_time(root.get_viewport_rid(),false)
	if not failure.is_empty(): finish_error(failure);return
	print("render_default_world: PASS (measurement completed; see each fixed_scene verdict; driving not measured)")
	quit(0)

static func read_json(path: String) -> Dictionary:
	if not FileAccess.file_exists(path): return {}
	var value: Variant=JSON.parse_string(FileAccess.get_file_as_string(path))
	return value if value is Dictionary else {}

func finish_error(message: String) -> void:
	push_error("render_default_world: "+message)
	quit(1)
