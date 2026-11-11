extends SceneTree
const APPEARANCE := preload("../regional_appearance.gd")
const RENDER := preload("../chunk_renderer.gd")
const DATA := preload("../chunk_data.gd")
const PLAN := preload("../render_memory.gd")
const CACHE := preload("../render_resource_cache.gd")
const QUALITY := preload("../display_quality.gd")
const PROFILE := preload("../environment_profile.gd")
var failed := false
class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(ref): return ref.get_ref() == null)
func check(value: bool, label: String) -> void:
	if not value: failed = true; push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	clipped_faces()
	for size in [128,256,512]:
		var cache := CACHE.new(func(n): return Lease.new(n))
		cache.environment_profile = PROFILE.defaults()
		cache.quality_profile = QUALITY.profile(1); cache.quality_profile.texture_size = size
		check(cache.require_environment_tiles(["sand","pavers"]),"select initial tiles")
		var context: RefCounted = cache.environment_context()
		check(context.tiles.size() == 2 and context.tiles.sand.get_width() == size,"only selected texture tier loads")
		var material: ShaderMaterial = context.generated_material(Color.WHITE,"sand")
		var old: Texture2D = material.get_shader_parameter("detail_tile")
		check(cache.require_environment_tiles(["ice"]),"admit added material")
		check(cache.material_context() == context,"pending neighbour preserves existing context")
		while not cache.prepare_environment(): pass
		check(context.tiles.size() == 3 and material.get_shader_parameter("detail_tile") == old,"append does not replace borrowed tiles")
		var before: int = cache.bytes(); cache.limit_bytes = before
		check(not cache.require_environment_tiles(["rock"]) and not "rock" in context.tile_kinds and cache.bytes() == before,"failed admission preserves old results")
		context = null; old = null
		cache.shutdown()
		check(cache.bytes()>0,"borrowed tile keeps its charge")
		material = null
		check(cache.bytes()==0,"last borrower releases all tile leases")
	await render_sets()
	QUALITY.current = {}
	print("regional_appearance_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
func clipped_faces() -> void:
	var doc := document("modern")
	doc.bounds = {"min":[0,0],"max":[6400,6400]}; doc.cell_size_cm = 3200
	doc.surface_areas = []; doc.barriers = []; doc.zones = []
	doc.buildings = [building("corner",3000,3000,"modern","shop",20)]
	doc.buildings[0].footprint = [[3000,3000],[3200,3000],[3200,3200],[3000,3200]]
	var native: RefCounted = ClassDB.instantiate("MapKitBridge")
	for cell in [Vector2i(0,0),Vector2i(1,0),Vector2i(0,1),Vector2i(1,1)]:
		var result: Dictionary = native.source_preview_packed(JSON.stringify(doc),ProjectSettings.globalize_path("res://"),cell.x,cell.y)
		check(result.get("ok",false),"clipped regional corner "+str(cell)+": "+str(result.get("error",{})))
		if not result.get("ok",false): continue
		var display: Dictionary = DATA.view(result.data.chunk).presentation
		check(display.regional_vertices.size() == display.regional_normals.size(),"clipped normal count")
		for normal: Vector3 in display.regional_normals:
			check(normal.is_finite() and normal.is_normalized(),"clipped source-precision normal")
func building(id: String, x: int, y: int, arch: String, kind: String, height := 700) -> Dictionary:
	var material: String = APPEARANCE.CATALOG.data.architectures[arch].material
	return {"id":id,"footprint":[[x,y],[x+600,y],[x+600,y+500],[x,y+500]],"base_cm":0,"height_cm":height,"usage":"residential","material":"concrete","roof":"flat","style_id":"building/%s/%s/%s" % [arch,kind,material]}
func document(arch: String) -> Dictionary:
	var d: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(get_script().resource_path.get_base_dir()+"/../../examples/minimal/document.json"))
	d.bounds = {"min":[0,0],"max":[3200,3200]};d.free_roam = true
	d.nodes = [];d.roads = [];d.zones = [];d.buildings = []
	d.environment = PROFILE.defaults();d.environment.architecture = arch
	d.attributions.append({"source":"Synthetic city fixture","license":"MIT","notice":JSON.stringify({"adapter":"osm-extract-v1","authored_units":{"source_denominator":1}})})
	var set_index: int = APPEARANCE.CATALOG.data.architectures.keys().find(arch)
	d.ground_style_id = "ground/"+APPEARANCE.GROUNDS[set_index%APPEARANCE.GROUNDS.size()]
	if arch == "polar_station": d.ground_style_id = "ground/snow"
	if arch in ["farm","rural"]: d.ground_style_id = "ground/compacted_earth"
	d.buildings = [building("a",400,400,arch,"detached"),building("b",1500,500,arch,"shop",1100),building("c",700,1650,arch,"religious" if arch == "cycladic" else "research" if arch == "polar_station" else "barn" if arch == "farm" else "civic",800)]
	d.buildings[0].style_id = "building/%s/%s/%s" % [arch,APPEARANCE.CATALOG.data.building_types.keys()[set_index],APPEARANCE.CATALOG.data.architectures[arch].material]
	if arch == "chinese_courtyard":
		d.buildings[2].holes = [[[850,1800],[850,2000],[1150,2000],[1150,1800]]]
	if arch == "cycladic": d.buildings[2].style_id += "/dome/blue_tile"
	d.surface_areas = [{"id":"road","polygon":[[100,1200],[3100,1200],[3100,1450],[100,1450]],"surface":"asphalt","style_id":"ground/asphalt"}]
	d.barriers = [{"id":"wall","line":[[300,2500],[2200,2500]],"kind":"wall","height_cm":80,"thickness_cm":25,"style_id":"barrier/quay/stone" if arch == "venetian" else "barrier/wall/brick"}]
	var plant_style := "conifer" if arch in ["nordic_timber","polar_station"] else "palm" if arch in ["tropical","southeast_asian","middle_eastern"] else "orchard" if arch in ["farm","rural","mediterranean"] else "deciduous"
	d.zones = [{"id":"trees","polygon":[[2300,1700],[3150,1700],[3150,3100],[2300,3100]],"kind":"forest","spacing_cm":450,"density_per_mille":1000,"exclusions":[],"tree":{"asset_id":"builtin:city_tree","radius_cm":120,"clearance_cm":30},"style_id":"vegetation/"+plant_style}]
	if arch in ["polar_station","adobe"]:
		d.zones[0].tree = {"asset_id":"builtin:shrub","radius_cm":42,"clearance_cm":30}
		d.zones[0].style_id = "vegetation/polar_scrub" if arch=="polar_station" else "vegetation/scrub"
	return d
