extends RefCounted
## Shared geometry of the virtual fence on the document `play_area` rings. The
## Runtime attaches one collision box per ring segment clipped to a cell; the edge
## stage draws the visible fence on the same rings. Map coordinates are integer
## centimetres ([x, y], y north = scene -z); lengths and heights are scene metres.
## No Client dependency and no retained state.
## Box depth outward of the ring; the inner face lies on the ring itself.
const THICKNESS_M := 0.3
## Visible striped panel height above the ground (edge stage).
const VISIBLE_HEIGHT_M := 2.4
## Collision top above the highest ground in a cell, and the depth below the lowest.
const COLLISION_HEIGHT_M := 12.0
const SINK_M := 2.0
const POST_SPACING_M := 4.0
## Guard allowance beyond the ring: a footprint corner further out has passed the fence.
const CONTACT_MARGIN_M := 0.01
const OBJECT_ID := "play-area-fence"
## Body/collider meta marking fence colliders (display floor rays skip them).
const META := "play_area_fence"

## Map-cm polygons of the bridge form (flat [x0, y0, ...] PackedInt64Array) or the
## document form ([[x, y], ...]) of the play-area rings; invalid rings are skipped.
static func rings_cm(play_area: Array) -> Array:
	var result: Array = []
	for ring: Variant in play_area:
		var polygon := PackedVector2Array()
		if ring is PackedInt64Array or ring is PackedInt32Array:
			if ring.size() % 2 != 0: continue
			for i in range(0, ring.size(), 2): polygon.append(Vector2(ring[i], ring[i + 1]))
		elif ring is PackedVector2Array: polygon = ring
		elif ring is Array:
			for point: Variant in ring:
				if point is not Array or point.size() != 2: polygon.clear(); break
				polygon.append(Vector2(float(point[0]), float(point[1])))
		if polygon.size() >= 3: result.append(polygon)
	return result

static func _area(ring: PackedVector2Array) -> float:
	var sum := 0.0
	for i in range(ring.size()):
		var a := ring[i]
		var b := ring[(i + 1) % ring.size()]
		sum += a.x * b.y - b.x * a.y
	return sum * 0.5

static func inside_any(point: Vector2, rings: Array) -> bool:
	for ring: PackedVector2Array in rings:
		if Geometry2D.is_point_in_polygon(point, ring): return true
	return false

## Fence segments [a, b, outward unit normal] in map cm. Each segment is lengthened
## by `extend_cm` past a convex ring vertex so neighbouring boxes close the outer
## corner. At a concave vertex turning by more than 90 degrees the box's outer
## corner would reach across the next side into the play area, so both sides are
## shortened there by `extend_cm * |cot(turn)|` (half the side at most); the
## remaining outside notch is narrower than the guard allowance. Gentler concave
## vertices stay exact. No box reaches into the play area.
## A segment whose outward side lies inside another ring (a shared edge) is skipped.
static func ring_segments(rings: Array, extend_cm: float) -> Array:
	var result: Array = []
	for r in range(rings.size()):
		var ring: PackedVector2Array = rings[r]
		var count := ring.size()
		var turn := 1.0 if _area(ring) > 0.0 else -1.0
		var others: Array = rings.duplicate()
		others.remove_at(r)
		for i in range(count):
			var a := ring[i]
			var b := ring[(i + 1) % count]
			var d := b - a
			if d.length_squared() <= 0.0: continue
			var dir := d.normalized()
			var normal := Vector2(dir.y, -dir.x) * turn
			if not others.is_empty() and inside_any((a + b) * 0.5 + normal * maxf(extend_cm, 1.0) * 2.0, others): continue
			var before := a - ring[(i - 1 + count) % count]
			var after := ring[(i + 2) % count] - b
			var length := d.length()
			if before.cross(d) * turn > 0.0: a -= dir * extend_cm
			else: a += dir * _concave_trim(before, dir, extend_cm, length)
			if d.cross(after) * turn > 0.0: b += dir * extend_cm
			else: b -= dir * _concave_trim(after, dir, extend_cm, length)
			result.append([a, b, normal])
	return result

