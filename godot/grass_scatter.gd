extends RefCounted
## Grass ground of one cell for the camera-local grass field (grass_field.gd).
## The cell's grass batches (PackedVector3Array triangle lists in scene metres,
## the same arrays its ground meshes were built from) are indexed by the
## field's 8 m world blocks: `refs` lists (part << 16 | triangle) per block in
## compressed rows (`offsets`), so the field rasterizes one block from only the
## triangles that reach it. Steep banks and slivers keep the lawn shader alone.
## Nothing is copied: the record shares the batch arrays. Work is split into
## steps bounded by count and by STEP_USEC of wall time.

const PLAN := preload("./render_memory.gd")
const BLOCK_METRES := PLAN.GRASS_BLOCK_METRES
## Triangles per admitted step, wall time checked every TIME_CHECK triangles.
const STEP_TRIANGLES := 1024
const STEP_USEC := 1500
const TIME_CHECK := 32
## Batches hold at most 512 triangles (render_memory.TRIANGLES_PER_BATCH).
const PART_SHIFT := 16

## `parts` is an Array of PackedVector3Array triangle lists (scene metres).
static func begin(parts: Array) -> Dictionary:
	var kept: Array = []
	for part: PackedVector3Array in parts:
		if part.size() >= 3 and part.size() / 3 < (1 << PART_SHIFT): kept.append(part)
	return {"parts": kept, "stage": "bounds", "part": 0, "triangle": 0, "low": Vector2(INF, INF), "high": -Vector2(INF, INF),
		"origin": Vector2i.ZERO, "size": Vector2i.ZERO, "counts": PackedInt32Array(), "offsets": PackedInt32Array(), "refs": PackedInt32Array(),
		"cursor": PackedInt32Array(), "peak_usec": 0, "steps": 0}

## Ground grass only: steep banks and slivers keep the lawn shader alone.
static func ground(a: Vector3, b: Vector3, c: Vector3) -> bool:
	var normal := (b - a).cross(c - a)
	var length := normal.length()
	return length > 1e-3 and absf(normal.y) / length >= 0.6

## Block of a coordinate range's low and high end. A triangle ending on a
## block border reaches no texel centre of the next block (EDGE inside).
const EDGE := 0.001
static func low_block(value: float) -> int: return floori((value + EDGE) / BLOCK_METRES)
static func high_block(value: float) -> int: return floori((value - EDGE) / BLOCK_METRES)

## Inclusive block range [low, high] of the triangle's horizontal bounds.
static func block_range(a: Vector3, b: Vector3, c: Vector3) -> Rect2i:
	var low := Vector2i(low_block(minf(a.x, minf(b.x, c.x))), low_block(minf(a.z, minf(b.z, c.z))))
	var high := Vector2i(high_block(maxf(a.x, maxf(b.x, c.x))), high_block(maxf(a.z, maxf(b.z, c.z))))
	return Rect2i(low, (high - low).max(Vector2i.ZERO))

## Horizontal x extent (min, max) of the triangle's slice with z0 <= z <= z1:
## every edge clipped to the band (the slice is their convex hull).
static func row_extent(a: Vector3, b: Vector3, c: Vector3, z0: float, z1: float) -> Vector2:
	var low := INF
	var high := -INF
	for edge in [[a, b], [b, c], [c, a]]:
		var p: Vector3 = edge[0]
		var q: Vector3 = edge[1]
		var t0 := 0.0
		var t1 := 1.0
		var dz := q.z - p.z
		if absf(dz) < 1e-9:
			if p.z < z0 or p.z > z1: continue
		else:
			var u0 := (z0 - p.z) / dz
			var u1 := (z1 - p.z) / dz
			t0 = maxf(t0, minf(u0, u1))
			t1 = minf(t1, maxf(u0, u1))
			if t0 > t1: continue
		var x0 := p.x + (q.x - p.x) * t0
		var x1 := p.x + (q.x - p.x) * t1
		low = minf(low, minf(x0, x1))
		high = maxf(high, maxf(x0, x1))
	return Vector2(low, high)

