extends SceneTree
## Transit details (transit_meshes.gd, chunk_renderer): subway entrance and
## station sign meshes inside their unchanged collision boxes (plus a skirt
## below the foot), the stair's fake depth on a closed slab above the ground,
## lit signs on the road-facing front and every pylon face, near/far tier
## variants, ranges and object ids, night emission materials, hidden proxy faces with
## untouched collision triangles (prepared and unprepared paths, owner and
## neighbour cells), the shared set's memory, a refused set failing the cell as
## it begins, and release of every cell and shared reservation. Rendered runs
## also compare the lit signs, landing, well and tunnel by night and day and
## look down the well over sloping ground.
const RENDER := preload("../chunk_renderer.gd")
const TRANSIT := preload("../transit_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const ENVIRONMENT := preload("../environment_materials.gd")
const SURFACE_NAMES := ["paint", "stone", "glow", "glass"]
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
	meshes()
	policy()
	await renderer()
	await night()
	QUALITY.current = {}
	print("transit_detail_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

static func triangles(mesh: ArrayMesh) -> int:
	var total := 0
	for surface in mesh.get_surface_count(): total += mesh.surface_get_array_index_len(surface) / 3
	return total

## Vertices, normals and colours of the surface whose material is `name`.
static func surface(mesh: ArrayMesh, materials: Dictionary, name: String) -> Array:
	for index in mesh.get_surface_count():
		if mesh.surface_get_material(index) == materials.get(name): return mesh.surface_get_arrays(index)
	return []

static func luminance(color: Color) -> float:
	return color.r * 0.2126 + color.g * 0.7152 + color.b * 0.0722

func meshes() -> void:
	var lease := Lease.new()
	var state := {}
	var steps := 1
	while not TRANSIT.build(state, lease): steps += 1
	check(steps == TRANSIT.STEPS.size(), "one part per call: %d steps" % steps)
	var set: Dictionary = state.result
	var again := {}
	while not TRANSIT.build(again): pass
	var vertices := 0
	var indices := 0
	var counts := []
	var expected := {"subway_entrance_near": ["paint", "stone", "glow", "glass"], "subway_entrance_far": ["paint", "stone", "glow", "glass"],
		"station_sign_near": ["paint", "stone", "glow"], "station_sign_far": ["paint", "stone", "glow"]}
	for key: String in expected:
		var mesh: ArrayMesh = set[key]
		var other: ArrayMesh = again.result[key]
		var names := []
		for index in mesh.get_surface_count():
			var material: Material = mesh.surface_get_material(index)
			names.append(set.materials.find_key(material))
			check(mesh.surface_get_arrays(index) == other.surface_get_arrays(index), "%s builds identically" % key)
			vertices += mesh.surface_get_array_len(index)
			indices += mesh.surface_get_array_index_len(index)
		check(names == expected[key], "%s surfaces %s" % [key, names])
		counts.append("%s %d triangles" % [key, triangles(mesh)])
	print("transit details: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared transit set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, TRANSIT.SHARED_BYTES])
	check(mesh_bytes + 65536 <= TRANSIT.SHARED_BYTES, "the shared set's meshes, materials and build arrays fit its allowance: %d B" % mesh_bytes)
	# Every vertex inside the collision box (plus the skirt below the foot
	# that hides a downhill side); the meshes fill the proxy so no box face is
	# left to imagine.
	for kind in ["subway_entrance", "station_sign"]:
		var proxy: AABB = TRANSIT.ENTRANCE_PROXY if kind == "subway_entrance" else TRANSIT.SIGN_PROXY
		var envelope := AABB(proxy.position - Vector3(0.0015, TRANSIT.SKIRT + 0.0015, 0.0015), proxy.size + Vector3(0.003, TRANSIT.SKIRT + 0.003, 0.003))
		for lod in TRANSIT.LODS:
			var mesh: ArrayMesh = set[kind + "_" + lod]
			var outside := 0
			for index in mesh.get_surface_count():
				for point: Vector3 in mesh.surface_get_arrays(index)[Mesh.ARRAY_VERTEX]:
					if not envelope.has_point(point): outside += 1
			var bounds := mesh.get_aabb()
			check(outside == 0, "%s %s stays inside its collision box: %d vertices outside" % [kind, lod, outside])
			check(bounds.size.x >= proxy.size.x - 0.03 and bounds.size.z >= proxy.size.z - 0.03 and bounds.end.y >= proxy.end.y - 0.03, "%s %s fills its proxy: %s" % [kind, lod, bounds])
		check(triangles(set[kind + "_near"]) >= 4 * triangles(set[kind + "_far"]), "the far %s is a simple variant" % kind)
	check(triangles(set.subway_entrance_far) <= 200 and triangles(set.station_sign_far) <= 80, "far meshes stay small")
	var materials: Dictionary = set.materials
	# Entrance: treads at the landing and each step down to the well floor;
	# nothing inside the parapets lies below FLOOR (a closed slab hides
	# sloping ground); the well darkens before the mouth, the tunnel stays
	# dim and its ceiling slopes down to the floor.
	var levels := {}
	var low := 0
	for name in ["stone", "glow", "paint"]:
		for lod in TRANSIT.LODS:
			var arrays: Array = surface(set["subway_entrance_" + lod], materials, name)
			for index in arrays[Mesh.ARRAY_VERTEX].size():
				var point: Vector3 = arrays[Mesh.ARRAY_VERTEX][index]
				if arrays[Mesh.ARRAY_NORMAL][index].y < 0.99 or absf(point.x) > 0.331: continue
				if point.z > -0.544 and point.z < 0.574 and point.y < TRANSIT.FLOOR - 0.0001: low += 1
				if lod == "near" and point.z > -0.441 and point.z < -0.289: levels[snappedf(point.y, 0.001)] = true
	var expected_levels := [TRANSIT.LANDING, TRANSIT.LANDING - TRANSIT.DROP, TRANSIT.LANDING - 2.0 * TRANSIT.DROP, TRANSIT.FLOOR].map(func(level: float) -> float: return snappedf(level, 0.001))
	check(expected_levels.all(func(level: float) -> bool: return levels.has(level)), "three steps down from the landing to the well floor: %s" % [levels.keys()])
	check(low == 0, "no upward surface inside the parapets lies below the slab floor %.3f: %d vertices" % [TRANSIT.FLOOR, low])
	var glow: Array = surface(set.subway_entrance_near, materials, "glow")
	var deep := 0.0
	var well := 0.0
	var tunnel := 0.0
	var landing := 1.0
	var ceiling := []
	var sign_face := 0
	for index in glow[Mesh.ARRAY_VERTEX].size():
		var point: Vector3 = glow[Mesh.ARRAY_VERTEX][index]
		var color: Color = glow[Mesh.ARRAY_COLOR][index]
		var normal: Vector3 = glow[Mesh.ARRAY_NORMAL][index]
		var lamp := point.y > 0.255 and point.z < 0.14
		if point.z > 0.5 and point.y < TRANSIT.FLOOR + 0.03: deep = maxf(deep, luminance(color))
		if normal.y > 0.99 and absf(point.y - TRANSIT.FLOOR) < 0.001 and point.z >= -0.1 and point.z <= TRANSIT.MOUTH: well = maxf(well, luminance(color))
		if point.z > TRANSIT.MOUTH + 0.001 and point.y < 0.3 and absf(point.x) < 0.301 and not lamp: tunnel = maxf(tunnel, luminance(color))
		if normal.y > 0.99 and absf(point.y - TRANSIT.LANDING) < 0.001 and point.z < -0.44: landing = minf(landing, luminance(color))
		if normal.y < -0.8 and point.z > 0.11 and point.y < 0.29: ceiling.append(point)
		if point.z < -0.59 and point.y > 0.85 and normal.z < -0.99: sign_face += 1
	check(deep < 0.02, "the stair darkens into the tunnel: %.3f" % deep)
	check(well <= 0.05, "the open well is dark (<= 0.05 linear) before the mouth: %.3f" % well)
	check(tunnel <= 0.08, "the tunnel interior stays dim (<= 0.08 linear): %.3f" % tunnel)
	check(landing >= 0.1, "the landing is softly lit: %.3f" % landing)
	ceiling.sort_custom(func(a: Vector3, b: Vector3) -> bool: return a.z < b.z)
	var slope: bool = ceiling.size() >= 8 and ceiling[-1].y <= TRANSIT.FLOOR + 0.03
	for k in ceiling.size() - 1: slope = slope and ceiling[k + 1].y <= ceiling[k].y + 0.0001 and ceiling[k].y <= 0.281
	check(slope, "the tunnel ceiling slopes down to the floor inside the mouth: %d vertices" % ceiling.size())
	check(sign_face >= 60, "the lit sign band faces the road (local -z): %d inlay vertices" % sign_face)
	check(not surface(set.subway_entrance_near, materials, "glass").is_empty(), "the canopy has glass")
	# Sign: lit light-box panels on all four faces at the top.
	var lit := {}
	var sign_glow: Array = surface(set.station_sign_near, materials, "glow")
	for index in sign_glow[Mesh.ARRAY_VERTEX].size():
		var normal: Vector3 = sign_glow[Mesh.ARRAY_NORMAL][index]
		if sign_glow[Mesh.ARRAY_VERTEX][index].y > 1.3 and absf(normal.y) < 0.01: lit[normal.round()] = true
	check(lit.size() == 4, "the pylon's light box is lit on four faces: %s" % [lit.keys()])
	# A quarter turn turns the entrance with its collision box.
	var turned := RENDER.FURNITURE.placement({"position": [0, 0, 0], "quarter_turns": 1})
	var box: AABB = turned * set.subway_entrance_near.get_aabb()
	check(box.size.x >= 1.17 and box.size.z <= 0.81, "a quarter-turned entrance spans the turned box: %s" % box.size)

## Ranges and shadows per tier, live (street furniture policy for solid kinds).
func policy() -> void:
	for level in 3:
		var value := QUALITY.profile(level)
		var near := float(value.furniture_near) + QUALITY.GROUP_REACH if level > 0 else 0.0
		for kind in ["subway_entrance", "station_sign"]:
			for lod in TRANSIT.LODS:
				var node := MultiMeshInstance3D.new()
				node.multimesh = MultiMesh.new()
				node.set_meta("mapkit_furniture_lod", lod)
				node.set_meta("mapkit_furniture_kind", kind)
				node.set_meta("mapkit_decoration", false)
				QUALITY.apply_node(node, value)
				var label := "%s %s tier %d" % [kind, lod, level]
				if lod == "near":
					check(node.visible == (level > 0) and (level == 0 or is_equal_approx(node.visibility_range_end, near)), label + " near range")
				else:
					check(node.visible and is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == 0.0, label + " far range, always drawn")
				check(node.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON, label + " shadows")
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

## Ground quad, an owner cell's quarter-turned entrance and station sign
## (their proxy boxes), a neighbour sign's clipped faces (no object) and a
## second entrance in another 32 m group.
func fixture(prepared: bool) -> Dictionary:
	var points := PackedVector3Array([Vector3(-20, 0, 20), Vector3(20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, -20)])
	var ids := ["ground", "ground"]
	for item: Array in [["entrance-a", Vector3(300, 55, -400), Vector3(120, 110, 80)], ["sign-a", Vector3(-500, 90, -300), Vector3(24, 180, 24)], ["sign-b", Vector3(1990, 90, -400), Vector3(20, 180, 24)]]:
		var first := points.size()
		proxy_box(points, item[1], item[2])
		for i in (points.size() - first) / 3: ids.append(item[0])
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": [], "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for id: String in ids:
		chunk.triangles.append({"surface": "concrete", "object_id": id})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"proxy_materials": {"entrance-a": "builtin:subway_entrance", "sign-a": "builtin:station_sign", "sign-b": "builtin:station_sign"}}
	chunk.objects.append({"id": "entrance-a", "asset_id": "builtin:subway_entrance", "position": [300, 0, 400], "quarter_turns": 1})
	chunk.objects.append({"id": "sign-a", "asset_id": "builtin:station_sign", "position": [-500, 0, 300], "quarter_turns": 0})
	chunk.objects.append({"id": "entrance-far", "asset_id": "builtin:subway_entrance", "position": [9000, 0, 300], "quarter_turns": 2, "yaw_offset_mdeg": 15000})
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(hidden.has("entrance-a") and hidden.has("sign-a") and hidden.has("sign-b") and not hidden.has("ground"), "owner and neighbour transit faces are hidden: %s" % [hidden.keys()])
	var without := fixture(false)
	without.triangles = without.triangles.slice(0, 2)
	without.scene_vertices = without.scene_vertices.slice(0, 6)
	check(RENDER.PLAN.estimate(fixture(false), 0) == RENDER.PLAN.estimate(without, 0), "hidden proxy faces add no display reservation")
	var batches := RENDER.PLAN.prepare_batches(fixture(false))
	check(batches.size() == 1 and batches[0].vertices.size() == 6, "prepared batches carry only the ground")
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
			while not RENDER.advance(job): pass
			var label := "%s tier %d" % ["prepared" if prepared else "unprepared", level]
			check(job.error.is_empty(), label + " attaches: %s" % job.error)
			check(chunk.triangles == collision and chunk.scene_vertices == collision_points, label + " leaves the collision triangles untouched")
			var drawn := 0
			for mesh: MeshInstance3D in parent.find_children("*", "MeshInstance3D", true, false):
				if mesh.mesh is ArrayMesh: drawn += mesh.mesh.surface_get_array_len(0)
			check(drawn == 6, label + " draws no proxy faces of entrances or signs: %d vertices" % drawn)
			var counts := {}
			for node: MultiMeshInstance3D in parent.find_children("Transit_*", "MultiMeshInstance3D", true, false):
				var key := "%s_%s" % [node.get_meta("mapkit_furniture_kind"), node.get_meta("mapkit_furniture_lod")]
				counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
				check(node.multimesh.mesh == cache.mesh_set(TRANSIT.KEY).get(key), label + " borrows the shared mesh")
				check(node.get_meta("mapkit_static_detail", false) and not node.get_meta("mapkit_decoration", true), label + " nodes are solid static details")
				var ids: PackedStringArray = node.get_meta("mapkit_object_ids", PackedStringArray())
				check(ids.size() == node.multimesh.instance_count and Array(ids).all(func(id): return id in ["entrance-a", "sign-a", "entrance-far"]), label + " nodes list their instances' object ids: %s" % ids)
				var near := key.ends_with("near")
				check(node.visible == (not near or level > 0), label + " %s visibility on this tier" % key)
			check(counts == {"subway_entrance_near": 2, "subway_entrance_far": 2, "station_sign_near": 1, "station_sign_far": 1}, label + " instances two entrances and one sign near and far: %s" % counts)
			check(parent.find_children("Transit_subway_entrance_*", "MultiMeshInstance3D", true, false).size() == 4, label + " entrances in two 32 m groups")
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), label + " cell lease retires with its transit nodes")
	check(cache.bytes() >= TRANSIT.SHARED_BYTES, "one shared transit set stays cached")
	check(cache.preparation_usec.has("transit_subway_entrance_near"), "preparation time is reported per part: %s" % [cache.preparation_usec.keys()])
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared transit set retires after its last cell")
	# Without a resource cache (standalone previews) the process-lifetime set.
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(standalone.find_children("Transit_*", "MultiMeshInstance3D", true, false).size() == 6, "standalone previews draw transit details from the process-lifetime set")
	preview.free()
	# A refused shared set fails the cell as it begins, before any upload:
	# its entrances would have no body.
	var denied := CACHE.new(func(bytes): return null if bytes == TRANSIT.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and parent.get_child_count() == 0 and not job.claims.has(TRANSIT.KEY), "a refused transit set fails the cell before any upload: %s" % job.get("error", {}))
	while not RENDER.advance(job): pass
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET", "a refused transit set fails the cell: %s" % job.get("error", {}))
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Cache-built materials: the glow is a night emitter (lighting_role 2) with
## vertex tints, paint and stone read weather and detail tiles, glass is
## translucent. Rendered runs compare the band pictogram, the tunnel, the
## sign panel, the landing and the well by night and by day in an unlit
## scene, and look down the well over 12 % sloping ground.
func night() -> void:
	var context := ENVIRONMENT.new()
	var state := {}
	while not TRANSIT.build(state, null, context): pass
	var made: Dictionary = state.result
	var glow: ShaderMaterial = made.materials.glow
	check(glow.get_shader_parameter("lighting_role") == 2 and glow.get_shader_parameter("vertex_tinted"), "lit parts glow through the street-lamp role with their tints")
	for kind in ["paint", "stone"]:
		var surface_material: ShaderMaterial = made.materials[kind]
		check(surface_material.get_shader_parameter("vertex_tinted") and surface_material.get_shader_parameter("lighting_role") == 0 and surface_material.get_meta("mapkit_surface", false), "%s reads vertex tints and weather" % kind)
		check(surface_material.get_shader_parameter("detail_enabled") == true, "%s carries its detail tile" % kind)
	var pane: StandardMaterial3D = made.materials.glass
	check(pane.transparency == BaseMaterial3D.TRANSPARENCY_ALPHA and pane.albedo_color.a < 0.5, "the canopy glass is translucent")
	if DisplayServer.get_name() == "headless":
		print("transit_detail_validator: headless, night pixels skipped")
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
	var entrance := MeshInstance3D.new()
	entrance.mesh = made.subway_entrance_near
	scene.add_child(entrance)
	var pylon := MeshInstance3D.new()
	pylon.mesh = made.station_sign_near
	pylon.position = Vector3(3.0, 0.0, 0.0)
	scene.add_child(pylon)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	scene.add_child(camera)
	camera.current = true
	var samples := {}
	# From the road: the band pictogram's white cab (x 0.315, between its
	# lamps), the tunnel ceiling just inside the mouth (beside its lamp) and
	# the sign's backlit panel (between its name bars and pictogram). From
	# above: the lit landing and the well floor just before the mouth.
	var targets := [["cab", Vector3(0.315, 0.92, -2.0), 0.008, Vector3.BACK], ["tunnel", Vector3(0.25, 0.27, -2.0), 0.004, Vector3.BACK],
		["panel", Vector3(3.0, 1.53, -2.0), 0.008, Vector3.BACK], ["landing", Vector3(0.15, 3.0, -0.49), 0.01, Vector3.DOWN], ["well", Vector3(0.15, 3.0, 0.06), 0.01, Vector3.DOWN]]
	for target: Array in targets:
		camera.size = target[2]
		camera.position = target[1]
		camera.look_at(target[1] + target[3], Vector3.UP if target[3] != Vector3.DOWN else Vector3.FORWARD)
		for sun in [-0.2, 0.8]:
			context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
			for frame in 5: await process_frame
			await RenderingServer.frame_post_draw
			var picture := root.get_texture().get_image()
			samples["%s_%s" % [target[0], "night" if sun < 0.0 else "day"]] = picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2)
	print("TRANSIT_GLOW ", samples)
	check(samples.cab_night.r > 0.6 and samples.cab_night.g > 0.6, "the sign's pictogram glows white at night: %s" % samples.cab_night)
	var peak := func(color: Color) -> float: return maxf(color.r, maxf(color.g, color.b))
	check(luminance(samples.landing_night) > 0.04 and luminance(samples.landing_night) < luminance(samples.cab_night) * 0.7, "the landing glows softly at night: %s" % samples.landing_night)
	check(luminance(samples.well_night) < luminance(samples.landing_night) * 0.5, "the well darkens before the mouth at night: %s" % samples.well_night)
	check(luminance(samples.tunnel_night) > 0.01 and luminance(samples.tunnel_night) < luminance(samples.landing_night), "the tunnel glows faintly, below the landing: %s" % samples.tunnel_night)
	check(peak.call(samples.panel_night) > 0.3 and peak.call(samples.panel_night) < 0.9, "the sign's backlit panel glows without clipping: %s" % samples.panel_night)
	for key in ["cab_day", "tunnel_day", "panel_day", "landing_day", "well_day"]:
		check(peak.call(samples[key]) < 0.1, "%s is dark in an unlit scene: %s" % [key, samples[key]])
	# Sloping ground (12 %, the steepest city cards) through the foot under the
	# entrance never shows inside the well: top-down by day only the unshaded
	# red ground could be bright.
	var ground_material := StandardMaterial3D.new()
	ground_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	ground_material.albedo_color = Color.RED
	ground_material.cull_mode = BaseMaterial3D.CULL_DISABLED
	var ground := MeshInstance3D.new()
	scene.add_child(ground)
	camera.size = 0.6
	camera.position = Vector3(0.0, 3.0, -0.12)
	camera.look_at(camera.position + Vector3.DOWN, Vector3.FORWARD)
	context.update(43200.0, 0.0, 0.0, 0.8, 1080)
	var poked := {}
	for direction: Vector2 in [Vector2(1, 1), Vector2(1, -1), Vector2(-1, 1), Vector2(-1, -1), Vector2(0, 1), Vector2(0, -1)]:
		var d := direction.normalized()
		var plane := PackedVector3Array()
		for corner: Vector2 in [Vector2(-3, -3), Vector2(3, -3), Vector2(3, 3), Vector2(-3, -3), Vector2(3, 3), Vector2(-3, 3)]:
			plane.append(Vector3(corner.x, 0.12 * corner.dot(d), corner.y))
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = plane
		var plane_mesh := ArrayMesh.new()
		plane_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		plane_mesh.surface_set_material(0, ground_material)
		ground.mesh = plane_mesh
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var picture := root.get_texture().get_image()
		var red := 0
		for y in picture.get_height():
			for x in picture.get_width():
				var pixel := picture.get_pixel(x, y)
				if pixel.r > 0.15 and pixel.r > 3.0 * pixel.g and pixel.r > 3.0 * pixel.b: red += 1
		poked[direction] = red
	print("TRANSIT_GROUND ", poked)
	check(poked.values().all(func(count: int) -> bool: return count == 0), "12 %% sloping ground never shows inside the stair well: %s" % poked)
	scene.queue_free()
	for frame in 3: await process_frame
