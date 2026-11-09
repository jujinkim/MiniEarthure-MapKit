extends RefCounted
## Camera-local grass field: a dense carpet of blades on grass ground around the
## active camera, at fixed memory however many cells are loaded.
##
## Three rings of tiles (1, 2 and 4 m) surround the camera, each one
## MultiMesh per quadrant of a shared tile mesh whose slots the shader
## (vegetation_grass.gdshader) places from world-tile integer hashes, so
## blades are anchored to the world and never swim. A slot is a grass card:
## an upright quad whose columns each hold at most one curved blade the
## shader draws analytically (pixels between blades are discarded), a row of
## blades for four vertices. A ring snaps to its own tile grid as the camera
## moves; rings hand over by camera distance and every ring thins its cards
## with distance toward one projected blade cover (COVER), so the carpet stays
## even across the handovers. Blades stand
## only on grass ground: a toroidal Texture2DArray of 8 m blocks (32 x 32
## texels of 0.25 m, R32F) around the camera holds the ground height of grass
## ground triangles under each texel centre (EMPTY elsewhere), rebuilt
## incrementally on the CPU from the registered cells' indexed grass triangles
## (grass_scatter.gd): nearest blocks first, resumable inside a block, at most
## STEP_USEC per frame with the window's layer clears, each block uploaded as
## one 4 KiB layer. A quadrant without grass blocks is hidden, so streets cost
## nothing. The field also sets the lawn shade under its blades
## (grass_field_distance, grass_field_shade) on its cells' lawn materials, and
## clears it when it shows no blades.
##
## Tier (display_quality grass_level, the cache texture tier): low has no
## field; medium and high size the window (BLOCKS) and the rings (RINGS);
## adaptive quality may lower the live rings (and their reach, never past the
## live tier's grass_distance) without touching the window. Memory:
## memory_bytes(level), one claim per resource cache
## (render_resource_cache.grass_field), or a process-lifetime field for cells
## without a cache.

const SHADER := preload("./vegetation_grass.gdshader")
const QUALITY := preload("./display_quality.gd")
const PLAN := preload("./render_memory.gd")
const SCATTER := preload("./grass_scatter.gd")
## Shared-resource key of a cache's field (render_resource_cache.grass_field).
const KEY := "builtin:grass_field"
const BLOCK_METRES := PLAN.GRASS_BLOCK_METRES
const BLOCK_TEXELS := 32
const TEXELS_PER_METRE := BLOCK_TEXELS / BLOCK_METRES
const EMPTY := -1.0e6
const STEP_USEC := 1000
## Grass triangles rasterized between wall-time checks.
const STEP_TRIANGLES := 16
## Window blocks per axis by tier: centred on the camera's nearest block
## corner, so the window reaches (BLOCKS / 2 - 0.5) x 8 m >= grass_distance + 4 m.
const BLOCKS := [0, 8, 12]
## Rings by tier: tile size (m), card and flower slots per tile, card width
## (m), blade columns per card, and camera distance fades in (x..y, none when
## x < 0) and out (z..w; the last ring ends at grass_distance). Flowers per m2
## inside / outside flower patches: FLOWERS; the heads of a patch share one
## drift colour (white, yellow, marigold, petunia, pansy, salvia or cosmos,
## vegetation_grass.gdshader DRIFT), heads elsewhere mix white, yellow and
## violet.
const RINGS := [[], [
	{"tile": 1.0, "cards": 64, "flowers": 1, "width": 0.24, "columns": 12, "fade": Vector4(-1.0, 0.0, 4.5, 6.0)},
	{"tile": 2.0, "cards": 72, "flowers": 2, "width": 0.4, "columns": 10, "fade": Vector4(4.5, 6.0, 11.0, 14.0)},
	{"tile": 4.0, "cards": 88, "flowers": 0, "width": 0.8, "columns": 8, "fade": Vector4(11.0, 14.0, 19.0, 24.0)}], [
	{"tile": 1.0, "cards": 80, "flowers": 1, "width": 0.24, "columns": 12, "fade": Vector4(-1.0, 0.0, 5.0, 6.5)},
	{"tile": 2.0, "cards": 88, "flowers": 2, "width": 0.48, "columns": 12, "fade": Vector4(5.0, 6.5, 13.0, 17.0)},
	{"tile": 4.0, "cards": 100, "flowers": 0, "width": 1.0, "columns": 10, "fade": Vector4(13.0, 17.0, 32.0, 40.0)}]]
