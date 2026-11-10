extends RefCounted
## Camera-local street life (docs/STREET_LIFE.md): voxel walkers on the
## registered cells' sidewalk walk lanes (presentation walk_points,
## walk_lanes, walk_links, walk_crossings and walk_crossing_signals,
## mapkit-godot walk_lanes.rs) and pigeon flocks at benches and bus shelters,
## at fixed memory however many cells are loaded. Display only: walkers have
## no collision, never touch Runtime state and are seeded locally.
##
## Graph: every lane, corner link and crossing is an edge between two nodes;
## ends of any registered cell within MERGE of each other share a node (a 2 m
## hash grid). Walkers keep their own pace and a lateral slot on the
## driving-side's right (left under left-hand traffic), take a crossing one
## time in three at a node that has one (never a signalled one, and only when
## no vehicle is within CROSS_CLEAR or would arrive within CROSS_ARRIVAL),
## else another lane or link, else turn round; they step aside from vehicles
## that would meet them, stand when boxed in and turn round after
## BOXED_SECONDS. Vehicle display poses arrive every frame
## (render_resource_cache.street_life_obstacles).
##
## Population: min(walker_count, Σ lane length × density × time × weather)
## within walker_distance of the camera, spawned 0.6–1.0 × that reach (out of
## view when possible) with a fade-in, dropped beyond reach + DROP_MARGIN.
## Cells always register their lanes while a cache exists; spawning and
## drawing follow the live display_quality street_life switch and
## walker_count / pigeon_count, so the toggle and adaptive trims need no
## reload. Memory: MEMORY_BYTES, one claim per resource cache
## (render_resource_cache.street_life_field), none on low; each cell's graph
## is in its estimate (render_memory.street_life_bytes).

const QUALITY := preload("./display_quality.gd")
const MESHES := preload("./pedestrian_meshes.gd")
## Shared-resource key of a cache's field (render_resource_cache.street_life_field).
const KEY := "builtin:street_life_field"
## Fixed bytes: the meshes and material, two walker MultiMeshes and one pigeon
## MultiMesh at 64 B per instance on the CPU and GPU with one retiring set,
## walker and pigeon state (packed arrays, under 128 B per slot), the flock
## records, the spawn candidates and the obstacle list.
const MEMORY_BYTES := 262144
const STEP_USEC := 500
## Instance slots (the high tier's walker_count and pigeon_count).
const MAX_WALKERS := 80
const MAX_PIGEONS := 32
const FLOATS := 16
## Game scale (as the builtins): figure height × 0.94–1.06, pace (m/s).
const HEIGHT := 0.57
const HEIGHT_RANGE := Vector2(0.94, 1.06)
const PACE := Vector2(0.33, 0.53)
## Lateral slot of the half width on the walker's own side.
const SLOT := Vector2(0.25, 0.85)
## Strides per metre of the gait (one phase per stride pair).
const STRIDE := 1.6
## Edge kinds and the half width of a crossing.
const LANE := 0
const LINK := 1
const CROSSING := 2
const CROSSING_HALF := 0.8
const MERGE := 1.5
const GRID := 2.0
## Give way: reaction reach and meeting time, sideways speed, room beyond the
## lane when the vehicle is on the sidewalk, and the boxed-in wait.
const GIVE_WAY := 3.3
const MEET_SECONDS := 2.0
const MEET_GAP := 0.8
const ASIDE_SPEED := 0.3
const ASIDE_OUT := 0.5
const BOXED_SECONDS := 2.0
## Crossings: share taken at a node, clearance and arrival window, and the
## longest wait at the kerb before walking on.
const CROSS_SHARE := 1.0 / 3.0
const CROSS_CLEAR := 3.0
const CROSS_ARRIVAL := 1.5
const CURB_WAIT := 6.0
## Population and fades.
const SPAWN := Vector2(0.6, 1.0)
const DROP_MARGIN := 3.0
const FADE_IN := 0.5
const CAMERA_FADE := Vector2(0.35, 0.8)
const VEHICLE_FADE := Vector2(0.25, 0.4)
const TIME_FACTOR := Vector2(0.25, 1.0)
const WET_FACTOR := 0.6
const WET_FROM := 0.15
const POPULATION_SECONDS := 0.5
const SPAWNS_PER_TICK := 2
## Pigeons: share of benches and shelters with a flock, flock size, reach,
## flush distances and speed, and the flight.
const PIGEON_SHARE := 0.3
const FLOCK := Vector2i(3, 7)
const PIGEON_REACH := 16.0
const FLUSH_NEAR := 1.0
const FLUSH_FAST := 2.0
const FLUSH_SPEED := 1.5
const FLIGHT_RISE := Vector2(1.5, 3.0)
const FLIGHT_RADIUS := Vector2(2.0, 4.0)
const FLIGHT_SECONDS := Vector2(6.0, 12.0)
const DAY_FROM := 0.5
## Ambience query: reach from a cell's rectangle, and the walker reach counted as near.
const AMBIENCE_REACH := 96.0
const AMBIENCE_WALKERS := 6.0
const AABB_REACH := 34.0

class Cell extends RefCounted:
	var field: WeakRef
	var root: WeakRef
	var uid := 0
	## Lane and link points, then each crossing's two mouths.
	var points := PackedVector3Array()
	## Arc length from each edge's first point.
	var arc := PackedFloat32Array()
	## Per edge: first point, point count, kind, half width, density, length,
	## signalled, and its two nodes.
	var first := PackedInt32Array()
	var count := PackedInt32Array()
	var kinds := PackedByteArray()
	var halves := PackedFloat32Array()
	var densities := PackedFloat32Array()
	var lengths := PackedFloat32Array()
	var signalled := PackedByteArray()
	var nodes := PackedInt32Array()
	var anchors := PackedVector3Array()
	var ambience := Vector4.ZERO
	var centre := Vector2.ZERO
	## Scene x/z rectangle: the cell bounds grown to every point and anchor.
	var rect := Rect2()
	var left_hand := false
	var visible := true
	var released := false
	func release() -> void:
		if released: return
		released = true
		var owner: RefCounted = field.get_ref() if field != null else null
		if owner != null: owner.remove(self)

## Bound once when the scripts load (pedestrian_meshes._warm compiles the shader).
static var _warm_meshes: Dictionary = MESHES.shared()

