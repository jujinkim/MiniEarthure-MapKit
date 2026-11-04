extends SceneTree
## Camera-local grass field (grass_scatter.gd, grass_field.gd,
## vegetation_grass.gdshader, chunk_renderer): bounded indexing steps that
## split batches identically, exact plane heights and coverage on grass
## ground only (never over a road strip, steep bank or sliver), rings per
## tier (none on low or polar ground) with an even blade cover across their
## handovers, live rings never reaching past the field tier's window, fixed
## memory inside memory_bytes, bounded per-frame block work (resumable
## blocks, window moves and a teleport), cell reservations equal to the
## estimate (less unused grass on low and polar ground), the lawn shade set
## only while blades draw, release of every cell, pin and lease, and rendered
## pixels: no blade over a road strip, blades over grass, and blades standing
## on sloped ground.
const RENDER := preload("../chunk_renderer.gd")
const GRASS := preload("../grass_scatter.gd")
const FIELD := preload("../grass_field.gd")
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

func record_reservation(bytes: int, box: Array, lease: Lease) -> Lease:
	box[0] = bytes
	return lease

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	indexing()
	heights()
	tiers()
	await renderer()
	if RenderingServer.get_current_rendering_method() != "dummy" and DisplayServer.get_name() != "headless": await pixels()
	else: print("grass_field_validator: pixel checks need --rendered")
	QUALITY.current = {}
	print("grass_field_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## Square of 2 m triangles from `origin` (x right, z toward -z as map y),
## heights from `height`, skipping columns in `holes` (x ranges, metres).
func ground(origin: Vector3, columns: int, rows: int, height: Callable, holes: Array = []) -> PackedVector3Array:
	var points := PackedVector3Array()
	for z in rows:
		for x in columns:
			var a := origin + Vector3(x * 2.0, 0.0, -z * 2.0)
			var skip := false
			for hole: Vector2 in holes:
				if a.x + 1.0 > hole.x and a.x + 1.0 < hole.y: skip = true
			if skip: continue
			for p: Vector3 in [a, a + Vector3(2, 0, 0), a + Vector3(0, 0, -2), a + Vector3(2, 0, 0), a + Vector3(2, 0, -2), a + Vector3(0, 0, -2)]:
				points.append(Vector3(p.x, height.call(p.x, p.z), p.z))
	return points

func flat(_x: float, _z: float) -> float: return 0.0
func slope(x: float, z: float) -> float: return 5.0 + 0.3 * x - 0.1 * z
func ridge(x: float, _z: float) -> float: return 2.0 + 0.2 * absf(x - 34.0)

func index(parts: Array) -> Dictionary:
	var state := GRASS.begin(parts)
	var steps := 0
	while not GRASS.advance(state): steps += 1
	return {"record": GRASS.record(state), "steps": steps, "peak": int(state.peak_usec)}

## Block heights of a record as the field builds them.
func blocks_of(record: Dictionary) -> Dictionary:
	var result := {}
	for z in record.size.y:
		for x in record.size.x:
			var block := Vector2i(record.origin.x + x, record.origin.y + z)
			var data := PackedFloat32Array()
			data.resize(FIELD.BLOCK_TEXELS * FIELD.BLOCK_TEXELS)
			data.fill(FIELD.EMPTY)
			var i: int = z * record.size.x + x
			for k in range(record.offsets[i], record.offsets[i + 1]):
				var ref: int = record.refs[k]
				var part: PackedVector3Array = record.parts[ref >> GRASS.PART_SHIFT]
				var t: int = (ref & 0xffff) * 3
				FIELD.fill(data, block, part[t], part[t + 1], part[t + 2])
			result[block] = data
	return result

func indexing() -> void:
	# 64 m square (2,048 triangles), a steep bank and a sliver.
	var points := ground(Vector3.ZERO, 32, 32, flat)
	points.append_array([Vector3(0, 0, 4), Vector3(4, 0, 4), Vector3(0, 6, 4)])
	points.append_array([Vector3(10, 0, 6), Vector3(30, 0, 6), Vector3(50, 0, 6.00001)])
	var whole := index([points])
	print("grass index: %d steps, peak step %d usec" % [whole.steps, whole.peak])
	check(whole.steps >= 3 and whole.peak < 8000, "indexing spans bounded steps: %d, %d usec" % [whole.steps, whole.peak])
	var record: Dictionary = whole.record
	check(record.origin == Vector2i(0, -8) and record.size == Vector2i(8, 8), "the record spans the grass blocks only (bank and sliver excluded): %s %s" % [record.origin, record.size])
	var triangles := points.size() / 3
	var held := GRASS.record_bytes(record) + triangles * 36
	check(held <= PLAN.grass_bytes(triangles), "a record stays inside the cell's grass reservation: %d of %d B" % [held, PLAN.grass_bytes(triangles)])
	var split := index([points.slice(0, 3000), points.slice(3000)])
	check(blocks_of(split.record) == blocks_of(record), "batch boundaries do not change the grass field")
	check(index([]).record.is_empty() and index([PackedVector3Array([Vector3.ZERO, Vector3(1, 5, 0), Vector3(0, 6, 0)])]).record.is_empty(), "no grass ground, no record")

func heights() -> void:
	# Sloped plane with a 4 m road strip (x 30..34): exact heights on grass,
	# no grass share past the strip's edge.
	var record: Dictionary = index([ground(Vector3(0, 0, 0), 32, 32, slope, [Vector2(30, 34)])]).record
	var data := blocks_of(record)
	var worst := 0.0
	var leaks := 0
	var holes := 0
	for i in 4000:
		var x := fmod(i * 0.6180339 * 64.0, 64.0)
		var z := -fmod(i * 0.4142136 * 64.0, 64.0)
		var at: Vector2 = FIELD.sample(data, x, z)
		var inside := x > 0.3 and x < 63.7 and z < -0.3 and z > -63.7 and (x < 29.7 or x > 34.3)
		var road := x > 30.13 and x < 33.87
		if inside:
			if at.y < 0.999: holes += 1
			else: worst = maxf(worst, absf(at.x - slope(x, z)))
		if road and at.y >= 0.5: leaks += 1
	check(holes == 0 and worst < 0.001, "grass ground is covered at its exact plane height: %d holes, %.5f m" % [holes, worst])
	check(leaks == 0, "no grass share reaches into the road strip: %d" % leaks)
	# A ridge crease (DEM terrain): bilinear heights stay within 3 cm.
	var crease := blocks_of(index([ground(Vector3.ZERO, 32, 4, ridge)]).record)
	var error := 0.0
	for i in 400:
		var x := 0.5 + i * 0.1575
		var at: Vector2 = FIELD.sample(crease, x, -3.3)
		error = maxf(error, absf(at.x - ridge(x, 0.0)))
	check(error < 0.03, "blades follow creased terrain within 3 cm: %.4f m" % error)
	# Block cost: one full grass block.
	var started := Time.get_ticks_usec()
	var block := PackedFloat32Array()
	block.resize(1024)
	var parts: Array = record.parts
	for k in range(record.offsets[0], record.offsets[1]):
		var ref: int = record.refs[k]
		var part: PackedVector3Array = parts[ref >> GRASS.PART_SHIFT]
		FIELD.fill(block, record.origin, part[(ref & 0xffff) * 3], part[(ref & 0xffff) * 3 + 1], part[(ref & 0xffff) * 3 + 2])
	print("grass block raster: %d usec" % (Time.get_ticks_usec() - started))

func tiers() -> void:
	check(FIELD.memory_bytes(0) == 0 and QUALITY.grass_level(QUALITY.profile(0)) == 0, "no field on low")
	var pressured := QUALITY.profile(0)
	pressured.texture_size = QUALITY.profile(2).texture_size
	check(QUALITY.grass_level(pressured) == 2 and QUALITY.grass_level(QUALITY.profile(1)) == 1, "the field builds at the cache texture tier")
	for tier in [1, 2]:
		var distance := float(QUALITY.profile(tier).grass_distance)
		var window: float = (FIELD.BLOCKS[tier] / 2 - 0.5) * FIELD.BLOCK_METRES
		check(window >= distance + 4.0, "tier %d window reaches %.1f m past grass_distance %.0f m" % [tier, window, distance])
		var rings := FIELD.ring_set(tier, distance)
		check(is_equal_approx((rings[-1].fade as Vector4).w, distance), "the last ring ends at grass_distance")
		var total := 0
		var vertices := 0
		var report := []
		for ring: Dictionary in rings:
			var tiles := FIELD.ring_tiles(ring)
			var mesh := FIELD.tile_mesh(ring)
			total += tiles.size()
			vertices += tiles.size() * mesh.surface_get_array_len(0)
			report.append("%.0f m: %d tiles x %d cards (thin from %.1f m)" % [ring.tile, tiles.size(), ring.cards, ring.thin])
			# Every point within the ring's reach is inside a listed tile for any
			# camera offset within half a tile of the snapped origin.
			var size: float = ring.tile
			var listed := {}
			for tile: Vector2i in tiles: listed[tile] = true
			var missing := 0
			for i in 600:
				var offset := Vector2(fmod(i * 0.618, 1.0) - 0.5, fmod(i * 0.414, 1.0) - 0.5) * size
				var angle := i * 2.39996
				var reach := fmod(i * 0.7548, 1.0) * (ring.fade as Vector4).w
				var point := offset + Vector2(cos(angle), sin(angle)) * reach
				if (ring.fade as Vector4).x >= 0.0 and reach < (ring.fade as Vector4).x: continue
				if not listed.has(Vector2i(floori(point.x / size), floori(point.y / size))): missing += 1
			check(missing == 0, "ring %.0f m covers its reach: %d missing" % [size, missing])
		print("tier %d rings: %s; %d instances, %d vertices if every quadrant draws" % [tier, ", ".join(report), total, vertices])
		check(vertices <= [0, 170000, 450000][tier], "tier %d blade vertices bounded: %d" % [tier, vertices])
		# Projected blade cover (blade area per ground area seen from the
		# design eye height) stays within 1.6x from 3 m to 0.75 x the reach.
		var low := INF
		var high := 0.0
		var profile := []
		for step in 60:
			var d := 3.0 + (distance * 0.75 - 3.0) * step / 59.0
			var c := cover(rings, tier, d)
			low = minf(low, c)
			high = maxf(high, c)
			if step % 6 == 0: profile.append("%.1f m %.2f" % [d, c])
		print("tier %d cover: %s" % [tier, ", ".join(profile)])
		check(low > 0.0 and high / low <= 1.6 and low >= FIELD.COVER[tier] * 0.75, "tier %d blade cover stays even across ring handovers: %.2f..%.2f" % [tier, low, high])
	# A medium field under high quality (raised before the next map load) keeps
	# the medium rings and reach: its window ends at 28 m.
	var medium: RefCounted = FIELD.new(1, null, null)
	var raised: Vector2 = medium.live_rings(QUALITY.profile(2))
	check(raised == Vector2(1, QUALITY.profile(1).grass_distance), "a medium field keeps medium rings under high quality: %s" % raised)
	check(FIELD.ring_set(1, raised.y)[-1].fade.w <= (FIELD.BLOCKS[1] / 2 - 0.5) * FIELD.BLOCK_METRES - 4.0, "the live reach stays inside the window")
	check(medium.live_rings(QUALITY.profile(0)) == Vector2.ZERO, "low has no live rings")
	medium.close()

## The lawn material of a cell's grass ground meshes.
func lawn_of(cell: Node3D) -> ShaderMaterial:
	for mesh: MeshInstance3D in cell.find_children("*", "MeshInstance3D", true, false):
		var material := mesh.material_override as ShaderMaterial
		if material != null and material.get_shader_parameter("lawn") == true: return material
	return null

## Projected cover at camera distance `d`: every ring's fade weight times its
## thinning and full-density card cover, over the design eye height.
func cover(rings: Array, tier: int, d: float) -> float:
	var total := 0.0
	for ring: Dictionary in rings:
		var fade: Vector4 = ring.fade
		var weight := (1.0 if fade.x < 0.0 else smoothstep(fade.x, fade.y, d)) * (1.0 - smoothstep(fade.z, fade.w, d)) * minf(1.0, float(ring.thin) / d)
		total += weight * FIELD.card_cover(ring) * d / FIELD.EYE_HEIGHT
	return total

func fixture(polar := false) -> Dictionary:
	var points := ground(Vector3.ZERO, 32, 32, flat)
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": [], "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for i in points.size() / 3:
		chunk.triangles.append({"surface": "grass", "object_id": "ground"})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {}
	if polar: chunk.presentation.environment_json = JSON.stringify({"climate": "polar"})
	chunk.render_batches = PLAN.prepare_batches(chunk)
	return chunk

## Measured bytes of a field: height layers, MultiMesh buffers (CPU and GPU),
## tile meshes (position, UV, index; CPU and GPU).
func measured(field: RefCounted) -> int:
	var total: int = field.blocks * field.blocks * 4096
	for ring: Dictionary in field.rings:
		var mesh: ArrayMesh = ring.mesh
		total += 2 * (mesh.surface_get_array_len(0) * 20 + mesh.surface_get_array_index_len(0) * 4)
		for part: Dictionary in ring.parts: total += 2 * (part.multimesh as MultiMesh).instance_count * 48
	return total

func renderer() -> void:
	var camera := Camera3D.new()
	root.add_child(camera)
	camera.position = Vector3(32, 2, -32)
	camera.current = true
	for polar in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var cache := CACHE.new(func(bytes): return Lease.new(bytes))
			cache.quality_profile = QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(polar)
			var planned := PLAN.estimate(chunk, 0)
			check(planned - PLAN.estimate({"triangles": [], "objects": [], "presentation": {}}, 0) >= PLAN.grass_bytes(2048), "the estimate reserves the cell's grass ground")
			check(PLAN.upper_bound({"triangles": chunk.triangles.size(), "objects": 0}) >= planned, "within the upper bound")
			var reserved := [0]
			var lease := Lease.new(planned)
			var job := RENDER.begin(chunk, parent, record_reservation.bind(reserved, lease), planned, cache)
			# A cell growing no grass (low, polar) returns its grass reservation.
			var unused := PLAN.grass_bytes(PLAN.batch_grass_triangles(chunk)) if level == 0 or polar else 0
			check(unused == 0 or unused == PLAN.grass_bytes(2048), "prepared batches count the estimate's grass triangles")
			check(reserved[0] == planned - unused, "tier %d polar %s reserves the estimate less unused grass: %d of %d" % [level, polar, reserved[0], planned])
			var steps := 0
			while not RENDER.advance(job): steps += 1
			check(job.error.is_empty(), "attachment completes: %s" % job.error)
			var expect: bool = level > 0 and not polar
			check(job.has("grass_cell") == expect, "tier %d polar %s registers grass: %s" % [level, polar, job.has("grass_cell")])
			var lawn := lawn_of(job.root)
			check(lawn != null, "the cell has a lawn material")
			if expect:
				var field: RefCounted = cache.entries[FIELD.KEY].grass_field
				for _frame in 600:
					await process_frame
					if field.window.x < (1 << 29) and field.dirty.is_empty(): break
				var cost: Dictionary = field.draw_cost()
				var bytes := measured(field)
				print("tier %d field: %s, %d of %d B, %s" % [level, cost, bytes, FIELD.memory_bytes(level), field.stats])
				check(bytes <= FIELD.memory_bytes(level) and int(cache.entries[FIELD.KEY].bytes) == FIELD.memory_bytes(level), "tier %d field stays inside its fixed claim: %d B" % [level, bytes])
				check(cost.draws <= 12 and cost.visible_draws > 0, "tier %d draws at most 12 instances, some over grass: %s" % [level, cost])
				check(int(field.stats.built) >= field.blocks * field.blocks and field.dirty.is_empty(), "the whole window builds in bounded frames: %s" % field.stats)
				check(int(field.stats.peak_steady_tick_usec) < 3 * FIELD.STEP_USEC, "a steady tick stays near the frame step: %d usec" % field.stats.peak_steady_tick_usec)
				check(is_equal_approx(float(lawn.get_shader_parameter("grass_field_distance")), field.live_distance) and is_equal_approx(float(lawn.get_shader_parameter("grass_field_shade")), FIELD.SHADE[level]), "the field shades the lawn under its blades")
				# Driving across block borders: bounded ticks, blocks resumed.
				var start := camera.position
				for step in 48:
					camera.position = start + Vector3(step * 0.5, 0.0, -step * 0.25)
					await process_frame
				for _frame in 120:
					await process_frame
					if field.dirty.is_empty() and field.building < 0: break
				print("tier %d driving: %s" % [level, field.stats])
				check(int(field.stats.peak_steady_tick_usec) < 3 * FIELD.STEP_USEC, "driving ticks stay near the frame step: %d usec" % field.stats.peak_steady_tick_usec)
				# A teleport clears every grass slot at once, then rebuilds.
				camera.position = start + Vector3(500.0, 0.0, 0.0)
				await process_frame
				check(field.slot_grass.count(1) == 0, "a teleport leaves no other block's grass")
				print("tier %d teleport: %d clears, peak tick %d usec" % [level, field.stats.peak_clears, field.stats.peak_tick_usec])
				camera.position = start
				for _frame in 600:
					await process_frame
					if field.dirty.is_empty() and field.building < 0: break
				check(field.slot_grass.count(1) > 0, "the grass rebuilds after a teleport back")
				# A medium field under high quality keeps its medium reach.
				if level == 1:
					QUALITY.current = QUALITY.profile(2)
					await process_frame
					check(field.live == 1 and is_equal_approx(field.live_distance, QUALITY.profile(1).grass_distance) and field.draw_cost().vertices <= 170000, "a medium field under high quality keeps medium rings: %d, %.0f m" % [field.live, field.live_distance])
					QUALITY.current = QUALITY.profile(1)
					await process_frame
				# Adaptive quality lowers the live rings without a new claim.
				if level == 2:
					QUALITY.current = QUALITY.profile(1)
					await process_frame
					check(field.live == 1 and field.draw_cost().instances < cost.instances, "a GPU-pressure dip lowers the live rings")
					QUALITY.current = QUALITY.profile(0)
					await process_frame
					check(field.draw_cost().visible_draws == 0, "low hides the field")
					check(float(lawn.get_shader_parameter("grass_field_distance")) == 0.0, "no lawn shade without blades")
					QUALITY.current = QUALITY.profile(2)
					await process_frame
				# A hidden cell drops its grass.
				parent.hide()
				for _frame in 40: await process_frame
				check(field.slot_grass.count(1) == 0 and field.draw_cost().visible_draws == 0, "a hidden cell's grass leaves the field")
				parent.show()
				RENDER.cancel(job)
				check(field.cells.is_empty() and int(cache.entries[FIELD.KEY].pins) == 0, "cancel releases the cell and its pin")
				check(float(lawn.get_shader_parameter("grass_field_distance")) == 0.0, "a released field leaves no lawn shade")
				await process_frame
			else:
				for _frame in 3: await process_frame
				var shade: Variant = lawn.get_shader_parameter("grass_field_distance") if lawn != null else null
				check(shade == null or float(shade) == 0.0, "tier %d polar %s: no lawn shade without a field" % [level, polar])
				RENDER.cancel(job)
			lawn = null
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), "the cell lease retires")
			cache.shutdown()
			for _frame in 4: await process_frame
			check(cache.bytes() == 0, "the field retires with its cache")
	# A refused field skips grass; the cell still attaches.
	QUALITY.current = QUALITY.profile(2)
	var denied := CACHE.new(func(bytes): return null if bytes == FIELD.memory_bytes(2) else Lease.new(bytes))
	denied.quality_profile = QUALITY.profile(2)
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture()
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), PLAN.estimate(chunk, 0), denied)
	while not RENDER.advance(job): pass
	check(job.error.is_empty() and not job.has("grass_cell"), "a refused field skips grass only: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	camera.queue_free()
	await process_frame

## Renders a SubViewport after the field has built: returns its image.
func shot(viewport: SubViewport, frames := 90) -> Image:
	for _frame in frames: await process_frame
	await RenderingServer.frame_post_draw
	return viewport.get_texture().get_image()

func pixels() -> void:
	var quality := QUALITY.profile(2)
	quality.vegetation_motion = 0.0
	QUALITY.current = quality
	var viewport := SubViewport.new()
	viewport.size = Vector2i(320, 320)
	viewport.own_world_3d = true
	viewport.transparent_bg = false
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(viewport)
	var environment := WorldEnvironment.new()
	environment.environment = Environment.new()
	environment.environment.background_mode = Environment.BG_COLOR
	environment.environment.background_color = Color.BLACK
	environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color = Color.WHITE
	environment.environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	viewport.add_child(environment)
	var scene := Node3D.new()
	viewport.add_child(scene)
	# Top view: grass 0..6 m with a road strip at x 2.5..3.5 (no grass ground).
	var kept := PackedVector3Array()
	for z in 14:
		for x in 14:
			var a := Vector3(-0.5 + x * 0.5, 0.0, 0.5 - z * 0.5)
			if a.x + 0.25 > 2.5 and a.x + 0.25 < 3.5: continue
			kept.append_array([a, a + Vector3(0.5, 0, 0), a + Vector3(0, 0, -0.5), a + Vector3(0.5, 0, 0), a + Vector3(0.5, 0, -0.5), a + Vector3(0, 0, -0.5)])
	scene.add_child(flat_mesh(kept, Color(0, 0, 1)))
	scene.add_child(flat_mesh(PackedVector3Array([Vector3(2.5, -0.001, 1), Vector3(3.5, -0.001, 1), Vector3(2.5, -0.001, -7), Vector3(3.5, -0.001, 1), Vector3(3.5, -0.001, -7), Vector3(2.5, -0.001, -7)]), Color(1, 0, 0)))
	var record: Dictionary = index([kept]).record
	var cell: RefCounted = FIELD.register(record, scene, null, Color(0.12, 0.25, 0.08))
	check(cell != null, "a standalone field registers")
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 6.0
	scene.add_child(camera)
	camera.position = Vector3(3.0, 2.5, -3.0)
	camera.rotation_degrees = Vector3(-90, 0, 0)
	camera.current = true
	var top := await shot(viewport)
	var road_blades := 0
	var road_pixels := 0
	var grass_blades := 0
	var grass_pixels := 0
	for y in top.get_height():
		for x in top.get_width():
			var world_x := 3.0 + (x + 0.5 - top.get_width() * 0.5) * 6.0 / top.get_width()
			var c := top.get_pixel(x, y)
			var blade := not (c.r > 0.9 and c.g < 0.1 and c.b < 0.1) and not (c.b > 0.9 and c.r < 0.1 and c.g < 0.1)
			if world_x > 2.8 and world_x < 3.2:
				road_pixels += 1
				road_blades += int(blade)
			elif world_x < 2.2 or world_x > 3.8:
				grass_pixels += 1
				grass_blades += int(blade)
	print("top view: %d/%d blade pixels over the road strip, %d/%d over grass" % [road_blades, road_pixels, grass_blades, grass_pixels])
	check(road_pixels > 1000 and road_blades == 0, "no blade stands on the road strip: %d" % road_blades)
	# Seen from straight above a blade is under a pixel wide: about 1.5 %.
	check(grass_blades > grass_pixels * 0.01, "blades stand all over the grass: %d of %d" % [grass_blades, grass_pixels])
	top.save_png(OS.get_environment("GRASS_FIELD_CAPTURE_DIR").path_join("grass_top.png") if OS.has_environment("GRASS_FIELD_CAPTURE_DIR") else "user://grass_top.png")
	cell.release()
	scene.queue_free()
	await process_frame
	# Side view of a 19 degree slope: every blade pixel lies on the ground line
	# (from 8 cm below, a leaning blade's root, to 36 cm above it, a flower)
	# and most columns carry blades.
	scene = Node3D.new()
	viewport.add_child(scene)
	var sloped := PackedVector3Array()
	for z in 4:
		for x in 12:
			var a := Vector3(x * 0.5, 0.0, -0.5 - z * 0.5)
			for p: Vector3 in [a, a + Vector3(0.5, 0, 0), a + Vector3(0, 0, -0.5), a + Vector3(0.5, 0, 0), a + Vector3(0.5, 0, -0.5), a + Vector3(0, 0, -0.5)]:
				sloped.append(Vector3(p.x, 0.35 * p.x, p.z))
	cell = FIELD.register(index([sloped]).record, scene, null, Color(0.12, 0.25, 0.08))
	camera = Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 6.4
	scene.add_child(camera)
	camera.position = Vector3(3.0, 1.0, 0.5)
	camera.current = true
	var side := await shot(viewport)
	var off := 0
	var columns := 0
	var filled := 0
	for x in side.get_width():
		var world_x := 3.0 + (x + 0.5 - side.get_width() * 0.5) * 6.4 / side.get_width()
		if world_x < 0.3 or world_x > 5.7: continue
		columns += 1
		var any := false
		for y in side.get_height():
			var c := side.get_pixel(x, y)
			if c.r + c.g + c.b < 0.02: continue
			var world_y := 1.0 - (y + 0.5 - side.get_height() * 0.5) * 6.4 / side.get_height()
			any = true
			if world_y < 0.35 * world_x - 0.08 or world_y > 0.35 * world_x + 0.36: off += 1
		filled += int(any)
	print("side view: %d of %d columns carry blades, %d pixels off the ground band" % [filled, columns, off])
	check(off == 0, "blades stand on the sloped ground: %d pixels off" % off)
	check(filled > columns * 0.8, "blades along the whole slope: %d of %d" % [filled, columns])
	side.save_png(OS.get_environment("GRASS_FIELD_CAPTURE_DIR").path_join("grass_side.png") if OS.has_environment("GRASS_FIELD_CAPTURE_DIR") else "user://grass_side.png")
	cell.release()
	viewport.queue_free()
	for _frame in 3: await process_frame

func flat_mesh(points: PackedVector3Array, colour: Color) -> MeshInstance3D:
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = points
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = colour
	material.cull_mode = BaseMaterial3D.CULL_DISABLED
	var node := MeshInstance3D.new()
	node.mesh = mesh
	node.material_override = material
	return node