## Card blades (vegetation_grass.gdshader): root half width in columns, the
## share of columns without a blade, blade height shares 0.45..1 of the card.
const BLADE_FILL := 0.32
const CARD_GAP := 0.15
## Mean share of a card's area its blades cover: columns with a blade x mean
## height share x root width x mean taper (full width to 60 % of the height).
const CARD_COVER := (1.0 - CARD_GAP) * 0.725 * BLADE_FILL * 2.0 * 0.8
## Mean projected share of a card's width (half turned toward the camera).
const CARD_FACING := 0.8
## Projected blade cover by tier (blade area per ground area seen from
## EYE_HEIGHT: cards per m2 x width x mean height x CARD_COVER x CARD_FACING x
## distance / EYE_HEIGHT): every ring keeps this cover from where its full
## density reaches it outward.
const COVER := [0.0, 0.9, 1.3]
const EYE_HEIGHT := 1.5
## Lawn shade under the blades by tier (grass_field_shade).
const SHADE := [0.0, 0.16, 0.18]
const FLOWERS := Vector2(0.35, 0.012)
## Card height range (m): the tallest blade of a card, before clumps.
const BLADE_HEIGHT := Vector2(0.12, 0.15)
## Instances are re-placed when the camera height leaves this band.
const AABB_HEIGHT := 96.0
## Ring origins lead the camera by its last frame's travel (at most this):
## the field ticks before the camera rig moves.
const LEAD_METRES := 1.5

class Cell extends RefCounted:
	var field: WeakRef
	var record: Dictionary
	var root: WeakRef
	var bounds: Rect2i
	var visible := false
	var tint: Color
	var lawn: WeakRef
	var released := false
	## Unregisters the cell (idempotent); its blocks are rebuilt without it.
	func release() -> void:
		if released: return
		released = true
		var owner: RefCounted = field.get_ref() if field != null else null
		if owner != null: owner.remove(self)

## Bound once when the renderer scripts load, so the shader's compilation never
## falls into an admitted streaming step (as vegetation_meshes._warm).
static var _warm_material: ShaderMaterial = _warm()

static func _warm() -> ShaderMaterial:
	var warm := ShaderMaterial.new()
	warm.shader = SHADER
	warm.set_shader_parameter("wind_strength", 0.0)
	return warm

static var _standalone: RefCounted

var level := 0
var live := -1
var live_distance := 0.0
var blocks := 0
var lease: RefCounted
var resources: WeakRef
var closed := false
var cells: Array = []
var texture: Texture2DArray
var slot_block: Array[Vector2i] = []
var slot_grass := PackedByteArray()
var slot_dirty := PackedByteArray()
var dirty: Array[int] = []
var dirty_sorted := false
var window := Vector2i(1 << 30, 1 << 30)
var rings: Array = []
var scenario: RID
var tint := Color(-1, -1, -1)
var wind := -1.0
var scratch := PackedFloat32Array()
var empty_layer: Image
## Block being rasterized into `scratch` across frames: slot (-1 none), next
## cell and next ref of that cell, and whether any texel is grass.
var building := -1
var build_cell := 0
var build_ref := -1
var build_any := false
var last_eye := Vector3.INF
## Lawn materials of the registered cells (instance id -> WeakRef) and the
## shade last set on them (distance, amount).
var lawns := {}
var published := Vector2(-1.0, -1.0)
var _connected := false
var _visibility_dirty := true
## Diagnostics: built blocks, uploaded layers, peak tick (steady: no ring
## rebuild and a window move of at most one block), peak block and clears.
var stats := {"built": 0, "uploads": 0, "peak_tick_usec": 0, "peak_steady_tick_usec": 0, "peak_block_usec": 0, "peak_clears": 0, "ticks": 0, "resumed": 0}

## Field tier of a quality profile: the tier of its cache textures
## (texture_size), 0 on low.
static func grass_level(value: Dictionary) -> int:
	return QUALITY.grass_level(value)

## Fixed bytes of a field at a tier: the height array (4 KiB per block layer)
## with one upload staging copy, and 512 KiB for the ring meshes, MultiMesh
## buffers (CPU and GPU, with one retiring set while a tier change rebuilds
## them), three materials, twelve render instances and the block scratch.
static func memory_bytes(tier: int) -> int:
	var count: int = BLOCKS[clampi(tier, 0, 2)]
	return 0 if count == 0 else count * count * BLOCK_TEXELS * BLOCK_TEXELS * 4 * 2 + 524288