var lease: RefCounted
var resources: WeakRef
var closed := false
var cells: Array = []
var by_uid := {}
var next_uid := 1
var node_pos := {}
var node_refs := {}
var node_grid := {}
var next_node := 1
## Walker state, one entry per walker (see the Walkers section).
const WALKER_ARRAYS := ["w_cell", "w_edge", "w_segment", "w_waiting", "w_flags", "w_top", "w_bottom", "w_s", "w_pace", "w_slot",
	"w_lateral", "w_height", "w_phase", "w_age", "w_waited", "w_boxed", "w_alpha", "w_shift", "w_position", "w_direction"]
var w_cell := PackedInt32Array()
var w_edge := PackedInt32Array()
var w_segment := PackedInt32Array()
var w_waiting := PackedInt64Array()
var w_flags := PackedByteArray()
var w_top := PackedByteArray()
var w_bottom := PackedByteArray()
var w_s := PackedFloat32Array()
var w_pace := PackedFloat32Array()
var w_slot := PackedFloat32Array()
var w_lateral := PackedFloat32Array()
var w_height := PackedFloat32Array()
var w_phase := PackedFloat32Array()
var w_age := PackedFloat32Array()
var w_waited := PackedFloat32Array()
var w_boxed := PackedFloat32Array()
var w_alpha := PackedFloat32Array()
var w_shift := PackedVector3Array()
var w_position := PackedVector3Array()
var w_direction := PackedVector3Array()
## Pigeon state, one entry per pigeon; flocks by anchor key.
const PIGEON_ARRAYS := ["p_flock", "p_home", "p_position", "p_flying", "p_time", "p_rise", "p_radius", "p_duration", "p_angle",
	"p_phase", "p_yaw", "p_tone", "p_idle", "p_alpha"]
var p_flock := PackedStringArray()
var p_home := PackedVector3Array()
var p_position := PackedVector3Array()
var p_flying := PackedByteArray()
var p_time := PackedFloat32Array()
var p_rise := PackedFloat32Array()
var p_radius := PackedFloat32Array()
var p_duration := PackedFloat32Array()
var p_angle := PackedFloat32Array()
var p_phase := PackedFloat32Array()
var p_yaw := PackedFloat32Array()
var p_tone := PackedFloat32Array()
var p_idle := PackedFloat32Array()
var p_alpha := PackedFloat32Array()
var flocks := {}
var rng := RandomNumberGenerator.new()
var obstacles := PackedVector3Array()
var velocities := PackedVector3Array()
var obstacle_usec := -1
var material: ShaderMaterial
var parts := {}
var scenario: RID
var last_eye := Vector3.INF
var population := 0
var population_at := -1.0
## Spawn candidates (lanes within reach): cell uid, edge, cumulative weight.
var candidate_cell := PackedInt32Array()
var candidate_edge := PackedInt32Array()
var candidate_sum := PackedFloat32Array()
var candidate_weight := 0.0
var clock := 0.0
var last_usec := -1
var _connected := false
## Validators step the field themselves (tick_step) when set.
var manual := false
## Conditions read from the environment context (daylight, wet or snowy).
var daylight := 1.0
var wet := false
## Diagnostics: ticks, peak tick, spawns, crossings taken, give-way reactions, flushes.
var stats := {"ticks": 0, "peak_tick_usec": 0, "spawned": 0, "crossed": 0, "gave_way": 0, "held": 0, "turned": 0, "flushed": 0}

## Display record of a decorated chunk view: its walk arrays and ambience
## (presentation), cell centre and scene x/z rectangle, driving side and pigeon anchors (a hashed
## PIGEON_SHARE of its benches and bus shelters), or {} without either key.
static func record(chunk: Dictionary) -> Dictionary:
	var presentation: Dictionary = chunk.get("presentation", {})
	if not presentation.has("ambience") and not presentation.has("walk_points"): return {}
	var result := {"ambience": presentation.get("ambience", Vector4.ZERO), "points": presentation.get("walk_points", PackedVector3Array()),
		"lanes": presentation.get("walk_lanes", PackedVector4Array()), "links": presentation.get("walk_links", PackedVector4Array()),
		"crossings": presentation.get("walk_crossings", PackedVector3Array()), "signals": presentation.get("walk_crossing_signals", PackedByteArray()),
		"centre": Vector2.ZERO, "rect": Rect2(), "left_hand": false, "anchors": PackedVector3Array()}
	var bounds: Variant = JSON.parse_string(str(presentation.get("cell_bounds_json", "")))
	if bounds is Dictionary and bounds.has("min") and bounds.has("max"):
		result.centre = Vector2(float(bounds.min[0]) + float(bounds.max[0]), -(float(bounds.min[1]) + float(bounds.max[1]))) * 0.005
		result.rect = Rect2(float(bounds.min[0]) * 0.01, -float(bounds.max[1]) * 0.01, (float(bounds.max[0]) - float(bounds.min[0])) * 0.01, (float(bounds.max[1]) - float(bounds.min[1])) * 0.01)
	for style: Variant in presentation.get("road_styles", {}).values():
		if style is Dictionary and bool(style.get("left_hand", false)): result.left_hand = true
	var anchors := PackedVector3Array()
	for object: Dictionary in chunk.get("objects", []):
		var id := str(object.get("asset_id", ""))
		if id != "builtin:bench" and id != "builtin:bus_shelter": continue
		if float(str(object.id).sha256_text().substr(0, 6).hex_to_int()) / 16777216.0 >= PIGEON_SHARE: continue
		var p: Array = object.position
		anchors.append(Vector3(float(p[0]), float(p[1]), -float(p[2])) * 0.01)
	result.anchors = anchors
	return result

## Registers a cell's record under `root`; returns the Cell handle (release()
## it with the cell) or null without a cache, on low or when the field's
## memory is refused.
static func register(value: Dictionary, root: Node3D, resources_cache: RefCounted) -> Cell:
	if value.is_empty() or not is_instance_valid(root) or resources_cache == null: return null
	if not resources_cache.has_method("street_life_field"): return null
	var field: RefCounted = resources_cache.street_life_field()
	if field == null: return null
	var cell: Cell = field.add(value, root)
	if cell == null: resources_cache.release(KEY)
	return cell

func _init(owner_lease: RefCounted, cache: RefCounted) -> void:
	lease = owner_lease
	resources = weakref(cache) if cache != null else null
	rng.seed = hash("street-life") ^ Time.get_ticks_usec()

## Fixes the walker and pigeon draws (validators).
func seed_random(value: int) -> void:
	rng.seed = value

