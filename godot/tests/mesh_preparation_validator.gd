extends SceneTree
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
const ASSET_LIBRARY := preload("../asset_library.gd")
const INSTANCES := preload("../render_instances.gd")
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
	cache.environment_profile={"lights":[BINDING.duplicate(true)]}
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
	for boundary in ["surface","generate","get_mesh","materials","material_surface","occluder","instance_parts"]:
		await cancel_at(boundary)
	await borrowed_prepared_mesh()
	await empty_prepared_mesh()
	for prepared in [false,true]:
		await replacement_tracking(prepared)
	light_role_lookup()
	await light_binding_snapshot()
	await failed_light_import()
	await imported_materials()
	for mode in ["opaque","transparent","overlay","surface_override"]:
		await instance_parts(mode)
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
	if boundary=="instance_parts":
		check(not partial.has("occluder_arrays") and cache.instance_parts(key).is_empty(),"part preparation exposes no partial template or retained CPU faces")
	else:
		check(partial.lod_job.has("occluder_arrays") if partial.has("lod_job") else partial.has("occluder_arrays"),"CPU faces stay with the admitted partial template "+boundary)
	var amount: int=cache.entries[key].bytes
	if held!=null: amount+=cache._context_bytes() # Its styled surface also borrows context resources.
	cache.release(key);cache.trim()
	check(cache.entries.has(shared),"cancelling one claimant preserves another's partial state")
	cache.shutdown()
	check(cache.prepare_template(shared,"fixture",ASSETS) and cache.template(shared,"fixture",ASSETS)==null,"late advance after shutdown cannot publish partial materials")
	cache.release(shared)
	check(not partial.has("occluder_arrays") and not partial.has("lod_job") and not partial.has("light_binding") and not partial.has("light_roles") and not partial.has("material_count"),"eviction drops preparation scratch even if diagnostics retain the entry "+boundary)
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

func empty_prepared_mesh() -> void:
	var cache := seeded_cache(ArrayMesh.new())
	var key: String=cache.entries.keys()[0]
	var item: Dictionary=cache.entries[key]
	var operations := []
	cache.mesh_observer=func(phase,_elapsed,_surface): operations.append(phase)
	var steps := 0
	while not cache.prepare_template(key,"fixture",ASSETS) and steps<20: steps+=1
	check(item.stage=="done" and steps<20,"zero-surface template completes without waiting for a material")
	check(not operations.has("material_bind") and item.template.mesh.get_surface_count()==0,"empty mesh stays empty without material work")
	check(item.template.cast_shadow==GeometryInstance3D.SHADOW_CASTING_SETTING_ON,"empty mesh is not classified as all-water")
	check(not item.has("material_count") and not item.has("material_surface"),"empty material finalization drops its cursor and count")
	cache.release(key);cache.shutdown()
	for frame in 3: await process_frame
	check(cache.bytes()==0,"empty prepared mesh retires its full reservation")

