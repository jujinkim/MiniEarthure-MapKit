extends SceneTree
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
const BINDING := {"asset_id":"fixture","window_materials":[2],"bulb_materials":[3]}
const ASSETS := {"fixture":{"content_hash":"synthetic-mesh","memory_bytes":4194304,"material_json":"null","solid":true}}
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes=amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed=true
	func retired() -> bool: return sealed and refs.all(func(ref): return ref.get_ref()==null)

func check(value: bool, label: String) -> void:
	if not value: failed=true;push_error(label)

func _initialize() -> void: run.call_deferred()

func fixture() -> ArrayMesh:
	var mesh := ArrayMesh.new()
	var sphere := SphereMesh.new();sphere.radial_segments=24;sphere.rings=12
	for index in 10:
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,sphere.get_mesh_arrays())
		var material := StandardMaterial3D.new()
		material.albedo_color=Color(.1*index,.3,.5)
		material.set_meta("mapkit_material_index",index)
		if index==4: material.transparency=BaseMaterial3D.TRANSPARENCY_ALPHA
		mesh.surface_set_material(index,material)
	return mesh

func reference_lods(mesh: Mesh) -> Mesh:
	# Independent whole-mesh reference for exact pre-staging geometry/LOD output.
	var importer := ImporterMesh.new()
	for index in mesh.get_surface_count():
		importer.add_surface(Mesh.PRIMITIVE_TRIANGLES,mesh.surface_get_arrays(index),[],{},mesh.surface_get_material(index))
	importer.generate_lods(60.0,0.0,[])
	return importer.get_mesh()

func geometry(mesh: Mesh) -> Array:
	var result := []
	var importer := ImporterMesh.from_mesh(mesh)
	for index in mesh.get_surface_count():
		var surface := [mesh.surface_get_arrays(index)]
		for lod in importer.get_surface_lod_count(index):
			surface.append([importer.get_surface_lod_size(index,lod),importer.get_surface_lod_indices(index,lod)])
		result.append(surface)
	return result

func seeded_cache(mesh: Mesh) -> RefCounted:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile={"lights":[BINDING]}
	cache.quality_profile=QUALITY.profile(0)
	cache.environment_context()
	var key: String=cache.claim("fixture",ASSETS)
	var node := MeshInstance3D.new();node.mesh=mesh
	var item: Dictionary=cache.entries[key]
	item.template=node;item.pending=[node];item.stage="lod"
	CACHE._track_node(item.lease,node)
	return cache

