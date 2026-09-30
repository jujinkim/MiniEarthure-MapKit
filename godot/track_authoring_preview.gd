extends RefCounted
## Draft view of the shared compiled frames; no editor-specific road model.
static func point(v: Array) -> Vector3:
	return Vector3(float(v[0]),float(v[1]),-float(v[2]))*0.01

static func create(document: Dictionary, selected := -1) -> Node3D:
	var root := Node3D.new()
	root.name="TrackAuthoringPreview"
	var bridge: RefCounted=ClassDB.instantiate("MapKitBridge")
	var result: Dictionary=JSON.parse_string(bridge.track_preview(JSON.stringify(document)))
	if not result.ok: return root
	for id: String in result.data.meshes:
		var vertices:=PackedVector3Array()
		for v: Array in result.data.meshes[id]: vertices.append(point(v))
		if vertices.is_empty(): continue
		var arrays:=[]
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX]=vertices
		var mesh:=ArrayMesh.new()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
		var view:=MeshInstance3D.new()
		view.mesh=mesh
		var material:=StandardMaterial3D.new()
		material.albedo_color=Color("f5ce5f") if id=="assembled-road-%d" % selected else Color("b9c9d0") if id.begins_with("assembled-wall") else Color("448fac")
		material.shading_mode=BaseMaterial3D.SHADING_MODE_UNSHADED
		material.cull_mode=BaseMaterial3D.CULL_DISABLED
		view.material_override=material
		root.add_child(view)
	for g: Dictionary in result.data.gimmicks:
		var visual:=preload("./gimmick_geometry.gd").visual(g)
		visual.transform=preload("./gimmick_geometry.gd").pose(g,0)
		root.add_child(visual)
	return root
