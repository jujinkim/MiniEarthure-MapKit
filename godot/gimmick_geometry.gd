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

static func resolved(g: Dictionary) -> Dictionary:
	if not g.has("track") or g.has("track_mesh"): return g
	var bridge = ClassDB.instantiate("MapKitBridge")
	var result: Dictionary = JSON.parse_string(bridge.resolve_gimmick(JSON.stringify(g)))
	return result.data if result.get("ok",false) else {}

static func track_point(v: Array) -> Vector3:
	return Vector3(v[0],v[1],-v[2])*0.0001

static func triangles(faces: Array) -> PackedVector3Array:
	var result := PackedVector3Array()
	for face: Array in faces:
		for v: Array in face: result.append(track_point(v))
	return result

static func visual(g: Dictionary) -> Node3D:
	g = resolved(g)
	var root := Node3D.new()
	if g.is_empty(): return root
	var material := StandardMaterial3D.new()
	material.albedo_color = Color8(g.color[0], g.color[1], g.color[2], g.color[3])
	material.roughness = 0.72
	if g.motion.kind in ["boost", "launch", "target_speed", "jump_height", "air_ring"]:
		material.emission_enabled = true
		material.emission = material.albedo_color * 0.35
	if g.has("track_mesh"):
		for role: String in ["inner","shell"]:
			var tool := SurfaceTool.new()
			tool.begin(Mesh.PRIMITIVE_TRIANGLES)
			var vertices := triangles(g.track_mesh[role])
			for i in range(0,vertices.size(),3):
				tool.set_normal((vertices[i+2]-vertices[i]).cross(vertices[i+1]-vertices[i]).normalized())
				for j in 3: tool.add_vertex(vertices[i+j])
			var mesh := MeshInstance3D.new()
			mesh.mesh = tool.commit()
			mesh.material_override = material
			mesh.set_meta("curved_driving_surface",role == "inner")
			root.add_child(mesh)
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
	if g.motion.kind in ["target_speed","jump_height","air_ring"]:
		# Local +Z is the declared direction; +Y is the launch normal.
		var arrow := MeshInstance3D.new()
		var top := 0.0
		for part: Dictionary in g.parts:
			for v: Array in part.vertices: top = maxf(top,float(v[1])*0.01)
		var tool := SurfaceTool.new()
		tool.begin(Mesh.PRIMITIVE_TRIANGLES)
		tool.set_normal(Vector3.UP)
		for point_value: Vector3 in [Vector3(-0.35,0,0.25),Vector3(0,0,-0.55),Vector3(0.35,0,0.25)]: tool.add_vertex(point_value)
		arrow.mesh = tool.commit()
		arrow.position = Vector3(0,top+0.012,0)
		var white := StandardMaterial3D.new()
		white.albedo_color = Color.WHITE
		white.emission_enabled = true
		white.emission = Color(0.4,0.4,0.4)
		arrow.material_override = white
		if g.motion.kind == "air_ring": arrow.position.y = float(g.effect.ring_radius_cm)*0.01+0.15
		root.add_child(arrow)
	return root
