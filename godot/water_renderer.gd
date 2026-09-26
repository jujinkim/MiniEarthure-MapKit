extends RefCounted
const SHADER := preload("./water_surface.gdshader")

static func mesh(record: Dictionary, first: int, count: int) -> MeshInstance3D:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	for triangle: Array in record.surface.slice(first, first+count):
		for i in [0,2,1]:
			var v: Array = triangle[i]
			vertices.append(Vector3(v[0],v[1],-v[2])*.01)
			normals.append(Vector3.UP)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_NORMAL] = normals
	var result := MeshInstance3D.new()
	result.name = "Water_"+str(record.body.id)
	var geometry := ArrayMesh.new()
	geometry.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	result.mesh = geometry
	var material := ShaderMaterial.new()
	material.shader = SHADER
	material.set_shader_parameter("flow",Vector2(record.body.flow_cm_s[0],-record.body.flow_cm_s[1])*.01)
	result.material_override = material
	result.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return result
