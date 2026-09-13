extends RefCounted
## Common display-only batches. Caller owns the lease, admission and frame budget.
const BATCH_VERTICES := 1536

static func begin(data: Dictionary, parent: Node3D, lease: RefCounted) -> Dictionary:
	var root := Node3D.new()
	root.visible = false
	parent.add_child(root)
	lease.track(root)
	lease.track(data.geometry)
	var material := StandardMaterial3D.new()
	material.vertex_color_use_as_albedo = true
	material.roughness = 1.0
	material.cull_mode = BaseMaterial3D.CULL_DISABLED
	lease.track(material)
	return {"root": root, "owner": data.geometry, "view": data.geometry.view(), "offset": 0,
		"material": material, "lease": lease, "done": false, "cancelled": false}

static func advance(job: Dictionary) -> bool:
	if job.done or job.cancelled: return true
	var count: int = job.view.vertices.size()
	if job.offset >= count:
		job.done = true
		job.view = {}
		job.owner = null
		return true
	var end := mini(count, int(job.offset) + BATCH_VERTICES)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = job.view.vertices.slice(job.offset, end)
	arrays[Mesh.ARRAY_NORMAL] = job.view.normals.slice(job.offset, end)
	arrays[Mesh.ARRAY_COLOR] = job.view.colors.slice(job.offset, end)
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var instance := MeshInstance3D.new()
	instance.mesh = mesh
	instance.material_override = job.material
	instance.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	job.root.add_child(instance)
	job.lease.track(mesh)
	job.offset = end
	return false

static func cancel(job: Dictionary) -> void:
	job.cancelled = true
	job.view = {}
	job.owner = null
	job.material = null
	if is_instance_valid(job.root) and not job.root.is_queued_for_deletion():
		job.root.visible = false
		job.root.queue_free()
	job.lease = null
