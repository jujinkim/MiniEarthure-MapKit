extends RefCounted
## Display-only map edge for bounded package maps: translucent panels standing on
## the document rectangle and a ground skirt in the map's ground colour outside it.
## No collision, occupancy or spawn metadata; the Runtime footprint guard still
## stops vehicles at the rectangle, so drive geometry never changes.
const NAME := "MapEdgeStage"
const WALL_HEIGHT_M := 12.0
const WALL_COLOR := Color(0.55, 0.85, 1.0)
const WALL_ALPHA := 0.30
const SKIRT_MARGIN_M := 2048.0
const SKIRT_THICKNESS_M := 0.5
## Presentation allowance: one wall mesh, four skirt boxes and two materials.
const RESERVATION_BYTES := 256 * 1024

## Scene-metre rectangle of the document bounds ([min_x, min_y, max_x, max_y] cm).
static func rectangle(bounds: PackedInt64Array, world_scale: float) -> Rect2:
	if bounds.size() != 4 or not is_finite(world_scale) or world_scale <= 0.0: return Rect2()
	var factor := world_scale / 100.0
	var low := Vector2(float(bounds[0]), -float(bounds[3])) * factor
	var high := Vector2(float(bounds[2]), -float(bounds[1])) * factor
	return Rect2(low, high - low)

## `ground` is the map's ground colour; `floor_y` the terrain base in scene metres.
static func create(bounds: PackedInt64Array, world_scale: float, ground: Color, floor_y := 0.0, skirt_margin := SKIRT_MARGIN_M) -> Node3D:
	var root := Node3D.new()
	root.name = NAME
	var area := rectangle(bounds, world_scale)
	if not area.has_area(): return root
	root.set_meta("edge_rectangle", area)
	root.set_meta("rc_display_only", true)
	root.add_child(_wall(area, floor_y))
	var ground_paint := StandardMaterial3D.new()
	ground_paint.albedo_color = ground
	ground_paint.roughness = 0.95
	var lo := area.position
	var hi := area.end
	var m := maxf(skirt_margin, 1.0)
	# Four slabs around the rectangle; none overlaps the map or another slab.
	for slab: Array in [
		["EdgeSkirtNorth", Rect2(lo.x - m, lo.y - m, area.size.x + 2.0 * m, m)],
		["EdgeSkirtSouth", Rect2(lo.x - m, hi.y, area.size.x + 2.0 * m, m)],
		["EdgeSkirtWest", Rect2(lo.x - m, lo.y, m, area.size.y)],
		["EdgeSkirtEast", Rect2(hi.x, lo.y, m, area.size.y)]]:
		root.add_child(_skirt(slab[0], slab[1], floor_y, ground_paint))
	return root

static func _wall(area: Rect2, floor_y: float) -> MeshInstance3D:
	var lo := area.position
	var hi := area.end
	var corners := PackedVector2Array([lo, Vector2(hi.x, lo.y), hi, Vector2(lo.x, hi.y)])
	var vertices := PackedVector3Array()
	var colors := PackedColorArray()
	var indices := PackedInt32Array()
	var bottom := Color(WALL_COLOR, WALL_ALPHA)
	var top := Color(WALL_COLOR, 0.0)
	for side in 4:
		var a := corners[side]
		var b := corners[(side + 1) % 4]
		var base := vertices.size()
		vertices.append_array([Vector3(a.x, floor_y, a.y), Vector3(b.x, floor_y, b.y),
			Vector3(b.x, floor_y + WALL_HEIGHT_M, b.y), Vector3(a.x, floor_y + WALL_HEIGHT_M, a.y)])
		colors.append_array([bottom, bottom, top, top])
		indices.append_array([base, base + 1, base + 2, base, base + 2, base + 3])
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_COLOR] = colors
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var paint := StandardMaterial3D.new()
	paint.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	paint.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	paint.vertex_color_use_as_albedo = true
	paint.cull_mode = BaseMaterial3D.CULL_DISABLED
	paint.disable_receive_shadows = true
	paint.albedo_color = Color.WHITE
	var wall := MeshInstance3D.new()
	wall.name = "EdgeWall"
	wall.mesh = mesh
	wall.material_override = paint
	wall.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	wall.gi_mode = GeometryInstance3D.GI_MODE_DISABLED
	return wall

static func _skirt(name: String, slab: Rect2, floor_y: float, paint: Material) -> MeshInstance3D:
	var box := BoxMesh.new()
	box.size = Vector3(slab.size.x, SKIRT_THICKNESS_M, slab.size.y)
	var skirt := MeshInstance3D.new()
	skirt.name = name
	skirt.mesh = box
	skirt.material_override = paint
	skirt.position = Vector3(slab.get_center().x, floor_y - SKIRT_THICKNESS_M * 0.5 - 0.02, slab.get_center().y)
	skirt.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	skirt.gi_mode = GeometryInstance3D.GI_MODE_DISABLED
	return skirt
