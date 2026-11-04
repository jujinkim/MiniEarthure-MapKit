extends RefCounted
## Public presentation-only policy. Never serialized into map or race state.
static var current: Dictionary = {}

static func profile(level: int) -> Dictionary:
	level = clampi(level, 0, 2)
	return {"level":level, "scale":[.75,1.0,1.0][level], "texture_size":[128,256,512][level],
		"particles":[256,512,768][level], "shadow_distance":[48.0,96.0,160.0][level],
		"water":level, "sky":level, "material_detail":level, "vegetation_motion":[0.0,.5,1.0][level], "lod_bias":[.55,.8,1.0][level], "decoration_distance":[64.0,112.0,192.0][level],
		"detail_distance":[96.0,128.0,160.0][level],
		"surface_effect_distance":[48.0,96.0,160.0][level], "weather_amount":[.4,.7,1.0][level],
		"occlusion":level > 0, "light_count":[4,6,8][level], "sky_radiance":[32,64,128][level],
		# Park vegetation: near trees and shrubs (body, and leaf cards from
		# medium) within vegetation_near of the camera (shrubs 0.6x), the coarse
		# far mesh beyond; shadows 0 none, 1 opaque crowns, 2 also cards; the
		# reach of the grass field's blades (grass_field.gd, rings at the cache
		# texture tier, see grass_level). No cards or grass on low.
		"vegetation_near":[24.0,32.0,48.0][level], "vegetation_cards":level > 0, "vegetation_shadows":level,
		"grass_distance":[0.0,24.0,40.0][level],
		# Street furniture: the detailed mesh within <kind>_near for
		# streetlights and benches (only as far as their small parts stay
		# resolvable), furniture_near for the other kinds (none on low), the
		# simple mesh beyond.
		"furniture_near":[0.0,32.0,48.0][level], "streetlight_near":[0.0,20.0,32.0][level], "bench_near":[0.0,12.0,20.0][level]}

static func active() -> Dictionary:
	return profile(1) if current.is_empty() else current.duplicate()

static func apply(viewport: Viewport, value: Dictionary) -> void:
	current = value.duplicate()
	# Scaling only the 3D buffer leaves Control/HUD text at its original size.
	viewport.scaling_3d_scale = clampf(float(value.scale), .67, 1.0)
	viewport.use_occlusion_culling = bool(value.occlusion)
	for target: Node in viewport.get_tree().get_nodes_in_group("mapkit_quality_targets"):
		if target.get_viewport() != viewport: continue
		if target.has_method("apply_display_quality"): target.apply_display_quality(value)
		else: apply_tree(target, value)

static func apply_tree(root: Node, value: Dictionary) -> void:
	apply_node(root, value)
	for child: Node in root.get_children(): apply_tree(child, value)

static func apply_node(node: Node, value: Dictionary) -> void:
	if node is GeometryInstance3D:
		node.lod_bias = float(value.lod_bias)
		if node.has_meta("mapkit_vegetation_lod"): apply_vegetation(node, value)
		elif node.has_meta("mapkit_furniture_lod"): apply_furniture(node, value)
		elif node.get_meta("mapkit_decoration",false):
			node.visibility_range_end = float(value.decoration_distance)
			node.visibility_range_end_margin = 8.0
		if node is MeshInstance3D or node is MultiMeshInstance3D:
			apply_material(node.material_override, value)
			var mesh: Mesh = node.mesh if node is MeshInstance3D else node.multimesh.mesh
			if mesh != null:
				for index in mesh.get_surface_count(): apply_material(mesh.surface_get_material(index),value)
	if node is OccluderInstance3D:
		var parent := node.get_parent()
		node.visible = bool(value.occlusion) and (not parent is MeshInstance3D or node_can_occlude(parent))

