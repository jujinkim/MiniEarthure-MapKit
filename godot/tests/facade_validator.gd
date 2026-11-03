extends SceneTree
## Generated building facades: key classification, twelve base colours and the
## record palettes shared with the distant mesh, floor pitch from the recorded
## map scale and shader uniforms.
const RENDER := preload("../chunk_renderer.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	func _init(amount: int) -> void: bytes = amount
	func track(_value: Object) -> void: pass
	func seal() -> void: pass

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile = {"lights":[]}
	var context: RefCounted = cache.environment_context()
	check(context != null,"shared environment context")
	var job := {"resources":cache,"urban_shader":cache.urban_shader()}
	var names := {}
	for uniform: Dictionary in MATERIALS.new().shader.get_shader_uniform_list(): names[uniform.name] = true
	check(names.has("facade_kind") and names.has("floor_pitch") and names.has("lighting_role") and names.has("sign_atlas"),"surface shader compiles with the facade uniforms")
	check(names.has("facade_body") and names.has("facade_roof") and names.has("facade_accent") and names.has("facade_palette"),"surface shader compiles with the record palette uniforms")
	# Per-solid choices must not depend on interpolated values: flat seeds and an
	# integer hash keep each sign slot and lit window one choice across its pixels.
	var code: String = MATERIALS.new().shader.code
	check(code.contains("varying flat float solid_seed;") and code.contains("varying flat float solid_base;"),"solid seed and base are flat varyings")
	check(code.contains("float pick = facade_unit(") and code.contains("facade_lit = step(0.35,facade_unit("),"sign picks and lit windows use the integer hash")
	check(code.contains("textureGrad(sign_atlas") and not code.contains("texture(sign_atlas"),"signs sample with analytic gradients")
	var kinds := {"concrete:residential":1,"brick:residential":2,"wood:residential":2,"concrete:commercial":3,"brick:commercial":3,"wood:industrial":4,"concrete:public":5,"brick:home":0,"concrete":0,"barrier:wall":0,"rc:wall":0,"builtin:tree":0}
	for key: String in kinds:
		check(RENDER.facade_kind(key) == kinds[key],"facade kind "+key)
	var scaled: ShaderMaterial = RENDER._generated_material(job,"concrete:commercial",{"facade_floor_cm":100})
	check(scaled.get_shader_parameter("facade_kind") == 3 and is_equal_approx(scaled.get_shader_parameter("floor_pitch"),1.0) and scaled.get_shader_parameter("lighting_role") == 1,"generated walls carry the facade kind, 1:3 floor pitch and night windows")
	check(scaled.get_shader_parameter("signs_enabled") == true and scaled.get_shader_parameter("sign_atlas") == context.signs,"commercial walls draw the shared sign atlas")
	var table: Dictionary = RENDER.FACADE_PALETTE.data
	var commercial: Dictionary = RENDER.facade_palette("concrete:commercial")
	check(scaled.get_shader_parameter("facade_palette") == true and scaled.get_shader_parameter("facade_body") == commercial.body and scaled.get_shader_parameter("facade_roof") == commercial.roof and scaled.get_shader_parameter("facade_accent") == commercial.accent,"generated walls bind their record palettes")
	for key: String in ["concrete:residential","concrete:commercial","concrete:industrial","concrete:public","brick:residential","wood:industrial","concrete:school"]:
		var palette: Dictionary = RENDER.facade_palette(key)
		var fields := key.split(":")
		var body_key: String = fields[0] if fields[0] != "concrete" else ("concrete:" + fields[1] if fields[1] in ["commercial","industrial","public"] else "concrete:residential")
		var rgb: Array = table.body[body_key][5]
		check(palette.body.size() == 8 and palette.roof.size() == 8 and palette.accent.size() == 8,"eight palette entries "+key)
		check(palette.body[5].is_equal_approx(Vector3(rgb[0],rgb[1],rgb[2])/255.0),"body palette follows facade_palette.json "+key)
		var roof: Array = table.roof[fields[0] if fields[0] != "concrete" else "concrete"][2]
		check(palette.roof[2].is_equal_approx(Vector3(roof[0],roof[1],roof[2])/255.0),"roof palette follows facade_palette.json "+key)
	var unscaled: ShaderMaterial = RENDER._generated_material(job,"brick:residential",{})
	check(is_equal_approx(unscaled.get_shader_parameter("floor_pitch"),3.0) and unscaled.get_shader_parameter("facade_kind") == 2,"missing scale provenance keeps 3 m floors")
	check(unscaled.get_shader_parameter("signs_enabled") == null,"homes carry no storefront signs")
	var plain: ShaderMaterial = RENDER._generated_material(job,"brick:home",{})
	check(plain.get_shader_parameter("facade_kind") == null and plain.get_shader_parameter("lighting_role") == null,"unknown use keeps a plain unlit wall")
	var ground: ShaderMaterial = RENDER._generated_material(job,"grass",{})
	check(ground.get_shader_parameter("facade_kind") == null,"ground keeps no facade")
	var standalone: Material = RENDER._generated_material({},"concrete:public",{"facade_floor_cm":100})
	check(standalone is StandardMaterial3D,"standalone preview without a context keeps its source material")
	var colours := {}
	for material: String in RENDER.BUILDING_BASE:
		for usage: String in RENDER.BUILDING_TINT:
			colours[RENDER.material_color(material+":"+usage).to_html(false)] = true
	check(colours.size() == 12,"twelve distinct material/use colours")
	check(RENDER.material_color("concrete:residential").to_html(false) == "d4cab8" and RENDER.material_color("brick:commercial").to_html(false) == "ae8273","palette matches the distant mesh (mapkit-package distant.rs)")
	check(RENDER.material_color("concrete:residential") != RENDER.material_color("concrete:commercial"),"uses differ within one material")
	job.clear(); context = null; scaled = null; unscaled = null; plain = null; ground = null
	cache.shutdown()
	print("facade_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
