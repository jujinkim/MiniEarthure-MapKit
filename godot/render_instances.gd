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
			result.append({"mesh": node.mesh, "material": node.material_override, "transform": transform, "shadow": node.cast_shadow})
		for child: Node3D in node.get_children(): pending.append({"node": child, "transform": transform})
	return result

static func begin(template: Node3D, count: int, parent: Node3D, lease: RefCounted) -> Dictionary:
	var pieces := parts(template)
	if pieces.is_empty() or count < 2: return {}
	var groups: Array = []
	for piece: Dictionary in pieces:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.mesh = piece.mesh
		multi.use_custom_data = true
		multi.instance_count = count
		multi.visible_instance_count = 0
		var node := MultiMeshInstance3D.new()
		node.multimesh = multi
		node.material_override = piece.material
		node.cast_shadow = piece.shadow
		parent.add_child(node)
		if lease != null:
			lease.track(multi)
			lease.track(node)
		groups.append({"multi": multi, "transform": piece.transform})
	return {"groups": groups, "next": 0, "ids": PackedStringArray(), "count": count}

static func append(group: Dictionary, transform: Transform3D, object_id: String, map_id := "") -> void:
	var index := int(group.next)
	for part: Dictionary in group.groups:
		part.multi.set_instance_transform(index, transform * part.transform)
		part.multi.set_instance_custom_data(index,Color(float((map_id+"/"+object_id).sha256_text().substr(0,6).hex_to_int())/16777215.0,0,0,1))
		part.multi.visible_instance_count = index + 1
	group.ids.append(object_id)
	group.next = index + 1