## Ranges are measured to a group's bounds centre and a group is a 32 m
## square (half diagonal 22.6 m): every range adds GROUP_REACH, so a plant
## within vegetation_near of the camera
## always draws its near mesh. Near and far meshes hand over at the same
## distance with VEGETATION_MARGIN, a dithered crossfade on Forward+ and
## hysteresis elsewhere (Mobile and Compatibility do not fade). Far trees keep
## no end range, like the canopies they replace: every tree of a loaded cell
## keeps a crown; distant silhouettes take over outside detail cells.
const VEGETATION_MARGIN := 6.0
const GROUP_REACH := 23.0
static func apply_vegetation(node: GeometryInstance3D, value: Dictionary) -> void:
	var kind: String = node.get_meta("mapkit_vegetation_kind","")
	var near := float(value.get("vegetation_near",32.0)) * (0.6 if kind == "shrub" else 1.0) + GROUP_REACH
	var shadows := int(value.get("vegetation_shadows",1)) if kind == "tree" else 0
	var fade := range_fade(RenderingServer.get_current_rendering_method())
	var lod := str(node.get_meta("mapkit_vegetation_lod"))
	node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows >= (2 if lod == "cards" else 1) else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	match lod:
		"body", "cards":
			node.visible = lod == "body" or bool(value.get("vegetation_cards",true))
			node.visibility_range_begin = 0.0
			node.visibility_range_end = near
			node.visibility_range_end_margin = VEGETATION_MARGIN
			node.visibility_range_fade_mode = fade
		"far":
			node.visibility_range_begin = near
			node.visibility_range_begin_margin = VEGETATION_MARGIN
			node.visibility_range_fade_mode = fade
			node.visibility_range_end = float(value.decoration_distance) if kind == "shrub" else 0.0
			node.visibility_range_end_margin = 8.0 if kind == "shrub" else 0.0

## Street furniture groups: streetlights and benches (street_furniture_meshes.gd,
## 16 m squares) show the near mesh within <kind>_near + FURNITURE_REACH, other
## kinds (32 m squares) within furniture_near + GROUP_REACH; the far mesh
## beyond (from 0 on low, where no near mesh draws). Far benches end at
## decoration_distance; far streetlights stay drawn, like the proxy faces they
## replace. Streetlights cast shadows from medium (none on low, like park
## trees), near benches on high, other kinds on every tier.
const FURNITURE_REACH := 11.5
## Near/far handover: a dithered crossfade on Forward+, hysteresis on Mobile
## and Compatibility (no fade there).
static func range_fade(method: String) -> GeometryInstance3D.VisibilityRangeFadeMode:
	return GeometryInstance3D.VISIBILITY_RANGE_FADE_SELF if method == "forward_plus" else GeometryInstance3D.VISIBILITY_RANGE_FADE_DISABLED

static func apply_furniture(node: GeometryInstance3D, value: Dictionary) -> void:
	var kind := str(node.get_meta("mapkit_furniture_kind",""))
	var own := kind == "streetlight" or kind == "bench"
	var near := float(value.get(kind + "_near" if own else "furniture_near",32.0))
	var reach := near + (FURNITURE_REACH if own else GROUP_REACH) if near > 0.0 else 0.0
	var bench := kind == "bench"
	var far := str(node.get_meta("mapkit_furniture_lod")) == "far"
	node.visibility_range_fade_mode = range_fade(RenderingServer.get_current_rendering_method())
	var shadows := true
	if kind == "streetlight": shadows = int(value.level) >= 1
	elif bench: shadows = not far and int(value.level) >= 2
	node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	# Bus shelter glass panes (street_signal_meshes.gd): one node per group,
	# drawn at every distance on every tier, casting no shadows.
	if node.get_meta("mapkit_furniture_glass",false):
		node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		node.visible = true
		node.visibility_range_begin = 0.0
		node.visibility_range_begin_margin = 0.0
		node.visibility_range_end = 0.0
		node.visibility_range_end_margin = 0.0
		return
	if far:
		node.visible = true
		node.visibility_range_begin = reach
		node.visibility_range_begin_margin = VEGETATION_MARGIN if reach > 0.0 else 0.0
		node.visibility_range_end = float(value.decoration_distance) if bench else 0.0
		node.visibility_range_end_margin = 8.0 if bench else 0.0
	else:
		node.visible = reach > 0.0
		node.visibility_range_begin = 0.0
		node.visibility_range_end = maxf(reach, 1.0)
		node.visibility_range_end_margin = VEGETATION_MARGIN