## Tiles of one ring around a camera inside the snapped origin's tile
## (relative tile indices): every tile a fading-in-or-visible blade can lie
## in for any camera offset within half a tile of the origin.
static func ring_tiles(ring: Dictionary) -> Array[Vector2i]:
	var size: float = ring.tile
	var fade: Vector4 = ring.fade
	var slack := size * sqrt(2.0)
	var reach := ceili(fade.w / size) + 2
	var result: Array[Vector2i] = []
	for z in range(-reach, reach):
		for x in range(-reach, reach):
			var centre := Vector2(x + 0.5, z + 0.5) * size
			if centre.length() >= fade.w + slack: continue
			if fade.x >= 0.0 and centre.length() + slack <= fade.x: continue
			result.append(Vector2i(x, z))
	return result

## Rings of a tier reaching `distance` (the live grass_distance): rings end
## there (the last ring's fade-out starts by 0.8 x distance) and rings starting
## past it are dropped. Each ring carries `thin`, the camera distance from
## which its full card density meets the tier's COVER; the shader thins cards
## by thin / distance beyond it.
static func ring_set(tier: int, distance: float) -> Array:
	tier = clampi(tier, 0, 2)
	var result := []
	if distance <= 0.0: return result
	for ring: Dictionary in RINGS[tier]:
		var config := ring.duplicate()
		var fade: Vector4 = config.fade
		if fade.x >= 0.0 and fade.y >= distance: break
		if fade.w >= distance: fade = Vector4(fade.x, fade.y, minf(fade.z, distance * 0.8), distance)
		config.fade = fade
		config.thin = COVER[tier] * EYE_HEIGHT / card_cover(config)
		result.append(config)
		if fade.w >= distance: break
	return result

## Blade area per ground area of a ring at full density (cover per metre of
## camera distance times EYE_HEIGHT).
static func card_cover(ring: Dictionary) -> float:
	var density: float = float(ring.cards) / (ring.tile * ring.tile)
	return density * float(ring.width) * (BLADE_HEIGHT.x + BLADE_HEIGHT.y) * 0.5 * CARD_COVER * CARD_FACING

## Registers a cell's grass record (grass_scatter.record) under `root` with
## its lawn material (the grass ground material whose shade the field sets,
## or null); returns the Cell handle (release() it with the cell) or null when
## the tier has no grass or the field's memory is refused.
static func register(record: Dictionary, root: Node3D, resources_cache: RefCounted, cell_tint: Color, lawn: Material = null) -> Cell:
	if record.is_empty() or not is_instance_valid(root): return null
	var field: RefCounted
	if resources_cache != null:
		if not resources_cache.has_method("grass_field"): return null
		field = resources_cache.grass_field()
	else:
		if _standalone == null or _standalone.closed:
			var tier := grass_level(QUALITY.active())
			if tier <= 0: return null
			_standalone = (load((QUALITY as Script).resource_path.get_base_dir().path_join("grass_field.gd")) as GDScript).new(tier, null, null)
		field = _standalone
	if field == null: return null
	var cell: Cell = field.add(record, root, cell_tint, lawn)
	if cell == null and resources_cache != null: resources_cache.release(KEY)
	return cell

func _init(tier: int, owner_lease: RefCounted, cache: RefCounted) -> void:
	level = clampi(tier, 1, 2)
	blocks = BLOCKS[level]
	lease = owner_lease
	resources = weakref(cache) if cache != null else null
	scratch.resize(BLOCK_TEXELS * BLOCK_TEXELS)
	var empty := PackedFloat32Array()
	empty.resize(BLOCK_TEXELS * BLOCK_TEXELS)
	empty.fill(EMPTY)
	empty_layer = Image.create_from_data(BLOCK_TEXELS, BLOCK_TEXELS, false, Image.FORMAT_RF, empty.to_byte_array())
	var layers: Array[Image] = []
	for i in blocks * blocks: layers.append(empty_layer)
	texture = Texture2DArray.new()
	texture.create_from_images(layers)
	if lease != null: lease.track(texture)
	slot_block.resize(blocks * blocks)
	slot_block.fill(Vector2i(1 << 30, 1 << 30))
	slot_grass.resize(blocks * blocks)
	slot_dirty.resize(blocks * blocks)

