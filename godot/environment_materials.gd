extends RefCounted
const SURFACE := preload("./atmosphere_surface.gdshader")
const PROFILE := preload("./environment_profile.gd")
const DETAIL := preload("./material_detail.gdshaderinc")
const TILE_KINDS := ["brick","plaster","wood","stone","asphalt","concrete","earth","grass"]
# Same conservative CPU/GPU/import allowance as validated package bitmaps.
const MEMORY_BYTES := 1048576 + 8 * 256 * 256 * 24
var tiles: Dictionary = {}
var pixels: Image
var texture: ImageTexture
var materials: Dictionary = {}
var shader: Shader

func _init() -> void:
	pixels = Image.create(2,1,false,Image.FORMAT_RGBAF)
	pixels.set_pixel(0,0,Color(0,0,0,0))
	pixels.set_pixel(1,0,Color(43200,0,1,64800))
	texture = ImageTexture.create_from_image(pixels)
	shader = Shader.new()
	shader.code = SURFACE.code.replace('#include "wet_surface.gdshaderinc"',preload("./wet_surface.gdshaderinc").code)
	shader.code = shader.code.replace('#include "material_detail.gdshaderinc"',DETAIL.code)
	for kind: String in TILE_KINDS:
		var path: String = get_script().resource_path.get_base_dir().path_join("textures/"+kind+".png")
		# Trusted library resources use Godot's import remap in exported packages.
		# Load only after the context lease; never pin bitmaps with script preloads.
		var tile := ResourceLoader.load(path,"Texture2D",ResourceLoader.CACHE_MODE_IGNORE) as Texture2D
		if tile != null:
			var image := tile.get_image()
			if image == null or image.is_empty(): continue
			image.generate_mipmaps()
			tiles[kind] = ImageTexture.create_from_image(image)

func update(seconds: float, wet: float, snow: float, sun_altitude: float, sunset_minutes: int) -> void:
	pixels.set_pixel(0,0,Color(wet,snow,0,0))
	pixels.set_pixel(1,0,Color(fposmod(seconds,86400.0),PROFILE.night_index(seconds),sun_altitude,sunset_minutes*60.0))
	texture.update(pixels)

func surface_material(source: Material, role: int, use_instances := false) -> Material:
	if not source is StandardMaterial3D: return source
	if source.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED: return source
	var key := "%d/%d/%d" % [source.get_instance_id(),role,int(use_instances)]
	if materials.has(key) and materials[key].get_ref() != null: return materials[key].get_ref()
	if materials.size() >= 256:
		for retired: String in materials.keys():
			if materials[retired].get_ref() == null: materials.erase(retired)
	var result := ShaderMaterial.new()
	result.shader = shader
	result.set_shader_parameter("environment_data",texture)
	result.set_shader_parameter("base_color",source.albedo_color)
	result.set_shader_parameter("roughness",source.roughness)
	result.set_shader_parameter("metallic",source.metallic)
	result.set_shader_parameter("vertex_tinted",source.vertex_color_use_as_albedo)
	result.set_shader_parameter("lighting_role",role)
	result.set_shader_parameter("instanced",use_instances)
	result.set_shader_parameter("uv_scale",source.uv1_scale)
	result.set_shader_parameter("uv_offset",source.uv1_offset)
	for slot: String in ["normal","ao","roughness","metallic"]:
		var bitmap: Texture2D = source.get(slot+"_texture")
		if bitmap == null: continue
		if slot in ["normal","ao"] and not source.get(slot+"_enabled"): continue
		result.set_shader_parameter(slot+"_enabled",true)
		result.set_shader_parameter(slot+"_texture",bitmap)
		if slot != "normal": result.set_shader_parameter(slot+"_channel",channel(int(source.get(slot+"_texture_channel"))))
	result.set_shader_parameter("normal_strength",source.normal_scale)
	result.set_shader_parameter("ao_uv2",source.ao_on_uv2)
	result.set_shader_parameter("ao_light_affect",source.ao_light_affect)
	var kind := source.resource_name.trim_prefix("mk_")
	bind_detail(result,kind)
	result.set_shader_parameter("terrain_surface",kind in ["grass","earth"])
	if kind.begins_with("water_"):
		result.set_shader_parameter("water_kind",1 if kind == "water_flow" else 2)
		result.set_meta("mapkit_water",true)
	if source.albedo_texture != null:
		result.set_shader_parameter("textured",true)
		result.set_shader_parameter("albedo_texture",source.albedo_texture)
	result.set_meta("mapkit_opaque",true)
	materials[key] = weakref(result)
	return result

static func channel(value: int) -> Vector4:
	return [Vector4(1,0,0,0),Vector4(0,1,0,0),Vector4(0,0,1,0),Vector4(0,0,0,1),Vector4(0.333333,0.333333,0.333333,0)][clampi(value,0,4)]

func bind_detail(material: ShaderMaterial, kind: String) -> void:
	if not tiles.has(kind): return
	material.set_shader_parameter("detail_enabled",true)
	material.set_shader_parameter("detail_tile",tiles[kind])
	material.set_shader_parameter("detail_scale",2.0 if kind in ["asphalt","earth","grass"] else 1.0)

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
	for tile: Texture2D in tiles.values(): lease.track(tile)
	for reference: WeakRef in materials.values():
		if reference.get_ref() != null: lease.track(reference.get_ref())
