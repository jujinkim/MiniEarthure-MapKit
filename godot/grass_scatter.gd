extends RefCounted
## Display-only grass tufts and small flowers on a cell's grass ground triangles.
## Deterministic: every value is an integer hash of the cell and the triangle's
## centimetre vertices (flower patches: of the tuft's 4 m world square), so
## batching order, step boundaries and frame timing never change the result.
## Density is the lower of DENSITY and cap / grass area, so a cell never holds
## more than `cap` tufts (display_quality grass_cap: 0/1500/4000); the near
## renderer shows them within grass_distance (24/40 m) and shrinks them into
## the ground before that range ends. Work is split into steps bounded by
## count and by STEP_USEC of wall time.

const VEGETATION := preload("./vegetation_meshes.gd")
const PLAN := preload("./render_memory.gd")
## Most tufts per square metre of grass.
const DENSITY := 0.9
## Group (one MultiMesh) size; each tuft joins the group of its own position,
## so a group never leaves its 32 m square.
const BUCKET_METRES := 32.0
## Groups per cell: a tuft of a further group is dropped (cells up to 224 m
## square never reach it).
const MAX_GROUPS := PLAN.GRASS_GROUPS
## Cells with less grass keep the lawn shader alone (and claim no shared set).
const MIN_AREA := 32.0
## Flowers cluster: a fifth of the 4 m squares are flower patches where 40 %
## of tufts carry a head (white, yellow or violet); elsewhere 2 %.
const FLOWER_PATCH_METRES := 4.0
const FLOWER_PATCHES := 0.2
const FLOWER_IN_PATCH := 0.4
const FLOWER_ELSEWHERE := 0.02
## Bounded work per admitted step: counts, and wall time checked every
## TIME_CHECK tufts or triangles.
const AREA_STEP := 1024
const SCATTER_TRIANGLES := 256
const SCATTER_INSTANCES := 256
const STEP_USEC := 1500
const TIME_CHECK := 16
## Per-instance MultiMesh floats: 3x4 transform and the custom colour.
const FLOATS := 16

## `parts` is an Array of PackedVector3Array triangle lists (scene metres).
static func begin(parts: Array, cell: Vector2i, cap: int, tint: Color) -> Dictionary:
	var state := {"parts": parts, "cell": cell, "cap": clampi(cap, 0, PLAN.GRASS_INSTANCES), "tint": tint, "stage": "area", "part": 0, "triangle": 0,
		"area": 0.0, "density": 0.0, "instance": 0, "count": -1, "key": 0, "emitted": 0, "groups": {}, "order": [], "peak_usec": 0}
	_settle(state)
	return state

## Moves past parts without a whole triangle left; false when every part is consumed.
static func _settle(state: Dictionary) -> bool:
	while state.part < state.parts.size() and state.triangle * 3 + 2 >= (state.parts[state.part] as PackedVector3Array).size():
		state.part += 1
		state.triangle = 0
	return state.part < state.parts.size()

## Ground grass only: steep banks and slivers keep the lawn shader alone.
static func _area(a: Vector3, b: Vector3, c: Vector3) -> float:
	var normal := (b - a).cross(c - a)
	var length := normal.length()
	return length * 0.5 if length > 1e-6 and absf(normal.y) / length >= 0.6 else 0.0

static func _key(cell: Vector2i, a: Vector3, b: Vector3, c: Vector3) -> int:
	var h := VEGETATION.hash32(cell.x * 73856093 ^ VEGETATION.hash32(cell.y * 19349663))
	for p: Vector3 in [a, b, c]:
		h = VEGETATION.hash32(h ^ VEGETATION.hash32(roundi(p.x * 100.0) * 83492791 ^ roundi(p.z * 100.0) * 2654435761))
	return h

## Next value of a hash chain in [0, 1).
static func _next_unit(chain: Array) -> float:
	chain[0] = VEGETATION.hash32(int(chain[0]) + 0x632be5ab)
	return float(int(chain[0]) & 0xffffff) / 16777216.0

## One bounded step; true when `groups` holds the final tufts.
static func advance(state: Dictionary) -> bool:
	if state.stage == "done": return true
	var started := Time.get_ticks_usec()
	var finished := _area_step(state, started) if state.stage == "area" else _scatter_step(state, started)
	state.peak_usec = maxi(int(state.peak_usec), Time.get_ticks_usec() - started)
	return finished

static func _area_step(state: Dictionary, started: int) -> bool:
	var steps := 0
	var area: float = state.area
	while _settle(state) and steps < AREA_STEP:
		if steps % TIME_CHECK == TIME_CHECK - 1 and Time.get_ticks_usec() - started > STEP_USEC: break
		var part: PackedVector3Array = state.parts[state.part]
		var i: int = state.triangle * 3
		area += _area(part[i], part[i + 1], part[i + 2])
		state.triangle += 1
		steps += 1
	state.area = area
	if _settle(state): return false
	state.density = minf(DENSITY, float(state.cap) / area) if area >= MIN_AREA else 0.0
	state.part = 0
	state.triangle = 0
	_settle(state)
	state.stage = "scatter" if state.density > 0.0 and state.cap > 0 else "done"
	if state.stage == "done": state.parts = []
	return state.stage == "done"