func add(record: Dictionary, root: Node3D, cell_tint: Color, lawn: Material = null) -> Cell:
	if closed: return null
	var cell := Cell.new()
	cell.field = weakref(self)
	cell.record = record
	cell.root = weakref(root)
	cell.bounds = Rect2i(record.origin, record.size)
	cell.tint = cell_tint
	# Only the atmosphere surface shader has the lawn shade uniforms; a plain
	# ground material (no prepared environment, as in Editor previews) keeps none.
	if lawn is ShaderMaterial:
		cell.lawn = weakref(lawn)
		if not lawns.has(lawn.get_instance_id()):
			lawns[lawn.get_instance_id()] = weakref(lawn)
			published = Vector2(-1.0, -1.0)
	cells.append(cell)
	if not _connected:
		(Engine.get_main_loop() as SceneTree).process_frame.connect(tick)
		_connected = true
	return cell

## Drops a cell (Cell.release); a cache-owned field releases its claim pin.
func remove(cell: Cell) -> void:
	var index := cells.find(cell)
	if index < 0: return
	cells.remove_at(index)
	# The block in progress walks the cell list by index: restart it.
	_abort_build()
	if cell.visible: _invalidate(cell.bounds)
	var cache: RefCounted = resources.get_ref() if resources != null else null
	if cache != null and cache.has_method("release"): cache.release(KEY)
	if cells.is_empty():
		_hide()
		if self == _standalone:
			close()
			_standalone = null

## Frees every render resource; the field cannot be used again.
func close() -> void:
	if closed: return
	closed = true
	if _connected:
		(Engine.get_main_loop() as SceneTree).process_frame.disconnect(tick)
		_connected = false
	_publish(0.0, 0.0)
	lawns.clear()
	_free_rings()
	for cell: Cell in cells: cell.released = true
	cells.clear()
	texture = null
	lease = null

## Live tier and reach for a quality profile: the profile's level capped at
## the field tier, and its grass_distance capped at that tier's own (the
## window of a medium field reaches only medium's grass_distance).
func live_rings(value: Dictionary) -> Vector2:
	var distance := float(value.get("grass_distance", 0.0))
	var tier := mini(int(value.get("level", 1)), level) if distance > 0.0 else 0
	if tier <= 0: return Vector2.ZERO
	return Vector2(tier, minf(distance, float(QUALITY.profile(tier).grass_distance)))

## Per-frame update (SceneTree.process_frame): cells, rings and blocks.
func tick() -> void:
	if closed: return
	var started := Time.get_ticks_usec()
	stats.ticks += 1
	_refresh_cells()
	var camera := _camera()
	var value: Dictionary = QUALITY.current if not QUALITY.current.is_empty() else QUALITY.profile(1)
	var wanted := live_rings(value)
	var tier := int(wanted.x)
	if cells.is_empty() or camera == null or tier <= 0:
		_hide()
		if cells.is_empty() and _connected:
			(Engine.get_main_loop() as SceneTree).process_frame.disconnect(tick)
			_connected = false
		return
	var world := camera.get_world_3d()
	var steady := true
	if tier != live or not is_equal_approx(wanted.y, live_distance) or world == null or world.scenario != scenario:
		if world == null: return
		scenario = world.scenario
		_build_rings(tier, wanted.y)
		steady = false
	_update_materials(value)
	_publish(live_distance, SHADE[live])
	var eye := camera.global_position
	# Lead by the last frame's travel: the camera rig moves after this tick.
	var lead := (eye - last_eye) if last_eye.is_finite() else Vector3.ZERO
	last_eye = eye
	if lead.length() > LEAD_METRES: lead = Vector3.ZERO
	var moved := _move_window(eye)
	if moved > 1: steady = false
	_move_rings(eye + lead)
	_build_blocks(eye, started)
	if _visibility_dirty: _update_visibility()
	var spent := Time.get_ticks_usec() - started
	stats.peak_tick_usec = maxi(int(stats.peak_tick_usec), spent)
	if steady: stats.peak_steady_tick_usec = maxi(int(stats.peak_steady_tick_usec), spent)

## Camera of the first registered cell inside the tree.
func _camera() -> Camera3D:
	for cell: Cell in cells:
		var node: Node3D = cell.root.get_ref()
		if node != null and node.is_inside_tree(): return node.get_viewport().get_camera_3d()
	return null

## Drops freed roots and rebuilds the blocks of cells shown or hidden.
func _refresh_cells() -> void:
	for i in range(cells.size() - 1, -1, -1):
		var cell: Cell = cells[i]
		var node: Node3D = cell.root.get_ref()
		if node == null or not is_instance_valid(node) or node.is_queued_for_deletion():
			cell.release()
			continue
		var shown := node.is_visible_in_tree()
		if shown != cell.visible:
			cell.visible = shown
			_invalidate(cell.bounds)

