extends RefCounted
## Pure worker-side planning; no display resources or game policy.
const DATA := preload("./chunk_data.gd")
const TRIANGLES_PER_BATCH := 512
const SPATIAL_METRES := 32.0
## Grass ground of one cell for the camera-local grass field (chunk_renderer,
## grass_scatter.gd, grass_field.gd; the field's own fixed memory is one
## shared claim per resource cache). A cell with grass ground keeps its grass
## batch arrays (36 B per triangle, shared, no copy) and their 8 m block index
## (up to three 4 B refs per triangle) while it is registered: 48 B per grass
## triangle plus GRASS_CELL_BYTES for the index rows, the large-triangle refs
## and the record. The worker estimate has no display tier: every estimate
## holds it, and a cell registering no grass (low, polar ground) returns it at
## attachment when its prepared batches count the grass triangles
## (batch_grass_triangles).
const GRASS_BLOCK_METRES := 8.0
const GRASS_TRIANGLE_BYTES := 48
const GRASS_CELL_BYTES := 32768
## City trees and shrubs need no extra allowance: each placement adds 64 B to
## the body, cards and far instance buffers (and a transient copy) and at most
## one group of three nodes and MultiMeshes (< 24 KiB), and a wide zone tree
## its presentation `city_tree_radii` entry (< 1 KiB), inside its 65,536 B
## object share.

## Streetlights and benches need no extra allowance either: each placement
## adds 48 B to its group's near and far instance buffers (and a transient
## copy), its object id to the group's shared id list (< 1 KiB) and at most
## one group (16 m square) of two nodes and MultiMeshes (< 16 KiB) inside its
## object share; the meshes are one shared set per resource cache
## (street_furniture_meshes.SHARED_BYTES).
## Traffic signals and bus shelters likewise: 48 B per placement in each of
## their near and far buffers (a shelter also its glass buffer), its id in
## the group's id list, at most one group of up to three nodes and
## MultiMeshes (< 32 KiB) per kind and 32 m square inside the object share;
## one shared set per resource cache (street_signal_meshes.SHARED_BYTES).
## Subway entrances and station signs likewise: 48 B per placement in each of
## their near and far buffers, its id in the group's id list, at most one
## group of two nodes and MultiMeshes (< 16 KiB) per kind and 32 m square
## inside the object share; one shared set per resource cache
## (transit_meshes.SHARED_BYTES).
## Road signs and guide signs likewise: 48 B per placement in each of its
## group's instance buffers (speed and guide signs: two near parts and a far
## one), its id in the group's id list, at most one group of up to three
## nodes and MultiMeshes (< 24 KiB) per kind, guide layout and 32 m square
## inside the object share; one shared set per resource cache
## (road_sign_meshes.SHARED_BYTES). A horizontal signal also adds 48 B to its
## name-board group (one node and MultiMesh per layout and square).
## Street trees and flower beds likewise: 64 B per instance in each of its
## group's body, cards and far buffers (12 transform and 4 custom floats; a
## planter or median bed is two instances, rim and planting, a median bed
## with a pine three), its id in the group's id list, at most one group of up
## to three nodes and MultiMeshes (< 24 KiB) per mesh, variant and 32 m
## square inside the object share; one shared set per resource cache
## (street_planting_meshes.SHARED_BYTES).

## Facade and rooftop fixtures (facade_fixture_meshes.gd; placed natively
## into the presentation's `facade_fixtures`, at most 1,024 per cell in at
## most 16 squares of 32 m, eight kinds): each fixture holds 64 B in its
## group's near and far instance buffers (12 transform and 4 custom floats),
## a transient copy and its 40 B packed entry, FIXTURE_BYTES in all; each
## group (one kind in one square) two nodes and MultiMeshes,
## FIXTURE_GROUP_BYTES. The meshes and the material are one shared set per
## resource cache (facade_fixture_meshes.SHARED_BYTES). Before generation a
## cell with building prisms reserves the cap (FIXTURE_BOUND_BYTES: 2.4 MiB).
const FIXTURE_BYTES := 384
const FIXTURE_GROUP_BYTES := 16384
const FIXTURE_BOUND_BYTES := 1024 * FIXTURE_BYTES + 16 * 8 * FIXTURE_GROUP_BYTES
static func fixture_bytes(chunk: Dictionary) -> int:
	var list: Variant = chunk.get("presentation", {}).get("facade_fixtures", PackedFloat32Array())
	if not list is PackedFloat32Array or list.is_empty(): return 0
	var groups := {}
	for i in range(0, list.size() - 9, 10): groups[Vector3i(floori(list[i] / 32.0), floori(list[i + 2] / 32.0), int(list[i + 4]))] = true
	return list.size() / 10 * FIXTURE_BYTES + groups.size() * FIXTURE_GROUP_BYTES

