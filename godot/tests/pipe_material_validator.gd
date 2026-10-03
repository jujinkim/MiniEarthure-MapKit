extends SceneTree
const GEOMETRY := preload("../gimmick_geometry.gd")
const PREVIEW := preload("../track_authoring_preview.gd")
var failures: Array[String] = []
func _initialize() -> void: run.call_deferred()
func check(value: bool, label: String) -> void:
	if not value: failures.append(label); push_error(label)
func verify_material(material: StandardMaterial3D, color: Color) -> void:
	check(material.albedo_color.is_equal_approx(color), "stored pipe colour")
	check(is_equal_approx(material.roughness, 0.82) and is_equal_approx(material.metallic, 0.65), "matte metal properties")
	check(not material.emission_enabled, "non-emissive pipe")
func run() -> void:
	var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
	var source := {"original_seed":null,"grounded_supports":false,"settings":{"seed":101,"circuit":false,"duration_seconds":60,"difficulty":"easy","categories":["driving"],"time_minutes":720},"instances":[],"connections":[],"paths":[],"checkpoints":[],"actions":[],"attachments":[]}
	for row: Array in [["pipe", "cylinder_curve", 400], ["road", "straight", 400], ["loop", "loop", 400], ["halfpipe", "banked_chicane", 400]]:
		source.instances.append({"id":row[0],"preset":row[1],"position_cm":[source.instances.size()*4000,0,0],"rotation_mdeg":[0,0,0],"width_cm":row[2],"entry_width_cm":400,"exit_width_cm":400,"control_points":[]})
	var result: Dictionary = JSON.parse_string(bridge.compile_track_source(JSON.stringify(source)))
	check(result.get("ok", false), "pipe fixture compile: " + str(result.get("error", {})))
	if not result.get("ok", false): quit(1); return
	var document: Dictionary = result.data.document
	var prepared := PREVIEW.prepare(document, bridge)
	check(not prepared.has("error"), "shared preview preparation")
	var world := Node3D.new(); root.add_child(world)
	var preview := Node3D.new(); world.add_child(preview); PREVIEW.apply(preview, prepared)
	var refs: Array[WeakRef] = []
	for g: Dictionary in document.gimmicks:
		var visual := GEOMETRY.visual(g)
		var pipe := GEOMETRY.is_pipe(g)
		for i in visual.get_child_count():
			var mesh: MeshInstance3D = visual.get_child(i)
			var view: MeshInstance3D = preview.get_meta("objects")[g.id+":%d" % i]
			check(mesh.mesh.get_faces() == view.mesh.get_faces(), "display/preview shared vertices")
			if pipe:
				verify_material(mesh.material_override, Color("596168"))
				verify_material(view.material_override, Color("596168"))
				check(mesh.material_override.cull_mode == view.material_override.cull_mode, "matching cull mode")
				refs.append(weakref(view.material_override)); refs.append(weakref(mesh.material_override))
			elif g.has("track"):
				check(is_equal_approx(mesh.material_override.roughness, 0.72) and mesh.material_override.metallic == 0.0, "loop/halfpipe material unchanged")
		visual.free()
	var first: MeshInstance3D = preview.get_meta("objects")["track-0-0:0"]
	var identity := first.material_override.get_instance_id()
	PREVIEW.select(preview, 0)
	check(first.material_override.albedo_color == Color("f5ce5f"), "pipe selection highlight")
	PREVIEW.select(preview, -1)
	verify_material(first.material_override, Color("596168"))
	PREVIEW.apply(preview, prepared)
	check(first.material_override.get_instance_id() == identity, "unchanged preview reuses material")
	var templates: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://addons/mapkit/godot/driving_templates.json"))
	check(templates.cylinder.color.map(func(v): return int(v)) == [89,97,104,255], "standalone default colour")
	var custom: Dictionary = templates.cylinder.duplicate(true)
	custom.color = [23, 144, 71, 255]
	var original := custom.duplicate(true)
	var standalone := GEOMETRY.visual(custom)
	verify_material(standalone.get_child(0).material_override, Color8(23,144,71))
	check(custom == original, "saved explicit colour is never rewritten")
	standalone.free()
	var camera := Camera3D.new(); world.add_child(camera)
	camera.position = Vector3(-6,9,7); camera.look_at(Vector3(2,1.4,-5))
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = 16; camera.current = true
	var light := DirectionalLight3D.new(); world.add_child(light); light.rotation_degrees = Vector3(-55,-30,0)
	var environment := WorldEnvironment.new(); environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR; environment.environment.background_color = Color("27313b")
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR; environment.environment.ambient_light_color = Color.WHITE; environment.environment.ambient_light_energy = 0.7
	world.add_child(environment); root.size = Vector2i(1100,720)
	var capture := OS.get_environment("MINIEARTHURE_PIPE_CAPTURE")
	if DisplayServer.get_name() != "headless" and capture != "":
		for mode: String in ["before", "after"]:
			for view: MeshInstance3D in preview.get_meta("objects").values():
				if not view.get_meta("pipe", false): continue
				view.material_override = GEOMETRY.pipe_material(Color("596168"))
				if mode == "before":
					view.material_override.albedo_color = Color8(60,160,230)
					view.material_override.roughness = 0.72; view.material_override.metallic = 0.0
			for i in 4: await process_frame
			await RenderingServer.frame_post_draw
			check(root.get_texture().get_image().save_png(capture+"-"+mode+".png") == OK, "fixed camera/light comparison capture")
	world.queue_free(); await process_frame; await process_frame
	for ref: WeakRef in refs: check(ref.get_ref() == null, "pipe material released with nodes")
	print("pipe_material_validator: ", "PASS" if failures.is_empty() else failures)
	quit(0 if failures.is_empty() else 1)
