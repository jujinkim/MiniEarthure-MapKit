extends SceneTree
## City streetlights (city_lamp_meshes.gd, city_lamp.gdshader,
## city_lamp_halo.gdshader, chunk_renderer, urban_surface lamp pools): a
## deterministic shared set built one part per step inside its allowance;
## for every id and design the placed near and far vertices inside the
## contract envelope (mapkit-core CITY_STREETLIGHT_ENVELOPE_CM), nothing
## beyond x = 0.25 m below 2.5 m, the low parts inside the 12 cm pole box,
## the lens at 3.10 m centred under the arm's reach (both sides on twins),
## a simple far mesh and the twin side collapsed on single lamps; design
## choices from the street square and axis (both sides of a street alike
## inside one square)
## and the object identity; every id in BUILTINS and DRAWN_OVER with hidden
## proxy faces; one group of near, far and halo per 16 m square with the
## reach in the instance data, ranges and shadows per tier; the lamp map
## bound on road and sidewalk materials only when the presentation lists
## one; a retiring lease, a refused set failing the cell before any upload.
## Rendered runs compare the lens, the halo and a lit road by night and day
## (MAPKIT_CITY_LAMP_CAPTURE, a PNG path, writes the night view).
const RENDER := preload("../chunk_renderer.gd")
const LAMPS := preload("../city_lamp_meshes.gd")
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
	envelope(set)
	designs()
	policy()
	await renderer()
	await night()
	QUALITY.current = {}
	print("city_lamp_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## Builds the set twice (one part per call) and measures its memory.
func meshes() -> Dictionary:
	var lease := Lease.new()
	var job := {}
	var steps := 0
	while not LAMPS.build(job, lease): steps += 1
	check(steps == LAMPS.STEPS.size() - 1, "one part per call: %d steps" % (steps + 1))
	var again := {}
	while not LAMPS.build(again): pass
	var set: Dictionary = job.result
	var vertices := 0
	var indices := 0
	for lod in LAMPS.LODS:
		var mesh: ArrayMesh = set[lod]
		check(mesh.get_surface_count() == 1 and mesh.surface_get_arrays(0) == (again.result[lod] as ArrayMesh).surface_get_arrays(0), "%s builds identically in one surface" % lod)
		vertices += mesh.surface_get_array_len(0)
		indices += mesh.surface_get_array_index_len(0)
		check(mesh.custom_aabb.size.x >= 4.0, "%s culls by the envelope of every reach: %s" % [lod, mesh.custom_aabb])
	var bytes := 2 * (vertices * 64 + indices * 4)
	print("shared city lamp set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, bytes, LAMPS.SHARED_BYTES])
	check(bytes + 131072 <= LAMPS.SHARED_BYTES, "the shared set's meshes, materials and build arrays fit its allowance: %d B" % bytes)
	return set

## Kept triangles of `mesh` for a reach and design: placed corner lists.
static func kept(mesh: ArrayMesh, reach_cm: int, flags: int) -> Array:
	var arrays := mesh.surface_get_arrays(0)
	var result := []
	var index: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	for k in range(0, index.size(), 3):
		var corners := []
		for j in 3:
			var i := index[k + j]
			corners.append(LAMPS.placed(arrays[Mesh.ARRAY_VERTEX][i], arrays[Mesh.ARRAY_TEX_UV][i], arrays[Mesh.ARRAY_TEX_UV2][i], reach_cm, flags))
		if corners.all(func(c) -> bool: return c != null): result.append([corners, int(round(arrays[Mesh.ARRAY_TEX_UV][index[k]].x))])
	return result

## Every id and design: inside the envelope, high above anything beyond the
## pole's low parts, the low parts inside the pole box, the lens under the
## head centre, the far mesh simple.
func envelope(set: Dictionary) -> void:
	var e: Array = LAMPS.ENVELOPE_CM
	var summary := []
	for id: String in LAMPS.IDS:
		var reach: int = LAMPS.IDS[id][0]
		var twin: bool = LAMPS.IDS[id][1]
		for flags in 64:
			if (flags & LAMPS.FLAG_TWIN != 0) != twin: continue
			if twin and flags & LAMPS.FLAG_PEDESTRIAN != 0: continue
			for lod in ["near", "far"]:
				var triangles := kept(set[lod], reach, flags)
				var label := "%s %s flags %d" % [id, lod, flags]
				var outside := 0
				var low := 0
				var box := 0
				var cover := [0, 0, 0, 0]
				var lens := []
				for triangle: Array in triangles:
					for p: Vector3 in triangle[0]:
						var back := -float(reach + e[1]) * 0.01 if twin else -float(e[0]) * 0.01
						if p.x < back - 1e-4 or p.x > float(reach + e[1]) * 0.01 + 1e-4 or absf(p.z) > float(e[2]) * 0.01 + 1e-4 or p.y < -LAMPS.SKIRT - 1e-4 or p.y > float(e[3]) * 0.01 + 1e-4: outside += 1
						if absf(p.x) > 0.25 + 1e-4 and p.y < 2.5: low += 1
						if p.y < 0.6 and (absf(p.x) > 0.0625 or absf(p.z) > 0.0625): box += 1
						# The visible base reaches every corner of the 12 cm collision box at and above car height.
						if lod == "near" and p.y >= 0.58 and p.y <= 0.61 and absf(p.x) >= 0.06 and absf(p.z) >= 0.06: cover[(1 if p.x > 0.0 else 0) + (2 if p.z > 0.0 else 0)] += 1
					if triangle[1] == LAMPS.ROLE_LENS and (triangle[0] as Array).all(func(p: Vector3) -> bool: return p.y > 3.0): lens.append_array(triangle[0])
				check(outside == 0, label + " stays inside the envelope: %d vertices outside" % outside)
				check(low == 0, label + " keeps parts beyond 0.25 m at least 2.5 m up: %d" % low)
				check(box == 0, label + " keeps its low parts inside the pole box: %d" % box)
				check(lod != "near" or cover.min() > 0, label + " covers every corner of the pole box up to car height: %s" % [cover])
				var sides := [1.0, -1.0] if twin else [1.0]
				for s: float in sides:
					var points := lens.filter(func(p: Vector3) -> bool: return p.x * s > 0.0)
					var centre := Vector3.ZERO
					for p: Vector3 in points: centre += p
					centre /= maxf(1.0, float(points.size()))
					check(not points.is_empty() and absf(absf(centre.x) - float(reach) * 0.01) < 0.03 and absf(centre.y - float(LAMPS.HEAD_CM) * 0.01) < 0.004, label + " lens at the head centre (side %d): %s" % [s, centre])
				check(twin or lens.all(func(p: Vector3) -> bool: return p.x > 0.0), label + " single lamps have no lens at -x above 3 m")
				if flags == (1 if twin else 0) and reach == 135: summary.append("%s %s %d triangles" % [id.trim_prefix("builtin:"), lod, triangles.size()])
				if lod == "far": check(triangles.size() <= (60 if twin else 40), label + " far mesh is simple: %d triangles" % triangles.size())
	print("city lamps: ", ", ".join(summary))
	var near := kept(set.near, 135, LAMPS.FLAG_BANNER | LAMPS.FLAG_PEDESTRIAN | LAMPS.FLAG_BOX).size()
	var plain := kept(set.near, 135, 0).size()
	check(near > plain + 100 and plain >= 10 * kept(set.far, 135, 0).size(), "optional parts and a detailed near mesh: %d / %d" % [near, plain])
	check(kept(set.halo, 75, 0).size() == 2 and kept(set.halo, 75, LAMPS.FLAG_TWIN).size() == 4, "one halo quad per head")
	# The shader mirrors keep() for every part.
	var code: String = preload("../city_lamp.gdshader").code
	for part in range(1, 11): check(code.contains("if(part == %d)" % part), "the shader keeps part %d" % part)

## Street designs: lamps of one street (same square, both sides) share arm,
## shaft, paint and banners; the choice is stable and every design occurs.
func designs() -> void:
	var a := LAMPS.design({"id": "a", "asset_id": "builtin:city_streetlight_135", "position": [1000, 0, 2000], "quarter_turns": 0, "yaw_offset_mdeg": 12000})
	var b := LAMPS.design({"id": "b", "asset_id": "builtin:city_streetlight_75", "position": [9000, 0, 2600], "quarter_turns": 2, "yaw_offset_mdeg": 12000})
	var street := LAMPS.FLAG_STRAIGHT | LAMPS.FLAG_OCTAGONAL | LAMPS.FLAG_PEDESTRIAN
	check(int(a[0]) & street == int(b[0]) & street and int(a[1]) == int(b[1]), "both sides of a street share the design: %s %s" % [a, b])
	check(LAMPS.design({"id": "a", "asset_id": "builtin:city_streetlight_135", "position": [1000, 0, 2000], "quarter_turns": 0, "yaw_offset_mdeg": 12000}) == a, "stable designs")
	var seen := {}
	var twins_walk := true
	for i in 400:
		var twin := i % 5 == 0
		var chosen := LAMPS.design({"id": "lamp-%d" % i, "asset_id": "builtin:city_streetlight_twin_75" if twin else "builtin:city_streetlight_195", "position": [i * 9100 - 1800000, 0, i * 5300 - 900000], "quarter_turns": i % 4, "yaw_offset_mdeg": (i * 7919) % 45000})
		for flag in [LAMPS.FLAG_STRAIGHT, LAMPS.FLAG_BANNER, LAMPS.FLAG_PEDESTRIAN, LAMPS.FLAG_BOX, LAMPS.FLAG_OCTAGONAL]:
			if int(chosen[0]) & flag: seen[flag] = true
		seen["paint%d" % (int(chosen[1]) & 3)] = true
		if twin and int(chosen[0]) & LAMPS.FLAG_PEDESTRIAN: twins_walk = false
		if (int(chosen[0]) & LAMPS.FLAG_TWIN != 0) != twin: twins_walk = false
	check(seen.size() == 9 and twins_walk, "every design occurs, twins carry no pedestrian arm: %s" % [seen.keys()])

## Ranges and shadows per tier.
func policy() -> void:
	for level in 3:
		var value := QUALITY.profile(level)
		var reach := float(value.streetlight_near) + QUALITY.FURNITURE_REACH if level > 0 else 0.0
		var end := float(value.decoration_distance) + QUALITY.FURNITURE_REACH
		for lod in LAMPS.LODS:
			var node := MultiMeshInstance3D.new()
			node.multimesh = MultiMesh.new()
			node.set_meta("mapkit_furniture_lod", lod)
			node.set_meta("mapkit_furniture_kind", "city_streetlight")
			QUALITY.apply_node(node, value)
			var label := "city lamp %s tier %d" % [lod, level]
			match lod:
				"near": check(node.visible == (level > 0) and (level == 0 or is_equal_approx(node.visibility_range_end, reach)), label + " near range")
				"far": check(node.visible and is_equal_approx(node.visibility_range_begin, reach) and is_equal_approx(node.visibility_range_end, end), label + " far range")
				_: check(node.visible == (level > 0) and is_equal_approx(node.visibility_range_end, end), label + " halo range")
			var shadows: bool = level >= 1 and lod != "halo"
			check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), label + " shadows")
			if lod == "halo":
				# By day the halos are hidden (no empty draws); night restores the tier's state.
				node.add_to_group(QUALITY.HALO_GROUP)
				root.add_child(node)
				QUALITY.set_night_lights(self, false)
				check(not node.visible, label + " halo hidden by day")
				QUALITY.apply_node(node, value)
				check(not node.visible, label + " halo stays hidden by day on re-apply")
				QUALITY.set_night_lights(self, true)
				check(node.visible == (level > 0), label + " halo shown at night")
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

