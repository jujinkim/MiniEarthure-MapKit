extends SceneTree
## Street trees and flower beds (street_planting_meshes.gd, chunk_renderer):
## a deterministic shared set built one part per step inside its allowance;
## for every species and variant the crown reach (with the largest sway)
## inside CROWN_CM (mapkit-core STREET_TREES), the canopy floor above 1.20 m beyond 30 cm of
## the axis at every instance scale, the trunk (or wrap) enclosing the trunk
## box up to car height and the pit fixtures inside the 36 cm pit, raised
## above the sidewalk; rims enclosing their boxes, plantings inside box plus
## 3 cm and under their height caps; variant choice by identity and block;
## every id in BUILTINS and DRAWN_OVER with hidden proxy faces; groups per
## mesh and 32 m square with ranges and shadows per tier, a retiring lease, a
## refused set failing the cell before any upload and release of every
## reservation. Rendered runs write a capture of every species and bed piece
## (MAPKIT_STREET_PLANTING_CAPTURE, a PNG path).
const RENDER := preload("../chunk_renderer.gd")
const STREET := preload("../street_planting_meshes.gd")
const VEG := preload("../vegetation_meshes.gd")
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

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	var set := meshes()
	trees(set)
	beds(set)
	choices()
	await renderer()
	await capture(set)
	QUALITY.current = {}
	print("street_planting_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

static func arrays(mesh: ArrayMesh) -> Array:
	return mesh.surface_get_arrays(0) if mesh.get_surface_count() > 0 else []

## Builds the set twice (one part per call) and measures its memory.
func meshes() -> Dictionary:
	var first := {}
	var steps := 0
	var slowest := ""
	var peak := 0
	while true:
		var started := Time.get_ticks_usec()
		var name: String = STREET.STEPS[int(first.get("step", 0))]
		var done := STREET.build(first)
		var elapsed := Time.get_ticks_usec() - started
		if elapsed > peak: peak = elapsed; slowest = name
		if done: break
		steps += 1
	var second := {}
	while not STREET.build(second): pass
	check(steps + 1 == STREET.STEPS.size() and steps > 60, "the shared set builds one part per step: %d" % steps)
	check(STREET._crowns.is_empty() and STREET._shapes.is_empty(), "no crown shapes outlive the build")
	var vertex_count := 0
	var index_count := 0
	var seen := {}
	for name: String in first.result:
		if name == "materials": continue
		var a: ArrayMesh = first.result[name]
		var b: ArrayMesh = second.result[name]
		check(a.get_surface_count() == b.get_surface_count() and (a.get_surface_count() == 0 or arrays(a) == arrays(b)), "%s builds identically" % name)
		if seen.has(a) or a.get_surface_count() == 0: continue
		seen[a] = true
		vertex_count += a.surface_get_array_len(0)
		index_count += a.surface_get_array_index_len(0)
	# CPU arrays and GPU buffers (56 B per vertex: position, normal, colour,
	# two UVs; 4 B per index), the leaf atlas as decoded image and mipmapped
	# texture (2 x 350 KiB), 128 KiB for two materials and build arrays.
	var mesh_bytes := 2 * (vertex_count * 56 + index_count * 4)
	print("street planting set: %d meshes, %d vertices, %d indices, %d B arrays of %d B; slowest step %s %d usec" % [seen.size(), vertex_count, index_count, mesh_bytes, STREET.SHARED_BYTES, slowest, peak])
	check(mesh_bytes + 2 * 350 * 1024 + 131072 <= STREET.SHARED_BYTES, "the shared set fits its allowance: %d B" % mesh_bytes)
	check(first.result.materials.body.get_meta("mapkit_vegetation_shader", "") == "body" and first.result.materials.cards.get_meta("mapkit_vegetation_shader", "") == "cards", "materials opt into vegetation quality (wind, low-tier regrowth)")
	for level in 3:
		QUALITY.apply_material(first.result.materials.body, QUALITY.profile(level))
		check(first.result.materials.body.get_shader_parameter("cards_hidden") == (level == 0), "low grows the shell back in place of cards")
	return first.result

## Crown reach, canopy floor, trunk enclosure and pit fixtures of every
## species and variant at its scale span and largest sway.
func trees(set: Dictionary) -> void:
	var sizes := []
	for species: String in STREET.SPECIES:
		for variant in ([0, 1, 2] if species == "pine" else [0, 1]):
			var label := "%s %d" % [species, variant]
			var crown_m := float(STREET.CROWN_CM[species]) / 100.0
			var largest := STREET.largest_scale(variant)
			var smallest := STREET.smallest_scale(variant)
			var reach := 0.0
			var low := INF
			var points := 0
			for part in ["body", "cards", "far"]:
				var data := arrays(set["%s_%s_%d" % [species, part, variant]])
				check(not data.is_empty(), label + " has a %s mesh" % part)
				if data.is_empty(): continue
				var vertices: PackedVector3Array = data[Mesh.ARRAY_VERTEX]
				var marks: PackedVector2Array = data[Mesh.ARRAY_TEX_UV]
				var weights: PackedVector2Array = data[Mesh.ARRAY_TEX_UV2]
				for i in vertices.size():
					var p := vertices[i]
					var sway := weights[i].x * (VEG.SWAY_PEAK * VEG.TREE_SWAY + (VEG.FLUTTER if part == "cards" else 0.0))
					var candidates := [[p, sway]]
					if part == "body" and marks[i].y > 0.0:
						candidates.append([Vector3(0.0, marks[i].x, 0.0) + (p - Vector3(0.0, marks[i].x, 0.0)) * marks[i].y, 0.0])
					for candidate: Array in candidates:
						var q: Vector3 = candidate[0]
						var r := Vector2(q.x, q.z).length()
						reach = maxf(reach, r * largest + float(candidate[1]))
						# Beyond 30 cm of the axis (with sway) at any scale of the
						# span the vertex stands at least 1.20 m up.
						for scale in [smallest, largest]:
							if r * scale + float(candidate[1]) > STREET.CANOPY_AXIS: low = minf(low, q.y * scale)
						points += 1
			check(reach <= crown_m + 0.001 and reach >= crown_m * (0.75 if variant != 1 else 0.4), label + " crown reaches %.3f m of %.2f m with sway" % [reach, crown_m])
			check(low >= STREET.CANOPY_MIN - 0.0005, label + " canopy floor beyond 30 cm: %.3f m" % low)
			# Trunk (young: the wrap) encloses the 14 cm box (half diagonal
			# 0.099 m) up to 0.60 m at the smallest scale: rings up to the first
			# above 0.60 m / scale, linear between.
			var rings: Array = STREET.trunk(species, variant)[0]
			var narrowest := INF
			var top := -INF
			for ring: Array in rings:
				var centre: Vector3 = ring[0]
				if top >= 0.6 / smallest: break
				top = centre.y
				narrowest = minf(narrowest, (float(ring[1]) * cos(PI / STREET.TRUNK_SIDES) - Vector2(centre.x, centre.z).length()) * smallest)
			check(top >= 0.6 / smallest and narrowest >= 0.099 and (rings[0][0] as Vector3).y <= 0.0, label + " trunk encloses the box to car height: %.4f m" % narrowest)
			# Pit fixtures and the trunk foot stay inside the 36 cm pit square
			# (38.9 cm at the largest scale: inside the 40 cm footprint); the
			# frame or mound rises above the 3 cm sidewalk.
			var body := arrays(set["%s_body_%d" % [species, variant]])
			var vertices: PackedVector3Array = body[Mesh.ARRAY_VERTEX]
			var widest := 0.0
			var rise := -INF
			for p in vertices:
				if p.y > 0.08: continue
				widest = maxf(widest, maxf(absf(p.x), absf(p.z)))
				rise = maxf(rise, p.y)
			check(widest * largest <= float(STREET.PIT_CM) / 200.0 + 0.0001 and widest <= STREET.PIT_HALF + 0.0001, label + " ground fixtures inside the pit: %.4f m" % widest)
			if variant != 2: check(rise >= 0.044, label + " pit rises above the sidewalk: %.3f m" % rise)
			var aabb: AABB = set["%s_body_%d" % [species, variant]].get_aabb().merge(set["%s_cards_%d" % [species, variant]].get_aabb())
			sizes.append("%s: %.2f m tall, reach %.2f m, %d vertices" % [label, aabb.end.y, reach, points])
	print("street trees: ", "; ".join(sizes))
	check(set.pine_cards_2 == set.pine_cards_0 and set.pine_far_2 == set.pine_far_0, "the median-bed pine shares the mature pine's crown")

## Rims enclose their proxy boxes; everything stays within box + 3 cm (with
## sway) and below the piece's height cap.
func beds(set: Dictionary) -> void:
	for piece: String in STREET.PIECES:
		var size: Vector3 = STREET.PIECES[piece]
		var names := []
		for style: int in STREET.RIM_STYLES[piece]: names.append("%s_rim_%d" % [piece, style])
		for kind in STREET.PLANTINGS: names.append("%s_planting_%d" % [piece, kind])
		names.append(piece + "_far")
		for name: String in names:
			var data := arrays(set[name])
			check(not data.is_empty(), name + " has geometry")
			if data.is_empty(): continue
			var vertices: PackedVector3Array = data[Mesh.ARRAY_VERTEX]
			var weights: PackedVector2Array = data[Mesh.ARRAY_TEX_UV2]
			var colors: PackedColorArray = data[Mesh.ARRAY_COLOR]
			var out := 0.0
			var high := -INF
			var lowest := INF
			var flowers := 0
			for i in vertices.size():
				var p := vertices[i]
				var sway := weights[i].x * VEG.SWAY_PEAK * VEG.TREE_SWAY
				out = maxf(out, maxf(absf(p.x) - size.x * 0.5, absf(p.z) - size.z * 0.5) + sway)
				high = maxf(high, p.y)
				lowest = minf(lowest, p.y)
				var c := colors[i]
				if c.a < 0.5 and maxf(c.r, c.b) > c.g and maxf(c.r, maxf(c.g, c.b)) - minf(c.r, minf(c.g, c.b)) > 0.25 * maxf(c.r, maxf(c.g, c.b)): flowers += 1
			check(out <= 0.03 and high <= float(STREET.PLANT_CAP[piece]) and lowest >= -0.021, "%s stays within its box + 3 cm (%.3f) and below %.2f m (%.3f)" % [name, out, STREET.PLANT_CAP[piece], high])
			if name.contains("_rim_") or name.ends_with("_far"):
				var aabb: AABB = set[name].get_aabb()
				check(aabb.position.x <= -size.x * 0.5 - 0.0049 and aabb.end.x >= size.x * 0.5 + 0.0049 and aabb.position.z <= -size.z * 0.5 - 0.0049 and aabb.end.z >= size.z * 0.5 + 0.0049 and aabb.position.y <= 0.0 and aabb.end.y >= size.y + 0.0049, name + " encloses its proxy box")
			if name.ends_with("_planting_0") or (name.ends_with("_planting_2") and piece != "median_bed"):
				check(flowers >= 30, name + " carries flower heads: %d vertices" % flowers)

## Variants by identity (young share, scale span, quarter turns) and rim
## styles and median hedges by 48 m block.
func choices() -> void:
	var young := 0
	var quarters := {}
	var styles := {}
	for i in 600:
		var object := {"id": "tree-%d" % i, "asset_id": "builtin:street_tree_zelkova", "position": [i * 250, 0, 0], "quarter_turns": 1, "yaw_offset_mdeg": 12000}
		var piece: Dictionary = STREET.pieces(object)[0]
		check(piece == STREET.pieces(object.duplicate(true))[0], "a tree's variant is stable")
		if piece.variant == 1:
			young += 1
			check(is_equal_approx(piece.scale, 1.0), "young trees stand at scale 1")
		else: check(piece.scale >= STREET.SCALE.x - 0.0001 and piece.scale <= STREET.SCALE.y + 0.0001, "mature scale in the span")
		# The crown turns with the placement plus a quarter turn.
		var basis: Basis = (piece.transform as Transform3D).basis.orthonormalized()
		var yaw := basis.get_euler().y
		var turn := wrapf(yaw - (PI / 2.0 + deg_to_rad(12.0)), 0.0, TAU)
		quarters[roundi(turn / (PI / 2.0)) % 4] = true
		check(absf(turn - roundi(turn / (PI / 2.0)) * PI / 2.0) < 0.001, "crowns turn by quarter turns from the placement yaw")
		var bed := {"id": "bed-%d" % i, "asset_id": "builtin:planter", "position": [i * 1200, 0, 0], "quarter_turns": 0}
		var parts := STREET.pieces(bed)
		styles[parts[0].lods.body[0]] = true
		var near := bed.duplicate(true)
		near.id = "other-%d" % i
		near.position = [i * 1200 + (1 if (i * 1200) % 4800 < 4700 else -1), 0, 1]
		check(STREET.pieces(near)[0].lods.body[0] == parts[0].lods.body[0], "neighbours in a block share a rim style")
	check(young >= 40 and young <= 110, "about 12 %% young replacements: %d of 600" % young)
	check(quarters.size() == 4 and styles.size() == 4, "all quarter turns and rim styles occur: %s %s" % [quarters.keys(), styles.keys()])
	var pine := STREET.pieces({"id": "median", "asset_id": "builtin:median_bed_pine", "position": [0, 0, 0], "quarter_turns": 0})
	check(pine.size() == 3 and pine[2].variant == 2 and pine[2].lods.body == ["pine_body_2"] and (pine[2].transform as Transform3D).origin.is_zero_approx(), "a median-bed pine stands at the bed's centre without a pit")
	check(not STREET.pieces({"id": "median", "asset_id": "builtin:median_bed", "position": [0, 0, 0], "quarter_turns": 0})[0].lods.body[0].ends_with("_3"), "median beds have no painted box rim")

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

func objects() -> Array:
	var result := []
	for i in 10:
		result.append({"id": "tree-%d" % i, "asset_id": STREET.IDS[i % 5], "position": [i * 270, 0, 300], "quarter_turns": 0, "yaw_offset_mdeg": 15000})
	result.append({"id": "planter-a", "asset_id": "builtin:planter", "position": [500, 0, 900], "quarter_turns": 0})
	result.append({"id": "square-a", "asset_id": "builtin:planter_square", "position": [900, 0, 900], "quarter_turns": 1})
	result.append({"id": "median-a", "asset_id": "builtin:median_bed", "position": [600, 0, -600], "quarter_turns": 0})
	result.append({"id": "median-b", "asset_id": "builtin:median_bed_pine", "position": [960, 0, -600], "quarter_turns": 0})
	return result

## Ground quad and the owner cell's pieces (their proxy boxes).
func fixture(prepared: bool) -> Dictionary:
	var vertices := PackedVector3Array([Vector3(-40, 0, 40), Vector3(40, 0, 40), Vector3(40, 0, -40), Vector3(-40, 0, 40), Vector3(40, 0, -40), Vector3(-40, 0, -40)])
	var ids := ["ground", "ground"]
	var materials := {}
	for object: Dictionary in objects():
		var first := vertices.size()
		var p: Array = object.position
		var id: String = object.asset_id
		if id.begins_with("builtin:street_tree_"): proxy_box(vertices, Vector3(p[0], 70, -p[2]), Vector3(14, 140, 14))
		elif id == "builtin:planter": proxy_box(vertices, Vector3(p[0], 7, -p[2]), Vector3(80, 14, 30))
		elif id == "builtin:planter_square": proxy_box(vertices, Vector3(p[0], 9, -p[2]), Vector3(60, 18, 60))
		else:
			proxy_box(vertices, Vector3(p[0], 4, -p[2]), Vector3(300, 8, 80))
			if id == "builtin:median_bed_pine": proxy_box(vertices, Vector3(p[0], 70, -p[2]), Vector3(14, 140, 14))
		for i in (vertices.size() - first) / 3: ids.append(object.id)
		materials[object.id] = id
	var chunk := {"cell": Vector2i(0, 0), "triangles": [], "objects": objects(), "scene_vertices": vertices, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for id: String in ids:
		chunk.triangles.append({"surface": "concrete", "object_id": id})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"proxy_materials": materials}
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var missing := STREET.IDS.filter(func(id: String) -> bool: return id not in RENDER.BUILTINS or id not in RENDER.PLAN.DRAWN_OVER)
	if not missing.is_empty():
		check(false, "renderer integration missing: %s" % [missing])
		return
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(objects().all(func(object): return hidden.has(object.id)) and not hidden.has("ground"), "proxy faces of every piece are hidden")
	for prepared in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var value := QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(prepared)
			var collision: Array = chunk.triangles.duplicate(true)
			var estimate := RENDER.PLAN.estimate(chunk, 0)
			var lease := Lease.new(estimate)
			var job := RENDER.begin(chunk, parent, func(bytes): return lease if bytes == estimate else null, estimate, cache)
			while not RENDER.advance(job): pass
			var label := "%s tier %d" % ["prepared" if prepared else "unprepared", level]
			check(job.error.is_empty(), label + " attaches: %s" % job.error)
			check(chunk.triangles == collision, label + " leaves the collision triangles untouched")
			var drawn := 0
			for mesh: MeshInstance3D in parent.find_children("*", "MeshInstance3D", true, false):
				if mesh.mesh is ArrayMesh: drawn += mesh.mesh.surface_get_array_len(0)
			check(drawn == 6, label + " draws no proxy faces: %d vertices" % drawn)
			var counts := {}
			var made: Dictionary = cache.mesh_set(STREET.KEY)
			for node: MultiMeshInstance3D in parent.find_children("StreetPlanting_*", "MultiMeshInstance3D", true, false):
				var kind: String = node.get_meta("mapkit_vegetation_kind")
				var lod: String = node.get_meta("mapkit_vegetation_lod")
				var key := "%s_%s" % [kind, lod]
				counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
				check(made.values().has(node.multimesh.mesh) and node.multimesh.use_custom_data and not node.get_meta("mapkit_object_ids", PackedStringArray()).is_empty(), label + " borrows the shared mesh with tint and object ids")
				var near := float(value.vegetation_near) * (0.6 if kind == "planter" else 1.0) + QUALITY.GROUP_REACH
				if lod == "far":
					check(is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == 0.0, label + " %s far mesh from the near range on, never ending" % kind)
				else:
					check(is_equal_approx(node.visibility_range_end, near) and node.visible == (lod == "body" or level > 0), label + " %s %s near range" % [kind, lod])
				var shadows: bool = (level >= (2 if lod == "cards" else 1)) if kind == "tree" else level >= 1
				check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), label + " %s %s shadows" % [kind, lod])
			# Ten street trees and the median pine (body, cards, far); four bed
			# pieces of a rim and a planting each, the rim carrying the far box.
			check(counts == {"tree_body": 11, "tree_cards": 11, "tree_far": 11, "planter_body": 8, "planter_far": 4}, label + " instances per kind and LOD: %s" % counts)
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), label + " cell lease retires with its nodes")
	check(cache.bytes() >= STREET.SHARED_BYTES, "one shared street planting set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared set retires after its last cell")
	QUALITY.current = {}
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(not standalone.find_children("StreetPlanting_*", "MultiMeshInstance3D", true, false).is_empty(), "standalone previews draw from the process-lifetime set")
	preview.free()
	var denied := CACHE.new(func(bytes): return null if bytes == STREET.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and parent.get_child_count() == 0 and not job.claims.has(STREET.KEY), "a refused set fails the cell before any upload: %s" % job.get("error", {}))
	while not RENDER.advance(job): pass
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Rendered runs: every species (mature and young) and bed piece in one frame.
func capture(set: Dictionary) -> void:
	var path := OS.get_environment("MAPKIT_STREET_PLANTING_CAPTURE")
	if DisplayServer.get_name() == "headless" or path.is_empty(): return
	root.size = Vector2i(1280, 720)
	var scene := Node3D.new()
	root.add_child(scene)
	var world := WorldEnvironment.new()
	world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color(0.62, 0.72, 0.82)
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_color = Color(0.75, 0.78, 0.85)
	world.environment.ambient_light_energy = 0.7
	scene.add_child(world)
	var sun := DirectionalLight3D.new()
	sun.rotation = Vector3(-0.85, 0.6, 0.0)
	sun.shadow_enabled = true
	scene.add_child(sun)
	var ground := MeshInstance3D.new()
	var plane := PlaneMesh.new()
	plane.size = Vector2(40, 20)
	ground.mesh = plane
	scene.add_child(ground)
	# Single nodes (the material's default leaf tint): the validator project's
	# Compatibility renderer draws instanced vegetation black in this scene.
	var place := func(names: Array, origin: Vector3, _custom: Color) -> void:
		for name: String in names:
			var node := MeshInstance3D.new()
			node.mesh = set[name]
			node.position = origin
			scene.add_child(node)
	for i in STREET.SPECIES.size():
		var species: String = STREET.SPECIES[i]
		for variant in 2:
			place.call(["%s_body_%d" % [species, variant], "%s_cards_%d" % [species, variant]], Vector3(-6.0 + i * 3.0 + variant * 1.3, 0, 0), VEG.leaf_custom(STREET.LEAVES[species], 0.5))
	for style in 4:
		for kind in 4:
			var origin := Vector3(-5.0 + style * 1.1, 0, 2.5 + kind * 0.55)
			place.call(["planter_rim_%d" % style], origin, Color(0, 0, 0, 1))
			place.call(["planter_planting_%d" % kind], origin, VEG.leaf_custom(STREET.PLANTING_LEAVES.planter[kind], 0.4))
			origin = Vector3(0.0 + style * 0.9, 0, 2.5 + kind * 0.85)
			place.call(["planter_square_rim_%d" % style], origin, Color(0, 0, 0, 1))
			place.call(["planter_square_planting_%d" % kind], origin, VEG.leaf_custom(STREET.PLANTING_LEAVES.planter[kind], 0.4))
	for kind in 4:
		var origin := Vector3(5.5, 0, 2.5 + kind * 1.0)
		place.call(["median_bed_rim_%d" % (0 if kind < 2 else 2), "median_bed_planting_%d" % kind], origin, VEG.leaf_custom(STREET.PLANTING_LEAVES.median_bed[kind], 0.4))
		if kind == 1: place.call(["pine_body_2", "pine_cards_2"], origin, VEG.leaf_custom(STREET.LEAVES.pine, 0.3))
	var camera := Camera3D.new()
	scene.add_child(camera)
	camera.current = true
	camera.position = Vector3(1.0, 3.2, -8.0)
	camera.look_at(Vector3(1.0, 1.0, 2.0), Vector3.UP)
	for frame in 8: await process_frame
	await RenderingServer.frame_post_draw
	var picture := root.get_texture().get_image()
	picture.save_png(path)
	print("street_planting_validator: capture ", path)
	scene.queue_free()
	for frame in 3: await process_frame