func add(value: Dictionary, root: Node3D) -> Cell:
	if closed: return null
	var cell := Cell.new()
	cell.field = weakref(self)
	cell.root = weakref(root)
	cell.uid = next_uid
	next_uid += 1
	cell.ambience = value.get("ambience", Vector4.ZERO)
	cell.centre = value.get("centre", Vector2.ZERO)
	cell.left_hand = bool(value.get("left_hand", false))
	cell.anchors = value.get("anchors", PackedVector3Array())
	var points: PackedVector3Array = value.get("points", PackedVector3Array())
	var rect: Rect2 = value.get("rect", Rect2())
	var grown := rect.has_area()
	if not grown: rect = Rect2(cell.centre, Vector2.ZERO)
	for list: PackedVector3Array in [points, value.get("crossings", PackedVector3Array()), cell.anchors]:
		for p: Vector3 in list:
			rect = rect.expand(Vector2(p.x, p.z)) if grown else Rect2(p.x, p.z, 0.0, 0.0)
			grown = true
	cell.rect = rect
	for group: Array in [[value.get("lanes", PackedVector4Array()), LANE], [value.get("links", PackedVector4Array()), LINK]]:
		for entry: Vector4 in group[0]:
			var start := int(entry.x)
			var size := int(entry.y)
			if size < 2 or start < 0 or start + size > points.size(): continue
			_edge(cell, points.slice(start, start + size), group[1], entry.z, entry.w if group[1] == LANE else 0.0, false)
	var mouths: PackedVector3Array = value.get("crossings", PackedVector3Array())
	var signals: PackedByteArray = value.get("signals", PackedByteArray())
	for c in mouths.size() / 2:
		_edge(cell, PackedVector3Array([mouths[c * 2], mouths[c * 2 + 1]]), CROSSING, CROSSING_HALF, 0.0, c < signals.size() and signals[c] != 0)
	for e in cell.first.size():
		for end in 2:
			var p := cell.points[cell.first[e] + (cell.count[e] - 1 if end == 1 else 0)]
			var node := _node_at(p)
			cell.nodes.append(node)
			var refs: PackedInt64Array = node_refs[node]
			refs.append((cell.uid << 21) | (e << 1) | end)
			node_refs[node] = refs
	cells.append(cell)
	by_uid[cell.uid] = cell
	if not _connected:
		(Engine.get_main_loop() as SceneTree).process_frame.connect(tick)
		_connected = true
	return cell

func _edge(cell: Cell, line: PackedVector3Array, kind: int, half: float, density: float, flag: bool) -> void:
	cell.first.append(cell.points.size())
	cell.count.append(line.size())
	var total := 0.0
	for i in line.size():
		if i > 0: total += Vector2(line[i].x - line[i - 1].x, line[i].z - line[i - 1].z).length()
		cell.points.append(line[i])
		cell.arc.append(total)
	cell.kinds.append(kind)
	cell.halves.append(half)
	cell.densities.append(density)
	cell.lengths.append(total)
	cell.signalled.append(1 if flag else 0)

static func _grid(p: Vector3) -> Vector2i:
	return Vector2i(floori(p.x / GRID), floori(p.z / GRID))

## The node within MERGE of `p` (nearest), or a new one.
func _node_at(p: Vector3) -> int:
	var home := _grid(p)
	var best := -1
	var gap := MERGE
	for dz in range(-1, 2):
		for dx in range(-1, 2):
			for node: int in node_grid.get(home + Vector2i(dx, dz), PackedInt32Array()):
				var d := Vector2(node_pos[node].x - p.x, node_pos[node].z - p.z).length()
				if d <= gap:
					gap = d
					best = node
	if best >= 0: return best
	best = next_node
	next_node += 1
	node_pos[best] = p
	node_refs[best] = PackedInt64Array()
	var list: PackedInt32Array = node_grid.get(home, PackedInt32Array())
	list.append(best)
	node_grid[home] = list
	return best

## Drops a cell (Cell.release): its edges, emptied nodes and walkers; a
## cache-owned field releases its claim pin.
func remove(cell: Cell) -> void:
	var index := cells.find(cell)
	if index < 0: return
	cells.remove_at(index)
	by_uid.erase(cell.uid)
	for e in cell.first.size():
		for end in 2:
			var node := cell.nodes[e * 2 + end]
			var refs: PackedInt64Array = node_refs.get(node, PackedInt64Array())
			var at := refs.find((cell.uid << 21) | (e << 1) | end)
			if at >= 0: refs.remove_at(at)
			if not refs.is_empty():
				node_refs[node] = refs
				continue
			if node_pos.has(node):
				var key := _grid(node_pos[node])
				var list: PackedInt32Array = node_grid.get(key, PackedInt32Array())
				var k := list.find(node)
				if k >= 0: list.remove_at(k)
				if list.is_empty(): node_grid.erase(key)
				else: node_grid[key] = list
			node_pos.erase(node)
			node_refs.erase(node)
	_drop_walkers(cell)
	_drop_flocks(cell)
	population_at = -1.0
	var cache: RefCounted = resources.get_ref() if resources != null else null
	if cache != null and cache.has_method("release"): cache.release(KEY)
	if cells.is_empty(): _hide()

## Frees every render resource; the field cannot be used again.
func close() -> void:
	if closed: return
	closed = true
	if _connected:
		(Engine.get_main_loop() as SceneTree).process_frame.disconnect(tick)
		_connected = false
	_free_parts()
	for cell: Cell in cells: cell.released = true
	cells.clear()
	by_uid.clear()
	_clear_walkers()
	flocks.clear()
	_prune_pigeons()
	node_pos.clear()
	node_refs.clear()
	node_grid.clear()
	material = null
	lease = null

## Vehicle display positions this frame (up to 8); velocities follow from
## the previous call unless given.
func set_obstacles(points: PackedVector3Array, given := PackedVector3Array()) -> void:
	var now := Time.get_ticks_usec()
	var list := points.slice(0, 8)
	var speeds := PackedVector3Array()
	speeds.resize(list.size())
	if given.size() >= list.size():
		for i in list.size(): speeds[i] = given[i]
	elif obstacle_usec >= 0 and obstacles.size() == list.size():
		var dt := maxf(0.001, float(now - obstacle_usec) / 1000000.0)
		for i in list.size(): speeds[i] = (list[i] - obstacles[i]) / dt if dt < 0.5 else Vector3.ZERO
	obstacles = list
	velocities = speeds
	obstacle_usec = now

