extends SceneTree
## Voxel ground grain (atmosphere_surface surface_grit): the surface shader
## declares it off by default; rendered, a flat earth quad captured untouched,
## then with the grain on and set back off, returns to its exact untouched
## pixels, and with it on keeps its mean luminance within 2% of off while the
## cells add tone variance, overhead and at a grazing view.
const SURFACE := preload("../atmosphere_surface.gdshader")
var failed := false
func check(value: bool, label: String) -> void:
	if not value: failed=true;push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var declared := SURFACE.get_shader_uniform_list().filter(func(u: Dictionary): return u.name == "surface_grit")
	check(declared.size() == 1,"surface shader declares surface_grit")
	if DisplayServer.get_name() != "headless": await pixels()
	print("surface_grit_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func pixels() -> void:
	root.size = Vector2i(128,128)
	var scene := Node3D.new(); root.add_child(scene)
	var world := WorldEnvironment.new(); world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_color = Color.WHITE
	world.environment.ambient_light_energy = 1.0
	world.environment.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED
	scene.add_child(world)
	# Dry daytime environment row: no rain, snow or night lights.
	var data := Image.create(4,1,false,Image.FORMAT_RGBAF); data.fill(Color(0,0,0,0))
	var environment_data := ImageTexture.create_from_image(data)
	var quad := MeshInstance3D.new(); var plane := PlaneMesh.new(); plane.size = Vector2(400,400)
	quad.mesh = plane; quad.position.y = 2.0
	scene.add_child(quad)
	var camera := Camera3D.new(); camera.far = 1000.0; scene.add_child(camera); camera.current = true
	for view in [[Vector3(0,12,0),Vector3(0,2,0.01),"overhead"],[Vector3(0,2.36,0),Vector3(0,2,-20),"grazing"]]:
		camera.position = view[0]; camera.look_at(view[1])
		# Untouched: a fresh material that never sets surface_grit renders the
		# shader default; on, then back off, must restore it exactly.
		var material := ShaderMaterial.new(); material.shader = SURFACE
		material.set_shader_parameter("environment_data",environment_data)
		material.set_shader_parameter("terrain_surface",true)
		material.set_shader_parameter("base_color",Color(.5,.45,.35))
		quad.material_override = material
		var baseline := await capture()
		material.set_shader_parameter("surface_grit",1.0)
		var on := await capture()
		material.set_shader_parameter("surface_grit",0.0)
		var off := await capture()
		check(off.get_data() == baseline.get_data(),"%s: grain set back to 0 restores the untouched pixels"%view[2])
		check(on.get_data() != baseline.get_data(),"%s: grain 1 changes the pixels"%view[2])
		var plain := stats(off); var grain := stats(on)
		print("SURFACE_GRIT view=%s off=%s on=%s"%[view[2],plain,grain])
		check(absf(grain[0]/maxf(plain[0],1e-3)-1.0) < 0.02,"%s: grain keeps the mean luminance within 2%%"%view[2])
		# The cells' own spread, net of the broad terrain variation.
		check(sqrt(maxf(grain[1]*grain[1]-plain[1]*plain[1],0.0)) > 0.002,"%s: grain cells add tone variance"%view[2])
	scene.free()
	await process_frame

func capture() -> Image:
	for frame in 3: await process_frame
	await RenderingServer.frame_post_draw
	return root.get_texture().get_image()

## Mean luminance and its standard deviation over the lit pixels.
static func stats(picture: Image) -> Array:
	var total := 0.0; var squares := 0.0; var count := 0
	for y in picture.get_height():
		for x in picture.get_width():
			var c := picture.get_pixel(x,y)
			var l := c.r*0.2126+c.g*0.7152+c.b*0.0722
			if l <= 0.0: continue
			total += l; squares += l*l; count += 1
	if count == 0: return [0.0,0.0]
	var mean := total/float(count)
	return [mean,sqrt(maxf(squares/float(count)-mean*mean,0.0))]
