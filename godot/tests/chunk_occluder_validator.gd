extends SceneTree
## Generated building batches build their occluders from the uploaded vertex
## array, never from a surface readback, and the faces equal the readback path.
const CHUNKS := preload("../chunk_renderer.gd")
const QUALITY := preload("../display_quality.gd")
var failed := false
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func _initialize() -> void: run.call_deferred()

func wall_vertices(count: int) -> PackedVector3Array:
	# Upright quads of varying size, facing and height band plus flat roof
	# triangles (skipped), enough to fill and churn the 64 held faces.
	var points := PackedVector3Array()
	for index in count:
		var x := float(index % 7) * 3.0
		var z := float(index / 7) * 2.5
		var base := float(index % 4) * 6.0
		var height := 3.0 + float((index * 7) % 5)
		var width := 2.0 + float((index * 5) % 9) * 0.5
		var facing := deg_to_rad(float(index * 37 % 360))
		var a := Vector3(x, base, z)
		var b := a + Vector3(cos(facing), 0, sin(facing)) * width
		points.append_array([a, b, b + Vector3.UP * height, a, b + Vector3.UP * height, a + Vector3.UP * height])
		points.append_array([a + Vector3.UP * height, b + Vector3.UP * height, a + Vector3(0, height, 1)])
	return points

## The selection before the early refusal (reference for identical output).
func reference_shape(points: PackedVector3Array, bounds: AABB) -> PackedVector3Array:
	var faces: Array[Dictionary] = []
	var sectors := {}
	var order := 0
	var stride := maxi(1, ceili(float(points.size()) / 3.0 / 512)) * 3
	for i in range(0, points.size() - 2, stride):
		var a := points[i]
		var b := points[i + 1]
		var c := points[i + 2]
		var normal := (b - a).cross(c - a)
		var area := normal.length_squared()
		if area < 4.0 or absf(normal.normalized().y) > .4: continue
		var center := (a + b + c) / 3.0
		var side := (0 if normal.x < 0.0 else 1) if absf(normal.x) >= absf(normal.z) else (2 if normal.z < 0.0 else 3)
		var band := clampi(floori((center.y - bounds.position.y) / maxf(.01, bounds.size.y) * 4.0), 0, 3)
		var face := {"a": a, "b": b, "c": c, "area": area, "sector": side * 4 + band, "order": order}
		var replace := -1
		if faces.size() >= 64:
			for index in faces.size():
				var held: Dictionary = faces[index]
				if held.sector != face.sector and int(sectors[held.sector]) <= 1: continue
				if replace < 0 or held.area < faces[replace].area: replace = index
			if replace < 0 or (sectors.has(face.sector) and face.area <= faces[replace].area):
				order += 1
				continue
			var old: int = faces[replace].sector
			sectors[old] -= 1
			if sectors[old] == 0: sectors.erase(old)
			faces[replace] = face
		else: faces.append(face)
		sectors[face.sector] = int(sectors.get(face.sector, 0)) + 1
		order += 1
	faces.sort_custom(func(x: Dictionary, y: Dictionary): return x.order < y.order)
	var result := PackedVector3Array()
	for face: Dictionary in faces:
		var center: Vector3 = (face.a + face.b + face.c) / 3.0
		for p: Vector3 in [face.a, face.b, face.c]: result.append(p.move_toward(center, .01))
	return result

func run() -> void:
	for count in [3, 40, 170, 600]:
		var vertices := wall_vertices(count)
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		var readback := ArrayMesh.new()
		readback.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		var expected: ArrayOccluder3D = QUALITY._occluder_shape(readback)
		check(expected != null and expected.vertices == reference_shape(vertices, readback.get_aabb()), "early refusal keeps the reference face selection (%d quads)" % count)
		var prepared := ArrayMesh.new()
		prepared.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		CHUNKS._prepare_batch_occluder(prepared, "concrete:residential", vertices)
		var cached: Dictionary = prepared.get_meta("mapkit_occluder_preparation", {})
		check(cached.has("shape"), "occluding batch prepares its occluder from CPU vertices (%d quads)" % count)
		var instance := QUALITY.occluder(prepared, null, QUALITY.profile(1))
		check(instance != null and instance.occluder == cached.get("shape"), "the node uses the prepared shape without a readback")
		check(expected != null and instance != null and instance.occluder.vertices == expected.vertices and instance.occluder.indices == expected.indices, "prepared faces equal the readback faces (%d quads)" % count)
		if instance != null: instance.free()
		var road := ArrayMesh.new()
		road.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		CHUNKS._prepare_batch_occluder(road, "urban:asphalt", vertices)
		check(not road.has_meta("mapkit_occluder_preparation"), "non-occluding batches prepare nothing")
	print("chunk_occluder_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)