## An asphalt ground quad and a concrete sidewalk quad, eight lamps (one of
## each id) in two 16 m squares and a neighbour lamp's clipped pole face; the
## lamp map when `lit`.
func fixture(prepared: bool, lit := true) -> Dictionary:
	var points := PackedVector3Array([Vector3(-20, 0, 20), Vector3(20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, -20)])
	var surfaces := ["asphalt", "asphalt"]
	var ids := ["ground", "ground"]
	points.append_array([Vector3(-20, 0.03, 22), Vector3(20, 0.03, 22), Vector3(20, 0.03, 20), Vector3(-20, 0.03, 22), Vector3(20, 0.03, 20), Vector3(-20, 0.03, 20)])
	surfaces.append_array(["concrete", "concrete"])
	ids.append_array(["walk", "walk"])
	var objects := []
	var materials := {}
	var index := 0
	for id: String in LAMPS.IDS:
		var object := {"id": "lamp-%d" % index, "asset_id": id, "position": [200 + index * 200 + (1400 if index >= 3 else 0), 0, -2100], "quarter_turns": 1, "yaw_offset_mdeg": 0}
		objects.append(object)
		materials[object.id] = id
		var first := points.size()
		proxy_box(points, Vector3(object.position[0], 165, 2100), Vector3(12, 330, 12))
		for i in (points.size() - first) / 3:
			ids.append(object.id)
			surfaces.append("concrete")
		index += 1
	var neighbour := points.size()
	proxy_box(points, Vector3(1995, 165, 300), Vector3(12, 330, 12))
	for i in (points.size() - neighbour) / 3:
		ids.append("lamp-n")
		surfaces.append("concrete")
	materials["lamp-n"] = "builtin:city_streetlight_75"
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": objects, "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for k in ids.size():
		chunk.triangles.append({"surface": surfaces[k], "object_id": ids[k]})
		for j in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"proxy_materials": materials, "urban_surfaces": true}
	if lit:
		var map := PackedByteArray()
		map.resize(9 * 9 * 2)
		map.fill(128)
		chunk.presentation.lamp_map = map
		chunk.presentation.lamp_map_rect = PackedFloat32Array([-20.0, -20.0, 40.0, 9.0])
		chunk.presentation.lamp_map_height = PackedFloat32Array([-20.48, 0.16])
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var missing := LAMPS.IDS.keys().filter(func(id: String) -> bool: return id not in RENDER.BUILTINS or id not in RENDER.PLAN.DRAWN_OVER)
	check(missing.is_empty(), "renderer integration: %s" % [missing])
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(hidden.has("lamp-0") and hidden.has("lamp-n") and not hidden.has("ground") and not hidden.has("walk"), "owner and neighbour lamp faces are hidden: %s" % [hidden.keys()])
	var bare := fixture(false)
	bare.triangles = bare.triangles.slice(0, 4)
	bare.scene_vertices = bare.scene_vertices.slice(0, 12)
	check(RENDER.PLAN.estimate(fixture(false), 0) == RENDER.PLAN.estimate(bare, 0), "hidden pole faces add no display reservation")
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	for prepared in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			for lit in [true, false]:
				var parent := Node3D.new()
				root.add_child(parent)
				var chunk := fixture(prepared, lit)
				var collision: Array = chunk.triangles.duplicate(true)
				var planned := RENDER.PLAN.estimate(chunk, 0)
				var lease := Lease.new(planned)
				var job := RENDER.begin(chunk, parent, func(bytes): return lease if bytes == planned else null, planned, cache)
				while not RENDER.advance(job): pass
				var label := "%s tier %d %s" % ["prepared" if prepared else "unprepared", level, "lit" if lit else "unlit"]
				check(job.error.is_empty(), label + " attaches: %s" % job.error)
				check(chunk.triangles == collision, label + " leaves the collision triangles untouched")
				var drawn := 0
				var bound := 0
				for mesh: MeshInstance3D in parent.find_children("*", "MeshInstance3D", true, false):
					if mesh.mesh is ArrayMesh: drawn += mesh.mesh.surface_get_array_len(0)
					var material: Material = mesh.material_override
					if material is ShaderMaterial and material.get_shader_parameter("lamp_enabled") == true and material.get_shader_parameter("lamp_map") is ImageTexture: bound += 1
				check(drawn == 12, label + " draws no pole faces: %d vertices" % drawn)
				check(bound == (2 if lit else 0), label + " binds the lamp map on road and sidewalk materials: %d" % bound)
				var counts := {}
				var groups := {}
				var made: Dictionary = cache.mesh_set(LAMPS.KEY)
				for node: MultiMeshInstance3D in parent.find_children("CityLamp_*", "MultiMeshInstance3D", true, false):
					var lod: String = node.get_meta("mapkit_furniture_lod")
					counts[lod] = int(counts.get(lod, 0)) + node.multimesh.instance_count
					groups[lod] = int(groups.get(lod, 0)) + 1
					check(node.multimesh.mesh == made.get(lod) and node.multimesh.use_custom_data, label + " borrows the shared %s mesh with design data" % lod)
					var ids: PackedStringArray = node.get_meta("mapkit_object_ids", PackedStringArray())
					check(ids.size() == node.multimesh.instance_count, label + " lists one object id per instance")
					if DisplayServer.get_name() == "headless": continue
					for i in ids.size():
						var object: Dictionary = chunk.objects[int(ids[i].trim_prefix("lamp-"))]
						var custom := node.multimesh.get_instance_custom_data(i)
						check(int(custom.r) == int(LAMPS.IDS[str(object.asset_id)][0]) and (int(custom.g) & LAMPS.FLAG_TWIN != 0) == bool(LAMPS.IDS[str(object.asset_id)][1]), label + " %s carries its reach and twin flag: %s" % [ids[i], custom])
						check(node.multimesh.get_instance_transform(i).is_equal_approx(LAMPS.placement(object)), label + " places %s like its pole box" % ids[i])
				check(counts == {"near": 8, "far": 8, "halo": 8} and groups == {"near": 2, "far": 2, "halo": 2}, label + " eight lamps in two 16 m groups: %s %s" % [counts, groups])
				RENDER.cancel(job)
				parent.queue_free()
				for _frame in 4: await process_frame
				check(lease.retired(), label + " cell lease retires with its lamps and lamp map")
	check(cache.bytes() >= LAMPS.SHARED_BYTES, "one shared lamp set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared set retires after its last cell")
	QUALITY.current = {}
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(standalone.find_children("CityLamp_*", "MultiMeshInstance3D", true, false).size() == 6, "standalone previews draw lamps from the process-lifetime set")
	preview.free()
	var denied := CACHE.new(func(bytes): return null if bytes == LAMPS.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and job.get("triangle", 0) == 0, "a refused lamp set fails the cell before uploads: %s" % job.get("error", {}))
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Rendered runs: the lens and halo glow only at night, and a road under a
## lamp map brightens at night only.
func night() -> void:
	var context := ENVIRONMENT.new()
	var job := {}
	while not LAMPS.build(job, null, context): pass
	var made: Dictionary = job.result
	check(made.material.get_shader_parameter("environment_enabled") == true and made.halo_material.get_shader_parameter("environment_enabled") == true, "lamp materials read the environment")
	if DisplayServer.get_name() == "headless":
		print("city_lamp_validator: headless, night pixels skipped")
		return
	root.size = Vector2i(160, 120)
	var scene := Node3D.new()
	root.add_child(scene)
	var world := WorldEnvironment.new()
	world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_energy = 0.0
	scene.add_child(world)
	# One single lamp (reach 1.35) as a MultiMesh with its design data, and an
	# urban road quad under a uniform lamp map.
	var nodes := {}
	for lod in ["near", "halo"]:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.use_custom_data = true
		multi.mesh = made[lod]
		multi.instance_count = 1
		multi.set_instance_transform(0, Transform3D())
		multi.set_instance_custom_data(0, Color(135, 0, 0, 1))
		var node := MultiMeshInstance3D.new()
		node.multimesh = multi
		scene.add_child(node)
		nodes[lod] = node
	var road := MeshInstance3D.new()
	var plane := PlaneMesh.new()
	plane.size = Vector2(6, 6)
	road.mesh = plane
	road.position = Vector3(1.35, 0.0, -6.0)
	var material := RENDER.surface_material("asphalt", {"urban_surfaces": true}, CACHE.wet_urban_shader())
	material.set_shader_parameter("environment_enabled", true)
	material.set_shader_parameter("environment_data", context.texture)
	# Full light with the lit feet at the road (G 0 = y 0): pools light it.
	var map := PackedByteArray([255, 0, 255, 0, 255, 0, 255, 0])
	var fake := {"chunk": {}}
	LAMPS.bind_light_map(fake, material, {"lamp_map": map, "lamp_map_rect": PackedFloat32Array([-10.0, -20.0, 30.0, 2.0]), "lamp_map_height": PackedFloat32Array([0.0, 0.16])})
	road.material_override = material
	scene.add_child(road)
	var camera := Camera3D.new()
	scene.add_child(camera)
	camera.position = Vector3(1.35, 1.2, -1.2)
	camera.look_at(Vector3(1.35, 3.1, 0.0), Vector3.UP)
	camera.current = true
	var samples := {}
	for sun in [-0.2, 0.8]:
		context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
		camera.look_at(Vector3(1.35, 3.1, 0.02), Vector3.UP)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var picture := root.get_texture().get_image()
		var lens := picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2)
		nodes.near.visible = false
		for frame in 3: await process_frame
		await RenderingServer.frame_post_draw
		picture = root.get_texture().get_image()
		var glow := picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2)
		nodes.near.visible = true
		camera.look_at(Vector3(1.35, 0.0, -6.0), Vector3.UP)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		picture = root.get_texture().get_image()
		var ground := picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2)
		samples[sun] = [lens, glow, ground]
		var path := OS.get_environment("MAPKIT_CITY_LAMP_CAPTURE")
		if not path.is_empty() and sun < 0.0: picture.save_png(path)
	print("CITY_LAMP night=", samples[-0.2], " day=", samples[0.8])
	var night_lens: Color = samples[-0.2][0]
	var day_lens: Color = samples[0.8][0]
	check(night_lens.r > 0.6 and night_lens.r >= night_lens.b, "the LED lens glows warm white at night: %s" % night_lens)
	check(maxf(day_lens.r, maxf(day_lens.g, day_lens.b)) < 0.15, "the lens is dark by day in an unlit scene: %s" % day_lens)
	check((samples[-0.2][1] as Color).r > (samples[0.8][1] as Color).r + 0.02, "the halo glows round the head at night only: %s" % [samples[-0.2][1]])
	check((samples[-0.2][2] as Color).r > (samples[0.8][2] as Color).r + 0.01, "the lamp map lights the road at night only: %s %s" % [samples[-0.2][2], samples[0.8][2]])
	scene.queue_free()
	for frame in 3: await process_frame