## Grass field tier (grass_field.gd: window, rings and memory): the tier its
## cache textures were loaded at (texture_size, which adaptive quality keeps
## at the user's ceiling), 0 on low. A GPU-pressure dip only lowers the live
## rings and grass_distance; cells loaded during it keep their grass. The
## field itself sets the lawn shade under its blades (grass_field_distance).
static func grass_level(value: Dictionary) -> int:
	var level := [128,256,512].find(int(value.get("texture_size",0)))
	return level if level >= 0 else clampi(int(value.get("level",1)),0,2)

static func apply_material(material: Material, value: Dictionary) -> void:
	if not material is ShaderMaterial: return
	if material.has_meta("mapkit_vegetation_shader"):
		material.set_shader_parameter("wind_strength",float(value.vegetation_motion))
		material.set_shader_parameter("cards_hidden",not bool(value.get("vegetation_cards",true)))
		return
	if material.get_meta("mapkit_surface",false):
		material.set_shader_parameter("detail_quality",int(value.material_detail))
		material.set_shader_parameter("wind_strength",float(value.vegetation_motion))
	elif material.get_meta("mapkit_detail",false): material.set_shader_parameter("detail_quality",int(value.material_detail))
	if material.get_meta("mapkit_surface",false) or material.get_meta("mapkit_detail",false):
		material.set_shader_parameter("surface_effect_distance",float(value.surface_effect_distance))
	if material.get_meta("mapkit_water", false):
		material.shader = preload("./water_surface_high.gdshader") if int(value.water) == 2 else preload("./water_surface.gdshader")
		material.set_shader_parameter("quality_level",int(value.water))

## The environment context's tile kinds end with this many facade relief
## layers (environment_materials.FACADE_LAYERS), loaded at most 256 px.
const FACADE_TILE_LAYERS := 5
const FACADE_TILE_MAX := 256

static func tile_bytes(size: int, count: int) -> int:
	# Mipmaps + generation scratch, CPU/GPU copies and retirement overlap,
	# for `count` tile kinds (the last FACADE_TILE_LAYERS are facade layers
	# capped at FACADE_TILE_MAX) and the 1024x640 sign atlas (512x320 with
	# 128px tiles).
	var facade := mini(size,FACADE_TILE_MAX)
	return 1048576 + (count - FACADE_TILE_LAYERS) * size * size * 24 + FACADE_TILE_LAYERS * facade * facade * 24 + (1024 * 640 if size >= 256 else 512 * 320) * 24

## Estimated sky radiance cubemap of a tier from the engine's texture-array
## allocation: six RGBA16F faces with mipmaps for each of eight roughness
## layers (unmeasured; excludes the small downsampled copy).
static func radiance_bytes(size: int) -> int:
	return 6 * 8 * size * size * 8 * 4 / 3

static func prepare_lods(mesh: Mesh, observer := Callable()) -> Mesh:
	var job := begin_lods(mesh)
	while not advance_lods(job,observer): pass
	return job.result

static func begin_lods(mesh: Mesh, retain_occluder_arrays := false) -> Dictionary:
	if mesh == null or mesh.get_surface_count()==0 or mesh.get_meta("mapkit_lods",false):
		return {"stage":"done","result":mesh,"owned":false}
	var job := {"stage":"surface","input":mesh,"source":ImporterMesh.new(),"surface":0,"result":null,"owned":true}
	if retain_occluder_arrays: job.occluder_arrays=[]
	return job