## Object ids whose generated proxy faces are not drawn: the presentation's
## hidden proxies and every builtin streetlight, which the renderer draws as
## a street_furniture_meshes.gd lamp inside its unchanged collision boxes, and
## every traffic signal and bus shelter, drawn as street_signal_meshes.gd
## signals and shelters standing where their boxes are, and every subway
## entrance and station sign, drawn as transit_meshes.gd entrances and pylons
## inside theirs, and every road sign and guide sign, drawn as
## road_sign_meshes.gd signs on their posts and poles, and every street tree
## and flower bed, drawn as street_planting_meshes.gd trunks in their tree pits
## and planter rims round their boxes (each owner cell lists
## the object; a neighbour cell holding clipped faces of it knows it from the
## proxy materials). Collision,
## occluders and lamp bindings never read these faces.
const DRAWN_OVER := ["builtin:streetlight", "builtin:signal_h", "builtin:signal_v", "builtin:bus_shelter", "builtin:subway_entrance", "builtin:station_sign",
	"builtin:sign_speed_30", "builtin:sign_speed_40", "builtin:sign_speed_50", "builtin:sign_speed_60", "builtin:sign_speed_80",
	"builtin:sign_speed_100", "builtin:sign_crossing", "builtin:sign_stop", "builtin:sign_yield", "builtin:sign_children", "builtin:guide_sign",
	"builtin:street_tree_ginkgo", "builtin:street_tree_zelkova", "builtin:street_tree_plane", "builtin:street_tree_cherry", "builtin:street_tree_pine",
	"builtin:planter", "builtin:planter_square", "builtin:median_bed", "builtin:median_bed_pine"]
static func hidden_objects(chunk: Dictionary) -> Dictionary:
	var result := {}
	var presentation: Dictionary = chunk.get("presentation", {})
	for id: Variant in presentation.get("hidden_proxies", PackedStringArray()): result[str(id)] = true
	var materials: Dictionary = presentation.get("proxy_materials", {})
	for id: Variant in materials:
		if str(materials[id]) in DRAWN_OVER: result[str(id)] = true
	for object: Dictionary in chunk.get("objects", []):
		if str(object.get("asset_id", "")) in DRAWN_OVER: result[str(object.id)] = true
	return result

## Reservation of a cell's grass ground for the grass field: `triangles` are
## its visible grass-material triangles (estimate counts them).
static func grass_bytes(triangles: int) -> int:
	return 0 if triangles <= 0 else GRASS_CELL_BYTES + triangles * GRASS_TRIANGLE_BYTES

## Grass triangles of a chunk's prepared batches (prepare_batches: the same
## visible grass-material triangles estimate counts), 0 without them.
static func batch_grass_triangles(chunk: Dictionary) -> int:
	var total := 0
	for batch: Dictionary in chunk.get("render_batches", []):
		if str(batch.get("key", "")) == "grass": total += (batch.vertices as PackedVector3Array).size() / 3
	return total

static func shared_bytes(chunk: Dictionary) -> int:
	var total := 0
	for asset: Dictionary in chunk.get("presentation", {}).get("assets", {}).values():
		if not asset.has("content_hash") or int(asset.get("memory_bytes", 0)) <= 0: return 0
		total += int(asset.memory_bytes)
	return total

## Admission bound before generation. A material can change on every triangle.
static func upper_bound(cost: Dictionary) -> int:
	return 131072 + (FIXTURE_BOUND_BYTES if int(cost.get("building_prisms", 0)) > 0 else 0) + grass_bytes(int(cost.triangles)) + int(cost.get("gimmick_bytes", 0))*16 + int(cost.get("water_bytes", 0)) + int(cost.triangles) * (768 + 12288) + int(cost.objects) * 65536 + int(cost.get("presentation_bytes", 0))

## Worker-only opaque surface grouping. Native source/collision order is untouched.
## Position, normal and UV take 96 bytes per visible triangle. Facade batches swap
## UV, which their world-space materials never sample, for the solid seed (UV2) and
## add the solid top (CUSTOM0): at most 108. Shared `concrete` batches holding
## framed sidewalk faces add UV2 (`sidewalk_arrays`): 120, within the existing
## 128-byte presentation allowance.
static func supports_batches(chunk: Dictionary) -> bool:
	var presentation: Dictionary = chunk.get("presentation", {})
	for id: String in presentation.get("proxy_materials", {}).values():
		if presentation.get("assets", {}).has(id): return false # Image alpha/order stays on the legacy path.
	return chunk.has("scene_vertices")

