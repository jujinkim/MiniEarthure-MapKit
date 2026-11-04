extends SceneTree
## Street furniture (street_furniture_meshes.gd, chunk_renderer): streetlight
## meshes inside the unchanged collision boxes and with the proxy's
## proportions (a base filling the pole box at car height), bench meshes inside
## the 40 cm footprint with slats, gaps and baked grain, near/far tier variants,
## per-kind ranges, shadows and fade modes, the night lens, hidden proxy faces
## with untouched collision triangles (prepared, unprepared and packed chunks,
## owner and neighbour cells), instance placement against the proxy boxes and
## the anchor convention, object ids for editor picking, the shared set's
## memory, an early refusal (failing a cell with streetlights, skipping benches
## otherwise), and release of every cell and shared reservation. Rendered runs
## also compare the lens by night and day.
const RENDER := preload("../chunk_renderer.gd")
const FURNITURE := preload("../street_furniture_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const ENVIRONMENT := preload("../environment_materials.gd")
var failed := false
## A grained slat: 2 end faces, 2 wide faces in 3 streaks and 2 narrow faces
## in 1, each of 3 segments along x, 12 bevel edges and 8 corners.
const SLAT_TRIANGLES := 2 * 2 + 2 * 3 * 3 * 2 + 2 * 3 * 2 + 12 * 2 + 8

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
	policy()
	await renderer()
	await night(set)
	QUALITY.current = {}
	print("street_furniture_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

static func triangles(mesh: ArrayMesh, surface: int) -> int:
	return mesh.surface_get_array_index_len(surface) / 3

func meshes() -> Dictionary:
	var lease := Lease.new()
	var job := {}
	var steps := 0
	while not FURNITURE.build(job, lease): steps += 1
	check(steps == FURNITURE.STEPS.size() - 1, "one part per call: %d steps" % (steps + 1))
	var set: Dictionary = job.result
	var again := {}
	while not FURNITURE.build(again): pass
	var vertices := 0
	var indices := 0
	var counts := []
	for kind in ["streetlight", "bench"]:
		for lod in FURNITURE.LODS:
			var mesh: ArrayMesh = set[kind + "_" + lod]
			var other: ArrayMesh = again.result[kind + "_" + lod]
			check(mesh.get_surface_count() == 2, "%s %s has two surfaces" % [kind, lod])
			var total := 0
			for surface in mesh.get_surface_count():
				check(mesh.surface_get_arrays(surface) == other.surface_get_arrays(surface), "%s %s builds identically" % [kind, lod])
				vertices += mesh.surface_get_array_len(surface)
				indices += mesh.surface_get_array_index_len(surface)
				total += triangles(mesh, surface)
			counts.append("%s %s %d triangles" % [kind, lod, total])
	print("street furniture: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared street furniture set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, FURNITURE.SHARED_BYTES])
	check(mesh_bytes + 65536 <= FURNITURE.SHARED_BYTES, "the shared set's meshes, materials and build arrays fit its allowance: %d B" % mesh_bytes)
	# Streetlights: every vertex inside the collision boxes; the proxy's 5 m
	# pole and 40 cm reach; the lens under each head facing down.
	for lod in FURNITURE.LODS:
		var mesh: ArrayMesh = set["streetlight_" + lod]
		var outside := 0
		var reach := 0.0
		var top := 0.0
		for surface in 2:
			for point: Vector3 in mesh.surface_get_arrays(surface)[Mesh.ARRAY_VERTEX]:
				if not FURNITURE.STREETLIGHT_PROXIES.any(func(box: AABB) -> bool: return box.grow(0.0015).has_point(point)): outside += 1
				reach = maxf(reach, absf(point.x))
				top = maxf(top, point.y)
		check(outside == 0, "streetlight %s stays inside its collision boxes: %d vertices outside" % [lod, outside])
		check(reach >= 0.38 and top >= 5.05 and top <= 5.125, "streetlight %s reaches %.2f m at %.2f m like its proxy" % [lod, reach, top])
		# At car height (10-75 cm) the base fills the 20 cm pole box to within
		# 6 mm on every side (15 mm at the bevelled corners).
		var faces := mesh.get_faces()
		for direction: Vector3 in [Vector3.RIGHT, Vector3.LEFT, Vector3.BACK, Vector3.FORWARD, Vector3(1, 0, 1).normalized(), Vector3(-1, 0, -1).normalized()]:
			for height in [0.1, 0.4, 0.75]:
				var box_edge := 0.1 / maxf(absf(direction.x), absf(direction.z))
				var hit := 0.0
				for index in range(0, faces.size(), 3):
					var at: Variant = Geometry3D.ray_intersects_triangle(Vector3(0, height, 0) + direction * 0.5, -direction, faces[index], faces[index + 1], faces[index + 2])
					if at != null: hit = maxf(hit, Vector2((at as Vector3).x, (at as Vector3).z).length())
				check(hit >= box_edge - (0.006 if direction.x * direction.z == 0.0 else 0.015), "streetlight %s base fills its box at %.2f m towards %s: %.4f of %.4f" % [lod, height, direction, hit, box_edge])
		var lens: Array = mesh.surface_get_arrays(1)
		var down := 0
		for index in lens[Mesh.ARRAY_VERTEX].size():
			if lens[Mesh.ARRAY_NORMAL][index].y < -0.99 and lens[Mesh.ARRAY_VERTEX][index].y > 4.98: down += 1
		check(down == 8, "streetlight %s has a downward lens under each head: %d" % [lod, down])
	check(triangles(set.streetlight_near, 0) >= 6 * triangles(set.streetlight_far, 0) and triangles(set.streetlight_far, 0) <= 120, "the far streetlight is a simple variant")
	# A quarter turn turns the head with its collision box.
	var turned := FURNITURE.placement({"position": [0, 0, 0], "quarter_turns": 1})
	var head: AABB = turned * set.streetlight_near.get_aabb()
	check(head.size.z >= 0.76 and head.size.x <= 0.2, "a quarter-turned streetlight spans the turned head box: %s" % head.size)
	# Benches: inside the 40 cm footprint, 0.6 m wide, four seat slats with
	# gaps, three backrest slats, the far variant simple.
	for lod in FURNITURE.LODS:
		var mesh: ArrayMesh = set["bench_" + lod]
		var wide := 0.0
		var outside := 0
		for surface in 2:
			for point: Vector3 in mesh.surface_get_arrays(surface)[Mesh.ARRAY_VERTEX]:
				if Vector2(point.x, point.z).length() > FURNITURE.BENCH_RADIUS or point.y < -0.0001 or point.y > 0.4: outside += 1
				wide = maxf(wide, absf(point.x))
		check(outside == 0 and wide >= 0.29 and wide <= 0.3, "bench %s stays in its footprint and is 0.6 m wide: %d outside, %.3f" % [lod, outside, wide])
	# Seat slat tops (y 0.198, facing up): four slats with >= 8 mm gaps, each
	# in three lengthwise grain streaks with darker ends.
	var wood: Array = set.bench_near.surface_get_arrays(0)
	var tops := []
	var streaks := {}
	var middle := 0.0
	var ends := 0.0
	for index in wood[Mesh.ARRAY_VERTEX].size():
		var point: Vector3 = wood[Mesh.ARRAY_VERTEX][index]
		if wood[Mesh.ARRAY_NORMAL][index].y < 0.99 or absf(point.y - 0.198) > 0.001: continue
		if not tops.has(snappedf(point.z, 0.0001)): tops.append(snappedf(point.z, 0.0001))
		var tint: Color = wood[Mesh.ARRAY_COLOR][index]
		if absf(point.x) < 0.26:
			middle = maxf(middle, tint.get_luminance())
			streaks[snappedf(tint.get_luminance(), 0.0001)] = true
		elif absf(point.x) > 0.28: ends = maxf(ends, tint.get_luminance())
	tops.sort()
	# Uncovered runs of the seat top across z (0.5 mm samples): the gaps.
	var spans := []
	var index_array: PackedInt32Array = wood[Mesh.ARRAY_INDEX]
	for index in range(0, index_array.size(), 3):
		var corners := [wood[Mesh.ARRAY_VERTEX][index_array[index]], wood[Mesh.ARRAY_VERTEX][index_array[index + 1]], wood[Mesh.ARRAY_VERTEX][index_array[index + 2]]]
		if corners.all(func(point: Vector3) -> bool: return absf(point.y - 0.198) < 0.001) and wood[Mesh.ARRAY_NORMAL][index_array[index]].y > 0.99:
			spans.append(Vector2(minf(corners[0].z, minf(corners[1].z, corners[2].z)), maxf(corners[0].z, maxf(corners[1].z, corners[2].z))))
	var gaps := []
	var run := 0
	for step in int((tops[-1] - tops[0]) / 0.0005):
		var z: float = tops[0] + (float(step) + 0.5) * 0.0005
		if spans.any(func(span: Vector2) -> bool: return z >= span.x and z <= span.y):
			if run > 0: gaps.append(float(run) * 0.0005)
			run = 0
		else: run += 1
	check(gaps.size() == FURNITURE.SEAT_SLATS - 1 and gaps.all(func(gap: float) -> bool: return gap >= 0.008), "four seat slats with gaps: %s" % [gaps])
	check(streaks.size() >= 3 * FURNITURE.SEAT_SLATS - 2, "seat slats carry lengthwise grain streaks: %d tints" % streaks.size())
	check(ends > 0.0 and ends < middle * 0.92, "slat ends are darker: %.4f of %.4f" % [ends, middle])
	check(triangles(set.bench_near, 0) == (FURNITURE.SEAT_SLATS + FURNITURE.BACK_SLATS) * SLAT_TRIANGLES and triangles(set.bench_near, 1) >= 600, "bevelled, grained seat and backrest slats on detailed frames: %d" % triangles(set.bench_near, 0))
	check(triangles(set.bench_far, 0) + triangles(set.bench_far, 1) <= 48, "the far bench is a simple variant")
	return set

## Ranges and shadows per tier, live; fade modes per rendering method.
func policy() -> void:
	check(QUALITY.range_fade("forward_plus") == GeometryInstance3D.VISIBILITY_RANGE_FADE_SELF, "Forward+ crossfades near and far meshes")
	for method in ["mobile", "gl_compatibility"]:
		check(QUALITY.range_fade(method) == GeometryInstance3D.VISIBILITY_RANGE_FADE_DISABLED, method + " hands over by hysteresis")
	for level in 3:
		var value := QUALITY.profile(level)
		for kind in ["streetlight", "bench"]:
			var near := float(value[kind + "_near"]) + QUALITY.FURNITURE_REACH if level > 0 else 0.0
			check(level == 0 or near <= (32.0 if kind == "bench" else 44.0), "%s detail ends near: %.1f m" % [kind, near])
			for lod in FURNITURE.LODS:
				var node := MultiMeshInstance3D.new()
				node.multimesh = MultiMesh.new()
				node.set_meta("mapkit_furniture_lod", lod)
				node.set_meta("mapkit_furniture_kind", kind)
				node.set_meta("mapkit_decoration", kind == "bench")
				QUALITY.apply_node(node, value)
				var label := "%s %s tier %d" % [kind, lod, level]
				check(node.visibility_range_fade_mode == QUALITY.range_fade(RenderingServer.get_current_rendering_method()), label + " fade mode")
				if lod == "near":
					check(node.visible == (level > 0) and (level == 0 or is_equal_approx(node.visibility_range_end, near)), label + " near range")
				else:
					check(is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == (float(value.decoration_distance) if kind == "bench" else 0.0), label + " far range")
				var shadows: bool = level >= 1 if kind == "streetlight" else (lod == "near" and level == 2)
				check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), label + " shadows")
				node.free()

## A box's 12 triangles (centimetres, scene axes) for a fake proxy.
static func proxy_box(points: PackedVector3Array, centre: Vector3, size: Vector3) -> void:
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
			points.append_array([quad[0], quad[1], quad[3], quad[0], quad[3], quad[2]])

## Ground quad, an owner cell's streetlight (two proxy boxes), a neighbour
## streetlight's clipped head face (no object) and three benches in two groups.
func fixture(prepared: bool) -> Dictionary:
	var points := PackedVector3Array([Vector3(-20, 0, 20), Vector3(20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, -20)])
	var ids := ["ground", "ground"]
	var lamp := points.size()
	proxy_box(points, Vector3(300, 250, -400), Vector3(20, 500, 20))
	# lamp-a stands a quarter turn round: its head box spans scene z.
	proxy_box(points, Vector3(300, 500, -400), Vector3(40, 25, 80))
	for i in (points.size() - lamp) / 3: ids.append("lamp-a")
	var neighbour := points.size()
	proxy_box(points, Vector3(1990, 500, -400), Vector3(20, 25, 40))
	for i in (points.size() - neighbour) / 3: ids.append("lamp-b")
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": [], "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for id: String in ids:
		chunk.triangles.append({"surface": "concrete", "object_id": id})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"proxy_materials": {"lamp-a": "builtin:streetlight", "lamp-b": "builtin:streetlight"}}
	chunk.objects.append({"id": "lamp-a", "asset_id": "builtin:streetlight", "position": [300, 0, 400], "quarter_turns": 1})
	for i in 2: chunk.objects.append({"id": "bench-%d" % i, "asset_id": "builtin:bench", "position": [-500 + i * 200, 0, 300], "quarter_turns": i})
	chunk.objects.append({"id": "bench-far", "asset_id": "builtin:bench", "position": [9000, 0, 300], "quarter_turns": 2, "yaw_offset_mdeg": 15000})
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