## Distance on the ground from `flat` to a cell's rectangle (0 inside it).
static func cell_distance(cell: Cell, flat: Vector2) -> float:
	var r := cell.rect
	return Vector2(maxf(maxf(r.position.x - flat.x, flat.x - r.end.x), 0.0), maxf(maxf(r.position.y - flat.y, flat.y - r.end.y), 0.0)).length()

## Area shares around `eye` (an inverse-distance mix of the registered cells'
## ambience within AMBIENCE_REACH of their rectangles) and the walkers within AMBIENCE_WALKERS.
func ambience_at(eye: Vector3) -> Dictionary:
	var mix := Vector4.ZERO
	var total := 0.0
	var flat := Vector2(eye.x, eye.z)
	for cell: Cell in cells:
		var d := cell_distance(cell, flat)
		if d > AMBIENCE_REACH: continue
		var w := 1.0 / maxf(d, 4.0)
		mix += cell.ambience * w
		total += w
	if total > 0.0: mix /= total
	var near := 0
	for p: Vector3 in w_position:
		if p.distance_to(eye) <= AMBIENCE_WALKERS: near += 1
	return {"grass": mix.x, "asphalt": mix.y, "built": mix.z, "water": mix.w, "walkers_near": near}

## Live walker and pigeon caps and reach of a quality profile.
static func live(value: Dictionary) -> Dictionary:
	var on := bool(value.get("street_life", false))
	return {"walkers": mini(int(value.get("walker_count", 0)), MAX_WALKERS) if on else 0,
		"pigeons": mini(int(value.get("pigeon_count", 0)), MAX_PIGEONS) if on else 0,
		"distance": float(value.get("walker_distance", 0.0)), "near": float(value.get("walker_near", 0.0))}

## Per-frame update (SceneTree.process_frame).
func tick() -> void:
	if closed or manual: return
	var started := Time.get_ticks_usec()
	var dt := clampf(float(started - last_usec) / 1000000.0, 0.0, 0.1) if last_usec >= 0 else 0.0
	last_usec = started
	tick_step(dt, started)

## One update of `dt` seconds (validators call it directly).
func tick_step(dt: float, started := -1) -> void:
	if closed: return
	if started < 0: started = Time.get_ticks_usec()
	stats.ticks += 1
	clock += dt
	if obstacle_usec >= 0 and started - obstacle_usec > 500000:
		obstacles = PackedVector3Array()
		velocities = PackedVector3Array()
	_refresh_cells()
	var camera := _camera()
	var value: Dictionary = QUALITY.current if not QUALITY.current.is_empty() else QUALITY.profile(1)
	var caps := live(value)
	if cells.is_empty() or camera == null or (int(caps.walkers) <= 0 and int(caps.pigeons) <= 0):
		_clear_walkers()
		flocks.clear()
		_prune_pigeons()
		_hide()
		if cells.is_empty() and _connected:
			(Engine.get_main_loop() as SceneTree).process_frame.disconnect(tick)
			_connected = false
		return
	var world := camera.get_world_3d()
	if world == null: return
	if world.scenario != scenario or parts.is_empty():
		scenario = world.scenario
		_build_parts()
	_read_conditions()
	var eye := camera.global_position
	last_eye = eye
	_walkers_step(dt, eye, camera, caps, started)
	_pigeons_step(dt, eye, caps)
	_draw(eye, float(caps.near))
	stats.peak_tick_usec = maxi(int(stats.peak_tick_usec), Time.get_ticks_usec() - started)

func _camera() -> Camera3D:
	for cell: Cell in cells:
		var node: Node3D = cell.root.get_ref()
		if node != null and node.is_inside_tree(): return node.get_viewport().get_camera_3d()
	return null

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
			population_at = -1.0
			if not shown:
				_drop_walkers(cell)
				_drop_flocks(cell)

func _read_conditions() -> void:
	daylight = 1.0
	wet = false
	var cache: RefCounted = resources.get_ref() if resources != null else null
	if cache == null or not cache.has_method("environment_pixel"): return
	var pixel: Variant = cache.environment_pixel()
	if pixel is Color:
		daylight = clampf(pixel.a, 0.0, 1.0)
		wet = pixel.r >= WET_FROM or pixel.g >= WET_FROM

# --- Walkers -----------------------------------------------------------------
# Walker and pigeon state lives in packed arrays (one entry per walker or
# pigeon, swap-removed), not in objects: a GDScript object of this size costs
# about 2 KiB, the arrays under 128 B per slot.

const FORWARD_FLAG := 1
const MOVING_FLAG := 2
const INTENT_FLAG := 4

func walker_count() -> int:
	return w_cell.size()

## Adds a walker on `edge` of `cell` at arc `s`; returns its index.
func add_walker(cell: Cell, edge: int, s: float, forward: bool, pace := 0.4, slot := 0.5, height := HEIGHT, top := 0, bottom := 0) -> int:
	var i := w_cell.size()
	w_cell.append(cell.uid)
	w_edge.append(edge)
	w_segment.append(_segment_of(cell, edge, s))
	w_waiting.append(-1)
	w_flags.append(FORWARD_FLAG if forward else 0)
	w_top.append(top)
	w_bottom.append(bottom)
	w_waited.append(0.0)
	w_boxed.append(0.0)
	w_alpha.append(0.0)
	w_age.append(0.0)
	w_s.append(s)
	w_pace.append(pace)
	w_slot.append(slot)
	w_height.append(height)
	w_phase.append(0.0)
	w_lateral.append(0.0)
	w_shift.append(Vector3.ZERO)
	w_position.append(Vector3.ZERO)
	w_direction.append(Vector3.FORWARD)
	w_lateral[i] = _home(i)
	_place(i)
	return i

func _flag(i: int, flag: int) -> bool:
	return (w_flags[i] & flag) != 0

func _set_flag(i: int, flag: int, value: bool) -> void:
	w_flags[i] = (w_flags[i] | flag) if value else (w_flags[i] & ~flag)

func _remove_walker(i: int) -> void:
	var last := w_cell.size() - 1
	for name: String in WALKER_ARRAYS:
		var list: Variant = get(name)
		if i != last: list[i] = list[last]
		list.resize(last)
		set(name, list)

func _clear_walkers() -> void:
	for name: String in WALKER_ARRAYS:
		var list: Variant = get(name)
		list.clear()
		set(name, list)