## One surface read/add, LOD generation or output upload per admitted call.
## A single engine operation cannot be preempted; keep its cost observable.
static func advance_lods(job: Dictionary, observer := Callable()) -> bool:
	if job.stage == "done": return true
	var source: ImporterMesh = job.source
	if job.stage == "surface":
		var mesh: Mesh = job.input
		var index: int = job.surface
		var started := Time.get_ticks_usec()
		var arrays := mesh.surface_get_arrays(index)
		observe_mesh(observer,"lod_arrays",started,index)
		# Keep only positions/indices already decoded by the renderer. Raw GLTF
		# importer arrays precede compression and can change geometry/LOD results.
		# Our LOD output uses no vertex compression, so these faces stay exact.
		if job.has("occluder_arrays"):
			job.occluder_arrays.append([arrays[Mesh.ARRAY_VERTEX],arrays[Mesh.ARRAY_INDEX]])
		started = Time.get_ticks_usec()
		source.add_surface(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, mesh.surface_get_material(index))
		observe_mesh(observer,"lod_add_surface",started,index)
		job.surface += 1
		if job.surface == mesh.get_surface_count(): job.stage = "generate"
	elif job.stage == "generate":
		var started := Time.get_ticks_usec()
		source.generate_lods(60.0, 0.0, [])
		observe_mesh(observer,"lod_generate",started)
		job.stage = "get_mesh"
	elif job.stage == "get_mesh":
		var started := Time.get_ticks_usec()
		job.result = source.get_mesh()
		observe_mesh(observer,"lod_get_mesh",started)
		job.result.set_meta("mapkit_lods",true)
		job.erase("input");job.erase("source")
		job.stage = "done"
	return job.stage == "done"

static func observe_mesh(observer: Callable, phase: String, started: int, surface := -1) -> void:
	if observer.is_valid(): observer.call(phase,Time.get_ticks_usec()-started,surface)

static func occluder(mesh: Mesh, lease: RefCounted, quality: Dictionary = {}) -> OccluderInstance3D:
	var prepared: Dictionary = mesh.get_meta("mapkit_occluder_preparation", {})
	var shape: ArrayOccluder3D = prepared.shape if prepared.has("shape") else _occluder_shape(mesh)
	if shape == null: return null
	var result := OccluderInstance3D.new()
	result.occluder = shape
	result.visible = bool((active() if quality.is_empty() else quality).occlusion)
	if lease != null: lease.track(shape); lease.track(result)
	return result

## Only immutable, fully styled shared-template meshes opt in. Mutable preview
## meshes keep deriving their current faces on demand. Cache empty results too.
static func prepare_occluder(mesh: Mesh, lease: RefCounted, surface_arrays: Array = [], observer := Callable()) -> void:
	if mesh.has_meta("mapkit_occluder_preparation"): return
	var shape := _occluder_shape(mesh,surface_arrays,observer)
	mesh.set_meta("mapkit_occluder_preparation", {"shape":shape})
	if shape != null and lease != null: lease.track(shape)

static func opaque_material(material: Material) -> bool:
	if material==null: return true
	if material.get_meta("mapkit_water",false): return false
	if material is BaseMaterial3D: return material.transparency==BaseMaterial3D.TRANSPARENCY_DISABLED
	return bool(material.get_meta("mapkit_opaque",false))

static func node_can_occlude(node: MeshInstance3D) -> bool:
	if not opaque_material(node.material_override): return false
	for surface in node.mesh.get_surface_count():
		if not opaque_material(node.get_surface_override_material(surface)): return false
	return true

## Returns whether the held faces changed.
static func _offer_occluder_face(faces: Array[Dictionary], sectors: Dictionary, face: Dictionary) -> bool:
	var replace := -1
	if faces.size()>=64:
		for index in faces.size():
			var held: Dictionary=faces[index]
			# Preserve at least one face in each represented direction/height band.
			if held.sector!=face.sector and int(sectors[held.sector])<=1: continue
			if replace<0 or held.area<faces[replace].area: replace=index
		if replace<0 or (sectors.has(face.sector) and face.area<=faces[replace].area): return false
		var old: int=faces[replace].sector
		sectors[old]-=1
		if sectors[old]==0: sectors.erase(old)
		faces[replace]=face
	else: faces.append(face)
	sectors[face.sector]=int(sectors.get(face.sector,0))+1
	return true

