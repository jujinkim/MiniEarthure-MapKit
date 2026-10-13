extends RefCounted
## Session-owned immutable resources. Policy/admission is supplied by the caller.
const ASSETS := preload("./asset_library.gd")
const URBAN := preload("./urban_surface.gdshader")
const ENVIRONMENT := preload("./environment_materials.gd")
const QUALITY := preload("./display_quality.gd")
const INSTANCES := preload("./render_instances.gd")
const CONTEXT_BYTES := ENVIRONMENT.MEMORY_BYTES
# Fixed 16-segment/8-ring canopy: source, importer scratch, CPU/GPU output and
# LOD indices overlap. Independent of asset content and material bindings.
const TREE_CANOPY_KEY := "builtin:tree:canopy"
const TREE_CANOPY_BYTES := 131072
var reserve: Callable
var limit_bytes := 128 * 1024 * 1024 # Shared resources; still charged to consumer admission.
var entries: Dictionary = {}
var retiring: Array = []
var imports := 0
var hits := 0
var import_usec := 0
var peak_import_usec := 0
var preparation_usec: Dictionary = {}
var preparation_peak_usec: Dictionary = {}
var mesh_observer: Callable # Opt-in diagnostics; never retains resource owners.
var _shader: Shader
var _shader_lease: RefCounted
var closed := false
var _environment: RefCounted
var _environment_pending := false
var _environment_failed := false
var _environment_lease: RefCounted
var environment_profile: Dictionary = {}
var quality_profile: Dictionary = preload("./display_quality.gd").active()


func _init(admission: Callable = Callable()) -> void: reserve = admission

func bytes() -> int:
	retiring = retiring.filter(func(lease: RefCounted): return not lease.retired())
	var total := 0
	if _shader_lease != null: total += int(_shader_lease.bytes)
	elif _shader != null: total += 65536
	if _environment_lease != null: total += int(_environment_lease.bytes)
	elif _environment != null: total += _context_bytes()
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
	return str(asset.content_hash) + ":" + str(asset.material_json) + ":" + texture_hash + ":" + str(asset.get("solid",true))

func environment_key_for(id: String, sources: Dictionary) -> String:
	return _key_with_binding(key_for(id,sources),_light_binding_for(id))

func _light_binding_for(id: String) -> Dictionary:
	# Cell sources carry only their relevant light bindings. Key this asset by
	# its own binding so adjacent cells still share the same imported template.
	var binding := {}
	for light: Dictionary in environment_profile.get("lights",[]):
		if light.asset_id == id: binding = light
	return binding

static func _key_with_binding(key: String, binding: Dictionary) -> String:
	return "" if key.is_empty() else key + ":" + JSON.stringify(binding, "", true).sha256_text()

func claim(id: String, sources: Dictionary) -> String:
	if closed: return ""
	var source_key := key_for(id,sources)
	if source_key.is_empty(): return ""
	var binding := _light_binding_for(id)
	var key := _key_with_binding(source_key,binding)
	var fresh := not entries.has(key)
	# Snapshot exactly the binding used by the key. Another cell can replace the
	# current profile between admitted steps; nested authored lists may change too.
	if fresh: binding = binding.duplicate(true)
	# The native import profile already includes CPU/GPU overlap (256 bytes per
	# vertex, 32 per index, 24 per image pixel). Replacing template meshes shares
	# textures and releases the old mesh; add a material/clock binding allowance.
	# The doubled native 8 KiB/node allowance also covers one small part/bounds
	# record per mesh node, borrowing the already tracked mesh/material.
	var amount := int(sources[id].memory_bytes) * 2 + 131072 # LOD generation, retained indices and occluders, including import overlap.
	var claimed := _claim(key,amount,"context")
	if fresh and not claimed.is_empty():
		entries[claimed].light_binding = binding
		# Validated lights have at most 64 window + 16 bulb indices. The small
		# integer lookup fits the existing 128 KiB material/clock allowance.
		entries[claimed].light_roles = ENVIRONMENT.prepare_light_roles(binding)
	return claimed

