extends RefCounted
## Display only. Call advance() per admitted batch; the owner controls frame budgets.
const ASSETS := preload("./asset_library.gd")
const DATA := preload("./chunk_data.gd")
const COLORS := {"asphalt": Color("30343b"), "concrete": Color("b7b8b0"),
	"dirt": Color("927456"), "gravel": Color("888477"), "grass": Color("738664")}
const URBAN_SURFACE := preload("./urban_surface.gdshader")
const PLAN := preload("./render_memory.gd")
const INSTANCES := preload("./render_instances.gd")
const TRIANGLES_PER_BATCH := PLAN.TRIANGLES_PER_BATCH
static var _advance_usec := 0
static var _release_usec := 0
static var _steps := 0

static func scene_position(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), -float(value[2])) * 0.01

## The caller owns admission policy. A lease implements track(Object) and seal().
## No game dependency: small standalone editor previews may omit admission.

static func begin(chunk: Dictionary, parent: Node3D, reserve: Callable = Callable(), planned_bytes: int = 0, resources: RefCounted = null) -> Dictionary:
	var view := DATA.view(chunk)
	var shared := PLAN.shared_bytes(view) if resources != null else 0
	# Older validated recipes have no per-asset immutable sharing metadata.
	# Keep their complete per-job reservation and local importer ownership.
	if resources != null and shared == 0 and not view.get("presentation", {}).get("assets", {}).is_empty():
		resources = null
	var lease: RefCounted
	if reserve.is_valid():
		var bytes := planned_bytes - shared
		if bytes > 0 and bytes <= 64 * 1024 * 1024 * 1024: lease = reserve.call(bytes)
		if lease == null:
			return {"root": null, "chunk": {}, "done": true, "cancelled": true,
				"materials": {}, "asset_materials": {}, "templates": {}, "lease": null,
				"error": {"code": "E_MEMORY_BUDGET", "message": "Display allocation exceeds memory allowance"}}
	var root := Node3D.new()
	root.set_meta("mapkit_render_root", true)
	if lease != null: lease.track(root)
	root.name = "MapCell_%s_%s" % [chunk.cell.x, chunk.cell.y]
	parent.add_child(root)
	var job := {"root": root, "chunk": view, "lease": lease, "display_lease": lease, "triangle": 0, "object": 0, "done": false, "cancelled": false, "materials": {}, "asset_materials": {}, "templates": {}, "error": {}, "resources": resources, "claims": {}, "instances": {}, "counts": {}, "borrowed_materials": {}, "steps": 0, "peak_step_usec": 0}
	if resources != null:
		resources.environment_profile = preload("./environment_profile.gd").defaults()
		var environment_json := str(view.get("presentation",{}).get("environment_json",""))
		if not environment_json.is_empty():
			var profile: Variant = JSON.parse_string(environment_json)
			if profile is Dictionary: resources.environment_profile = profile
		var sources: Dictionary = view.get("presentation", {}).get("assets", {})
		for id: String in sources:
			var key: String = resources.claim(id, sources)
			if key.is_empty():
				fail(job, "E_MEMORY_BUDGET", "Shared display resources exceed memory allowance: %s (%d bytes cached)" % [id,resources.bytes()])
				return job
			job.claims[id] = key
		for object: Dictionary in view.objects:
			var id := str(object.asset_id)
			job.counts[id] = int(job.counts.get(id, 0)) + 1
	return job

static func advance(job: Dictionary) -> bool:
	var started := Time.get_ticks_usec()
	var done := _advance(job)
	_advance_usec += Time.get_ticks_usec() - started
	_steps += 1
	job.steps = int(job.get("steps", 0)) + 1
	job.peak_step_usec = maxi(int(job.get("peak_step_usec", 0)), Time.get_ticks_usec() - started)
	return done