## Drops the walkers on `cell`.
func _drop_walkers(cell: Cell) -> void:
	for i in range(w_cell.size() - 1, -1, -1):
		if w_cell[i] == cell.uid: _remove_walker(i)

func _walkers_step(dt: float, eye: Vector3, camera: Camera3D, caps: Dictionary, started: int) -> void:
	var reach := float(caps.distance)
	var cap := int(caps.walkers)
	if cap <= 0 or reach <= 0.0:
		_clear_walkers()
		return
	if population_at < 0.0 or clock - population_at >= POPULATION_SECONDS:
		population_at = clock
		_count_population(eye, reach, cap)
	for i in range(w_cell.size() - 1, -1, -1):
		if Vector2(w_position[i].x - eye.x, w_position[i].z - eye.z).length() > reach + DROP_MARGIN: _remove_walker(i)
	while w_cell.size() > population:
		_drop_one(camera)
	var spawned := 0
	while w_cell.size() < population and spawned < SPAWNS_PER_TICK and Time.get_ticks_usec() - started < STEP_USEC:
		spawned += 1
		_spawn(eye, reach, camera)
	for i in w_cell.size(): _walk(i, dt)

## Walkers wanted: min(cap, Σ lane length × density × time × weather) over
## lane segments whose middle lies within `reach`; keeps the spawn candidates.
func _count_population(eye: Vector3, reach: float, cap: int) -> void:
	candidate_cell.clear()
	candidate_edge.clear()
	candidate_sum.clear()
	candidate_weight = 0.0
	var total := 0.0
	for cell: Cell in cells:
		if not cell.visible or cell.first.is_empty(): continue
		if cell_distance(cell, Vector2(eye.x, eye.z)) > reach: continue
		for e in cell.first.size():
			if cell.kinds[e] != LANE or cell.lengths[e] <= 0.0: continue
			var inside := 0.0
			for k in cell.count[e] - 1:
				var a := cell.points[cell.first[e] + k]
				var b := cell.points[cell.first[e] + k + 1]
				var mid := (a + b) * 0.5
				if Vector2(mid.x - eye.x, mid.z - eye.z).length() <= reach: inside += Vector2(b.x - a.x, b.z - a.z).length()
			if inside <= 0.0: continue
			var weight := inside * cell.densities[e]
			total += weight
			candidate_weight += weight
			candidate_cell.append(cell.uid)
			candidate_edge.append(e)
			candidate_sum.append(candidate_weight)
	var factor := lerpf(TIME_FACTOR.x, TIME_FACTOR.y, daylight) * (WET_FACTOR if wet else 1.0)
	population = mini(cap, int(floor(total * factor + 0.5)))

## Drops the walker farthest away, out of view first.
func _drop_one(camera: Camera3D) -> void:
	var worst := 0
	var score := -INF
	for i in w_cell.size():
		var d := w_position[i].distance_to(camera.global_position) + (0.0 if camera.is_position_in_frustum(w_position[i]) else 1000.0)
		if d > score:
			score = d
			worst = i
	_remove_walker(worst)

func _spawn(eye: Vector3, reach: float, camera: Camera3D) -> void:
	if candidate_cell.is_empty() or candidate_weight <= 0.0: return
	var chosen: Array = []
	var fallback: Array = []
	for attempt in 8:
		var pick := mini(candidate_sum.bsearch(rng.randf() * candidate_weight), candidate_sum.size() - 1)
		var cell: Cell = by_uid.get(candidate_cell[pick])
		if cell == null: continue
		var e: int = candidate_edge[pick]
		var s := rng.randf() * cell.lengths[e]
		var p := _point(cell, e, s)
		var d := Vector2(p.x - eye.x, p.z - eye.z).length()
		if d < reach * SPAWN.x or d > reach * SPAWN.y: continue
		if not camera.is_position_in_frustum(p):
			chosen = [cell, e, s]
			break
		if fallback.is_empty(): fallback = [cell, e, s]
	if chosen.is_empty(): chosen = fallback
	if chosen.is_empty(): return
	var i := add_walker(chosen[0], chosen[1], chosen[2], rng.randf() < 0.5, rng.randf_range(PACE.x, PACE.y), rng.randf_range(SLOT.x, SLOT.y),
		HEIGHT * rng.randf_range(HEIGHT_RANGE.x, HEIGHT_RANGE.y), rng.randi_range(0, 15), [0, 1, 2, 3, 1, 0, 11, 8, 10, 6][rng.randi_range(0, 9)])
	w_phase[i] = rng.randf()
	stats.spawned += 1

## Lateral offset a walker keeps: its slot of the half width on its side.
func _home(i: int) -> float:
	var cell: Cell = by_uid[w_cell[i]]
	var side := -1.0 if cell.left_hand else 1.0
	return side * w_slot[i] * cell.halves[w_edge[i]]

func _segment_of(cell: Cell, e: int, s: float) -> int:
	var first := cell.first[e]
	var k := 0
	while k < cell.count[e] - 2 and cell.arc[first + k + 1] < s: k += 1
	return k

func _point(cell: Cell, e: int, s: float) -> Vector3:
	var k := _segment_of(cell, e, s)
	var first := cell.first[e]
	var a := cell.points[first + k]
	var b := cell.points[first + k + 1]
	var span := cell.arc[first + k + 1] - cell.arc[first + k]
	return a.lerp(b, clampf((s - cell.arc[first + k]) / span, 0.0, 1.0) if span > 0.0 else 0.0)

## Centreline point, walking direction and position of walker `i`.
func _place(i: int) -> void:
	var cell: Cell = by_uid[w_cell[i]]
	var e := w_edge[i]
	var s := w_s[i]
	var first := cell.first[e]
	var k := w_segment[i]
	while k < cell.count[e] - 2 and cell.arc[first + k + 1] < s: k += 1
	while k > 0 and cell.arc[first + k] > s: k -= 1
	w_segment[i] = k
	var a := cell.points[first + k]
	var b := cell.points[first + k + 1]
	var span := cell.arc[first + k + 1] - cell.arc[first + k]
	var centre := a.lerp(b, clampf((s - cell.arc[first + k]) / span, 0.0, 1.0) if span > 0.0 else 0.0)
	var along := Vector3(b.x - a.x, 0.0, b.z - a.z).normalized()
	if along == Vector3.ZERO: along = Vector3.FORWARD
	var direction := along if _flag(i, FORWARD_FLAG) else -along
	w_direction[i] = direction
	w_position[i] = centre + Vector3(-direction.z, 0.0, direction.x) * w_lateral[i] + w_shift[i]

