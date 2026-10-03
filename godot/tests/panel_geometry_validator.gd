extends SceneTree
const GEOMETRY := preload("../gimmick_geometry.gd")
const PREVIEW := preload("../track_authoring_preview.gd")
var failures: Array[String]=[]
func _initialize() -> void: run.call_deferred()
func check(value: bool, label: String) -> void:
	if not value: failures.append(label); push_error(label)
func run() -> void:
	var bridge: RefCounted=ClassDB.instantiate("MapKitBridge")
	var source := {"original_seed":null,"grounded_supports":false,"settings":{"seed":101,"circuit":false,"duration_seconds":60,"difficulty":"easy","categories":["driving"],"time_minutes":720},"instances":[],"connections":[],"paths":[],"checkpoints":[],"actions":[],"attachments":[]}
	for i in 2:
		var id: String="road-%d" % i
		source.instances.append({"id":id,"preset":"slope_up","position_cm":[i*650,0,0],"rotation_mdeg":[0,0,0],"width_cm":400,"entry_width_cm":400,"exit_width_cm":400,"control_points":[]})
		source.actions.append({"id":"panel-%d" % i,"kind":"acceleration_panel" if i==0 else "jump_panel","piece":id,"sample":4,"height_cm":200,"panel_width_percent":50,"panel_alignment":"center","landing":null})
	var result: Dictionary=JSON.parse_string(bridge.compile_track_source(JSON.stringify(source)))
	check(result.get("ok",false),"synthetic panel document compiles: " + str(result.get("error",{})))
	if not result.get("ok",false): quit(1);return
	var document: Dictionary=result.data.document
	var prepared := PREVIEW.prepare(document,bridge)
	check(not prepared.has("error"),"shared preview preparation")
	var world:=Node3D.new();root.add_child(world)
	var preview:=Node3D.new();world.add_child(preview);PREVIEW.apply(preview,prepared)
	for g: Dictionary in document.gimmicks:
		if not g.id.begins_with("action-"): continue
		var visual:=GEOMETRY.visual(g)
		check(visual.get_child_count()==g.parts.size(),"markings add no extra meshes")
		var part_index:=0
		for mesh: MeshInstance3D in visual.get_children():
			var row: Dictionary=prepared.objects[g.id+":%d" % part_index]
			check(mesh.mesh.get_faces()==row.arrays[Mesh.ARRAY_VERTEX],"Client and Editor share exact top/body vertices")
			check(mesh.material_override is ShaderMaterial and row.panel==GEOMETRY.panel_style(g),"shared procedural marking and footprint")
			part_index+=1
		visual.free()
	var camera:=Camera3D.new();world.add_child(camera);camera.position=Vector3(3.25,14,3);camera.look_at(Vector3(3.25,0.5,-4));camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=13;camera.current=true
	var light:=DirectionalLight3D.new();world.add_child(light);light.rotation_degrees=Vector3(-65,-25,0)
	var environment:=WorldEnvironment.new();environment.environment=Environment.new();environment.environment.background_mode=Environment.BG_COLOR;environment.environment.background_color=Color("27313b");environment.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR;environment.environment.ambient_light_color=Color.WHITE;environment.environment.ambient_light_energy=0.6;world.add_child(environment)
	root.size=Vector2i(1100,720)
	for i in 8: await process_frame
	if DisplayServer.get_name()!="headless":
		await RenderingServer.frame_post_draw
		var capture:=OS.get_environment("MINIEARTHURE_PANEL_CAPTURE")
		if capture!="": check(root.get_texture().get_image().save_png(capture)==OK,"synthetic panel capture")
	world.queue_free();await process_frame;await process_frame
	print("panel_geometry_validator: ","PASS" if failures.is_empty() else failures)
	quit(0 if failures.is_empty() else 1)
