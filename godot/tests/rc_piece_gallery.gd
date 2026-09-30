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
		var source: Dictionary=JSON.parse_string(native.track_shortcut_source()).data
		source.original_seed=null
		for key in ["connections","paths","checkpoints","actions","attachments"]: source[key]=[]
		source.instances=[{"id":"gallery","preset":kind,"position_cm":[0,0,0],"rotation_mdeg":[0,0,0],"width_cm":400,"entry_width_cm":400,"exit_width_cm":400,"control_points":[]}]
		var generated: Dictionary=JSON.parse_string(native.compile_track_source(JSON.stringify(source)))
		check(generated.ok,"explicit gallery source "+kind)
		if not generated.ok: continue
		var piece: Dictionary=generated.data.document.assembled_track.pieces[0]
		var world: Node3D=preload("../track_authoring_preview.gd").create(generated.data.document)
		root.add_child(world)
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
