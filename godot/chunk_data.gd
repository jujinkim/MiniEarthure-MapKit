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


static func scene_vertex(chunk: Dictionary, triangle: int, vertex: int, scale: float = 0.125) -> Vector3:
	if chunk.has("packed_version"):
		var values: PackedInt64Array = chunk.vertices_cm
		var offset := triangle * 9 + vertex * 3
		return Vector3(float(values[offset]), float(values[offset + 1]), -float(values[offset + 2])) * (scale / 100.0)
	var values: Array = chunk.triangles[triangle].vertices[vertex]
	return Vector3(float(values[0]), float(values[1]), -float(values[2])) * (scale / 100.0)
