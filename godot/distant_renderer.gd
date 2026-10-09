extends RefCounted
## Common display-only batches. Caller owns the lease, admission and frame budget.
const BATCH_VERTICES := 1536

static func begin(data: Dictionary, parent: Node3D, lease: RefCounted, resources: RefCounted = null) -> Dictionary:
	var root := Node3D.new()
	root.set_meta("mapkit_render_root", true)
	root.add_to_group("mapkit_quality_targets")
	root.name = "MapKitDistant"
	root.visible = false
	parent.add_child(root)
	lease.track(root)
	lease.track(data.geometry)
	var material: Material = StandardMaterial3D.new()
	material.vertex_color_use_as_albedo = true
	material.roughness = 1.0
	material.cull_mode = BaseMaterial3D.CULL_DISABLED
	if resources != null:
		var context: RefCounted = resources.environment_context()
		if context != null:
			material = context.surface_material(material,0)
			material.set_shader_parameter("distant",true)
	lease.track(material)
	# Two bounded shared materials per job. Water retains the existing depth,
	# refraction and quality path, so cell LOD does not create square colour seams.
	var water := ShaderMaterial.new()
	water.shader = preload("./water_surface.gdshader")
	water.set_meta("mapkit_water",true)
	preload("./display_quality.gd").apply_material(water,preload("./display_quality.gd").active())
	water.set_shader_parameter("flow_from_uv",true)
	lease.track(water)
	return {"root": root, "owner": data.geometry, "view": data.geometry.view(), "offset": 0,
		"material": material, "water_material":water, "lease": lease, "done": false, "cancelled": false}

static func advance(job: Dictionary) -> bool:
	if job.done or job.cancelled: return true
	var count: int = job.view.vertices.size()
	if job.offset >= count:
		job.done = true
		job.view = {}
		job.owner = null
		return true
	var end := mini(count, int(job.offset) + BATCH_VERTICES)
	var kind: int = 0 if job.view.get("decoration", PackedByteArray()).is_empty() else int(job.view.decoration[job.offset])
	if job.view.has("decoration"):
		for vertex in range(int(job.offset) + 3, end, 3):
			if int(job.view.decoration[vertex]) != kind:
				end = vertex
				break
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = job.view.vertices.slice(job.offset, end)
	arrays[Mesh.ARRAY_NORMAL] = job.view.normals.slice(job.offset, end)
	arrays[Mesh.ARRAY_COLOR] = job.view.colors.slice(job.offset, end)
	if job.view.has("light_data"): arrays[Mesh.ARRAY_TEX_UV] = job.view.light_data.slice(job.offset,end)
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var instance := MeshInstance3D.new()
	instance.mesh = mesh
	instance.material_override = job.water_material if kind==2 else job.material
	instance.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	instance.set_meta("mapkit_decoration", kind==1)
	preload("./display_quality.gd").apply_node(instance, preload("./display_quality.gd").active())
	job.root.add_child(instance)
	job.lease.track(mesh)
	job.offset = end
	return false

static func cancel(job: Dictionary) -> void:
	job.cancelled = true
	job.view = {}
	job.owner = null
	job.material = null
	job.water_material = null
	if is_instance_valid(job.root) and not job.root.is_queued_for_deletion():
		job.root.visible = false
		job.root.queue_free()
	job.lease = null
