extends SceneTree
const CACHE := preload("../render_resource_cache.gd")
const RENDER := preload("../chunk_renderer.gd")
const QUALITY := preload("../display_quality.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func reference() -> Mesh:
	# Original per-object canopy, independent of the cache's shape factory.
	var shape := SphereMesh.new()
	shape.radius = 1.84; shape.height = 4.0; shape.radial_segments = 16; shape.rings = 8
	return QUALITY.prepare_lods(shape)

func geometry(mesh: Mesh) -> Array:
	var result := []
	var importer := ImporterMesh.from_mesh(mesh)
	for surface in mesh.get_surface_count():
		var values := [mesh.surface_get_primitive_type(surface),mesh.surface_get_arrays(surface)]
		for lod in importer.get_surface_lod_count(surface):
			values.append([importer.get_surface_lod_size(surface,lod),importer.get_surface_lod_indices(surface,lod)])
		result.append(values)
	return result

func run() -> void:
	await preparation()
	for boundary in ["canopy_source","surface","generate","get_mesh","done"]:
		await cancel_at(boundary)
	await cancel_attachment()
	await attachment()
	await detached_node()
	await rejected()
	QUALITY.current = {}
	print("tree_canopy_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func preparation() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	var key: String = cache.claim_tree_canopy()
	var second: String = cache.claim_tree_canopy()
	check(key == second and cache.bytes() == CACHE.TREE_CANOPY_BYTES,"concurrent claims reserve one bounded canopy before allocation")
	check(cache.tree_canopy(key) == null,"unprepared mesh is never published")
	var operations := []
	cache.mesh_observer = func(phase,_elapsed,_surface): operations.append(phase)
	var phases := []
	for step in 4:
		operations.clear()
		var done: bool = cache.prepare_tree_canopy(key if step%2 == 0 else second)
		phases.append_array(operations)
		check(done == (step == 3),"one admitted source/read/generate/upload stage per call")
		check(cache.tree_canopy(key) == null if not done else cache.tree_canopy(key) != null,"only complete geometry published")
	check(phases == ["lod_arrays","lod_add_surface","lod_generate","lod_get_mesh"],"one shared readback/generation/upload sequence")
	var mesh: Mesh = cache.tree_canopy(key)
	check(geometry(mesh) == geometry(reference()),"all geometry slots and every LOD index/threshold exactly match original")
	check(mesh.surface_get_material(0) == null,"shared canopy has no mutable surface material")
	var before: Dictionary = cache.preparation_usec.duplicate()
	operations.clear()
	check(cache.prepare_tree_canopy(key) and operations.is_empty() and cache.preparation_usec == before,"ready reuse performs no preparation")
	cache.release(key); cache.release(second); cache.trim()
	check(cache.entries.is_empty() and cache.bytes() == CACHE.TREE_CANOPY_BYTES,"evicted borrowed mesh remains charged")
	# With the entire cap still borrowed, another copy cannot receive credit.
	cache.limit_bytes = CACHE.TREE_CANOPY_BYTES
	check(cache.claim_tree_canopy().is_empty(),"retiring borrower blocks over-budget replacement")
	mesh = null
	for _frame in 3: await process_frame
	check(cache.bytes() == 0,"last mesh borrower releases the shared charge")
	key = cache.claim_tree_canopy()
	check(not key.is_empty(),"retired capacity admits a fresh canopy")
	cache.release(key); cache.shutdown()
	check(cache.bytes() == 0,"unused claim returns reservation")

func cancel_at(boundary: String) -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	var key: String = cache.claim_tree_canopy()
	var second: String = cache.claim_tree_canopy()
	var item: Dictionary = cache.entries[key]
	for _step in 4:
		if item.stage == boundary or (item.has("lod_job") and item.lod_job.stage == boundary): break
		cache.prepare_tree_canopy(key)
	check(item.stage == boundary or (item.has("lod_job") and item.lod_job.stage == boundary),"reached cancellation boundary "+boundary)
	var held: RefCounted = item.get("mesh") if boundary == "done" else item.lod_job.source if item.has("lod_job") else null
	cache.release(key); cache.trim()
	check(cache.entries.has(second),"one cancellation preserves concurrent claimant "+boundary)
	cache.shutdown()
	check(cache.claim_tree_canopy().is_empty() and cache.prepare_tree_canopy(second) and cache.tree_canopy(second) == null,"closed cache rejects late preparation/publication "+boundary)
	cache.release(second)
	check(not item.has("lod_job") and not item.has("mesh"),"eviction clears scratch despite retained diagnostics "+boundary)
	for _frame in 3: await process_frame
	check(cache.bytes() == (CACHE.TREE_CANOPY_BYTES if held != null else 0),"partial/final borrower remains charged "+boundary)
	held = null
	for _frame in 3: await process_frame
	check(cache.bytes() == 0,"last partial/final borrower retires "+boundary)

func fixture(prepared: bool) -> Dictionary:
	var vertices := PackedVector3Array([Vector3.ZERO,Vector3.RIGHT,Vector3.FORWARD])
	var chunk := {"cell":Vector2i.ZERO,"triangles":[{"surface":"grass","object_id":"ground"}],"objects":[],"scene_vertices":vertices,"scene_normals":PackedVector3Array([Vector3.UP,Vector3.UP,Vector3.UP]),"wall_uv":PackedVector2Array([Vector2.ZERO,Vector2.RIGHT,Vector2.UP])}
	for index in 3:
		chunk.objects.append({"id":"tree-%d"%index,"asset_id":"builtin:tree","position":[index*500,100,index*100],"quarter_turns":index,"yaw_offset_mdeg":17000})
	if prepared: chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func attachment() -> void:
	QUALITY.current = QUALITY.profile(0)
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(0)
	var parent := Node3D.new(); root.add_child(parent)
	var jobs := []; var leases := []
	var reads := []
	cache.mesh_observer = func(phase,_elapsed,_surface): reads.append(phase)
	for prepared in [false,true]:
		var chunk := fixture(prepared)
		var lease := Lease.new(RENDER.PLAN.estimate(chunk,0)); leases.append(lease)
		jobs.append(RENDER.begin(chunk,parent,func(_bytes): return lease,lease.bytes,cache))
	for _step in 100:
		for job: Dictionary in jobs:
			if not job.done: RENDER.advance(job)
		if jobs.all(func(job): return job.done): break
	check(jobs.all(func(job): return job.done and job.error.is_empty()),"interleaved prepared/raw attachments finish")
	check(reads.count("lod_arrays") == 1 and reads.count("lod_generate") == 1 and reads.count("lod_get_mesh") == 1,"six canopies across cells prepare only once")
	var first: MeshInstance3D = jobs[0].root.get_child(1)
	var other: MeshInstance3D = jobs[1].root.get_child(1)
	var mesh: Mesh = first.mesh
	check(geometry(mesh) == geometry(reference()),"renderer binds unchanged geometry/LOD")
	for job: Dictionary in jobs:
		for index in 3:
			var canopy: MeshInstance3D = job.root.get_child(index+1)
			check(canopy.mesh == mesh and canopy.get_meta("mapkit_decoration",false),"all placements borrow one decorative mesh")
			check(canopy.position == Vector3(index*5,5.8,-index) and canopy.basis == Basis.IDENTITY,"original canopy placement ignores tree yaw")
			check(canopy.get_child_count() == 0,"leaf canopy adds no occluder")
			check(canopy == first or canopy.material_override != first.material_override,"mutable canopy material remains independent")
	check(first.material_override.get_meta("mapkit_vegetation",false),"leaf shader role preserved")
	await pixels(first,cache)
	QUALITY.apply_node(first,QUALITY.profile(2))
	check(first.visibility_range_end == 192 and other.visibility_range_end == 64,"node quality visibility is independent")
	first.visible = false; first.position += Vector3.RIGHT
	check(other.visible and other.position == Vector3(0,5.8,0),"node visibility/transform mutations are independent")
	first.material_override.set_shader_parameter("base_color",Color.RED)
	check(other.material_override.get_shader_parameter("base_color") != Color.RED,"material edits do not leak across trees")
	first = null; other = null
	RENDER.cancel(jobs[0]); jobs[0] = {}
	for _frame in 4: await process_frame
	check(leases[0].retired(),"dead cell reservation retires while another cell and cache share its mesh")
	cache.trim()
	check(cache.bytes() == cache._context_bytes()+CACHE.TREE_CANOPY_BYTES,"pressure retains borrowed mesh and live material context")
	RENDER.cancel(jobs[1]); jobs.clear(); parent.queue_free(); cache.shutdown()
	for _frame in 4: await process_frame
	check(leases[1].retired() and cache.bytes() == CACHE.TREE_CANOPY_BYTES,"mesh alone holds only shared lease after cells/context retire")
	mesh = null
	for _frame in 4: await process_frame
	check(cache.bytes() == 0,"final rendered mesh borrower retires")
	# Standalone preview keeps its job-local material and geometry path.
	parent = Node3D.new(); root.add_child(parent)
	var preview := RENDER.begin(fixture(false),parent)
	while not RENDER.advance(preview): pass
	check(preview.error.is_empty() and geometry(preview.root.get_child(1).mesh) == geometry(reference()),"standalone preview geometry preserved")
	check(preview.root.get_child(1).material_override is StandardMaterial3D,"standalone preview leaf remains a source material")
	RENDER.cancel(preview); parent.queue_free()
	await process_frame

func pixels(canopy: MeshInstance3D, cache: RefCounted) -> void:
	# Compare the same rendered frame: leaf motion intentionally depends on TIME.
	var viewports := []; var nodes := []
	for original in [false,true]:
		var viewport := SubViewport.new(); viewport.size = Vector2i(160,160)
		viewport.own_world_3d = true; viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
		root.add_child(viewport); viewports.append(viewport)
		var node := MeshInstance3D.new(); nodes.append(node)
		if original:
			var leaf := StandardMaterial3D.new(); leaf.albedo_color = Color("486447"); leaf.resource_name = "mk_leaf"
			node.mesh = reference(); node.material_override = RENDER._environment_surface({"resources":cache},leaf)
		else:
			node.mesh = canopy.mesh; node.material_override = canopy.material_override
		viewport.add_child(node)
		var camera := Camera3D.new(); viewport.add_child(camera)
		camera.position = Vector3(5,3,7); camera.look_at(Vector3.ZERO); camera.current = true
		var light := DirectionalLight3D.new(); viewport.add_child(light); light.rotation_degrees = Vector3(-40,-30,0)
	for level in 3:
		for node: MeshInstance3D in nodes: QUALITY.apply_node(node,QUALITY.profile(level))
		for _frame in 3: await process_frame
		await RenderingServer.frame_post_draw
		var actual: Image = viewports[0].get_texture().get_image()
		check(actual.get_pixel(80,80) != actual.get_pixel(0,0),"canopy pixel comparison renders visible geometry")
		check(viewports[1].get_texture().get_image().get_data() == actual.get_data(),"identical original canopy pixels at quality %d"%level)
	for viewport: SubViewport in viewports: viewport.queue_free()
	await process_frame

func cancel_attachment() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(0)
	var parent := Node3D.new(); root.add_child(parent)
	var jobs := []; var leases := []
	for _index in 2:
		var chunk := fixture(false)
		var lease := Lease.new(RENDER.PLAN.estimate(chunk,0)); leases.append(lease)
		jobs.append(RENDER.begin(chunk,parent,func(_bytes): return lease,lease.bytes,cache))
	for _step in 40:
		RENDER.advance(jobs[0])
		if jobs[0].last_phase == "canopy_preparation": break
	check(cache.entries[CACHE.TREE_CANOPY_KEY].lod_job.stage == "surface","renderer can cancel before first canopy readback")
	RENDER.cancel(jobs[0]); cache.trim()
	check(cache.entries[CACHE.TREE_CANOPY_KEY].pins == 1,"renderer cancellation releases exactly its own claim")
	while not RENDER.advance(jobs[1]): pass
	check(jobs[1].error.is_empty() and jobs[1].root.get_child_count() == 4,"remaining renderer claimant finishes the partial canopy")
	RENDER.cancel(jobs[1]); jobs.clear(); parent.queue_free(); cache.shutdown()
	for _frame in 4: await process_frame
	check(leases.all(func(lease): return lease.retired()) and cache.bytes() == 0,"interleaved cancellation retires cell and shared leases")

func detached_node() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(0)
	var parent := Node3D.new(); root.add_child(parent)
	var chunk := fixture(false)
	var lease := Lease.new(RENDER.PLAN.estimate(chunk,0))
	var job := RENDER.begin(chunk,parent,func(_bytes): return lease,lease.bytes,cache)
	while not RENDER.advance(job): pass
	var held: MeshInstance3D = job.root.get_child(1)
	job.root.remove_child(held); held.mesh = null; held.material_override = null
	RENDER.cancel(job); job.clear(); parent.queue_free(); cache.shutdown()
	for _frame in 4: await process_frame
	check(not lease.retired() and cache.bytes() == 0,"detached node holds cell overhead independently of shared geometry")
	held.free()
	for _frame in 4: await process_frame
	check(lease.retired(),"detached canopy node returns cell reservation after release")

func rejected() -> void:
	var denied := CACHE.new(func(_bytes): return null)
	check(denied.claim_tree_canopy().is_empty() and denied.entries.is_empty(),"denied shared reservation allocates no mesh")
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.limit_bytes = CACHE.TREE_CANOPY_BYTES-1
	check(cache.claim_tree_canopy().is_empty() and cache.bytes() == 0,"cache bound applies before canopy allocation")
	cache.limit_bytes = 128*1024*1024
	cache.environment_profile = {"lights":[]}; cache.quality_profile = QUALITY.profile(0)
	cache.environment_context()
	cache.limit_bytes = cache.bytes()+CACHE.TREE_CANOPY_BYTES-1
	var parent := Node3D.new(); root.add_child(parent)
	var chunk := fixture(false)
	var lease := Lease.new(RENDER.PLAN.estimate(chunk,0))
	var job := RENDER.begin(chunk,parent,func(_bytes): return lease,lease.bytes,cache)
	check(job.cancelled and job.error.get("code") == "E_MEMORY_BUDGET" and job.claims.is_empty(),"renderer rejection cancels and clears claims")
	parent.queue_free(); cache.shutdown(); job.clear()
	for _frame in 4: await process_frame
	check(lease.retired() and cache.bytes() == 0,"rejected attachment returns all reservations")