func replacement_tracking(prepared: bool) -> void:
	# Independent references must keep the complete asset charge after the
	# hierarchy is gone, whether LOD output is new or a borrowed mesh is copied.
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,BoxMesh.new().get_mesh_arrays())
	var source := StandardMaterial3D.new();source.normal_enabled=true;source.ao_enabled=true
	var bitmaps: Array[Texture2D] = []
	for slot in [BaseMaterial3D.TEXTURE_ALBEDO,BaseMaterial3D.TEXTURE_NORMAL,BaseMaterial3D.TEXTURE_AMBIENT_OCCLUSION,BaseMaterial3D.TEXTURE_ROUGHNESS,BaseMaterial3D.TEXTURE_METALLIC]:
		var bitmap := ImageTexture.create_from_image(Image.create(2,2,false,Image.FORMAT_RGBA8))
		source.set_texture(slot,bitmap);bitmaps.append(bitmap)
	mesh.surface_set_material(0,source)
	if prepared: mesh=QUALITY.prepare_lods(mesh)
	var cache := seeded_cache(mesh)
	var key: String=cache.entries.keys()[0]
	var item: Dictionary=cache.entries[key]
	var template: MeshInstance3D=item.template
	var child := MeshInstance3D.new();child.mesh=mesh
	var branch := Node3D.new();template.add_child(branch);branch.add_child(child)
	var override := StandardMaterial3D.new();child.material_override=override
	var overlay := StandardMaterial3D.new();child.material_overlay=overlay
	var surface_override := StandardMaterial3D.new();child.set_surface_override_material(0,surface_override)
	CACHE._track_node(item.lease,template)
	var lease: Lease=item.lease
	for bitmap: Texture2D in bitmaps:
		check(lease.refs.any(func(ref): return ref.get_ref()==bitmap),"import tracks each of the five independent source bitmap slots")
	check(lease.refs.any(func(ref): return ref.get_ref()==surface_override),"initial hierarchy tracks nested surface overrides")
	var steps := 0
	while item.stage!="done" and steps<100:
		var stage: String=item.stage
		var tracked := lease.refs.size()
		cache.prepare_template(key,"fixture",ASSETS);steps+=1
		if stage in ["lod_prepare","materials"]:
			var added := lease.refs.slice(tracked)
			check(added.size()<=1 and added.all(func(ref): return ref.get_ref() is Mesh),"mesh replacement never rewalks imported materials or children")
	check(item.stage=="done","nested tracking fixture completes")
	check(child.material_overlay==overlay and child.get_surface_override_material(0)==surface_override,"node overlay and surface override identities survive replacement")
	var held_mesh: Mesh=child.mesh
	var held_material: Material=template.mesh.surface_get_material(0)
	var held_override: Material=child.material_override
	var amount: int=item.bytes
	cache.release(key);cache.shutdown()
	mesh=null;source=null;override=null;item={}
	for frame in 3: await process_frame
	check(cache.bytes()==amount+cache._context_bytes(),"replacement mesh and binding retain asset/context charges")
	held_mesh=null;held_material=null
	for frame in 3: await process_frame
	check(cache.bytes()==amount+cache._context_bytes(),"independent styled override retains asset/context charges")
	held_override=null
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"original node bindings and source bitmaps retain the asset charge")
	surface_override=null;bitmaps.clear()
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"independent overlay alone retains the full asset charge")
	overlay=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"final overlay release retires the replacement lease")