func run() -> void:
	check(QUALITY.prepare_lods(null)==null,"missing mesh remains empty")
	var empty := ArrayMesh.new()
	check(QUALITY.prepare_lods(empty)==empty,"empty mesh requires no importer work")
	var source := fixture()
	var expected := reference_lods(source)
	var expected_geometry := geometry(expected)
	var job := QUALITY.begin_lods(source)
	var operations := []
	var observer := func(phase,elapsed,surface):
		operations.append([phase,surface]);check(elapsed>=0,"nonnegative inner operation cost")
	for index in 10:
		operations.clear()
		check(not QUALITY.advance_lods(job,observer),"partial surfaces cannot publish a result")
		check(job.result==null and operations==[["lod_arrays",index],["lod_add_surface",index]],"one surface read per admitted step")
	operations.clear()
	check(not QUALITY.advance_lods(job,observer) and operations==[["lod_generate",-1]],"LOD generation has its own admission")
	operations.clear()
	check(QUALITY.advance_lods(job,observer) and operations==[["lod_get_mesh",-1]],"output upload has its own admission")
	check(not job.has("input") and not job.has("source"),"completed preparation releases importer scratch")
	check(geometry(job.result)==expected_geometry,"staging preserves arrays, normals, UVs, winding and every LOD index/distance")
	check(ImporterMesh.from_mesh(job.result).get_surface_lod_count(0)>0,"comparison exercises generated LODs")
	check(QUALITY.prepare_lods(job.result)==job.result,"prepared mesh remains reusable")
	var cache := seeded_cache(source)
	var key: String=cache.entries.keys()[0]
	var shared: String=cache.claim("fixture",ASSETS)
	var item: Dictionary=cache.entries[key]
	var lod_output: Mesh
	var steps := 0
	while item.stage!="done" and steps<100:
		operations.clear();cache.mesh_observer=observer
		cache.prepare_template(shared if steps%2 else key,"fixture",ASSETS)
		cache.mesh_observer=Callable()
		if item.stage=="materials": lod_output=item.styling.mesh
		if operations.any(func(op): return op[0]=="occluder_cached_arrays"):
			check(operations.all(func(op): return op[0]=="occluder_cached_arrays"),"occlusion reads only retained CPU positions/indices")
		check(operations.filter(func(op): return op[0]=="lod_arrays").size()<=1 and operations.filter(func(op): return op[0]=="material_bind").size()<=1,"interleaved claims cannot drain multiple input/material surfaces")
		check(not operations.any(func(op): return op[0]=="occluder_arrays"),"new LOD output needs no occluder GPU readback")
		steps+=1
	check(item.stage=="done" and steps<100,"interleaved claims complete a single template")
	check(not item.has("occluder_arrays"),"ready template releases retained CPU faces")
	var styled: Mesh=item.template.mesh
	check(styled==lod_output and styled!=source,"new private LOD mesh is styled without a duplicate")
	check(geometry(styled)==expected_geometry,"direct styling preserves every geometry/LOD buffer")
	for index in 10:
		check(source.surface_get_material(index) is StandardMaterial3D,"published input materials are immutable")
		var material := styled.surface_get_material(index)
		if index==4: check(material==source.surface_get_material(index),"transparent material identity/order preserved")
		else:
			check(material is ShaderMaterial,"opaque material wrapped")
			check(material.get_shader_parameter("lighting_role")== (1 if index==2 else 2 if index==3 else 0),"per-surface lighting role preserved")
	var before: Dictionary=cache.preparation_usec.duplicate()
	check(cache.prepare_template(shared,"fixture",ASSETS) and cache.preparation_usec==before,"ready reuse performs no preparation")
	cache.release(key);cache.release(shared);cache.shutdown()
	lod_output=null;styled=null;item={};source=null;expected=null;job={}
	for frame in 3: await process_frame
	check(cache.bytes()==0,"completed staged resources retire")
	# Test cancellation at each persistent scratch boundary, including a styled
	# partial mesh held independently of the template and cache.
	for boundary in ["surface","generate","get_mesh","materials","material_surface","occluder"]:
		await cancel_at(boundary)
	await borrowed_prepared_mesh()
	print("mesh_preparation_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func cancel_at(boundary: String) -> void:
	var cache := seeded_cache(fixture())
	var key: String=cache.entries.keys()[0]
	var shared: String=cache.claim("fixture",ASSETS)
	var steps := 0
	while steps<100:
		cache.prepare_template(key,"fixture",ASSETS);steps+=1
		var item: Dictionary=cache.entries[key]
		if item.stage==boundary:
			if boundary!="material_surface" or item.material_surface>0: break
		if item.has("lod_job") and item.lod_job.stage==boundary:
			if boundary!="surface" or item.lod_job.surface>0: break
	check(steps<100,"reached cancellation boundary "+boundary)
	var held: Mesh=cache.entries[key].styling.mesh if boundary=="material_surface" else null
	var partial: Dictionary=cache.entries[key]
	check(partial.lod_job.has("occluder_arrays") if partial.has("lod_job") else partial.has("occluder_arrays"),"CPU faces stay with the admitted partial template "+boundary)
	var amount: int=cache.entries[key].bytes
	if held!=null: amount+=cache._context_bytes() # Its styled surface also borrows context resources.
	cache.release(key);cache.trim()
	check(cache.entries.has(shared),"cancelling one claimant preserves another's partial state")
	cache.shutdown()
	check(cache.prepare_template(shared,"fixture",ASSETS) and cache.template(shared,"fixture",ASSETS)==null,"late advance after shutdown cannot publish partial materials")
	cache.release(shared)
	check(not partial.has("occluder_arrays") and not partial.has("lod_job"),"eviction drops CPU faces even if diagnostics retain the entry "+boundary)
	for frame in 3: await process_frame
	check(cache.entries.is_empty(),"shutdown removes partial template "+boundary)
	check(cache.bytes()==(amount if held!=null else 0),"scratch retires while a partial mesh borrower stays charged "+boundary)
	held=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"last partial borrower retires "+boundary)

func borrowed_prepared_mesh() -> void:
	var mesh := QUALITY.prepare_lods(fixture())
	var cache := seeded_cache(mesh)
	var key: String=cache.entries.keys()[0]
	while not cache.prepare_template(key,"fixture",ASSETS): pass
	check(cache.entries[key].template.mesh!=mesh,"already prepared borrowed mesh gets private material bindings")
	check(mesh.surface_get_material(0) is StandardMaterial3D,"borrowed prepared source stays immutable")
	cache.release(key);cache.shutdown();mesh=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"borrowed preparation path retires")
