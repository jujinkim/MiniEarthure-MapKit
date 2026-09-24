extends RefCounted
## Current-v1 declarative geometry. Time is supplied by an authority or preview.
static func point(v: Array) -> Vector3:
	return Vector3(v[0], v[1], -v[2]) * 0.01

static func pose(g: Dictionary, elapsed_ms: int) -> Transform3D:
	var angles := Vector3(g.rotation_mdeg[0], g.rotation_mdeg[1], g.rotation_mdeg[2]) * (PI / 180000.0)
	var basis := Basis.from_euler(Vector3(-angles.x, -angles.y, angles.z))
	var position := point(g.position)
	var m: Dictionary = g.motion
	var phase := TAU * float(posmod(elapsed_ms + int(m.phase_ms), int(m.period_ms))) / float(m.period_ms)
	if m.kind == "translate": position += point(m.delta_cm) * (0.5 - 0.5 * cos(phase))
	elif m.kind == "rotate":
		var axis: Vector3 = [Vector3.LEFT, Vector3.DOWN, Vector3.BACK][int(m.axis)]
		basis = Basis(axis, phase) * basis
	return Transform3D(basis * Basis.from_scale(Vector3(g.scale_per_mille[0], g.scale_per_mille[1], g.scale_per_mille[2]) * 0.001), position)

static func velocity(g: Dictionary, elapsed_ms: int, at: Vector3) -> Vector3:
	var m: Dictionary = g.motion
	var rate := TAU * 1000.0 / float(m.period_ms)
	if m.kind == "translate":
		return point(m.delta_cm) * 0.5 * rate * sin(rate * float(elapsed_ms + int(m.phase_ms)) * 0.001)
	if m.kind == "rotate":
		return ([Vector3.LEFT, Vector3.DOWN, Vector3.BACK][int(m.axis)] * rate).cross(at - point(g.position))
	return Vector3.ZERO

static func points(part: Dictionary) -> PackedVector3Array:
	var result := PackedVector3Array()
	for v: Array in part.vertices: result.append(point(v))
	return result

static func visual(g: Dictionary) -> Node3D:
	var root := Node3D.new()
	var material := StandardMaterial3D.new()
	material.albedo_color = Color8(g.color[0], g.color[1], g.color[2], g.color[3])
	material.roughness = 0.72
	if g.motion.kind in ["boost", "launch"]:
		material.emission_enabled = true
		material.emission = material.albedo_color * 0.35
	for part: Dictionary in g.parts:
		var vertices := points(part)
		var tool := SurfaceTool.new()
		tool.begin(Mesh.PRIMITIVE_TRIANGLES)
		for face: Array in part.faces:
			# Convex faces are outward counterclockwise in map coordinates.
			# point() reflects Z, already producing Godot's clockwise front face.
			tool.set_normal((vertices[face[2]]-vertices[face[0]]).cross(vertices[face[1]]-vertices[face[0]]).normalized())
			for i in [0, 1, 2]: tool.add_vertex(vertices[face[i]])
		var mesh := MeshInstance3D.new()
		mesh.mesh = tool.commit()
		mesh.material_override = material
		root.add_child(mesh)
	return root
