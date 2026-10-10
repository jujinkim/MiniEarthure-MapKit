extends RefCounted
## Shared bounded screen-space facility pins. They never create physics bodies.
const MAX_VISIBLE := 128
const FONT_SIZE := 14
const COLOR := Color("41d9c7")

static func layout(pois: Array, project: Callable, viewport_rect: Rect2, font: Font) -> Array:
	var placed: Array = []
	var occupied: Array[Rect2] = []
	# Input uses canonical stable IDs; callers preserve source order between frames.
	for poi: Dictionary in pois:
		if placed.size() >= MAX_VISIBLE: break
		var point: Variant = project.call(poi.position)
		if point is not Vector2 or not point.is_finite() or not viewport_rect.grow(-8).has_point(point): continue
		var label: String = str(poi.name).left(64)
		var extent := font.get_string_size(label, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE)
		extent.x = minf(extent.x, 240.0)
		var pin := Rect2(point - Vector2(6, 6), Vector2(12, 12))
		if occupied.any(func(rect: Rect2): return rect.intersects(pin)): continue
		var box := Rect2(point + Vector2(9, -extent.y * 0.5), extent + Vector2(8, 4))
		var show_label := viewport_rect.encloses(box) and not occupied.any(func(rect: Rect2): return rect.intersects(box))
		occupied.append(pin)
		if show_label: occupied.append(box)
		placed.append({"point":point, "label":label if show_label else "", "box":box})
	return placed

static func paint(canvas: CanvasItem, entries: Array, font: Font) -> void:
	for entry: Dictionary in entries:
		canvas.draw_circle(entry.point, 5, Color("102725"), true, -1, true)
		canvas.draw_circle(entry.point, 3, COLOR, true, -1, true)
		if entry.label == "": continue
		canvas.draw_rect(entry.box, Color("102725dd"))
		canvas.draw_string(font, entry.box.position + Vector2(4, font.get_ascent(FONT_SIZE)+2), entry.label, HORIZONTAL_ALIGNMENT_LEFT, 240, FONT_SIZE, Color.WHITE)