func _walk(i: int, dt: float) -> void:
	w_age[i] += dt
	w_shift[i] = w_shift[i].move_toward(Vector3.ZERO, maxf(w_pace[i], 0.3) * dt)
	var cell: Cell = by_uid[w_cell[i]]
	var half: float = cell.halves[w_edge[i]]
	var target := _home(i)
	var pace := w_pace[i]
	var speed := pace
	var limit := half
	var position := w_position[i]
	var direction := w_direction[i]
	if w_waiting[i] >= 0:
		_hold(i, dt)
		speed = 0.0
	else:
		var threatened := false
		var right := Vector2(-direction.z, direction.x)
		for o in obstacles.size():
			var rel := Vector2(obstacles[o].x - position.x, obstacles[o].z - position.z)
			if rel.length() > GIVE_WAY: continue
			var relative := Vector2(velocities[o].x, velocities[o].z) - Vector2(direction.x, direction.z) * pace
			var meet := 0.0
			if relative.length_squared() > 1e-6: meet = clampf(-rel.dot(relative) / relative.length_squared(), 0.0, MEET_SECONDS + 1.0)
			if meet > MEET_SECONDS or (rel + relative * meet).length() > MEET_GAP: continue
			threatened = true
			var across := rel.dot(right)
			# On the sidewalk itself (curbs are 3 cm): room beyond the lane.
			var on_walk := absf(obstacles[o].y - position.y) < 0.2 and absf(across) <= half + ASIDE_OUT
			limit = half + (ASIDE_OUT if on_walk else 0.0)
			target = -signf(across if absf(across) > 0.01 else 1.0) * limit
			speed = pace * 0.5
			if absf(w_lateral[i] - target) < 0.02 and rel.length() < 1.2:
				speed = 0.0
				w_boxed[i] += dt
				if w_boxed[i] >= BOXED_SECONDS:
					_turn(i)
					stats.turned += 1
			break
		if threatened: stats.gave_way += 1
		else: w_boxed[i] = 0.0
	w_lateral[i] = move_toward(w_lateral[i], clampf(target, -limit, limit), ASIDE_SPEED * dt)
	_set_flag(i, MOVING_FLAG, speed > 0.0)
	if speed > 0.0:
		w_phase[i] = fposmod(w_phase[i] + speed * dt * STRIDE * 0.5, 1.0)
		cell = by_uid[w_cell[i]]
		var length: float = cell.lengths[w_edge[i]]
		var s := w_s[i] + speed * dt * (1.0 if _flag(i, FORWARD_FLAG) else -1.0)
		w_s[i] = clampf(s, 0.0, length)
		if s >= length or s <= 0.0: _arrive(i)
	else:
		# Feet together when standing.
		w_phase[i] = move_toward(w_phase[i], roundf(w_phase[i] * 2.0) * 0.5, dt)
	_place(i)

func _turn(i: int) -> void:
	_set_flag(i, FORWARD_FLAG, not _flag(i, FORWARD_FLAG))
	w_lateral[i] = -w_lateral[i]
	w_boxed[i] = 0.0

## At the end of an edge: a crossing one time in three where a crossing (or
## the kerb link leading to one) leaves the node, unsignalled; else another
## lane or link (never a spur ending at the kerb of a signalled crossing),
## else turn round. A walker that took a kerb link crosses at its mouth.
func _arrive(i: int) -> void:
	var here: Cell = by_uid[w_cell[i]]
	var edge := w_edge[i]
	var node := here.nodes[edge * 2 + (1 if _flag(i, FORWARD_FLAG) else 0)]
	var crossings: Array = []
	var routes: Array = []
	var others: Array = []
	var from_crossing := here.kinds[edge] == CROSSING
	for ref: int in node_refs.get(node, PackedInt64Array()):
		var cell: Cell = by_uid.get(ref >> 21)
		if cell == null or not cell.visible: continue
		var e := (ref >> 1) & 0xFFFFF
		if cell == here and e == edge: continue
		match _route(cell, e, ref & 1):
			2: crossings.append(ref)
			3: routes.append(ref)
			1: others.append(ref)
	var intent := _flag(i, INTENT_FLAG)
	_set_flag(i, INTENT_FLAG, false)
	if not crossings.is_empty() and not from_crossing and (intent or rng.randf() < CROSS_SHARE):
		var ref: int = crossings[rng.randi_range(0, crossings.size() - 1)]
		if _crossing_clear(by_uid[ref >> 21], (ref >> 1) & 0xFFFFF): _enter(i, ref)
		else:
			w_waiting[i] = ref
			w_waited[i] = 0.0
			stats.held += 1
		return
	if not routes.is_empty() and not from_crossing and crossings.is_empty() and rng.randf() < CROSS_SHARE:
		_enter(i, routes[rng.randi_range(0, routes.size() - 1)])
		_set_flag(i, INTENT_FLAG, true)
		return
	others.append_array(routes)
	if not others.is_empty():
		_enter(i, others[rng.randi_range(0, others.size() - 1)])
		return
	_turn(i)
	stats.turned += 1

## How an edge leaving a node at its end `end` serves a walker: 0 not at all
## (a hidden cell's edge, a signalled crossing, or a link ending only at
## those), 1 a lane or link, 2 an unsignalled crossing, 3 a kerb link to one.
func _route(cell: Cell, e: int, end: int) -> int:
	if not cell.visible: return 0
	if cell.kinds[e] == CROSSING: return 2 if cell.signalled[e] == 0 else 0
	if cell.kinds[e] == LANE: return 1
	var far := cell.nodes[e * 2 + (1 - end)]
	var onward := false
	for ref: int in node_refs.get(far, PackedInt64Array()):
		var other: Cell = by_uid.get(ref >> 21)
		if other == null or not other.visible: continue
		var k := (ref >> 1) & 0xFFFFF
		if other == cell and k == e: continue
		if other.kinds[k] == CROSSING:
			if other.signalled[k] == 0: return 3
		else: onward = true
	return 1 if onward else 0

