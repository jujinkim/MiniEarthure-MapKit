extends SceneTree
## The map edge stage is a bounded, display-only node: translucent panels on the
## document rectangle, a ground skirt outside it, and a sky ground tint setter.
const EDGE := preload("../edge_stage.gd")
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
	# Play-area rings: striped panels along each ring, per-vertex floors.
	var area_rings := [[[1000, 1000], [200000, 1000], [100000, 300000]], [[150000, 320000], [160000, 320000], [155000, 330000]]]
	var outlined := EDGE.create(bounds, 1.0, Color.GRAY, 2.0, EDGE.SKIRT_MARGIN_M, area_rings)
	var ring_wall: MeshInstance3D = outlined.get_node("EdgeWall")
	var ring_vertices: PackedVector3Array = (ring_wall.mesh as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	check(ring_vertices.size() == 24 and outlined.get_meta("edge_rings").size() == 2, "one panel per ring edge (3 + 3)")
	check(ring_vertices[0].is_equal_approx(Vector3(10.0, 2.0 - EDGE.SINK_M, -10.0)) and is_equal_approx(ring_vertices[2].y, 2.0 + EDGE.WALL_HEIGHT_M), "ring panels stand on the floor, sunk below it")
	check(ring_wall.material_override is ShaderMaterial and outlined.get_node_or_null("EdgeSkirtNorth") != null, "no-entry stripes; the rectangle skirt stays")
	var floors := [PackedFloat32Array([5.0, 6.0, 7.0]), PackedFloat32Array([1.0, 1.0, 1.0])]
	check(EDGE.set_floors(outlined, floors), "floors follow sampled ground")
	ring_vertices = ((outlined.get_node("EdgeWall") as MeshInstance3D).mesh as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
	check(is_equal_approx(ring_vertices[1].y, 6.0 - EDGE.SINK_M) and outlined.get_children().filter(func(n: Node) -> bool: return n.name == "EdgeWall").size() == 1, "one rebuilt wall on the new floors")
	check(not EDGE.set_floors(outlined, [PackedFloat32Array([1.0])]), "a floor count mismatch is refused")
	outlined.free()
	# Sky ground tint: stored before configure, applied to the sky material afterwards.
	var renderer: Node3D = RENDERER.new()
	renderer.set_ground_tint(Color(0.5, 0.4, 0.3), 1.0)
	check(renderer.ground_tint == Color(0.5, 0.4, 0.3) and renderer.ground_tint_weight == 1.0, "tint stored before the sky exists")
	var environment := Environment.new()
	renderer.configure(environment, DirectionalLight3D.new(), false)
	renderer.set_ground_tint(Color(0.2, 0.3, 0.4), 2.0)
	check(renderer.sky_material.get_shader_parameter("ground_tint") == Vector3(0.2, 0.3, 0.4) and renderer.sky_material.get_shader_parameter("ground_tint_weight") == 1.0, "tint reaches the sky shader, weight clamped")
	renderer.free()
	if not failed: print("edge_stage_validator: PASS")
	quit(1 if failed else 0)
