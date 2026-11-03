extends SceneTree
## Generated street details (signals, benches, shrubs) share process-lifetime
## meshes and materials. A cell's display lease must retire with its nodes.
const CHUNKS := preload("../chunk_renderer.gd")
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
	check(not shared.is_empty() and CHUNKS._details.size() == shared.size(), "both cells reuse one cached mesh and material per detail key")
	# City trees: one shadow-casting canopy above the 140 cm trunk, 0.85–1.15×;
	# shrubs and canopies vary by identity among three shared variants.
	var canopies := {}
	var shrubs := {}
	for index in 24:
		var tree := CHUNKS._street_detail("builtin:city_tree", {"id": "tree-%d" % index, "position": [0, 0, 0], "quarter_turns": 0})
		var crown: MeshInstance3D = tree.get_child(0)
		var scale := crown.basis.get_scale().x
		check(tree.get_child_count() == 1 and crown.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON and scale >= 0.849 and scale <= 1.151, "a city tree draws one scaled shadow-casting canopy")
		check(crown.position.y - crown.mesh.height * scale * 0.5 <= 1.40, "the canopy covers the trunk top")
		canopies[crown.mesh] = true
		var shrub := CHUNKS._street_detail("builtin:shrub", {"id": "shrub-%d" % index, "position": [0, 0, 0], "quarter_turns": 0})
		shrubs[shrub.get_child(0).mesh] = true
		tree.free(); shrub.free()
	check(canopies.size() == 3 and shrubs.size() == 3, "three canopy and three shrub variants")
	# The renderer instances a cell's city tree canopies: one MultiMesh per variant.
	var cell_lease := Lease.new()
	var cell := Node3D.new()
	root.add_child(cell)
	var objects: Array = []
	for index in 40: objects.append({"id": "tree-%d" % index, "asset_id": "builtin:city_tree", "position": [index * 300, 0, 0], "quarter_turns": 0})
	objects.append({"id": "bench", "asset_id": "builtin:bench", "position": [0, 0, 900], "quarter_turns": 0})
	var job := {"lease": cell_lease, "root": cell, "chunk": {"objects": objects}}
	for object: Dictionary in objects:
		if object.asset_id == "builtin:city_tree": CHUNKS._city_canopy_instance(job, object)
	var groups: Array = cell.find_children("*", "MultiMeshInstance3D", false, false)
	var drawn := 0
	for group: MultiMeshInstance3D in groups: drawn += group.multimesh.visible_instance_count
	check(groups.size() == 3 and drawn == 40 and groups.all(func(g): return g.multimesh.instance_count == 40 and g.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_ON), "40 canopies in three shared-mesh MultiMeshes")
	# Headless rendering keeps no instance buffers: compare the shared placement
	# with the node path's canopy instead.
	var first: Dictionary = CHUNKS.city_canopy_transform(objects[1])
	var single := CHUNKS._street_detail("builtin:city_tree", objects[1])
	var crown: MeshInstance3D = single.get_child(0)
	check(groups[first.variant].multimesh.mesh == crown.mesh and (single.transform * crown.transform).is_equal_approx(first.transform), "instances and nodes place the same canopy")
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
