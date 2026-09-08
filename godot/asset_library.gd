extends RefCounted
## Only the validated native presentation view reaches this display adapter.
## No extraction, paths, ResourceLoader, scene scripts or GLTF extensions.

static func material(id: String, source: Dictionary, cache: Dictionary) -> StandardMaterial3D:
	if cache.has(id): return cache[id]
	if not source.has(id): return null
	var asset: Dictionary = source[id]
	var result := StandardMaterial3D.new()
	result.roughness = 0.9
	var descriptor: Variant = JSON.parse_string(asset.material_json)
	var texture_id := ""
	if not str(asset.path).ends_with(".glb"): texture_id = id
	if descriptor is Dictionary:
		var c: Array = descriptor.albedo_rgba
		result.albedo_color = Color8(int(c[0]), int(c[1]), int(c[2]), int(c[3]))
		result.metallic = float(descriptor.metallic_per_mille) / 1000.0
		result.roughness = float(descriptor.roughness_per_mille) / 1000.0
		result.cull_mode = BaseMaterial3D.CULL_DISABLED if descriptor.double_sided else BaseMaterial3D.CULL_BACK
		texture_id = str(descriptor.get("albedo_texture", texture_id))
	if not texture_id.is_empty():
		if not source.has(texture_id): return null
		var texture: Dictionary = source[texture_id]
		var image := Image.new()
		var error := image.load_png_from_buffer(texture.bytes) if str(texture.path).ends_with(".png") else image.load_webp_from_buffer(texture.bytes)
		if error != OK or image.is_empty(): return null
		result.albedo_texture = ImageTexture.create_from_image(image)
		if image.detect_alpha() != Image.ALPHA_NONE: result.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	if result.albedo_color.a < 1.0: result.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	cache[id] = result
	return result

static func template(id: String, source: Dictionary, materials: Dictionary) -> Node3D:
	if not source.has(id) or not str(source[id].path).ends_with(".glb"): return null
	if source[id].bytes.size() < 28: return null
	var document := GLTFDocument.new()
	var state := GLTFState.new()
	# Embedded image textures stay in memory. Native validation has rejected every URI.
	state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED
	if document.append_from_buffer(source[id].bytes, "", state, GLTFDocument.IMPORT_FLAG_GENERATE_TANGENT_ARRAYS) != OK: return null
	var root := document.generate_scene(state)
	if root == null: return null
	var pending: Array[Node] = [root]
	var meshes := 0
	var override: StandardMaterial3D
	if source[id].material_json != "null":
		override = material(id, source, materials)
		if override == null:
			root.free()
			return null
	while not pending.is_empty():
		var node := pending.pop_back() as Node
		# Defense at the engine boundary, including importer-created name-suffix nodes.
		if not node is Node3D or node.get_script() != null or (node.get_class() != "Node3D" and not node is MeshInstance3D):
			root.free()
			return null
		if node is MeshInstance3D:
			if node.mesh == null:
				root.free()
				return null
			meshes += 1
			if override != null: node.material_override = override
		for child: Node in node.get_children(): pending.append(child)
	if meshes == 0:
		root.free()
		return null
	return root as Node3D