func instance_parts(mode: String) -> void:
	var mesh := ArrayMesh.new()
	var box := BoxMesh.new()
	for index in 2:
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,box.get_mesh_arrays())
		mesh.surface_set_material(index,StandardMaterial3D.new())
	if mode=="transparent": mesh.surface_get_material(1).transparency=BaseMaterial3D.TRANSPARENCY_ALPHA
	var cache := seeded_cache(mesh)
	var key: String=cache.entries.keys()[0]
	var shared: String=cache.claim("fixture",ASSETS)
	var item: Dictionary=cache.entries[key]
	var template: MeshInstance3D=item.template
	template.transform=Transform3D(Basis(Vector3.UP,.3),Vector3(1,2,3))
	var branch := Node3D.new();branch.scale=Vector3(-1,2,1);template.add_child(branch)
	var child := MeshInstance3D.new();child.mesh=BoxMesh.new();child.position=Vector3(3,1,2)
	child.mesh.size=Vector3(8,4,.5)
	child.cast_shadow=GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	child.material_override=StandardMaterial3D.new();branch.add_child(child)
	if mode=="overlay": template.material_overlay=StandardMaterial3D.new()
	if mode=="surface_override": template.set_surface_override_material(0,StandardMaterial3D.new())
	CACHE._track_node(item.lease,template)
	var steps := 0
	while item.stage!="instance_parts" and steps<100:
		check(cache.instance_parts(key).is_empty(),"partial template has no published parts: "+mode)
		check(not cache.prepare_template(key,"fixture",ASSETS),"LOD/material/occluder work yields before part preparation")
		steps+=1
	check(steps<100 and not item.has("instance_parts"),"part extraction has its own admitted boundary")
	check(cache.prepare_template(shared,"fixture",ASSETS),"final part preparation publishes the template")
	var pieces: Array=cache.instance_parts(key)
	var reference := INSTANCES.parts(template)
	check(pieces.is_read_only() and pieces.size()==reference.size(),"cached part count includes empty fallback: "+mode)
	for index in pieces.size():
		for field in ["mesh","material","transform","shadow"]:
			check(pieces[index][field]==reference[index][field],"cached final part field/order: "+field)
		check(pieces[index].mesh_bounds==reference[index].mesh.get_aabb(),"cached bounds stay in mesh-local coordinates")
	reference.clear()
	check(is_same(pieces,cache.instance_parts(shared)),"concurrent claims share the same part list, including empty fallback")
	var before: Dictionary=cache.preparation_usec.duplicate()
	check(cache.prepare_template(key,"fixture",ASSETS) and before==cache.preparation_usec,"ready part reuse performs no preparation")
	var scene := Node3D.new();root.add_child(scene)
	check(INSTANCES.begin_parts(pieces,0,scene,null).is_empty(),"zero placements allocate no group")
	var groups := []
	for index in 3:
		QUALITY.current=QUALITY.profile(index)
		var group := INSTANCES.begin_parts(pieces,2,scene,null,false)
		if mode=="opaque":
			check(pieces.size()==2 and pieces.all(func(piece): return piece.is_read_only()),"one immutable part per mesh, not per surface")
			check(pieces[0].transform.is_equal_approx(template.transform) and pieces[1].transform.is_equal_approx(template.transform*branch.transform*child.transform),"root and reflected nested transforms keep traversal order")
			check(pieces[1].shadow==GeometryInstance3D.SHADOW_CASTING_SETTING_OFF and pieces[1].material==child.material_override,"child override and shadow survive preparation")
			var placement := Transform3D(Basis(Vector3.UP,.7),Vector3(index*32,0,0))
			INSTANCES.append(group,placement,str(index),"map")
			for part: Dictionary in group.groups:
				var expected_bounds: AABB=(placement*part.transform)*part.multi.mesh.get_aabb()
				check(part.bounds.is_equal_approx(expected_bounds) and part.multi.custom_aabb.is_equal_approx(expected_bounds.grow(.08)),"cached bounds preserve composed rotations/reflections and populated-only leaf margin")
				check(part.mesh_bounds==part.multi.mesh.get_aabb(),"each spatial group borrows the mesh-local bounds")
			var next_placement := Transform3D(Basis(Vector3.UP,-.4),Vector3(index*32+2,1,7))
			INSTANCES.append(group,next_placement,"next-"+str(index),"map")
			for part: Dictionary in group.groups:
				var first_bounds: AABB=(placement*part.transform)*part.multi.mesh.get_aabb()
				var next_bounds: AABB=(next_placement*part.transform)*part.multi.mesh.get_aabb()
				check(part.multi.custom_aabb.is_equal_approx(first_bounds.merge(next_bounds).grow(.08)),"subsequent placement merges original-space bounds without extra transform expansion")
			check(group.groups[1].node.get_child_count()==2,"cached large-mesh bounds retain per-placement occluders")
			check(group.groups[0].multi.mesh==pieces[0].mesh and group.groups[1].node.material_override==pieces[1].material,"spatial groups borrow final template resources")
			check(is_equal_approx(group.groups[0].node.lod_bias,QUALITY.current.lod_bias),"reused parts still read current node quality")
		else: check(group.is_empty(),"unsupported template retains ordinary-instance fallback: "+mode)
		groups.append(group)
	var held_mesh: Mesh=pieces[0].mesh if mode=="opaque" else null
	var held_material: Material=pieces[1].material if mode=="opaque" else null
	var amount: int=item.bytes+cache._context_bytes() if mode=="opaque" else 0
	cache.release(key);cache.trim()
	check(cache.entries.has(shared),"pressure cannot evict another claimant's prepared parts")
	cache.release(shared);cache.trim()
	check(not item.has("instance_parts") and cache.instance_parts(key).is_empty(),"eviction clears cached references even when entry diagnostics survive")
	cache.shutdown()
	check(cache.instance_parts(shared).is_empty() and cache.template(shared,"fixture",ASSETS)==null,"late access cannot restore evicted parts")
	for group: Dictionary in groups:
		for part: Dictionary in group.get("groups",[]): part.clear()
		group.clear()
	scene.free();groups.clear();pieces=[];mesh=null;box=null;item={}
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"last mesh/material borrowers keep template and context charged: "+mode)
	held_mesh=null
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"independent material borrower keeps full template lease: "+mode)
	held_material=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"last part borrower returns all reservations: "+mode)
	QUALITY.current={}
	# Ordinary preview calls must re-evaluate mutable material eligibility and pose.
	var preview := MeshInstance3D.new();preview.mesh=BoxMesh.new()
	var parent := Node3D.new()
	var first := INSTANCES.begin(preview,1,parent,null)
	preview.position=Vector3(7,1,4)
	var second := INSTANCES.begin(preview,1,parent,null)
	check(first.groups[0].transform!=second.groups[0].transform and second.groups[0].transform==preview.transform,"preview edits are extracted on every setup")
	preview.mesh.size=Vector3(4,2,3)
	INSTANCES.append(second,Transform3D.IDENTITY,"edited")
	check(second.groups[0].bounds.is_equal_approx(preview.transform*preview.mesh.get_aabb()),"mutable preview refreshes mesh bounds between setup and placement")
	preview.material_overlay=StandardMaterial3D.new()
	check(INSTANCES.begin(preview,1,parent,null).is_empty(),"preview eligibility is not cached")
	parent.free();preview.free();first.clear();second.clear()

