extends SceneTree
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
const WATER := preload("../water_renderer.gd")
const INSTANCES := preload("../render_instances.gd")
var failed := false
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
	var scene := Node3D.new();root.add_child(scene)
	var mesh := SphereMesh.new();mesh.radial_segments=48;mesh.rings=24
	var lod := QUALITY.prepare_lods(mesh)
	var importer := ImporterMesh.from_mesh(lod)
	check(importer.get_surface_lod_count(0)>0,"template cache contains actual engine LOD index buffers")
	check(QUALITY.prepare_lods(lod)==lod,"immutable LOD mesh reused")
	var template := MeshInstance3D.new();template.mesh=lod
	var group := INSTANCES.begin(template,2,scene,null)
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
	var metre_scaled := INSTANCES.begin(single_template,1,scene,null)
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
