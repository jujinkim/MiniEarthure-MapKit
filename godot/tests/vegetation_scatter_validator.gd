extends SceneTree
## Park vegetation (vegetation_meshes.gd, grass_scatter.gd, chunk_renderer):
## deterministic grass scatter with per-tier caps and bounded steps, no grass on
## low or polar ground, tier-sized grass reservations, tree/shrub body, card and
## far ranges and shadows per tier, collision trunk coverage, crown reach
## inside the placement footprint, visible branches, and release of every cell
## and shared reservation.
const RENDER := preload("../chunk_renderer.gd")
const GRASS := preload("../grass_scatter.gd")
const VEGETATION := preload("../vegetation_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount := 0) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func record_reservation(bytes: int, box: Array, lease: Lease) -> Lease:
	box[0] = bytes
	return lease

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	scatter()
	meshes()
	await renderer()
	QUALITY.current = {}
	print("vegetation_scatter_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## A 64 m square of 2 m ground triangles (2,048), a steep bank and a sliver.
func ground(offset := Vector3.ZERO) -> PackedVector3Array:
	var points := PackedVector3Array()
	for z in 32:
		for x in 32:
			var a := offset + Vector3(x * 2.0, 0.0, -z * 2.0)
			points.append_array([a, a + Vector3(2, 0, 0), a + Vector3(0, 0, -2), a + Vector3(2, 0, 0), a + Vector3(2, 0, -2), a + Vector3(0, 0, -2)])
	points.append_array([offset + Vector3(0, 0, 4), offset + Vector3(4, 0, 4), offset + Vector3(0, 6, 4)])
	return points

func run_scatter(parts: Array, cap: int) -> Dictionary:
	var state := GRASS.begin(parts, Vector2i(3, -2), cap, Color(0.2, 0.3, 0.1))
	var steps := 0
	var before := 0
	while not GRASS.advance(state):
		steps += 1
		check(state.emitted - before <= GRASS.SCATTER_INSTANCES, "one step emits at most %d tufts" % GRASS.SCATTER_INSTANCES)
		before = state.emitted
	var groups := []
	while true:
		var group := GRASS.take_group(state)
		if group.is_empty(): break
		groups.append(group)
	return {"state": state, "steps": steps, "groups": groups}

func scatter() -> void:
	var points := ground()
	var high := run_scatter([points], 4000)
	print("grass scatter: %d steps, peak step %d usec" % [high.steps, high.state.peak_usec])
	check(int(high.state.peak_usec) < 8000, "one scatter step stays within a frame work budget: %d usec" % high.state.peak_usec)
	var again := run_scatter([points], 4000)
	var count := 0
	var flowers := 0
	for group: Dictionary in high.groups:
		count += int(group.count)
		var buffer: PackedFloat32Array = group.buffer
		for i in group.count:
			var o := Vector3(buffer[i * 16 + 3], buffer[i * 16 + 7], buffer[i * 16 + 11])
			check(o.x >= 0.0 and o.x <= 64.0 and o.z <= 0.0 and o.z >= -64.0 and is_equal_approx(o.y, -0.02), "tufts lie on ground triangles, not the steep bank: %s" % o)
			var cell := Vector2i(floori(o.x / GRASS.BUCKET_METRES), floori(o.z / GRASS.BUCKET_METRES))
			check(cell == group.group, "a group holds only its own 32 m square")
			check(buffer[i * 16 + 15] >= 1.0, "every tuft carries instance data")
			flowers += int(buffer[i * 16 + 15] >= 2.0)
	# 4,096 m2 at 0.9 tufts per m2 stays under the high cap.
	check(count > 3400 and count < 3900, "density bound below the cap: %d" % count)
	check(flowers > count / 25 and flowers < count / 5, "about one tuft in ten carries a flower, in patches: %d" % flowers)
	check(high.steps > 8, "scatter spans bounded steps: %d" % high.steps)
	check(high.groups.map(func(g): return g.buffer) == again.groups.map(func(g): return g.buffer), "identical input gives identical tufts")
	# Splitting the triangles into other parts (batches) changes nothing.
	var split := [points.slice(0, 3000), points.slice(3000)]
	check(run_scatter(split, 4000).groups.map(func(g): return g.buffer) == high.groups.map(func(g): return g.buffer), "batch boundaries do not move tufts")
	for cap in [0, 1500]:
		var limited := run_scatter([points], cap)
		var total := 0
		for group: Dictionary in limited.groups: total += int(group.count)
		check(total <= cap and total >= cap * 0.9, "cap %d holds: %d" % [cap, total])
	check(run_scatter([points], 999999).state.cap == RENDER.PLAN.GRASS_INSTANCES, "the memory plan bounds every cap")
	var moved := run_scatter([ground(Vector3(64, 0, 0))], 4000)
	check(moved.groups.map(func(g): return g.buffer) != high.groups.map(func(g): return g.buffer), "other triangles scatter differently")
	check(VEGETATION.hash32(-5) == VEGETATION.hash32(-5) and VEGETATION.hash32(1) != VEGETATION.hash32(2) and VEGETATION.unit(7, 3, 1) == VEGETATION.unit(7, 3, 1), "integer hash is stable")

func meshes() -> void:
	var first: Dictionary = VEGETATION.begin()
	var steps := 0
	var timings := []
	while true:
		var name: String = VEGETATION.STEPS[first.step]
		var started := Time.get_ticks_usec()
		var done := VEGETATION.advance(first)
		timings.append("%s %d" % [name, Time.get_ticks_usec() - started])
		if done: break
		steps += 1
	print("shared vegetation steps (usec): ", ", ".join(timings))
	var second: Dictionary = VEGETATION.begin()
	while not VEGETATION.advance(second): pass
	check(steps + 1 == VEGETATION.STEPS.size() and steps > 20, "the shared set builds one part per step: %d" % steps)
	for kind in ["tree_body", "tree_cards", "tree_far", "shrub_body", "shrub_cards", "shrub_far"]:
		for variant in 3:
			var a: ArrayMesh = first.result[kind][variant]
			var b: ArrayMesh = second.result[kind][variant]
			check(a.get_surface_count() == 1 and a.surface_get_arrays(0) == b.surface_get_arrays(0), "%s %d builds identically" % [kind, variant])
	for plant in ["tree", "shrub"]:
		# Every vertex at the largest scale stays inside the placement footprint
		# (CITY_TREE_RADIUS 120 cm; shrubs the baseline 42 cm), the body's
		# shell also grown back to the full crown as on low.
		var limit := 1.2 if plant == "tree" else 0.42
		var largest: float = (VEGETATION.TREE_SCALE if plant == "tree" else VEGETATION.SHRUB_SCALE).y
		for variant in 3:
			var widest := 0.0
			for part in VEGETATION.KINDS:
				var arrays: Array = (first.result[plant + "_" + part][variant] as ArrayMesh).surface_get_arrays(0)
				var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
				var marks: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
				for i in vertices.size():
					var p := vertices[i]
					if part == "body" and marks[i].y > 0.0: p = Vector3(0.0, marks[i].x, 0.0) + (p - Vector3(0.0, marks[i].x, 0.0)) * marks[i].y
					widest = maxf(widest, Vector2(p.x, p.z).length() * largest)
			check(widest <= limit + 0.001 and widest >= limit * 0.85, "%s %d reaches %.3f m of %.2f m" % [plant, variant, widest, limit])
	for variant in 3:
		var body: ArrayMesh = first.result.tree_body[variant]
		var cards: ArrayMesh = first.result.tree_cards[variant]
		var triangles := body.surface_get_array_index_len(0) / 3
		var card_triangles := cards.surface_get_array_index_len(0) / 3
		check(triangles >= 400 and triangles <= 600 and card_triangles >= 240 and card_triangles <= 400, "near tree %d: trunk, branches and hull (%d) plus leaf cards (%d)" % [variant, triangles, card_triangles])
		var far: ArrayMesh = first.result.tree_far[variant]
		check(far.surface_get_array_index_len(0) / 3 <= 120, "far tree %d is coarse" % variant)
		var arrays: Array = body.surface_get_arrays(0)
		var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
		var colors: PackedColorArray = arrays[Mesh.ARRAY_COLOR]
		var marks: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
		# The 14 cm collision box (half diagonal 0.099 m) stays inside the trunk
		# up to 1.4 m at the smallest scale: every trunk ring (bark, COLOR.a 0)
		# up to the first one above 1.4 / 0.85 m, between which the tube is linear.
		var narrowest := INF
		var underside := INF
		var top := 0.0
		for ring: Vector2 in VEGETATION.TRUNK:
			if top < 1.4 / 0.85: top = ring.x
		for i in vertices.size():
			if marks[i].y > 0.0: underside = minf(underside, vertices[i].y)
			if colors[i].a > 0.5 or vertices[i].y > top + 0.001 or vertices[i].y < 0.0: continue
			if not VEGETATION.TRUNK.any(func(ring): return absf(ring.x - vertices[i].y) < 0.0005): continue
			narrowest = minf(narrowest, Vector2(vertices[i].x, vertices[i].z).length() * cos(PI / 8.0) * 0.85)
		check(narrowest >= 0.099, "trunk %d covers the collision box: %.3f m" % [variant, narrowest])
		# Branches fork below the crown's underside: some bark away from the
		# trunk axis lies under the hull.
		var forks := 0
		for i in vertices.size():
			if colors[i].a < 0.5 and vertices[i].y < underside - 0.2 and Vector2(vertices[i].x, vertices[i].z).length() > 0.2: forks += 1
		check(forks >= 10, "tree %d shows branches under its crown: %d vertices" % [variant, forks])
	var grass: ArrayMesh = first.result.grass
	check(grass.surface_get_array_index_len(0) / 3 <= 90, "grass tuft stays small")
	for kind in ["body", "cards", "grass"]:
		var material: ShaderMaterial = first.result.materials[kind]
		check(material.get_meta("mapkit_vegetation_shader", "") == kind, "%s material opts into vegetation quality" % kind)
		for level in 3:
			QUALITY.apply_material(material, QUALITY.profile(level))
			check(material.get_shader_parameter("wind_strength") == QUALITY.profile(level).vegetation_motion, "wind follows the tier")
			check(material.get_shader_parameter("cards_hidden") == (level == 0), "low grows the body hull in place of cards")

func fixture(polar := false) -> Dictionary:
	var points := ground()
	var chunk := {"cell": Vector2i(2, 5), "triangles": [], "objects": [], "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for i in points.size() / 3:
		chunk.triangles.append({"surface": "grass", "object_id": "ground"})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	if polar: chunk.presentation = {"environment_json": JSON.stringify({"climate": "polar"})}
	for i in 12:
		chunk.objects.append({"id": "tree-%d" % i, "asset_id": "builtin:city_tree", "position": [i * 500, 0, i * 300], "quarter_turns": 0})
	for i in 6:
		chunk.objects.append({"id": "shrub-%d" % i, "asset_id": "builtin:shrub", "position": [i * 400, 0, 2000], "quarter_turns": 1})
	chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var counts := {}
	for polar in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(polar)
			var planned := RENDER.PLAN.estimate(chunk, 0)
			check(planned >= RENDER.PLAN.GRASS_CELL_BYTES and RENDER.PLAN.upper_bound({"triangles": chunk.triangles.size(), "objects": chunk.objects.size()}) >= planned, "grass cells estimate the grass allowance within the upper bound")
			var cap: int = 0 if polar else int(QUALITY.profile(level).grass_instances)
			var lease := Lease.new(planned)
			var reserved := [0]
			var job := RENDER.begin(chunk, parent, record_reservation.bind(reserved, lease), planned, cache)
			check(reserved[0] == (planned if cap > 0 else planned - RENDER.PLAN.GRASS_CELL_BYTES), "tier %d polar %s reserves grass only when it builds some: %d of %d" % [level, polar, reserved[0], planned])
			var steps := 0
			while not RENDER.advance(job): steps += 1
			check(job.error.is_empty(), "attachment completes: %s" % job.error)
			var grass := 0
			var trees := {"body": 0, "cards": 0, "far": 0}
			var value := QUALITY.profile(level)
			for node: Node in parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false):
				var lod: String = node.get_meta("mapkit_vegetation_lod")
				var kind: String = node.get_meta("mapkit_vegetation_kind")
				var near := float(value.vegetation_near) * (0.6 if kind == "shrub" else 1.0) + QUALITY.GROUP_REACH
				if lod == "grass": grass += node.multimesh.instance_count
				elif kind == "tree": trees[lod] += node.multimesh.instance_count
				check(node.get_meta("mapkit_decoration", false) and node.multimesh.use_custom_data, "vegetation nodes carry decoration ranges and instance data")
				if lod in ["body", "cards"]:
					check(node.visible == (lod == "body" or level > 0) and is_equal_approx(node.visibility_range_end, near), "near range per tier covers the whole group")
				if lod == "far":
					check(is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == (float(value.decoration_distance) if kind == "shrub" else 0.0), "far begins where near ends; far trees keep their crowns")
				if lod == "grass": check(node.visible and node.visibility_range_end > float(value.grass_distance), "grass range per tier")
				var shadows: bool = kind == "tree" and lod != "grass" and level >= (2 if lod == "cards" else 1)
				check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), "%s %s shadows on tier %d" % [kind, lod, level])
			counts[[polar, level]] = grass
			check(trees.body == 12 and trees.cards == 12 and trees.far == 12, "every tree has body, cards and far instances: %s" % trees)
			check(grass <= cap and (cap == 0 or grass >= mini(cap, 3400) * 0.9), "tier %d polar %s holds %d tufts (cap %d)" % [level, polar, grass, cap])
			check(job.root.get_meta("mapkit_render_root", false) and steps > 20, "vegetation work spans admitted steps: %d" % steps)
			# Live tier changes reach existing vegetation nodes.
			QUALITY.apply_tree(job.root, QUALITY.profile(0))
			for node: Node in parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false):
				var lod: String = node.get_meta("mapkit_vegetation_lod")
				if lod in ["cards", "grass"]: check(not node.visible, "low hides cards and grass")
				elif lod == "body": check(node.visible and node.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "low keeps near bodies without shadows")
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), "the cell lease retires with its vegetation nodes")
	check(counts[[false, 0]] == 0 and counts[[false, 1]] > 0 and counts[[false, 2]] > counts[[false, 1]], "no grass on low, more on high: %s" % counts)
	# GPU pressure lowers effects but keeps the ceiling's texture tier: cells
	# loaded meanwhile build the ceiling's grass, hidden until quality returns.
	var pressured := QUALITY.profile(0)
	pressured.texture_size = QUALITY.profile(2).texture_size
	check(QUALITY.grass_cap(pressured) == 4000 and QUALITY.grass_cap(QUALITY.profile(1)) == 1500 and QUALITY.grass_cap(QUALITY.profile(0)) == 0, "grass builds at the cache texture tier")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "shared vegetation retires after its last cell")
	# A refused shared reservation skips grass but fails a cell with trees.
	var denied := CACHE.new(func(bytes): return null if bytes == CACHE.VEGETATION_BYTES else Lease.new(bytes))
	QUALITY.current = QUALITY.profile(2)
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture()
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	while not RENDER.advance(job): pass
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET", "denied vegetation fails a cell with trees")
	chunk.objects.clear()
	chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	job = RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	while not RENDER.advance(job): pass
	check(job.error.is_empty() and parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false).is_empty(), "denied vegetation skips optional grass: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	await process_frame
