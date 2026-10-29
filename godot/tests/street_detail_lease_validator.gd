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
	check(leases.all(func(lease): return not lease.retired()), "a live cell keeps its lease")
	world.get_child(0).free()
	for frame in 2: await process_frame
	check(leases[0].retired() and not leases[1].retired(), "a freed cell retires its lease while cached details stay alive")
	world.free()
	for frame in 2: await process_frame
	check(leases[1].retired(), "the last cell retires too")
	print("street_detail_lease_validator: ", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)
