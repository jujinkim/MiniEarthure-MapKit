extends RefCounted
## Public presentation-only policy. Never serialized into map or race state.
static var current: Dictionary = {}

static func profile(level: int) -> Dictionary:
	level = clampi(level, 0, 2)
	return {"level":level, "scale":[.75,1.0,1.0][level], "texture_size":[128,256,512][level],
		"particles":[256,512,768][level], "shadow_distance":[48.0,96.0,160.0][level],
		"water":level, "lod_bias":[.55,.8,1.0][level], "decoration_distance":[64.0,112.0,192.0][level],
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
	if node is OccluderInstance3D: node.visible = bool(value.occlusion)

static func apply_material(material: Material, value: Dictionary) -> void:
	if not material is ShaderMaterial: return
	if material.get_meta("mapkit_water", false):
		material.shader = preload("./water_surface_high.gdshader") if int(value.water) == 2 else preload("./water_surface.gdshader")
		material.set_shader_parameter("quality_level",int(value.water))

static func tile_bytes(size: int, count: int) -> int:
	# Mipmaps + generation scratch, CPU/GPU copies and retirement overlap.
	return 1048576 + count * size * size * 24

static func prepare_lods(mesh: Mesh) -> Mesh:
	if mesh == null or mesh.get_meta("mapkit_lods",false): return mesh
	var source := ImporterMesh.new()
	for index in mesh.get_surface_count():
		source.add_surface(Mesh.PRIMITIVE_TRIANGLES, mesh.surface_get_arrays(index), [], {}, mesh.surface_get_material(index))
	source.generate_lods(60.0, 0.0, [])
	var result := source.get_mesh()
	result.set_meta("mapkit_lods",true)
	return result

static func occluder(mesh: Mesh, lease: RefCounted) -> OccluderInstance3D:
	# A conservative subset of actual opaque upright faces. Never fill a model
	# AABB: doors, junction cuts, branches and windows must stay open.
	var vertices := PackedVector3Array()
	for surface in mesh.get_surface_count():
		var material := mesh.surface_get_material(surface)
		if material is BaseMaterial3D and material.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED: continue
		var arrays := mesh.surface_get_arrays(surface)
		var points: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
		var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX] if arrays[Mesh.ARRAY_INDEX] != null else PackedInt32Array()
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
	var result := OccluderInstance3D.new()
	result.occluder = shape
	result.visible = bool(active().occlusion)
	if lease != null: lease.track(shape); lease.track(result)
	return result