func _invalidate(bounds: Rect2i) -> void:
	for slot in slot_block.size():
		if bounds.has_point(slot_block[slot]):
			if slot == building: _abort_build()
			if slot_dirty[slot] == 0:
				slot_dirty[slot] = 1
				dirty.append(slot)
				dirty_sorted = false

## Drops the block in progress; its slot stays dirty and builds again.
func _abort_build() -> void:
	if building < 0: return
	if slot_dirty[building] == 0:
		slot_dirty[building] = 1
		dirty.append(building)
		dirty_sorted = false
	building = -1

## Slot of a world block in the toroidal window.
func slot_of(block: Vector2i) -> int:
	return posmod(block.x, blocks) + posmod(block.y, blocks) * blocks

## Recentres the window; returns how many blocks it moved (Chebyshev). A slot
## that changes block and held grass is cleared at once (never another
## block's grass), counted against this frame's STEP_USEC.
func _move_window(eye: Vector3) -> int:
	var centre := Vector2i(roundi(eye.x / BLOCK_METRES), roundi(eye.z / BLOCK_METRES))
	var low := centre - Vector2i(blocks / 2, blocks / 2)
	if low == window: return 0
	var moved := maxi(absi(low.x - window.x), absi(low.y - window.y)) if window.x < (1 << 29) else 0
	window = low
	var clears := 0
	for z in range(low.y, low.y + blocks):
		for x in range(low.x, low.x + blocks):
			var block := Vector2i(x, z)
			var slot := slot_of(block)
			if slot_block[slot] == block: continue
			slot_block[slot] = block
			if slot == building: building = -1
			if slot_grass[slot] != 0:
				_upload(slot, false)
				clears += 1
				_visibility_dirty = true
			if slot_dirty[slot] == 0:
				slot_dirty[slot] = 1
				dirty.append(slot)
	stats.peak_clears = maxi(int(stats.peak_clears), clears)
	dirty_sorted = false
	return moved

## Rasterizes the nearest dirty blocks until STEP_USEC of this frame is spent
## (counted from the tick's start), checking the time every STEP_TRIANGLES
## triangles; a block left unfinished resumes next frame.
func _build_blocks(eye: Vector3, started: int) -> void:
	if building < 0 and dirty.is_empty(): return
	if not dirty_sorted:
		var keys := PackedInt64Array()
		for slot: int in dirty:
			var centre := (Vector2(slot_block[slot]) + Vector2(0.5, 0.5)) * BLOCK_METRES
			keys.append(int(centre.distance_squared_to(Vector2(eye.x, eye.z))) * 1024 + slot)
		keys.sort()
		dirty.clear()
		for key in keys: dirty.append(int(key % 1024))
		dirty_sorted = true
	var first := true
	while building >= 0 or not dirty.is_empty():
		if not first and Time.get_ticks_usec() - started > STEP_USEC: return
		first = false
		if building < 0:
			building = dirty.pop_front()
			slot_dirty[building] = 0
			build_cell = 0
			build_ref = -1
			build_any = false
			scratch.fill(EMPTY)
		else:
			stats.resumed += 1
		var before := Time.get_ticks_usec()
		var done := _raster_step(started)
		stats.peak_block_usec = maxi(int(stats.peak_block_usec), Time.get_ticks_usec() - before)
		if not done: return
		var slot := building
		building = -1
		if build_any or slot_grass[slot] != 0:
			_upload(slot, build_any)
			_visibility_dirty = true
		stats.built += 1

## Continues the block in progress; true when it is complete. Returns early
## (false) once STEP_USEC has passed since `started`.
func _raster_step(started: int) -> bool:
	var block := slot_block[building]
	var count := 0
	while build_cell < cells.size():
		var cell: Cell = cells[build_cell]
		if not cell.visible or not cell.bounds.has_point(block):
			build_cell += 1
			build_ref = -1
			continue
		var record := cell.record
		var size: Vector2i = record.size
		var index: int = (block.y - record.origin.y) * size.x + (block.x - record.origin.x)
		var offsets: PackedInt32Array = record.offsets
		var refs: PackedInt32Array = record.refs
		var parts: Array = record.parts
		if build_ref < 0: build_ref = offsets[index]
		var end: int = offsets[index + 1]
		while build_ref < end:
			if count >= STEP_TRIANGLES:
				count = 0
				if Time.get_ticks_usec() - started > STEP_USEC: return false
			var ref: int = refs[build_ref]
			var part: PackedVector3Array = parts[ref >> SCATTER.PART_SHIFT]
			var t: int = (ref & ((1 << SCATTER.PART_SHIFT) - 1)) * 3
			if fill(scratch, block, part[t], part[t + 1], part[t + 2]): build_any = true
			build_ref += 1
			count += 1
		build_cell += 1
		build_ref = -1
	return true

