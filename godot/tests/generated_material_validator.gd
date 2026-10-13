extends SceneTree
const RENDER := preload("../chunk_renderer.gd")
const CACHE := preload("../render_resource_cache.gd")
const QUALITY := preload("../display_quality.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

# Reference appearance uses the public preview factory and imported-material path.
func reference(job: Dictionary, key: String, presentation: Dictionary) -> Material:
	var source := RENDER.surface_material(key,presentation,job.get("urban_shader"))
	if source is StandardMaterial3D:
		source.albedo_color = RENDER.ground_color(key,presentation)
		source.roughness = 0.9
		source.cull_mode = BaseMaterial3D.CULL_DISABLED
		source.resource_name = "mk_"+RENDER.detail_kind(key)
	return RENDER._environment_surface(job,source)

func compare(actual: ShaderMaterial, expected: ShaderMaterial, label: String) -> void:
	check(actual.shader == expected.shader,label+" shared shader identity")
	for uniform: Dictionary in actual.shader.get_shader_uniform_list():
		var name: String = uniform.name
		var a: Variant = actual.get_shader_parameter(name)
		var b: Variant = expected.get_shader_parameter(name)
		var fallback: Variant = RenderingServer.shader_get_parameter_default(actual.shader.get_rid(),name)
		if a == null: a = fallback
		if b == null: b = fallback
		# StandardMaterial stores float32; GDScript numbers reach the same GPU
		# representation without first round-tripping through that resource.
		if a is float and b is float:
			a = PackedFloat32Array([a])[0]; b = PackedFloat32Array([b])[0]
		check(a == b,label+" effective uniform "+name+": "+str(a)+" != "+str(b))
	for key in ["mapkit_surface","mapkit_vegetation","mapkit_detail","mapkit_opaque","mapkit_water"]:
		check(actual.get_meta(key,false) == expected.get_meta(key,false),label+" metadata "+key)

func run() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile = {"lights":[]}
	var context: RefCounted = cache.environment_context()
	var job := {"resources":cache,"urban_shader":cache.urban_shader()}
	var keys := ["grass","dirt","gravel","asphalt","concrete","brick:home","wood:home","builtin:tree","builtin:fence","builtin:streetlight","safety:metal"]
	for level in 3:
		QUALITY.current = QUALITY.profile(level)
		for climate in ["default","polar","arid","tropical","tinted"]:
			var profile := {"climate":climate}
			if climate == "tinted": profile.ground_color = [40,100,150]
			var presentation := {"environment_json":JSON.stringify(profile)}
			for key: String in keys:
				var actual: ShaderMaterial = RENDER._generated_material(job,key,presentation)
				var expected: ShaderMaterial = reference(job,key,presentation)
				compare(actual,expected,"%s/%s/%d" % [key,climate,level])
				QUALITY.apply_material(actual,QUALITY.profile(2-level))
				QUALITY.apply_material(expected,QUALITY.profile(2-level))
				compare(actual,expected,"live quality "+key)
	var urban := {"urban_surfaces":true,"road_styles":{"road:test":{"surface":1,"marked":true,"path_count":1,"road_paths":PackedVector4Array([Vector4(0,0,4,0)])}}}
	for key: String in ["road:test","asphalt","concrete"]:
		compare(RENDER._generated_material(job,key,urban),reference(job,key,urban),"urban "+key)
	for key: String in ["rc:floor","rc:wall","rc:road:0"]:
		compare(RENDER._generated_material(job,key,{"track_seed":42}),reference(job,key,{"track_seed":42}),key)
	var preview: Material = RENDER._generated_material({},"safety:metal",{})
	check(preview is StandardMaterial3D and is_equal_approx(preview.metallic,0.8) and is_equal_approx(preview.roughness,0.9),"standalone preview keeps source material")
	preview = null
	await compare_pixels(job)
	await attachment_paths(cache)
	var first: ShaderMaterial = RENDER._generated_material(job,"grass",{})
	var second: ShaderMaterial = RENDER._generated_material(job,"grass",{})
	check(first != second,"cells own independent mutable bindings")
	first.set_shader_parameter("base_color",Color.RED)
	check(second.get_shader_parameter("base_color") != Color.RED,"one cell cannot mutate another")
	var held := second
	job.clear();context = null;first = null;second = null
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() > 0,"last material borrower keeps the context charged")
	held = null
	for _frame in 4: await process_frame
	check(cache.bytes() == 0,"last borrower returns all shared reservations")
	QUALITY.current = {}
	print("generated_material_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func compare_pixels(job: Dictionary) -> void:
	var viewport := SubViewport.new();viewport.size = Vector2i(160,120)
	viewport.own_world_3d = true;viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(viewport)
	var scene := Node3D.new();viewport.add_child(scene)
	var mesh := MeshInstance3D.new();mesh.mesh = BoxMesh.new();scene.add_child(mesh)
	var camera := Camera3D.new();scene.add_child(camera)
	camera.position = Vector3(2,2,3);camera.look_at(Vector3.ZERO);camera.current = true
	var light := DirectionalLight3D.new();scene.add_child(light);light.rotation_degrees = Vector3(-45,-30,0)
	for key: String in ["grass","wood:home","safety:metal"]:
		mesh.material_override = reference(job,key,{})
		for _frame in 3: await process_frame
		await RenderingServer.frame_post_draw
		var expected := viewport.get_texture().get_image().get_data()
		mesh.material_override = RENDER._generated_material(job,key,{})
		for _frame in 3: await process_frame
		await RenderingServer.frame_post_draw
		check(viewport.get_texture().get_image().get_data() == expected,"identical rendered pixels: "+key)
	viewport.queue_free()
	await process_frame

func attachment_paths(cache: RefCounted) -> void:
	var vertices := PackedVector3Array([Vector3.ZERO,Vector3.RIGHT,Vector3.FORWARD])
	var chunk := {"cell":Vector2i.ZERO,"triangles":[{"surface":"grass","object_id":"ground"}],"objects":[],"scene_vertices":vertices,"scene_normals":PackedVector3Array([Vector3.UP,Vector3.UP,Vector3.UP]),"wall_uv":PackedVector2Array([Vector2.ZERO,Vector2.RIGHT,Vector2.UP])}
	for prepared in [false,true]:
		var input := chunk.duplicate(true)
		if prepared: input.render_batches = RENDER.PLAN.prepare_batches(input)
		var parent := Node3D.new();root.add_child(parent)
		var lease := Lease.new(RENDER.PLAN.estimate(input,0))
		var job := RENDER.begin(input,parent,func(_bytes): return lease,lease.bytes,cache)
		while not RENDER.advance(job): pass
		check(job.error.is_empty(),"generated surface attachment")
		var mesh: MeshInstance3D = job.root.get_child(0)
		check(mesh.mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX] == vertices,"prepared/raw geometry unchanged")
		check(mesh.material_override is ShaderMaterial,"both attachment paths bind final material")
		mesh = null
		RENDER.cancel(job);parent.queue_free();job.clear()
		for _frame in 4: await process_frame
		var alive := []
		for ref: WeakRef in lease.refs:
			if ref.get_ref() != null: alive.append(ref.get_ref().get_class())
		check(lease.retired(),"cancelled cell releases its private material and mesh; sealed=%s alive=%s" % [lease.sealed,alive])