## The fixture as a packed chunk (chunk_data packed fields only).
static func packed(chunk: Dictionary) -> Dictionary:
	var result := {"packed_version": 1, "cell": chunk.cell, "objects": chunk.objects, "presentation": chunk.presentation,
		"surface_indices": PackedByteArray(), "object_ids": PackedStringArray(), "object_indices": PackedInt32Array()}
	for triangle: Dictionary in chunk.triangles:
		result.surface_indices.append(preload("../chunk_data.gd").SURFACES.find(str(triangle.surface)))
		if not result.object_ids.has(str(triangle.object_id)): result.object_ids.append(str(triangle.object_id))
		result.object_indices.append(result.object_ids.find(str(triangle.object_id)))
	return result

## Instances stand where their proxy boxes are (scene axes, quarter turns and
## yaw like glTF anchors; rendered runs) and list their object ids in
## instance order.
func placements(parent: Node3D, chunk: Dictionary, label: String) -> void:
	var objects := {}
	for object: Dictionary in chunk.objects: objects[str(object.id)] = object
	var lamp_boxes := []
	var lamp_points: PackedVector3Array = chunk.scene_vertices.slice(6, 6 + 72)
	for first in [0, 36]:
		var box := AABB(lamp_points[first], Vector3.ZERO)
		for index in 36: box = box.expand(lamp_points[first + index])
		lamp_boxes.append(box.grow(0.0015))
	for node: MultiMeshInstance3D in parent.find_children("Furniture_*", "MultiMeshInstance3D", true, false):
		var ids: PackedStringArray = node.get_meta("mapkit_object_ids", PackedStringArray())
		check(ids.size() == node.multimesh.instance_count, label + " lists one object id per instance")
		# The headless dummy renderer keeps no instance transforms.
		if DisplayServer.get_name() == "headless": continue
		for index in mini(ids.size(), node.multimesh.instance_count):
			var object: Dictionary = objects.get(ids[index], {})
			var anchor := Node3D.new()
			anchor.position = RENDER.scene_position(object.get("position", [0, 0, 0]))
			anchor.rotation.y = float(object.get("quarter_turns", 0)) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0)
			var placed := node.multimesh.get_instance_transform(index)
			check(placed.is_equal_approx(anchor.transform), label + " places %s like a glTF anchor: %s" % [ids[index], placed])
			anchor.free()
			if ids[index] == "lamp-a":
				check(placed.origin.is_equal_approx(Vector3(3, 0, -4)) and placed.basis.x.is_equal_approx(Vector3(0, 0, -1)), label + " lamp-a stands at (3, 0, -4) a quarter turn round: %s" % placed)
				var outside := 0
				for point: Vector3 in node.multimesh.mesh.get_faces():
					if not lamp_boxes.any(func(box: AABB) -> bool: return box.has_point(placed * point)): outside += 1
				check(outside == 0, label + " lamp-a %s stays inside its placed proxy boxes: %d vertices outside" % [node.get_meta("mapkit_furniture_lod"), outside])

