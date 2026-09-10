extends RefCounted
## Display only. Call advance() per admitted batch; the owner controls frame budgets.
const ASSETS := preload("./asset_library.gd")
const DATA := preload("./chunk_data.gd")
const COLORS := {"asphalt": Color("30343b"), "concrete": Color("b7b8b0"),
	"dirt": Color("927456"), "gravel": Color("888477"), "grass": Color("738664")}
const PLAN := preload("./render_memory.gd")
const TRIANGLES_PER_BATCH := PLAN.TRIANGLES_PER_BATCH

static func scene_position(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), -float(value[2])) * 0.01

## The caller owns admission policy. A lease implements track(Object) and seal().
## No game dependency: small standalone editor previews may omit admission.

static func begin(chunk: Dictionary, parent: Node3D, reserve: Callable = Callable(), planned_bytes: int = 0) -> Dictionary:
	var view := DATA.view(chunk)
	var lease: RefCounted
	if reserve.is_valid():
		var bytes := planned_bytes
		if bytes > 0 and bytes <= 64 * 1024 * 1024 * 1024: lease = reserve.call(bytes)
		if lease == null:
			return {"root": null, "chunk": {}, "done": true, "cancelled": true,
				"materials": {}, "asset_materials": {}, "templates": {}, "lease": null,
				"error": {"code": "E_MEMORY_BUDGET", "message": "Display allocation exceeds memory allowance"}}
	var root := Node3D.new()
	if lease != null: lease.track(root)
	root.name = "MapCell_%s_%s" % [chunk.cell.x, chunk.cell.y]
	parent.add_child(root)
	return {"root": root, "chunk": view, "lease": lease, "triangle": 0, "object": 0, "done": false, "cancelled": false, "materials": {}, "asset_materials": {}, "templates": {}, "error": {}}

static func advance(job: Dictionary) -> bool:
	if job.done or job.cancelled:
		return true
	if not is_instance_valid(job.root) or job.root.is_queued_for_deletion():
		job.cancelled = true
		dispose(job)
		return true
	var chunk: Dictionary = job.chunk
	var presentation: Dictionary = chunk.get("presentation", {})
	var sources: Dictionary = presentation.get("assets", {})
	var offset := int(job.triangle)
	if offset < DATA.count(chunk):
		var skipped := 0
		while offset < DATA.count(chunk) and DATA.object_id(chunk, offset) in presentation.get("hidden_proxies", PackedStringArray()) and skipped < TRIANGLES_PER_BATCH:
			offset += 1
			skipped += 1
		if skipped > 0:
			job.triangle = offset
			return false
		var key := display_material_key(chunk, offset)
		var end := mini(offset + TRIANGLES_PER_BATCH, DATA.count(chunk))
		var first := offset
		while offset < end and display_material_key(chunk, offset) == key and not DATA.object_id(chunk, offset) in presentation.get("hidden_proxies", PackedStringArray()):
			offset += 1
		var mesh := MeshInstance3D.new()
		if chunk.has("scene_vertices"):
			var arrays := []
			arrays.resize(Mesh.ARRAY_MAX)
			arrays[Mesh.ARRAY_VERTEX] = chunk.scene_vertices.slice(first * 3, offset * 3)
			arrays[Mesh.ARRAY_NORMAL] = chunk.scene_normals.slice(first * 3, offset * 3)
			arrays[Mesh.ARRAY_TEX_UV] = (chunk.wall_uv if key.begins_with("asset:") else chunk.ground_uv).slice(first * 3, offset * 3)
			var prepared := ArrayMesh.new()
			prepared.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
			mesh.mesh = prepared
		else:
			var surface := SurfaceTool.new()
			surface.begin(Mesh.PRIMITIVE_TRIANGLES)
			for triangle in range(first, offset):
				var normal := (DATA.scene_vertex(chunk, triangle, 1) - DATA.scene_vertex(chunk, triangle, 0)).cross(DATA.scene_vertex(chunk, triangle, 2) - DATA.scene_vertex(chunk, triangle, 0)).abs()
				for index in [0, 2, 1]:
					var point := DATA.scene_vertex(chunk, triangle, index)
					var uv := Vector2(point.x, point.z)
					if key.begins_with("asset:") and normal.y < maxf(normal.x, normal.z):
						uv = Vector2(point.z, point.y) if normal.x > normal.z else Vector2(point.x, point.y)
					surface.set_uv(uv)
					surface.add_vertex(point)
			surface.generate_normals()
			mesh.mesh = surface.commit()
		if not job.materials.has(key):
			var material := ASSETS.material(key.substr(6), sources, job.asset_materials) if key.begins_with("asset:") else StandardMaterial3D.new()
			if material == null:
				mesh.free()
				return fail(job, "E_RENDER_ASSET", "Validated image could not be displayed")
			if not key.begins_with("asset:"):
				material.albedo_color = material_color(key)
				material.roughness = 0.9
				material.cull_mode = BaseMaterial3D.CULL_DISABLED
			job.materials[key] = material
		mesh.material_override = job.materials[key]
		track_resources(job, mesh)
		job.root.add_child(mesh)
		job.triangle = offset
	elif int(job.object) < chunk.objects.size():
		var finish := mini(int(job.object) + 1, chunk.objects.size())
		while int(job.object) < finish:
			var object: Dictionary = chunk.objects[job.object]
			job.object += 1
			var id := str(object.asset_id)
			if not id.begins_with("builtin:"):
				if not sources.has(id): return fail(job, "E_RENDER_ASSET", "Presentation bytes are missing")
				if not str(sources[id].path).ends_with(".glb"): continue # textured proxy faces
				if not job.templates.has(id):
					var template := ASSETS.template(id, sources, job.asset_materials)
					if template == null: return fail(job, "E_RENDER_ASSET", "Validated GLB could not be displayed")
					track_resources(job, template)
					job.templates[id] = template
				var instance: Node3D = job.templates[id].duplicate(0)
				var anchor := Node3D.new()
				anchor.set_meta("mapkit_asset_id", id)
				anchor.set_meta("mapkit_object_id", str(object.id))
				anchor.position = scene_position(object.position)
				# glTF metres: x-right, y-up, z-back; local map y points forward.
				anchor.rotation.y = float(object.quarter_turns) * PI / 2.0
				anchor.scale = Vector3.ONE
				anchor.add_child(instance)
				job.root.add_child(anchor)
				continue
			if id not in ["builtin:tree", "builtin:fence", "builtin:streetlight"]:
				return fail(job, "E_RENDER_ASSET", "Unknown built-in asset")
			if id != "builtin:tree": continue
			var canopy := MeshInstance3D.new()
			var shape := SphereMesh.new()
			shape.radius = 1.84
			shape.height = 4.0
			canopy.mesh = shape
			canopy.position = scene_position(object.position) + Vector3.UP * 4.8
			var leaf := StandardMaterial3D.new()
			leaf.albedo_color = Color("486447")
			canopy.material_override = leaf
			track_resources(job, canopy)
			job.root.add_child(canopy)
	else:
		job.done = true
		dispose(job)
	return job.done