func meshes(node: Node) -> Array[MeshInstance3D]:
	var result: Array[MeshInstance3D] = []
	if node is MeshInstance3D: result.append(node)
	for child: Node in node.get_children(): result.append_array(meshes(child))
	return result

func light_role_lookup() -> void:
	# Direct callers accept numeric conversion and overlapping lists too.
	var binding := {"window_materials":[2.0,"4",4],"bulb_materials":[2,3]}
	var roles := MATERIALS.prepare_light_roles(binding)
	check(roles.is_read_only(),"prepared light roles are immutable")
	var source := StandardMaterial3D.new()
	for index in 6:
		var expected: int=[0,0,1,2,1,0][index]
		check(MATERIALS.surface_role(source,binding,index)==expected and MATERIALS.surface_role(source,binding,index,roles)==expected,"prepared/direct surface-index role "+str(index))
		check(MATERIALS.surface_role(null,binding,index,roles)==expected,"missing material uses surface-index role "+str(index))
	source.set_meta("mapkit_material_index",3)
	check(MATERIALS.surface_role(source,binding,2,roles)==2 and MATERIALS.surface_role(source,binding,2)==2,"material index overrides surface index")
	var context := MATERIALS.new(128,true)
	var pending: Material=context.import_surface(source,2,binding,roles)
	check(pending.get_shader_parameter("lighting_role")==2 and pending.shader==null,"prepared import keeps the metadata role without activating a shader")
	binding.window_materials.clear();binding.bulb_materials.clear();binding.window_materials.append(3)
	check(MATERIALS.surface_role(source,binding,2)==1 and MATERIALS.surface_role(source,binding,2,roles)==2,"direct edits stay live while prepared roles remain fixed")
	check(MATERIALS.surface_role(source,binding,2,MATERIALS.prepare_light_roles({}))==0,"empty prepared roles never fall back to a later binding")
	check(context.import_surface(source,2,binding).get_shader_parameter("lighting_role")==1,"direct import reads the current binding")
	var mesh := BoxMesh.new();mesh.material=source
	var preview: Mesh=context.style_mesh(mesh,binding)
	check(preview.surface_get_material(0).get_shader_parameter("lighting_role")==1,"preview uses the current role")
	binding.window_materials.clear();binding.bulb_materials.append(3)
	preview=context.style_mesh(mesh,binding)
	check(preview.surface_get_material(0).get_shader_parameter("lighting_role")==2 and mesh.material==source,"next preview refreshes authored roles without mutating the source")

