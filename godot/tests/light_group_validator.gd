extends SceneTree
const RENDERER := preload("../environment_renderer.gd")
const CHUNKS := preload("../chunk_renderer.gd")
class LampResources extends RefCounted:
	var environment_profile := {"lights":[{"asset_id":"fixture:lamp", "bulb_materials":[0], "position_cm":[0,312,0], "range_cm":1200, "color":[255,218,165]}]}
var failed := false
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func _initialize() -> void: run.call_deferred()
func group(priority: int, count: int, x: float) -> Dictionary:
	var lamps: Array = []
	for index in count:
		lamps.append({"position":Vector3(x,index,0),"basis":Basis.IDENTITY,"color":Color.WHITE,"energy":1.0,"range":2.0,"angle":35.0})
	return {"priority":priority,"position":Vector3(x,0,0),"lights":lamps}
func run() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var cell := Node3D.new()
	world.add_child(cell)
	CHUNKS._environment_lamp({"root":cell,"resources":LampResources.new()}, "fixture:lamp", {"quarter_turns":0,"position":[0,0,0]}, {})
	var street: Dictionary = cell.get_meta("environment_lamps")[0]
	check(is_equal_approx(tan(deg_to_rad(street.angle))/tan(deg_to_rad(48.0)), 3.0), "streetlight ground footprint is three times as wide")
	check(is_equal_approx(street.range, 36.0) and street.energy == 2.0 and is_equal_approx(street.position.y, 3.12), "wider streetlight reaches the road with unchanged mounting and energy")
	check(street.attenuation == 0.0, "streetlight keeps its wider footprint visible")
	for mobile in [false,true]:
		var renderer := RENDERER.new()
		var sun := DirectionalLight3D.new()
		world.add_child(sun); world.add_child(renderer)
		renderer.configure(Environment.new(),sun,mobile)
		var candidates := [group(3,1,3),group(2,2,4),group(1,1,1),group(0,2,0),group(2,2,2)]
		for lamp: Dictionary in candidates[3].lights: lamp.attenuation = 0.0; lamp.angle_attenuation = 0.25
		renderer._night_lights = true
		renderer._light_candidates = [group(4,1,8).lights[0]]
		renderer.update_dynamic_lights(Vector3.ZERO,candidates)
		check(renderer.lights.size() == (4 if mobile else 8), "platform pool cap")
		check(renderer.lights[0].position.x == 0 and renderer.lights[1].position.x == 0 and renderer.lights[2].position.x == 1, "priority and atomic pair before secondary groups")
		check(renderer.lights[0].spot_attenuation == 0.0 and renderer.lights[1].spot_attenuation == 0.0 and renderer.lights[2].spot_attenuation == 1.0, "per-light attenuation leaves other groups at default")
		check(renderer.lights[0].spot_angle_attenuation == 0.25 and renderer.lights[2].spot_angle_attenuation == 1.0, "per-light cone softness leaves other groups at default")
		if mobile:
			check(renderer.lights[3].position.x == 3, "single remaining slot skips entire pairs")
		else:
			check(renderer.lights[3].position.x == 2 and renderer.lights[4].position.x == 2 and renderer.lights[5].position.x == 4 and renderer.lights[6].position.x == 4, "equal priority sorts by distance and preserves pairs")
		renderer.update_dynamic_lights(Vector3.ZERO,[group(0,2,100)])
		check(renderer.lights.filter(func(light): return light.visible).size()==1 and renderer.lights[0].position.x==8, "out of range group yields slots to static lights")
		check(renderer.lights[0].spot_attenuation == 1.0, "static light resets attenuation when reusing a slot")
		check(renderer.lights[0].spot_angle_attenuation == 1.0, "static light resets cone softness when reusing a slot")
		renderer._night_lights = false
		renderer.update_dynamic_lights(Vector3.ZERO,[])
		check(renderer.lights.all(func(light): return not light.visible), "clear hides all pooled lights")
		check(renderer.lights.filter(func(light): return light.shadow_enabled).size() <= 1, "one dynamic shadow")
		renderer.update_dynamic_lights(Vector3.ZERO,[group(0,2,1)])
		check(renderer.lights[0].visible and renderer.lights[1].visible and not renderer.lights[2].visible, "reentry reuses pool")
		for _frame in 3: await process_frame
		renderer.free(); sun.free()
	world.free()
	for _frame in 5: await process_frame
	print("light_group_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)
