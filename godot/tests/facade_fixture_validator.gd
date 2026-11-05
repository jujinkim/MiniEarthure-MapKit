extends SceneTree
## Facade and rooftop fixtures (facade_fixture_meshes.gd, native
## facade_fixtures.rs and roof_fixtures.rs): the shared set builds one part
## per call, identically, inside its memory allowance and inside the native
## clearance boxes (rail posts and legs where the shader anchors them) and
## binds the record palettes; native placement through the bridge puts blade
## signs only on commercial walls (outward, at 1:3 scale), air-conditioner
## units on house, commercial and public walls, none on apartments, and
## rooftop fixtures on the roofs (penthouses on the tall ones, rims along
## their walls); the renderer instances exactly the packed list per kind and
## square with the shared meshes, design bits and transforms scaled by kind,
## sets visibility per tier (low: far blade signs, penthouses and tanks only),
## reserves and releases the cell's bytes with its lease, skips fixtures when
## the shared set is refused and draws them from the process-lifetime set
## without a cache. Rendered runs also compare a blade face and a penthouse
## door lamp by night and day.
const RENDER := preload("../chunk_renderer.gd")
const FIXTURES := preload("../facade_fixture_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const ENVIRONMENT := preload("../environment_materials.gd")
const PLAN := preload("../render_memory.gd")
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
	var chunk := native()
	if not chunk.is_empty():
		await renderer(chunk)
	await night()
	QUALITY.current = {}
	print("facade_fixture_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## The vertex shader's anchors (facade_fixture.gdshader) for a rim or solar
## row `ratio` = pitch / length: posts spread, legs keep their width.
static func anchored(p: Vector3, uv: Vector2, uv2: Vector2, ratio: float) -> Vector3:
	var role := int(roundf(uv.x))
	if role == FIXTURES.Role.POST:
		var intervals := clampf(roundf(1.0 / (0.6 * ratio)), 1.0, 31.0)
		if uv2.x > intervals + 0.5: return Vector3.ZERO
		return Vector3(lerpf(0.015 * ratio, 1.0 - 0.015 * ratio, uv2.x / intervals) + p.x * ratio, p.y, p.z)
	if role == FIXTURES.Role.LEG: return Vector3(uv2.x + p.x * ratio, p.y, p.z)
	return p

## Native clearance boxes (facade_fixtures.rs, roof_fixtures.rs, scale 1,
## MARGIN 0.03): every mesh vertex stays inside them; wall plates may reach
## the wall (z 0), rooftop fixtures stand on the roof (y 0). Penthouses and
## tanks are unit boxes; rims and solar rows run x 0..1 (checked anchored for
## an eight-floor length).
func meshes() -> void:
	var lease := Lease.new()
	var job := {}
	var steps := 0
	while not FIXTURES.build(job, lease): steps += 1
	check(steps == FIXTURES.STEPS.size() - 1, "one part per call: %d steps" % (steps + 1))
	var again := {}
	while not FIXTURES.build(again): pass
	var vertices := 0
	var indices := 0
	var boxes := {"blade": AABB(Vector3(-FIXTURES.BLADE_THICKNESS * 0.5 - 0.03, -0.03, 0.0), Vector3(FIXTURES.BLADE_THICKNESS + 0.06, FIXTURES.BLADE_HEIGHT + 0.06, FIXTURES.BLADE_STANDOFF + FIXTURES.BLADE_WIDTH + 0.03)),
		"ac": AABB(Vector3(-FIXTURES.AC_WIDTH * 0.5 - 0.03, -FIXTURES.AC_RACK - 0.03, 0.0), Vector3(FIXTURES.AC_WIDTH + 0.06, FIXTURES.AC_RACK + FIXTURES.AC_HEIGHT + 0.06, FIXTURES.AC_STANDOFF + FIXTURES.AC_DEPTH + 0.03)),
		"penthouse": AABB(Vector3(-0.5, 0.0, -0.5), Vector3.ONE), "tank": AABB(Vector3(-0.5, 0.0, -0.5), Vector3.ONE),
		"condenser": AABB(Vector3(-FIXTURES.CONDENSER_WIDTH * 0.5 - 0.03, 0.0, -FIXTURES.CONDENSER_DEPTH * 0.5 - 0.03), Vector3(FIXTURES.CONDENSER_WIDTH + 0.06, FIXTURES.CONDENSER_HEIGHT + 0.03, FIXTURES.CONDENSER_DEPTH + 0.06)),
		"edge": AABB(Vector3(0.0, 0.0, 0.0), Vector3(1.0, FIXTURES.RIM_HEIGHT + FIXTURES.RAIL_HEIGHT + 0.03, FIXTURES.RIM_THICKNESS + 0.03)),
		"solar": AABB(Vector3(-0.03, 0.0, -FIXTURES.SOLAR_DEPTH * 0.5 - 0.03), Vector3(1.06, FIXTURES.SOLAR_HEIGHT + 0.03, FIXTURES.SOLAR_DEPTH + 0.06)),
		"mast": AABB(Vector3(-FIXTURES.MAST_FOOTPRINT * 0.5 - 0.03, 0.0, -FIXTURES.MAST_FOOTPRINT * 0.5 - 0.03), Vector3(FIXTURES.MAST_FOOTPRINT + 0.06, FIXTURES.ANTENNA_HEIGHT + 0.03, FIXTURES.MAST_FOOTPRINT + 0.06))}
	var counts := []
	for kind: String in FIXTURES.KINDS:
		for lod: String in FIXTURES.LODS:
			var mesh: ArrayMesh = job.result[kind + "_" + lod]
			var other: ArrayMesh = again.result[kind + "_" + lod]
			check(mesh.get_surface_count() == 1 and mesh.surface_get_material(0) == job.result.material, "%s %s: one surface with the shared material" % [kind, lod])
			check(mesh.surface_get_arrays(0) == other.surface_get_arrays(0), "%s %s builds identically" % [kind, lod])
			var arrays := mesh.surface_get_arrays(0)
			var points: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
			vertices += points.size()
			indices += mesh.surface_get_array_index_len(0)
			counts.append("%s %s %d triangles" % [kind, lod, mesh.surface_get_array_index_len(0) / 3])
			var box: AABB = boxes[kind].grow(0.0015)
			var uvs: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
			var uv2s: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV2]
			var outside := []
			for k in points.size():
				var p := anchored(points[k], uvs[k], uv2s[k], 1.0 / 8.0) if kind in ["edge", "solar"] else points[k]
				if not box.has_point(p): outside.append(p)
			check(outside.is_empty(), "%s %s stays inside its clearance box: %s outside %s" % [kind, lod, outside.slice(0, 4), box])
			if kind == "blade":
				# Both broad faces carry the sign (role 1) facing +-x, face
				# coordinates 0..1.
				var uv: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
				var uv2: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV2]
				var normals: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
				var faces := 0
				for i in uv.size():
					if int(uv[i].x) != FIXTURES.Role.FACE: continue
					faces += 1
					check(absf(normals[i].x) > 0.999 and uv2[i].x >= 0.0 and uv2[i].x <= 1.0 and uv2[i].y >= 0.0 and uv2[i].y <= 1.0, "blade faces look along the wall with 0..1 face coordinates")
				check(faces == 8, "two sign faces of four vertices: %d" % faces)
	print("facade fixtures: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared facade fixture set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, FIXTURES.SHARED_BYTES])
	check(mesh_bytes + 65536 <= FIXTURES.SHARED_BYTES, "the shared set fits its allowance: %d B" % mesh_bytes)
	check(lease.refs.size() == FIXTURES.STEPS.size() + 1, "the lease tracks the material, its shader and every mesh")
	var material: ShaderMaterial = job.result.material
	check((material.get_shader_parameter("fixture_body") as PackedVector3Array).size() == 48 and (material.get_shader_parameter("fixture_roof") as PackedVector3Array).size() == 24, "the material binds the record palettes")

## A 1:3 document (100 cm floors) with a commercial block, a brick house, a
## public hall and an apartment block, generated and decorated natively.
func document() -> String:
	var building := func(id: String, rect: Array, height: int, usage: String, material: String) -> Dictionary:
		return {"id": id, "footprint": [[rect[0], rect[1]], [rect[2], rect[1]], [rect[2], rect[3]], [rect[0], rect[3]]], "base_cm": 0, "height_cm": height, "usage": usage, "material": material, "roof": "flat"}
	var d := {"free_roam": false, "map_id": "facade-fixtures", "revision": 1, "bounds": {"min": [0, 0], "max": [102400, 102400]}, "cell_size_cm": 51200,
		"seed": 42, "recipe_version": 1, "theme": "default", "terrain_base_cm": 0, "heightmaps": [],
		"nodes": [{"id": "west", "position": [0, 20, 25600], "level": 0}, {"id": "east", "position": [102400, 20, 25600], "level": 0}],
		"roads": [{"id": "ground-road", "from": "west", "to": "east", "points": [[0, 20, 25600], [51200, 20, 25600], [102400, 20, 25600]], "widths_cm": [800, 800], "surfaces": ["asphalt", "asphalt"], "kind": "ground", "clearance_cm": null, "sidewalk_cm": null}],
		"buildings": [building.call("shop", [30000, 30000, 32000, 34000], 1500, "commercial", "concrete"), building.call("home", [27000, 10000, 28200, 13000], 900, "residential", "brick"),
			building.call("hall", [34000, 10000, 36000, 13000], 900, "public", "concrete"), building.call("flats", [40000, 40000, 42000, 42000], 3000, "residential", "concrete")],
		"zones": [], "assets": [], "placements": [],
		"attributions": [{"source": "osm#layer", "license": "ODbL-1.0", "notice": JSON.stringify({"adapter": "osm-extract-v1", "authored_units": {"metres_per_unit": 1, "source_denominator": 3}})}],
		"provenance": {"tool_id": "facade-fixture-validator", "version": "1", "build_id": "test", "fingerprint": "untrusted-and-accepted", "first_created": "2026-11-05T00:00:00Z", "last_edited": "2026-11-05T00:00:00Z"}}
	return JSON.stringify(d)

func native() -> Dictionary:
	if not ClassDB.class_exists("MapKitBridge"):
		check(false, "the native bridge is loaded")
		return {}
	var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
	var generated: Dictionary = bridge.preview_document_packed(document(), 0, 0)
	check(generated.get("ok", false), "native preview: %s" % generated.get("error", {}))
	if not generated.get("ok", false): return {}
	var chunk: Dictionary = generated.data.chunk
	var list := FIXTURES.fixtures(chunk)
	check(int(chunk.presentation.get("facade_floor_cm", 0)) == 100 and chunk.presentation.has("facade_fixtures_capped"), "1:3 floors and the cap report")
	check(list.size() > 0 and list.size() % FIXTURES.FLOATS == 0, "fixtures listed: %d floats" % list.size())
	var kinds := {}
	for kind: String in FIXTURES.KINDS: kinds[kind] = 0
	var hosts := {}
	# Footprints (scene x and map y in metres) and roof heights.
	var sites := {"shop": [300.0, 300.0, 320.0, 340.0, 15.0], "home": [270.0, 100.0, 282.0, 130.0, 9.0], "hall": [340.0, 100.0, 360.0, 130.0, 9.0], "flats": [400.0, 400.0, 420.0, 420.0, 30.0]}
	for i in range(0, list.size(), FIXTURES.FLOATS):
		var x := list[i]
		var z := -list[i + 2]
		var kind: String = FIXTURES.KINDS[int(list[i + 4])]
		kinds[kind] += 1
		var host := "other"
		for name: String in sites:
			var r: Array = sites[name]
			if x > r[0] - 0.1 and x < r[2] + 0.1 and z > r[1] - 0.1 and z < r[3] + 0.1: host = name
		hosts["%s/%s" % [kind, host]] = int(hosts.get("%s/%s" % [kind, host], 0)) + 1
		check(is_equal_approx(list[i + 5], 1.0), "fixtures scale to the 1:3 floor")
		if kind in ["blade", "ac"]:
			# Local +z faces out of the host's footprint.
			var out := Vector3(sin(list[i + 3]), 0.0, cos(list[i + 3]))
			var ahead := Vector2(x + out.x * 0.5, z - out.z * 0.5)
			check(not (ahead.x > 300.0 and ahead.x < 320.0 and ahead.y > 300.0 and ahead.y < 340.0), "wall fixtures face out of the shop")
			check(is_zero_approx(list[i + 8]) and is_zero_approx(list[i + 9]), "wall fixtures keep their mesh size")
		else:
			check(host != "other" and is_equal_approx(list[i + 1], sites[host][4]), "rooftop fixtures stand on a roof: %s %s at %.2f" % [kind, host, list[i + 1]])
			check(list[i + 8] > 0.0 or kind in ["condenser", "mast"], "rooftop fixtures list their width")
		if kind == "blade": check(host == "shop" and list[i + 6] >= 0.7 and list[i + 6] <= 1.3 and list[i + 1] >= 1.5, "blade signs only on the commercial walls above the shop storey: %s" % host)
		if kind == "ac": check(host in ["shop", "home", "hall"], "units only on commercial, house and public walls: %s" % host)
	print("facade fixtures placed: ", hosts)
	check(kinds.blade >= 8 and kinds.ac >= 2, "blade signs and units: %s" % kinds)
	check(int(hosts.get("penthouse/flats", 0)) >= 1 and int(hosts.get("penthouse/shop", 0)) >= 1 and kinds.tank >= 1, "penthouses on the tall roofs and a tank: %s" % hosts)
	check(int(hosts.get("edge/flats", 0)) >= 4 and int(hosts.get("edge/home", 0)) >= 4, "rims along the roofs' walls: %s" % hosts)
	check(not hosts.keys().any(func(k: String) -> bool: return k.ends_with("/other")), "nothing off the buildings")
	return chunk

func renderer(chunk: Dictionary) -> void:
	var list := FIXTURES.fixtures(chunk)
	var expected := {}
	var groups := {}
	for i in range(0, list.size(), FIXTURES.FLOATS):
		var kind: String = FIXTURES.KINDS[int(list[i + 4])]
		expected[kind] = int(expected.get(kind, 0)) + 1
		groups[Vector3i(floori(list[i] / 32.0), floori(list[i + 2] / 32.0), int(list[i + 4]))] = true
	var bytes := PLAN.fixture_bytes(chunk)
	check(bytes == list.size() / FIXTURES.FLOATS * PLAN.FIXTURE_BYTES + groups.size() * PLAN.FIXTURE_GROUP_BYTES and bytes <= PLAN.FIXTURE_BOUND_BYTES, "the cell reserves its fixtures: %d B" % bytes)
	check(PLAN.FIXTURE_BOUND_BYTES == 1024 * PLAN.FIXTURE_BYTES + 16 * FIXTURES.KINDS.size() * PLAN.FIXTURE_GROUP_BYTES, "the bound covers the cap in every square and kind")
	var bare := chunk.duplicate()
	bare.presentation = chunk.presentation.duplicate()
	bare.presentation.erase("facade_fixtures")
	check(PLAN.estimate(RENDER.DATA.view(chunk), 0) - PLAN.estimate(RENDER.DATA.view(bare), 0) == bytes, "the estimate carries the fixture reservation")
	check(PLAN.upper_bound({"triangles": 0, "objects": 0, "building_prisms": 1}) - PLAN.upper_bound({"triangles": 0, "objects": 0}) == PLAN.FIXTURE_BOUND_BYTES, "cells with buildings reserve the fixture cap before generation")
	var cache := CACHE.new(func(amount): return Lease.new(amount))
	cache.quality_profile = QUALITY.profile(1)
	for level in 3:
		QUALITY.current = QUALITY.profile(level)
		var parent := Node3D.new()
		root.add_child(parent)
		var planned := PLAN.estimate(RENDER.DATA.view(chunk), 0)
		var lease := Lease.new(planned)
		var job := RENDER.begin(chunk, parent, func(amount): return lease if amount == planned else null, planned, cache)
		while not RENDER.advance(job): pass
		var label := "tier %d" % level
		check(job.error.is_empty(), label + " attaches: %s" % job.error)
		var counts := {}
		for node: MultiMeshInstance3D in parent.find_children("Fixture_*", "MultiMeshInstance3D", true, false):
			var kind := str(node.get_meta("mapkit_fixture_kind"))
			var lod := str(node.get_meta("mapkit_fixture_lod"))
			counts["%s_%s" % [kind, lod]] = int(counts.get("%s_%s" % [kind, lod], 0)) + node.multimesh.instance_count
			check(node.multimesh.mesh == cache.mesh_set(FIXTURES.KEY).get(kind + "_" + lod) and node.multimesh.use_custom_data, label + " borrows the shared mesh with custom data")
			check(node.get_meta("mapkit_static_detail", false) and node.get_meta("mapkit_decoration", false), label + " decoration metadata")
			var shown := node.visible
			if kind in FIXTURES.LOW_KINDS: check(shown == (lod == "far" or level > 0), label + " %s %s visibility" % [kind, lod])
			else: check(shown == (level > 0), label + " %s %s visibility" % [kind, lod])
			if lod == "far": check(node.visibility_range_end > node.visibility_range_begin, label + " far range ends beyond its start")
		var wanted := {}
		for kind: String in expected:
			wanted[kind + "_near"] = expected[kind]
			wanted[kind + "_far"] = expected[kind]
		check(counts == wanted, label + " instances the packed list near and far: %s vs %s" % [counts, wanted])
		if DisplayServer.get_name() != "headless": placements(parent, list, label)
		RENDER.cancel(job)
		parent.queue_free()
		for _frame in 4: await process_frame
		check(lease.retired(), label + " cell lease retires with its fixture nodes")
	check(cache.bytes() >= FIXTURES.SHARED_BYTES, "one shared fixture set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared fixture set retires after its last cell")
	# Without a cache: the process-lifetime set.
	var preview := Node3D.new()
	root.add_child(preview)
	RENDER.attach(chunk, preview)
	check(preview.find_children("Fixture_*", "MultiMeshInstance3D", true, false).size() > 0, "standalone previews draw fixtures from the process-lifetime set")
	preview.free()
	# A refused shared set skips the fixtures, never the cell.
	var denied := CACHE.new(func(amount): return null if amount == FIXTURES.SHARED_BYTES else Lease.new(amount))
	var parent := Node3D.new()
	root.add_child(parent)
	var job := RENDER.begin(chunk, parent, func(amount): return Lease.new(amount), PLAN.estimate(RENDER.DATA.view(chunk), 0), denied)
	while not RENDER.advance(job): pass
	check(job.error.is_empty() and parent.find_children("Fixture_*", "MultiMeshInstance3D", true, false).is_empty() and not job.claims.has(FIXTURES.KEY), "a refused fixture set skips the fixtures: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Every packed fixture is one instance at its mount point, turned to its
## yaw, with its design bits in the custom data (rendered runs: the dummy
## headless renderer keeps no instance data).
func placements(parent: Node3D, list: PackedFloat32Array, label: String) -> void:
	var wanted := {}
	for i in range(0, list.size(), FIXTURES.FLOATS):
		wanted["%s|%.3f,%.3f,%.3f|%d" % [FIXTURES.KINDS[int(list[i + 4])], list[i], list[i + 1], list[i + 2], int(list[i + 7])]] = [list[i + 3], list[i + 8], list[i + 9], list[i + 6]]
	var seen := 0
	for node: MultiMeshInstance3D in parent.find_children("Fixture_*_near_*", "MultiMeshInstance3D", true, false):
		var multi := node.multimesh
		for k in multi.instance_count:
			var t := multi.get_instance_transform(k)
			var c := multi.get_instance_custom_data(k)
			var bits := int(c.r) | (int(c.g) << 8) | (int(c.b) << 16)
			var key := "%s|%.3f,%.3f,%.3f|%d" % [node.get_meta("mapkit_fixture_kind"), t.origin.x, t.origin.y, t.origin.z, bits]
			check(wanted.has(key), label + " instance matches a packed fixture: " + key)
			if wanted.has(key):
				var entry: Array = wanted[key]
				var out := t.basis.z.normalized()
				check(out.is_equal_approx(Vector3(sin(entry[0]), 0.0, cos(entry[0]))), label + " instance faces its yaw")
				var kind := str(node.get_meta("mapkit_fixture_kind"))
				if kind in ["penthouse", "tank"]: check(absf(t.basis.x.length() - entry[1]) < 1e-4 and absf(t.basis.z.length() - entry[2]) < 1e-4 and absf(t.basis.y.length() - entry[3]) < 1e-4, label + " %s scales to its size" % kind)
				if kind in ["edge", "solar"]: check(absf(t.basis.x.length() - entry[1]) < 1e-4, label + " %s stretches to its length" % kind)
				seen += 1
	check(seen == list.size() / FIXTURES.FLOATS, label + " every packed fixture is instanced once: %d" % seen)

## The blade face glows by night and is dark by day in an unlit scene.
func night() -> void:
	if DisplayServer.get_name() == "headless":
		print("facade_fixture_validator: headless, blade face pixels skipped")
		return
	var context := ENVIRONMENT.new()
	var job := {}
	while not FIXTURES.build(job, null, context): pass
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
	var multi := MultiMesh.new()
	multi.transform_format = MultiMesh.TRANSFORM_3D
	multi.use_custom_data = true
	multi.mesh = job.result.blade_near
	multi.instance_count = 1
	multi.set_instance_transform(0, Transform3D())
	var bits := 0x5a3c71
	multi.set_instance_custom_data(0, Color(bits & 255, (bits >> 8) & 255, (bits >> 16) & 255, FIXTURES.BLADE_HEIGHT / FIXTURES.BLADE_WIDTH))
	var node := MultiMeshInstance3D.new()
	node.multimesh = multi
	scene.add_child(node)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 0.4
	scene.add_child(camera)
	camera.position = Vector3(1.0, FIXTURES.BLADE_HEIGHT * 0.75, FIXTURES.BLADE_STANDOFF + FIXTURES.BLADE_WIDTH * 0.5)
	camera.look_at(camera.position - Vector3.RIGHT, Vector3.UP)
	camera.current = true
	# The brightest tenth of the face's pixels (lit panel or letters).
	var levels := []
	for sun in [-0.2, 0.8]:
		context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var picture := root.get_texture().get_image()
		var values := PackedFloat32Array()
		for y in picture.get_height():
			for x in picture.get_width():
				var p := picture.get_pixel(x, y)
				values.append(maxf(p.r, maxf(p.g, p.b)))
		values.sort()
		levels.append(values[int(values.size() * 0.97)])
		var folder := OS.get_environment("FACADE_FIXTURE_CAPTURE")
		if not folder.is_empty(): picture.save_png(folder.path_join("blade_%s.png" % ("night" if sun < 0.0 else "day")))
	print("BLADE_FACE night=%.4f day=%.4f" % levels)
	check(levels[0] > 0.1 and levels[1] < 0.03, "the blade face glows at night and is dark by day unlit: %s" % [levels])
	# A penthouse door lamp (a design whose lamp burns) at 1:3 scale: faintly
	# lit at night, dark by day in the unlit scene.
	var lit := 0
	while pcg(lit ^ 3266489917) % 10 >= 7: lit += 512
	multi.mesh = job.result.penthouse_near
	multi.set_instance_transform(0, Transform3D(Basis.from_scale(Vector3(1.1, 0.9, 1.4)), Vector3.ZERO))
	multi.set_instance_custom_data(0, Color(lit & 255, (lit >> 8) & 255, (lit >> 16) & 255, 1.0))
	camera.size = 0.25
	camera.position = Vector3(0.0, 0.77 * 0.9, 2.0)
	camera.look_at(Vector3(0.0, 0.77 * 0.9, 0.0), Vector3.UP)
	var lamp := []
	for sun in [-0.2, 0.8]:
		context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
		for frame in 5: await process_frame
		await RenderingServer.frame_post_draw
		var picture := root.get_texture().get_image()
		var brightest := 0.0
		for y in picture.get_height():
			for x in picture.get_width():
				var p := picture.get_pixel(x, y)
				brightest = maxf(brightest, maxf(p.r, maxf(p.g, p.b)))
		lamp.append(brightest)
		var folder := OS.get_environment("FACADE_FIXTURE_CAPTURE")
		if not folder.is_empty(): picture.save_png(folder.path_join("lamp_%s.png" % ("night" if sun < 0.0 else "day")))
	print("DOOR_LAMP night=%.4f day=%.4f" % lamp)
	check(lamp[0] > 0.3 and lamp[1] < 0.03, "the penthouse door lamp burns at night and is dark by day unlit: %s" % [lamp])
	scene.queue_free()
	for frame in 3: await process_frame

## facade_pcg of facade_signs.gdshaderinc (32-bit).
static func pcg(v: int) -> int:
	var state := (v * 747796405 + 2891336453) & 0xFFFFFFFF
	var word := (((state >> ((state >> 28) + 4)) ^ state) * 277803737) & 0xFFFFFFFF
	return ((word >> 22) ^ word) & 0xFFFFFFFF
