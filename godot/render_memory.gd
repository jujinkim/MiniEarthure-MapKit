extends RefCounted
## Pure worker-side planning; no display resources or game policy.
const DATA := preload("./chunk_data.gd")
const TRIANGLES_PER_BATCH := 512
const SPATIAL_METRES := 32.0
## Park grass tufts of one cell with grass ground (chunk_renderer, grass_scatter.gd):
## per tuft 64 B in the GPU instance buffer plus the same transient CPU copy,
## and at most 64 MultiMesh groups at 8 KiB (node, server instance and buffer
## record). The worker estimate has no display tier, so it charges the high
## cap (4,000) to every cell that can grow grass; chunk_renderer.begin drops
## the charge where the cell builds none (low tier, polar ground).
const GRASS_INSTANCES := 4000
const GRASS_GROUPS := 64
const GRASS_CELL_BYTES := GRASS_INSTANCES * 128 + GRASS_GROUPS * 8192
## City trees and shrubs need no extra allowance: each placement adds 64 B to
## the body, cards and far instance buffers (and a transient copy) and at most
## one group of three nodes and MultiMeshes (< 24 KiB), inside its 65,536 B
## object share.

## Whether a cell can build grass tufts: any grass ground triangle or grass
## material override. A superset of the triangles the renderer scatters on, so
## the estimate and the reservation in begin() agree without a triangle pass.
static func has_grass(chunk: Dictionary) -> bool:
	var presentation: Dictionary = chunk.get("presentation", {})
	if (presentation.get("road_materials", PackedStringArray()) as PackedStringArray).has("grass"): return true
	if presentation.get("proxy_materials", {}).values().has("grass"): return true
	if chunk.has("packed_version"): return chunk.surface_indices.has(DATA.SURFACES.find("grass"))
	for triangle: Dictionary in chunk.get("triangles", []):
		if str(triangle.surface) == "grass": return true
	return false

static func shared_bytes(chunk: Dictionary) -> int:
	var total := 0
	for asset: Dictionary in chunk.get("presentation", {}).get("assets", {}).values():
		if not asset.has("content_hash") or int(asset.get("memory_bytes", 0)) <= 0: return 0
		total += int(asset.memory_bytes)
	return total

## Admission bound before generation. A material can change on every triangle.
static func upper_bound(cost: Dictionary) -> int:
	return 131072 + GRASS_CELL_BYTES + int(cost.get("gimmick_bytes", 0))*16 + int(cost.get("water_bytes", 0)) + int(cost.triangles) * (768 + 12288) + int(cost.objects) * 65536 + int(cost.get("presentation_bytes", 0))

## Worker-only opaque surface grouping. Native source/collision order is untouched.
## Position, normal and UV take 96 bytes per visible triangle. Facade batches swap
## UV, which their world-space materials never sample, for the solid seed (UV2) and,
## commercial only, the solid top (CUSTOM0): at most 108, within the existing
## 128-byte presentation allowance.
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
		# Both halves of a rectangular face share these bounds. An arbitrary first
		# corner split them at bucket boundaries and changed under cyclic winding.
		var a: Vector3 = chunk.scene_vertices[triangle * 3]
		var b: Vector3 = chunk.scene_vertices[triangle * 3 + 1]
		var c: Vector3 = chunk.scene_vertices[triangle * 3 + 2]
		var point := (a.min(b).min(c) + a.max(b).max(c)) * .5
		var key := material_key(chunk, triangle) + "|%d,%d,%d" % [floori(point.x/SPATIAL_METRES),floori(point.y/SPATIAL_METRES),floori(point.z/SPATIAL_METRES)]
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
			var uv2 := PackedVector2Array()
			var custom0 := PackedFloat32Array()
			vertices.resize(count); normals.resize(count)
			var source_uv: PackedVector2Array = chunk.wall_uv
			var seeds: PackedVector2Array = chunk.get("facade_seeds", PackedVector2Array())
			var tops: PackedFloat32Array = chunk.get("facade_tops", PackedFloat32Array())
			var facade: int = facade_arrays(key.get_slice("|",0)) if seeds.size() == source_uv.size() else 0
			if facade == 0: uv.resize(count)
			else: uv2.resize(count)
			if facade == 2 and tops.size() == source_uv.size(): custom0.resize(count)
			for index in count:
				var source := int(indices[first + index / 3]) * 3 + index % 3
				vertices[index] = chunk.scene_vertices[source]
				normals[index] = chunk.scene_normals[source]
				if not uv.is_empty(): uv[index] = source_uv[source]
				if not uv2.is_empty(): uv2[index] = seeds[source]
				if not custom0.is_empty(): custom0[index] = tops[source]
			result.append({"key": key.get_slice("|",0), "vertices": vertices, "normals": normals, "uv": uv, "uv2": uv2, "custom0": custom0})
	return result

## Facade arrays a display batch carries: 0 UV only, 1 the solid seed (UV2) for a
## generated building facade (chunk_renderer.facade_kind > 0), 2 also the solid
## top (CUSTOM0) for commercial facades, the only reader.
static func facade_arrays(key: String) -> int:
	var fields := key.split(":")
	if fields.size() != 2 or not fields[0] in ["brick","wood","concrete"] or not fields[1] in ["residential","commercial","industrial","public"]: return 0
	return 2 if fields[1] == "commercial" else 1

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
	var water_bytes := 0
	for record: Dictionary in preload("./water_query.gd").records(chunk): water_bytes += 32768 + record.surface.size()*1024
	var grind_bytes := 0
	for line: Dictionary in preload("./grind_geometry.gd").records(chunk): grind_bytes += 32768 + line.get("samples",[]).size()*8192
	return (GRASS_CELL_BYTES if has_grass(chunk) else 0) + grind_bytes + water_bytes + 65536 + (65536 if chunk.get("presentation", {}).get("urban_surfaces", false) else 0) + visible_triangles * 768 + batches * 12288 + chunk.objects.size() * 65536 + asset_bytes + chunk.get("presentation", {}).get("road_styles", {}).size() * 16384


static func material_key(chunk: Dictionary, triangle: int) -> String:
	var presentation: Dictionary = chunk.get("presentation", {})
	if presentation.has("track_seed"):
		var object := DATA.object_id(chunk, triangle)
		var track_key := preload("./rc_venue.gd").surface_key(object)
		if not track_key.is_empty(): return track_key
	var road_keys: PackedStringArray = presentation.get("road_materials", PackedStringArray())
	if triangle < road_keys.size() and not road_keys[triangle].is_empty(): return road_keys[triangle]
	var id := str(presentation.get("proxy_materials", {}).get(DATA.object_id(chunk, triangle), ""))
	if not id.is_empty():
		return "asset:" + id if presentation.get("assets", {}).has(id) else id
	return DATA.material_key(chunk, triangle)
