extends SceneTree
## Road signs (road_sign_meshes.gd, chunk_renderer): pole signs on posts
## inside the unchanged 6 cm post box (everything below 0.66 m), plates within
## the importer's footprint and printed toward local -z; the guide sign's pole
## inside its 10 cm box with everything else above vehicle height and the
## board reaching over the road, an open far truss, board prints inside the
## blue field and laid out for the driver's right; printed colours per kind,
## near/far variants,
## the shared set's memory, materials, ranges and shadows per tier, planned
## groups per kind, variant and 32 m square with object ids and a retiring
## lease; every id listed in chunk_renderer BUILTINS and
## render_memory.DRAWN_OVER, hidden proxy faces with untouched collision
## triangles, instance counts, a refused set failing the cell before any upload
## and release of every reservation. Rendered runs compare a speed and a guide
## sign by day and night.
const RENDER := preload("../chunk_renderer.gd")
const SIGNS := preload("../road_sign_meshes.gd")
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
	geometry(set)
	colours(set)
	policy()
	await planned(set)
	await renderer()
	await lights()
	QUALITY.current = {}
	print("road_sign_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

static func triangles(mesh: ArrayMesh) -> int:
	var total := 0
	for surface in mesh.get_surface_count(): total += mesh.surface_get_array_index_len(surface) / 3
	return total

static func points(mesh: ArrayMesh) -> PackedVector3Array:
	var result := PackedVector3Array()
	for index in mesh.get_surface_count(): result.append_array(mesh.surface_get_arrays(index)[Mesh.ARRAY_VERTEX])
	return result

## Arrays of the surface whose material is `name`.
static func surface(mesh: ArrayMesh, materials: Dictionary, name: String) -> Array:
	for index in mesh.get_surface_count():
		if mesh.surface_get_material(index) == materials.get(name): return mesh.surface_get_arrays(index)
	return []

## Every part a kind draws at `lod` (guide: layout `layout`).
static func drawn(set: Dictionary, kind: String, lod: String, layout := 0) -> Array:
	return SIGNS.parts(kind, layout)[lod].map(func(key: String) -> ArrayMesh: return set[key])

static func all_points(meshes: Array) -> PackedVector3Array:
	var result := PackedVector3Array()
	for mesh: ArrayMesh in meshes: result.append_array(points(mesh))
	return result

static func all_triangles(meshes: Array) -> int:
	var total := 0
	for mesh: ArrayMesh in meshes: total += triangles(mesh)
	return total

## Normals of the printed surfaces of `meshes`.
static func printed_normals(meshes: Array, materials: Dictionary) -> PackedVector3Array:
	var result := PackedVector3Array()
	for mesh: ArrayMesh in meshes:
		var arrays: Array = surface(mesh, materials, "sign")
		if not arrays.is_empty(): result.append_array(arrays[Mesh.ARRAY_NORMAL])
	return result

static func mesh_keys() -> Array:
	return SIGNS.STEPS.filter(func(step: String) -> bool: return not step.begins_with("material_"))

func meshes() -> Dictionary:
	var lease := Lease.new()
	var state := {}
	var steps := 1
	while not SIGNS.build(state, lease): steps += 1
	check(steps == SIGNS.STEPS.size(), "one part per call: %d steps" % steps)
	var set: Dictionary = state.result
	var again := {}
	while not SIGNS.build(again): pass
	var vertices := 0
	var indices := 0
	var counts := []
	for key: String in mesh_keys():
		var mesh: ArrayMesh = set[key]
		var other: ArrayMesh = again.result[key]
		var names := []
		for index in mesh.get_surface_count():
			names.append(set.materials.find_key(mesh.surface_get_material(index)))
			check(mesh.surface_get_arrays(index) == other.surface_get_arrays(index), "%s builds identically" % key)
			vertices += mesh.surface_get_array_len(index)
			indices += mesh.surface_get_array_index_len(index)
		var printed_only: bool = key.ends_with("_digits") or key.begins_with("guide_board_")
		check(names == (["sign"] if printed_only else ["paint", "sign"]), "%s surfaces %s" % [key, names])
		counts.append("%s %d" % [key, triangles(mesh)])
	print("road sign triangles: ", ", ".join(counts))
	var mesh_bytes := 2 * (vertices * 56 + indices * 4)
	print("shared road sign set: %d vertices, %d indices, %d B arrays of %d B" % [vertices, indices, mesh_bytes, SIGNS.SHARED_BYTES])
	check(mesh_bytes + 65536 <= SIGNS.SHARED_BYTES, "the shared set's meshes, materials and build arrays fit its allowance: %d B" % mesh_bytes)
	check(mesh_bytes + 65536 >= SIGNS.SHARED_BYTES * 0.75, "the allowance stays near its measured need: %d B" % mesh_bytes)
	check(lease.refs.size() == SIGNS.STEPS.size(), "the lease tracks every mesh and material: %d" % lease.refs.size())
	check(SIGNS.KINDS.size() == 11 and SIGNS.POLE_KINDS.size() == 10, "eleven builtins, ten on posts")
	return set

func geometry(set: Dictionary) -> void:
	var post := SIGNS.POST_PROXY.grow(0.0015)
	for kind: String in SIGNS.POLE_KINDS:
		for lod: String in SIGNS.LODS:
			var meshes := drawn(set, kind, lod)
			var label := "%s %s" % [kind, lod]
			var below := 0
			var wide := 0
			var top := -INF
			var bottom := INF
			for point: Vector3 in all_points(meshes):
				if point.y < 0.66 and point.y >= 0.0 and not post.has_point(point): below += 1
				if absf(point.x) > 0.16 or absf(point.z) > 0.06: wide += 1
				top = maxf(top, point.y)
				bottom = minf(bottom, point.y)
			check(below == 0, label + " stands inside the 6 cm post box below 0.66 m: %d vertices outside" % below)
			check(wide == 0, label + " stays within 16 cm of the post across x and 6 cm along z: %d vertices outside" % wide)
			check(top <= 1.06 and top >= 1.03, label + " tops out with the post: %.3f" % top)
			check(bottom >= -SIGNS.SKIRT - 0.001, label + " reaches no lower than the skirt: %.3f" % bottom)
			check(all_triangles(meshes) <= (700 if lod == "near" else 60), label + " triangle count: %d" % all_triangles(meshes))
			# Printed fronts face the approaching traffic (local -z).
			var normals := printed_normals(meshes, set.materials)
			var facing := 0
			for normal: Vector3 in normals:
				if normal.z < -0.99: facing += 1
			check(facing == normals.size() and facing > 0, label + " printed faces all face -z: %d/%d" % [facing, normals.size()])
		var near := all_triangles(drawn(set, kind, "near"))
		var far := all_triangles(drawn(set, kind, "far"))
		check(near >= 5 * far, "the far %s is a simple variant: %d vs %d" % [kind, far, near])
	var pole := SIGNS.GUIDE_PROXY.grow(0.0015)
	var far_triangles := triangles(set.guide_far)
	for layout in 5:
		var lod := "far" if layout == 4 else "near"
		var meshes := drawn(set, "guide", lod, layout % 4)
		var label := "guide %s %d" % [lod, layout]
		var low := 0
		var reach := 0.0
		var top := -INF
		for point: Vector3 in all_points(meshes):
			if point.y < SIGNS.CLEAR_HEIGHT and point.y >= 0.0 and not pole.has_point(point): low += 1
			reach = maxf(reach, point.x)
			top = maxf(top, point.y)
		check(low == 0, label + " keeps everything below 1.70 m inside the pole box: %d vertices outside" % low)
		check(reach >= 2.5 and reach <= 2.6, label + " reaches over the road: %.3f" % reach)
		check(top <= 2.52 and top >= 2.48, label + " tops out at the pole: %.3f" % top)
		check(all_triangles(meshes) <= (1800 if lod == "near" else 120), label + " triangle count: %d" % all_triangles(meshes))
		var normals := printed_normals(meshes, set.materials)
		var facing := 0
		for normal: Vector3 in normals:
			if normal.z < -0.99: facing += 1
		check(facing == normals.size(), label + " printed board faces -z")
		if lod == "near": check(all_triangles(meshes) >= 5 * far_triangles, label + " is detailed against the far board: %d vs %d" % [all_triangles(meshes), far_triangles])
	for a in 4:
		for b in range(a + 1, 4):
			check(surface(set["guide_board_%d" % a], set.materials, "sign") != surface(set["guide_board_%d" % b], set.materials, "sign"), "guide layouts %d and %d differ" % [a, b])
	# The far truss stays open: painted parts outside the pole box (and its
	# skirt) are the two arm bars, the diagonal's ends between them or the
	# board behind its print.
	var board_box := Rect2(SIGNS.BOARD.position, SIGNS.BOARD.size).grow(0.001)
	var solid := 0
	var paint_far: Array = surface(set.guide_far, set.materials, "paint")
	var paint_vertices: PackedVector3Array = paint_far[Mesh.ARRAY_VERTEX]
	for point: Vector3 in paint_vertices:
		if point.y < 0.0 or pole.has_point(point) or board_box.has_point(Vector2(point.x, point.y)): continue
		if absf(point.y - 2.44) <= 0.03 or absf(point.y - SIGNS.LOWER_ARM) <= 0.03: continue
		solid += 1
	check(solid == 0, "the far guide truss is open (arm bars and a diagonal, no plate): %d vertices elsewhere" % solid)
	var diagonal := 0
	for t in range(0, paint_far[Mesh.ARRAY_INDEX].size(), 3):
		var c: Vector3 = (paint_vertices[paint_far[Mesh.ARRAY_INDEX][t]] + paint_vertices[paint_far[Mesh.ARRAY_INDEX][t + 1]] + paint_vertices[paint_far[Mesh.ARRAY_INDEX][t + 2]]) / 3.0
		if c.x > 0.06 and c.x < SIGNS.BOARD.position.x - 0.01 and c.y > SIGNS.LOWER_ARM + 0.05 and c.y < 2.39: diagonal += 1
	check(diagonal > 0 and diagonal <= 12, "the far truss keeps one diagonal between the pole and the board: %d triangles" % diagonal)
	# Every layout prints inside the board's inner field (border 27 mm).
	var field_half := SIGNS.BOARD.size * 0.5 - Vector2(0.027, 0.027)
	var field_centre := SIGNS.BOARD.get_center()
	for layout in 4:
		var outside := 0
		for point: Vector3 in points(set["guide_board_%d" % layout]):
			if absf(point.x - field_centre.x) > field_half.x + 0.0005 or absf(point.y - field_centre.y) > field_half.y + 0.0005: outside += 1
		check(outside == 0, "guide layout %d prints inside the blue field: %d vertices outside" % [layout, outside])
	# Handedness: on the -z front the viewer's right is local -x, so layout 1's
	# right branch ends at x = centre - 0.48 (its arrow tip) and nothing is
	# drawn at the mirrored spot.
	var front := SIGNS.face(Vector3(field_centre.x, field_centre.y, SIGNS.BOARD_FRONT), Vector3.FORWARD)
	check(SIGNS.at(front, Vector2(1.0, 0.0)).x < field_centre.x, "a -z face's right is local -x")
	var tip := Vector2(field_centre.x - 0.48, field_centre.y - 0.02)
	var mirror := Vector2(field_centre.x + 0.48, field_centre.y - 0.02)
	var at_tip := 0
	var at_mirror := 0
	for point: Vector3 in points(set.guide_board_1):
		if Vector2(point.x, point.y).distance_to(tip) < 0.001: at_tip += 1
		if Vector2(point.x, point.y).distance_to(mirror) < 0.01: at_mirror += 1
	check(at_tip > 0 and at_mirror == 0, "guide layout 1's branch points to the driver's right: %d at the tip, %d mirrored" % [at_tip, at_mirror])
	# Board prints lie just in front of the board; digits just in front of the plate.
	for key: String in mesh_keys():
		if not (key.begins_with("guide_board_") or key.ends_with("_digits")): continue
		var plane: float = SIGNS.BOARD_FRONT if key.begins_with("guide") else SIGNS.FRONT
		var off := 0
		for point: Vector3 in points(set[key]):
			if point.z > plane - 0.0005 or point.z < plane - 3.0 * SIGNS.PROUD - 0.0005: off += 1
		check(off == 0, key + " stands proud of its face: %d vertices elsewhere" % off)
	# The importer's variant: stable, spread over the layouts.
	var seen := {}
	for index in 64:
		var object := {"id": "import-l-place-%05d" % index}
		var layout := SIGNS.variant(object, 4)
		check(layout == SIGNS.variant(object, 4) and layout >= 0 and layout < 4, "variant is stable and in range")
		seen[layout] = true
	check(seen.size() == 4, "64 identities use every guide layout: %s" % [seen.keys()])

## Colour of the printed triangle under (x, y) in the plane z = `plane`.
static func colour_at(mesh: ArrayMesh, materials: Dictionary, p: Vector2, plane: float) -> Color:
	var arrays: Array = surface(mesh, materials, "sign")
	var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	for t in range(0, indices.size(), 3):
		var a := vertices[indices[t]]
		var b := vertices[indices[t + 1]]
		var c := vertices[indices[t + 2]]
		if absf(a.z - plane) > 0.0001 or absf(b.z - plane) > 0.0001 or absf(c.z - plane) > 0.0001: continue
		if Geometry2D.point_is_inside_triangle(p, Vector2(a.x, a.y), Vector2(b.x, b.y), Vector2(c.x, c.y)): return arrays[Mesh.ARRAY_COLOR][indices[t]]
	return Color(0, 0, 0, 0)

static func name_of(color: Color) -> String:
	if color.a == 0.0: return "none"
	if color.r > 0.4 and color.g < 0.1 and color.b < 0.1: return "red"
	if color.r > 0.6 and color.g > 0.6 and color.b > 0.6: return "white"
	if color.b > 0.15 and color.b > 2.5 * color.r: return "blue"
	if color.r > 0.5 and color.g > 0.3 and color.b < 0.05: return "yellow"
	if color.r < 0.02 and color.g < 0.02 and color.b < 0.02: return "black"
	return "other"

## Printed colour shares of a mesh's sign surface (vertex colours by role).
static func tints(mesh: ArrayMesh, materials: Dictionary) -> Dictionary:
	var arrays: Array = surface(mesh, materials, "sign")
	var result := {"red": 0, "white": 0, "blue": 0, "yellow": 0, "black": 0}
	for color: Color in arrays[Mesh.ARRAY_COLOR]:
		if color.r > 0.4 and color.g < 0.1 and color.b < 0.1: result.red += 1
		elif color.r > 0.6 and color.g > 0.6 and color.b > 0.6: result.white += 1
		elif color.b > 0.15 and color.b > 2.5 * color.r: result.blue += 1
		elif color.r > 0.5 and color.g > 0.3 and color.b < 0.05: result.yellow += 1
		elif color.r < 0.02 and color.g < 0.02 and color.b < 0.02: result.black += 1
	return result

func colours(set: Dictionary) -> void:
	var speeds := []
	for kind: String in SIGNS.POLE_KINDS:
		var near := tints(set[SIGNS.parts(kind).near[0]], set.materials)
		var far := tints(set[SIGNS.parts(kind).far[0]], set.materials)
		var label := "%s printed colours %s" % [kind, near]
		match kind:
			"crossing": check(near.blue > 0 and near.white > 0 and far.blue > 0, label)
			"stop": check(near.red > 0 and near.white > 0 and near.yellow == 0, label)
			"yield": check(near.red > 0 and near.white > 0, label)
			"children": check(near.yellow > 0 and near.red > 0 and near.black > 0 and near.white > 0 and far.yellow > 0, label)
			_:
				var digits := tints(set[kind + "_digits"], set.materials)
				check(near.red > 0 and near.white > 0 and digits.black > 0 and digits.red + digits.white == 0 and far.red > 0 and far.white > 0, label + " digits %s" % digits)
				speeds.append(surface(set[kind + "_digits"], set.materials, "sign"))
	for a in speeds.size():
		for b in range(a + 1, speeds.size()): check(speeds[a] != speeds[b], "speed signs %d and %d print different digits" % [a, b])
	# Field colours under points of the printed front (symbols excluded).
	var expect := [["speed_60", Vector2(0.0, 0.905 + 0.115), "red"], ["speed_60", Vector2(0.0, 0.905 - 0.075), "white"],
		["stop", Vector2(0.0, 0.88), "red"], ["stop", Vector2(0.0, 0.9 + 0.08), "white"], ["yield", Vector2(0.0, 0.88), "white"],
		["yield", Vector2(0.0, 0.9 + 0.07), "red"], ["children", Vector2(0.0, 0.8636 + 0.06), "yellow"], ["children", Vector2(0.0, 0.8636 - 0.07), "red"],
		["children", Vector2(0.013, 0.733), "white"], ["crossing", Vector2(0.09, 0.905 + 0.09), "blue"], ["crossing", Vector2(0.0, 0.905 + 0.105), "white"]]
	for item: Array in expect:
		var found := colour_at(set[SIGNS.parts(item[0]).near[0]], set.materials, item[1], SIGNS.FRONT)
		check(name_of(found) == item[2], "%s at %s is %s: %s" % [item[0], item[1], item[2], found])
	var field := colour_at(set.guide_near, set.materials, Vector2(1.67 + 0.45, 2.08 - 0.2), SIGNS.BOARD_FRONT)
	check(name_of(field) == "blue", "the guide field is blue: %s" % field)
	var border := colour_at(set.guide_near, set.materials, Vector2(1.67, 2.08 + 0.3 - 0.021), SIGNS.BOARD_FRONT)
	check(name_of(border) == "white", "the guide board has a white border: %s" % border)
	var board := tints(set.guide_near, set.materials)
	var symbols := tints(set.guide_board_0, set.materials)
	check(board.blue > 0 and board.white > 0 and symbols.white > 0 and symbols.blue > 0, "the guide board is blue with white symbols and a shield: %s %s" % [board, symbols])

## Ranges and shadows per tier, live (street furniture policy for solid kinds).
func policy() -> void:
	for level in 3:
		var value := QUALITY.profile(level)
		var near := float(value.furniture_near) + QUALITY.GROUP_REACH if level > 0 else 0.0
		for kind in ["speed_50", "guide"]:
			for lod in SIGNS.LODS:
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

static func objects() -> Array:
	return [{"id": "speed-a", "asset_id": "builtin:sign_speed_50", "position": [300, 0, 400], "quarter_turns": 1},
		{"id": "speed-b", "asset_id": "builtin:sign_speed_50", "position": [500, 0, 400], "quarter_turns": 3, "yaw_offset_mdeg": 20000},
		{"id": "stop-a", "asset_id": "builtin:sign_stop", "position": [-500, 0, 300], "quarter_turns": 0},
		{"id": "speed-far", "asset_id": "builtin:sign_speed_50", "position": [9000, 0, 300], "quarter_turns": 2},
		{"id": "guide-a", "asset_id": "builtin:guide_sign", "position": [1200, 0, -600], "quarter_turns": 0}]

## A fake renderer job: groups per kind, variant and 32 m square; attached
## nodes carry the range metadata and object ids; low draws the far mesh only;
## the lease retires with the nodes.
func planned(set: Dictionary) -> void:
	for level in [0, 1]:
		QUALITY.current = QUALITY.profile(level)
		var parent := Node3D.new()
		root.add_child(parent)
		var lease := Lease.new()
		var job := {"root": parent, "road_signs": set, "lease": lease}
		for object: Dictionary in objects():
			check(SIGNS.ready(job) == 1, "a job holding the set is ready")
			SIGNS.plan(job, object)
		var keys: Array = job.road_sign_plan.keys()
		keys.sort()
		var guide := "guide/%d/0,0" % SIGNS.variant(objects()[4], 4)
		check(keys == [guide, "speed_50/0/0,-1", "speed_50/0/2,-1", "stop/0/-1,-1"], "groups per kind, variant and 32 m square: %s" % [keys])
		for key: String in job.road_sign_plan:
			for part: PackedFloat32Array in job.road_sign_plan[key].parts: check(part.size() == 12, key + " instances carry a transform only")
		while not job.road_sign_plan.is_empty(): SIGNS.attach_group(job)
		var counts := {}
		for node: MultiMeshInstance3D in parent.find_children("RoadSigns_*", "MultiMeshInstance3D", true, false):
			var key := "%s_%s" % [node.get_meta("mapkit_furniture_kind"), node.get_meta("mapkit_furniture_lod")]
			counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
			check(set.values().has(node.multimesh.mesh), "nodes borrow the shared meshes")
			check(node.get_meta("mapkit_static_detail", false) and not node.get_meta("mapkit_decoration", true), "nodes are solid static details")
			var ids: PackedStringArray = node.get_meta("mapkit_object_ids", PackedStringArray())
			check(ids.size() == node.multimesh.instance_count and Array(ids).all(func(id): return id in ["speed-a", "speed-b", "stop-a", "speed-far", "guide-a"]), "nodes list their instances' object ids: %s" % ids)
			if level == 0: check(node.visible == (node.get_meta("mapkit_furniture_lod") == "far"), "low draws far meshes only")
		check(counts == {"speed_50_near": 6, "speed_50_far": 3, "stop_near": 1, "stop_far": 1, "guide_near": 2, "guide_far": 1}, "instances per part near and far: %s" % counts)
		var guide_node: MultiMeshInstance3D = parent.find_children("RoadSigns_guide_board_*", "MultiMeshInstance3D", true, false)[0]
		check(guide_node.multimesh.mesh == set["guide_board_%d" % SIGNS.variant(objects()[4], 4)], "the guide board layout follows the object identity")
		lease.seal()
		parent.queue_free()
		job.clear()
		for _frame in 4: await process_frame
		check(lease.retired(), "tier %d: the lease retires with the sign nodes" % level)
	QUALITY.current = {}

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

## Ground quad, the owner cell's signs (their proxy boxes) and a neighbour
## sign's clipped post face (no object).
func fixture(prepared: bool) -> Dictionary:
	var vertices := PackedVector3Array([Vector3(-20, 0, 20), Vector3(20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, 20), Vector3(20, 0, -20), Vector3(-20, 0, -20)])
	var ids := ["ground", "ground"]
	var materials := {}
	for object: Dictionary in objects():
		var first := vertices.size()
		var p: Array = object.position
		var guide: bool = object.asset_id == "builtin:guide_sign"
		proxy_box(vertices, Vector3(p[0], 125 if guide else 52, -p[2]), Vector3(10, 250, 10) if guide else Vector3(6, 104, 6))
		for i in (vertices.size() - first) / 3: ids.append(object.id)
		materials[object.id] = object.asset_id
	var neighbour := vertices.size()
	proxy_box(vertices, Vector3(1998, 52, -400), Vector3(4, 104, 6))
	for i in (vertices.size() - neighbour) / 3: ids.append("sign-n")
	materials["sign-n"] = "builtin:sign_yield"
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
	var missing := SIGNS.KINDS.keys().filter(func(id: String) -> bool: return id not in RENDER.BUILTINS or id not in RENDER.PLAN.DRAWN_OVER)
	if not missing.is_empty():
		check(false, "renderer integration missing: %s" % [missing])
		return
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	var hidden := RENDER.PLAN.hidden_objects(fixture(false))
	check(["speed-a", "speed-b", "stop-a", "speed-far", "guide-a", "sign-n"].all(func(id): return hidden.has(id)) and not hidden.has("ground"), "owner and neighbour sign faces are hidden: %s" % [hidden.keys()])
	var without := fixture(false)
	without.triangles = without.triangles.slice(0, 2)
	without.scene_vertices = without.scene_vertices.slice(0, 6)
	check(RENDER.PLAN.estimate(fixture(false), 0) == RENDER.PLAN.estimate(without, 0), "hidden proxy faces add no display reservation")
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
			check(drawn == 6, label + " draws no proxy faces of signs: %d vertices" % drawn)
			var counts := {}
			var made: Dictionary = cache.mesh_set(SIGNS.KEY)
			for node: MultiMeshInstance3D in parent.find_children("RoadSigns_*", "MultiMeshInstance3D", true, false):
				var key := "%s_%s" % [node.get_meta("mapkit_furniture_kind"), node.get_meta("mapkit_furniture_lod")]
				counts[key] = int(counts.get(key, 0)) + node.multimesh.instance_count
				check(made.values().has(node.multimesh.mesh), label + " borrows the shared mesh")
			check(counts == {"speed_50_near": 6, "speed_50_far": 3, "stop_near": 1, "stop_far": 1, "guide_near": 2, "guide_far": 1}, label + " instances per part near and far: %s" % counts)
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), label + " cell lease retires with its sign nodes")
	check(cache.bytes() >= SIGNS.SHARED_BYTES, "one shared road sign set stays cached")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "the shared road sign set retires after its last cell")
	var preview := Node3D.new()
	root.add_child(preview)
	var standalone := RENDER.attach(fixture(false), preview)
	check(standalone.find_children("RoadSigns_*", "MultiMeshInstance3D", true, false).size() == 11, "standalone previews draw signs from the process-lifetime set")
	preview.free()
	# A refused shared set fails the cell as it begins, before any upload.
	var denied := CACHE.new(func(bytes): return null if bytes == SIGNS.SHARED_BYTES else Lease.new(bytes))
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture(true)
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET" and parent.get_child_count() == 0 and not job.claims.has(SIGNS.KEY), "a refused road sign set fails the cell before any upload: %s" % job.get("error", {}))
	while not RENDER.advance(job): pass
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Cache-built materials: printed faces glow through the street-lamp role
## with their tints, paint reads weather. Rendered runs compare a speed sign's
## red ring and white disc and the guide board by day and night in an unlit
## scene (MAPKIT_ROAD_SIGN_CAPTURE optionally writes the frames).
func lights() -> void:
	var context := ENVIRONMENT.new()
	var state := {}
	while not SIGNS.build(state, null, context): pass
	var made: Dictionary = state.result
	var printed: ShaderMaterial = made.materials.sign
	var paint: ShaderMaterial = made.materials.paint
	check(printed.get_shader_parameter("lighting_role") == 2 and printed.get_shader_parameter("vertex_tinted"), "printed faces glow at night through the lamp role")
	check(paint.get_shader_parameter("lighting_role") == 0 and paint.get_shader_parameter("vertex_tinted") and paint.get_meta("mapkit_surface", false), "paint reads vertex tints and weather")
	if DisplayServer.get_name() == "headless":
		print("road_sign_validator: headless, sign pixels skipped")
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
	for part: Array in [["speed_near", Vector3.ZERO], ["speed_60_digits", Vector3.ZERO], ["guide_near", Vector3(4.0, 0.0, 0.0)], ["guide_board_0", Vector3(4.0, 0.0, 0.0)]]:
		var shown := MeshInstance3D.new()
		shown.mesh = made[part[0]]
		shown.position = part[1]
		scene.add_child(shown)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	scene.add_child(camera)
	camera.current = true
	var samples := {}
	# The red ring (between rim and disc), the white disc beside the digits
	# and the guide board's blue field below its left branch.
	var targets := [["ring", Vector3(0.0, 0.905 + 0.115, -1.0), 0.006], ["disc", Vector3(0.0, 0.905 - 0.075, -1.0), 0.006],
		["field", Vector3(4.0 + 1.67 + 0.45, 2.08 - 0.2, -1.0), 0.01]]
	for target: Array in targets:
		camera.size = target[2]
		camera.position = target[1]
		camera.look_at(target[1] + Vector3.BACK, Vector3.UP)
		for sun in [-0.2, 0.8]:
			context.update(72000.0 if sun < 0.0 else 43200.0, 0.0, 0.0, sun, 1080)
			for frame in 5: await process_frame
			await RenderingServer.frame_post_draw
			var picture := root.get_texture().get_image()
			samples["%s_%s" % [target[0], "night" if sun < 0.0 else "day"]] = picture.get_pixel(picture.get_width() / 2, picture.get_height() / 2)
	print("ROAD_SIGN_GLOW ", samples)
	var peak := func(color: Color) -> float: return maxf(color.r, maxf(color.g, color.b))
	check(samples.ring_night.r > 0.3 and samples.ring_night.r > 3.0 * samples.ring_night.g, "the red ring glows red at night: %s" % samples.ring_night)
	check(samples.disc_night.r > 0.4 and samples.disc_night.g > 0.4 and samples.disc_night.b > 0.4 and peak.call(samples.disc_night) < 0.95, "the white disc glows without clipping: %s" % samples.disc_night)
	check(samples.field_night.b > 0.1 and samples.field_night.b > 2.0 * samples.field_night.r, "the guide field glows blue at night: %s" % samples.field_night)
	for key in ["ring_day", "disc_day", "field_day"]:
		check(peak.call(samples[key]) < 0.1, "%s is dark in an unlit scene: %s" % [key, samples[key]])
	scene.queue_free()
	for frame in 3: await process_frame
