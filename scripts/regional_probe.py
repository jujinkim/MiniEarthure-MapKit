"""Native partial source lifecycle, using the same packed generation bridge."""
PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
var failures: Array[String] = []
func check(ok: bool, description: String) -> void:
    if not ok:
        failures.append(description)
        push_error(description)
func _initialize() -> void:
    call_deferred("run")
func run() -> void:
    check(ClassDB.class_exists("MapKitRegionReader"), "indexed native class registered")
    if not failures.is_empty():
        quit(1)
        return
    var reader: RefCounted = ClassDB.instantiate("MapKitRegionReader")
    var opened: Dictionary = JSON.parse_string(reader.open_index(ProjectSettings.globalize_path("res://fixture.mkregions"), 536870912, ""))
    check(opened.ok and opened.data.verification == "index-only" and opened.data.region_count == 4, "bounded index-only open")
    var original: RefCounted = ClassDB.instantiate("MapKitBridge")
    check(JSON.parse_string(original.open_package(ProjectSettings.globalize_path("res://fixture.memap"))).ok, "legacy comparison source")
    var live: RefCounted
    var first_hash: String
    for y in 2:
        for x in 2:
            var located: Dictionary = JSON.parse_string(reader.region_for_cell(x, y))
            check(located.ok, "world cell to storage region")
            var generation: int = reader.begin_request()
            var rejected: Dictionary = reader.load_region(located.data.region, int(located.data.cost.validation_peak_bytes) - 1, generation)
            check(not rejected.ok and rejected.error.code == "E_MEMORY_BUDGET", "pre-read memory rejection")
            var loaded: Dictionary = reader.load_region(located.data.region, int(located.data.cost.validation_peak_bytes), generation)
            check(loaded.ok and reader.request_is_current(generation), "exact allowance source admission")
            var bridge: RefCounted = loaded.bridge
            var expected: Dictionary = original.generate_chunk_occupied_packed(x, y, 200000)
            var actual: Dictionary = bridge.generate_chunk_occupied_packed(x, y, 200000)
            check(actual.ok and actual.data.generated_sha256 == expected.data.generated_sha256, "global source hash parity")
            check(actual.data.occupancy.view() == expected.data.occupancy.view(), "full collision volumes agree")
            check(not bridge.generate_chunk_packed(1 - x, y).ok, "neighbor requires its own prepared source")
            var info: Dictionary = JSON.parse_string(bridge.chunk_archive_info(x, y))
            var saved: Dictionary = bridge.generate_chunk_archived(x, y, info.data.max_bytes)
            check(saved.ok and bridge.restore_chunk_archive(x, y, saved.data.archive).ok, "region-keyed generated archive roundtrip")
            if x == 0 and y == 0:
                live = bridge
                first_hash = actual.data.generated_sha256
    var world := Node3D.new()
    root.add_child(world)
    var body := StaticBody3D.new()
    var shape := ConcavePolygonShape3D.new()
    var collision := CollisionShape3D.new()
    var geometry: Dictionary = DATA.view(live.generate_chunk_packed(0, 0).data.chunk)
    var faces := PackedVector3Array()
    for triangle in DATA.count(geometry):
        for vertex in [0, 2, 1]:
            faces.append(DATA.scene_vertex(geometry, triangle, vertex))
    shape.set_faces(faces)
    shape.backface_collision = true
    collision.shape = shape
    body.add_child(collision)
    world.add_child(body)
    await physics_frame
    await physics_frame
    var query := PhysicsRayQueryParameters3D.create(Vector3(256, 8, -256), Vector3(256, 6, -256))
    var hit: Dictionary = world.get_world_3d().direct_space_state.intersect_ray(query)
    check(not hit.is_empty() and absf(hit.position.y - 7.0) < 0.001, "regional bridge supports actual physics ray at exact authored height")
    var generation: int = reader.begin_request()
    var worker := Thread.new()
    check(worker.start(func(): return reader.load_region(1, 536870912, generation)) == OK, "native worker started")
    reader.cancel_request()
    var late: Dictionary = worker.wait_to_finish()
    check(not reader.request_is_current(generation), "cancelled or already finished candidate cannot commit")
    check(late.ok or late.error.code == "E_CANCELLED", "worker cancellation has bounded typed result")
    check(live.generate_chunk_packed(0, 0).data.generated_sha256 == first_hash, "current source survives cancellation and join")
    hit = world.get_world_3d().direct_space_state.intersect_ray(query)
    check(not hit.is_empty() and hit.collider == body and world.get_child_count() == 1, "rejected source leaves current collider installed")
    var stale: Dictionary = reader.load_region(0, 536870912, generation)
    check(not stale.ok and stale.error.code == "E_CANCELLED", "late/duplicate read rejected")
    var fresh: int = reader.begin_request()
    check(worker.start(func(): return reader.load_region(0, 536870912, fresh)) == OK, "fresh worker starts")
    var completed: Dictionary = worker.wait_to_finish()
    check(completed.ok and reader.request_is_current(fresh), "fresh worker constructs transferable native bridge")
    check(completed.bridge.generate_chunk_packed(0, 0).data.generated_sha256 == first_hash, "worker bridge usable after join")
    reader = null
    check(live.generate_chunk_packed(0, 0).data.generated_sha256 == first_hash, "borrowed snapshot owns source after reader retirement")
    live = null
    original = null
    world.queue_free()
    await process_frame
    print("MapKit regional source probe: ", "PASS" if failures.is_empty() else str(failures))
    quit(0 if failures.is_empty() else 1)
'''