func claim_tree_canopy() -> String:
	return _claim(TREE_CANOPY_KEY,TREE_CANOPY_BYTES,"canopy_source")

func _claim(key: String, amount: int, stage: String) -> String:
	if closed: return ""
	if entries.has(key):
		entries[key].pins += 1
		hits += 1
		return key
	if bytes() + amount > limit_bytes or entries.size() >= 256: trim()
	if bytes() + amount > limit_bytes or entries.size() >= 256: return ""
	var lease: RefCounted = reserve.call(amount) if reserve.is_valid() else null
	if reserve.is_valid() and lease == null: return ""
	entries[key] = {"pins": 1, "bytes": amount, "lease": lease, "template": null, "materials": {}, "stage":stage, "pending":[], "failed":false, "last_used": Time.get_ticks_msec()}
	return key

static func tree_canopy_source() -> SphereMesh:
	var shape := SphereMesh.new()
	shape.radius = 1.84
	shape.height = 4.0
	shape.radial_segments = 16
	shape.rings = 8
	return shape

func prepare_tree_canopy(key: String) -> bool:
	if closed or not entries.has(key): return true
	var item: Dictionary = entries[key]
	if item.stage == "done": return true
	var stage: String = "canopy_source" if item.stage == "canopy_source" else "canopy_lod_"+str(item.lod_job.stage)
	var started := Time.get_ticks_usec()
	if item.stage == "canopy_source":
		item.lod_job = QUALITY.begin_lods(tree_canopy_source())
		if item.lease != null:
			item.lease.track(item.lod_job.input)
			item.lease.track(item.lod_job.source)
		item.stage = "canopy_lods"
	elif QUALITY.advance_lods(item.lod_job,mesh_observer):
		item.mesh = item.lod_job.result
		if item.lease != null: item.lease.track(item.mesh)
		item.erase("lod_job")
		item.stage = "done"
	var elapsed := Time.get_ticks_usec()-started
	preparation_usec[stage] = int(preparation_usec.get(stage,0))+elapsed
	preparation_peak_usec[stage] = maxi(int(preparation_peak_usec.get(stage,0)),elapsed)
	return item.stage == "done"

func tree_canopy(key: String) -> Mesh:
	if closed or not entries.has(key) or entries[key].stage != "done": return null
	return entries[key].get("mesh")

