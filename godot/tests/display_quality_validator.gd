extends SceneTree
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
const WATER := preload("../water_renderer.gd")
const INSTANCES := preload("../render_instances.gd")
const RENDER := preload("../chunk_renderer.gd")
var failed := false

class Lease extends RefCounted:
	var refs: Array[WeakRef] = []
	var sealed := false
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func check(ok: bool, label: String) -> void:
	if not ok: failed=true; push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	for level in 3:
		var p := QUALITY.profile(level)
		var cache := CACHE.new()
		cache.quality_profile=p
		cache.environment_profile={"lights":[]}
		var context: RefCounted=cache.environment_context()
		check(context!=null and cache.bytes()==QUALITY.tile_bytes(p.texture_size,10),"exact admitted tier memory")
		for texture: Texture2D in context.tiles.values(): check(texture.get_width()==p.texture_size,"requested texture resolution")
		check(context.signs.get_width()==(1024 if p.texture_size>=256 else 512),"sign atlas halves with 128px tiles")
		QUALITY.current=QUALITY.profile(2-level)
		check(context==cache.environment_context(),"live quality keeps existing texture owner")
		cache.shutdown();context=null
	var vertices := PackedVector3Array([Vector3.ZERO,Vector3.RIGHT,Vector3.FORWARD,Vector3(65,0,0),Vector3(66,0,0),Vector3(65,0,-1)])
	var chunk := {"triangles":[{"surface":"concrete","object_id":"a"},{"surface":"concrete","object_id":"b"}],"objects":[],"scene_vertices":vertices,"scene_normals":PackedVector3Array([Vector3.UP,Vector3.UP,Vector3.UP,Vector3.UP,Vector3.UP,Vector3.UP]),"wall_uv":PackedVector2Array([Vector2.ZERO,Vector2.ONE,Vector2.ZERO,Vector2.ZERO,Vector2.ONE,Vector2.ZERO])}
	var planner := preload("../render_memory.gd")
	var batches: Array = planner.prepare_batches(chunk)
	check(batches.size()==2 and batches[0].key==batches[1].key,"equal materials in distant spatial buckets stay independently cullable")
	chunk.render_batches=batches
	check(planner.upper_bound({"triangles":2,"objects":0})>=planner.estimate(chunk,0),"pre-generation allowance covers spatial batches and occluders")
	chunk.scene_vertices=PackedVector3Array([Vector3(31,0,1),Vector3(33,0,1),Vector3(31,0,3),Vector3(33,0,3),Vector3(31,0,3),Vector3(33,0,1)])
	batches=planner.prepare_batches(chunk)
	check(batches.size()==1 and batches[0].vertices==chunk.scene_vertices,"a face crossing a bucket edge batches together without changing ordered geometry")
	var scene := Node3D.new();root.add_child(scene)
	await tracked_quality()
	await instance_quality()
	occluder_reuse(scene)
	occluder_coverage(scene)
	var mesh := SphereMesh.new();mesh.radial_segments=48;mesh.rings=24
	var lod := QUALITY.prepare_lods(mesh)
	var importer := ImporterMesh.from_mesh(lod)
	check(importer.get_surface_lod_count(0)>0,"template cache contains actual engine LOD index buffers")
	check(QUALITY.prepare_lods(lod)==lod,"immutable LOD mesh reused")
	var template := MeshInstance3D.new();template.mesh=lod
	var obstacle := INSTANCES.begin(template,1,scene,null,true)
	for level in 3:
		QUALITY.apply_tree(scene,QUALITY.profile(level))
		check(obstacle.groups[0].node.visibility_range_end==0.0,"small collision object remains visible at every quality")
	var group := INSTANCES.begin(template,2,scene,null,false)
	QUALITY.apply_tree(scene,QUALITY.profile(0))
	check(group.groups[0].node.visibility_range_end==64.0,"explicit soft decoration uses the low distance")
	obstacle.clear()
	INSTANCES.append(group,Transform3D(Basis.IDENTITY,Vector3(10,0,0)),"a")
	INSTANCES.append(group,Transform3D(Basis.IDENTITY,Vector3(12,0,0)),"b")
	check(group.groups[0].multi.custom_aabb.has_point(Vector3(10,0,0)) and group.groups[0].multi.custom_aabb.size.x<5.0,"MultiMesh bounds follow only populated instances")
	template.free()
	var surfaces := ArrayMesh.new()
	var unit := BoxMesh.new()
	surfaces.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,unit.get_mesh_arrays())
	surfaces.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,unit.get_mesh_arrays())
	var single_template := MeshInstance3D.new();single_template.mesh=surfaces
	var single := INSTANCES.begin(single_template,1,scene,null)
	check(single.get("groups",[]).size()==1,"multi-material geometry is drawn once, including single-object groups")
	var pose := Transform3D(Basis(Vector3.UP,.37),Vector3(5,1,3))
	INSTANCES.append(single,pose,"single-yaw","fixture-map")
	check(single.groups[0].multi.get_instance_transform(0).is_equal_approx(pose),"single-object arbitrary yaw and translation survive")
	check(single.groups[0].multi.get_instance_custom_data(0).r>0 and single.groups[0].multi.mesh.get_surface_count()==2,"seed custom data and both material surfaces survive")
	single_template.scale=Vector3.ONE*4
	var metre_scaled := INSTANCES.begin(single_template,1,scene,null,false)
	check(not metre_scaled.groups[0].node.get_meta("mapkit_decoration"),"actual-metre root scale keeps buildings out of small-prop distance culling")
	metre_scaled.clear()
	single_template.free();single.clear()
	var fallback := MeshInstance3D.new();fallback.mesh=surfaces
	var shared := ShaderMaterial.new();shared.shader=preload("../atmosphere_surface.gdshader")
	shared.set_meta("mapkit_opaque",true)
	surfaces.surface_set_material(0,shared);surfaces.surface_set_material(1,shared)
	preload("../chunk_renderer.gd")._seed_fallback_materials(fallback,.27,null)
	var seeded: ShaderMaterial=fallback.get_active_material(0)
	check(seeded!=shared and seeded==fallback.get_active_material(1),"fallback owns one parameter binding across identical surfaces")
	check(seeded.shader==shared.shader and is_equal_approx(seeded.get_shader_parameter("building_seed"),.27),"fallback keeps stable lighting seed and shared shader")
	check(is_equal_approx(shared.get_shader_parameter("building_seed"),.5),"fallback never mutates the shared template material")
	fallback.material_override=shared
	preload("../chunk_renderer.gd")._seed_fallback_materials(fallback,.63,null)
	check(fallback.material_override!=shared and is_equal_approx(fallback.material_override.get_shader_parameter("building_seed"),.63),"fallback handles material overrides")
	fallback.free()
	var box := BoxMesh.new();box.size=Vector3(5,3,.5)
	var occluder := QUALITY.occluder(box,null)
	check(occluder!=null and occluder.occluder.vertices.size()<=192,"bounded actual face occluder")
	if occluder!=null: scene.add_child(occluder)
	var leaves := StandardMaterial3D.new();leaves.resource_name="mk_leaf"
	var weather := MATERIALS.new()
	var moving: ShaderMaterial=weather.surface_material(leaves,0)
	for level in 3:
		QUALITY.apply_material(moving,QUALITY.profile(level))
		check(int(moving.get_shader_parameter("detail_quality"))==level,"material detail follows live preset")
		check(is_equal_approx(moving.get_shader_parameter("wind_strength"),QUALITY.profile(level).vegetation_motion),"leaf motion follows live preset, disabled on low")
	check(moving.get_shader_parameter("vegetation"),"only leaf material opts into motion")
	moving=null;weather=null;leaves=null
	var record := {"body":{"id":"fixture","flow_cm_s":[25,5]},"surface":[[[-300,0,-300],[300,0,-300],[300,0,300]],[[-300,0,-300],[300,0,300],[-300,0,300]]]}
	var water := WATER.mesh(record,0,2);scene.add_child(water)
	var floor_mesh := MeshInstance3D.new();floor_mesh.mesh=BoxMesh.new();floor_mesh.mesh.size=Vector3(8,.5,8);floor_mesh.position.y=-.8;scene.add_child(floor_mesh)
	var camera := Camera3D.new();scene.add_child(camera);camera.position=Vector3(3,3,4);camera.look_at(Vector3.ZERO)
	var light := DirectionalLight3D.new();light.rotation_degrees.x=-50;scene.add_child(light)
	for level in 3:
		QUALITY.apply_material(water.material_override,QUALITY.profile(level))
		for i in 5: await process_frame
		check(int(water.material_override.get_shader_parameter("quality_level"))==level,"live water quality binding")
	scene.queue_free();group.clear();importer=null;lod=null
	for i in 4: await process_frame
	QUALITY.current={}
	print("display_quality_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func tracked_quality() -> void:
	var lease := Lease.new()
	var job := {"lease":lease}
	var parent := MeshInstance3D.new();parent.mesh=BoxMesh.new()
	parent.set_meta("mapkit_decoration",true)
	var child := MeshInstance3D.new();child.mesh=BoxMesh.new();child.mesh.size=Vector3(8,4,.5)
	child.set_meta("mapkit_occluder",true)
	parent.add_child(child)
	for node: MeshInstance3D in [parent,child]:
		var material := ShaderMaterial.new();material.shader=Shader.new()
		material.shader.code="shader_type spatial; uniform int detail_quality = 1; uniform float wind_strength = 0.5;"
		material.set_meta("mapkit_surface",true)
		material.set_meta("mapkit_opaque",true)
		node.material_override=material
	for level in 3:
		QUALITY.current=QUALITY.profile(level)
		var snapshot := QUALITY.active()
		snapshot.lod_bias=-1.0
		check(QUALITY.active().lod_bias==QUALITY.profile(level).lod_bias,"active profile is an independent copy")
		RENDER.track_resources(job,parent,true)
		check(child.get_child_count()==1,"tracking creates one occluder before child traversal")
		var occluder: OccluderInstance3D=child.get_child(0)
		var expected := QUALITY.profile(level)
		for repeat in 2:
			if repeat==1: RENDER.track_resources(job,parent,true)
			for node: MeshInstance3D in [parent,child]:
				check(is_equal_approx(node.lod_bias,expected.lod_bias),"recursive node LOD follows each call's quality")
				check(node.material_override.get_shader_parameter("detail_quality")==level and node.material_override.get_shader_parameter("wind_strength")==expected.vegetation_motion,"recursive material quality applies before tracking and repairs external edits")
			check(parent.visibility_range_end==expected.decoration_distance and parent.visibility_range_end_margin==8.0 and child.visibility_range_end==0.0,"recursive quality preserves decoration versus solid distance")
			check(occluder.visible==expected.occlusion,"new and existing occluder visibility follows each call's quality")
			if repeat==0:
				for node: MeshInstance3D in [parent,child]:
					node.lod_bias=9.0
					node.material_override.set_shader_parameter("detail_quality",-1)
					node.material_override.set_shader_parameter("wind_strength",-1.0)
				parent.visibility_range_end=1.0
				occluder.visible=not expected.occlusion
	var held: ArrayOccluder3D=child.get_child(0).occluder
	var tracked := lease.refs.map(func(value): return value.get_ref())
	check(tracked.slice(0,8)==[parent,parent.material_override,parent.material_override.shader,held,child.get_child(0),child.mesh,child.material_override,child.material_override.shader],"lease callback order and root-only shared mesh ownership survive")
	check(not parent.mesh in tracked,"shared root mesh does not hold the cell lease")
	tracked.clear()
	parent.free();job.clear();lease.seal()
	for _frame in 4: await process_frame
	check(not lease.retired(),"retained occluder shape keeps tracked lease alive")
	held=null
	for _frame in 4: await process_frame
	check(lease.retired(),"last tracked borrower releases lease")
	# Standalone callers still get immediate root quality and occluder visibility.
	var preview := MeshInstance3D.new();preview.mesh=BoxMesh.new();preview.mesh.size=Vector3(8,4,.5)
	preview.set_meta("mapkit_occluder",true)
	QUALITY.current=QUALITY.profile(0)
	RENDER.track_resources({},preview)
	check(is_equal_approx(preview.lod_bias,QUALITY.active().lod_bias) and not preview.get_child(0).visible,"lease-free preview applies quality before returning")
	preview.free()

func instance_quality() -> void:
	var template := Node3D.new();template.position=Vector3(1,0,2)
	var branch := Node3D.new();branch.scale=Vector3(-1,1.5,1);template.add_child(branch)
	for index in 3:
		var piece := MeshInstance3D.new();piece.mesh=BoxMesh.new()
		piece.mesh.size=Vector3(8,4,.5) if index<2 else Vector3.ONE
		piece.position=Vector3(index*10,0,0)
		var material := ShaderMaterial.new();material.shader=Shader.new()
		material.shader.code="shader_type spatial; uniform int detail_quality = 1; uniform float wind_strength = 0.5;"
		material.set_meta("mapkit_surface",true);material.set_meta("mapkit_opaque",true)
		piece.material_override=material
		branch.add_child(piece)
	for level in 3:
		var scene := Node3D.new();root.add_child(scene)
		var lease := Lease.new()
		QUALITY.current=QUALITY.profile(level)
		var group := INSTANCES.begin(template,2,scene,lease,false)
		check(group.groups.size()==3,"nested template emits each part once")
		for part: Dictionary in group.groups:
			var node: MultiMeshInstance3D=part.node
			var decoration: bool=node.get_meta("mapkit_decoration")
			check(is_equal_approx(node.lod_bias,QUALITY.current.lod_bias) and node.visibility_range_end==(QUALITY.current.decoration_distance if decoration else 0.0),"all new parts receive current LOD and decoration distance")
			check(node.material_override.get_shader_parameter("detail_quality")==level and node.material_override.get_shader_parameter("wind_strength")==QUALITY.current.vegetation_motion,"all new parts receive current material quality")
		# Zero, one and two placements exercise cancellation before/after append and completion.
		for index in level:
			QUALITY.current=QUALITY.profile((level+index+1)%3)
			var pose := Transform3D(Basis(Vector3.UP,.37+index),Vector3(5+index*20,1,3))
			var object_id := "object-%d"%index
			INSTANCES.append(group,pose,object_id,"fixture-map")
			var expected_seed := float(("fixture-map/"+object_id).sha256_text().substr(0,6).hex_to_int())/16777215.0
			for part: Dictionary in group.groups:
				var multi: MultiMesh=part.multi
				check(multi.get_instance_transform(index).is_equal_approx(pose*part.transform),"every part retains nested reflection and placement transform")
				check(multi.get_instance_color(index)==Color.WHITE and multi.get_instance_custom_data(index).is_equal_approx(Color(expected_seed,0,0,1)),"all parts retain the map/object seed and white tint")
				check(multi.visible_instance_count==index+1,"each part publishes the same populated count")
				if not part.node.get_meta("mapkit_decoration"):
					var occluder: OccluderInstance3D=part.node.get_child(index)
					check(occluder.visible==QUALITY.current.occlusion and occluder.transform.is_equal_approx(pose*part.transform),"new occluders read quality at placement and retain each part's pose")
				else: check(part.node.get_child_count()==0,"small decoration skips occluder creation")
				multi=null # Do not let the validator itself retain a placement borrower.
			check(group.next==index+1 and group.ids[index]==object_id,"placement identity and progress remain per object")
		for part: Dictionary in group.groups:
			part.node.lod_bias=9.0
			part.node.material_override.set_shader_parameter("detail_quality",-1)
		QUALITY.apply_tree(scene,QUALITY.current)
		for part: Dictionary in group.groups:
			check(is_equal_approx(part.node.lod_bias,QUALITY.current.lod_bias) and part.node.material_override.get_shader_parameter("detail_quality")==QUALITY.current.material_detail,"live quality repairs existing part state at the same preset")
			for occluder: OccluderInstance3D in part.node.get_children():
				check(occluder.visible==QUALITY.current.occlusion,"live quality updates all earlier placements")
		var held: MultiMesh=group.groups[0].multi
		check(lease.refs[0].get_ref()==held and lease.refs[1].get_ref()==group.groups[0].node,"MultiMesh and node tracking order preserved")
		# Drop test-owned part containers before awaiting resource retirement.
		for part: Dictionary in group.groups: part.clear()
		group.groups.clear()
		scene.free();group.clear();lease.seal()
		for _frame in 4: await process_frame
		check(not lease.retired(),"retained MultiMesh keeps cancelled/completed group charged")
		held=null
		for _frame in 4: await process_frame
		check(lease.retired(),"group retires with shared template alive: level=%d live=%s"%[level,lease.refs.filter(func(value): return value.get_ref()!=null).map(func(value): return value.get_ref().get_class())])
	template.free()

func occluder_reuse(scene: Node3D) -> void:
	# Two walls and a lintel leave a central doorway, with indexed opaque faces.
	var points := PackedVector3Array()
	var indices := PackedInt32Array()
	for rect: Rect2 in [Rect2(-4,0,3,4),Rect2(1,0,3,4),Rect2(-1,2,2,2)]:
		var offset := points.size()
		points.append_array(PackedVector3Array([Vector3(rect.position.x,rect.position.y,0),Vector3(rect.end.x,rect.position.y,0),Vector3(rect.end.x,rect.end.y,0),Vector3(rect.position.x,rect.end.y,0)]))
		indices.append_array(PackedInt32Array([offset,offset+1,offset+2,offset,offset+2,offset+3]))
	var arrays := [];arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX]=points;arrays[Mesh.ARRAY_INDEX]=indices
	var facade := ArrayMesh.new();facade.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
	# Large transparent/vegetation faces across the opening must remain excluded.
	var pane := BoxMesh.new();pane.size=Vector3(8,4,.1)
	for vegetation in [false,true]:
		var material := StandardMaterial3D.new()
		if vegetation: material.set_meta("mapkit_vegetation",true)
		else: material.transparency=BaseMaterial3D.TRANSPARENCY_ALPHA
		facade.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,pane.get_mesh_arrays())
		facade.surface_set_material(facade.get_surface_count()-1,material)
	var original := QUALITY.occluder(facade,null)
	var expected_vertices: PackedVector3Array=original.occluder.vertices
	var expected_indices: PackedInt32Array=original.occluder.indices
	check(expected_vertices.size()==18,"only opaque doorway walls contribute occlusion")
	var lod_job := QUALITY.begin_lods(facade,true)
	while not QUALITY.advance_lods(lod_job): pass
	var operations := []
	QUALITY.prepare_occluder(lod_job.result,null,lod_job.occluder_arrays,func(phase,_elapsed,surface): operations.append([phase,surface]))
	var retained := QUALITY.occluder(lod_job.result,null)
	check(retained.occluder.vertices==expected_vertices and retained.occluder.indices==expected_indices,"retained LOD positions preserve doorway faces, ordering and inset exactly")
	check(operations==[["occluder_cached_arrays",0]],"opaque faces use CPU arrays and excluded surfaces perform no readback")
	for surface in facade.get_surface_count():
		var displayed: Array=lod_job.result.surface_get_arrays(surface)
		check(lod_job.occluder_arrays[surface]==[displayed[Mesh.ARRAY_VERTEX],displayed[Mesh.ARRAY_INDEX]],"retained CPU faces match uploaded LOD output exactly")
	retained.free();lod_job={}
	QUALITY.prepare_occluder(facade,null)
	var first := QUALITY.occluder(facade,null)
	QUALITY.prepare_occluder(facade,null)
	var second := QUALITY.occluder(facade,null)
	check(first!=second and first.occluder==second.occluder,"independent placements reuse one immutable shape")
	check(first.occluder.vertices==expected_vertices and first.occluder.indices==expected_indices,"prepared occluder preserves every ordered face and inset")
	for i in range(0,expected_vertices.size(),3):
		var a: Vector3=expected_vertices[i];var b: Vector3=expected_vertices[i+1];var c: Vector3=expected_vertices[i+2]
		check(maxf(a.x,maxf(b.x,c.x))<=-1 or minf(a.x,minf(b.x,c.x))>=1 or minf(a.y,minf(b.y,c.y))>=2,"no occluder triangle bridges the doorway")
	for level in 3:
		QUALITY.apply_node(first,QUALITY.profile(level))
		check(first.visible==bool(QUALITY.profile(level).occlusion),"shared shape preserves per-instance quality visibility")
	original.free();first.free();second.free()
	# Opaque instancing path retains nested scale, reflection and placement yaw.
	var opaque := ArrayMesh.new();opaque.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
	QUALITY.prepare_occluder(opaque,null)
	var template := MeshInstance3D.new();template.mesh=opaque
	template.scale=Vector3(-1.5,2,1);template.position=Vector3(1,0,2)
	var group := INSTANCES.begin(template,2,scene,null)
	var pose := Transform3D(Basis(Vector3.UP,.43),Vector3(5,0,7))
	INSTANCES.append(group,pose,"first")
	INSTANCES.append(group,Transform3D.IDENTITY,"second")
	var children: Array[Node]=group.groups[0].node.get_children()
	check(children.size()==2 and children[0].occluder==children[1].occluder,"MultiMesh placements borrow the prepared shape")
	check(children[0].transform.is_equal_approx(pose*template.transform),"occlusion follows reflected nested mesh and placement transform")
	check(opaque.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]==points and opaque.surface_get_arrays(0)[Mesh.ARRAY_INDEX]==indices,"preparation does not alter display geometry")
	template.free();group.clear();children.clear()
	var ground := PlaneMesh.new();ground.size=Vector2(8,8)
	QUALITY.prepare_occluder(ground,null)
	check(ground.has_meta("mapkit_occluder_preparation") and QUALITY.occluder(ground,null)==null,"empty occluder result is prepared and reused")
	# Unprepared mutable previews must keep observing their latest geometry.
	var mutable := BoxMesh.new();mutable.size=Vector3(8,4,.1)
	var before := QUALITY.occluder(mutable,null)
	mutable.size=Vector3(10,6,.1)
	var after := QUALITY.occluder(mutable,null)
	check(before.occluder.vertices!=after.occluder.vertices and not mutable.has_meta("mapkit_occluder_preparation"),"uncached preview reflects mesh edits")
	before.free();after.free()

