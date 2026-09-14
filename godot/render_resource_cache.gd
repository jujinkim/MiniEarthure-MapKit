extends RefCounted
## Session-owned immutable resources. Policy/admission is supplied by the caller.
const ASSETS := preload("./asset_library.gd")
const URBAN := preload("./urban_surface.gdshader")
var reserve: Callable
var limit_bytes := 64 * 1024 * 1024
var entries: Dictionary = {}
var retiring: Array = []
var imports := 0
var hits := 0
var import_usec := 0
var peak_import_usec := 0
var _shader: Shader
var _shader_lease: RefCounted
var closed := false

func _init(admission: Callable = Callable()) -> void: reserve = admission

func bytes() -> int:
	retiring = retiring.filter(func(lease: RefCounted): return not lease.retired())
	var total := 0
	if _shader_lease != null: total += int(_shader_lease.bytes)
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

func claim(id: String, sources: Dictionary) -> String:
	if closed: return ""
	var key := key_for(id, sources)
	if key.is_empty(): return ""
	if entries.has(key):
		entries[key].pins += 1
		hits += 1
		return key
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
		if item.template != null: _track_node(item.lease, item.template)
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
		_shader = URBAN.duplicate()
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
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: lease.track(texture)

func diagnostics() -> Dictionary:
	return {"entries": entries.size(), "bytes": bytes(), "imports": imports, "hits": hits, "import_usec": import_usec, "peak_import_usec": peak_import_usec}
