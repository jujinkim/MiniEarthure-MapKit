extends SceneTree
const RENDER := preload("../chunk_renderer.gd")
const PLAN := preload("../render_memory.gd")
const DATA := preload("../chunk_data.gd")
const STAGE := preload("../track_stage.gd")
const VENUE := preload("../rc_venue.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true;push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	root.size=Vector2i(1440,900)
	for seed in 3:
		var native: RefCounted = ClassDB.instantiate("MapKitBridge")
		var settings: Dictionary = JSON.parse_string(native.track_catalogue()).data.defaults
		settings.seed=seed;settings.duration_seconds=30;settings.gimmicks=[]
		var file := ProjectSettings.globalize_path("user://venue-%d.memap" % seed)
		var generated: Dictionary = JSON.parse_string(native.generate_track(JSON.stringify(settings),file))
		check(generated.ok,"generated visual fixture")
		if not generated.ok: quit(1);return
		check(JSON.parse_string(native.open_package(file)).ok,"visual fixture load")
		var assembly: Dictionary = generated.data.document.assembled_track
		var world := Node3D.new();root.add_child(world)
		var stage := STAGE.create(native.map_bounds(),assembly);world.add_child(stage)
		check(stage.get_meta("rc_venue_theme")==seed,"seeded venue style")
		var pending: Array[Node] = [stage];var nodes := 0
		while not pending.is_empty():
			var node: Node = pending.pop_back();nodes+=1
			check(not node is CollisionObject3D,"stage remains display-only")
			pending.append_array(node.get_children())
		check(nodes<180,"bounded stage nodes")
		var cells := {}
		for piece: Dictionary in assembly.pieces:
			for sample: Dictionary in piece.path:
				var pos: Array = sample.position_cm
				var window: Dictionary = JSON.parse_string(native.cell_window(pos[0],pos[2]))
				for cell: Dictionary in window.data.cells: cells[Vector2i(cell.x,cell.y)]=true
		var saw_road := false
		for cell: Vector2i in cells:
			var packed: Dictionary = native.generate_chunk_packed(cell.x,cell.y)
			check(packed.ok,"native fixture geometry")
			var presentation: Dictionary = native.with_presentation(packed.data)
			check(presentation.ok,"native presentation")
			if not presentation.ok: continue
			var chunk: Dictionary = presentation.data.chunk
			check(int(chunk.presentation.track_seed)==seed,"native seed reaches common renderer")
			var view := DATA.view(chunk)
			for i in DATA.count(view):
				if DATA.object_id(view,i).begins_with("assembled-road-"):
					saw_road=true
					check(PLAN.material_key(view,i).begins_with("rc:road:"),"road uses RC material")
			var job := RENDER.begin(chunk,world)
			while not RENDER.advance(job): pass
			check(job.error.is_empty(),"RC render completes")
		check(saw_road,"visible racing surface")
		if DisplayServer.get_name()!="headless":
			var environment := WorldEnvironment.new();environment.environment=Environment.new()
			environment.environment.background_mode=Environment.BG_COLOR
			environment.environment.background_color=Color("bdcfdf")
			environment.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR
			environment.environment.ambient_light_color=Color.WHITE;environment.environment.ambient_light_energy=0.65
			world.add_child(environment)
			var light := DirectionalLight3D.new();world.add_child(light);light.rotation_degrees=Vector3(-65,-25,0)
			var camera := Camera3D.new();world.add_child(camera);camera.current=true
			camera.position=Vector3(20,20,12);camera.look_at(Vector3(0,0,-16))
			camera.fov=60
			for _i in 4: await process_frame
			await RenderingServer.frame_post_draw
			var capture := OS.get_environment("RC_VENUE_CAPTURE")
			if not capture.is_empty(): root.get_texture().get_image().save_png(capture.path_join("venue-%d.png" % seed))
		world.free();await process_frame
	print("rc_venue_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
