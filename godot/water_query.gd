extends RefCounted
## Shared current water contract; scene z is negative map y. No physics nodes.
var _cells: Dictionary = {}

static func records(chunk: Dictionary) -> Array:
	if chunk.has("water_bodies_json"):
		var parsed: Variant = JSON.parse_string(chunk.water_bodies_json)
		return parsed if parsed is Array else []
	return chunk.get("water_bodies", [])

func replace(cell: Vector2i, records_value: Array) -> void:
	# One assignment publishes a complete cell. Other owners of the same body
	# remain valid while this owner is released or replaced.
	var prepared: Array = []
	for record: Dictionary in records_value:
		var body: Dictionary = record.body.duplicate(true)
		body.outline = _ring(body.polygon)
		body.holes = []
		for ring: Array in body.islands: body.holes.append(_ring(ring))
		var bounds := Rect2(body.outline[0], Vector2.ZERO)
		for point: Vector2 in body.outline: bounds = bounds.expand(point)
		body.bounds = bounds
		prepared.append(body)
	_cells[cell] = prepared

func release(cell: Vector2i) -> void:
	_cells.erase(cell)

func clear() -> void:
	_cells.clear()

static func _ring(points: Array) -> PackedVector2Array:
	var result := PackedVector2Array()
	for p: Array in points: result.append(Vector2(p[0], -p[1]) * .01)
	return result

static func _inside(point: Vector2, ring: PackedVector2Array) -> bool:
	# Closed shoreline, closed dry islands; mirror core integer edge semantics.
	for i in ring.size():
		if Geometry2D.get_closest_point_to_segment(point, ring[i], ring[(i+1)%ring.size()]).distance_squared_to(point) < 0.00000001: return true
	return Geometry2D.is_point_in_polygon(point, ring)

func column(position: Vector3) -> Dictionary:
	var point := Vector2(position.x, position.z)
	var seen := {}
	var result := {}
	for bodies: Array in _cells.values():
		for body: Dictionary in bodies:
			if seen.has(body.id): continue
			seen[body.id] = true
			if position.y < float(body.bottom_cm)*.01 or not body.bounds.grow(.0001).has_point(point) or not _inside(point, body.outline): continue
			var dry := false
			for ring: PackedVector2Array in body.holes:
				if _inside(point, ring): dry = true; break
			if dry: continue
			if result.is_empty() or float(body.surface_cm)*.01 > float(result.surface) or (float(body.surface_cm)*.01 == float(result.surface) and str(body.id) < str(result.id)):
				result = {"id":body.id, "surface":float(body.surface_cm)*.01, "bottom":float(body.bottom_cm)*.01, "flow":Vector3(body.flow_cm_s[0],0,-body.flow_cm_s[1])*.01}
	return result

func sample(position: Vector3) -> Dictionary:
	var result := column(position)
	return result if not result.is_empty() and position.y <= float(result.surface) else {}
