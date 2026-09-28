extends SceneTree
## Actual generated geometry and shared materials; scale car is 0.3375 x 0.75m.
const RENDER := preload("../chunk_renderer.gd")
const STAGE := preload("../track_stage.gd")
const GEOMETRY := preload("../gimmick_geometry.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true;push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	root.size=Vector2i(1200,900)
	for kind in ["banked_chicane","overpass","roller_waves","offset_jump"]:
		if OS.has_environment("RC_PIECE_KINDS") and kind not in OS.get_environment("RC_PIECE_KINDS").split(","): continue
		var native: RefCounted = ClassDB.instantiate("MapKitBridge")
		var settings: Dictionary = JSON.parse_string(native.track_catalogue()).data.defaults
		settings.duration_seconds=120;settings.difficulty="hard";settings.gimmicks=[kind]
		var generated := {};var index := -1;var file := ""
		for seed in range(1,9):
			settings.seed=seed;file=ProjectSettings.globalize_path("user://%s-%d.memap" % [kind,seed])
			generated=JSON.parse_string(native.generate_track(JSON.stringify(settings),file))
			if not generated.ok: break
			for i in generated.data.document.assembled_track.pieces.size():
				if generated.data.document.assembled_track.pieces[i].id==kind: index=i;break
			if index>=0: break
		check(generated.get("ok",false) and index>=0,"piece fixture "+kind)
		if index<0: continue
		check(JSON.parse_string(native.open_package(file)).ok,"piece opens")
		var assembly: Dictionary = generated.data.document.assembled_track
		var piece: Dictionary = assembly.pieces[index]
		var world := Node3D.new();root.add_child(world)
		world.add_child(STAGE.create(native.map_bounds(),assembly))
		var window: Dictionary = JSON.parse_string(native.cell_window(piece.origin_cm[0],piece.origin_cm[2]))
		for cell: Dictionary in window.data.cells:
			var packed: Dictionary = native.generate_chunk_packed(cell.x,cell.y)
			check(packed.ok,"piece geometry")
			var presentation: Dictionary = native.with_presentation(packed.data)
			check(presentation.ok,"piece presentation")
			var job := RENDER.begin(presentation.data.chunk,world)
			while not RENDER.advance(job): pass
			check(job.error.is_empty(),"piece rendering")
		for gimmick: Dictionary in generated.data.document.get("gimmicks",[]):
			if not gimmick.id.begins_with("track-%d-" % index): continue
			var visual := GEOMETRY.visual(gimmick);world.add_child(visual);visual.transform=GEOMETRY.pose(gimmick,350)
		var car := Node3D.new();world.add_child(car)
		car.position=GEOMETRY.point(piece.origin_cm)+Vector3.UP*0.15
		car.rotation.y=-float(piece.quarter_turns)*PI/2.0
		STAGE.box(car,Vector3.ZERO,Vector3(0.3375,0.13125,0.75),STAGE.paint(Color("f4e33d")))
		STAGE.box(car,Vector3(0,0.11,0),Vector3(0.25,0.1,0.32),STAGE.paint(Color("15202b")))
		var origin := GEOMETRY.point(piece.origin_cm)
		var basis := Basis(Vector3.UP,-float(piece.quarter_turns)*PI/2.0)
		var camera := Camera3D.new();world.add_child(camera);camera.current=true
		camera.position=origin+basis*Vector3(23,25,10);camera.look_at(origin+basis*Vector3(0,0,-16))
		camera.fov=60
		var light := DirectionalLight3D.new();world.add_child(light);light.rotation_degrees=Vector3(-65,-25,0)
		var env := WorldEnvironment.new();env.environment=Environment.new();world.add_child(env)
		env.environment.background_mode=Environment.BG_COLOR;env.environment.background_color=Color("b6c9d8")
		env.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR;env.environment.ambient_light_color=Color.WHITE;env.environment.ambient_light_energy=0.6
		var title := Label.new();root.add_child(title);title.position=Vector2(28,24);title.add_theme_font_size_override("font_size",26);title.text=kind+"  |  scale car: 0.3375 x 0.75 m"
		for _i in 3: await process_frame
		if DisplayServer.get_name()!="headless":
			await RenderingServer.frame_post_draw
			var output := OS.get_environment("RC_PIECE_CAPTURE")
			if not output.is_empty(): check(root.get_texture().get_image().save_png(output.path_join(kind+".png"))==OK,"piece capture")
		title.free();world.free();await process_frame
	print("rc_piece_gallery: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