static func display_material_key(chunk: Dictionary, triangle: int) -> String:
	return PLAN.material_key(chunk, triangle)

## Weak references extend the charge when a trusted consumer retains a mesh,
## material or texture after its scene node is destroyed. Templates share these
## resources with duplicate(0) instances; no global renderer cache owns them.
static func track_resources(job: Dictionary, node: Node) -> void:
	if job.get("lease") == null: return
	if node is MeshInstance3D:
		job.lease.track(node.mesh)
		track_material(job, node.material_override)
		for i in node.mesh.get_surface_count():
			track_material(job, node.mesh.surface_get_material(i))
			track_material(job, node.get_surface_override_material(i))
	for child: Node in node.get_children(): track_resources(job, child)

static func track_material(job: Dictionary, material: Material) -> void:
	if material == null: return
	job.lease.track(material)
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: job.lease.track(texture)

static func dispose(job: Dictionary) -> void:
	for template: Node in job.templates.values():
		if is_instance_valid(template): template.free()
	job.templates = {}
	job.asset_materials = {}
	job.materials = {}
	job.chunk = {}
	if job.get("lease") != null:
		job.lease.seal()
		job.lease = null

static func fail(job: Dictionary, code: String, message: String) -> bool:
	job.error = {"code": code, "message": message}
	cancel(job)
	return true

static func material_color(key: String) -> Color:
	if key.begins_with("builtin:"):
		return {"builtin:tree": Color("745138"), "builtin:fence": Color("9b764f"), "builtin:streetlight": Color("555e63")}.get(key, Color.GRAY)
	if not key.contains(":"):
		return COLORS.get(key, Color.GRAY)
	var fields := key.split(":")
	var base: Color = {"brick": Color("b46c50"), "wood": Color("997347"), "concrete": Color("b7b8b0")}.get(fields[0], Color.GRAY)
	var tint: Color = {"residential": Color("f4dfc3"), "commercial": Color("c6dfec"), "industrial": Color("bbbec6"), "public": Color("eee3b2")}.get(fields[1], Color.WHITE)
	return base.lerp(tint, 0.2)

static func cancel(job: Dictionary) -> void:
	job.cancelled = true
	dispose(job)
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
