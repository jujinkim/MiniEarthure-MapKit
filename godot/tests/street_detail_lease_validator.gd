extends SceneTree
## Generated street details (single-node shrubs, city trees, street trees and
## flower beds) share process-lifetime meshes and materials. A cell's display
## lease must retire with its nodes.
const CHUNKS := preload("../chunk_renderer.gd")
const QUALITY := preload("../display_quality.gd")
class Lease extends RefCounted:
	var refs: Array[WeakRef] = []
	var sealed := false
	func track(value: Object) -> void: refs.append(weakref(value))
	func seal() -> void: sealed=true
	func retired() -> bool: return sealed and refs.all(func(ref): return ref.get_ref()==null)
var failed := false
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var leases: Array = []
	var shared := {}
	for cell in 2:
		var lease := Lease.new()
		leases.append(lease)
		var job := {"lease": lease}
		var cell_root := Node3D.new()
		world.add_child(cell_root)
		lease.track(cell_root)
		for id: String in CHUNKS.STREET_DETAILS:
			var detail := CHUNKS._street_detail(id, {"id": "%s-%d" % [id, cell], "position": [100 * cell, 0, 0], "quarter_turns": cell})
			CHUNKS.track_resources(job, detail)
			cell_root.add_child(detail)
		lease.seal()
		for mesh: MeshInstance3D in cell_root.find_children("*", "MeshInstance3D", true, false):
			shared[mesh.mesh] = true
			shared[mesh.material_override] = true
	var vegetation: Dictionary = CHUNKS.VEGETATION.shared()
	var shared_vegetation := {}
	for plant in CHUNKS.VEGETATION.PLANTS:
		for part in CHUNKS.VEGETATION.KINDS:
			for mesh: Mesh in vegetation[plant + "_" + part]: shared_vegetation[mesh] = true
	var street: Dictionary = CHUNKS.STREET.shared()
	for name: String in street:
		if name != "materials": shared_vegetation[street[name]] = true
	# Vegetation nodes carry their materials on the shared mesh surfaces.
	check(not shared.is_empty() and shared.keys().all(func(value): return value == null or shared_vegetation.has(value)), "street details borrow only shared vegetation resources")
	# City trees: a near body (trunk, branches, crown hull) and its leaf cards
	# rooted at the object, 0.85–1.15×, shadows by tier; shrubs and trees vary
	# by identity among three shared variants.
	var canopies := {}
	var shrubs := {}
	QUALITY.current = QUALITY.profile(2)
	for index in 24:
		var tree := CHUNKS._street_detail("builtin:city_tree", {"id": "tree-%d" % index, "position": [0, 0, 0], "quarter_turns": index % 4})
		var crown: MeshInstance3D = tree.get_child(0)
		var cards: MeshInstance3D = tree.get_child(1)
		var placed := tree.transform * crown.transform
		var scale := placed.basis.get_scale().x
		check(tree.get_child_count() == 2 and crown.get_meta("mapkit_vegetation_lod") == "body" and cards.get_meta("mapkit_vegetation_lod") == "cards" and crown.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON and scale >= 0.849 and scale <= 1.151 and placed.origin.is_zero_approx(), "a city tree draws a scaled shadow-casting body and cards at its trunk foot")
		var bounds: AABB = crown.mesh.get_aabb().merge(cards.mesh.get_aabb())
		check(bounds.position.y <= 0.0 and bounds.end.y >= 3.4 and crown.mesh.get_surface_count() == 1 and cards.mesh.get_surface_count() == 1, "trunk from the ground into a crown over 3.4 m with a leaf-card mesh")
		canopies[crown.mesh] = true
		var shrub := CHUNKS._street_detail("builtin:shrub", {"id": "shrub-%d" % index, "position": [0, 0, 0], "quarter_turns": 0})
		var bush: MeshInstance3D = shrub.get_child(0)
		check(bush.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF and bush.mesh.get_aabb().end.y < 1.0, "a shrub is a low bush without shadows")
		shrubs[bush.mesh] = true
		tree.free(); shrub.free()
	QUALITY.current = {}
	check(canopies.size() == 3 and shrubs.size() == 3, "three tree and three shrub variants")
	# The renderer instances a cell's trees and shrubs: per kind, variant and
	# 32 m group a body, cards and far MultiMesh with per-instance data.
	var cell_lease := Lease.new()
	var cell := Node3D.new()
	root.add_child(cell)
	var objects: Array = []
	for index in 40: objects.append({"id": "tree-%d" % index, "asset_id": "builtin:city_tree", "position": [index * 300, 0, 0], "quarter_turns": 0})
	for index in 10: objects.append({"id": "shrub-%d" % index, "asset_id": "builtin:shrub", "position": [index * 300, 0, 400], "quarter_turns": 0})
	objects.append({"id": "bench", "asset_id": "builtin:bench", "position": [0, 0, 900], "quarter_turns": 0})
	# Park-zone trees (every fourth) have the 200 cm footprint the native
	# presentation lists; the others keep 120 cm street crowns.
	var radii := {}
	for index in range(0, 40, 4): radii["tree-%d" % index] = 200
	var job := {"lease": cell_lease, "root": cell, "chunk": {"objects": objects, "presentation": {"city_tree_radii": radii}}, "vegetation": vegetation}
	for object: Dictionary in objects:
		if CHUNKS.VEGETATION_IDS.has(object.asset_id): CHUNKS._plan_vegetation(job, object)
	var expected_groups: int = job.vegetation_plan.size()
	while not job.vegetation_plan.is_empty(): CHUNKS._attach_vegetation_group(job)
	var groups: Array = cell.find_children("*", "MultiMeshInstance3D", false, false)
	var drawn := {"tree": 0, "shrub": 0}
	var wide := 0
	for group: MultiMeshInstance3D in groups:
		if group.get_meta("mapkit_vegetation_lod") == "body":
			drawn[group.get_meta("mapkit_vegetation_kind")] += group.multimesh.instance_count
			if vegetation.wide_body.has(group.multimesh.mesh): wide += group.multimesh.instance_count
		check(group.multimesh.use_custom_data and group.get_meta("mapkit_decoration", false) and group.get_meta("mapkit_static_detail", false), "instances carry tint/phase and decoration ranges")
		# Medium (the default profile): opaque tree crowns cast, cards and shrubs do not.
		var casts: bool = group.get_meta("mapkit_vegetation_kind") == "tree" and group.get_meta("mapkit_vegetation_lod") != "cards"
		check(group.cast_shadow == (GeometryInstance3D.SHADOW_CASTING_SETTING_ON if casts else GeometryInstance3D.SHADOW_CASTING_SETTING_OFF), "medium: tree bodies and far crowns cast shadows, cards and shrubs do not")
		var aabb := group.get_aabb()
		check(aabb.size.x <= CHUNKS.VEGETATION_GROUP_METRES + 6.0 and aabb.size.z <= CHUNKS.VEGETATION_GROUP_METRES + 6.0, "a group spans one 32 m square plus its crowns")
	check(groups.size() == expected_groups * 3 and drawn.tree == 40 and drawn.shrub == 10, "40 trees and 10 shrubs in body/cards/far groups: %d groups, %s" % [groups.size(), drawn])
	check(wide == 10, "the ten 200 cm zone trees draw wide park crowns: %d" % wide)
	# Headless rendering keeps no instance buffers: compare the shared placement
	# with the node path's tree instead.
	for radius in [0, 200]:
		var first: Dictionary = CHUNKS.city_canopy_transform(objects[1], radius)
		var single := CHUNKS._street_detail("builtin:city_tree", objects[1], radius)
		var crown: MeshInstance3D = single.get_child(0)
		check(vegetation[first.mesh + "_body"][first.variant] == crown.mesh and first.mesh == ("wide" if radius == 200 else "tree") and (single.transform * crown.transform).is_equal_approx(first.transform), "instances and nodes place the same %d cm tree" % radius)
		single.free()
	cell_lease.seal()
	cell.free()
	for frame in 2: await process_frame
	check(cell_lease.retired(), "freeing the cell retires its canopy lease")
	check(leases.all(func(lease): return not lease.retired()), "a live cell keeps its lease")
	world.get_child(0).free()
	for frame in 2: await process_frame
	check(leases[0].retired() and not leases[1].retired(), "a freed cell retires its lease while cached details stay alive")
	world.free()
	for frame in 2: await process_frame
	check(leases[1].retired(), "the last cell retires too")
	print("street_detail_lease_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)