func _upload(slot: int, grass: bool) -> void:
	texture.update_layer(Image.create_from_data(BLOCK_TEXELS, BLOCK_TEXELS, false, Image.FORMAT_RF, scratch.to_byte_array()) if grass else empty_layer, slot)
	slot_grass[slot] = 1 if grass else 0
	stats.uploads += 1

## Writes the triangle's plane height at every texel centre of the block it
## covers (edges inclusive, so neighbours leave no crack; the higher surface
## wins where grass overlaps). Returns whether any texel was written.
static func fill(data: PackedFloat32Array, block: Vector2i, a: Vector3, b: Vector3, c: Vector3) -> bool:
	# Texel space of the block: texel (u, v) has its centre at integer u, v.
	var ou := float(block.x * BLOCK_TEXELS) + 0.5
	var ov := float(block.y * BLOCK_TEXELS) + 0.5
	var au := a.x * TEXELS_PER_METRE - ou
	var av := a.z * TEXELS_PER_METRE - ov
	var bu := b.x * TEXELS_PER_METRE - ou
	var bv := b.z * TEXELS_PER_METRE - ov
	var cu := c.x * TEXELS_PER_METRE - ou
	var cv := c.z * TEXELS_PER_METRE - ov
	var e1u := bu - au
	var e1v := bv - av
	var e2u := cu - au
	var e2v := cv - av
	var denom := e1u * e2v - e1v * e2u
	if absf(denom) < 1e-6: return false
	var hu := (e2v * (b.y - a.y) - e1v * (c.y - a.y)) / denom
	var hv := (e1u * (c.y - a.y) - e2u * (b.y - a.y)) / denom
	var first := maxi(0, ceili(minf(av, minf(bv, cv)) - 1e-4))
	var last := mini(BLOCK_TEXELS - 1, floori(maxf(av, maxf(bv, cv)) + 1e-4))
	var written := false
	for v in range(first, last + 1):
		var row := float(v)
		var low := INF
		var high := -INF
		# Each edge crossing this row's centre line widens the span.
		if (av - row) * (bv - row) <= 0.0:
			if absf(bv - av) < 1e-9: low = minf(low, minf(au, bu)); high = maxf(high, maxf(au, bu))
			else:
				var x := au + (row - av) * (bu - au) / (bv - av)
				low = minf(low, x); high = maxf(high, x)
		if (bv - row) * (cv - row) <= 0.0:
			if absf(cv - bv) < 1e-9: low = minf(low, minf(bu, cu)); high = maxf(high, maxf(bu, cu))
			else:
				var x := bu + (row - bv) * (cu - bu) / (cv - bv)
				low = minf(low, x); high = maxf(high, x)
		if (cv - row) * (av - row) <= 0.0:
			if absf(av - cv) < 1e-9: low = minf(low, minf(cu, au)); high = maxf(high, maxf(cu, au))
			else:
				var x := cu + (row - cv) * (au - cu) / (av - cv)
				low = minf(low, x); high = maxf(high, x)
		var start := maxi(0, ceili(low - 1e-4))
		var end := mini(BLOCK_TEXELS - 1, floori(high + 1e-4))
		if start > end: continue
		var h := a.y + (float(start) - au) * hu + (row - av) * hv
		var index := v * BLOCK_TEXELS + start
		for u in range(start, end + 1):
			if h > data[index]: data[index] = h
			h += hu
			index += 1
		written = true
	return written