func occluder_coverage(scene: Node3D) -> void:
	var vertices := PackedVector3Array()
	for index in 100:
		var x := float(index%10)*3.0
		var y := float(index/10)*2.0
		vertices.append_array(PackedVector3Array([Vector3(x,y,0),Vector3(x+2,y,0),Vector3(x,y+2,0)]))
	vertices.append_array(PackedVector3Array([Vector3(0,30,0),Vector3(20,30,0),Vector3(0,45,0)]))
	var arrays := [];arrays.resize(Mesh.ARRAY_MAX);arrays[Mesh.ARRAY_VERTEX]=vertices
	var mesh := ArrayMesh.new();mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
	var shape := QUALITY._occluder_shape(mesh)
	check(shape.vertices.size()==192,"occluder selection retains the existing 64-face output bound")
	var highest := 0.0
	for vertex: Vector3 in shape.vertices: highest=maxf(highest,vertex.y)
	check(highest>40.0,"late upper-storey wall survives dense lower facade faces")
	var node := MeshInstance3D.new();node.mesh=mesh;node.set_meta("mapkit_occluder",true);scene.add_child(node)
	var alpha := StandardMaterial3D.new();alpha.transparency=BaseMaterial3D.TRANSPARENCY_ALPHA
	node.material_override=alpha
	RENDER.track_resources({},node)
	check(node.get_child_count()==0,"transparent node override cannot turn an opaque source into a false occluder")
	node.material_override=null;node.set_surface_override_material(0,alpha)
	RENDER.track_resources({},node)
	check(node.get_child_count()==0,"transparent per-surface override also excludes the ordinary-instance occluder")
	node.set_surface_override_material(0,null)
	RENDER.track_resources({},node)
	check(node.get_child_count()==1,"opaque source can create its conservative occluder")
	node.material_override=alpha
	QUALITY.apply_tree(node,QUALITY.profile(2))
	check(not node.get_child(0).visible,"mutable preview override hides a previously created occluder")
	mesh.surface_set_material(0,ShaderMaterial.new())
	check(QUALITY._occluder_shape(mesh)==null,"unknown shader opacity never blocks the camera")
	node.free()
