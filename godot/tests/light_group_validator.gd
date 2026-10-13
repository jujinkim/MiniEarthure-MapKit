extends SceneTree
const RENDERER := preload("../environment_renderer.gd")
const CHUNKS := preload("../chunk_renderer.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const PLAN := preload("../render_memory.gd")
const LAMP := {"asset_id":"fixture:lamp", "bulb_materials":[0], "position_cm":[0,312,0], "range_cm":1200, "color":[255,218,165]}
class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes=amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed=true
	func retired() -> bool: return sealed and refs.all(func(ref): return ref.get_ref()==null)
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
	CHUNKS._environment_lamp({"root":cell,"lamp_bindings":CHUNKS._lamp_bindings({"lights":[LAMP]},{"fixture:lamp":true})}, "fixture:lamp", {"quarter_turns":0,"position":[0,0,0]})
	var street: Dictionary = cell.get_meta("environment_lamps")[0]
	check(is_equal_approx(tan(deg_to_rad(street.angle))/tan(deg_to_rad(48.0)), 3.0), "streetlight ground footprint is three times as wide")
	check(is_equal_approx(street.range, 36.0) and street.energy == 2.0 and is_equal_approx(street.position.y, 3.12), "wider streetlight reaches the road with unchanged mounting and energy")
	check(street.attenuation == 0.0, "streetlight keeps its wider footprint visible")
	await cell_lamps(world)
	for level in [2,0]:
		var renderer := RENDERER.new()
		var sun := DirectionalLight3D.new()
		world.add_child(sun); world.add_child(renderer)
		renderer.configure(Environment.new(),sun,level==0)
		renderer.apply_display_quality(QUALITY.profile(level))
		var candidates := [group(3,1,3),group(2,2,4),group(1,1,1),group(0,2,0),group(2,2,2)]
		for lamp: Dictionary in candidates[3].lights: lamp.attenuation = 0.0; lamp.angle_attenuation = 0.25
		renderer._night_lights = true
		renderer._light_candidates = [group(4,1,8).lights[0]]
		renderer.update_dynamic_lights(Vector3.ZERO,candidates)
		check(renderer.lights.size() == 8, "bounded reusable light pool")
		check(renderer.lights.filter(func(light): return light.visible).size()==int(QUALITY.profile(level).light_count), "quality limits the active pool")
		check(renderer.lights[0].position.x == 0 and renderer.lights[1].position.x == 0 and renderer.lights[2].position.x == 1, "priority and atomic pair before secondary groups")
		check(renderer.lights[0].spot_attenuation == 0.0 and renderer.lights[1].spot_attenuation == 0.0 and renderer.lights[2].spot_attenuation == 1.0, "per-light attenuation leaves other groups at default")
		check(renderer.lights[0].spot_angle_attenuation == 0.25 and renderer.lights[2].spot_angle_attenuation == 1.0, "per-light cone softness leaves other groups at default")
		if level==0:
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

func cell_lamps(world: Node3D) -> void:
	for transparent in [false,true]:
		var node := MeshInstance3D.new();node.mesh=BoxMesh.new()
		var material := StandardMaterial3D.new()
		if transparent: material.transparency=BaseMaterial3D.TRANSPARENCY_ALPHA
		node.mesh.material=material
		var document := GLTFDocument.new();var state := GLTFState.new()
		check(document.append_from_scene(node,state)==OK,"synthetic lamp GLB export")
		var data := document.generate_buffer(state)
		node.free();node=null;material=null;document=null;state=null
		var asset := {"path":"lamp.glb","bytes":data,"material_json":"null","content_hash":data.hex_encode().sha256_text(),"memory_bytes":1048576,"solid":true}
		var lamp := LAMP.duplicate(true);lamp.position_cm=[100,312,200]
		var unlit := lamp.duplicate(true);unlit.asset_id="fixture:unlit";unlit.bulb_materials=[]
		var unused := lamp.duplicate(true);unused.asset_id="fixture:unused"
		var chunk := {"cell":{"x":0,"y":0},"triangles":[],"objects":[
			{"id":"first","asset_id":"fixture:lamp","position":[0,0,0],"quarter_turns":0},
			{"id":"second","asset_id":"fixture:lamp","position":[1000,500,2000],"quarter_turns":1,"yaw_offset_mdeg":45000},
			{"id":"unlit","asset_id":"fixture:unlit","position":[0,0,0],"quarter_turns":0}],
			"presentation":{"assets":{"fixture:lamp":asset,"fixture:unlit":asset},"environment_json":JSON.stringify({"lights":[lamp,unlit,unused]})}}
		var cache := CACHE.new(func(bytes): return Lease.new(bytes));cache.quality_profile=QUALITY.profile(0)
		var leases: Array = []
		var reserve := func(bytes):
			var lease := Lease.new(bytes);leases.append(lease);return lease
		var planned := PLAN.estimate(chunk,PLAN.shared_bytes(chunk))
		var first := CHUNKS.begin(chunk,world,reserve,planned,cache)
		check(first.error.is_empty() and first.lamp_bindings.keys()==["fixture:lamp"],"only placed assets with bulbs allocate lamp records")
		var snapshot: Dictionary=first.lamp_bindings["fixture:lamp"]
		check(snapshot.is_read_only(),"shared placement descriptor cannot be changed in place")
		cache.environment_profile.lights[0].position_cm[0]=999
		cache.environment_profile.lights[0].color[0]=0
		var dark := chunk.duplicate(true);dark.presentation.environment_json=JSON.stringify({"lights":[]})
		var second := CHUNKS.begin(dark,world,reserve,planned,cache)
		var steps := 0
		while (not first.done or not second.done) and steps<200:
			CHUNKS.advance(first);CHUNKS.advance(second);steps+=1
		check(steps<200 and first.error.is_empty() and second.error.is_empty(),"interleaved cells complete with their own lamp profiles")
		check(not first.has("lamp_bindings") and not second.has("lamp_bindings"),"completed jobs discard lamp preparation records")
		var lamps: Array=first.root.get_meta("environment_lamps",[])
		check(lamps.size()==2 and not second.root.has_meta("environment_lamps"),"profile replacement cannot add or remove another cell's lamps")
		if lamps.size()==2:
			check(lamps[0].position.is_equal_approx(Vector3(1,3.12,-2)),"authored offset survives nested source edits")
			check(lamps[1].position.is_equal_approx(Vector3(10-3/sqrt(2.0),8.12,-20+1/sqrt(2.0))),"quarter-turn plus fine yaw preserves mounted lamp coordinates")
			check(lamps[0].color==Color(1,218.0/255.0,165.0/255.0) and lamps[0].range==36.0 and lamps[0].basis==Basis.looking_at(Vector3.DOWN,Vector3.FORWARD),"cached color, range and downward basis match authored values")
			lamps[0].energy=0.0
			check(lamps[1].energy==2.0 and snapshot.energy==2.0,"placements own independent lamp records")
		check((first.root.find_children("*","MultiMeshInstance3D",true,false).size()>0)==not transparent,"both instanced and transparent fallback paths exercise lamp placement")
		var partial := CHUNKS.begin(chunk,world,reserve,planned,cache)
		steps=0
		while not partial.root.has_meta("environment_lamps") and not partial.done and steps<200:
			CHUNKS.advance(partial);steps+=1
		check(steps<200 and not partial.done,"cancel after a lamp placement while objects remain")
		CHUNKS.cancel(partial)
		check(not partial.has("lamp_bindings") and partial.claims.is_empty() and CHUNKS.advance(partial),"cancellation clears lamp scratch and ignores late attachment")
		CHUNKS.cancel(first);CHUNKS.cancel(second);cache.shutdown()
		for frame in 3: await process_frame
		check(cache.bytes()==0 and leases.all(func(lease): return lease.retired()),"lamp descriptors retain no resources after cell retirement")
