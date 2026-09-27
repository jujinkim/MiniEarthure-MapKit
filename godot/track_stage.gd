extends Node3D
## Toy display stage only: no physics bodies, occupancy or spawn metadata.
static func create(bounds: PackedInt64Array) -> Node3D:
	var root := Node3D.new()
	root.name = "ToyTrackStage"
	var floor_mesh := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(float(bounds[2]-bounds[0])*0.01+40.0,2.0,float(bounds[3]-bounds[1])*0.01+40.0)
	floor_mesh.mesh = box
	floor_mesh.position = Vector3(float(bounds[0]+bounds[2])*0.005,-12.0,-float(bounds[1]+bounds[3])*0.005)
	var material := StandardMaterial3D.new()
	material.albedo_color = Color("638b97")
	material.roughness = 1.0
	floor_mesh.material_override = material
	root.add_child(floor_mesh)
	for i in 12:
		var block := MeshInstance3D.new()
		var shape := BoxMesh.new()
		shape.size = Vector3(6,3+float(i%3)*2,6)
		block.mesh = shape
		block.position = floor_mesh.position + Vector3(-box.size.x*0.4+box.size.x*0.8*float(i%6)/5.0,shape.size.y*0.5+1,box.size.z*0.4*(1 if i<6 else -1))
		var paint := StandardMaterial3D.new()
		paint.albedo_color = [Color("eeab52"),Color("73b9d4"),Color("d97076")][i%3]
		block.material_override = paint
		root.add_child(block)
	return root
