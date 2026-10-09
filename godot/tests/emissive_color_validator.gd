extends SceneTree
## Actual near/far pixels retain authored neon hue and switch off in daylight.
const MATERIALS := preload("../environment_materials.gd")
var failed := false
func check(value: bool, label: String) -> void:
	if not value: failed=true;push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	root.size=Vector2i(128,128)
	var scene:=Node3D.new();root.add_child(scene)
	var world:=WorldEnvironment.new();world.environment=Environment.new()
	world.environment.background_mode=Environment.BG_COLOR
	world.environment.background_color=Color.BLACK
	world.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_energy=0.0
	scene.add_child(world)
	var camera:=Camera3D.new();camera.position=Vector3(0,0,4)
	camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=2.4
	scene.add_child(camera);camera.look_at(Vector3.ZERO);camera.current=true
	var context:=MATERIALS.new()
	for tint: Color in [Color(.24,.88,.90),Color(.91,.32,.58)]:
		var samples: Array[Color]=[]
		for distant in [false,true]:
			var arrays:=[];arrays.resize(Mesh.ARRAY_MAX)
			arrays[Mesh.ARRAY_VERTEX]=PackedVector3Array([Vector3(-1,-1,0),Vector3(1,-1,0),Vector3(1,1,0),Vector3(-1,-1,0),Vector3(1,1,0),Vector3(-1,1,0)])
			var normals:=PackedVector3Array();var colors:=PackedColorArray();var uv:=PackedVector2Array()
			for i in 6: normals.append(Vector3.BACK);colors.append(tint.linear_to_srgb());uv.append(Vector2(0,2))
			arrays[Mesh.ARRAY_NORMAL]=normals;arrays[Mesh.ARRAY_COLOR]=colors;arrays[Mesh.ARRAY_TEX_UV]=uv
			var mesh:=ArrayMesh.new();mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
			var node:=MeshInstance3D.new();node.mesh=mesh;scene.add_child(node)
			var source:=StandardMaterial3D.new()
			# GLTFDocument converts linear PBR factors to Godot's colour property.
			source.albedo_color=tint.linear_to_srgb() if not distant else Color.WHITE
			var material: ShaderMaterial=context.surface_material(source,2 if not distant else 0)
			material.set_shader_parameter("distant",distant);node.material_override=material
			context.update(72000,0,0,-.2,1080)
			for frame in 5:await process_frame
			await RenderingServer.frame_post_draw
			var picture:=root.get_texture().get_image()
			var night:=picture.get_pixel(floori(picture.get_width()*.5),floori(picture.get_height()*.5));samples.append(night)
			check(maxf(night.r,maxf(night.g,night.b))>.4,"night sign remains visible")
			check(absf(night.r-night.g)>.12,"night sign keeps cyan/pink hue instead of white")
			context.update(43200,0,0,.8,1080)
			for frame in 3:await process_frame
			await RenderingServer.frame_post_draw
			picture=root.get_texture().get_image()
			var day:=picture.get_pixel(floori(picture.get_width()*.5),floori(picture.get_height()*.5))
			check(maxf(day.r,maxf(day.g,day.b))<.04,"daylight switches sign emission off")
			node.free()
		print("NEON_COLOR primary=",samples[0]," distant=",samples[1])
		var delta:=maxf(absf(samples[0].r-samples[1].r),maxf(absf(samples[0].g-samples[1].g),absf(samples[0].b-samples[1].b)))
		check(delta<.04,"near/far neon hue mismatch")
	scene.queue_free();context=null
	for frame in 3:await process_frame
	print("emissive_color_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
