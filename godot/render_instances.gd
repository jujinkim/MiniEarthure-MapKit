extends RefCounted
## Cell-local, opaque static instancing. Original mesh surfaces and transforms survive.
static func parts(template: Node3D) -> Array:
	var result: Array = []
	var pending: Array = [{"node": template, "transform": Transform3D.IDENTITY}]
	while not pending.is_empty():
		var next: Dictionary = pending.pop_back()
		var node: Node3D = next.node
		var transform: Transform3D = next.transform * node.transform
		if node is MeshInstance3D:
			if node.material_overlay != null: return []
			for i in node.mesh.get_surface_count():
				# MultiMesh cannot carry per-surface instance overrides.
				if node.get_surface_override_material(i) != null: return []
				var material: Material = node.get_active_material(i)
				if material != null and not material.get_meta("mapkit_opaque",false) and (not material is BaseMaterial3D or material.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED): return []
			# One MultiMesh draws all surfaces of this mesh. Appending inside
			# the surface loop duplicates the entire object for every material.
			result.append({"mesh": node.mesh, "material": node.material_override, "transform": transform, "shadow": node.cast_shadow})
		for child: Node3D in node.get_children(): pending.append({"node": child, "transform": transform})
	return result

static func begin(template: Node3D, count: int, parent: Node3D, lease: RefCounted) -> Dictionary:
	var pieces := parts(template)
	# A single opaque object still uses INSTANCE_CUSTOM for its seed. Falling
	# back to an instance uniform exhausts the hardware's shared shader buffer
	# in dense cells; this path keeps the same meshes, materials and transform.
	if pieces.is_empty() or count < 1: return {}
	var groups: Array = []
	for piece: Dictionary in pieces:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.mesh = piece.mesh
		multi.use_custom_data = true
		# Explicit white preserves mesh vertex tint in the compatibility renderer.
		multi.use_colors = true
		multi.instance_count = count
		multi.visible_instance_count = 0
		var node := MultiMeshInstance3D.new()
		node.multimesh = multi
		node.material_override = piece.material
		node.cast_shadow = piece.shadow
		var authored_bounds: AABB=piece.transform * piece.mesh.get_aabb()
		node.set_meta("mapkit_decoration", authored_bounds.size.length() <= 6.0)
		preload("./display_quality.gd").apply_node(node,preload("./display_quality.gd").active())
		parent.add_child(node)
		if lease != null:
			lease.track(multi)
			lease.track(node)
		groups.append({"multi": multi, "node":node, "transform": piece.transform, "bounds":AABB(), "bounded":false})
	return {"groups": groups, "next": 0, "ids": PackedStringArray(), "count": count, "lease":lease}

static func append(group: Dictionary, transform: Transform3D, object_id: String, map_id := "") -> void:
	var index := int(group.next)
	for part: Dictionary in group.groups:
		var pose: Transform3D = transform * part.transform
		part.multi.set_instance_transform(index, pose)
		var bounds: AABB = pose * part.multi.mesh.get_aabb()
		part.bounds = part.bounds.merge(bounds) if part.bounded else bounds
		part.bounded = true
		part.multi.custom_aabb = part.bounds
		if bounds.size.length() > 6.0:
			var occluder := preload("./display_quality.gd").occluder(part.multi.mesh,group.lease)
			if occluder != null:
				occluder.transform = pose
				part.node.add_child(occluder)
		part.multi.set_instance_color(index,Color.WHITE)
		part.multi.set_instance_custom_data(index,Color(float((map_id+"/"+object_id).sha256_text().substr(0,6).hex_to_int())/16777215.0,0,0,1))
		part.multi.visible_instance_count = index + 1
	group.ids.append(object_id)
	group.next = index + 1