## Shortening of a side at a concave vertex whose neighbouring side runs along
## `other`: zero up to a 90-degree turn, `extend_cm * |cot(turn)|` beyond it.
static func _concave_trim(other: Vector2, dir: Vector2, extend_cm: float, length: float) -> float:
	if other.length_squared() <= 0.0: return 0.0
	var unit := other.normalized()
	var cosine := unit.dot(dir)
	if cosine >= 0.0: return 0.0
	return minf(extend_cm * -cosine / maxf(absf(unit.cross(dir)), 0.000001), length * 0.5)

## Liang-Barsky clip of a..b to a closed rectangle; empty when nothing remains.
static func clip(a: Vector2, b: Vector2, rect: Rect2) -> PackedVector2Array:
	var d := b - a
	var t0 := 0.0
	var t1 := 1.0
	for edge in [[-d.x, a.x - rect.position.x], [d.x, rect.end.x - a.x], [-d.y, a.y - rect.position.y], [d.y, rect.end.y - a.y]]:
		var p: float = edge[0]
		var q: float = edge[1]
		if is_zero_approx(p):
			if q < 0.0: return PackedVector2Array()
			continue
		var t := q / p
		if p < 0.0: t0 = maxf(t0, t)
		else: t1 = minf(t1, t)
		if t0 > t1: return PackedVector2Array()
	if (a + d * t0).is_equal_approx(a + d * t1): return PackedVector2Array()
	return PackedVector2Array([a + d * t0, a + d * t1])

## The ring segments inside `rect` grown by `pad` (map cm), so walls of adjacent
## cells overlap at the shared cell edge instead of leaving a seam.
static func segments_in_rect(segments: Array, rect: Rect2, pad: float) -> Array:
	var result: Array = []
	var area := rect.grow(pad)
	for segment: Array in segments:
		var piece := clip(segment[0], segment[1], area)
		if not piece.is_empty(): result.append([piece[0], piece[1], segment[2]])
	return result

## Map-cm rectangle of a document cell (`map_bounds()` [min_x, min_y, max_x, max_y]).
static func cell_rect(bounds: PackedInt64Array, cell_cm: int, cell: Vector2i) -> Rect2:
	if bounds.size() != 4 or cell_cm <= 0: return Rect2()
	var low := Vector2(bounds[0] + cell.x * cell_cm, bounds[1] + cell.y * cell_cm)
	var high := Vector2(mini(bounds[0] + (cell.x + 1) * cell_cm, bounds[2]), mini(bounds[1] + (cell.y + 1) * cell_cm, bounds[3]))
	return Rect2(low, high - low)

## Per-world layout: extended segments and a cell -> segment index over each
## segment's padded extent, plus a cell -> ring edge index over each unextended
## ring edge's extent (the fenced-out test). Built once per configured map and
## world scale.
static func layout(rings: Array, bounds: PackedInt64Array, cell_cm: int, scale: float) -> Dictionary:
	if rings.is_empty() or bounds.size() != 4 or cell_cm <= 0 or not is_finite(scale) or scale <= 0.0: return {}
	var thickness := THICKNESS_M * 100.0 / scale
	var index := {}
	for segment: Array in ring_segments(rings, thickness):
		_index_span(index, segment, segment[0], segment[1], thickness, bounds, cell_cm)
	var edges := {}
	for ring: PackedVector2Array in rings:
		for i in range(ring.size()):
			var a := ring[i]
			var b := ring[(i + 1) % ring.size()]
			_index_span(edges, [a, b], a, b, 0.0, bounds, cell_cm)
	return {"rings": rings, "bounds": bounds, "cell_cm": cell_cm, "scale": scale, "thickness_cm": thickness, "index": index, "edges": edges}