## Ground height and grass share at a world point, as the shader samples it,
## from block data (validators): `blocks_data` maps a block to its 1,024 heights.
static func sample(blocks_data: Dictionary, x: float, z: float) -> Vector2:
	var p := Vector2(x, z) * TEXELS_PER_METRE - Vector2(0.5, 0.5)
	var cell := p.floor()
	var f := p - cell
	var weights := [(1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y]
	var share := 0.0
	var height := 0.0
	for k in 4:
		var texel := Vector2i(int(cell.x) + k % 2, int(cell.y) + k / 2)
		var block := Vector2i(floori(texel.x / float(BLOCK_TEXELS)), floori(texel.y / float(BLOCK_TEXELS)))
		var data: PackedFloat32Array = blocks_data.get(block, PackedFloat32Array())
		if data.is_empty(): continue
		var h := data[(texel.y - block.y * BLOCK_TEXELS) * BLOCK_TEXELS + texel.x - block.x * BLOCK_TEXELS]
		if h <= EMPTY * 0.1: continue
		share += weights[k]
		height += weights[k] * h
	return Vector2(height / share if share > 0.0 else 0.0, share)

## One tile mesh per ring: `cards` card slots (an upright quad) then
## `flowers` flower slots (stem and five-petal head). VERTEX carries the
## template (side, height share; a flower head's z marks its petal rim), UV
## the slot and kind (0 card, 1 stem, 2 head).
static func tile_mesh(ring: Dictionary) -> ArrayMesh:
	var vertices := PackedVector3Array()
	var uv := PackedVector2Array()
	var indices := PackedInt32Array()
	var cards := int(ring.cards)
	for slot in cards + int(ring.flowers):
		var first := vertices.size()
		if slot < cards:
			vertices.append_array([Vector3(-1, 0, 0), Vector3(1, 0, 0), Vector3(-1, 1, 0), Vector3(1, 1, 0)])
			indices.append_array([first, first + 1, first + 3, first, first + 3, first + 2])
			for i in 4: uv.append(Vector2(slot, 0.0))
			continue
		for point: Vector3 in [Vector3(-1, 0, 0), Vector3(1, 0, 0), Vector3(0, 1, 0)]:
			vertices.append(point)
			uv.append(Vector2(slot, 1.0))
		indices.append_array([first, first + 1, first + 2])
		var centre := vertices.size()
		vertices.append(Vector3.ZERO)
		uv.append(Vector2(slot, 2.0))
		for petal in 5:
			var angle := TAU * petal / 5.0
			vertices.append(Vector3(cos(angle), sin(angle), 1.0))
			uv.append(Vector2(slot, 2.0))
		for petal in 5:
			indices.append_array([centre, centre + 1 + (petal + 1) % 5, centre + 1 + petal])
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_TEX_UV] = uv
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	mesh.resource_name = "mk_grass_tile_%dm" % int(ring.tile)
	return mesh

func _build_rings(tier: int, distance: float) -> void:
	_free_rings()
	live = tier
	live_distance = distance
	tint = Color(-1, -1, -1)
	wind = -1.0
	var cache: RefCounted = resources.get_ref() if resources != null else null
	var context: RefCounted = cache.environment_context() if cache != null and cache.has_method("environment_context") else null
	var index := 0
	for config: Dictionary in ring_set(tier, distance):
		var material := ShaderMaterial.new()
		material.shader = SHADER
		material.resource_name = "mk_grass_field_%d" % index
		material.set_shader_parameter("field_heights", texture)
		material.set_shader_parameter("field_blocks", blocks)
		material.set_shader_parameter("tile_metres", config.tile)
		material.set_shader_parameter("ring_seed", index * 7919 + 1)
		material.set_shader_parameter("card_slots", config.cards)
		material.set_shader_parameter("fade", config.fade)
		material.set_shader_parameter("thin_distance", config.thin)
		material.set_shader_parameter("card_width", config.width)
		material.set_shader_parameter("card_columns", config.columns)
		material.set_shader_parameter("blade_fill", BLADE_FILL)
		material.set_shader_parameter("card_gap", CARD_GAP)
		material.set_shader_parameter("blade_height", BLADE_HEIGHT)
		var area: float = config.tile * config.tile / maxf(1.0, float(config.flowers))
		material.set_shader_parameter("flower_share", Vector2(minf(1.0, FLOWERS.x * area), minf(1.0, FLOWERS.y * area)))
		if context != null and context.has_method("bind_vegetation"): context.bind_vegetation(material)
		var mesh := tile_mesh(config)
		mesh.surface_set_material(0, material)
		var quadrants: Array = [[], [], [], []]
		for tile: Vector2i in ring_tiles(config):
			quadrants[int(tile.x >= 0) + 2 * int(tile.y >= 0)].append(tile)
		var parts := []
		for q in 4:
			var tiles: Array = quadrants[q]
			if tiles.is_empty(): continue
			var multi := MultiMesh.new()
			multi.transform_format = MultiMesh.TRANSFORM_3D
			multi.mesh = mesh
			multi.instance_count = tiles.size()
			var low := Vector2(INF, INF)
			var high := -low
			for i in tiles.size():
				var corner := Vector2(tiles[i]) * float(config.tile)
				multi.set_instance_transform(i, Transform3D(Basis(), Vector3(corner.x, 0.0, corner.y)))
				low = low.min(corner)
				high = high.max(corner + Vector2.ONE * float(config.tile))
			var instance := RenderingServer.instance_create2(multi.get_rid(), scenario)
			RenderingServer.instance_geometry_set_cast_shadows_setting(instance, RenderingServer.SHADOW_CASTING_SETTING_OFF)
			RenderingServer.instance_set_visible(instance, false)
			if lease != null: lease.track(multi)
			parts.append({"multimesh": multi, "instance": instance, "low": low, "high": high, "visible": false})
		if lease != null:
			lease.track(mesh)
			lease.track(material)
		rings.append({"config": config, "material": material, "mesh": mesh, "parts": parts, "origin": Vector2(INF, INF), "height": INF})
		index += 1
	_visibility_dirty = true