static func prepare_batches(chunk: Dictionary, cancelled: Callable = Callable()) -> Array:
	if not supports_batches(chunk): return []
	var groups: Dictionary = {}
	var hidden := hidden_objects(chunk)
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
			if facade == 1 and tops.size() == source_uv.size(): custom0.resize(count)
			for index in count:
				var source := int(indices[first + index / 3]) * 3 + index % 3
				vertices[index] = chunk.scene_vertices[source]
				normals[index] = chunk.scene_normals[source]
				if not uv.is_empty(): uv[index] = source_uv[source]
				if not uv2.is_empty(): uv2[index] = seeds[source]
				if not custom0.is_empty(): custom0[index] = tops[source]
			if key.get_slice("|",0) == "concrete":
				var framed := sidewalk_arrays(chunk, indices.slice(first, first + count / 3), uv)
				if not framed.is_empty(): uv = framed[0]; uv2 = framed[1]
			result.append({"key": key.get_slice("|",0), "vertices": vertices, "normals": normals, "uv": uv, "uv2": uv2, "custom0": custom0})
	return result

## Display-only sidewalk frames (mapkit-godot road_style.rs) on the shared
## `concrete` batches: framed triangles take (along, across) as UV and
## (crossing, kind + 8 * section) as UV2; the batch's other triangles keep
## their UV and a zero UV2 (world-aligned paving). Returns [uv, uv2], or an
## empty array when no triangle of `triangles` (ascending) is framed. UV2 adds
## 24 bytes per batched triangle: 120 B, inside the 128-byte presentation
## allowance (`supports_batches`). The presentation's own sidewalk lists (a 4 B
## index and three 16 B frames, about 52 B per framed triangle) fall inside the
## road-style estimate of 256 B per cell triangle (presentation.rs cost).
static func sidewalk_arrays(chunk: Dictionary, triangles: PackedInt32Array, uv: PackedVector2Array) -> Array:
	var presentation: Dictionary = chunk.get("presentation", {})
	var listed: PackedInt32Array = presentation.get("sidewalk_triangles", PackedInt32Array())
	var frames: PackedVector4Array = presentation.get("sidewalk_frames", PackedVector4Array())
	if listed.is_empty() or frames.size() != listed.size() * 3 or uv.size() != triangles.size() * 3: return []
	var framed := uv.duplicate()
	var uv2 := PackedVector2Array()
	uv2.resize(uv.size())
	var found := false
	for t in triangles.size():
		var k := listed.bsearch(triangles[t])
		if k >= listed.size() or listed[k] != triangles[t]: continue
		found = true
		for c in 3:
			var f: Vector4 = frames[k * 3 + c]
			framed[t * 3 + c] = Vector2(f.x, f.y)
			uv2[t * 3 + c] = Vector2(f.z, f.w)
	return [framed, uv2] if found else []

## Facade arrays a display batch carries: 0 UV only, 1 the solid seed (UV2) and
## top (CUSTOM0) for a generated building facade (chunk_renderer.facade_kind > 0):
## commercial storeys fit below the top and every kind draws its parapet coping there
## (on walls that end at it: the top is negated on triangles rising above it); flat
## roof faces carry the solid's roof frame (4000 + the angle) instead of the top.
static func facade_arrays(key: String) -> int:
	var fields := key.split(":")
	if fields.size() != 2 or not fields[0] in ["brick","wood","concrete"] or not fields[1] in ["residential","commercial","industrial","public"]: return 0
	return 1

static func estimate(chunk: Dictionary, asset_bytes: int) -> int:
	if asset_bytes < 0 or asset_bytes > 64 * 1024 * 1024 * 1024: return -1
	if not chunk.get("presentation", {}).get("assets", {}).is_empty() and asset_bytes == 0: return -1
	var batches := 0
	var run := 0
	var previous := ""
	var hidden := hidden_objects(chunk)
	var visible_triangles := 0
	var grass_triangles := 0
	for i in DATA.count(chunk):
		if DATA.object_id(chunk, i) in hidden:
			run = 0
			continue
		var key := material_key(chunk, i)
		visible_triangles += 1
		if key == "grass": grass_triangles += 1
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
	return fixture_bytes(chunk) + grass_bytes(grass_triangles) + grind_bytes + water_bytes + 65536 + (65536 if chunk.get("presentation", {}).get("urban_surfaces", false) else 0) + visible_triangles * 768 + batches * 12288 + chunk.objects.size() * 65536 + asset_bytes + chunk.get("presentation", {}).get("road_styles", {}).size() * 16384


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
