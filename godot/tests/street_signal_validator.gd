extends SceneTree
## Traffic signals and bus shelters (street_signal_meshes.gd, chunk_renderer):
## signal poles inside the unchanged pole box (everything below 0.6 m) with
## heads where the earlier renderer drew them (three lenses on both faces, red
## at the arm tip or on top for either driving side, visors, backplate, a
## pedestrian signal), shelters inside their proxies' envelope with posts,
## roof and back wall where their collision boxes are, near/far tier variants,
## ranges and shadows (glass: one node, always drawn, no shadows), hidden
## proxy faces with untouched collision triangles (prepared and unprepared
## paths, owner and neighbour cells), the lamp phase per quarter turn in the
## instance custom data, the shared set's memory, a refused set failing the
## cell before any upload, and release of every cell and shared reservation.
## Rendered runs also compare lit and unlit lenses of two planned heads a
## quarter turn apart (one with a 45 degree offset) and the shelter poster by
## night and day.
const RENDER := preload("../chunk_renderer.gd")
const SIGNALS := preload("../street_signal_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const ENVIRONMENT := preload("../environment_materials.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount := 0) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	var set := meshes()
	signals(set)
	shelters(set)
	policy()
	await renderer()
	await lights()
	QUALITY.current = {}
	print("street_signal_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

static func triangles(mesh: ArrayMesh) -> int:
	var total := 0
	for surface in mesh.get_surface_count(): total += mesh.surface_get_array_index_len(surface) / 3
	return total

static func points(mesh: ArrayMesh, surface := -1) -> PackedVector3Array:
	var result := PackedVector3Array()
	for index in mesh.get_surface_count():
		if surface < 0 or index == surface: result.append_array(mesh.surface_get_arrays(index)[Mesh.ARRAY_VERTEX])
	return result

func meshes() -> Dictionary:
	var lease := Lease.new()
	var state := {}
	var steps := 1
	while not SIGNALS.build(state, lease): steps += 1
	check(steps == SIGNALS.STEPS.size(), "one part per call: %d steps" % steps)
	var set: Dictionary = state.result
	var again := {}
	while not SIGNALS.build(again): pass
	var vertices := 0
	var indices := 0
	var counts := []
	for step: String in SIGNALS.STEPS:
		if step.begins_with("material_"): continue
		var mesh: ArrayMesh = set[step]
		var other: ArrayMesh = again.result[step]
		for surface in mesh.get_surface_count():
			check(mesh.surface_get_arrays(surface) == other.surface_get_arrays(surface), "%s builds identically" % step)
			vertices += mesh.surface_get_array_len(surface)
			indices += mesh.surface_get_array_index_len(surface)
		counts.append("%s %d" % [step, triangles(mesh)])
	print("signal and shelter triangles: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared signal set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, SIGNALS.SHARED_BYTES])
	check(mesh_bytes + 65536 <= SIGNALS.SHARED_BYTES, "the shared set's meshes, materials and build arrays fit its allowance: %d B" % mesh_bytes)
	check(lease.refs.size() == SIGNALS.STEPS.size() + 1, "the lease tracks every mesh, material and the lens shader: %d" % lease.refs.size())
	return set

## Lens centres of a mesh's lens surface by role: [centre, normal] pairs.
static func lens_centres(mesh: ArrayMesh) -> Dictionary:
	var arrays: Array = mesh.surface_get_arrays(1)
	var result := {}
	for index in arrays[Mesh.ARRAY_VERTEX].size():
		var color: Color = arrays[Mesh.ARRAY_COLOR][index]
		var role := roundi(color.r * 4.0)
		# Domed vehicle lenses: the centre vertex; pedestrian squares: corner (0, 0).
		if (role <= SIGNALS.GREEN and color.g < 0.01) or (role > SIGNALS.GREEN and color.g < 0.01 and color.b < 0.01):
			if not result.has(role): result[role] = []
			result[role].append([arrays[Mesh.ARRAY_VERTEX][index], arrays[Mesh.ARRAY_NORMAL][index]])
	return result

func signals(set: Dictionary) -> void:
	for kind in ["signal_h", "signal_v"]:
		for lod in SIGNALS.LODS:
			var mesh: ArrayMesh = set[kind + "_" + lod]
			var label := "%s %s" % [kind, lod]
			check(mesh.get_surface_count() == 2, label + " has paint and lens surfaces")
			var base := 0
			var top := 0.0
			var reach := 0.0
			for point: Vector3 in points(mesh):
				if point.y < 0.6 and not SIGNALS.SIGNAL_POLE.grow(0.0015).has_point(point): base += 1
				top = maxf(top, point.y)
				reach = maxf(reach, point.x)
			check(base == 0, label + " stands inside the 8 cm pole box below 0.6 m: %d vertices outside" % base)
			check(top <= 2.2015 and top >= 2.1, label + " ends at the 2.2 m pole top: %.3f" % top)
			if kind == "signal_h": check(reach >= 1.5 and reach <= 1.57, label + " reaches over the road to the head: %.3f" % reach)
			else: check(reach <= 0.12, label + " keeps its head on the pole: %.3f" % reach)
		var near: ArrayMesh = set[kind + "_near"]
		var far: ArrayMesh = set[kind + "_far"]
		check(triangles(near) >= 6 * triangles(far) and triangles(far) <= 400, "%s far is a simple variant: %d vs %d" % [kind, triangles(far), triangles(near)])
		# Vehicle lenses: red, amber, green on both faces around the head
		# centre (the importer turns -z toward the approaching traffic under
		# right-hand driving, +z under left-hand driving); red at the arm-tip
		# (road-centre) end of a horizontal head, on top of a vertical one.
		var centre: Vector3 = SIGNALS.H_HEAD if kind == "signal_h" else SIGNALS.V_HEAD
		for lod in SIGNALS.LODS:
			var by_role := lens_centres(set[kind + "_" + lod])
			for role in [SIGNALS.RED, SIGNALS.AMBER, SIGNALS.GREEN]:
				var found: Array = by_role.get(role, [])
				var sides := {}
				for item: Array in found:
					var at: Vector3 = item[0]
					var normal: Vector3 = item[1]
					sides[signf(normal.z)] = true
					check(absf(normal.z) > 0.99 and (at - centre).length() <= 0.15, "%s %s lens %d faces along z on its head: %s" % [kind, lod, role, at])
					if role == SIGNALS.RED:
						check(at.x > centre.x + 0.1 if kind == "signal_h" else at.y > centre.y + 0.1, "%s %s red lens at the arm tip or on top of either face: %s" % [kind, lod, at])
				check(found.size() == 2 and sides.size() == 2, "%s %s lights role %d on both faces: %d" % [kind, lod, role, found.size()])
		var lenses := lens_centres(near)
		for role in [SIGNALS.STOP, SIGNALS.WALK]:
			var found: Array = lenses.get(role, [])
			check(found.size() == 1 and (found[0][1] as Vector3).x > 0.99, "%s pedestrian lens %d faces across the road" % [kind, role])
		# Visors: paint reaching out of both faces over the lenses (shorter on
		# the pole side of a vertical head), clear of the pole.
		for s: float in [-1.0, 1.0]:
			var hooded := 0
			for point: Vector3 in points(near, 0):
				var out := (point.z - centre.z) * s
				if out > 0.038 and out < 0.09 and absf(point.x - centre.x) < 0.25 and absf(point.y - centre.y) < 0.24: hooded += 1
			check(hooded >= 3 * 6, "%s visors reach out of face %+.0f: %d vertices" % [kind, s, hooded])
		if kind == "signal_v":
			var touching := 0
			for point: Vector3 in points(near, 0):
				var radial := Vector2(point.x, point.z).length()
				if absf(point.y - centre.y) < 0.24 and point.z > -0.034 and radial < 0.06 and radial > 0.032: touching += 1
			check(touching == 0, "signal_v pole-side visors stay clear of the pole: %d vertices" % touching)
	# Heads where the earlier renderer drew them.
	var h_lens := AABB(SIGNALS.H_HEAD, Vector3.ZERO)
	for point: Vector3 in points(set.signal_h_near, 1):
		if point.x > 0.5: h_lens = h_lens.expand(point)
	check(absf(h_lens.get_center().x - SIGNALS.H_HEAD.x) < 0.01 and absf(h_lens.get_center().y - SIGNALS.H_HEAD.y) < 0.01, "the horizontal head hangs at %s" % h_lens.get_center())

func shelters(set: Dictionary) -> void:
	for step in ["shelter_near", "shelter_far", "shelter_glass"]:
		var outside := 0
		for point: Vector3 in points(set[step]):
			if not SIGNALS.SHELTER_ENVELOPE.grow(0.0015).has_point(point): outside += 1
		check(outside == 0, "%s stays inside the proxies' envelope: %d vertices outside" % [step, outside])
	# Each collision box holds the part standing in it: posts, roof, back wall.
	for lod in SIGNALS.LODS:
		var paint := points(set["shelter_" + lod], 0)
		for index in SIGNALS.SHELTER_PROXIES.size():
			var box: AABB = SIGNALS.SHELTER_PROXIES[index]
			var inside := 0
			for point: Vector3 in paint:
				if box.grow(0.001).has_point(point): inside += 1
			check(inside >= 8, "shelter %s draws something in collision box %d: %d vertices" % [lod, index, inside])
		var top := 0.0
		for point: Vector3 in paint: top = maxf(top, point.y)
		check(top >= 0.95, "shelter %s roof reaches the roof box: %.3f" % [lod, top])
	check(set.shelter_near.get_surface_count() == 2 and set.shelter_far.get_surface_count() == 2, "shelters carry painted steel and lit signs")
	check(triangles(set.shelter_near) >= 6 * triangles(set.shelter_far) and triangles(set.shelter_far) <= 200, "the far shelter is a simple variant: %d vs %d" % [triangles(set.shelter_far), triangles(set.shelter_near)])
	# Glass: two back bays in the back wall box, two half-depth side screens.
	var glass := points(set.shelter_glass)
	var back := 0
	for point: Vector3 in glass:
		if SIGNALS.SHELTER_PROXIES[1].grow(0.001).has_point(point): back += 1
	check(glass.size() == 32 and back == 16, "glass: two back panes in the back wall and two side screens, each split by the frosted band: %d/%d" % [back, glass.size()])
	# The roof slopes toward the rear gutter.
	var front := 0.0
	var rear := 0.0
	for point: Vector3 in points(set.shelter_near, 0):
		if point.y > 0.93:
			if point.z < -0.2: front = maxf(front, point.y)
			elif point.z > 0.2 and point.z < 0.24: rear = maxf(rear, point.y)
	check(front - rear >= 0.01, "the roof slopes from %.3f at the front to %.3f at the back" % [front, rear])

## Ranges and shadows per tier, live: glass never casts.
func policy() -> void:
	for level in 3:
		var value := QUALITY.profile(level)
		var near := float(value.furniture_near) + QUALITY.GROUP_REACH if level > 0 else 0.0
		for kind in ["signal_h", "shelter", "glass"]:
			for lod in (["far"] if kind == "glass" else SIGNALS.LODS):
				var node := MultiMeshInstance3D.new()
				node.multimesh = MultiMesh.new()
				node.set_meta("mapkit_furniture_lod", lod)
				node.set_meta("mapkit_furniture_kind", "shelter" if kind == "glass" else kind)
				if kind == "glass": node.set_meta("mapkit_furniture_glass", true)
				QUALITY.apply_node(node, value)
				var label := "%s %s tier %d" % [kind, lod, level]
				if kind == "glass": check(node.visible and node.visibility_range_begin == 0.0 and node.visibility_range_end == 0.0, label + " drawn at every distance")
				elif lod == "near": check(node.visible == (level > 0) and (level == 0 or is_equal_approx(node.visibility_range_end, near)), label + " near range")
				else: check(node.visible and is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == 0.0, label + " far range always drawn")
				check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_OFF if kind == "glass" else GeometryInstance3D.SHADOW_CASTING_SETTING_ON), label + " shadows")
				node.free()

