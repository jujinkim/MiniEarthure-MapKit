extends SceneTree
## Park vegetation (vegetation_meshes.gd, chunk_renderer; the grass field has
## grass_field_validator): reservations equal to the estimate (less unused
## grass on low and polar ground), tree/shrub body, card and
## far ranges and shadows per tier, collision trunk coverage, crown reach
## inside every placement footprint (street 120 cm, park zones up to 200 cm),
## visible branches, the shared set's memory, and release of every cell and
## shared reservation.
const RENDER := preload("../chunk_renderer.gd")
const VEGETATION := preload("../vegetation_meshes.gd")
const QUALITY := preload("../display_quality.gd")
const CACHE := preload("../render_resource_cache.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	var refs: Array[WeakRef] = []
	var sealed := false
	func _init(amount := 0) -> void: bytes = amount
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed = true
	func retired() -> bool: return sealed and refs.all(func(value): return value.get_ref() == null)

func record_reservation(bytes: int, box: Array, lease: Lease) -> Lease:
	box[0] = bytes
	return lease

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

func run() -> void:
	meshes()
	await renderer()
	await rendered_palette()
	QUALITY.current = {}
	print("vegetation_scatter_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## A 64 m square of 2 m ground triangles (2,048), a steep bank and a sliver.
func ground(offset := Vector3.ZERO) -> PackedVector3Array:
	var points := PackedVector3Array()
	for z in 32:
		for x in 32:
			var a := offset + Vector3(x * 2.0, 0.0, -z * 2.0)
			points.append_array([a, a + Vector3(2, 0, 0), a + Vector3(0, 0, -2), a + Vector3(2, 0, 0), a + Vector3(2, 0, -2), a + Vector3(0, 0, -2)])
	points.append_array([offset + Vector3(0, 0, 4), offset + Vector3(4, 0, 4), offset + Vector3(0, 6, 4)])
	return points

func meshes() -> void:
	var first: Dictionary = VEGETATION.begin()
	var steps := 0
	var timings := []
	while true:
		var name: String = VEGETATION.STEPS[first.step]
		var started := Time.get_ticks_usec()
		var done := VEGETATION.advance(first)
		timings.append("%s %d" % [name, Time.get_ticks_usec() - started])
		if done: break
		steps += 1
	print("shared vegetation steps (usec): ", ", ".join(timings))
	var second: Dictionary = VEGETATION.begin()
	while not VEGETATION.advance(second): pass
	check(steps + 1 == VEGETATION.STEPS.size() and steps > 20, "the shared set builds one part per step: %d" % steps)
	# Crown shapes are build scratch, released with each variant's last part.
	check(VEGETATION._crowns.is_empty() and VEGETATION._shapes.is_empty(), "no crown shapes outlive the build: %d" % (VEGETATION._crowns.size() + VEGETATION._shapes.size()))
	var vertex_count := 0
	var index_count := 0
	for plant in VEGETATION.PLANTS:
		for part in VEGETATION.KINDS:
			var kind: String = plant + "_" + part
			for variant in 3:
				var a: ArrayMesh = first.result[kind][variant]
				var b: ArrayMesh = second.result[kind][variant]
				if part == "cards" and VEGETATION.REGIONAL.data.has(plant):
					check(a.get_surface_count()==0 and b.get_surface_count()==0,"regional opaque crowns need no alpha cards")
					continue
				check(a.get_surface_count() == 1 and a.surface_get_arrays(0) == b.surface_get_arrays(0), "%s %d builds identically" % [kind, variant])
				vertex_count += a.surface_get_array_len(0)
				index_count += a.surface_get_array_index_len(0)
	# Shared set memory: CPU arrays and GPU buffers (56 B per vertex: position,
	# normal, colour, two UVs; 4 B per index), the leaf atlas as decoded image
	# and mipmapped texture (2 x 350 KiB) and 128 KiB for the two materials
	# and one mesh's build arrays stay inside SHARED_BYTES.
	var mesh_bytes := 2 * (vertex_count * 56 + index_count * 4)
	print("shared vegetation set: %d vertices, %d indices, %d B arrays of %d B" % [vertex_count, index_count, mesh_bytes, VEGETATION.SHARED_BYTES])
	var templates := 0
	for plant: Dictionary in VEGETATION.REGIONAL.data.values():
		templates += 4096
		for variant: Array in plant.variants: templates += variant.size()*512
	check(mesh_bytes + 2 * 350 * 1024 + 131072 + templates <= VEGETATION.SHARED_BYTES, "the shared set's meshes, templates and atlas fit its allowance: %d B" % (mesh_bytes+templates))
	# Every vertex at the largest scale plus its largest wind sway (high tier:
	# SWAY_PEAK x TREE_SWAY x its weight, cards another FLUTTER) stays inside
	# the placement footprint (street trees CITY_TREE_RADIUS 120 cm, zone trees
	# their declared 120-200 cm via tree_set; shrubs the baseline 42 cm); the
	# body's shell grown back to the full crown on low, where nothing sways.
	var reaches := {}
	for plant in VEGETATION.PLANTS:
		for variant in 3:
			var points := PackedVector2Array() # (horizontal reach at scale 1, sway metres)
			for part in VEGETATION.KINDS:
				var mesh: ArrayMesh = first.result[plant+"_"+part][variant]
				if mesh.get_surface_count()==0: continue
				var arrays: Array = mesh.surface_get_arrays(0)
				var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
				var marks: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
				var weights: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV2]
				for i in vertices.size():
					var p := vertices[i]
					points.append(Vector2(Vector2(p.x, p.z).length(), weights[i].x * (VEGETATION.SWAY_PEAK * VEGETATION.TREE_SWAY + (VEGETATION.FLUTTER if part == "cards" else 0.0))))
					if part == "body" and marks[i].y > 0.0:
						p = Vector3(0.0, marks[i].x, 0.0) + (p - Vector3(0.0, marks[i].x, 0.0)) * marks[i].y
						points.append(Vector2(Vector2(p.x, p.z).length(), 0.0))
			reaches["%s_%d" % [plant, variant]] = points
	var reach_at := func(key: String, largest: float) -> float:
		var reach := 0.0
		for point: Vector2 in reaches[key]: reach = maxf(reach, point.x * largest + point.y)
		return reach
	var widest := {}
	for key: String in reaches:
		widest[key] = 0.0
		for point: Vector2 in reaches[key]: widest[key] = maxf(widest[key], point.x)
	for variant in 3:
		var shrub: float = reach_at.call("shrub_%d" % variant, VEGETATION.SHRUB_SCALE.y)
		check(shrub <= 0.421 and shrub >= 0.42 * 0.85, "shrub %d reaches %.3f m of 0.42 m" % [variant, shrub])
	for radius in [120, 140, 159, 160, 180, 200]:
		var set: Dictionary = VEGETATION.tree_set(radius)
		check(set.plant == ("wide" if radius >= VEGETATION.WIDE_FROM_CM else "tree"), "a %d cm footprint grows %s trees" % [radius, set.plant])
		var largest: float = float(set.factor) * (set.span as Vector2).y
		for variant in 3:
			var reach: float = reach_at.call("%s_%d" % [set.plant, variant], largest)
			# Oval park crowns are narrower by design.
			var least := 0.75 if set.plant == "wide" and variant == 1 else 0.85
			check(reach <= radius / 100.0 + 0.001 and reach >= radius / 100.0 * least, "%s %d at %d cm reaches %.3f m with sway" % [set.plant, variant, radius, reach])
	for style in ["conifer","palm","orchard","scrub","polar_scrub"]:
		var tree: bool = style in ["conifer","palm","orchard"]
		for radius in ([120,160,200] if tree else [42]):
			var set := VEGETATION.regional_set("vegetation/"+style,tree,radius)
			for variant in 3:
				var reach: float = reach_at.call("%s_%d" % [set.plant,variant],float(set.factor)*set.span.y)
				check(reach<=float(radius)/100.0,"regional crown and wind stay inside declared footprint: "+style)
		var object := {"id":"regional-"+style,"asset_id":"builtin:city_tree" if tree else "builtin:shrub","position":[0,0,0],"quarter_turns":0}
		var placed := RENDER.vegetation_placement(object,"",120,"vegetation/"+style)
		check(placed.mesh == VEGETATION.regional_kind("vegetation/"+style,tree),"regional instance selects its source style")
	# Wide crowns are barely squeezed (distinct widths) and keep square cards.
	for variant in 3:
		var shape: Dictionary = VEGETATION.crown("wide", variant)
		check(shape.fit >= 0.9, "wide %d keeps its shape: fit %.3f" % [variant, shape.fit])
		var corners: PackedVector3Array = (first.result.wide_cards[variant] as ArrayMesh).surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
		var skewed := 0
		for c in range(0, corners.size(), 4):
			var a := (corners[c + 1] - corners[c]).length()
			var b := (corners[c + 3] - corners[c]).length()
			if absf(a - b) > 0.01 * maxf(a, b) + 0.0001: skewed += 1
		check(skewed == 0, "wide %d leaf cards stay square: %d skewed" % [variant, skewed])
	var sizes := []
	for plant in ["tree", "wide"]:
		var limits: Array = [400, 600, 240, 400] if plant == "tree" else [480, 560, 560, 700]
		for variant in 3:
			var body: ArrayMesh = first.result[plant + "_body"][variant]
			var cards: ArrayMesh = first.result[plant + "_cards"][variant]
			var triangles := body.surface_get_array_index_len(0) / 3
			var card_triangles := cards.surface_get_array_index_len(0) / 3
			check(triangles >= limits[0] and triangles <= limits[1] and card_triangles >= limits[2] and card_triangles <= limits[3], "near %s %d: trunk, branches and hull (%d) plus leaf cards (%d)" % [plant, variant, triangles, card_triangles])
			var far: ArrayMesh = first.result[plant + "_far"][variant]
			check(far.surface_get_array_index_len(0) / 3 <= 120, "far %s %d is coarse" % [plant, variant])
			var arrays: Array = body.surface_get_arrays(0)
			var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
			var colors: PackedFloat32Array = arrays[Mesh.ARRAY_CUSTOM0]
			var marks: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
			# The 14 cm collision box (half diagonal 0.099 m) stays inside the
			# trunk up to 1.4 m at the set's smallest scale: every trunk ring
			# (bark, CUSTOM0.a 0) up to the first one above 1.4 m / that scale,
			# between which the tube is linear.
			var smallest := VEGETATION.smallest_scale(plant)
			var rings: Array = VEGETATION.trunk(plant)
			var narrowest := INF
			var underside := INF
			var top := 0.0
			for ring: Vector2 in rings:
				if top < 1.4 / smallest: top = ring.x
			check(top >= 1.4 / smallest, "%s trunk rings reach above the box at the smallest scale" % plant)
			for i in vertices.size():
				if marks[i].y > 0.0: underside = minf(underside, vertices[i].y)
				if colors[i*4+3] > 0.5 or vertices[i].y > top + 0.001 or vertices[i].y < 0.0: continue
				if not rings.any(func(ring): return absf(ring.x - vertices[i].y) < 0.0005): continue
				narrowest = minf(narrowest, Vector2(vertices[i].x, vertices[i].z).length() * cos(PI / 8.0) * smallest)
			check(narrowest >= 0.099, "%s trunk %d covers the collision box: %.3f m" % [plant, variant, narrowest])
			# Branches fork below the crown's underside: some bark away from the
			# trunk axis lies under the hull.
			var forks := 0
			for i in vertices.size():
				if colors[i*4+3] < 0.5 and vertices[i].y < underside - 0.2 and Vector2(vertices[i].x, vertices[i].z).length() > 0.2: forks += 1
			check(forks >= 10, "%s %d shows branches under its crown: %d vertices" % [plant, variant, forks])
			var aabb := body.get_aabb().merge(cards.get_aabb())
			sizes.append("%s %d: %.2f m wide, %.2f m tall at scale 1, %d+%d triangles" % [plant, variant, 2.0 * widest["%s_%d" % [plant, variant]], aabb.end.y, triangles, card_triangles])
			if plant == "wide":
				# Park crowns: about 4.7-5.7 m tall over the wide span at a 200 cm
				# footprint, the leafy crown (cards) no taller than 1.1 x its width.
				var tall := aabb.end.y
				check(tall * VEGETATION.WIDE_SCALE.x >= 4.6 and tall * VEGETATION.WIDE_SCALE.y <= 7.2, "wide %d is %.2f-%.2f m tall" % [variant, tall * VEGETATION.WIDE_SCALE.x, tall * VEGETATION.WIDE_SCALE.y])
				var across: float = 2.0 * widest["wide_%d" % variant]
				check(cards.get_aabb().size.y <= 1.1 * across, "wide %d crown is %.2f m tall, %.2f m wide" % [variant, cards.get_aabb().size.y, across])
	print("tree sizes: ", "; ".join(sizes))
	for kind in ["body", "cards"]:
		var material: ShaderMaterial = first.result.materials[kind]
		check(material.get_meta("mapkit_vegetation_shader", "") == kind, "%s material opts into vegetation quality" % kind)
		for level in 3:
			QUALITY.apply_material(material, QUALITY.profile(level))
			check(material.get_shader_parameter("wind_strength") == QUALITY.profile(level).vegetation_motion, "wind follows the tier")
			check(material.get_shader_parameter("cards_hidden") == (level == 0), "low grows the body hull in place of cards")

func fixture(polar := false) -> Dictionary:
	var points := ground()
	var chunk := {"cell": Vector2i(2, 5), "triangles": [], "objects": [], "scene_vertices": points, "scene_normals": PackedVector3Array(), "wall_uv": PackedVector2Array()}
	for i in points.size() / 3:
		chunk.triangles.append({"surface": "grass", "object_id": "ground"})
		for k in 3:
			chunk.scene_normals.append(Vector3.UP)
			chunk.wall_uv.append(Vector2.ZERO)
	chunk.presentation = {"city_tree_radii": {}}
	if polar: chunk.presentation.environment_json = JSON.stringify({"climate": "polar"})
	for i in 12:
		chunk.objects.append({"id": "tree-%d" % i, "asset_id": "builtin:city_tree", "position": [i * 500, 0, i * 300], "quarter_turns": 0})
		# Half are park-zone trees with the widest footprint.
		if i % 2 == 0: chunk.presentation.city_tree_radii["tree-%d" % i] = 200
	for i in 6:
		chunk.objects.append({"id": "shrub-%d" % i, "asset_id": "builtin:shrub", "position": [i * 400, 0, 2000], "quarter_turns": 1})
	chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	return chunk

func renderer() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.quality_profile = QUALITY.profile(1)
	for polar in [false, true]:
		for level in 3:
			QUALITY.current = QUALITY.profile(level)
			var parent := Node3D.new()
			root.add_child(parent)
			var chunk := fixture(polar)
			var planned := RENDER.PLAN.estimate(chunk, 0)
			check(RENDER.PLAN.upper_bound({"triangles": chunk.triangles.size(), "objects": chunk.objects.size()}) >= planned, "the estimate stays within the upper bound")
			var lease := Lease.new(planned)
			var reserved := [0]
			var job := RENDER.begin(chunk, parent, record_reservation.bind(reserved, lease), planned, cache)
			# A cell growing no grass (low, polar) returns its grass reservation.
			var unused := RENDER.PLAN.grass_bytes(RENDER.PLAN.batch_grass_triangles(chunk)) if level == 0 or polar else 0
			check(reserved[0] == planned - unused, "tier %d polar %s reserves the estimate less unused grass: %d of %d" % [level, polar, reserved[0], planned])
			var steps := 0
			while not RENDER.advance(job): steps += 1
			check(job.error.is_empty(), "attachment completes: %s" % job.error)
			var trees := {"body": 0, "cards": 0, "far": 0}
			var wide := 0
			var value := QUALITY.profile(level)
			for node: Node in parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false):
				var lod: String = node.get_meta("mapkit_vegetation_lod")
				var kind: String = node.get_meta("mapkit_vegetation_kind")
				var near := float(value.vegetation_near) * (0.6 if kind == "shrub" else 1.0) + QUALITY.GROUP_REACH
				if kind == "tree":
					trees[lod] += node.multimesh.instance_count
					if lod == "body" and str(node.multimesh.mesh.resource_name).begins_with("mk_park_tree_"): wide += node.multimesh.instance_count
				if RenderingServer.get_current_rendering_method() != "forward_plus" and lod in ["body","cards","far"]:
					check((node.visibility_range_begin_margin if lod=="far" else node.visibility_range_end_margin)==0,"no fresh-cell hysteresis gap between canopy LODs")
				check(node.get_meta("mapkit_decoration", false) and node.multimesh.use_custom_data, "vegetation nodes carry decoration ranges and instance data")
				if lod in ["body", "cards"]:
					check(node.visible == (lod == "body" or level > 0) and is_equal_approx(node.visibility_range_end, near), "near range per tier covers the whole group")
				if lod == "far":
					check(is_equal_approx(node.visibility_range_begin, near) and node.visibility_range_end == (float(value.decoration_distance) if kind == "shrub" else 0.0), "far begins where near ends; far trees keep their crowns")
				var shadows: bool = kind == "tree" and level >= (2 if lod == "cards" else 1)
				check(node.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if shadows else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), "%s %s shadows on tier %d" % [kind, lod, level])
			check(trees.body == 12 and trees.cards == 12 and trees.far == 12, "every tree has body, cards and far instances: %s" % trees)
			check(wide == 6, "zone trees with a 200 cm footprint draw wide park crowns: %d" % wide)
			check(job.root.get_meta("mapkit_render_root", false) and steps > 20, "vegetation work spans admitted steps: %d" % steps)
			# Live tier changes reach existing vegetation nodes.
			QUALITY.apply_tree(job.root, QUALITY.profile(0))
			for node: Node in parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false):
				var lod: String = node.get_meta("mapkit_vegetation_lod")
				if lod == "cards": check(not node.visible, "low hides cards")
				elif lod == "body": check(node.visible and node.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "low keeps near bodies without shadows")
			RENDER.cancel(job)
			parent.queue_free()
			for _frame in 4: await process_frame
			check(lease.retired(), "the cell lease retires with its vegetation nodes")
	cache.shutdown()
	for _frame in 4: await process_frame
	check(cache.bytes() == 0, "shared vegetation retires after its last cell")
	# A refused shared reservation fails a cell with trees; a cell of grass
	# ground alone never claims it.
	var denied := CACHE.new(func(bytes): return null if bytes == CACHE.VEGETATION_BYTES else Lease.new(bytes))
	QUALITY.current = QUALITY.profile(2)
	var parent := Node3D.new()
	root.add_child(parent)
	var chunk := fixture()
	var job := RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	while not RENDER.advance(job): pass
	check(job.get("error", {}).get("code", "") == "E_MEMORY_BUDGET", "denied vegetation fails a cell with trees")
	chunk.objects.clear()
	chunk.render_batches = RENDER.PLAN.prepare_batches(chunk)
	job = RENDER.begin(chunk, parent, func(bytes): return Lease.new(bytes), RENDER.PLAN.estimate(chunk, 0), denied)
	while not RENDER.advance(job): pass
	check(job.error.is_empty() and parent.find_children("Vegetation_*", "MultiMeshInstance3D", true, false).is_empty(), "denied vegetation leaves a cell without plants untouched: %s" % job.error)
	RENDER.cancel(job)
	parent.queue_free()
	denied.shutdown()
	await process_frame

