extends RefCounted
## Display-only map edge for bounded package maps: translucent panels standing on
## the document rectangle, or striped no-entry panels along the `play_area` rings
## when the map has them, and a ground skirt in the map's ground colour outside the
## rectangle. No collision, occupancy or spawn metadata; the Runtime footprint
## guard still stops vehicles at the same edge, so drive geometry never changes.
const NAME := "MapEdgeStage"
const WALL_HEIGHT_M := 12.0
const WALL_COLOR := Color(0.55, 0.85, 1.0)
const WALL_ALPHA := 0.30
const SKIRT_MARGIN_M := 2048.0
const SKIRT_THICKNESS_M := 0.5
## Presentation allowance: one wall mesh, four skirt boxes and two materials.
const RESERVATION_BYTES := 256 * 1024
## Play-area panels: diagonal no-entry stripes of STRIPE_M, sunk SINK_M into the
## ground below each vertex floor so sloped terrain shows no gap.
const STRIPE_M := 3.0
const SINK_M := 1.0
const STRIPE_SHADER := """shader_type spatial;
render_mode unshaded, cull_disabled, blend_mix, depth_draw_never, shadows_disabled;
uniform float stripe_m = 3.0;
uniform float alpha = 0.3;
varying float along;
varying float rise;
void vertex() { along = UV.x; rise = UV.y; }
void fragment() {
	bool red = fract((along + rise * 12.0) / (2.0 * stripe_m)) < 0.5;
	ALBEDO = red ? vec3(0.9, 0.12, 0.1) : vec3(0.95, 0.95, 0.92);
	ALPHA = alpha * clamp(1.0 - rise, 0.0, 1.0);
}
"""

## Scene-metre rings of the document `play_area` (map cm [[x, y], ...] rings).
static func rings(play_area: Array, world_scale: float) -> Array:
	var result: Array = []
	if not is_finite(world_scale) or world_scale <= 0.0: return result
	var factor := world_scale / 100.0
	for ring: Variant in play_area:
		if ring is not Array or ring.size() < 3: continue
		var points := PackedVector2Array()
		for point: Variant in ring:
			if point is not Array or point.size() != 2: return []
			points.append(Vector2(float(point[0]), -float(point[1])) * factor)
		result.append(points)
	return result

## Scene-metre rectangle of the document bounds ([min_x, min_y, max_x, max_y] cm).
static func rectangle(bounds: PackedInt64Array, world_scale: float) -> Rect2:
	if bounds.size() != 4 or not is_finite(world_scale) or world_scale <= 0.0: return Rect2()
	var factor := world_scale / 100.0
	var low := Vector2(float(bounds[0]), -float(bounds[3])) * factor
	var high := Vector2(float(bounds[2]), -float(bounds[1])) * factor
	return Rect2(low, high - low)

## `ground` is the map's ground colour; `floor_y` the terrain base in scene metres.
## `play_area` is the document's ring list; empty keeps the rectangle wall.
static func create(bounds: PackedInt64Array, world_scale: float, ground: Color, floor_y := 0.0, skirt_margin := SKIRT_MARGIN_M, play_area: Array = []) -> Node3D:
	var root := Node3D.new()
	root.name = NAME
	var area := rectangle(bounds, world_scale)
	if not area.has_area(): return root
	root.set_meta("edge_rectangle", area)
	root.set_meta("rc_display_only", true)
	var outline := rings(play_area, world_scale)
	if outline.is_empty():
		root.add_child(_wall(area, floor_y))
	else:
		var floors: Array = []
		for ring: PackedVector2Array in outline:
			var levels := PackedFloat32Array()
			levels.resize(ring.size())
			levels.fill(floor_y)
			floors.append(levels)
		root.set_meta("edge_rings", outline)
		root.set_meta("edge_floors", floors)
		root.add_child(_ring_wall(outline, floors))
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

## Rebuilds the play-area panels on per-vertex floors (scene metres, one
## PackedFloat32Array per ring), e.g. ground heights a consumer sampled once the
## cells under them loaded. Returns false for a rectangle stage or a size mismatch.
static func set_floors(root: Node3D, floors: Array) -> bool:
	if not is_instance_valid(root) or not root.has_meta("edge_rings"): return false
	var outline: Array = root.get_meta("edge_rings")
	if floors.size() != outline.size(): return false
	for i in range(outline.size()):
		if floors[i] is not PackedFloat32Array or floors[i].size() != outline[i].size(): return false
	var previous := root.get_node_or_null("EdgeWall")
	var wall := _ring_wall(outline, floors)
	if previous != null:
		wall.material_override = previous.material_override
		root.remove_child(previous)
		previous.free()
	root.add_child(wall)
	root.set_meta("edge_floors", floors)
	return true

static func _ring_wall(outline: Array, floors: Array) -> MeshInstance3D:
	var vertices := PackedVector3Array()
	var uvs := PackedVector2Array()
	var indices := PackedInt32Array()
	for r in range(outline.size()):
		var ring: PackedVector2Array = outline[r]
		var levels: PackedFloat32Array = floors[r]
		var along := 0.0
		for i in range(ring.size()):
			var j := (i + 1) % ring.size()
			var a := ring[i]
			var b := ring[j]
			var length := a.distance_to(b)
			var base := vertices.size()
			vertices.append_array([Vector3(a.x, levels[i] - SINK_M, a.y), Vector3(b.x, levels[j] - SINK_M, b.y),
				Vector3(b.x, levels[j] + WALL_HEIGHT_M, b.y), Vector3(a.x, levels[i] + WALL_HEIGHT_M, a.y)])
			uvs.append_array([Vector2(along, 0.0), Vector2(along + length, 0.0), Vector2(along + length, 1.0), Vector2(along, 1.0)])
			indices.append_array([base, base + 1, base + 2, base, base + 2, base + 3])
			along += length
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_TEX_UV] = uvs
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	if not vertices.is_empty(): mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var shader := Shader.new()
	shader.code = STRIPE_SHADER
	var paint := ShaderMaterial.new()
	paint.shader = shader
	paint.set_shader_parameter("stripe_m", STRIPE_M)
	paint.set_shader_parameter("alpha", WALL_ALPHA)
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
