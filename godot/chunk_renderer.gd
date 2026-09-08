extends RefCounted
## Display only. Call advance() per admitted batch; the owner controls frame budgets.
const DATA := preload("./chunk_data.gd")
const COLORS := {"asphalt": Color("30343b"), "concrete": Color("b7b8b0"),
	"dirt": Color("927456"), "gravel": Color("888477"), "grass": Color("738664")}
const TRIANGLES_PER_BATCH := 512

static func scene_position(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), -float(value[2])) * 0.00125

static func begin(chunk: Dictionary, parent: Node3D) -> Dictionary:
	var root := Node3D.new()
	root.name = "MapCell_%s_%s" % [chunk.cell.x, chunk.cell.y]
	parent.add_child(root)
	return {"root": root, "chunk": DATA.view(chunk), "triangle": 0, "object": 0, "done": false, "cancelled": false, "materials": {}}

static func advance(job: Dictionary) -> bool:
	if job.done or job.cancelled:
		return true
	if not is_instance_valid(job.root) or job.root.is_queued_for_deletion():
		job.cancelled = true
		job.chunk = {}
		job.materials = {}
		return true
	var chunk: Dictionary = job.chunk
	var offset := int(job.triangle)
	if offset < DATA.count(chunk):
		var key := DATA.material_key(chunk, offset)
		var surface := SurfaceTool.new()
		surface.begin(Mesh.PRIMITIVE_TRIANGLES)
		var end := mini(offset + TRIANGLES_PER_BATCH, DATA.count(chunk))
		while offset < end and DATA.material_key(chunk, offset) == key:
			for index in [0, 2, 1]:
				var point := DATA.scene_vertex(chunk, offset, index)
				surface.set_uv(Vector2(point.x, point.z))
				surface.add_vertex(point)
			offset += 1
		surface.generate_normals()
		var mesh := MeshInstance3D.new()
		mesh.mesh = surface.commit()
		if not job.materials.has(key):
			var material := StandardMaterial3D.new()
			material.albedo_color = material_color(key)
			material.roughness = 0.9
			material.cull_mode = BaseMaterial3D.CULL_DISABLED
			job.materials[key] = material
		mesh.material_override = job.materials[key]
		job.root.add_child(mesh)
		job.triangle = offset
	elif int(job.object) < chunk.objects.size():
		var finish := mini(int(job.object) + 8, chunk.objects.size())
		while int(job.object) < finish:
			var object: Dictionary = chunk.objects[job.object]
			job.object += 1
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
			job.root.add_child(canopy)
	else:
		job.done = true
		job.chunk = {}
		job.materials = {}
	return job.done

static func material_color(key: String) -> Color:
	if not key.contains(":"):
		return COLORS.get(key, Color.GRAY)
	var fields := key.split(":")
	var base: Color = {"brick": Color("b46c50"), "wood": Color("997347"), "concrete": Color("b7b8b0")}.get(fields[0], Color.GRAY)
	var tint: Color = {"residential": Color("f4dfc3"), "commercial": Color("c6dfec"), "industrial": Color("bbbec6"), "public": Color("eee3b2")}.get(fields[1], Color.WHITE)
	return base.lerp(tint, 0.2)

static func cancel(job: Dictionary) -> void:
	job.cancelled = true
	job.chunk = {}
	job.materials = {}
	if is_instance_valid(job.root) and not job.root.is_queued_for_deletion():
		var parent: Node = job.root.get_parent()
		if parent != null:
			parent.remove_child(job.root)
		job.root.queue_free()

static func attach(chunk: Dictionary, parent: Node3D) -> Node3D:
	# Synchronous convenience for existing small editor previews; same renderer source.
	var job := begin(chunk, parent)
	while not advance(job):
		pass
	return job.root