## A box's 12 triangles (centimetres, scene axes) for a fake proxy.
static func proxy_box(target: PackedVector3Array, centre: Vector3, size: Vector3) -> void:
	var h := size * 0.005
	var c := centre * 0.01
	var corner := func(x: float, y: float, z: float) -> Vector3: return c + Vector3(h.x * x, h.y * y, h.z * z)
	for axis in 3:
		for s in [-1.0, 1.0]:
			var quad := []
			for a in [-1.0, 1.0]:
				for b in [-1.0, 1.0]:
					var v := [a, b]
					v.insert(axis, s)
					quad.append(corner.call(v[0], v[1], v[2]))
			target.append_array([quad[0], quad[1], quad[3], quad[0], quad[3], quad[2]])

## Ground quad, an owner cell's two horizontal signals (32 m apart, one turned
## a quarter), a vertical signal and a shelter with their proxy boxes, and a
## neighbour signal's clipped pole face (no object).
func fixture(prepared: bool) -> Dictionary:
	var vertices := PackedVector3Array([Vector3(-20, 0, 20), Vector3(20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, -20)])
	var ids := ["ground", "ground"]
	var placed := [["sig-a", "builtin:signal_h", Vector3(300, 0, 400), 0], ["sig-b", "builtin:signal_h", Vector3(3600, 0, 400), 1],
		["sig-v", "builtin:signal_v", Vector3(600, 0, 400), 2], ["stop", "builtin:bus_shelter", Vector3(-500, 0, 300), 0]]
	for item: Array in placed:
		var first := vertices.size()
		var at: Vector3 = item[2]
		var scene := Vector3(at.x, at.y, -at.z)
		if item[1] == "builtin:bus_shelter":
			proxy_box(vertices, scene + Vector3(0, 95, 0), Vector3(160, 4, 50))
			proxy_box(vertices, scene + Vector3(0, 47, 23), Vector3(160, 86, 3))
			for x in [-78, 78]: proxy_box(vertices, scene + Vector3(x, 45, -21), Vector3(4, 90, 4))
		else:
			proxy_box(vertices, scene + Vector3(0, 110, 0), Vector3(8, 220, 8))
		for i in (vertices.size() - first) / 3: ids.append(item[0])
	var neighbour := vertices.size()
	proxy_box(vertices, Vector3(1995, 110, -400), Vector3(8, 220, 8))
	for i in (vertices.size() - neighbour) / 3: ids.append("sig-n")
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": [], "scene_vertices": vertices, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for id: String in ids:
		chunk.triangles.append({"surface": "concrete", "object_id": id})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"proxy_materials": {"sig-a": "builtin:signal_h", "sig-b": "builtin:signal_h", "sig-v": "builtin:signal_v", "stop": "builtin:bus_shelter", "sig-n": "builtin:signal_h"}}
	for item: Array in placed:
		var at: Vector3 = item[2]
		chunk.objects.append({"id": item[0], "asset_id": item[1], "position": [at.x, at.y, at.z], "quarter_turns": item[3]})
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(["sig-a", "sig-b", "sig-v", "stop", "sig-n"].all(func(id): return hidden.has(id)) and not hidden.has("ground"), "owner and neighbour signal and shelter faces are hidden: %s" % [hidden.keys()])
	var without := fixture(false)
	without.triangles = without.triangles.slice(0, 2)
	without.scene_vertices = without.scene_vertices.slice(0, 6)
	check(RENDER.PLAN.estimate(fixture(false), 0) == RENDER.PLAN.estimate(without, 0), "hidden proxy faces add no display reservation")
	var batches := RENDER.PLAN.prepare_batches(fixture(false))
	check(batches.size() == 1 and batches[0].vertices.size() == 6, "prepared batches carry only the ground")
	# The lamp phase (the lens shader's, mirrored by phase()): the parity of
	# the nearest quarter turn of the actual yaw, a yaw exactly halfway going
	# to the next one. Within the importer's ±45 degree offsets it follows the
	# quarter turns; larger editor offsets (up to ±360 degrees) follow the
	# heading. The rendered check compares the shader at 44 and 45 degrees.
	for turns in [-1, 0, 1, 2, 3, 5]:
		for offset in [-200000, -45000, -44999, -5000, 0, 20000, 44999, 45000, 135000, 200000, 360000]:
			var lamp := {"position": [0, 0, 0], "quarter_turns": turns, "yaw_offset_mdeg": offset}
			var expected := 0.5 * float(posmod(floori(float(turns * 90000 + offset + 45000) / 90000.0), 2))
			if absi(offset) < 45000 or offset == -45000: check(expected == 0.5 * float(posmod(turns, 2)), "turns %d offset %d: the quarter turns decide" % [turns, offset])
			check(SIGNALS.phase(lamp) == expected, "turns %d offset %d: phase %.1f, expected %.1f" % [turns, offset, SIGNALS.phase(lamp), expected])
	var planned := {"root": null}
	for object: Dictionary in fixture(false).objects: SIGNALS.plan(planned, object)
	for key: String in planned.signal_plan:
		for part: PackedFloat32Array in planned.signal_plan[key].parts: check(part.size() == 12, "%s instances carry a transform only" % key)
	check(planned.signal_plan.size() == 4, "signals and shelters group per kind and 32 m square: %s" % [planned.signal_plan.keys()])
	for prepared in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(prepared)
			var collision: Array = chunk.triangles.duplicate(true)
			var collision_points: PackedVector3Array = chunk.scene_vertices.duplicate()
			var estimate := RENDER.PLAN.estimate(chunk, 0)
			var lease := Lease.new(estimate)
			var job := RENDER.begin(chunk, parent, func(bytes): return lease if bytes == estimate else null, estimate, cache)
			while not RENDER.advance(job): pass
			var label := "%s tier %d" % ["prepared" if prepared else "unprepared", level]
			check(job.error.is_empty(), label + " attaches: %s" % job.error)
			check(chunk.triangles == collision and chunk.scene_vertices == collision_points, label + " leaves the collision triangles untouched")
			var drawn := 0
			for mesh: MeshInstance3D in parent.find_children("*", "MeshInstance3D", true, false):
				if mesh.mesh is ArrayMesh: drawn += mesh.mesh.surface_get_array_len(0)
			check(drawn == 6, label + " draws no proxy faces of signals or shelters: %d vertices" % drawn)
			var counts := {}
			var made: Dictionary = cache.mesh_set(SIGNALS.KEY)
			for node: MultiMeshInstance3D in parent.find_children("Signals_*", "MultiMeshInstance3D", true, false):
				var kind: String = node.get_meta("mapkit_furniture_kind")
				var key := "%s_%s%s" % [kind, node.get_meta("mapkit_furniture_lod"), "_glass" if node.get_meta("mapkit_furniture_glass", false) else ""]
				counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
				check(made.values().has(node.multimesh.mesh), label + " borrows the shared mesh")
				check(node.get_meta("mapkit_static_detail", false) and not node.multimesh.use_custom_data, label + " nodes carry static detail metadata")
				var ids: PackedStringArray = node.get_meta("mapkit_object_ids", PackedStringArray())
				check(ids.size() == node.multimesh.instance_count and Array(ids).all(func(id): return id in ["sig-a", "sig-b", "sig-v", "stop"]), label + " nodes list their instances' object ids: %s" % ids)
				if level == 0: check(node.visible == (node.get_meta("mapkit_furniture_lod") == "far"), label + " low draws far meshes only")
			check(counts == {"signal_h_near": 2, "signal_h_far": 2, "signal_v_near": 1, "signal_v_far": 1, "shelter_near": 1, "shelter_far": 1, "shelter_far_glass": 1},
				label + " instances two horizontal and one vertical signal and a glazed shelter near and far: %s" % counts)
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), label + " cell lease retires with its signal nodes")
	check(cache.bytes() >= SIGNALS.SHARED_BYTES, "one shared signal set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared signal set retires after its last cell")
	# Without a resource cache (standalone previews) the process-lifetime set.
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(standalone.find_children("Signals_*", "MultiMeshInstance3D", true, false).size() == 9, "standalone previews draw signals from the process-lifetime set")
	preview.free()
	# A refused shared set fails the cell as it begins, before any upload:
	# its signals would have no body.
	var denied := CACHE.new(func(bytes): return null if bytes == SIGNALS.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and parent.get_child_count() == 0, "a refused signal set fails the cell before any upload: %s" % job.get("error", {}))
	while not RENDER.advance(job): pass
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET", "a refused signal set fails the cell: %s" % job.get("error", {}))
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Cache-built materials; rendered runs compare two instanced vertical heads a
## quarter turn apart in one frame (exactly one red lens lit, the other's
## green or amber), the instanced paint's vertex tint, and the shelter poster
## by night and day in an unlit scene.
func lights() -> void:
	var context := ENVIRONMENT.new()
	var state := {}
	while not SIGNALS.build(state, null, context): pass
	var made: Dictionary = state.result
	var sign: ShaderMaterial = made.materials.sign
	var paint: ShaderMaterial = made.materials.paint
	check(sign.get_shader_parameter("lighting_role") == 2 and sign.get_shader_parameter("vertex_tinted"), "signs and posters glow at night through the lamp role")
	check(paint.get_shader_parameter("lighting_role") == 0 and paint.get_shader_parameter("vertex_tinted") and paint.get_meta("mapkit_surface", false), "painted steel reads vertex tints and weather")
	check(made.materials.lens is ShaderMaterial and made.materials.glass is StandardMaterial3D and made.materials.glass.transparency == BaseMaterial3D.TRANSPARENCY_ALPHA, "cycling lenses and translucent glass")
	if DisplayServer.get_name() == "headless":
		print("street_signal_validator: headless, lens pixels skipped")
		return
	root.size = Vector2i(320, 240)
	var scene := Node3D.new()
	root.add_child(scene)
	var world := WorldEnvironment.new()
	world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_energy = 0.0
	scene.add_child(world)
	var sun := DirectionalLight3D.new()
	sun.rotation = Vector3(0.0, PI, 0.0)
	sun.visible = false
	scene.add_child(sun)
	# Head A at the origin turned 44 degrees (even: the nearest quarter turn
	# is 0), seen along its -z by the root camera; head B 10 m away turned
	# exactly 45 degrees (the tie goes to the odd quarter turn), seen along its
	# -z by a second camera in a sub-viewport of the same world: both in one
	# frame, planned and attached as the renderer does.
	var head_a := {"id": "a", "asset_id": "builtin:signal_v", "position": [0, 0, 0], "quarter_turns": 0, "yaw_offset_mdeg": 44000}
	var head_b := {"id": "b", "asset_id": "builtin:signal_v", "position": [1000, 0, 0], "quarter_turns": 0, "yaw_offset_mdeg": 45000}
	check(SIGNALS.phase(head_a) == 0.0 and SIGNALS.phase(head_b) == 0.5, "44 and 45 degree heads run half a cycle apart")
	var place_a := SIGNALS.F.placement(head_a)
	var place_b := SIGNALS.F.placement(head_b)
	var heads := Node3D.new()
	scene.add_child(heads)
	var group := {"root": heads, "signals": made}
	SIGNALS.plan(group, head_a)
	SIGNALS.plan(group, head_b)
	SIGNALS.attach_group(group)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 2.4
	scene.add_child(camera)
	camera.position = place_a * Vector3(0.0, 1.1, -3.0)
	camera.look_at(place_a * Vector3(0.0, 1.1, 0.0), Vector3.UP)
	camera.current = true
	var side := SubViewport.new()
	side.size = root.size
	side.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	scene.add_child(side)
	var other := Camera3D.new()
	other.projection = Camera3D.PROJECTION_ORTHOGONAL
	other.size = 2.4
	side.add_child(other)
	other.position = place_b * Vector3(0.0, 1.1, -3.0)
	other.look_at(place_b * Vector3(0.0, 1.1, 0.0), Vector3.UP)
	other.current = true
	for frame in 6: await process_frame
	await RenderingServer.frame_post_draw
	var pictures := [root.get_texture().get_image(), side.get_texture().get_image()]
	if not OS.get_environment("SIGNAL_DEBUG_PNG").is_empty(): pictures[0].save_png(OS.get_environment("SIGNAL_DEBUG_PNG"))
	var lit := []
	for index in 2:
		var row := []
		var view: Camera3D = camera if index == 0 else other
		var place := place_a if index == 0 else place_b
		for role in 3:
			var at := place * Vector3(0.0, SIGNALS.V_HEAD.y + 0.135 * float(1 - role), SIGNALS.V_HEAD.z - 0.031)
			row.append((pictures[index] as Image).get_pixelv(Vector2i(view.unproject_position(at))))
		lit.append(row)
	# Painted parts keep their vertex tint when instanced (lit from the
	# front): the yellow button (its most yellow pixel around the projected
	# case: about a pixel wide in this view) against the grey pole.
	sun.visible = true
	for frame in 4: await process_frame
	await RenderingServer.frame_post_draw
	var front := root.get_texture().get_image()
	if not OS.get_environment("SIGNAL_DEBUG_PNG").is_empty(): front.save_png(OS.get_environment("SIGNAL_DEBUG_PNG").replace(".png", "-front.png"))
	var case := Vector2i(camera.unproject_position(place_a * Vector3(-0.034, 0.36, -0.023)))
	var button := Color.BLACK
	for dx in range(-2, 3):
		for dy in range(-3, 4):
			var c := front.get_pixelv(case + Vector2i(dx, dy))
			if c.r - c.b > button.r - button.b: button = c
	var pole: Color = front.get_pixelv(Vector2i(camera.unproject_position(place_a * Vector3(0.0, 1.2, -0.027))))
	print("SIGNAL_LENSES ", lit, " button=", button, " pole=", pole)
	check(button.r > 0.2 and button.r > 1.6 * button.b and pole.r < 1.3 * pole.b + 0.02, "instanced paint keeps its vertex tints: button %s, pole %s" % [button, pole])
	# Lit lenses emit 0.41 (green) to 0.9 (red) in this untonemapped scene;
	# unlit ones stay black.
	var bright := func(c: Color) -> bool: return maxf(c.r, maxf(c.g, c.b)) > 0.3
	var reds := 0
	for index in 2:
		var row: Array = lit[index]
		var red: bool = bright.call(row[0]) and row[0].r > 2.0 * row[0].g
		reds += int(red)
		var others := int(bright.call(row[1])) + int(bright.call(row[2]))
		check((red and others == 0) or (not red and others == 1), "one lamp of head %d is lit: %s" % [index, row])
	check(reds == 1, "a quarter turn runs half a cycle apart: exactly one head shows red: %s" % [lit])
	side.queue_free()
	sun.queue_free()
	heads.queue_free()
	# The poster glows at night and is dark by day in an unlit scene.
	var shelter := MeshInstance3D.new()
	shelter.mesh = made.shelter_near
	scene.add_child(shelter)
	camera.size = 0.3
	camera.position = Vector3(0.52, 0.6, -1.0)
	camera.look_at(Vector3(0.52, 0.6, 0.23), Vector3.UP)
	var samples := []
	for altitude in [-0.2, 0.8]:
		context.update(72000.0 if altitude < 0.0 else 43200.0, 0.0, 0.0, altitude, 1080)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var image := root.get_texture().get_image()
		samples.append(image.get_pixelv(Vector2i(camera.unproject_position(Vector3(0.52, 0.6, 0.2172)))))
	print("SHELTER_POSTER night=", samples[0], " day=", samples[1])
	check(maxf(samples[0].r, maxf(samples[0].g, samples[0].b)) > 0.15, "the poster glows at night: %s" % samples[0])
	check(maxf(samples[1].r, maxf(samples[1].g, samples[1].b)) < 0.1, "the poster is dark by day in an unlit scene: %s" % samples[1])
	scene.queue_free()
	for frame in 3: await process_frame
