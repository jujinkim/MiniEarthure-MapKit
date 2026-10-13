extends RefCounted
## Public presentation-only policy. Never serialized into map or race state.
static var current: Dictionary = {}

static func profile(level: int) -> Dictionary:
	level = clampi(level, 0, 2)
	return {"level":level, "scale":[.75,1.0,1.0][level], "texture_size":[128,256,512][level],
		"particles":[256,512,768][level], "shadow_distance":[48.0,96.0,160.0][level],
		"water":level, "sky":level, "material_detail":level, "vegetation_motion":[0.0,.5,1.0][level], "lod_bias":[.55,.8,1.0][level], "decoration_distance":[64.0,112.0,192.0][level],
		"occlusion":level > 0, "light_count":[4,6,8][level]}

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
		if node.get_meta("mapkit_decoration",false):
			node.visibility_range_end = float(value.decoration_distance)
			node.visibility_range_end_margin = 8.0
		if node is MeshInstance3D or node is MultiMeshInstance3D:
			apply_material(node.material_override, value)
			var mesh: Mesh = node.mesh if node is MeshInstance3D else node.multimesh.mesh
			if mesh != null:
				for index in mesh.get_surface_count(): apply_material(mesh.surface_get_material(index),value)
	if node is OccluderInstance3D: node.visible = bool(value.occlusion)

static func apply_material(material: Material, value: Dictionary) -> void:
	if not material is ShaderMaterial: return
	if material.get_meta("mapkit_surface",false):
		material.set_shader_parameter("detail_quality",int(value.material_detail))
		material.set_shader_parameter("wind_strength",float(value.vegetation_motion))
	elif material.get_meta("mapkit_detail",false): material.set_shader_parameter("detail_quality",int(value.material_detail))
	if material.get_meta("mapkit_water", false):
		material.shader = preload("./water_surface_high.gdshader") if int(value.water) == 2 else preload("./water_surface.gdshader")
		material.set_shader_parameter("quality_level",int(value.water))

static func tile_bytes(size: int, count: int) -> int:
	# Mipmaps + generation scratch, CPU/GPU copies and retirement overlap.
	return 1048576 + count * size * size * 24

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

static func _occluder_shape(mesh: Mesh, surface_arrays: Array = [], observer := Callable()) -> ArrayOccluder3D:
	# A conservative subset of actual opaque upright faces. Never fill a model
	# AABB: doors, junction cuts, branches and windows must stay open.
	var vertices := PackedVector3Array()
	for surface in mesh.get_surface_count():
		var material := mesh.surface_get_material(surface)
		if material is BaseMaterial3D and material.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED: continue
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
		for i in range(0,count-2,3):
			var a := points[indices[i] if not indices.is_empty() else i]
			var b := points[indices[i+1] if not indices.is_empty() else i+1]
			var c := points[indices[i+2] if not indices.is_empty() else i+2]
			var normal := (b-a).cross(c-a)
			if normal.length_squared() < 4.0 or absf(normal.normalized().y) > .4: continue
			# Shrink each face a centimetre to avoid false silhouette occlusion.
			var center := (a+b+c)/3.0
			for p in [a,b,c]: vertices.append(p.move_toward(center,.01))
			if vertices.size() >= 192: break
		if vertices.size() >= 192: break
	if vertices.is_empty(): return null
	var indices := PackedInt32Array()
	for i in vertices.size(): indices.append(i)
	var shape := ArrayOccluder3D.new()
	shape.set_arrays(vertices,indices)
	return shape
