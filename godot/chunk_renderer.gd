extends RefCounted
## Display only. GameRuntime owns physics registration and boundary admission.

const COLORS := {
	"asphalt": Color("30343b"), "concrete": Color("b7b8b0"),
	"dirt": Color("927456"), "gravel": Color("888477"), "grass": Color("738664"),
}

static func scene_position(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), -float(value[2])) * 0.00125

static func attach(chunk: Dictionary, parent: Node3D) -> Node3D:
	var root := Node3D.new()
	root.name = "MapCell_%s_%s" % [chunk.cell.x, chunk.cell.y]
	parent.add_child(root)
	var groups := {}
	for triangle: Dictionary in chunk.triangles:
		var key := str(triangle.surface)
		if not groups.has(key):
			groups[key] = []
		groups[key].append(triangle)
	for key: String in groups:
		var triangles: Array = groups[key]
		for start in range(0, triangles.size(), 2048):
			var surface := SurfaceTool.new()
			surface.begin(Mesh.PRIMITIVE_TRIANGLES)
			for index in range(start, mini(start + 2048, triangles.size())):
				var vertices: Array = triangles[index].vertices
				# Reversed winding matches Godot's clockwise front faces.
				for vi in [0, 2, 1]:
					var point := scene_position(vertices[vi])
					surface.set_uv(Vector2(point.x, point.z))
					surface.add_vertex(point)
			surface.generate_normals()
			var mesh := MeshInstance3D.new()
			mesh.mesh = surface.commit()
			var material := StandardMaterial3D.new()
			material.albedo_color = COLORS.get(key, Color.GRAY)
			material.roughness = 0.9
			material.cull_mode = BaseMaterial3D.CULL_DISABLED
			mesh.material_override = material
			root.add_child(mesh)
	for object: Dictionary in chunk.objects:
		if str(object.asset_id) != "builtin:tree":
			continue
		var canopy := MeshInstance3D.new()
		var shape := SphereMesh.new()
		shape.radius = 0.23
		shape.height = 0.5
		canopy.mesh = shape
		canopy.position = scene_position(object.position) + Vector3.UP * 0.6
		var leaf := StandardMaterial3D.new()
		leaf.albedo_color = Color("486447")
		canopy.material_override = leaf
		root.add_child(canopy)
	return root
