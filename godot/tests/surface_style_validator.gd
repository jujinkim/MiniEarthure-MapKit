extends SceneTree
## Compare actual pixels across material paths, quality, snow and night.
const MATERIALS := preload("../environment_materials.gd")
const QUALITY := preload("../display_quality.gd")
var failed := false
func _initialize() -> void: run.call_deferred()
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func pixel() -> Color:
	for frame in 4: await process_frame
	await RenderingServer.frame_post_draw
	var picture := root.get_texture().get_image()
	return picture.get_pixelv(picture.get_size()/2)
func run() -> void:
	root.size = Vector2i(128,128)
	var scene := Node3D.new(); root.add_child(scene)
	var environment := WorldEnvironment.new(); environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color = Color.WHITE
	environment.environment.ambient_light_energy = 1.0
	scene.add_child(environment)
	var camera := Camera3D.new(); camera.position = Vector3(0,0,4)
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = 2.4
	scene.add_child(camera); camera.look_at(Vector3.ZERO)
	var context := MATERIALS.new()
	var node := MeshInstance3D.new(); scene.add_child(node)
	for tint in [Color("898989"),Color("b46c50"),Color("486447"),Color("69c9d0")]:
		var arrays := []; arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = PackedVector3Array([Vector3(-1,-1,0),Vector3(1,-1,0),Vector3(1,1,0),Vector3(-1,-1,0),Vector3(1,1,0),Vector3(-1,1,0)])
		var normals := PackedVector3Array(); var colors := PackedColorArray(); var uv := PackedVector2Array()
		for i in 6: normals.append(Vector3.UP); colors.append(tint); uv.append(Vector2.ZERO)
		arrays[Mesh.ARRAY_NORMAL] = normals; arrays[Mesh.ARRAY_COLOR] = colors; arrays[Mesh.ARRAY_TEX_UV] = uv
		var mesh := ArrayMesh.new(); mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays); node.mesh = mesh
		var source := StandardMaterial3D.new(); source.albedo_color = tint
		var imported: ShaderMaterial = context.surface_material(source,0)
		var generated: ShaderMaterial = context.generated_material(tint,"")
		var distant: ShaderMaterial = context.generated_material(Color.WHITE,"")
		distant.set_shader_parameter("distant",true)
		var seasons: Array[Color] = []
		for weather in [[0.0,0.8],[0.0,-0.3],[1.0,0.8]]:
			context.update(43200,0,weather[0],weather[1],1080)
			var reference := Color.BLACK
			for level in 3:
				for material in [imported,generated,distant]:
					QUALITY.apply_material(material,QUALITY.profile(level))
					material.set_shader_parameter("surface_effect_distance",1.0)
					node.material_override = material
					var actual := await pixel()
					if material == imported and level == 0: reference = actual
					check(Vector3(reference.r,reference.g,reference.b).distance_to(Vector3(actual.r,actual.g,actual.b))<.025,"same colour across near/far and quality: "+str([tint,weather,level]))
			seasons.append(reference)
		if tint == Color("898989"):
			check(seasons[0].r>seasons[0].b,"daylight grade is warm")
			check(absf(seasons[1].r-seasons[1].b)<.015,"night removes warm grade")
			check(seasons[2].b>seasons[2].r,"snow retains its cool colour")
	node.material_override = null; node.mesh = null
	scene.queue_free(); context = null
	for frame in 4: await process_frame
	print("surface_style_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