## Exercise GPU attributes, including the Compatibility instance/mesh layout.
func rendered_palette() -> void:
	if DisplayServer.get_name() == "headless": return
	var viewport := SubViewport.new();viewport.size=Vector2i(128,128);viewport.own_world_3d=true
	viewport.render_target_update_mode=SubViewport.UPDATE_ALWAYS;root.add_child(viewport)
	var world := Node3D.new();viewport.add_child(world)
	var environment := WorldEnvironment.new();environment.environment=Environment.new()
	environment.environment.background_mode=Environment.BG_COLOR;environment.environment.background_color=Color.BLACK
	environment.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR;environment.environment.ambient_light_color=Color.WHITE;environment.environment.ambient_light_energy=1.0
	world.add_child(environment)
	var camera := Camera3D.new();world.add_child(camera);camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=5
	camera.position=Vector3(0,2,8);camera.look_at(Vector3(0,2,0));camera.current=true
	var multi := MultiMesh.new();multi.transform_format=MultiMesh.TRANSFORM_3D;multi.use_custom_data=true
	multi.mesh=VEGETATION.shared().tree_far[0];multi.instance_count=1;multi.set_instance_transform(0,Transform3D.IDENTITY)
	multi.set_instance_custom_data(0,Color(0.08,0.4,0.03,1.0))
	var node := MultiMeshInstance3D.new();node.multimesh=multi;world.add_child(node)
	for frame in 3: await process_frame
	await RenderingServer.frame_post_draw
	var rendered := viewport.get_texture().get_image()
	var green := 0;var red := 0
	for y in 80:
		for x in 128:
			var pixel := rendered.get_pixel(x,y)
			if pixel.g>0.05 and pixel.g>pixel.r*1.5 and pixel.g>pixel.b*1.5: green+=1
			if pixel.r>0.05 and pixel.r>pixel.g*1.5: red+=1
	check(green>100 and red<green/10,"rendered foliage retains its green instance tint: green=%d red=%d" % [green,red])
	viewport.queue_free()
	await process_frame
