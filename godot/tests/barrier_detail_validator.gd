extends SceneTree
## City barrier detail (barrier_detail_meshes.gd, barrier_detail.gdshader,
## native barrier_details.rs): the shared set builds one part per call,
## identically, inside its memory allowance, with an opaque, a dithered
## (near infill) and a blended (far, never discarding) material; native
## presentation through the bridge lists runs and posts of every fence,
## wall and hedge family at 1:3 (bent and joined lines, a road gate, a
## closed loop, flat and sloped ground); every placed vertex (the vertex
## shader's conversion mirrored on the CPU) stays inside the union of its
## barrier's collision prisms; the renderer hides the prism faces only with
## a list, instances exactly the planned groups with the shared meshes,
## sets ranges and shadows per tier (low: far meshes only), reserves and
## releases the cell's bytes with its lease, fails the cell when the shared
## set is refused and draws from the process-lifetime set without a cache
## (keeping run tops for editor picking).
const RENDER := preload("../chunk_renderer.gd")
const BARRIERS := preload("../barrier_detail_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const PLAN := preload("../render_memory.gd")
var failed := false
## Barrier IDs whose designs (barrier_design.rs) are each family and style.
const FAMILIES := {"bars-plain-4": 40, "bars-spear-8": 40, "bars-arch-0": 40, "bars-tall-27": 67, "mesh-0": 67, "chain-0": 67, "boards-13": 40, "pickets-3": 40,
	"pipe-3": 27, "brick-cap-8": 67, "brick-soldier-5": 67, "block-2": 67, "precast-1": 67, "stone-9": 67, "giwa-5": 67, "hedge-flat-2": 27, "hedge-round-23": 27}

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
	var chunk := native()
	if not chunk.is_empty():
		contained(chunk, set, "flat")
		await renderer(chunk)
	# Sloped cells: posts and runs at a cell seam may reach into the
	# neighbouring cell's piece, so the union spans every listed cell.
	var sloped := slope()
	var union := {}
	for cell: Dictionary in sloped:
		var shapes := prisms(cell)
		for id: String in shapes: union[id] = union.get(id, []) + shapes[id]
	for cell: Dictionary in sloped: contained(cell, set, "sloped", union)
	QUALITY.current = {}
	print("barrier_detail_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## The shared set: one part per call, identical rebuilds, the measured
## buffers inside SHARED_BYTES, three materials (opaque, dithered infill,
## blended far without discard), and the shader's variant rule mirrored.
func meshes() -> Dictionary:
	var lease := Lease.new()
	var job := {}
	var steps := 0
	while not BARRIERS.build(job, lease): steps += 1
	check(steps == BARRIERS.STEPS.size() - 1, "one part per call: %d steps" % (steps + 1))
	var again := {}
	while not BARRIERS.build(again): pass
	var vertices := 0
	var indices := 0
	var counts := []
	for role: String in BARRIERS.NEAR_ROLES + BARRIERS.FAR_ROLES:
		var mesh: ArrayMesh = job.result[role]
		check(mesh.get_surface_count() == 1, role + ": one surface")
		check(mesh.surface_get_arrays(0) == (again.result[role] as ArrayMesh).surface_get_arrays(0), role + " builds identically")
		var wanted: Material = job.result.infill_material if role == "infill" else (job.result.far_material if role == "fence_far" else job.result.material)
		check(mesh.surface_get_material(0) == wanted, role + " uses its material")
		vertices += (mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX] as PackedVector3Array).size()
		indices += mesh.surface_get_array_index_len(0)
		counts.append("%s %d triangles" % [role, mesh.surface_get_array_index_len(0) / 3])
	print("barrier meshes: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared barrier set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, BARRIERS.SHARED_BYTES])
	check(mesh_bytes + 96 * 1024 <= BARRIERS.SHARED_BYTES, "the shared set fits its allowance: %d B" % mesh_bytes)
	check(lease.refs.size() == BARRIERS.STEPS.size() + 3, "the lease tracks the materials, their shaders and every mesh")
	var opaque: String = (job.result.material as ShaderMaterial).shader.code
	var infill: String = (job.result.infill_material as ShaderMaterial).shader.code
	var far: String = (job.result.far_material as ShaderMaterial).shader.code
	check(not opaque.contains("#define BARRIER"), "the opaque material is neither dithered nor blended")
	check(infill.contains("#define BARRIER_INFILL") and not infill.contains("#define BARRIER_FAR"), "the near infill alpha-tests")
	check(far.contains("#define BARRIER_FAR") and far.contains("blend_mix") and not far.contains("#define BARRIER_INFILL"), "the far fence blends")
	# discard only inside the near infill's block, so far meshes never discard.
	var guarded := opaque.substr(opaque.find("#ifdef BARRIER_INFILL"))
	guarded = guarded.substr(0, guarded.find("#endif"))
	check(opaque.count("discard;") == 1 and guarded.contains("discard;"), "discard only in the near infill")
	check(job.result.material.get_meta("mapkit_opaque", false) and job.result.infill_material.get_meta("mapkit_opaque", false) and not job.result.far_material.get_meta("mapkit_opaque", true), "opaque metadata")
	var palette: PackedVector3Array = job.result.material.get_shader_parameter("barrier_palette")
	check(palette.size() == 40, "the material binds the palette: %d" % palette.size())
	# The CPU mirror of barrier_keep(): every variant of the meshes has a rule.
	var variants := {}
	for role: String in BARRIERS.NEAR_ROLES + BARRIERS.FAR_ROLES:
		for value: Vector2 in (job.result[role] as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_TEX_UV]: variants[int(floorf(value.y * 0.5))] = true
	for variant: int in variants:
		var kept := false
		for bits in 4096:
			for flags in [0, 1, 3]:
				kept = kept or BARRIERS.keep(variant, bits | 3 << 12, flags, 2.0)
		check(kept, "variant %d belongs to some design" % variant)
		check(variant == 0 or opaque.contains("v==%d" % variant) or opaque.contains("v>=%d" % variant) or opaque.contains("v<=%d" % variant) or variant in [2, 3, 4, 7, 8, 31], "the shader keeps variant %d" % variant)
	return job.result

func document(heightmap := false) -> String:
	var barriers := []
	var k := 0
	for id: String in FAMILIES:
		var kind := "hedge" if id.begins_with("hedge") else ("wall" if id in ["brick-cap-8", "brick-soldier-5", "block-2", "precast-1", "stone-9", "giwa-5"] else "fence")
		var thickness: int = {"fence": 5, "wall": 10, "hedge": 20}[kind]
		# A gentle (6°) and a hard (47°) joint on every line.
		var y := 2000 + k * 600
		barriers.append({"id": id, "kind": kind, "line": [[2000, y], [6000, y], [9000, y + 315], [9300, y + 715]], "height_cm": FAMILIES[id], "thickness_cm": thickness})
		k += 1
	# A fence across the ground road (a gate) and a closed wall loop.
	barriers.append({"id": "gate-fence", "kind": "fence", "line": [[12000, 22000], [12000, 29000]], "height_cm": 40, "thickness_cm": 5})
	barriers.append({"id": "loop-wall", "kind": "wall", "line": [[14000, 2000], [18000, 2000], [18000, 5000], [14000, 5000], [14000, 2000]], "height_cm": 67, "thickness_cm": 10})
	barriers.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return str(a.id) < str(b.id))
	var d := {"free_roam": false, "map_id": "barrier-detail", "revision": 1, "bounds": {"min": [0, 0], "max": [102400, 102400]}, "cell_size_cm": 51200,
		"seed": 42, "recipe_version": 1, "theme": "default", "terrain_base_cm": 0,
		"heightmaps": [{"cell": {"x": 0, "y": 0}, "path": "h.png", "spacing_cm": 400, "offset_cm": 0, "step_cm": 1}] if heightmap else [],
		"nodes": [{"id": "west", "position": [0, 20, 25600], "level": 0}, {"id": "east", "position": [102400, 20, 25600], "level": 0}],
		"roads": [{"id": "ground-road", "from": "west", "to": "east", "points": [[0, 20, 25600], [51200, 20, 25600], [102400, 20, 25600]], "widths_cm": [800, 800], "surfaces": ["asphalt", "asphalt"], "kind": "ground", "clearance_cm": null, "sidewalk_cm": null}],
		"buildings": [], "zones": [], "assets": [], "placements": [], "barriers": barriers,
		"attributions": [{"source": "osm#layer", "license": "ODbL-1.0", "notice": JSON.stringify({"adapter": "osm-extract-v1", "authored_units": {"metres_per_unit": 1, "source_denominator": 3}})}],
		"provenance": {"tool_id": "barrier-detail-validator", "version": "1", "build_id": "test", "fingerprint": "untrusted-and-accepted", "first_created": "2026-11-06T00:00:00Z", "last_edited": "2026-11-06T00:00:00Z"}}
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
	var runs := BARRIERS.runs(chunk)
	var posts := BARRIERS.posts(chunk)
	var ids := BARRIERS.ids(chunk)
	check(runs.size() > 0 and runs.size() % BARRIERS.RUN_FLOATS == 0 and posts.size() % BARRIERS.POST_FLOATS == 0 and not bool(chunk.presentation.barrier_capped), "runs and posts listed")
	var families := {}
	var kinds := {}
	var gates := 0
	var corners := {}
	for i in range(0, runs.size(), BARRIERS.RUN_FLOATS):
		var bits := int(runs[i + 9])
		var id: String = ids[int(runs[i + 12])]
		families[id] = bits
		check((bits >> 12) & 15 == 3, "the 1:3 scale rides in the bits")
		check(Vector2(runs[i + 3], runs[i + 5]).length() > 0.0 and runs[i + 8] > 0.0, "runs have a length and height")
	for i in range(0, posts.size(), BARRIERS.POST_FLOATS):
		var id: String = ids[int(posts[i + 12])]
		var kind := int(posts[i + 10])
		kinds[kind] = int(kinds.get(kind, 0)) + 1
		if kind == BARRIERS.POST_GATE: gates += 1
		if kind == BARRIERS.POST_CORNER: corners[id] = int(corners.get(id, 0)) + 1
		check(posts[i + 11] > 0.0 and posts[i + 11] <= 1.25001, "post sizes")
	for id: String in FAMILIES:
		check(families.has(id), "%s is listed" % id)
		if id.begins_with("hedge"): continue
		check(int(corners.get(id, 0)) == 1, "%s: one corner post at its hard joint, none at the gentle one: %d" % [id, int(corners.get(id, 0))])
	check(gates == 2, "two gate posts at the road: %d" % gates)
	check(int(corners.get("loop-wall", 0)) == 4, "four corner piers on the closed loop")
	# The designs the IDs were chosen for (barrier_design.rs).
	var want := {"bars-plain-4": [1, 0, 0, 0], "bars-spear-8": [1, 0, 1, 0], "bars-arch-0": [1, 0, 2, 0], "mesh-0": [1, 1, 0, 0], "chain-0": [1, 2, 0, 0],
		"boards-13": [1, 3, 0, 0], "pickets-3": [1, 3, 0, 1], "pipe-3": [1, 4, 0, 0], "brick-soldier-5": [2, 0, 1, 0], "precast-1": [2, 2, 0, 0], "giwa-5": [2, 3, 1, 0], "hedge-round-23": [0, 1, 1, 0]}
	for id: String in want:
		var bits: int = families.get(id, 0)
		check([bits & 3, (bits >> 2) & 7, (bits >> 8) & 3, (bits >> 10) & 3] == want[id], "%s design %s" % [id, [bits & 3, (bits >> 2) & 7, (bits >> 8) & 3, (bits >> 10) & 3]])
	print("barrier posts by kind: ", kinds, " runs ", runs.size() / BARRIERS.RUN_FLOATS)
	# Every barrier face of the cell is hidden with the list, none without.
	var view := RENDER.DATA.view(chunk)
	var hidden := PLAN.hidden_objects(view)
	for id: String in families: check(hidden.has(id), "%s prism faces hidden" % id)
	var bare := chunk.duplicate()
	bare.presentation = chunk.presentation.duplicate()
	bare.presentation.erase("barrier_runs")
	var shown := PLAN.hidden_objects(RENDER.DATA.view(bare))
	check(not families.keys().any(func(id): return shown.has(id)), "without a list the prisms stay drawn")
	check(PLAN.barrier_bytes(view) > 0 and PLAN.barrier_bytes(RENDER.DATA.view(bare)) == 0, "the estimate carries the barrier reservation")
	return chunk

## Collision prisms of a chunk by barrier ID: [planes (normal, offset), bounds].
static func prisms(chunk: Dictionary) -> Dictionary:
	var view: Dictionary = chunk.geometry.view()
	var values: PackedInt64Array = view.asset_convex_vertices_cm
	var offsets: PackedInt32Array = view.asset_convex_offsets
	var ids: PackedStringArray = view.asset_convex_ids
	var faces := [[0,4,6],[0,6,2],[1,3,7],[1,7,5],[0,1,5],[0,5,4],[2,6,7],[2,7,3],[0,2,3],[0,3,1],[4,5,7],[4,7,6]]
	var result := {}
	for c in ids.size():
		if offsets[c + 1] - offsets[c] != 24: continue
		var points := []
		var centre := Vector3.ZERO
		for k in 8:
			var at := offsets[c] + k * 3
			points.append(Vector3(values[at], values[at + 1], -values[at + 2]) * 0.01)
			centre += points[-1] / 8.0
		var planes := []
		var box := AABB(points[0], Vector3.ZERO)
		for p: Vector3 in points: box = box.expand(p)
		for f: Array in faces:
			var a: Vector3 = points[f[0]]
			var n: Vector3 = (points[f[1]] - a).cross(points[f[2]] - a)
			if n.length_squared() < 1e-12: continue
			n = n.normalized()
			if n.dot(centre - a) > 0.0: n = -n
			planes.append([n, n.dot(a)])
		if not result.has(ids[c]): result[ids[c]] = []
		result[ids[c]].append([planes, box.grow(0.002)])
	return result

## Every vertex of every instance, placed as the vertex shader places it,
## lies inside the union of its barrier's prisms (1 mm).
func contained(chunk: Dictionary, set: Dictionary, label: String, union := {}) -> void:
	var shapes := prisms(chunk) if union.is_empty() else union
	var plan := BARRIERS.plan(chunk)
	var tested := 0
	var outside := []
	for key: String in plan:
		var group: Dictionary = plan[key]
		var arrays: Array = (set[group.role] as ArrayMesh).surface_get_arrays(0)
		var count: int = (arrays[Mesh.ARRAY_VERTEX] as PackedVector3Array).size()
		var buffer: PackedFloat32Array = group.buffer
		for i in buffer.size() / 16:
			var b := buffer.slice(i * 16, i * 16 + 16)
			var t := Transform3D(Basis(Vector3(b[0], b[4], b[8]), Vector3(b[1], b[5], b[9]), Vector3(b[2], b[6], b[10])), Vector3(b[3], b[7], b[11]))
			var custom := Color(b[12], b[13], b[14], b[15])
			var id: String = group.ids[i]
			var own: Array = shapes.get(id, [])
			for k in count:
				var p: Variant = BARRIERS.placed(arrays, k, t, custom)
				if p == null: continue
				tested += 1
				var inside := false
				for shape: Array in own:
					if not (shape[1] as AABB).has_point(p): continue
					inside = (shape[0] as Array).all(func(plane: Array) -> bool: return (plane[0] as Vector3).dot(p) - float(plane[1]) <= 0.001)
					if inside: break
				if not inside and outside.size() < 8: outside.append("%s %s %s" % [group.role, id, p])
	print("%s: %d placed vertices tested" % [label, tested])
	check(tested > 1000 and outside.is_empty(), "%s: every placed vertex stays inside its barrier's prisms: %s" % [label, outside])

## Sloped cells: the document with a 12 % / 6 % heightmap through the
## bounded source preview (32 m cells, 2 m terrain grid).
func slope() -> Array:
	var directory := OS.get_user_data_dir().path_join("barrier_slope")
	DirAccess.make_dir_recursive_absolute(directory)
	var side := 129
	var rows := PackedByteArray()
	for j in side:
		rows.append(0)
		for i in side:
			var h := mini(int(i * 400 * 0.12 + j * 400 * 0.06), 65535)
			rows.append(h >> 8)
			rows.append(h & 255)
	var png := PackedByteArray([137, 80, 78, 71, 13, 10, 26, 10])
	var header := PackedByteArray()
	for value in [side, side]: header.append_array(PackedByteArray([value >> 24 & 255, value >> 16 & 255, value >> 8 & 255, value & 255]))
	header.append_array(PackedByteArray([16, 0, 0, 0, 0]))
	png.append_array(chunk_bytes("IHDR", header))
	png.append_array(chunk_bytes("IDAT", rows.compress(FileAccess.COMPRESSION_DEFLATE)))
	png.append_array(chunk_bytes("IEND", PackedByteArray()))
	var file := FileAccess.open(directory.path_join("h.png"), FileAccess.WRITE)
	file.store_buffer(png)
	file.close()
	var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
	var chunks := []
	for y in 4:
		for x in 3:
			var generated: Dictionary = bridge.source_preview_packed(document(true), directory, x, y)
			check(generated.get("ok", false), "sloped preview %d,%d: %s" % [x, y, generated.get("error", {})])
			if generated.get("ok", false) and not BARRIERS.runs(generated.data.chunk).is_empty(): chunks.append(generated.data.chunk)
	check(chunks.size() >= 6, "sloped cells with barriers: %d" % chunks.size())
	return chunks

static func chunk_bytes(type: String, data: PackedByteArray) -> PackedByteArray:
	var body := type.to_ascii_buffer()
	body.append_array(data)
	var out := PackedByteArray([data.size() >> 24 & 255, data.size() >> 16 & 255, data.size() >> 8 & 255, data.size() & 255])
	out.append_array(body)
	var crc := 0xFFFFFFFF
	for byte in body:
		crc ^= byte
		for _bit in 8: crc = (crc >> 1) ^ (0xEDB88320 & -(crc & 1))
	crc ^= 0xFFFFFFFF
	out.append_array(PackedByteArray([crc >> 24 & 255, crc >> 16 & 255, crc >> 8 & 255, crc & 255]))
	return out

func renderer(chunk: Dictionary) -> void:
	var view := RENDER.DATA.view(chunk)
	# Every group of a square shares one AABB: its posts and piers join the
	# square of the run they stand on.
	var plan := BARRIERS.plan(chunk)
	var squares := {}
	for key: String in plan:
		var square := key.get_slice("/", 1)
		if not squares.has(square): squares[square] = plan[key].bounds
		check(is_same(squares[square], plan[key].bounds), "%s shares its square's bounds" % key)
	var orphans := plan.keys().filter(func(key: String) -> bool: return key.begins_with("post/") or key.begins_with("pier/")).filter(
		func(key: String) -> bool: return not plan.has("fence_far/" + key.get_slice("/", 1)) and not plan.has("solid_far/" + key.get_slice("/", 1)))
	check(orphans.is_empty(), "posts and piers share a square with the far group that paints them: %s" % [orphans])
	var low_plan := BARRIERS.plan(chunk, false, true)
	check(low_plan.size() <= 2 and low_plan.keys().all(func(key: String) -> bool: return key in ["fence_far/cell", "solid_far/cell"]), "a low plan is one far group per role for the cell: %s" % [low_plan.keys()])
	var cache := CACHE.new(func(amount): return Lease.new(amount))
	cache.quality_profile = QUALITY.profile(1)
	var camera := Camera3D.new()
	root.add_child(camera)
	camera.position = Vector3(50.0, 3.0, -10.0)
	camera.look_at(Vector3(60.0, 0.0, -40.0))
	for level in 3:
		QUALITY.current = QUALITY.profile(level)
		var expected := {}
		var tier_plan := BARRIERS.plan(chunk, false, level == 0)
		for key: String in tier_plan: expected[tier_plan[key].role] = int(expected.get(tier_plan[key].role, 0)) + (tier_plan[key].buffer as PackedFloat32Array).size() / 16
		var parent := Node3D.new()
		root.add_child(parent)
		var planned := PLAN.estimate(view, 0)
		var lease := Lease.new(planned)
		var job := RENDER.begin(chunk, parent, func(amount): return lease if amount == planned else null, planned, cache)
		while not RENDER.advance(job): pass
		var label := "tier %d" % level
		check(job.error.is_empty(), label + " attaches: %s" % job.error)
		var counts := {}
		var buffers := 0
		var reach := float(QUALITY.profile(level).barrier_near) + QUALITY.GROUP_REACH
		var boxes := {}
		for node: MultiMeshInstance3D in parent.find_children("Barrier_*", "MultiMeshInstance3D", true, false):
			var far_node := str(node.get_meta("mapkit_barrier_role")).ends_with("_far")
			boxes[node.multimesh.custom_aabb] = int(boxes.get(node.multimesh.custom_aabb, 0)) | (2 if far_node else 1)
			check(node.multimesh.custom_aabb.has_volume(), label + " groups carry their square's custom AABB")
			check(level > 0 or node.get_meta("mapkit_barrier_low_plan", false), label + " low cells plan whole-cell far groups")
			var role := str(node.get_meta("mapkit_barrier_role"))
			counts[role] = int(counts.get(role, 0)) + node.multimesh.instance_count
			buffers += node.multimesh.instance_count * 64
			check(node.multimesh.mesh == cache.mesh_set(BARRIERS.KEY).get(role) and node.multimesh.use_custom_data, label + " borrows the shared mesh with custom data")
			check(node.get_meta("mapkit_static_detail", false) and not node.get_meta("mapkit_decoration", true), label + " solid detail metadata")
			check((node.get_meta("mapkit_object_ids") as PackedStringArray).size() == node.multimesh.instance_count, label + " one barrier ID per instance")
			check(not node.has_meta("mapkit_pick_faces"), label + " leased cells keep no pick faces")
			var far := role.ends_with("_far")
			check(node.get_meta("mapkit_barrier_lod") == ("far" if far else "near"), label + " lod metadata")
			if far:
				check(node.visible and is_equal_approx(node.visibility_range_begin, reach if level > 0 else 0.0) and node.visibility_range_end == 0.0, label + " %s begins at the near reach and never ends" % role)
			else:
				check(node.visible == (level > 0), label + " %s near visibility" % role)
				if level > 0: check(is_equal_approx(node.visibility_range_end, reach), label + " %s ends at the near reach" % role)
			var shadows := node.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON
			var want: bool = {"solid": true, "pier": true, "solid_far": true, "fence": level >= 1, "post": level >= 1, "infill": level >= 2, "fence_far": false}[role]
			check(shadows == want, label + " %s shadows" % role)
		check(counts == expected, label + " instances the planned groups: %s vs %s" % [counts, expected])
		check(level == 0 or boxes.values().all(func(lods: int) -> bool: return lods == 3), label + " every near group's AABB is a far group's (one handover centre)")
		check(buffers <= PLAN.barrier_bytes(view), label + " the estimate covers the instance buffers")
		if DisplayServer.get_name() != "headless":
			camera.make_current()
			for _frame in 3: await process_frame
		RENDER.cancel(job)
		parent.queue_free()
		for _frame in 4: await process_frame
		check(lease.retired(), label + " cell lease retires with its barrier nodes")
	check(cache.bytes() >= BARRIERS.SHARED_BYTES, "one shared barrier set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared barrier set retires after its last cell")
	# Without a cache (editor and standalone previews): the process-lifetime
	# set, and the far groups keep the run tops for picking.
	var preview := Node3D.new()
	root.add_child(preview)
	RENDER.attach(chunk, preview)
	var picked := preview.find_children("Barrier_*_far_*", "MultiMeshInstance3D", true, false)
	check(not picked.is_empty() and picked.all(func(node): return node.has_meta("mapkit_pick_faces") and (node.get_meta("mapkit_pick_faces") as PackedVector3Array).size() == node.multimesh.instance_count * 6), "standalone previews draw barriers and keep pick faces")
	preview.free()
	# A refused shared set fails the cell: its prism faces are hidden.
	var denied := CACHE.new(func(amount): return null if amount == BARRIERS.SHARED_BYTES else Lease.new(amount))
	var parent := Node3D.new()
	root.add_child(parent)
	var job := RENDER.begin(chunk, parent, func(amount): return Lease.new(amount), PLAN.estimate(view, 0), denied)
	while not RENDER.advance(job): pass
	check(str(job.error.get("code", "")) == "E_MEMORY_BUDGET", "a refused barrier set fails the cell: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	camera.queue_free()
	await process_frame
