extends SceneTree
## The map edge stage is a bounded, display-only node: translucent panels on the
## document rectangle or a virtual fence on the play-area rings, a ground skirt
## outside it, and a sky ground tint setter. The shared fence geometry the Runtime
## turns into collision boxes is checked here too.
const EDGE := preload("../edge_stage.gd")
const FENCE := preload("../play_area_fence.gd")
const RENDERER := preload("../environment_renderer.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed=true;push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var bounds := PackedInt64Array([0, 0, 217600, 358400])
	var area := EDGE.rectangle(bounds, 1.0)
	check(area.position.is_equal_approx(Vector2(0.0, -3584.0)) and area.end.is_equal_approx(Vector2(2176.0, 0.0)), "rectangle maps y to -z in scene metres")
	check(EDGE.rectangle(bounds, 2.0).size.is_equal_approx(Vector2(4352.0, 7168.0)), "rectangle follows the world scale")
	check(not EDGE.rectangle(PackedInt64Array(), 1.0).has_area() and not EDGE.rectangle(bounds, 0.0).has_area(), "invalid bounds or scale give no area")
	var empty := EDGE.create(PackedInt64Array(), 1.0, Color.GRAY)
	check(empty.name == EDGE.NAME and empty.get_child_count() == 0, "missing bounds build an empty stage")
	empty.free()
	var stage := EDGE.create(bounds, 1.0, Color("8e8f88"), 0.0)
	root.add_child(stage)
	check(stage.name == EDGE.NAME and stage.get_meta("edge_rectangle") == area, "stage carries its rectangle")
	var pending: Array[Node] = [stage]
	while not pending.is_empty():
		var node: Node = pending.pop_back()
		check(not node is CollisionObject3D and not node is CollisionShape3D, "display-only nodes")
		pending.append_array(node.get_children())
	var wall: MeshInstance3D = stage.get_node("EdgeWall")
	var mesh: ArrayMesh = wall.mesh
	check(mesh.get_surface_count() == 1 and mesh.surface_get_array_len(0) == 16 and mesh.surface_get_array_index_len(0) == 24, "four wall panels, eight triangles")
	var vertices: PackedVector3Array = mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	var colors: PackedColorArray = mesh.surface_get_arrays(0)[Mesh.ARRAY_COLOR]
	for index in vertices.size():
		var v := vertices[index]
		var on_edge := is_equal_approx(v.x, area.position.x) or is_equal_approx(v.x, area.end.x) or is_equal_approx(v.z, area.position.y) or is_equal_approx(v.z, area.end.y)
		check(on_edge and (is_zero_approx(v.y) or is_equal_approx(v.y, EDGE.WALL_HEIGHT_M)), "wall vertex on the rectangle at floor or top height: %s" % v)
		# Surface colours are stored as 8-bit channels.
		check(absf(colors[index].a - (EDGE.WALL_ALPHA if is_zero_approx(v.y) else 0.0)) <= 1.0 / 255.0, "wall alpha fades from the floor to the top")
	var paint: StandardMaterial3D = wall.material_override
	check(paint.transparency == BaseMaterial3D.TRANSPARENCY_ALPHA and paint.shading_mode == BaseMaterial3D.SHADING_MODE_UNSHADED and paint.cull_mode == BaseMaterial3D.CULL_DISABLED and paint.vertex_color_use_as_albedo, "translucent unshaded two-sided wall")
	check(wall.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "wall casts no shadow")
	var skirts := 0
	for child: Node in stage.get_children():
		if not child.name.begins_with("EdgeSkirt"): continue
		skirts += 1
		var box: BoxMesh = (child as MeshInstance3D).mesh
		var slab := Rect2(Vector2(child.position.x, child.position.z) - Vector2(box.size.x, box.size.z) * 0.5, Vector2(box.size.x, box.size.z))
		check(not slab.intersects(area) and slab.has_area(), "%s lies outside the map" % child.name)
		check(child.position.y < 0.0 and (child.material_override as StandardMaterial3D).albedo_color == Color("8e8f88"), "%s sits just below the floor in the ground colour" % child.name)
		for other: Node in stage.get_children():
			if other == child or not other.name.begins_with("EdgeSkirt"): continue
			var other_box: BoxMesh = (other as MeshInstance3D).mesh
			var other_slab := Rect2(Vector2(other.position.x, other.position.z) - Vector2(other_box.size.x, other_box.size.z) * 0.5, Vector2(other_box.size.x, other_box.size.z))
			check(not slab.intersects(other_slab), "%s and %s do not overlap" % [child.name, other.name])
	check(skirts == 4, "four skirt slabs")
	var ring := Rect2(area.position - Vector2.ONE * EDGE.SKIRT_MARGIN_M, area.size + Vector2.ONE * EDGE.SKIRT_MARGIN_M * 2.0)
	var covered := 0.0
	for child: Node in stage.get_children():
		if child.name.begins_with("EdgeSkirt"): covered += (child as MeshInstance3D).mesh.size.x * (child as MeshInstance3D).mesh.size.z
	check(is_equal_approx(covered, ring.get_area() - area.get_area()), "skirts cover the whole margin ring")
	var again := EDGE.create(bounds, 1.0, Color("8e8f88"), 0.0)
	check(again.get_child_count() == stage.get_child_count() and (again.get_node("EdgeSkirtEast") as Node3D).position == (stage.get_node("EdgeSkirtEast") as Node3D).position, "deterministic layout")
	again.free()
	var raised := EDGE.create(bounds, 1.0, Color.WHITE, 3.0)
	check(is_equal_approx(((raised.get_node("EdgeWall") as MeshInstance3D).mesh as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_VERTEX][0].y, 3.0) and (raised.get_node("EdgeSkirtNorth") as Node3D).position.y < 3.0, "terrain base lifts the wall and skirt")
	raised.free()
	stage.free()
	# Play-area rings: one fence quad per ring edge, per-vertex floors.
	var area_rings := [[[1000, 1000], [200000, 1000], [100000, 300000]], [[150000, 320000], [160000, 320000], [155000, 330000]]]
	var outlined := EDGE.create(bounds, 1.0, Color.GRAY, 2.0, EDGE.SKIRT_MARGIN_M, area_rings)
	pending.clear()
	pending.append(outlined)
	while not pending.is_empty():
		var node: Node = pending.pop_back()
		check(not node is CollisionObject3D and not node is CollisionShape3D, "the fence display carries no collision")
		pending.append_array(node.get_children())
	var ring_wall: MeshInstance3D = outlined.get_node("EdgeWall")
	var ring_arrays: Array = (ring_wall.mesh as ArrayMesh).surface_get_arrays(0)
	var ring_vertices: PackedVector3Array = ring_arrays[Mesh.ARRAY_VERTEX]
	var ring_uvs: PackedVector2Array = ring_arrays[Mesh.ARRAY_TEX_UV]
	check(ring_vertices.size() == 24 and outlined.get_meta("edge_rings").size() == 2, "one quad per ring edge (3 + 3), no extra fence geometry")
	check(ring_vertices[0].is_equal_approx(Vector3(10.0, 2.0 - EDGE.SINK_M, -10.0)) and is_equal_approx(ring_vertices[2].y, 2.0 + EDGE.WALL_HEIGHT_M), "ring fence stands on the floor, sunk below it, up to the wall height")
	check(is_equal_approx(EDGE.SINK_M, FENCE.SINK_M), "display and collision share the sink depth")
	check(ring_uvs[0].is_equal_approx(Vector2(0.0, -EDGE.SINK_M)) and is_equal_approx(ring_uvs[2].y, EDGE.WALL_HEIGHT_M) and is_equal_approx(ring_uvs[4].x, ring_uvs[1].x) and is_equal_approx(ring_uvs[1].x, 1990.0), "UV is metres along the ring and above the floor, continuous across corners")
	check(ring_wall.material_override is ShaderMaterial and outlined.get_node_or_null("EdgeSkirtNorth") != null, "fence shader; the rectangle skirt stays")
	var fence_paint: ShaderMaterial = ring_wall.material_override
	check(is_equal_approx(fence_paint.get_shader_parameter("alpha"), EDGE.FENCE_ALPHA) and EDGE.FENCE_ALPHA >= 0.3, "translucent cyan hologram")
	check(is_equal_approx(FENCE.VISIBLE_HEIGHT_M, 12.0) and is_equal_approx(fence_paint.get_shader_parameter("wall_m"), FENCE.VISIBLE_HEIGHT_M), "visible and physical barrier share the twelve metre height")
	check(EDGE.STRIPE_SHADER.contains("EMISSION") and EDGE.STRIPE_SHADER.contains("float circle") and not EDGE.STRIPE_SHADER.contains("post_m"), "emissive rims and no-entry symbols without fence posts")
	check(ring_wall.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "fence casts no shadow")
	var floors := [PackedFloat32Array([5.0, 6.0, 7.0]), PackedFloat32Array([1.0, 1.0, 1.0])]
	check(EDGE.set_floors(outlined, floors), "floors follow sampled ground")
	ring_vertices = ((outlined.get_node("EdgeWall") as MeshInstance3D).mesh as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	check(is_equal_approx(ring_vertices[1].y, 6.0 - EDGE.SINK_M) and outlined.get_children().filter(func(n: Node) -> bool: return n.name == "EdgeWall").size() == 1, "one rebuilt wall on the new floors")
	check((outlined.get_node("EdgeWall") as MeshInstance3D).material_override == fence_paint, "the rebuilt wall keeps the fence material")
	check(not EDGE.set_floors(outlined, [PackedFloat32Array([1.0])]), "a floor count mismatch is refused")
	# Shared ground fitting (single-player and Host/Join views): requests per cell,
	# one downward ray per vertex that passes through the Runtime fence colliders.
	var requests := EDGE.floor_requests(outlined, 1.0, func(x: int, y: int) -> PackedInt32Array: return PackedInt32Array([floori(x / 100000.0), floori(y / 100000.0)]))
	check(requests.get(Vector2i(0, 0)) == [[0, 0]] and requests.get(Vector2i(2, 0)) == [[0, 1]] and requests.get(Vector2i(1, 3), []).size() == 4, "fence vertices are requested by the cell under them")
	var ground := StaticBody3D.new()
	var ground_shape := CollisionShape3D.new()
	ground_shape.shape = BoxShape3D.new()
	(ground_shape.shape as BoxShape3D).size = Vector3(4.0, 2.0, 4.0)
	ground.add_child(ground_shape)
	ground.position = Vector3(10.0, 3.0, -10.0)
	var fence_body := StaticBody3D.new()
	fence_body.set_meta(FENCE.META, true)
	var fence_shape := CollisionShape3D.new()
	fence_shape.shape = BoxShape3D.new()
	(fence_shape.shape as BoxShape3D).size = Vector3(2.0, 30.0, 2.0)
	fence_body.add_child(fence_shape)
	fence_body.position = Vector3(10.0, 5.0, -10.0)
	root.add_child(ground)
	root.add_child(fence_body)
	await physics_frame
	await physics_frame
	check(EDGE.fit_floors(outlined, root.get_world_3d().direct_space_state, requests, [Vector2i(0, 0)]), "a registered cell refits its vertices")
	check(is_equal_approx(outlined.get_meta("edge_floors")[0][0], 4.0) and not requests.has(Vector2i(0, 0)) and requests.has(Vector2i(2, 0)), "the vertex stands on the ground below the fence top; only that cell's requests are consumed")
	check(not EDGE.fit_floors(outlined, root.get_world_3d().direct_space_state, requests, [Vector2i(5, 5)]), "a cell without fence vertices changes nothing")
	ground.free()
	fence_body.free()
	outlined.free()
	# Honest display allowance: three copies of each included segment fit the base.
	check(EDGE.INCLUDED_RING_SEGMENTS * EDGE.RING_SEGMENT_BYTES * 3 + 16 * 1024 <= EDGE.RESERVATION_BYTES, "base allowance covers the included ring segments")
	check(EDGE.reservation_bytes(area_rings) == EDGE.RESERVATION_BYTES and EDGE.reservation_bytes([]) == EDGE.RESERVATION_BYTES, "small play areas use the base allowance")
	var large := []
	for i in 1000: large.append([roundi(100000 + 90000 * cos(TAU * i / 1000.0)), roundi(150000 + 90000 * sin(TAU * i / 1000.0))])
	check(EDGE.reservation_bytes([large]) == EDGE.RESERVATION_BYTES + (1000 - EDGE.INCLUDED_RING_SEGMENTS) * EDGE.RING_SEGMENT_BYTES * 3, "a larger play area reserves its extra segments")
	# Shared fence geometry (map cm): outward normals, convex-corner extension, clipping.
	check(FENCE.rings_cm([PackedInt64Array([0, 0, 10, 0, 0, 10])]).size() == 1 and FENCE.rings_cm([[[0, 0], [10, 0], [0, 10]]])[0].size() == 3 and FENCE.rings_cm([[[0, 0], [10, 0]]]).is_empty(), "flat and document rings, short rings skipped")
	var square := [PackedVector2Array([Vector2(0, 0), Vector2(1000, 0), Vector2(1000, 1000), Vector2(0, 1000)])]
	var reversed: PackedVector2Array = square[0].duplicate()
	reversed.reverse()
	for ring_set: Array in [square, [reversed]]:
		var segments := FENCE.ring_segments(ring_set, 30.0)
		check(segments.size() == 4, "one segment per square edge")
		for segment: Array in segments:
			var middle: Vector2 = (segment[0] + segment[1]) * 0.5
			check(not Geometry2D.is_point_in_polygon(middle + segment[2] * 10.0, square[0]) and Geometry2D.is_point_in_polygon(middle - segment[2] * 10.0, square[0]), "normal points out of the ring for either winding")
			check(is_equal_approx(segment[0].distance_to(segment[1]), 1060.0), "convex corners extend each side by the thickness")
	var notch := [PackedVector2Array([Vector2(0, 0), Vector2(1000, 0), Vector2(1000, 1000), Vector2(500, 1000), Vector2(500, 500), Vector2(0, 500)])]
	var notch_segments := FENCE.ring_segments(notch, 30.0)
	check(notch_segments[3][0].is_equal_approx(Vector2(500, 1030)) and notch_segments[3][1].is_equal_approx(Vector2(500, 500)), "a right-angle concave corner stays exact (no box reaches inside)")
	# A sharp concave notch (about 164 degrees): untrimmed, a box's outer corner
	# would reach about 29 cm inside; both sides shorten, so no box corner does.
	var sharp := PackedVector2Array([Vector2(0, 0), Vector2(1000, 0), Vector2(1000, 1000), Vector2(600, 1000), Vector2(500, 300), Vector2(400, 1000), Vector2(0, 1000)])
	var sharp_reversed := sharp.duplicate()
	sharp_reversed.reverse()
	for notched: PackedVector2Array in [sharp, sharp_reversed]:
		var inside := 0
		for segment: Array in FENCE.ring_segments([notched], 30.0):
			var a: Vector2 = segment[0]
			var b: Vector2 = segment[1]
			var along := (b - a).normalized()
			for corner: Vector2 in [a + along * 0.5 + segment[2] * 0.5, b - along * 0.5 + segment[2] * 0.5, a + along * 0.5 + segment[2] * 29.5, b - along * 0.5 + segment[2] * 29.5]:
				if Geometry2D.is_point_in_polygon(corner, notched): inside += 1
		check(inside == 0, "no fence box corner lies inside a sharply notched ring (%d)" % inside)
	var clipped := FENCE.clip(Vector2(-10, 50), Vector2(200, 50), Rect2(0, 0, 100, 100))
	check(clipped.size() == 2 and clipped[0].is_equal_approx(Vector2(0, 50)) and clipped[1].is_equal_approx(Vector2(100, 50)) and FENCE.clip(Vector2(-10, 150), Vector2(200, 150), Rect2(0, 0, 100, 100)).is_empty(), "segments clip to the closed rectangle")
	var map_bounds := PackedInt64Array([0, 0, 3000, 2000])
	var fence_ring := [PackedVector2Array([Vector2(100, 100), Vector2(1500, 100), Vector2(1500, 1500), Vector2(100, 1500)])]
	var fence := FENCE.layout(fence_ring, map_bounds, 1000, 1.0)
	check(FENCE.cell_rect(map_bounds, 1000, Vector2i(2, 1)) == Rect2(2000, 1000, 1000, 1000), "cell rectangles follow the document grid")
	var first := FENCE.cell_segments(fence, Vector2i(0, 0))
	var second := FENCE.cell_segments(fence, Vector2i(1, 0))
	check(first.size() == 2 and second.size() == 2, "each straddled cell clips its own ring pieces")
	var bottom_first: Array = first.filter(func(s: Array) -> bool: return is_equal_approx(s[0].y, 100.0))[0]
	var bottom_second: Array = second.filter(func(s: Array) -> bool: return is_equal_approx(s[0].y, 100.0))[0]
	check(bottom_first[1].x >= 1000.0 + 29.0 and bottom_second[0].x <= 1000.0 - 29.0, "pieces overlap across the cell edge so the wall stays continuous")
	check(not FENCE.fenced_out(fence, Vector2i(0, 0)) and not FENCE.fenced_out(fence, Vector2i(1, 1)) and FENCE.fenced_out(fence, Vector2i(2, 0)) and FENCE.fenced_out(fence, Vector2i(2, 1)), "only cells wholly outside every ring are fenced out")
	# A ring 10 cm short of a cell edge: the next cell holds the end of a corner-
	# extended box, yet lies wholly outside the ring.
	var near := FENCE.layout([PackedVector2Array([Vector2(100, 100), Vector2(1990, 100), Vector2(1990, 1500), Vector2(100, 1500)])], map_bounds, 1000, 1.0)
	check(not FENCE.cell_segments(near, Vector2i(2, 0)).is_empty() and FENCE.fenced_out(near, Vector2i(2, 0)) and not FENCE.fenced_out(near, Vector2i(1, 0)), "a cell holding only a box end past the ring is still fenced out")
	check(not FENCE.fenced_out({}, Vector2i(2, 0)) and FENCE.layout([], map_bounds, 1000, 1.0).is_empty(), "no play area: nothing is fenced out")
	var placed := FENCE.box([Vector2(0, 0), Vector2(1000, 0), Vector2(0, -1)], -2.0, 12.0, 1.0)
	var pose: Transform3D = placed[0]
	check(pose.origin.is_equal_approx(Vector3(5.0, 5.0, FENCE.THICKNESS_M * 0.5)) and (placed[1] as Vector3).is_equal_approx(Vector3(10.0, 14.0, FENCE.THICKNESS_M)), "box inner face on the ring, depth outward (map -y = scene +z)")
	check(is_equal_approx(pose.basis.determinant(), 1.0) and pose.basis.x.is_equal_approx(Vector3.RIGHT), "box basis follows the segment")
	check(FENCE.box([Vector2(0, 0), Vector2(0, 0), Vector2(0, -1)], 0.0, 1.0, 1.0).is_empty(), "degenerate segments make no box")
	var grown := FENCE.reach(fence_ring, 31.0)
	check(FENCE.within(Vector2(800, 800), grown) and FENCE.within(Vector2(75, 800), grown) and not FENCE.within(Vector2(60, 800), grown) and not FENCE.within(Vector2(2500, 800), grown), "reach adds the fence allowance beyond the ring only")
	# Sky ground tint: stored before configure, applied to the sky material afterwards.
	var renderer: Node3D = RENDERER.new()
	renderer.set_ground_tint(Color(0.5, 0.4, 0.3), 1.0)
	check(renderer.ground_tint == Color(0.5, 0.4, 0.3) and renderer.ground_tint_weight == 1.0, "tint stored before the sky exists")
	var environment := Environment.new()
	var sun := DirectionalLight3D.new()
	renderer.configure(environment, sun, false)
	renderer.set_ground_tint(Color(0.2, 0.3, 0.4), 2.0)
	check(renderer.sky_material.get_shader_parameter("ground_tint") == Vector3(0.2, 0.3, 0.4) and renderer.sky_material.get_shader_parameter("ground_tint_weight") == 1.0, "tint reaches the sky shader, weight clamped")
	renderer.free()
	sun.free()
	if not failed: print("edge_stage_validator: PASS")
	quit(1 if failed else 0)