func _enter(i: int, ref: int) -> void:
	var before := w_position[i]
	var old: Cell = by_uid[w_cell[i]]
	var side := w_lateral[i] / maxf(0.01, old.halves[w_edge[i]])
	var cell: Cell = by_uid[ref >> 21]
	var e := (ref >> 1) & 0xFFFFF
	var at_end := (ref & 1) == 1
	if cell.kinds[e] == CROSSING: stats.crossed += 1
	w_cell[i] = cell.uid
	w_edge[i] = e
	_set_flag(i, FORWARD_FLAG, not at_end)
	_set_flag(i, INTENT_FLAG, false)
	w_s[i] = cell.lengths[e] if at_end else 0.0
	w_segment[i] = _segment_of(cell, e, w_s[i])
	w_lateral[i] = clampf(side, -1.0, 1.0) * cell.halves[e]
	w_shift[i] = Vector3.ZERO
	w_waiting[i] = -1
	_place(i)
	var shift := before - w_position[i]
	w_shift[i] = shift if shift.length() <= MERGE * 2.0 else Vector3.ZERO

## Waiting at the kerb: enter once the crossing is clear, walk on after
## CURB_WAIT; turn round when its cell is gone or hidden.
func _hold(i: int, dt: float) -> void:
	w_waited[i] += dt
	var ref := w_waiting[i]
	var cell: Cell = by_uid.get(ref >> 21)
	if cell == null or not cell.visible:
		w_waiting[i] = -1
		_turn(i)
		return
	if _crossing_clear(cell, (ref >> 1) & 0xFFFFF):
		_enter(i, ref)
	elif w_waited[i] >= CURB_WAIT:
		w_waiting[i] = -1
		_turn(i)

## No vehicle within CROSS_CLEAR of the crossing, now or along its course
## over the next CROSS_ARRIVAL seconds.
func _crossing_clear(cell: Cell, e: int) -> bool:
	var a := cell.points[cell.first[e]]
	var b := cell.points[cell.first[e] + 1]
	var p := Vector2(a.x, a.z)
	var q := Vector2(b.x, b.z)
	for o in obstacles.size():
		var v := Vector2(velocities[o].x, velocities[o].z)
		for step in 4:
			var at := Vector2(obstacles[o].x, obstacles[o].z) + v * (CROSS_ARRIVAL * step / 3.0)
			if Geometry2D.get_closest_point_to_segment(at, p, q).distance_to(at) <= CROSS_CLEAR: return false
	return true

# --- Pigeons -----------------------------------------------------------------

func pigeon_count() -> int:
	return p_flock.size()

func _remove_pigeon(i: int) -> void:
	var last := p_flock.size() - 1
	for name: String in PIGEON_ARRAYS:
		var list: Variant = get(name)
		if i != last: list[i] = list[last]
		list.resize(last)
		set(name, list)

func _drop_flocks(cell: Cell) -> void:
	for key: String in flocks.keys():
		if flocks[key].cell == cell: flocks.erase(key)
	_prune_pigeons()

func _prune_pigeons() -> void:
	for i in range(p_flock.size() - 1, -1, -1):
		if not flocks.has(p_flock[i]): _remove_pigeon(i)

func _pigeons_step(dt: float, eye: Vector3, caps: Dictionary) -> void:
	var cap := int(caps.pigeons)
	var allowed := cap > 0 and daylight >= DAY_FROM and not wet
	var wanted := {}
	if allowed:
		for cell: Cell in cells:
			if not cell.visible or cell.anchors.is_empty(): continue
			if cell_distance(cell, Vector2(eye.x, eye.z)) > PIGEON_REACH: continue
			for a in cell.anchors.size():
				var anchor := cell.anchors[a]
				if Vector2(anchor.x - eye.x, anchor.z - eye.z).length() <= PIGEON_REACH: wanted["%d:%d" % [cell.uid, a]] = [cell, anchor]
	for key: String in flocks.keys():
		if not wanted.has(key) and (not allowed or not flocks[key].airborne): flocks.erase(key)
	_prune_pigeons()
	for key: String in wanted:
		if flocks.has(key): continue
		var anchor: Vector3 = wanted[key][1]
		var draw := absi(hash(Vector3i(roundi(anchor.x * 100.0), roundi(anchor.y * 100.0), roundi(anchor.z * 100.0))))
		var size := FLOCK.x + draw % (FLOCK.y - FLOCK.x + 1)
		if p_flock.size() + size > cap: continue
		flocks[key] = {"cell": wanted[key][0], "anchor": anchor, "airborne": false}
		for k in size:
			var angle := rng.randf() * TAU
			var home := anchor + Vector3(cos(angle), 0.0, sin(angle)) * rng.randf_range(0.35, 1.1)
			p_flock.append(key)
			p_home.append(home)
			p_position.append(home)
			p_flying.append(0)
			p_time.append(0.0)
			p_angle.append(0.0)
			p_phase.append(0.0)
			p_alpha.append(0.0)
			p_rise.append(0.0)
			p_radius.append(0.0)
			p_duration.append(0.0)
			p_yaw.append(rng.randf() * TAU)
			p_tone.append(rng.randf())
			p_idle.append(rng.randf_range(0.5, 3.0))
	for key: String in flocks:
		var flock: Dictionary = flocks[key]
		if not flock.airborne and _flush(key):
			flock.airborne = true
			stats.flushed += 1
			for i in p_flock.size():
				if p_flock[i] != key: continue
				p_flying[i] = 1
				p_time[i] = 0.0
				p_rise[i] = rng.randf_range(FLIGHT_RISE.x, FLIGHT_RISE.y)
				p_radius[i] = rng.randf_range(FLIGHT_RADIUS.x, FLIGHT_RADIUS.y)
				p_duration[i] = rng.randf_range(FLIGHT_SECONDS.x, FLIGHT_SECONDS.y)
				p_angle[i] = atan2(p_home[i].z - flock.anchor.z, p_home[i].x - flock.anchor.x)
	for i in p_flock.size(): _fly(i, dt)
	for key: String in flocks:
		var flock: Dictionary = flocks[key]
		if not flock.airborne: continue
		var landed := true
		for i in p_flock.size():
			if p_flock[i] == key and p_flying[i] != 0: landed = false
		if landed: flock.airborne = false

## A vehicle within FLUSH_NEAR of a pigeon of the flock, or within FLUSH_FAST above FLUSH_SPEED.
func _flush(key: String) -> bool:
	for o in obstacles.size():
		var reach := FLUSH_FAST if Vector2(velocities[o].x, velocities[o].z).length() > FLUSH_SPEED else FLUSH_NEAR
		for i in p_flock.size():
			if p_flock[i] == key and Vector2(obstacles[o].x - p_position[i].x, obstacles[o].z - p_position[i].z).length() <= reach: return true
	return false