func _free_rings() -> void:
	for ring: Dictionary in rings:
		for part: Dictionary in ring.parts: RenderingServer.free_rid(part.instance)
	rings.clear()
	live = -1
	live_distance = 0.0

func _update_materials(value: Dictionary) -> void:
	var motion := float(value.get("vegetation_motion", 0.5))
	var colour: Color = (cells[0] as Cell).tint
	if is_equal_approx(motion, wind) and colour.is_equal_approx(tint): return
	wind = motion
	tint = colour
	for ring: Dictionary in rings:
		ring.material.set_shader_parameter("wind_strength", motion)
		ring.material.set_shader_parameter("grass_tint", Vector3(colour.r, colour.g, colour.b))

func _move_rings(eye: Vector3) -> void:
	for ring: Dictionary in rings:
		var size: float = ring.config.tile
		var origin := Vector2(roundf(eye.x / size), roundf(eye.z / size)) * size
		if origin == ring.origin and absf(eye.y - float(ring.height)) < AABB_HEIGHT * 0.25: continue
		var moved: bool = origin != ring.origin
		ring.origin = origin
		ring.height = eye.y
		for part: Dictionary in ring.parts:
			RenderingServer.instance_set_transform(part.instance, Transform3D(Basis(), Vector3(origin.x, 0.0, origin.y)))
			var low: Vector2 = part.low
			var high: Vector2 = part.high
			RenderingServer.instance_set_custom_aabb(part.instance, AABB(Vector3(low.x, eye.y - AABB_HEIGHT, low.y), Vector3(high.x - low.x, AABB_HEIGHT * 2.0, high.y - low.y)))
		if moved: _visibility_dirty = true

## A quadrant draws only when a grass block of the window reaches it.
func _update_visibility() -> void:
	_visibility_dirty = false
	var grass: Array[Rect2] = []
	for slot in slot_grass.size():
		if slot_grass[slot] != 0: grass.append(Rect2(Vector2(slot_block[slot]) * BLOCK_METRES, Vector2.ONE * BLOCK_METRES))
	for ring: Dictionary in rings:
		for part: Dictionary in ring.parts:
			var area := Rect2(ring.origin + part.low, part.high - part.low)
			var shown := false
			for rect: Rect2 in grass:
				if rect.intersects(area):
					shown = true
					break
			if shown != part.visible:
				part.visible = shown
				RenderingServer.instance_set_visible(part.instance, shown)

func _hide() -> void:
	for ring: Dictionary in rings:
		for part: Dictionary in ring.parts:
			if part.visible:
				part.visible = false
				RenderingServer.instance_set_visible(part.instance, false)
	_visibility_dirty = true
	_publish(0.0, 0.0)

## Sets the lawn shade under the blades on the cells' lawn materials
## (atmosphere_surface grass_field_distance and grass_field_shade): the live
## reach while the field draws, 0 when it shows no blades.
func _publish(distance: float, shade: float) -> void:
	var value := Vector2(distance, shade)
	if value == published: return
	published = value
	for id: int in lawns.keys():
		var material: Material = lawns[id].get_ref()
		if material == null:
			lawns.erase(id)
			continue
		material.set_shader_parameter("grass_field_distance", distance)
		material.set_shader_parameter("grass_field_shade", shade)

## Draw work of the live rings: instances (tiles), vertices per frame if every
## quadrant draws, and render instances (draw calls).
func draw_cost() -> Dictionary:
	var result := {"instances": 0, "vertices": 0, "draws": 0, "visible_draws": 0}
	for ring: Dictionary in rings:
		var per_tile: int = (ring.mesh as ArrayMesh).surface_get_array_len(0)
		for part: Dictionary in ring.parts:
			result.instances += (part.multimesh as MultiMesh).instance_count
			result.vertices += (part.multimesh as MultiMesh).instance_count * per_tile
			result.draws += 1
			result.visible_draws += int(part.visible)
	return result