func prepare_template(key: String, id: String, sources: Dictionary) -> bool:
	# One admitted phase per call; the scheduler owns the shared frame budget.
	# Claims prevent eviction until every cell waiting on this template finishes.
	if closed or not entries.has(key): return true
	var item: Dictionary = entries[key]
	if item.stage == "done": return true
	var stage: String = item.stage
	var diagnostic_stage := "lod_"+str(item.lod_job.stage) if stage == "lod_prepare" else stage
	var started := Time.get_ticks_usec()
	if stage == "import":
		var prepare_material: Callable = _environment.import_surface.bind(item.light_binding,item.light_roles) if _environment != null else Callable()
		item.template = ASSETS.template(id, sources, item.materials, prepare_material)
		imports += 1
		item.failed = item.template == null
		if item.failed: item.stage = "done"
		else:
			_track_node(item.lease, item.template)
			item.pending = [item.template]
			item.stage = "lod"
	elif stage == "context":
		# Shared texture upload/context setup is separate from GLB and LOD work.
		if not prepare_environment(): return false
		item.stage = "import"
	elif stage == "lod":
		if item.pending.is_empty(): item.stage = "instance_parts"
		else:
			var node: Node3D = item.pending.pop_back()
			item.pending.append_array(node.get_children())
			if node is MeshInstance3D:
				var mesh_size: float = node.mesh.get_aabb().size.length()
				node.set_meta("mapkit_decoration",not bool(sources[id].get("solid",true)) and mesh_size <= 6.0)
				node.set_meta("mapkit_occluder",mesh_size > 6.0)
				item.styling = node
				item.lod_job = QUALITY.begin_lods(node.mesh,true)
				item.stage = "lod_prepare"
	elif stage == "lod_prepare":
		if QUALITY.advance_lods(item.lod_job,mesh_observer):
			item.styling.mesh = item.lod_job.result
			item.owned_mesh = item.lod_job.owned
			item.occluder_arrays = item.lod_job.get("occluder_arrays",[])
			item.erase("lod_job")
			# Import already tracked the hierarchy and its bindings. LOD output
			# preserves those material references; only its new mesh needs a lease.
			if item.owned_mesh and item.lease != null: item.lease.track(item.styling.mesh)
			item.stage = "materials"
	elif stage == "materials":
		var node: MeshInstance3D = item.styling
		if _environment != null:
			# New LOD output belongs solely to this unpublished template. Rebinding
			# its surfaces avoids a second whole-mesh GPU readback/copy.
			if not item.owned_mesh:
				var copy_started := Time.get_ticks_usec()
				node.mesh = node.mesh.duplicate()
				QUALITY.observe_mesh(mesh_observer,"material_duplicate",copy_started)
				# Shallow duplication retains the tracked materials and textures.
				if item.lease != null: item.lease.track(node.mesh)
			item.material_surface = 0
			# LOD output is final; admitted material steps only replace bindings.
			item.material_count = node.mesh.get_surface_count()
			item.water = item.material_count > 0
			item.stage = "material_surface"
		else: item.stage = "occluder"
		item.erase("owned_mesh")
	elif stage == "material_surface":
		var node: MeshInstance3D = item.styling
		var index: int = item.material_surface
		if index < item.material_count:
			var style_started := Time.get_ticks_usec()
			var replaced: bool = _environment.style_surface(node.mesh,item.light_binding,index,false,mesh_observer,item.light_roles)
			# Include callee-local retirement after its inner binding timer returns.
			QUALITY.observe_mesh(mesh_observer,"material_style",style_started,index)
			var track_started := Time.get_ticks_usec()
			var material: Material = node.mesh.surface_get_material(index)
			# Pending shader activation and passthrough keep the imported binding
			# and source bitmaps. The context already owns the activated shader.
			if replaced: _track_material(item.lease,material)
			QUALITY.observe_mesh(mesh_observer,"material_track",track_started,index)
			item.water = item.water and material != null and material.get_meta("mapkit_water",false)
			item.material_surface += 1
		if item.material_surface == item.material_count:
			var finalize_started := Time.get_ticks_usec()
			node.extra_cull_margin = .08
			if item.water: node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
			if node.material_override != null:
				node.material_override = _environment.surface_material(node.material_override,0)
				_track_material(item.lease,node.material_override)
			item.erase("material_surface");item.erase("material_count");item.erase("water")
			QUALITY.observe_mesh(mesh_observer,"material_finalize",finalize_started)
			item.stage = "occluder"
	elif stage == "occluder":
		# Derive once after LOD/material replacement, as its own admitted step.
		# Root/placement scaling can make even a small mesh an occluder candidate.
		var node: MeshInstance3D = item.styling
		QUALITY.prepare_occluder(node.mesh,item.lease,item.occluder_arrays,mesh_observer)
		item.erase("occluder_arrays")
		item.erase("styling")
		item.stage = "lod"
	elif stage == "instance_parts":
		# Publish only after all LODs, bindings and occluders are final. Empty
		# results are reusable too (transparent/override fallback).
		var pieces := INSTANCES.parts(item.template)
		for piece: Dictionary in pieces:
			piece.mesh_bounds = piece.mesh.get_aabb()
			piece.make_read_only()
		pieces.make_read_only()
		item.instance_parts = pieces
		item.stage = "done"
	if item.stage == "done":
		item.erase("light_binding")
		item.erase("light_roles")
	var elapsed := Time.get_ticks_usec() - started
	if stage != "context":
		preparation_usec[diagnostic_stage] = int(preparation_usec.get(diagnostic_stage,0)) + elapsed
		preparation_peak_usec[diagnostic_stage] = maxi(int(preparation_peak_usec.get(diagnostic_stage,0)),elapsed)
	if stage == "import":
		import_usec += elapsed
		peak_import_usec = maxi(peak_import_usec,elapsed)
	return item.stage == "done"