func _fly(i: int, dt: float) -> void:
	p_alpha[i] = minf(1.0, p_alpha[i] + dt / FADE_IN)
	if p_flying[i] == 0:
		p_idle[i] -= dt
		if p_idle[i] <= 0.0:
			p_idle[i] = rng.randf_range(0.8, 3.5)
			p_yaw[i] += rng.randf_range(-1.2, 1.2)
		p_phase[i] = 0.0
		p_position[i] = p_home[i]
		return
	p_time[i] += dt
	p_phase[i] = fposmod(p_phase[i] + dt * 6.0, 1.0)
	var anchor: Vector3 = flocks[p_flock[i]].anchor
	p_angle[i] += TAU / maxf(4.0, p_duration[i] * 0.6) * dt
	var circle := anchor + Vector3(cos(p_angle[i]) * p_radius[i], p_rise[i], sin(p_angle[i]) * p_radius[i])
	var blend := smoothstep(0.0, 1.5, p_time[i]) * (1.0 - smoothstep(p_duration[i] - 1.5, p_duration[i], p_time[i]))
	var before := p_position[i]
	p_position[i] = p_home[i].lerp(circle, blend)
	var travel := p_position[i] - before
	if Vector2(travel.x, travel.z).length() > 1e-4: p_yaw[i] = atan2(-travel.x, -travel.z)
	if p_time[i] >= p_duration[i]:
		p_flying[i] = 0
		p_position[i] = p_home[i]

# --- Drawing -----------------------------------------------------------------

func _build_parts() -> void:
	_free_parts()
	var meshes := MESHES.shared()
	material = ShaderMaterial.new()
	material.shader = MESHES.SHADER
	material.resource_name = "mk_street_life"
	if lease != null: lease.track(material)
	for key: String in ["near", "far", "pigeon"]:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.use_custom_data = true
		multi.mesh = meshes[key]
		multi.instance_count = MAX_PIGEONS if key == "pigeon" else MAX_WALKERS
		multi.visible_instance_count = 0
		var instance := RenderingServer.instance_create2(multi.get_rid(), scenario)
		RenderingServer.instance_geometry_set_material_override(instance, material.get_rid())
		RenderingServer.instance_geometry_set_cast_shadows_setting(instance, RenderingServer.SHADOW_CASTING_SETTING_OFF)
		RenderingServer.instance_set_visible(instance, false)
		if lease != null: lease.track(multi)
		parts[key] = {"multimesh": multi, "instance": instance, "shown": 0, "aabb_eye": Vector3.INF}

func _free_parts() -> void:
	for part: Dictionary in parts.values(): RenderingServer.free_rid(part.instance)
	parts.clear()

func _hide() -> void:
	for part: Dictionary in parts.values():
		if int(part.shown) > 0:
			part.shown = 0
			(part.multimesh as MultiMesh).visible_instance_count = 0
			RenderingServer.instance_set_visible(part.instance, false)

## One instance row: the transform (basis rows with the origin) then INSTANCE_CUSTOM.
static func _row(basis: Basis, origin: Vector3, custom: Vector4) -> PackedFloat32Array:
	return PackedFloat32Array([basis.x.x, basis.y.x, basis.z.x, origin.x, basis.x.y, basis.y.y, basis.z.y, origin.y,
		basis.x.z, basis.y.z, basis.z.z, origin.z, custom.x, custom.y, custom.z, custom.w])

## Fade at the camera and inside a vehicle's box.
func _fade(p: Vector3, eye: Vector3) -> float:
	var alpha := smoothstep(CAMERA_FADE.x, CAMERA_FADE.y, p.distance_to(eye))
	for o: Vector3 in obstacles:
		alpha *= smoothstep(VEHICLE_FADE.x, VEHICLE_FADE.y, Vector2(o.x - p.x, o.z - p.z).length())
	return alpha

func _draw(eye: Vector3, near_reach: float) -> void:
	if parts.is_empty(): return
	var near := PackedFloat32Array()
	var far := PackedFloat32Array()
	var birds := PackedFloat32Array()
	for i in w_cell.size():
		var at := w_position[i]
		w_alpha[i] = minf(1.0, w_age[i] / FADE_IN) * _fade(at, eye)
		if w_alpha[i] <= 0.0 or near.size() + far.size() >= MAX_WALKERS * FLOATS: continue
		var close := at.distance_to(eye) <= near_reach
		var basis := Basis.looking_at(w_direction[i], Vector3.UP).scaled(Vector3.ONE * w_height[i])
		var row := _row(basis, at, Vector4((w_top[i] + 0.5) / 16.0, (w_bottom[i] + 0.5) / 16.0, w_phase[i], w_alpha[i]))
		if close: near.append_array(row)
		else: far.append_array(row)
	for i in p_flock.size():
		var alpha := p_alpha[i] * _fade(p_position[i], eye)
		if alpha <= 0.0 or birds.size() >= MAX_PIGEONS * FLOATS: continue
		birds.append_array(_row(Basis(Vector3.UP, p_yaw[i]), p_position[i], Vector4(p_tone[i], float(p_flying[i]), p_phase[i], alpha)))
	for key: String in parts:
		var part: Dictionary = parts[key]
		var multi: MultiMesh = part.multimesh
		var buffer: PackedFloat32Array = near if key == "near" else (far if key == "far" else birds)
		var shown := buffer.size() / FLOATS
		if shown > 0:
			buffer.resize(multi.instance_count * FLOATS)
			multi.buffer = buffer
			# Culling box around the camera, moved per part when it draws.
			var centre: Vector3 = part.aabb_eye
			if not centre.is_finite() or centre.distance_to(eye) > 4.0:
				part.aabb_eye = eye
				RenderingServer.instance_set_custom_aabb(part.instance, AABB(eye - Vector3(AABB_REACH, 24.0, AABB_REACH), Vector3(AABB_REACH * 2.0, 48.0, AABB_REACH * 2.0)))
		multi.visible_instance_count = shown
		if (shown > 0) != (int(part.shown) > 0): RenderingServer.instance_set_visible(part.instance, shown > 0)
		part.shown = shown

## Draw work: drawn walkers (near, far), pigeons and visible draw calls.
func draw_cost() -> Dictionary:
	var result := {"near": 0, "far": 0, "pigeons": 0, "visible_draws": 0}
	for key: String in parts:
		var shown := int(parts[key].shown)
		result["pigeons" if key == "pigeon" else key] = shown
		result.visible_draws += int(shown > 0)
	return result