static func _advance(job: Dictionary) -> bool:
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
	if chunk.has("render_batches"):
		if offset < chunk.render_batches.size(): return _prepared_batch(job, chunk.render_batches[offset])
		offset = DATA.count(chunk)
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
			arrays[Mesh.ARRAY_TEX_UV] = chunk.wall_uv.slice(first * 3, offset * 3)
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
					if normal.y < maxf(normal.x, normal.z):
						uv = Vector2(point.z, point.y) if normal.x > normal.z else Vector2(point.x, point.y)
					surface.set_uv(uv)
					surface.add_vertex(point)
			surface.generate_normals()
			mesh.mesh = surface.commit()
		if not job.materials.has(key):
			if presentation.get("urban_surfaces", false) and not job.has("urban_shader"):
				job.urban_shader = job.resources.urban_shader() if job.get("resources") != null else wet_urban_shader()
				if job.urban_shader == null:
					mesh.free()
					return fail(job, "E_MEMORY_BUDGET", "Shared surface shader exceeds memory allowance")
			var material: Material
			if key.begins_with("asset:"):
				var id := key.substr(6)
				material = job.resources.material(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.material(id, sources, job.asset_materials)
				if material != null and job.get("resources") != null: job.borrowed_materials[material.get_instance_id()] = true
			else: material = surface_material(key, presentation, job.get("urban_shader"))
			if material == null:
				mesh.free()
				return fail(job, "E_RENDER_ASSET", "Validated image could not be displayed")
			if material is StandardMaterial3D and not key.begins_with("asset:"):
				material.albedo_color = ground_color(key, presentation)
				material.roughness = 0.9
				material.cull_mode = BaseMaterial3D.CULL_DISABLED
			material = _environment_surface(job, material)
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
					var template: Node3D = job.resources.template(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.template(id, sources, job.asset_materials)
					if template == null: return fail(job, "E_RENDER_ASSET", "Validated GLB could not be displayed")
					if job.get("resources") == null: track_resources(job, template)
					job.templates[id] = template
					# Import and scene attachment are different budgeted steps.
					job.object -= 1
					return false
				if job.get("resources") != null:
					if not job.instances.has(id):
						job.instances[id] = INSTANCES.begin(job.templates[id], int(job.counts.get(id, 0)), job.root, job.lease)
						job.object -= 1
						return false
					if not job.instances[id].is_empty():
						var transform := Transform3D(Basis(Vector3.UP, float(object.quarter_turns) * PI / 2.0), scene_position(object.position))
						INSTANCES.append(job.instances[id], transform, str(object.id), str(presentation.get("map_id","")))
						_environment_lamp(job,id,object,presentation)
						continue
				var instance: Node3D = job.templates[id].duplicate(0)
				var pending: Array = [instance]
				while not pending.is_empty():
					var piece: Node3D = pending.pop_back()
					if piece is MeshInstance3D: piece.set_instance_shader_parameter("building_seed",float((str(presentation.get("map_id",""))+"/"+str(object.id)).sha256_text().substr(0,6).hex_to_int())/16777215.0)
					pending.append_array(piece.get_children())
				_environment_lamp(job,id,object,presentation)
				var anchor := Node3D.new()
				anchor.set_meta("mapkit_asset_id", id)
				anchor.set_meta("mapkit_object_id", str(object.id))
				anchor.position = scene_position(object.position)
				# glTF metres: x-right, y-up, z-back; local map y points forward.
				anchor.rotation.y = float(object.quarter_turns) * PI / 2.0
				anchor.scale = Vector3.ONE
				anchor.add_child(instance)
				job.root.add_child(anchor)
				track_instance_nodes(job.lease, anchor)
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

static func _prepared_batch(job: Dictionary, batch: Dictionary) -> bool:
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = batch.vertices
	arrays[Mesh.ARRAY_NORMAL] = batch.normals
	arrays[Mesh.ARRAY_TEX_UV] = batch.uv
	var mesh := MeshInstance3D.new()
	var resource := ArrayMesh.new()
	resource.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	mesh.mesh = resource
	var key: String = batch.key
	if not job.materials.has(key):
		var presentation: Dictionary = job.chunk.get("presentation", {})
		var sources: Dictionary = presentation.get("assets", {})
		if presentation.get("urban_surfaces", false) and not job.has("urban_shader"):
			job.urban_shader = job.resources.urban_shader() if job.get("resources") != null else wet_urban_shader()
			if job.urban_shader == null:
				mesh.free()
				return fail(job, "E_MEMORY_BUDGET", "Shared surface shader exceeds memory allowance")
		var material: Material
		if key.begins_with("asset:"):
			var id := key.substr(6)
			material = job.resources.material(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.material(id, sources, job.asset_materials)
			if material != null and job.get("resources") != null: job.borrowed_materials[material.get_instance_id()] = true
		else:
			material = surface_material(key, presentation, job.get("urban_shader"))
			if material is StandardMaterial3D:
				material.albedo_color = ground_color(key, presentation)
				material.roughness = 0.9
				material.cull_mode = BaseMaterial3D.CULL_DISABLED
		if material == null:
			mesh.free()
			return fail(job, "E_RENDER_ASSET", "Validated image could not be displayed")
		material = _environment_surface(job, material)
		job.materials[key] = material
	mesh.material_override = job.materials[key]
	track_resources(job, mesh)
	job.root.add_child(mesh)
	job.triangle += 1
	return false

static func display_material_key(chunk: Dictionary, triangle: int) -> String:
	return PLAN.material_key(chunk, triangle)

## Weak references extend the charge when a trusted consumer retains a mesh,
## material or texture after its scene node is destroyed. Templates share these
## resources with duplicate(0) instances; the optional session cache owns shared leases.
static func track_instance_nodes(lease: RefCounted, node: Node) -> void:
	if lease == null: return
	lease.track(node)
	for child: Node in node.get_children(): track_instance_nodes(lease, child)

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
	if job.get("borrowed_materials", {}).has(material.get_instance_id()): return
	job.lease.track(material)
	if material is ShaderMaterial and job.get("resources") == null: job.lease.track(material.shader)
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: job.lease.track(texture)

static func dispose(job: Dictionary) -> void:
	if job.get("resources") != null:
		for key: String in job.get("claims", {}).values(): job.resources.release(key)
	else:
		for template: Node in job.templates.values():
			if is_instance_valid(template): template.free()
	job.claims = {}
	job.resources = null
	job.instances = {}
	job.counts = {}
	job.borrowed_materials = {}
	job.templates = {}
	job.asset_materials = {}
	job.materials = {}
	job.chunk = {}
	job.erase("urban_shader")
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

## Authored climate changes static ground colour only, never surface traction.
static func ground_color(key: String, presentation: Dictionary) -> Color:
	if key != "grass": return material_color(key)
	var profile: Variant = JSON.parse_string(str(presentation.get("environment_json", "{}")))
	if not profile is Dictionary: return material_color(key)
	return {"polar":Color("d9e4e8"),"arid":Color("b89a6a"),"tropical":Color("526b40")}.get(str(profile.get("climate", "")),material_color(key))

static func cancel(job: Dictionary) -> void:
	var started := Time.get_ticks_usec()
	job.cancelled = true
	dispose(job)
	if is_instance_valid(job.root) and not job.root.is_queued_for_deletion():
		var parent: Node = job.root.get_parent()
		if parent != null:
			parent.remove_child(job.root)
		job.root.queue_free()
	_release_usec += Time.get_ticks_usec() - started

static func diagnostics() -> Dictionary:
	return {"advance_usec": _advance_usec, "release_usec": _release_usec, "steps": _steps}

static func attach(chunk: Dictionary, parent: Node3D) -> Node3D:
	# Synchronous convenience for existing small editor previews; same renderer source.
	var job := begin(chunk, parent)
	while not advance(job):
		pass
	return job.root

static func surface_material(key: String, presentation: Dictionary, shader: Shader) -> Material:
	if key == "safety:metal":
		var metal := StandardMaterial3D.new()
		metal.albedo_color = Color(0.58, 0.63, 0.68)
		metal.metallic = 0.8
		metal.roughness = 0.3
		return metal
	if not presentation.get("urban_surfaces", false) or not (key.begins_with("road:") or key in ["asphalt", "concrete"]): return StandardMaterial3D.new()
	var material := ShaderMaterial.new()
	material.shader = shader
	var style: Dictionary = presentation.get("road_styles", {}).get(key, {})
	var concrete := key == "concrete" or int(style.get("surface", 0)) == 1
	material.set_shader_parameter("base_color", COLORS["concrete" if concrete else "asphalt"])
	material.set_shader_parameter("paving", concrete)
	material.set_shader_parameter("marked", not style.is_empty())
	for name: String in style:
		if name != "surface": material.set_shader_parameter(name, style[name])
	return material

static func _environment_lamp(job: Dictionary, id: String, object: Dictionary, presentation: Dictionary) -> void:
	var resources: RefCounted = job.get("resources")
	if resources == null: return
	for binding: Dictionary in resources.environment_profile.get("lights",[]):
		if binding.asset_id != id or binding.get("bulb_materials",[]).is_empty(): continue
		var lamps: Array = job.root.get_meta("environment_lamps",[])
		var rotation := Basis(Vector3.UP,float(object.quarter_turns)*PI/2.0)
		var point: Array = binding.position_cm
		var position: Vector3 = scene_position(object.position)+rotation*Vector3(point[0],point[1],-point[2])*0.01
		var rgb: Array = binding.color
		lamps.append({"position":position,"basis":Basis.looking_at(Vector3.DOWN,Vector3.FORWARD),
			"range":float(binding.range_cm)*0.01,"energy":2.0,"color":Color(float(rgb[0])/255.0,float(rgb[1])/255.0,float(rgb[2])/255.0)})
		job.root.set_meta("environment_lamps",lamps)
		job.root.add_to_group("mapkit_environment_cells")

static func _environment_surface(job: Dictionary, material: Material) -> Material:
	if job.get("resources") == null: return material
	var context: RefCounted = job.resources.environment_context()
	if context == null: return material
	if material is ShaderMaterial and material.shader == job.get("urban_shader"):
		material.set_shader_parameter("environment_enabled",true)
		material.set_shader_parameter("environment_data",context.texture)
		return material
	return context.surface_material(material,0)

static func wet_urban_shader() -> Shader:
	var result := Shader.new()
	result.code = URBAN_SURFACE.code.replace('#include "wet_surface.gdshaderinc"',preload("./wet_surface.gdshaderinc").code)
	return result
