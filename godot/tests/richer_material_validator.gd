extends SceneTree
const MATERIALS := preload("../environment_materials.gd")
const CACHE := preload("../render_resource_cache.gd")
const QUALITY := preload("../display_quality.gd")
var failed := false
class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount: int) -> void: bytes=amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed=true
	func retired() -> bool: return sealed and refs.all(func(r): return r.get_ref()==null)
func check(value: bool, label: String) -> void:
	if not value: failed=true;push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var denied := CACHE.new(func(_bytes): return null)
	denied.environment_profile={"lights":[]}
	check(denied.environment_context()==null and denied.bytes()==0,"denial allocates no tiles or context")
	denied.shutdown()
	var phased := CACHE.new(func(bytes): return Lease.new(bytes))
	phased.environment_profile={"lights":[]}
	check(phased.begin_environment() and phased._environment.tiles.is_empty(),"streaming admission allocates no tile uploads")
	check(not phased.prepare_environment() and phased._environment.tile_image!=null,"one step decodes one tile")
	check(phased.environment_context()==null and phased._environment.tiles.is_empty(),"observer cannot synchronously finish partial preparation")
	phased.shutdown()
	for i in 3: await process_frame
	check(phased.bytes()==0,"partial tile cancellation retires decoded image and shared lease")
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile={"lights":[]}
	var context: RefCounted=cache.environment_context()
	check(context!=null and cache.bytes()==MATERIALS.MEMORY_BYTES,"shared texture charge before materialization")
	check(context.tiles.size()==10,"ten common 256px packed tiles")
	var source:=StandardMaterial3D.new()
	var picture:=Image.create(128,128,false,Image.FORMAT_RGBA8);picture.fill(Color(.5,.5,1,1))
	var bitmap:=ImageTexture.create_from_image(picture)
	source.albedo_texture=bitmap;source.normal_enabled=true;source.normal_texture=bitmap
	source.normal_scale=.43;source.ao_enabled=true;source.ao_texture=bitmap;source.ao_texture_channel=BaseMaterial3D.TEXTURE_CHANNEL_BLUE
	source.ao_on_uv2=true;source.ao_light_affect=.7
	source.roughness_texture=bitmap;source.roughness_texture_channel=BaseMaterial3D.TEXTURE_CHANNEL_GREEN
	source.metallic_texture=bitmap;source.metallic_texture_channel=BaseMaterial3D.TEXTURE_CHANNEL_RED
	source.uv1_scale=Vector3(2,3,1);source.uv1_offset=Vector3(.2,.3,0)
	var styled: ShaderMaterial=context.surface_material(source,0)
	for slot: String in ["albedo","normal","ao","roughness","metallic"]:
		check(styled.get_shader_parameter(slot+"_texture")==bitmap,slot+" original texture retained")
	check(styled.get_shader_parameter("normal_strength")==source.normal_scale,"normal scale")
	check(styled.get_shader_parameter("ao_channel")==Vector4(0,0,1,0) and styled.get_shader_parameter("ao_uv2"),"AO channel and UV2")
	check(styled.get_shader_parameter("roughness_channel")==Vector4(0,1,0,0) and styled.get_shader_parameter("metallic_channel")==Vector4(1,0,0,0),"packed GLB channels")
	check(styled.get_shader_parameter("uv_scale")==source.uv1_scale,"UV scale survives weather wrapper")
	var texture_lease:=Lease.new(128*128*24)
	CACHE._track_material(texture_lease,styled)
	check(texture_lease.refs.size()==6,"all five bitmap slots participate in lifetime accounting")
	var plain:=StandardMaterial3D.new();plain.resource_name="mk_brick"
	var tile_material: ShaderMaterial=context.surface_material(plain,0,true)
	var other:=StandardMaterial3D.new();other.resource_name="mk_brick"
	var tile_other: ShaderMaterial=context.surface_material(other,0)
	check(tile_material.get_shader_parameter("detail_tile")==tile_other.get_shader_parameter("detail_tile"),"templates share tile identity")
	context.update(72000,.8,.3,-.2,1080)
	check(is_equal_approx(context.pixels.get_pixel(0,0).r,.8),"weather remains shared")
	# Actual import tests the material names, metre UVs and night-window binding.
	var path: String=get_script().resource_path.get_base_dir().path_join("../../assets/richer-library/assets/shop-0.glb")
	var bytes:=FileAccess.get_file_as_bytes(path)
	var assets={"shop":{"path":"shop.glb","bytes":bytes,"material_json":"null","content_hash":bytes.hex_encode().sha256_text(),"memory_bytes":4194304}}
	var asset_denied := CACHE.new(func(_amount): return null)
	check(asset_denied.claim("shop",assets).is_empty() and asset_denied.entries.is_empty(),"admission denial creates no template or occluder")
	asset_denied.shutdown()
	await partial_occluder_cancel(assets)
	cache.environment_profile={"lights":[{"asset_id":"shop","window_materials":[4],"bulb_materials":[]}]}
	var key: String=cache.claim("shop",assets)
	var shared_key: String=cache.claim("shop",assets)
	check(shared_key==key,"simultaneous cells share the preparation claim")
	check(not cache.prepare_template(key,"shop",assets),"import yields before LOD/material/GPU preparation")
	var stages := []
	while cache.entries[key].stage != "done":
		stages.append(cache.entries[key].stage)
		cache.prepare_template(key,"shop",assets)
	var template: Node3D=cache.template(key,"shop",assets)
	check(stages.has("lod") and stages.has("materials") and stages.has("occluder") and cache.imports==1,"LOD, styling and occluder preparation use separate admitted steps")
	var preparation: Dictionary=cache.diagnostics().preparation_usec
	cache.release(shared_key)
	check(cache.template(key,"shop",assets)==template and cache.imports==1,"second consumer reuses prepared resources")
	check(cache.diagnostics().preparation_usec==preparation,"second consumer does not repeat derived preparation")
	check(template!=null,"actual GLB imports with shared textured materials")
	var held_shape: ArrayOccluder3D
	var template_bytes: int=cache.entries[key].bytes
	if template!=null:
		var pending: Array[Node]=[template];var details:=0
		while not pending.is_empty():
			var node: Node=pending.pop_back()
			if node is MeshInstance3D:
				check(node.mesh.has_meta("mapkit_occluder_preparation"),"every styled template mesh has a prepared result")
				var occluder := QUALITY.occluder(node.mesh,null)
				if occluder!=null:
					held_shape=occluder.occluder
					occluder.free()
				for i in node.mesh.get_surface_count():
					var mat: Material=node.mesh.surface_get_material(i)
					if mat is ShaderMaterial and mat.get_shader_parameter("detail_enabled"): details+=1
			pending.append_array(node.get_children())
		check(details>=3,"GLB material roles select brick/wood/stone shared tiles")
	check(held_shape!=null,"fixture lends a standalone occluder resource")
	# Compile both textured wrapper and urban shader on the real renderer.
	var mesh:=MeshInstance3D.new();mesh.mesh=BoxMesh.new();mesh.material_override=styled;root.add_child(mesh)
	var camera:=Camera3D.new();camera.position=Vector3(0,1,3);root.add_child(camera);camera.look_at(Vector3.ZERO)
	var road:=MeshInstance3D.new();road.mesh=PlaneMesh.new();road.position.y=-.6;root.add_child(road)
	var road_material:=ShaderMaterial.new();road_material.shader=cache.urban_shader();context.bind_detail(road_material,"asphalt");road.material_override=road_material
	for i in 4: await process_frame
	mesh.free();road.free();camera.free();road_material=null
	var held_tile: Texture2D=tile_material.get_shader_parameter("detail_tile")
	cache.release(key);cache.release(key);cache.shutdown()
	context=null;styled=null;tile_material=null;tile_other=null;template=null;source=null;plain=null;other=null;bitmap=null;picture=null
	for i in 6: await process_frame
	check(cache.bytes()==MATERIALS.MEMORY_BYTES+template_bytes,"borrowed tile and occluder retain their independent shared leases")
	held_tile=null
	for i in 6: await process_frame
	check(cache.bytes()==template_bytes,"standalone occluder borrower retains the template charge after mesh retirement")
	held_shape=null
	for i in 3: await process_frame
	check(cache.bytes()==0,"last shared borrower releases the complete charge")
	print("richer_material_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func partial_occluder_cancel(assets: Dictionary) -> void:
	for finish_shape in [false,true]:
		var cache := CACHE.new(func(bytes): return Lease.new(bytes))
		var key: String=cache.claim("shop",assets)
		var shared: String=cache.claim("shop",assets)
		while cache.entries[key].stage not in ["occluder","done"]:
			cache.prepare_template(key,"shop",assets)
		check(cache.entries[key].stage=="occluder","fixture reaches staged occluder preparation")
		if finish_shape: cache.prepare_template(key,"shop",assets)
		cache.shutdown();cache.release(key)
		check(cache.entries.has(shared),"one cancelled claim cannot evict another consumer's partial template")
		cache.release(shared)
		for i in 3: await process_frame
		check(cache.entries.is_empty() and cache.bytes()==0,"cancel before/after occluder preparation drops pending nodes and resources")
