extends SceneTree
## Generated building facades: key classification, twelve-colour palette shared
## with the distant mesh, floor pitch from the recorded map scale and shader uniforms.
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
	var kinds := {"concrete:residential":1,"brick:residential":2,"wood:residential":2,"concrete:commercial":3,"brick:commercial":3,"wood:industrial":4,"concrete:public":5,"brick:home":0,"concrete":0,"barrier:wall":0,"rc:wall":0,"builtin:tree":0}
	for key: String in kinds:
		check(RENDER.facade_kind(key) == kinds[key],"facade kind "+key)
	var scaled: ShaderMaterial = RENDER._generated_material(job,"concrete:commercial",{"facade_floor_cm":100})
	check(scaled.get_shader_parameter("facade_kind") == 3 and is_equal_approx(scaled.get_shader_parameter("floor_pitch"),1.0) and scaled.get_shader_parameter("lighting_role") == 1,"generated walls carry the facade kind, 1:3 floor pitch and night windows")
	check(scaled.get_shader_parameter("signs_enabled") == true and scaled.get_shader_parameter("sign_atlas") == context.signs,"commercial walls draw the shared sign atlas")
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
