extends SceneTree
## Shared native preview, render material and real convex contact regression.
const RENDER := preload("../chunk_renderer.gd")
const DATA := preload("../chunk_data.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed = true; push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	root.size = Vector2i(1120,760)
	var document := FileAccess.get_file_as_string("res://addons/mapkit/godot/tests/road_safety_fixture.json")
	var native: RefCounted = ClassDB.instantiate("MapKitBridge")
	var world := Node3D.new(); root.add_child(world)
	var jobs: Array = []
	var metal_count := 0
	var curved_styles := 0
	for x in 2:
		var generated: Dictionary = native.preview_document_packed(document,x,0)
		check(generated.get("ok",false),"native shared plan: "+str(generated.get("error",{})))
		if not generated.get("ok",false): world.free(); quit(1); return
		var chunk: Dictionary = generated.data.chunk
		var view: Dictionary = DATA.view(chunk)
		for i in DATA.count(view):
			if DATA.object_id(view,i).ends_with(":safety:metal"):
				metal_count += 1
				check(not DATA.spawnable(view,i),"rail is not a recovery surface")
		for style: Dictionary in chunk.presentation.road_styles.values():
			check(style.road_paths.size()==128 and style.path_count<=128,"bounded path uniforms")
			if style.path_count>2: curved_styles += 1
		for i in DATA.prism_count(view):
			var body := StaticBody3D.new(); body.set_meta("recovery_surface",false)
			var collider := CollisionShape3D.new(); var shape := ConvexPolygonShape3D.new()
			shape.points = DATA.prism_points(view,i,1.0); collider.shape=shape
			body.add_child(collider);world.add_child(body)
		var job := RENDER.begin(chunk,world)
		while not RENDER.advance(job): pass
		check(job.error.is_empty(),"shared renderer completed")
		jobs.append(job)
	check(metal_count>0 and curved_styles>0,"metal faces and curved lane paths present")
	var material := RENDER.surface_material("safety:metal",{},null) as StandardMaterial3D
	check(material.metallic>=0.8 and material.roughness<0.5,"metal is a display material")
	await physics_frame; await physics_frame
	var query := PhysicsRayQueryParameters3D.create(Vector3(20,4.25,-42),Vector3(20,4.25,-46))
	var hit := world.get_world_3d().direct_space_state.intersect_ray(query)
	check(not hit.is_empty(),"ray from carriageway hits the metal rail")
	if not hit.is_empty(): check(not hit.collider.get_meta("recovery_surface"),"contact is excluded from recovery")
	if DisplayServer.get_name() != "headless":
		var environment := WorldEnvironment.new(); environment.environment=Environment.new()
		environment.environment.background_mode=Environment.BG_COLOR;environment.environment.background_color=Color("aabecb")
		environment.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR
		environment.environment.ambient_light_color=Color.WHITE;environment.environment.ambient_light_energy=0.5
		world.add_child(environment)
		var light := DirectionalLight3D.new(); world.add_child(light); light.rotation_degrees=Vector3(-55,-25,0)
		var camera := Camera3D.new(); world.add_child(camera);camera.current=true
		camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=70
		camera.position=Vector3(50,65,25);camera.look_at(Vector3(45,0,-25))
		await process_frame;await RenderingServer.frame_post_draw
		var path := OS.get_environment("MAPKIT_ROAD_CAPTURE")
		if not path.is_empty(): root.get_texture().get_image().save_png(path)
	for job: Dictionary in jobs: RENDER.dispose(job)
	world.queue_free();await process_frame;await process_frame
	print("road_safety_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
