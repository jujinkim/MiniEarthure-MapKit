extends RefCounted
const SURFACE := preload("./atmosphere_surface.gdshader")
const PROFILE := preload("./environment_profile.gd")
var pixels: Image
var texture: ImageTexture
var materials: Dictionary = {}
var shader: Shader

func _init() -> void:
	pixels = Image.create(2,1,false,Image.FORMAT_RGBAF)
	pixels.set_pixel(0,0,Color(0,0,0,0))
	pixels.set_pixel(1,0,Color(43200,0,1,64800))
	texture = ImageTexture.create_from_image(pixels)
	shader = SURFACE.duplicate()

func update(seconds: float, wet: float, snow: float, sun_altitude: float, sunset_minutes: int) -> void:
	pixels.set_pixel(0,0,Color(wet,snow,0,0))
	pixels.set_pixel(1,0,Color(fposmod(seconds,86400.0),PROFILE.night_index(seconds),sun_altitude,sunset_minutes*60.0))
	texture.update(pixels)

func surface_material(source: Material, role: int, use_instances := false) -> Material:
	if not source is StandardMaterial3D: return source
	if source.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED: return source
	var key := "%d/%d/%d" % [source.get_instance_id(),role,int(use_instances)]
	if materials.has(key) and materials[key].get_ref() != null: return materials[key].get_ref()
	var result := ShaderMaterial.new()
	result.shader = shader
	result.set_shader_parameter("environment_data",texture)
	result.set_shader_parameter("base_color",source.albedo_color)
	result.set_shader_parameter("roughness",source.roughness)
	result.set_shader_parameter("metallic",source.metallic)
	result.set_shader_parameter("lighting_role",role)
	result.set_shader_parameter("instanced",use_instances)
	if source.albedo_texture != null:
		result.set_shader_parameter("textured",true)
		result.set_shader_parameter("albedo_texture",source.albedo_texture)
	result.set_meta("mapkit_opaque",true)
	materials[key] = weakref(result)
	return result

func style_mesh(mesh: Mesh, binding: Dictionary, instances := false) -> Mesh:
	# Own surface bindings; vertex/index buffers and texture resources remain shared.
	var copy: Mesh = mesh.duplicate()
	for index in copy.get_surface_count():
		var source := mesh.surface_get_material(index)
		var material_index := int(source.get_meta("mapkit_material_index",index)) if source != null else index
		var role := 1 if binding.get("window_materials",[]).any(func(value): return int(value)==material_index) else (2 if binding.get("bulb_materials",[]).any(func(value): return int(value)==material_index) else 0)
		copy.surface_set_material(index, surface_material(mesh.surface_get_material(index),role,instances))
	return copy

func track(lease: RefCounted) -> void:
	if lease == null: return
	lease.track(pixels)
	lease.track(texture)
	lease.track(shader)
	for reference: WeakRef in materials.values():
		if reference.get_ref() != null: lease.track(reference.get_ref())