func render_sets() -> void:
	var output := OS.get_environment("MAPKIT_REGIONAL_CAPTURE")
	if output.is_empty(): output = OS.get_user_data_dir()+"/regional-captures"
	DirAccess.make_dir_recursive_absolute(output)
	QUALITY.current = QUALITY.profile(1)
	var native: RefCounted = ClassDB.instantiate("MapKitBridge")
	var viewport := SubViewport.new();viewport.size = Vector2i(768,512);viewport.own_world_3d = true
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS;root.add_child(viewport)
	var gallery := Image.create(1536,2016,false,Image.FORMAT_RGBA8)
	var index := 0
	var scenes: Array = APPEARANCE.CATALOG.data.architectures.keys()
	scenes.append_array(["venetian-composition","cycladic-composition","polar_station-composition","farm-composition","modern-composition"])
	for scene_name: String in scenes:
		if not OS.get_environment("MAPKIT_REGIONAL_SCENE").is_empty() and not scene_name in OS.get_environment("MAPKIT_REGIONAL_SCENE").split(","): continue
		var arch := scene_name.trim_suffix("-composition")
		var doc := document(arch)
		if scene_name.ends_with("-composition"): compose(doc,arch)
		var result: Dictionary = native.source_preview_packed(JSON.stringify(doc),ProjectSettings.globalize_path("res://"),0,0)
		check(result.get("ok",false),"native "+arch+": "+str(result.get("error",{})))
		if not result.get("ok",false): continue
		var chunk: Dictionary = DATA.view(result.data.chunk)
		for area: Dictionary in doc.surface_areas:
			check(chunk.presentation.appearance_styles.get(area.id,"") == area.style_id,"surface style remains attached "+area.id)
		check(not chunk.presentation.regional_keys.is_empty(),"regional native geometry "+arch)
		check(chunk.presentation.regional_omitted_modules == 0,"uncapped representative "+arch)
		var cancelled := PLAN.prepare_batches(chunk,func(): return true)
		check(cancelled.is_empty(),"cancel batches "+arch)
		chunk.render_batches = PLAN.prepare_batches(chunk)
		check(chunk.render_batches.any(func(b): return str(b.key).begins_with("building/")),"regional batches "+arch)
		var world := Node3D.new();viewport.add_child(world)
		var atmosphere := WorldEnvironment.new();atmosphere.environment = Environment.new()
		atmosphere.environment.background_mode = Environment.BG_COLOR;atmosphere.environment.background_color = Color("c7d3db")
		atmosphere.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR;atmosphere.environment.ambient_light_color = Color.WHITE;atmosphere.environment.ambient_light_energy = 0.3
		world.add_child(atmosphere)
		var light := DirectionalLight3D.new();world.add_child(light);light.rotation_degrees = Vector3(-50,-30,0);light.shadow_enabled = true;light.light_energy = 0.7
		var camera := Camera3D.new();world.add_child(camera);camera.projection = Camera3D.PROJECTION_ORTHOGONAL;camera.size = 37
		camera.position = Vector3(34,25,20);camera.look_at(Vector3(15,3,-14));camera.current = true
		var label := Label.new();label.text = scene_name;label.position = Vector2(14,10);label.add_theme_font_size_override("font_size",24);viewport.add_child(label)
		var cache := CACHE.new();cache.quality_profile = QUALITY.current
		var job := RENDER.begin(chunk,world,Callable(),PLAN.estimate(chunk,0),cache)
		var steps := 0
		while not RENDER.advance(job) and steps<10000:
			steps += 1
			if steps%32==0: await process_frame
		check(job.error.is_empty() and steps<10000,"render "+arch+": "+str(job.error))
		for frame in 3: await process_frame
		await RenderingServer.frame_post_draw
		var captured := viewport.get_texture().get_image()
		check(captured.save_png(output+"/"+scene_name+".png") == OK,"capture "+arch)
		if OS.get_environment("MAPKIT_REGIONAL_FAR") == "1":
			var source := OS.get_user_data_dir()+"/regional-source/"+scene_name
			DirAccess.make_dir_recursive_absolute(source)
			var validated: Dictionary = JSON.parse_string(native.validate_document(JSON.stringify(doc)))
			check(validated.get("ok",false),"canonical distant source "+scene_name)
			if not validated.get("ok",false): continue
			var file := FileAccess.open(source+"/document.json",FileAccess.WRITE);file.store_string(validated.data.canonical);file.close()
			var opened: Dictionary = JSON.parse_string(native.open_project(source))
			check(opened.get("ok",false),"distant source "+scene_name+str(opened.get("error",{})))
			var far: Dictionary = native.generate_far_chunk(0,0)
			check(far.get("ok",false),"distant product "+scene_name)
			if far.get("ok",false):
				var distant := preload("../distant_renderer.gd")
				var far_job := distant.begin(far.data,world,Lease.new(0),cache)
				while not distant.advance(far_job): await process_frame
				check(far_job.done and not far_job.cancelled,"distant rendering completes "+scene_name)
				job.root.visible=false;far_job.root.visible=true
				for frame in 3: await process_frame
				await RenderingServer.frame_post_draw
				check(viewport.get_texture().get_image().save_png(output+"/"+scene_name+"-far.png")==OK,"distant capture "+scene_name)
				distant.cancel(far_job)
		captured.resize(384,288,Image.INTERPOLATE_LANCZOS)
		gallery.blit_rect(captured,Rect2i(0,0,384,288),Vector2i((index%4)*384,(index/4)*288))
		index += 1
		world.queue_free();label.queue_free();cache.shutdown()
		await process_frame
	check(gallery.save_png(output+"/architecture-gallery.png") == OK,"architecture gallery")
	viewport.queue_free()
	await process_frame