func renderer() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(hidden.has("lamp-a") and hidden.has("lamp-b") and not hidden.has("ground") and not hidden.has("bench-0"), "owner and neighbour streetlight faces are hidden: %s" % [hidden.keys()])
	var bare := fixture(false)
	bare.objects = []
	bare.presentation.proxy_materials = {}
	var without := fixture(false)
	without.triangles = without.triangles.slice(0, 2)
	without.scene_vertices = without.scene_vertices.slice(0, 6)
	check(RENDER.PLAN.estimate(fixture(false), 0) == RENDER.PLAN.estimate(without, 0), "hidden proxy faces add no display reservation")
	var batches := RENDER.PLAN.prepare_batches(fixture(false))
	check(batches.size() == 1 and batches[0].vertices.size() == 6, "prepared batches carry only the ground")
	var ordinary := RENDER.PLAN.prepare_batches(bare)
	check(ordinary.reduce(func(sum, batch): return sum + batch.vertices.size(), 0) == bare.scene_vertices.size(), "other proxies keep their faces")
	var packed_chunk := packed(fixture(false))
	check(RENDER.PLAN.hidden_objects(packed_chunk).keys() == hidden.keys(), "a packed chunk hides the same streetlights")
	check(RENDER.PLAN.estimate(packed_chunk, 0) == RENDER.PLAN.estimate(packed(without), 0) and RENDER.PLAN.estimate(packed_chunk, 0) == RENDER.PLAN.estimate(without, 0), "hidden proxy faces add no reservation to a packed chunk")
	for prepared in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(prepared)
			var collision: Array = chunk.triangles.duplicate(true)
			var collision_points: PackedVector3Array = chunk.scene_vertices.duplicate()
			var planned := RENDER.PLAN.estimate(chunk, 0)
			var lease := Lease.new(planned)
			var job := RENDER.begin(chunk, parent, func(bytes): return lease if bytes == planned else null, planned, cache)
			var steps := 0
			while not RENDER.advance(job): steps += 1
			var label := "%s tier %d" % ["prepared" if prepared else "unprepared", level]
			check(job.error.is_empty(), label + " attaches: %s" % job.error)
			check(chunk.triangles == collision and chunk.scene_vertices == collision_points, label + " leaves the collision triangles untouched")
			var drawn := 0
			for mesh: MeshInstance3D in parent.find_children("*", "MeshInstance3D", true, false):
				if mesh.mesh is ArrayMesh: drawn += mesh.mesh.surface_get_array_len(0)
			check(drawn == 6, label + " draws no proxy faces of streetlights: %d vertices" % drawn)
			var counts := {}
			for node: MultiMeshInstance3D in parent.find_children("Furniture_*", "MultiMeshInstance3D", true, false):
				var key := "%s_%s" % [node.get_meta("mapkit_furniture_kind"), node.get_meta("mapkit_furniture_lod")]
				counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
				check(node.multimesh.mesh == cache.mesh_set(FURNITURE.KEY).get(key), label + " borrows the shared mesh")
				check(node.get_meta("mapkit_static_detail", false) and not node.multimesh.use_custom_data, label + " nodes carry static detail metadata")
			check(counts == {"streetlight_near": 1, "streetlight_far": 1, "bench_near": 3, "bench_far": 3}, label + " instances one streetlight and three benches near and far: %s" % counts)
			check(parent.find_children("Furniture_bench_*", "MultiMeshInstance3D", true, false).size() == 4, label + " benches in two 16 m groups")
			placements(parent, chunk, label)
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), label + " cell lease retires with its furniture nodes")
	check(cache.bytes() >= FURNITURE.SHARED_BYTES, "one shared furniture set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared furniture set retires after its last cell")
	# Without a resource cache (standalone previews) the process-lifetime set.
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(standalone.find_children("Furniture_*", "MultiMeshInstance3D", true, false).size() == 6, "standalone previews draw furniture from the process-lifetime set")
	preview.free()
	# A refused shared set fails a cell with streetlights as it begins, before
	# any upload; a cell of benches only skips them.
	var denied := CACHE.new(func(bytes): return null if bytes == FURNITURE.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and job.get("triangle", 0) == 0, "a refused furniture set fails a cell with streetlights before uploads: %s" % job.get("error", {}))
	var benches := fixture(true)
	benches.objects = benches.objects.filter(func(object: Dictionary) -> bool: return str(object.asset_id) == "builtin:bench")
	job = RENDER.begin(benches, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(benches, 0), denied)
	while not RENDER.advance(job): pass
	check(job.error.is_empty() and parent.find_children("Furniture_*", "MultiMeshInstance3D", true, false).is_empty() and not job.claims.has(FURNITURE.KEY), "a refused furniture set skips a cell's benches: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	await process_frame

## The cache-built lens is a night emitter (lighting_role 2); rendered runs
## compare it from below by night and by day in an unlit scene.
func night(_set: Dictionary) -> void:
	var context := ENVIRONMENT.new()
	var job := {}
	while not FURNITURE.build(job, null, context): pass
	var made: Dictionary = job.result
	var lens: ShaderMaterial = made.materials.lens
	check(lens.get_shader_parameter("lighting_role") == 2 and not lens.get_shader_parameter("vertex_tinted"), "the lens glows through the street-lamp role")
	for kind in ["paint", "wood"]:
		var surface: ShaderMaterial = made.materials[kind]
		check(surface.get_shader_parameter("vertex_tinted") and surface.get_shader_parameter("lighting_role") == 0 and surface.get_meta("mapkit_surface", false), "%s reads vertex tints and weather" % kind)
		check(surface.get_shader_parameter("detail_enabled") == true, "%s carries its detail tile" % kind)
	if DisplayServer.get_name() == "headless":
		print("street_furniture_validator: headless, lens pixels skipped")
		return
	root.size = Vector2i(96, 96)
	var scene := Node3D.new()
	root.add_child(scene)
	var world := WorldEnvironment.new()
	world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_energy = 0.0
	scene.add_child(world)
	var lamp := MeshInstance3D.new()
	lamp.mesh = made.streetlight_near
	scene.add_child(lamp)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 0.1
	scene.add_child(camera)
	camera.position = Vector3(0.285, 4.6, 0.0)
	camera.look_at(Vector3(0.285, 5.0, 0.0), Vector3.FORWARD)
	camera.current = true
	var samples := []
	for sun in [-0.2, 0.8]:
		context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var picture := root.get_texture().get_image()
		samples.append(picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2))
	print("STREETLIGHT_LENS night=", samples[0], " day=", samples[1])
	check(samples[0].r > 0.5 and samples[0].r >= samples[0].b, "the lens glows warm at night: %s" % samples[0])
	check(maxf(samples[1].r, maxf(samples[1].g, samples[1].b)) < 0.1, "the lens is dark by day in an unlit scene: %s" % samples[1])
	scene.queue_free()
	for frame in 3: await process_frame