## Appends `item` to every cell whose rectangle meets the a..b extent grown by `pad`.
static func _index_span(index: Dictionary, item: Array, a: Vector2, b: Vector2, pad: float, bounds: PackedInt64Array, cell_cm: int) -> void:
	var columns := ceili(float(bounds[2] - bounds[0]) / cell_cm)
	var rows := ceili(float(bounds[3] - bounds[1]) / cell_cm)
	var low := a.min(b) - Vector2.ONE * pad
	var high := a.max(b) + Vector2.ONE * pad
	for y in range(clampi(floori((low.y - bounds[1]) / cell_cm), 0, rows - 1), clampi(floori((high.y - bounds[1]) / cell_cm), 0, rows - 1) + 1):
		for x in range(clampi(floori((low.x - bounds[0]) / cell_cm), 0, columns - 1), clampi(floori((high.x - bounds[0]) / cell_cm), 0, columns - 1) + 1):
			var cell := Vector2i(x, y)
			if not index.has(cell): index[cell] = []
			index[cell].append(item)

## Clipped fence segments of one cell (map cm).
static func cell_segments(fence: Dictionary, cell: Vector2i) -> Array:
	if fence.is_empty() or not fence.index.has(cell): return []
	return segments_in_rect(fence.index[cell], cell_rect(fence.bounds, fence.cell_cm, cell), fence.thickness_cm)

## True when the cell rectangle lies wholly outside every ring: nothing in it is
## reachable inside the fence, so it never decides whether a car is held. The
## test uses the ring edges themselves, not the fence boxes: a cell may hold the
## outer end of a corner-extended box (within the thickness of the ring) and still
## be fenced out, because the neighbouring cell's padded piece attaches that box
## too. A ring edge on the cell rectangle's border keeps the cell (conservative).
static func fenced_out(fence: Dictionary, cell: Vector2i) -> bool:
	if fence.is_empty(): return false
	var rect := cell_rect(fence.bounds, fence.cell_cm, cell)
	if not rect.has_area(): return false
	for edge: Array in fence.edges.get(cell, []):
		if not clip(edge[0], edge[1], rect).is_empty(): return false
	return not inside_any(rect.get_center(), fence.rings)

## Scene-space box of one map-cm segment: [Transform3D, size]. The inner face
## stands on the ring and the depth goes outward; `low`/`high` are scene metres.
static func box(segment: Array, low: float, high: float, scale: float) -> Array:
	var factor := scale / 100.0
	var a: Vector2 = segment[0]
	var b: Vector2 = segment[1]
	var normal: Vector2 = segment[2]
	var start := Vector3(a.x, 0.0, -a.y) * factor
	var along := Vector3(b.x, 0.0, -b.y) * factor - start
	var length := along.length()
	if length <= 0.0 or high <= low: return []
	var x := along / length
	var center := start + along * 0.5 + Vector3(normal.x, 0.0, -normal.y) * (THICKNESS_M * 0.5)
	center.y = (low + high) * 0.5
	return [Transform3D(Basis(x, Vector3.UP, x.cross(Vector3.UP)), center), Vector3(length, high - low, THICKNESS_M)]

## Rings grown by `margin_cm` (map cm), for the guard's "passed the fence" test:
## [[outer, [holes...]], ...] from Geometry2D's exact offset.
static func reach(rings: Array, margin_cm: float) -> Array:
	var result: Array = []
	for ring: PackedVector2Array in rings:
		var parts: Array = Geometry2D.offset_polygon(ring, margin_cm, Geometry2D.JOIN_ROUND) if margin_cm > 0.0 else [ring]
		if parts.is_empty(): continue
		parts.sort_custom(func(p: PackedVector2Array, q: PackedVector2Array) -> bool: return absf(_area(p)) > absf(_area(q)))
		result.append([parts[0], parts.slice(1)])
	return result

static func within(point: Vector2, grown: Array) -> bool:
	for entry: Array in grown:
		if not Geometry2D.is_point_in_polygon(point, entry[0]): continue
		var hole := false
		for inner: PackedVector2Array in entry[1]:
			if Geometry2D.is_point_in_polygon(point, inner): hole = true; break
		if not hole: return true
	return false
