extends SceneTree
## Street-life field (street_life_field.gd, pedestrian_meshes.gd,
## pedestrian.gdshader, chunk_renderer, render_resource_cache): quality keys
## per tier (none on low), the cell estimate and its return without a field,
## registration whatever the switch, a refused or low claim (no field, the
## cell still renders), walker caps and reach per tier, the toggle and a
## zero walker_count drawing nothing while lanes stay registered and turning
## on again without a reload, give-way stepping aside, holding at the kerb
## beside a vehicle, never entering a signalled crossing or a hidden cell's,
## pigeons flushing and landing, fades, the ambience query, culling by the
## cell rectangle (a camera far from a large cell's centre), fixed memory
## inside MEMORY_BYTES, and release/close dropping every pin, node and render
## resource. Seeded.
const RENDER := preload("../chunk_renderer.gd")
const FIELD := preload("../street_life_field.gd")
const MESHES := preload("../pedestrian_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
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
	var camera := Camera3D.new()
	root.add_child(camera)
	camera.position = Vector3(0, 1.5, -4)
	camera.current = true
	tiers()
	await renderer()
	await field_behaviour(camera)
	if RenderingServer.get_current_rendering_method() != "dummy" and DisplayServer.get_name() != "headless": await pixels(camera)
	else: print("street_life_field_validator: pixel checks need --rendered")
	QUALITY.current = {}
	print("street_life_field_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func on(level: int, extra := {}) -> Dictionary:
	var value := QUALITY.profile(level)
	value.street_life = true
	value.merge(extra, true)
	return value

## A block edge: two parallel lanes (scene z 0 and -8) from x -30 to 30 split
## at the crossings x 0 (free) and x 20 (signalled), a pigeon anchor and the
## ambience of the cell.
func record() -> Dictionary:
	var points := PackedVector3Array()
	var lanes := PackedVector4Array()
	for z in [0.0, -8.0]:
		for span: Vector2 in [Vector2(-30, 0), Vector2(0, 20), Vector2(20, 30)]:
			lanes.append(Vector4(points.size(), 2, 0.4, 0.3))
			points.append(Vector3(span.x, 0.03, z))
			points.append(Vector3(span.y, 0.03, z))
	var crossings := PackedVector3Array([Vector3(0, 0.03, 0), Vector3(0, 0.03, -8), Vector3(20, 0.03, 0), Vector3(20, 0.03, -8)])
	return {"points": points, "lanes": lanes, "links": PackedVector4Array(), "crossings": crossings, "signals": PackedByteArray([0, 1]),
		"ambience": Vector4(0.2, 0.5, 0.3, 0.0), "centre": Vector2(0, -4), "left_hand": false, "anchors": PackedVector3Array([Vector3(6, 0.03, 3)])}

func tiers() -> void:
	var counts := []
	for level in 3:
		var value := QUALITY.profile(level)
		counts.append([value.walker_count, value.walker_distance, value.walker_near, value.pigeon_count, value.street_life])
		var live := FIELD.live(value)
		check(int(live.walkers) == 0 and int(live.pigeons) == 0, "tier %d is off until the switch is set" % level)
		live = FIELD.live(on(level))
		check(int(live.walkers) == [0, 40, 80][level] and int(live.pigeons) == [0, 16, 32][level] and is_equal_approx(float(live.distance), [14.0, 20.0, 28.0][level]), "tier %d caps: %s" % [level, live])
	check(counts == [[0, 14.0, 0.0, 0, false], [40, 20.0, 6.0, 16, false], [80, 28.0, 8.0, 32, false]], "quality keys per tier: %s" % [counts])
	check(MESHES.box_counts() == {"near": 6, "far": 3, "pigeon": 5}, "voxel figures of 6, 3 and 5 boxes")
	var low := CACHE.new(func(bytes): return Lease.new(bytes))
	low.quality_profile = QUALITY.profile(0)
	check(low.street_life_field() == null and not low.entries.has(FIELD.KEY), "low claims no field")
	var refused := CACHE.new(func(_bytes): return null)
	refused.quality_profile = QUALITY.profile(2)
	check(refused.street_life_field() == null, "a refused claim gives no field")
	var orphan := Node3D.new()
	check(FIELD.register(record(), orphan, null) == null, "no field without a cache")
	orphan.free()

## A small legacy chunk with street-life presentation through the renderer.
func chunk() -> Dictionary:
	var value := {"cell": Vector2i(0, 0), "triangles": [], "objects": [], "scene_vertices": PackedVector3Array(), "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for t in [[Vector3(-30, 0, 5), Vector3(30, 0, 5), Vector3(30, 0, -15)], [Vector3(-30, 0, 5), Vector3(30, 0, -15), Vector3(-30, 0, -15)]]:
		value.triangles.append({"surface": "concrete", "object_id": "ground"})
		for p: Vector3 in t:
			value.scene_vertices.append(p)
			value.scene_normals.append(Vector3.UP)
			value.wall_uv.append(Vector2.ZERO)
	var r := record()
	value.presentation = {"walk_points": r.points, "walk_lanes": r.lanes, "walk_links": r.links, "walk_crossings": r.crossings,
		"walk_crossing_signals": r.signals, "ambience": r.ambience, "cell_bounds_json": JSON.stringify({"min": [-3000, -500], "max": [3000, 1500]})}
	value.objects.append({"id": "bench-a", "asset_id": "builtin:bench", "position": [600, 3, -300], "quarter_turns": 0})
	value.render_batches = PLAN.prepare_batches(value)
	return value

func renderer() -> void:
	var view := chunk()
	var bytes := PLAN.street_life_bytes(view)
	check(bytes == PLAN.STREET_LIFE_CELL_BYTES + (12 + 4) * PLAN.STREET_LIFE_POINT_BYTES + (6 + 2) * PLAN.STREET_LIFE_EDGE_BYTES, "the cell estimate counts points, mouths and edges: %d" % bytes)
	check(PLAN.street_life_bytes({"presentation": {}}) == 0, "a cell without street life reserves nothing")
	var bare := view.duplicate()
	bare.presentation = {}
	check(PLAN.estimate(view, 0) - PLAN.estimate(bare, 0) == bytes, "the estimate holds the cell's graph")
	var record_value := FIELD.record(view)
	check(record_value.anchors.size() <= 1 and is_equal_approx(record_value.centre.x, 0.0) and is_equal_approx(record_value.centre.y, -5.0), "the record carries the cell centre and hashed anchors: %s" % record_value.centre)
	check(record_value.rect.is_equal_approx(Rect2(-30, -15, 60, 20)), "the record carries the cell rectangle in scene x/z: %s" % record_value.rect)
	for case: Array in [[2, true], [1, true], [0, true], [2, false]]:
		var level: int = case[0]
		var grant: bool = case[1]
		QUALITY.current = QUALITY.profile(level)
		var cache := CACHE.new(func(amount): return null if (not grant and amount == FIELD.MEMORY_BYTES) else Lease.new(amount))
		cache.quality_profile = QUALITY.profile(level)
		var parent := Node3D.new()
		root.add_child(parent)
		var planned := PLAN.estimate(view, 0)
		var reserved := [0]
		var job := RENDER.begin(view, parent, func(amount): reserved[0] = amount; return Lease.new(amount), planned, cache)
		while not RENDER.advance(job): pass
		check(job.error.is_empty(), "tier %d grant %s: the cell renders: %s" % [level, grant, job.error])
		var expect := level > 0 and grant
		check(job.has("street_life_cell") == expect, "tier %d grant %s registers street life: %s" % [level, grant, job.has("street_life_cell")])
		check(reserved[0] == planned - (0 if level > 0 else bytes), "tier %d reserves the estimate%s: %d of %d" % [level, " less the unused graph" if level == 0 else "", reserved[0], planned])
		if expect:
			var field: RefCounted = cache.entries[FIELD.KEY].street_life_field
			check(field.cells.size() == 1 and field.node_pos.size() > 0, "lanes register while the switch is off")
			check(int(cache.entries[FIELD.KEY].pins) == 1 and int(cache.entries[FIELD.KEY].bytes) == FIELD.MEMORY_BYTES, "one pin of the fixed claim")
			RENDER.cancel(job)
			check(int(cache.entries[FIELD.KEY].pins) == 0 and field.cells.is_empty() and field.node_pos.is_empty() and field.node_refs.is_empty() and field.node_grid.is_empty(), "cancel releases the cell, its nodes and the pin")
		else:
			RENDER.cancel(job)
		cache.shutdown()
		if expect: check(not cache.entries.has(FIELD.KEY), "shutdown evicts the field")
		parent.queue_free()
		await process_frame
	QUALITY.current = {}

## Weak references to the field's MultiMeshes and material.
func resources_of(field: RefCounted) -> Array:
	var held: Array = field.parts.values().map(func(part: Dictionary) -> WeakRef: return weakref(part.multimesh))
	held.append(weakref(field.material))
	return held

func walker_on(field: RefCounted, cell: RefCounted, edge: int, s: float, forward := true) -> int:
	return field.add_walker(cell, edge, s, forward)

func run_steps(field: RefCounted, count: int) -> void:
	for i in count: field.tick_step(0.05)

func field_behaviour(camera: Camera3D) -> void:
	var cache := CACHE.new(func(amount): return Lease.new(amount))
	cache.quality_profile = QUALITY.profile(2)
	var parent := Node3D.new()
	root.add_child(parent)
	var cell: RefCounted = FIELD.register(record(), parent, cache)
	check(cell != null, "the cell registers")
	var field: RefCounted = cache.entries[FIELD.KEY].street_life_field
	field.manual = true
	field.seed_random(7)
	# Six lanes and two crossings share eight nodes at x -30, 0, 20, 30 on both sides.
	check(field.node_pos.size() == 8, "lane ends and crossing mouths share nodes: %d" % field.node_pos.size())
	# Switch off: nothing draws, the lanes stay.
	QUALITY.current = QUALITY.profile(2)
	run_steps(field, 40)
	check(field.walker_count() == 0 and field.draw_cost().visible_draws == 0 and field.cells.size() == 1, "off draws nothing; lanes stay registered")
	# On without a reload.
	QUALITY.current = on(2)
	run_steps(field, 200)
	var cost: Dictionary = field.draw_cost()
	print("high: %d walkers, %s, %s" % [field.walker_count(), cost, field.stats])
	check(field.walker_count() > 0 and field.walker_count() <= 80, "turning on spawns walkers: %d" % field.walker_count())
	check(cost.near + cost.far > 0 and cost.visible_draws > 0, "walkers draw: %s" % cost)
	for p: Vector3 in field.w_position:
		check(Vector2(p.x - camera.position.x, p.z - camera.position.z).length() <= 28.0 + FIELD.DROP_MARGIN, "walkers stay within reach")
	# Six lanes of 120 m whose segment middles lie within 28 m, at 0.3 per metre.
	var expected := 36
	check(field.population == expected, "population follows lane length x density: %d of %d" % [field.population, expected])
	# Medium caps and reach.
	QUALITY.current = on(1)
	field.population_at = -1.0
	run_steps(field, 200)
	check(field.walker_count() <= 40, "medium caps walkers: %d" % field.walker_count())
	for p: Vector3 in field.w_position:
		check(Vector2(p.x - camera.position.x, p.z - camera.position.z).length() <= 20.0 + FIELD.DROP_MARGIN, "medium reach")
	# Night and rain thin the crowd.
	var day: int = field.population
	field.daylight = 0.0
	field.wet = true
	field._count_population(camera.position, 20.0, 40)
	check(day == 30 and field.population == int(floor(float(day) * 0.25 * 0.6 + 0.5)), "night and rain thin the crowd: %d of %d" % [field.population, day])
	# A zero walker_count draws nothing.
	QUALITY.current = on(2, {"walker_count": 0, "pigeon_count": 0})
	run_steps(field, 5)
	check(field.walker_count() == 0 and field.draw_cost().visible_draws == 0 and field.cells.size() == 1, "walker_count 0 draws nothing; lanes stay")
	QUALITY.current = on(2)
	run_steps(field, 100)
	check(field.walker_count() > 0, "walkers return")
	field._clear_walkers()
	# Give way: a car meeting a walker head-on on its lane.
	var w := walker_on(field, cell, 1, 5.0)
	var start: float = field.w_lateral[w]
	for i in 20:
		field.set_obstacles(PackedVector3Array([field.w_position[w] + Vector3(2.4 - i * 0.05, 0.0, 0.0)]), PackedVector3Array([Vector3(-1.0, 0, 0)]))
		field._walk(w, 0.05)
	check(absf(field.w_lateral[w] - start) > 0.1 and int(field.stats.gave_way) > 0, "give-way steps aside: %.2f -> %.2f" % [start, field.w_lateral[w]])
	field._clear_walkers()
	# Boxed in: a car creeping at the walker along the sidewalk; it steps
	# aside to the edge, stands, then turns round.
	w = walker_on(field, cell, 1, 5.0)
	var forward: bool = field._flag(w, FIELD.FORWARD_FLAG)
	var stood := 0
	for i in 200:
		if field._flag(w, FIELD.FORWARD_FLAG) != forward: break
		field.set_obstacles(PackedVector3Array([field.w_position[w] + Vector3(0.9, 0, 0)]), PackedVector3Array([Vector3(-0.3, 0, 0)]))
		field._walk(w, 0.05)
		if not field._flag(w, FIELD.MOVING_FLAG): stood += 1
	check(field._flag(w, FIELD.FORWARD_FLAG) != forward and stood >= 30, "boxed in it stands and turns round after %d frames" % stood)
	field._clear_walkers()
	# Hold at the kerb: at the node x 0 with a car beside the free crossing.
	field.set_obstacles(PackedVector3Array([Vector3(1.5, 0.0, -4.0)]), PackedVector3Array([Vector3.ZERO]))
	w = walker_on(field, cell, 0, 30.0)
	field._set_flag(w, FIELD.INTENT_FLAG, true)
	field._arrive(w)
	check(field.w_waiting[w] >= 0 and cell.kinds[field.w_edge[w]] != FIELD.CROSSING, "a walker holds at the kerb beside a car")
	var held: Vector3 = field.w_position[w]
	for i in 20:
		field.set_obstacles(PackedVector3Array([Vector3(1.5, 0.0, -4.0)]), PackedVector3Array([Vector3.ZERO]))
		field._walk(w, 0.05)
	check(field.w_waiting[w] >= 0 and field.w_position[w].distance_to(held) < 0.05, "it keeps standing while the car is there")
	field.set_obstacles(PackedVector3Array([Vector3(40.0, 0.0, 40.0)]), PackedVector3Array([Vector3.ZERO]))
	field._walk(w, 0.05)
	check(field.w_waiting[w] < 0 and cell.kinds[field.w_edge[w]] == FIELD.CROSSING, "it crosses once the car has gone")
	field._clear_walkers()
	# A hidden cell: the walker waiting for its crossing turns round, and no route enters it.
	field.set_obstacles(PackedVector3Array([Vector3(1.5, 0.0, -4.0)]), PackedVector3Array([Vector3.ZERO]))
	w = walker_on(field, cell, 0, 30.0)
	field._set_flag(w, FIELD.INTENT_FLAG, true)
	field._arrive(w)
	var waited: bool = field.w_waiting[w] >= 0
	cell.visible = false
	field._hold(w, 0.05)
	check(waited and field.w_waiting[w] < 0 and field._route(cell, 6, 0) == 0 and field._route(cell, 0, 1) == 0, "a hidden cell's crossing is never entered")
	cell.visible = true
	check(field._route(cell, 6, 0) == 2, "a shown cell's free crossing is a route again")
	field._clear_walkers()
	# A car about to arrive also blocks the crossing.
	field.set_obstacles(PackedVector3Array([Vector3(6.0, 0.0, -4.0)]), PackedVector3Array([Vector3(-3.0, 0, 0)]))
	check(not field._crossing_clear(cell, 6), "a vehicle arriving within 1.5 s blocks the crossing")
	field.set_obstacles(PackedVector3Array([Vector3(6.0, 0.0, -4.0)]), PackedVector3Array([Vector3(3.0, 0, 0)]))
	check(field._crossing_clear(cell, 6), "a vehicle driving away does not")
	# Never into the signalled crossing at x 20, even when meaning to cross.
	field.set_obstacles(PackedVector3Array(), PackedVector3Array())
	var entered := 0
	for i in 60:
		w = walker_on(field, cell, 1, 20.0)
		field._set_flag(w, FIELD.INTENT_FLAG, true)
		field._arrive(w)
		if cell.kinds[field.w_edge[w]] == FIELD.CROSSING or field.w_waiting[w] >= 0: entered += 1
	field._clear_walkers()
	check(entered == 0 and cell.signalled[7] == 1, "walkers never enter a signalled crossing: %d" % entered)
	# One time in three at a free crossing.
	var crossed := 0
	for i in 300:
		w = walker_on(field, cell, 0, 30.0)
		field._arrive(w)
		if cell.kinds[field.w_edge[w]] == FIELD.CROSSING: crossed += 1
	field._clear_walkers()
	check(crossed > 60 and crossed < 140, "a free crossing is taken about one time in three: %d of 300" % crossed)
	# Fades near the camera.
	check(field._fade(camera.position + Vector3(0.3, 0, 0), camera.position) == 0.0 and field._fade(camera.position + Vector3(2, 0, 0), camera.position) == 1.0, "walkers fade at the camera")
	field.set_obstacles(PackedVector3Array([Vector3(3, 0, 3)]), PackedVector3Array([Vector3.ZERO]))
	check(field._fade(Vector3(3.1, 0, 3), camera.position) == 0.0, "walkers fade inside a vehicle's box")
	# Pigeons at the anchor; a car flushes them; they land again.
	field.set_obstacles(PackedVector3Array(), PackedVector3Array())
	run_steps(field, 10)
	check(field.pigeon_count() >= 3 and field.pigeon_count() <= 7, "a flock sits at the anchor: %d" % field.pigeon_count())
	for i in 4:
		field.set_obstacles(PackedVector3Array([Vector3(6.2, 0.03, 3.0)]), PackedVector3Array([Vector3.ZERO]))
		field.tick_step(0.05)
	var flying: int = field.p_flying.count(1)
	check(int(field.stats.flushed) > 0 and flying == field.pigeon_count(), "a vehicle flushes the flock: %d of %d" % [flying, field.pigeon_count()])
	var highest := 0.0
	field.set_obstacles(PackedVector3Array(), PackedVector3Array())
	for i in 300:
		field.tick_step(0.05)
		for p: Vector3 in field.p_position: highest = maxf(highest, p.y)
	check(highest >= 1.4 and field.pigeon_count() > 0 and field.p_flying.count(1) == 0 and field.p_position == field.p_home, "pigeons arc up and land back: %.2f m" % highest)
	field.wet = true
	field._pigeons_step(0.05, camera.position, FIELD.live(on(2)))
	check(field.pigeon_count() == 0, "no pigeons in the rain")
	# Ambience.
	var amb: Dictionary = field.ambience_at(camera.position)
	check(is_equal_approx(amb.asphalt, 0.5) and is_equal_approx(amb.built, 0.3) and int(amb.walkers_near) >= 0, "ambience mixes the cells: %s" % amb)
	check(cache.ambience_at(Vector3(500, 0, 500)).asphalt == 0.0, "no cell within reach mixes nothing")
	# A 1,024 m cell whose lanes and anchor lie near a corner, 680 m from its
	# centre: a camera there counts its lanes, sees its pigeons and its ambience.
	var big := record()
	var shift := Vector3(480, 0, 480)
	for key: String in ["points", "crossings", "anchors"]:
		var list: PackedVector3Array = big[key]
		for i in list.size(): list[i] += shift
		big[key] = list
	big.centre = Vector2.ZERO
	big.rect = Rect2(-512, -512, 1024, 1024)
	big.ambience = Vector4(0.0, 0.0, 0.0, 1.0)
	var big_cell: RefCounted = FIELD.register(big, parent, cache)
	var corner := Vector3(480, 1.5, 476)
	field._count_population(corner, 28.0, 80)
	var flock_before: int = field.pigeon_count()
	field.wet = false
	field.daylight = 1.0
	field._pigeons_step(0.05, corner, FIELD.live(on(2)))
	check(field.population > 0 and field.pigeon_count() > flock_before and is_equal_approx(field.ambience_at(corner).water, 1.0), "a large cell culls by its rectangle: %d walkers, %d pigeons, %s" % [field.population, field.pigeon_count(), field.ambience_at(corner)])
	big_cell.release()
	check(int(cache.entries[FIELD.KEY].pins) == 1, "the large cell's pin returns")
	# Fixed memory: three MultiMeshes (CPU and GPU, one retiring set), meshes and state.
	var measured := 0
	for part: Dictionary in field.parts.values(): measured += 2 * 2 * (part.multimesh as MultiMesh).instance_count * 64
	for mesh: ArrayMesh in MESHES.shared().values(): measured += 2 * (mesh.surface_get_array_len(0) * 32 + mesh.surface_get_array_index_len(0) * 4)
	# Walker state of every slot, measured; pigeon state likewise per slot
	# with its flock record, spawn candidates of 2,048 lanes and the obstacles.
	field._clear_walkers()
	var before := OS.get_static_memory_usage()
	for i in FIELD.MAX_WALKERS: walker_on(field, cell, 1, 5.0)
	var walker_bytes := OS.get_static_memory_usage() - before
	field._clear_walkers()
	measured += walker_bytes + FIELD.MAX_PIGEONS * 128 + (FIELD.MAX_PIGEONS / 3) * 1024 + 2048 * 12 + 8 * 24 * 2
	print("walker state %d B for %d slots" % [walker_bytes, FIELD.MAX_WALKERS])
	check(walker_bytes <= FIELD.MAX_WALKERS * 256, "walker state stays small: %d B" % walker_bytes)
	print("street life field: %d of %d B, peak tick %d usec" % [measured, FIELD.MEMORY_BYTES, field.stats.peak_tick_usec])
	check(measured <= FIELD.MEMORY_BYTES, "the field stays inside its fixed claim: %d B" % measured)
	# Release and close: the MultiMeshes and the material are freed with the
	# parts (the RenderingServer offers no query of a freed instance RID).
	var rendered := resources_of(field)
	cell.release()
	check(int(cache.entries[FIELD.KEY].pins) == 0 and field.cells.is_empty() and field.walker_count() == 0 and field.node_pos.is_empty(), "release unpins and drops walkers and nodes")
	cache.shutdown()
	check(field.closed and field.parts.is_empty() and rendered.size() == 4 and rendered.all(func(ref: WeakRef) -> bool: return ref.get_ref() == null), "close frees the render resources")
	parent.queue_free()
	await process_frame

## Rendered: a near and a far walker and a pigeon change the pixels in front
## of the camera (the shader compiles and draws on this renderer).
func pixels(camera: Camera3D) -> void:
	var cache := CACHE.new(func(amount): return Lease.new(amount))
	cache.quality_profile = QUALITY.profile(2)
	var parent := Node3D.new()
	root.add_child(parent)
	var cell: RefCounted = FIELD.register(record(), parent, cache)
	var field: RefCounted = cache.entries[FIELD.KEY].street_life_field
	field.manual = true
	QUALITY.current = on(2)
	var light := DirectionalLight3D.new()
	root.add_child(light)
	light.rotation_degrees = Vector3(-50, 30, 0)
	camera.position = Vector3(-10, 0.45, 2.6)
	camera.look_at(Vector3(-10, 0.3, 0))
	field.scenario = camera.get_world_3d().scenario
	field._build_parts()
	var snap := func() -> Image:
		field._draw(camera.global_position, 8.0)
		for i in 4: await process_frame
		return root.get_texture().get_image()
	var empty: Image = await snap.call()
	for case: Array in [[20.0, "near"], [10.0, "far"]]:
		field._clear_walkers()
		if case[1] == "far":
			camera.position = Vector3(-20, 0.6, 9.5)
			camera.look_at(Vector3(-20, 0.3, 0))
			empty = await snap.call()
		var w: int = field.add_walker(cell, 0, case[0], true, 0.4, 0.5, FIELD.HEIGHT, 2, 1)
		field.w_age[w] = 1.0
		var drawn: Image = await snap.call()
		var changed := 0
		for y in range(0, drawn.get_height(), 2):
			for x in range(0, drawn.get_width(), 2):
				if drawn.get_pixel(x, y).r != empty.get_pixel(x, y).r or drawn.get_pixel(x, y).b != empty.get_pixel(x, y).b: changed += 1
		print("%s walker: %d changed pixels, %s" % [case[1], changed, field.draw_cost()])
		check(changed > 8 and int(field.draw_cost()[case[1]]) == 1, "a %s walker draws: %d pixels" % [case[1], changed])
		# Optional look at the figures (a directory to write PNGs into).
		var keep := OS.get_environment("STREET_LIFE_SNAPSHOTS")
		if not keep.is_empty(): drawn.save_png(keep.path_join("street_life_%s.png" % case[1]))
	field._clear_walkers()
	cell.release()
	cache.shutdown()
	light.queue_free()
	parent.queue_free()
	await process_frame