func light_binding_snapshot() -> void:
	var cache := seeded_cache(fixture())
	var key: String=cache.entries.keys()[0]
	var item: Dictionary=cache.entries[key]
	var binding: Dictionary=cache.environment_profile.lights[0]
	var other := {"asset_id":"other","window_materials":[0],"bulb_materials":[1]}
	cache.environment_profile.lights.append(other)
	var assets := ASSETS.duplicate(true);assets.other=assets.fixture.duplicate(true)
	var shared: String=cache.claim("fixture",assets)
	var other_key: String=cache.claim("other",assets)
	check(shared==key and other_key!=key,"unrelated cell lights preserve sharing; different asset bindings stay separate")
	check(cache.entries[other_key].light_binding==other,"each claim captures its own asset binding")
	check(item.light_roles=={2:1,3:2} and cache.entries[other_key].light_roles=={0:1,1:2},"asset role lookups remain independent")
	binding.window_materials[0]=7;binding.bulb_materials[0]=8
	var changed: String=cache.claim("fixture",assets)
	check(changed!=key and changed==cache.environment_key_for("fixture",assets),"edited authored binding gets its own matching key")
	check(item.light_binding==BINDING and item.light_roles=={2:1,3:2},"claim snapshot and lookup isolate nested list edits")
	cache.environment_profile={"lights":[other]}
	var unbound: String=cache.claim("fixture",assets)
	check(unbound!=key and cache.entries[unbound].light_binding.is_empty(),"missing binding is captured independently of other assets")
	check(cache.entries[unbound].light_roles.is_empty() and cache.entries[unbound].light_roles.is_read_only(),"absent bindings have a prepared empty lookup")
	cache.environment_profile={"lights":[{"asset_id":"fixture","window_materials":[],"bulb_materials":[]}]}
	var empty: String=cache.claim("fixture",assets)
	check(empty!=unbound and empty!=key,"explicit empty lists retain their original cache identity")
	while not cache.prepare_template(shared,"fixture",assets): pass
	check(not item.has("light_binding") and not item.has("light_roles"),"completed template releases its claim snapshot and lookup")
	for index in item.template.mesh.get_surface_count():
		var material: Material=item.template.mesh.surface_get_material(index)
		if material is ShaderMaterial:
			check(material.get_shader_parameter("lighting_role")==(1 if index==2 else 2 if index==3 else 0),"all surfaces keep claimed roles across profile replacement")
	var held: Material=item.template.mesh.surface_get_material(2)
	var amount: int=item.bytes+cache._context_bytes()
	cache.release(key);cache.trim()
	check(cache.entries.has(shared),"another claimant protects prepared binding during pressure")
	for claim_key: String in [shared,other_key,changed,unbound,empty]: cache.release(claim_key)
	cache.trim();cache.shutdown()
	check(cache.entries.is_empty() and not item.has("light_binding") and not item.has("light_roles"),"eviction removes every completed and partial binding snapshot and lookup")
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"last light material borrower retains template and context allowances")
	held=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"last light material release retires all allowances")

func failed_light_import() -> void:
	var assets := ASSETS.duplicate(true)
	assets.fixture.path="fixture.glb";assets.fixture.bytes=PackedByteArray()
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile=QUALITY.profile(0);cache.environment_profile={"lights":[BINDING]}
	var key: String=cache.claim("fixture",assets)
	var item: Dictionary=cache.entries[key]
	while not cache.prepare_template(key,"fixture",assets): pass
	check(item.failed and not item.has("light_binding") and not item.has("light_roles"),"failed import drops snapshot and lookup")
	cache.release(key);cache.shutdown()
	for frame in 3: await process_frame
	check(cache.bytes()==0,"failed light import retires its reservations")