func compose(d: Dictionary, arch: String) -> void:
	if arch == "venetian":
		d.ground_style_id = "ground/stone_paving"
		d.surface_areas = []
		d.water_bodies = [{"id":"canal","polygon":[[0,1150],[3200,1150],[3200,1500],[0,1500]],"surface_cm":-20,"bottom_cm":-220,"flow_cm_s":[20,0]}]
		d.nodes = [{"id":"south","position":[2600,70,950],"level":1},{"id":"north","position":[2600,70,1700],"level":1}]
		d.roads = [{"id":"footbridge","from":"south","to":"north","points":[[2600,70,950],[2600,70,1700]],"widths_cm":[150],"surfaces":["concrete"],"kind":"bridge","sidewalk_cm":0,"style_id":"road/bridge/stone_paving"}]
		d.barriers = [{"id":"quay","line":[[100,1120],[2400,1120]],"kind":"wall","height_cm":65,"thickness_cm":25,"style_id":"barrier/quay/stone"}]
	elif arch == "cycladic":
		d.ground_style_id = "ground/stone_paving"
		d.buildings[1].base_cm = 100;d.buildings[2].base_cm = 200
		d.buildings[0].style_id += "/vault/whitewash"
		for index in [1,2]:
			var terrace: Dictionary = d.buildings[index].duplicate(true)
			terrace.id = "terrace-%d" % index;terrace.height_cm = terrace.base_cm;terrace.base_cm = 0
			terrace.style_id = "building/cycladic/utility/whitewash/flat/whitewash"
			d.buildings.append(terrace)
		d.surface_areas[0].style_id = "ground/stone_paving"
	elif arch == "polar_station":
		d.buildings[0].style_id = "building/polar_station/utility/metal"
		d.surface_areas[0].style_id = "ground/ice"
	elif arch == "farm":
		d.buildings[0].style_id = "building/farm/farmhouse/wood"
		d.surface_areas[0].style_id = "ground/compacted_earth"
		d.surface_areas.append({"id":"field","polygon":[[2300,1700],[3100,1700],[3100,2900],[2300,2900]],"surface":"dirt","style_id":"ground/cropland"})
	else:
		d.buildings[0].style_id = "building/modern/school/brick"
		d.buildings[1].style_id = "building/modern/hospital/concrete"
		d.buildings[2].style_id = "building/modern/office/glass"
