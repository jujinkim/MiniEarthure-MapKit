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
	if (not g.has("track") and g.get("curved_faces",[]).is_empty()) or g.has("track_mesh"): return g
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

static func is_panel(g: Dictionary) -> bool:
	return g.motion.kind in ["boost","launch","target_speed","jump_height"]

static func part_triangles(g: Dictionary, part: Dictionary) -> PackedVector3Array:
	var vertices:=points(part)
	var result:=PackedVector3Array()
	var panel:=is_panel(g)
	var scale:=Vector3(g.scale_per_mille[0],g.scale_per_mille[1],g.scale_per_mille[2])*.001
	for face: Array in part.faces:
		var normal: Vector3=(vertices[face[2]]-vertices[face[0]]).cross(vertices[face[1]]-vertices[face[0]]).normalized()
		if panel and normal.y<.4: continue
		# Inverse-transpose normal, then inverse scale: exactly 0.5mm in world
		# space even on nonuniformly scaled/sloped authoring geometry.
		var offset:=(normal/scale).normalized()/scale*.0005 if panel else Vector3.ZERO
		for index in face: result.append(vertices[index]+offset)
	return result

static func panel_style(g: Dictionary) -> Dictionary:
	if not is_panel(g): return {}
	var low := Vector2(INF, INF)
	var high := Vector2(-INF, -INF)
	for part: Dictionary in g.parts:
		for v: Array in part.vertices:
			var p := Vector2(float(v[0]), -float(v[2])) * 0.01
			low = low.min(p); high = high.max(p)
	return {"rect":Vector4(low.x,low.y,high.x-low.x,high.y-low.y), "jump":g.motion.kind in ["launch","jump_height"]}

static func panel_material(style: Dictionary) -> ShaderMaterial:
	var material := ShaderMaterial.new()
	material.shader = load((new().get_script() as Script).resource_path.get_base_dir() + "/panel_marking.gdshader")
	material.set_shader_parameter("panel_rect", style.rect)
	material.set_shader_parameter("jump_panel", style.jump)
	material.set_shader_parameter("panel_color", Color8(30,220,210) if style.jump else Color8(255,113,35))
	return material

static func is_pipe(g: Dictionary) -> bool:
	return g.get("track", {}).get("kind", "") in ["cylinder", "swept_cylinder"]

## Shared display/authoring material. Stored colours remain authoritative.
static func pipe_material(color: Color) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.82
	material.metallic = 0.65
	material.emission_enabled = false
	return material

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
	var panel := panel_style(g)
	var display_material: Material = pipe_material(material.albedo_color) if is_pipe(g) else panel_material(panel) if not panel.is_empty() else material
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
			mesh.material_override = display_material
			mesh.set_meta("curved_driving_surface",role == "inner")
			root.add_child(mesh)
	for part: Dictionary in g.parts:
		var vertices := part_triangles(g,part)
		var tool := SurfaceTool.new()
		tool.begin(Mesh.PRIMITIVE_TRIANGLES)
		for index in range(0,vertices.size(),3):
			tool.set_normal((vertices[index+2]-vertices[index]).cross(vertices[index+1]-vertices[index]).normalized())
			for i in 3: tool.add_vertex(vertices[index+i])
		var mesh := MeshInstance3D.new()
		mesh.mesh = tool.commit()
		mesh.material_override = display_material
		root.add_child(mesh)
	if g.motion.kind == "air_ring":
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