func template(key: String, id: String, sources: Dictionary) -> Node3D:
	# Synchronous convenience for standalone previews. Streaming calls the phased
	# API first; no phase is repeated when another cell shares the same claim.
	if closed or not entries.has(key): return null
	while not prepare_template(key,id,sources): pass
	return entries[key].template

func instance_parts(key: String) -> Array:
	# Never advance or expose a partially prepared template through this accessor.
	if closed or not entries.has(key) or entries[key].stage != "done": return []
	return entries[key].get("instance_parts",[])

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
	item.pending.clear()
	item.erase("styling")
	item.erase("lod_job")
	item.erase("mesh")
	item.erase("occluder_arrays")
	item.erase("light_binding")
	item.erase("light_roles")
	item.erase("material_count")
	item.erase("instance_parts")
	if item.lease != null:
		item.lease.seal()
		retiring.append(item.lease)

func shutdown() -> void:
	closed = true
	mesh_observer = Callable()
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
		_track_material(lease, node.material_overlay)
		for i in node.mesh.get_surface_count():
			_track_material(lease, node.mesh.surface_get_material(i))
			_track_material(lease, node.get_surface_override_material(i))
	for child: Node in node.get_children(): _track_node(lease, child)

static func _track_material(lease: RefCounted, material: Material) -> void:
	if lease == null or material == null: return
	lease.track(material)
	if material is ShaderMaterial:
		# Context owns the shared shader/state texture; this lease owns the source bitmap.
		for slot: String in ["albedo_texture","normal_texture","ao_texture","roughness_texture","metallic_texture"]:
			var bitmap: Variant = material.get_shader_parameter(slot)
			if bitmap is Texture2D: lease.track(bitmap)
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: lease.track(texture)

func diagnostics() -> Dictionary:
	return {"entries": entries.size(), "bytes": bytes(), "imports": imports, "hits": hits, "import_usec": import_usec, "peak_import_usec": peak_import_usec, "preparation_usec":preparation_usec.duplicate(), "preparation_peak_usec":preparation_peak_usec.duplicate()}

func begin_environment() -> bool:
	if closed or _environment_failed: return false
	if environment_profile.is_empty(): return true
	if _environment != null: return true
	if bytes() + _context_bytes() > limit_bytes: trim()
	if bytes() + _context_bytes() > limit_bytes: return false
	_environment_lease = reserve.call(_context_bytes()) if reserve.is_valid() else null
	if reserve.is_valid() and _environment_lease == null: return false
	_environment = ENVIRONMENT.new(int(quality_profile.texture_size),true)
	_environment.track(_environment_lease)
	_environment_pending = true
	return true

func prepare_environment() -> bool:
	if not begin_environment(): return true
	if not _environment_pending: return true
	var phase := "texture_load" if _environment.tile_image == null else "mipmap_upload"
	var started := Time.get_ticks_usec()
	var done: bool = _environment.prepare_tile(_environment_lease)
	var elapsed := Time.get_ticks_usec() - started
	preparation_usec[phase] = int(preparation_usec.get(phase,0)) + elapsed
	preparation_peak_usec[phase] = maxi(int(preparation_peak_usec.get(phase,0)),elapsed)
	if done:
		_environment_pending = false
		if _environment.load_failed:
			_environment_failed = true
			_environment = null
			if _environment_lease != null:
				_environment_lease.seal()
				retiring.append(_environment_lease)
				_environment_lease = null
	return done

func environment_context() -> RefCounted:
	# Observers must not collapse streaming preparation into a single frame.
	if _environment_pending: return null
	if _environment == null:
		if not begin_environment(): return null
		while not prepare_environment(): pass # standalone synchronous convenience
	return _environment

static func wet_urban_shader() -> Shader:
	var result := Shader.new()
	result.code = URBAN.code.replace('#include "wet_surface.gdshaderinc"',preload("./wet_surface.gdshaderinc").code).replace('#include "material_detail.gdshaderinc"',preload("./material_detail.gdshaderinc").code)
	return result

func _context_bytes() -> int:
	return preload("./display_quality.gd").tile_bytes(int(quality_profile.texture_size), ENVIRONMENT.TILE_KINDS.size())