func imported_materials() -> void:
	# Export a synthetic hierarchy once, then compare the ordinary GLB path with
	# early parameter capture. Both still use Godot's compressed uploaded arrays.
	var source := Node3D.new()
	var mesh := fixture()
	mesh.surface_get_material(5).resource_name="mk_leaf"
	mesh.surface_get_material(6).resource_name="mk_water_flow"
	for index in 2:
		var node := MeshInstance3D.new();node.mesh=mesh
		node.position=Vector3(index*3,1,2);node.scale=Vector3(-1,2,1)
		source.add_child(node)
	var document := GLTFDocument.new();var state := GLTFState.new()
	check(document.append_from_scene(source,state)==OK,"synthetic GLB export")
	var data := document.generate_buffer(state)
	source.free();source=null;mesh=null;document=null;state=null
	var assets := {"fixture":{"path":"fixture.glb","bytes":data,"material_json":"null","content_hash":data.hex_encode().sha256_text(),"memory_bytes":4194304,"solid":true}}
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile={"lights":[BINDING]};cache.quality_profile=QUALITY.profile(0)
	var key: String=cache.claim("fixture",assets)
	cache.environment_profile={"lights":[{"asset_id":"fixture","window_materials":[7],"bulb_materials":[8]}]}
	while cache.entries[key].stage!="lod": cache.prepare_template(key,"fixture",assets)
	var context: RefCounted=cache.environment_context()
	var original_refs := []
	var capture := func(material: Material,index: int):
		if material is StandardMaterial3D and material.transparency==BaseMaterial3D.TRANSPARENCY_DISABLED:
			original_refs.append(weakref(material))
		return context.import_surface(material,index,BINDING)
	var captured := ASSET_LIBRARY.template("fixture",assets,{},capture)
	check(not original_refs.is_empty() and original_refs.all(func(ref): return ref.get_ref()==null),"opaque import sources retire before yielding an engine frame")
	captured.free();captured=null;capture=Callable()
	for node: MeshInstance3D in meshes(cache.entries[key].template):
		for index in node.mesh.get_surface_count():
			var material := node.mesh.surface_get_material(index)
			if material is ShaderMaterial:
				check(material.shader==null and material.get_meta("mapkit_pending_surface",false),"import captures parameters without a transient shader")
			else: check(material is StandardMaterial3D and material.transparency!=BaseMaterial3D.TRANSPARENCY_DISABLED,"transparent import is untouched")
	var expected := ASSET_LIBRARY.template("fixture",assets,{})
	for node: MeshInstance3D in meshes(expected):
		node.mesh=context.style_mesh(QUALITY.prepare_lods(node.mesh),BINDING)
	var operations := []
	cache.mesh_observer=func(phase,_elapsed,surface): operations.append([phase,surface])
	while cache.entries[key].stage!="done":
		operations.clear();cache.prepare_template(key,"fixture",assets)
		check(operations.filter(func(op): return op[0]=="material_bind").size()<=1,"imported shader activation stays surface-admitted")
	cache.mesh_observer=Callable()
	var actual_nodes := meshes(cache.entries[key].template)
	var expected_nodes := meshes(expected)
	check(actual_nodes.size()==expected_nodes.size(),"import preserves hierarchy mesh count")
	for index in actual_nodes.size():
		var actual := actual_nodes[index];var reference := expected_nodes[index]
		check(actual.transform==reference.transform,"import preserves reflected hierarchy transforms")
		check(geometry(actual.mesh)==geometry(reference.mesh),"early material capture preserves compressed geometry and every generated LOD")
		for surface in actual.mesh.get_surface_count():
			var a := actual.mesh.surface_get_material(surface);var b := reference.mesh.surface_get_material(surface)
			check(a.get_class()==b.get_class(),"material class and surface order preserved")
			if a is ShaderMaterial and b is ShaderMaterial:
				check(a.shader==b.shader and not a.has_meta("mapkit_pending_surface"),"ready template has its final shader")
				for parameter in ["base_color","roughness","metallic","vertex_tinted","lighting_role","instanced","uv_scale","uv_offset","normal_strength","ao_uv2","ao_light_affect","vegetation","terrain_surface","water_kind","detail_tile","detail_quality","wind_strength"]:
					check(a.get_shader_parameter(parameter)==b.get_shader_parameter(parameter),"imported parameter preserved: "+parameter)
			else:
				check(a.transparency==b.transparency and a.albedo_color==b.albedo_color,"transparent appearance preserved")
	expected.free();expected=null;actual_nodes.clear();expected_nodes.clear();context=null
	cache.release(key);cache.shutdown()
	for frame in 3: await process_frame
	check(cache.bytes()==0,"early material capture releases all template/context resources")
	for boundary in ["context","import","lod","material_surface"]:
		await cancel_imported(assets,boundary)

func cancel_imported(assets: Dictionary, boundary: String) -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile={"lights":[BINDING]};cache.quality_profile=QUALITY.profile(0)
	var key: String=cache.claim("fixture",assets)
	var shared: String=cache.claim("fixture",assets)
	var partial: Dictionary=cache.entries[key]
	while cache.entries[key].stage!=boundary: cache.prepare_template(key,"fixture",assets)
	var held: Mesh
	if boundary in ["lod","material_surface"]: held=meshes(cache.entries[key].template)[0].mesh
	var amount: int=cache.entries[key].bytes+cache._context_bytes() if held!=null else 0
	cache.release(key);cache.trim()
	check(cache.entries.has(shared),"partial imported template stays with its other claimant "+boundary)
	cache.shutdown()
	check(cache.template(shared,"fixture",assets)==null,"shutdown cannot publish a shaderless template "+boundary)
	cache.release(shared)
	check(not partial.has("light_binding") and not partial.has("light_roles"),"cancelled import releases the claimed light snapshot and lookup "+boundary)
	for frame in 3: await process_frame
	check(cache.bytes()==amount,"partial imported mesh and context keep their full leases "+boundary)
	held=null
	for frame in 3: await process_frame
	check(cache.bytes()==0,"last pending-shader borrower retires "+boundary)