static func _occluder_shape(mesh: Mesh, surface_arrays: Array = [], observer := Callable()) -> ArrayOccluder3D:
	# A conservative subset of actual opaque upright faces. Never fill a model
	# AABB: doors, junction cuts, branches and windows must stay open.
	var vertices := PackedVector3Array()
	var faces: Array[Dictionary] = []
	var sectors := {}
	var bounds := mesh.get_aabb()
	var surface_count := mesh.get_surface_count()
	var per_surface := maxi(4,512/maxi(1,surface_count))
	var order := 0
	# Smallest held area: once 64 faces are held, a face of an already held
	# sector no larger than every held face is always refused, so it is skipped
	# before the scan (same selection, no per-face 64-entry pass).
	var smallest := INF
	for surface in surface_count:
		var material := mesh.surface_get_material(surface)
		if not opaque_material(material): continue
		if material != null and material.get_meta("mapkit_vegetation",false): continue
		var started := Time.get_ticks_usec()
		var face_arrays: Array
		if surface_arrays.is_empty():
			var arrays := mesh.surface_get_arrays(surface)
			face_arrays=[arrays[Mesh.ARRAY_VERTEX],arrays[Mesh.ARRAY_INDEX]]
		else: face_arrays=surface_arrays[surface]
		observe_mesh(observer,"occluder_arrays" if surface_arrays.is_empty() else "occluder_cached_arrays",started,surface)
		var points: PackedVector3Array = face_arrays[0]
		var indices: PackedInt32Array = face_arrays[1] if face_arrays[1] != null else PackedInt32Array()
		var count := indices.size() if not indices.is_empty() else points.size()
		# Bound candidate work while covering the whole surface instead of its
		# first faces. All selected faces remain a subset of real opaque geometry.
		var stride := maxi(1,ceili(float(count)/3.0/per_surface)) * 3
		for i in range(0,count-2,stride):
			var a := points[indices[i] if not indices.is_empty() else i]
			var b := points[indices[i+1] if not indices.is_empty() else i+1]
			var c := points[indices[i+2] if not indices.is_empty() else i+2]
			var normal := (b-a).cross(c-a)
			var area := normal.length_squared()
			if area < 4.0 or absf(normal.normalized().y) > .4: continue
			var center := (a+b+c)/3.0
			var side := (0 if normal.x<0.0 else 1) if absf(normal.x)>=absf(normal.z) else (2 if normal.z<0.0 else 3)
			var band := clampi(floori((center.y-bounds.position.y)/maxf(.01,bounds.size.y)*4.0),0,3)
			var sector := side*4+band
			if faces.size()>=64 and area<=smallest and sectors.has(sector):
				order+=1
				continue
			if _offer_occluder_face(faces,sectors,{"a":a,"b":b,"c":c,"area":area,"sector":sector,"order":order}):
				if faces.size()<64: smallest=minf(smallest,area)
				else:
					smallest=INF
					for held: Dictionary in faces: smallest=minf(smallest,held.area)
			order+=1
	faces.sort_custom(func(a: Dictionary,b: Dictionary): return a.order<b.order)
	for face: Dictionary in faces:
		var center: Vector3=(face.a+face.b+face.c)/3.0
		# Shrink each face a centimetre to avoid false silhouette occlusion.
		for p: Vector3 in [face.a,face.b,face.c]: vertices.append(p.move_toward(center,.01))
	if vertices.is_empty(): return null
	var indices := PackedInt32Array()
	for i in vertices.size(): indices.append(i)
	var shape := ArrayOccluder3D.new()
	shape.set_arrays(vertices,indices)
	return shape