## Moves past parts without a whole triangle left; false when every part is consumed.
static func _settle(state: Dictionary) -> bool:
	while state.part < state.parts.size() and state.triangle * 3 + 2 >= (state.parts[state.part] as PackedVector3Array).size():
		state.part += 1
		state.triangle = 0
	return state.part < state.parts.size()

## One bounded step; true when the record is complete (record()).
static func advance(state: Dictionary) -> bool:
	if state.stage == "done": return true
	var started := Time.get_ticks_usec()
	var steps := 0
	while _settle(state) and steps < STEP_TRIANGLES:
		if steps % TIME_CHECK == TIME_CHECK - 1 and Time.get_ticks_usec() - started > STEP_USEC: break
		var part: PackedVector3Array = state.parts[state.part]
		var t: int = state.triangle * 3
		var a := part[t]
		var b := part[t + 1]
		var c := part[t + 2]
		if ground(a, b, c):
			match state.stage:
				"bounds":
					state.low = Vector2(minf(state.low.x, minf(a.x, minf(b.x, c.x))), minf(state.low.y, minf(a.z, minf(b.z, c.z))))
					state.high = Vector2(maxf(state.high.x, maxf(a.x, maxf(b.x, c.x))), maxf(state.high.y, maxf(a.z, maxf(b.z, c.z))))
				"count", "fill":
					var span := block_range(a, b, c)
					var ref: int = (int(state.part) << PART_SHIFT) | int(state.triangle)
					for z in range(span.position.y, span.end.y + 1):
						# Only the blocks the triangle's slice of this block row reaches.
						var extent := row_extent(a, b, c, z * BLOCK_METRES, (z + 1) * BLOCK_METRES)
						if extent.x > extent.y: continue
						for x in range(maxi(span.position.x, low_block(extent.x)), mini(span.end.x, high_block(extent.y)) + 1):
							var index: int = (z - state.origin.y) * state.size.x + (x - state.origin.x)
							if state.stage == "count": state.counts[index] += 1
							else:
								state.refs[state.cursor[index]] = ref
								state.cursor[index] += 1
		state.triangle += 1
		steps += 1
	state.steps += 1
	if _settle(state):
		state.peak_usec = maxi(int(state.peak_usec), Time.get_ticks_usec() - started)
		return false
	match state.stage:
		"bounds":
			if state.low.x > state.high.x: _finish(state)
			else:
				state.origin = Vector2i(low_block(state.low.x), low_block(state.low.y))
				state.size = (Vector2i(high_block(state.high.x), high_block(state.high.y)) - state.origin).max(Vector2i.ZERO) + Vector2i.ONE
				state.counts.resize(state.size.x * state.size.y)
				state.counts.fill(0)
				state.stage = "count"
		"count":
			# Compressed rows: offsets[i]..offsets[i + 1] are block i's refs.
			var offsets := PackedInt32Array()
			offsets.resize(state.counts.size() + 1)
			var total := 0
			for i in state.counts.size():
				offsets[i] = total
				total += state.counts[i]
			offsets[state.counts.size()] = total
			state.offsets = offsets
			state.cursor = offsets.slice(0, state.counts.size())
			state.counts = PackedInt32Array()
			state.refs.resize(total)
			state.stage = "fill"
		"fill": _finish(state)
	state.part = 0
	state.triangle = 0
	state.peak_usec = maxi(int(state.peak_usec), Time.get_ticks_usec() - started)
	return state.stage == "done"

static func _finish(state: Dictionary) -> void:
	state.stage = "done"
	state.cursor = PackedInt32Array()
	if state.refs.is_empty(): state.parts = []

## The finished record ({} when the cell has no ground grass): parts, origin
## and size (8 m blocks), offsets and refs.
static func record(state: Dictionary) -> Dictionary:
	if state.stage != "done" or state.refs.is_empty(): return {}
	return {"parts": state.parts, "origin": state.origin, "size": state.size, "offsets": state.offsets, "refs": state.refs}

## Bytes a record holds beyond the shared batch arrays it references.
static func record_bytes(value: Dictionary) -> int:
	if value.is_empty(): return 0
	return (value.offsets as PackedInt32Array).size() * 4 + (value.refs as PackedInt32Array).size() * 4
