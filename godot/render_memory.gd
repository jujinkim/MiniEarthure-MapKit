extends RefCounted
## Pure worker-side planning; no display resources or game policy.
const DATA := preload("./chunk_data.gd")
const TRIANGLES_PER_BATCH := 512

static func shared_bytes(chunk: Dictionary) -> int:
	var total := 0
	for asset: Dictionary in chunk.get("presentation", {}).get("assets", {}).values():
		if not asset.has("content_hash") or int(asset.get("memory_bytes", 0)) <= 0: return 0
		total += int(asset.memory_bytes)
	return total

## Admission bound before generation. A material can change on every triangle.
static func upper_bound(cost: Dictionary) -> int:
	return 131072 + int(cost.triangles) * (512 + 8192) + int(cost.objects) * 65536 + int(cost.get("presentation_bytes", 0))

## Worker-only opaque surface grouping. Native source/collision order is untouched.
## 96 bytes per visible triangle fits the existing 128-byte presentation allowance.
static func supports_batches(chunk: Dictionary) -> bool:
	var presentation: Dictionary = chunk.get("presentation", {})
	for id: String in presentation.get("proxy_materials", {}).values():
		if presentation.get("assets", {}).has(id): return false # Image alpha/order stays on the legacy path.
	return chunk.has("scene_vertices")

static func prepare_batches(chunk: Dictionary, cancelled: Callable = Callable()) -> Array:
	if not supports_batches(chunk): return []
	var groups: Dictionary = {}
	var hidden: Variant = chunk.get("presentation", {}).get("hidden_proxies", PackedStringArray())
	for triangle in DATA.count(chunk):
		if triangle % TRIANGLES_PER_BATCH == 0 and cancelled.is_valid() and cancelled.call(): return []
		if DATA.object_id(chunk, triangle) in hidden: continue
		var key := material_key(chunk, triangle)
		if not groups.has(key): groups[key] = PackedInt32Array()
		groups[key].append(triangle)
	var result: Array = []
	for key: String in groups:
		var indices: PackedInt32Array = groups[key]
		for first in range(0, indices.size(), TRIANGLES_PER_BATCH):
			if cancelled.is_valid() and cancelled.call(): return []
			var count := mini(TRIANGLES_PER_BATCH, indices.size() - first) * 3
			var vertices := PackedVector3Array()
			var normals := PackedVector3Array()
			var uv := PackedVector2Array()
			vertices.resize(count); normals.resize(count); uv.resize(count)
			var source_uv: PackedVector2Array = chunk.wall_uv if key.begins_with("asset:") else chunk.ground_uv
			for index in count:
				var source := int(indices[first + index / 3]) * 3 + index % 3
				vertices[index] = chunk.scene_vertices[source]
				normals[index] = chunk.scene_normals[source]
				uv[index] = source_uv[source]
			result.append({"key": key, "vertices": vertices, "normals": normals, "uv": uv})
	return result

static func estimate(chunk: Dictionary, asset_bytes: int) -> int:
	if asset_bytes < 0 or asset_bytes > 64 * 1024 * 1024 * 1024: return -1
	if not chunk.get("presentation", {}).get("assets", {}).is_empty() and asset_bytes == 0: return -1
	var batches := 0
	var run := 0
	var previous := ""
	var hidden: Variant = chunk.get("presentation", {}).get("hidden_proxies", PackedStringArray())
	var visible_triangles := 0
	for i in DATA.count(chunk):
		if DATA.object_id(chunk, i) in hidden:
			run = 0
			continue
		var key := material_key(chunk, i)
		visible_triangles += 1
		if run == 0 or run == TRIANGLES_PER_BATCH or key != previous:
			batches += 1
			run = 0
		previous = key
		run += 1
	# SurfaceTool CPU scratch, retained mesh arrays/server copy, node/material
	# overhead, built-in SphereMesh and custom instance anchors. Asset allowance
	# comes from the validated native package, never compressed byte length.
	if chunk.has("render_batches"): batches = chunk.render_batches.size()
	return 65536 + (65536 if chunk.get("presentation", {}).get("urban_surfaces", false) else 0) + visible_triangles * 512 + batches * 8192 + chunk.objects.size() * 65536 + asset_bytes


static func material_key(chunk: Dictionary, triangle: int) -> String:
	var presentation: Dictionary = chunk.get("presentation", {})
	var road_keys: PackedStringArray = presentation.get("road_materials", PackedStringArray())
	if triangle < road_keys.size() and not road_keys[triangle].is_empty(): return road_keys[triangle]
	var id := str(presentation.get("proxy_materials", {}).get(DATA.object_id(chunk, triangle), ""))
	if not id.is_empty():
		return "asset:" + id if presentation.get("assets", {}).has(id) else id
	return DATA.material_key(chunk, triangle)
