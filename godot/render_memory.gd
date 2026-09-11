extends RefCounted
## Pure worker-side planning; no display resources or game policy.
const DATA := preload("./chunk_data.gd")
const TRIANGLES_PER_BATCH := 512

static func estimate(chunk: Dictionary, asset_bytes: int) -> int:
	if asset_bytes < 0 or asset_bytes > 64 * 1024 * 1024 * 1024: return -1
	if not chunk.get("presentation", {}).get("assets", {}).is_empty() and asset_bytes == 0: return -1
	var batches := 0
	var run := 0
	var previous := ""
	var hidden: Variant = chunk.get("presentation", {}).get("hidden_proxies", PackedStringArray())
	for i in DATA.count(chunk):
		if DATA.object_id(chunk, i) in hidden:
			run = 0
			continue
		var key := material_key(chunk, i)
		if run == 0 or run == TRIANGLES_PER_BATCH or key != previous:
			batches += 1
			run = 0
		previous = key
		run += 1
	# SurfaceTool CPU scratch, retained mesh arrays/server copy, node/material
	# overhead, built-in SphereMesh and custom instance anchors. Asset allowance
	# comes from the validated native package, never compressed byte length.
	return 65536 + (65536 if chunk.get("presentation", {}).get("urban_surfaces", false) else 0) + DATA.count(chunk) * 512 + batches * 8192 + chunk.objects.size() * 65536 + asset_bytes


static func material_key(chunk: Dictionary, triangle: int) -> String:
	var presentation: Dictionary = chunk.get("presentation", {})
	var road_keys: PackedStringArray = presentation.get("road_materials", PackedStringArray())
	if triangle < road_keys.size() and not road_keys[triangle].is_empty(): return road_keys[triangle]
	var id := str(presentation.get("proxy_materials", {}).get(DATA.object_id(chunk, triangle), ""))
	if not id.is_empty():
		return "asset:" + id if presentation.get("assets", {}).has(id) else id
	return DATA.material_key(chunk, triangle)

