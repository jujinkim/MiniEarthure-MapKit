extends RefCounted
## Public, view-only checkpoint preview. No game physics or completion authority.
static func create(checkpoints: Array) -> Node3D:
	var root := Node3D.new()
	var material := StandardMaterial3D.new()
	material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = Color(0.05,0.85,1.0,0.25)
	for cp: Dictionary in checkpoints.slice(0,64):
		var marker := MeshInstance3D.new()
		var mesh := SphereMesh.new()
		mesh.radius = float(cp.radius_cm)*0.01
		mesh.height = mesh.radius if cp.shape == "hemisphere" else mesh.radius*2
		mesh.is_hemisphere = cp.shape == "hemisphere"
		mesh.radial_segments = 24
		mesh.rings = 12
		marker.mesh = mesh
		marker.material_override = material
		marker.position = Vector3(cp.position_cm[0],cp.position_cm[1],-cp.position_cm[2])*0.01
		root.add_child(marker)
	return root
