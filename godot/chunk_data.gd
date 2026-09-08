extends RefCounted
## Read-only display/collision adapter. Packed integers preserve generated v6 centimetres.
const SURFACES := ["asphalt", "concrete", "dirt", "gravel", "grass"]

static func view(chunk: Dictionary) -> Dictionary:
	if not chunk.has("packed_version"):
		return chunk
	var result := chunk.duplicate()
	result.merge(chunk.geometry.view())
	return result

static func count(chunk: Dictionary) -> int:
	return chunk.surface_indices.size() if chunk.has("packed_version") else chunk.triangles.size()

static func surface(chunk: Dictionary, triangle: int) -> String:
	return SURFACES[int(chunk.surface_indices[triangle])] if chunk.has("packed_version") else str(chunk.triangles[triangle].surface)

static func spawnable(chunk: Dictionary, triangle: int) -> bool:
	return bool(chunk.spawnable[triangle]) if chunk.has("packed_version") else bool(chunk.triangles[triangle].spawnable)

static func object_id(chunk: Dictionary, triangle: int) -> String:
	return str(chunk.object_ids[chunk.object_indices[triangle]]) if chunk.has("packed_version") else str(chunk.triangles[triangle].object_id)

static func material_key(chunk: Dictionary, triangle: int) -> String:
	if chunk.has("packed_version") and chunk.has("building_materials"):
		var index := int(chunk.object_indices[triangle])
		if not str(chunk.building_materials[index]).is_empty():
			return str(chunk.building_materials[index]) + ":" + str(chunk.building_usages[index])
	elif not chunk.get("building_prisms", []).is_empty():
		for prism: Dictionary in chunk.building_prisms:
			if str(prism.object_id) == object_id(chunk, triangle):
				return str(prism.material) + ":" + str(prism.usage)
	return surface(chunk, triangle)

static func building_prism_count(chunk: Dictionary) -> int:
	return chunk.get("building_prism_object_indices", PackedInt32Array()).size() if chunk.has("packed_version") else chunk.get("building_prisms", []).size()

static func prism_count(chunk: Dictionary) -> int:
	var extra: int = chunk.get("asset_convex_ids", PackedStringArray()).size() if chunk.has("packed_version") else chunk.get("asset_convexes", []).size()
	return building_prism_count(chunk) + extra

static func prism_id(chunk: Dictionary, index: int) -> String:
	if index >= building_prism_count(chunk):
		var extra := index - building_prism_count(chunk)
		return str(chunk.asset_convex_ids[extra]) if chunk.has("packed_version") else str(chunk.asset_convexes[extra].object_id)
	return str(chunk.object_ids[chunk.building_prism_object_indices[index]]) if chunk.has("packed_version") else str(chunk.building_prisms[index].object_id)

static func prism_points(chunk: Dictionary, index: int, scale: float) -> PackedVector3Array:
	var result := PackedVector3Array()
	if index >= building_prism_count(chunk):
		var extra := index - building_prism_count(chunk)
		if chunk.has("packed_version"):
			var offsets: PackedInt32Array = chunk.asset_convex_offsets
			var values: PackedInt64Array = chunk.asset_convex_vertices_cm
			for offset in range(offsets[extra], offsets[extra + 1], 3):
				result.append(Vector3(values[offset], values[offset + 1], -values[offset + 2]) * (scale / 100.0))
		else:
			for v: Array in chunk.asset_convexes[extra].shape.vertices:
				result.append(Vector3(v[0], v[1], -v[2]) * (scale / 100.0))
		return result
	for vertex in 6:
		if chunk.has("packed_version"):
			var offset := index * 18 + vertex * 3
			var values: PackedInt64Array = chunk.building_prism_vertices_cm
			result.append(Vector3(values[offset], values[offset + 1], -values[offset + 2]) * (scale / 100.0))
		else:
			var prism: Dictionary = chunk.building_prisms[index]
			var point: Array = prism.footprint[vertex % 3]
			var height := float(prism.bottom_cm if vertex < 3 else prism.top_cm[vertex - 3])
			result.append(Vector3(float(point[0]), height, -float(point[1])) * (scale / 100.0))
	return result


static func scene_vertex(chunk: Dictionary, triangle: int, vertex: int, scale: float = 0.125) -> Vector3:
	if chunk.has("packed_version"):
		var values: PackedInt64Array = chunk.vertices_cm
		var offset := triangle * 9 + vertex * 3
		return Vector3(float(values[offset]), float(values[offset + 1]), -float(values[offset + 2])) * (scale / 100.0)
	var values: Array = chunk.triangles[triangle].vertices[vertex]
	return Vector3(float(values[0]), float(values[1]), -float(values[2])) * (scale / 100.0)
