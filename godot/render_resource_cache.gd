extends RefCounted
## Session-owned immutable resources. Policy/admission is supplied by the caller.
const ASSETS := preload("./asset_library.gd")
const URBAN := preload("./urban_surface.gdshader")
var reserve: Callable
var limit_bytes := 128 * 1024 * 1024 # Textured templates; still charged to consumer admission.
var entries: Dictionary = {}
var retiring: Array = []
var imports := 0
var hits := 0
var import_usec := 0
var peak_import_usec := 0
var _shader: Shader
var _shader_lease: RefCounted
var closed := false
var _environment: RefCounted
var _environment_lease: RefCounted
var environment_profile: Dictionary = {}


func _init(admission: Callable = Callable()) -> void: reserve = admission

func bytes() -> int:
	retiring = retiring.filter(func(lease: RefCounted): return not lease.retired())
	var total := 0
	if _shader_lease != null: total += int(_shader_lease.bytes)
	if _environment_lease != null: total += int(_environment_lease.bytes)
	for item: Dictionary in entries.values(): total += int(item.bytes)
	for lease: RefCounted in retiring: total += int(lease.bytes)
	return total

static func key_for(id: String, sources: Dictionary) -> String:
	if not sources.has(id): return ""
	var asset: Dictionary = sources[id]
	if not asset.has("content_hash") or int(asset.get("memory_bytes", 0)) <= 0: return ""
	var texture_hash := ""
	var material: Variant = JSON.parse_string(str(asset.material_json))
	if material is Dictionary:
		var texture_id := str(material.get("albedo_texture", ""))
		if not texture_id.is_empty():
			if not sources.has(texture_id) or not sources[texture_id].has("content_hash"): return ""
			texture_hash = str(sources[texture_id].content_hash)
	return str(asset.content_hash) + ":" + str(asset.material_json) + ":" + texture_hash

func environment_key_for(id: String, sources: Dictionary) -> String:
	var key := key_for(id, sources)
	if key.is_empty(): return ""
	# Cell sources carry only their relevant light bindings. Key this asset by
	# its own binding so adjacent cells still share the same imported template.
	var binding := {}
	for light: Dictionary in environment_profile.get("lights",[]):
		if light.asset_id == id: binding = light
	key += ":" + JSON.stringify(binding, "", true).sha256_text()
	return key

func claim(id: String, sources: Dictionary) -> String:
	if closed: return ""
	var key := environment_key_for(id,sources)
	if key.is_empty(): return ""
	if entries.has(key):
		entries[key].pins += 1
		hits += 1
		return key
	# The native import profile already includes CPU/GPU overlap (256 bytes per
	# vertex, 32 per index, 24 per image pixel). Replacing template meshes shares
	# textures and releases the old mesh; add a material/clock binding allowance.
	var amount := int(sources[id].memory_bytes) + 65536
	if bytes() + amount > limit_bytes or entries.size() >= 256: trim()
	if bytes() + amount > limit_bytes or entries.size() >= 256: return ""
	var lease: RefCounted = reserve.call(amount) if reserve.is_valid() else null
	if reserve.is_valid() and lease == null: return ""
	entries[key] = {"pins": 1, "bytes": amount, "lease": lease, "template": null, "materials": {}, "last_used": Time.get_ticks_msec()}
	return key

func template(key: String, id: String, sources: Dictionary) -> Node3D:
	if not entries.has(key): return null
	var item: Dictionary = entries[key]
	if item.template == null:
		var started := Time.get_ticks_usec()
		item.template = ASSETS.template(id, sources, item.materials)
		var elapsed := Time.get_ticks_usec() - started
		import_usec += elapsed
		peak_import_usec = maxi(peak_import_usec, elapsed)
		imports += 1
		if item.template != null:
			var context := environment_context()
			if context != null:
				var binding := {}
				for light: Dictionary in environment_profile.get("lights",[]):
					if light.asset_id == id: binding = light
				var pending: Array = [item.template]
				while not pending.is_empty():
					var node: Node3D = pending.pop_back()
					if node is MeshInstance3D:
						node.mesh = context.style_mesh(node.mesh,binding)
						if node.material_override != null:
							node.material_override = context.surface_material(node.material_override,0)
					pending.append_array(node.get_children())
			_track_node(item.lease, item.template)
	return item.template

func material(key: String, id: String, sources: Dictionary) -> Material:
	if not entries.has(key): return null
	var item: Dictionary = entries[key]
	var value := ASSETS.material(id, sources, item.materials)
	_track_material(item.lease, value)
	return value

func urban_shader() -> Shader:
	if closed: return null
	if _shader == null:
		if bytes() + 65536 > limit_bytes: trim()
		if bytes() + 65536 > limit_bytes: return null
		_shader_lease = reserve.call(65536) if reserve.is_valid() else null
		if reserve.is_valid() and _shader_lease == null: return null
		_shader = wet_urban_shader()
		if _shader_lease != null: _shader_lease.track(_shader)
	return _shader

func release(key: String) -> void:
	if not entries.has(key): return
	entries[key].pins = maxi(0, int(entries[key].pins) - 1)
	entries[key].last_used = Time.get_ticks_msec()
	if closed and entries[key].pins == 0: _evict(key)

func trim() -> void:
	for key: String in entries.keys():
		if entries[key].pins == 0: _evict(key)

func _evict(key: String) -> void:
	var item: Dictionary = entries[key]
	entries.erase(key)
	if is_instance_valid(item.template): item.template.free()
	item.materials.clear()
	if item.lease != null:
		item.lease.seal()
		retiring.append(item.lease)

func shutdown() -> void:
	closed = true
	_environment = null
	if _environment_lease != null:
		_environment_lease.seal()
		retiring.append(_environment_lease)
		_environment_lease = null
	trim()
	_shader = null
	if _shader_lease != null:
		_shader_lease.seal()
		retiring.append(_shader_lease)
		_shader_lease = null

static func _track_node(lease: RefCounted, node: Node) -> void:
	if lease == null: return
	if node is MeshInstance3D:
		lease.track(node.mesh)
		_track_material(lease, node.material_override)
		for i in node.mesh.get_surface_count():
			_track_material(lease, node.mesh.surface_get_material(i))
			_track_material(lease, node.get_surface_override_material(i))
	for child: Node in node.get_children(): _track_node(lease, child)

static func _track_material(lease: RefCounted, material: Material) -> void:
	if lease == null or material == null: return
	lease.track(material)
	if material is ShaderMaterial:
		# Context owns the shared shader/state texture; this lease owns the source bitmap.
		var albedo: Variant = material.get_shader_parameter("albedo_texture")
		if albedo is Texture2D: lease.track(albedo)
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: lease.track(texture)

func diagnostics() -> Dictionary:
	return {"entries": entries.size(), "bytes": bytes(), "imports": imports, "hits": hits, "import_usec": import_usec, "peak_import_usec": peak_import_usec}

func environment_context() -> RefCounted:
	if closed or environment_profile.is_empty(): return null
	if _environment == null:
		if bytes() + 1048576 > limit_bytes: return null
		_environment_lease = reserve.call(1048576) if reserve.is_valid() else null
		if reserve.is_valid() and _environment_lease == null: return null
		_environment = preload("./environment_materials.gd").new()
		_environment.track(_environment_lease)
	return _environment

static func wet_urban_shader() -> Shader:
	var result := Shader.new()
	result.code = URBAN.code.replace('#include "wet_surface.gdshaderinc"',preload("./wet_surface.gdshaderinc").code)
	return result