static func _scatter_step(state: Dictionary, started: int) -> bool:
	var triangles := 0
	var emitted := 0
	var tint: Color = state.tint
	var fresh := {} # group -> PackedFloat32Array of this step
	var chain := [0]
	while _settle(state) and triangles < SCATTER_TRIANGLES and emitted < SCATTER_INSTANCES and state.emitted < state.cap:
		if (triangles + emitted) % TIME_CHECK == TIME_CHECK - 1 and Time.get_ticks_usec() - started > STEP_USEC: break
		var part: PackedVector3Array = state.parts[state.part]
		var t: int = state.triangle * 3
		var a := part[t]
		var b := part[t + 1]
		var c := part[t + 2]
		if state.count < 0:
			state.key = _key(state.cell, a, b, c)
			chain[0] = state.key
			state.count = floori(_area(a, b, c) * state.density + _next_unit(chain))
			state.instance = 0
		var key: int = state.key
		while state.instance < state.count and emitted < SCATTER_INSTANCES and state.emitted < state.cap:
			if emitted % TIME_CHECK == TIME_CHECK - 1 and Time.get_ticks_usec() - started > STEP_USEC: break
			var i: int = state.instance
			chain[0] = VEGETATION.hash32(key ^ (i + 1) * 0x9e3779b1)
			var r1 := _next_unit(chain)
			var r2 := _next_unit(chain)
			if r1 + r2 > 1.0: r1 = 1.0 - r1; r2 = 1.0 - r2
			var p := a + (b - a) * r1 + (c - a) * r2 + Vector3(0.0, -0.02, 0.0)
			var yaw := _next_unit(chain) * TAU
			var scale := 0.75 + 0.6 * _next_unit(chain)
			var tall := scale * (0.85 + 0.3 * _next_unit(chain))
			var pick := _next_unit(chain)
			var colour := _next_unit(chain)
			var bright := 0.82 + 0.3 * _next_unit(chain)
			var phase := _next_unit(chain)
			state.instance += 1
			state.emitted += 1
			emitted += 1
			var group := Vector2i(floori(p.x / BUCKET_METRES), floori(p.z / BUCKET_METRES))
			if not state.groups.has(group):
				if state.groups.size() >= MAX_GROUPS: continue
				state.groups[group] = []
				state.order.append(group)
			var patch := VEGETATION.hash32(floori(p.x / FLOWER_PATCH_METRES) * 73856093 ^ floori(p.z / FLOWER_PATCH_METRES) * 19349663)
			var share := FLOWER_IN_PATCH if float(patch & 0xffff) / 65536.0 < FLOWER_PATCHES else FLOWER_ELSEWHERE
			var flower := (1 + int(colour * 3.0) % 3) if pick < share else 0
			var cosine := cos(yaw) * scale
			var sine := sin(yaw) * scale
			if not fresh.has(group): fresh[group] = PackedFloat32Array()
			var values: PackedFloat32Array = fresh[group]
			var n := values.size()
			values.resize(n + FLOATS)
			# Basis(UP, yaw) scaled (scale, tall, scale), rows of the 3x4 transform.
			values[n] = cosine; values[n + 1] = 0.0; values[n + 2] = sine; values[n + 3] = p.x
			values[n + 4] = 0.0; values[n + 5] = tall; values[n + 6] = 0.0; values[n + 7] = p.y
			values[n + 8] = -sine; values[n + 9] = 0.0; values[n + 10] = cosine; values[n + 11] = p.z
			values[n + 12] = tint.r * bright; values[n + 13] = tint.g * bright; values[n + 14] = tint.b * bright
			values[n + 15] = 1.0 + float(flower) + phase * 0.999
			fresh[group] = values
		if state.instance >= state.count:
			state.count = -1
			state.triangle += 1
			triangles += 1
		else: break
	for group: Vector2i in fresh: (state.groups[group] as Array).append(fresh[group])
	if _settle(state) and state.emitted < state.cap: return false
	state.stage = "done"
	state.parts = []
	return true

## Takes the next finished group: {"buffer": PackedFloat32Array, "count": int}
## or {} when none is left.
static func take_group(state: Dictionary) -> Dictionary:
	if state.order.is_empty(): return {}
	var group: Vector2i = state.order.pop_front()
	var buffer := PackedFloat32Array()
	for values: PackedFloat32Array in state.groups[group]: buffer.append_array(values)
	state.groups.erase(group)
	return {"buffer": buffer, "count": buffer.size() / FLOATS, "group": group}
