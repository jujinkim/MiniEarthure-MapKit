"""Native recipe-2 render and PhysicsServer tests of original synthetic roads."""
PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
var failures: Array[String] = []
var world: Node3D
func check(ok: bool, message: String) -> void:
    if not ok:
        failures.append(message)
        push_error(message)
func ray(a: Vector3, b: Vector3) -> Dictionary:
    return world.get_world_3d().direct_space_state.intersect_ray(PhysicsRayQueryParameters3D.create(world.to_global(a), world.to_global(b)))
func pos(x: float, h: float, y: float) -> Vector3: return Vector3(x, h, -y) * 0.00125
func _initialize() -> void: run.call_deferred()
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    var opened: Dictionary = JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://roads.memap")))
    check(opened.ok, "independent recipe-2 package load")
    if not opened.ok:
        quit(1)
        return
    check(JSON.parse_string(bridge.cell_window(0, 0)).data.recipe_version == 2, "window reports the explicit recipe-2 source")
    world = Node3D.new()
    root.add_child(world)
    world.position = Vector3(11, 3, -7)
    for y in 2:
        for x in 2:
            var packed: Dictionary = bridge.generate_chunk_packed(x, y)
            var raw: Dictionary = JSON.parse_string(bridge.generate_chunk(x, y))
            check(packed.ok and raw.ok, "roads generated")
            if not packed.ok or not raw.ok:
                quit(1)
                return
            check(packed.data.generated_sha256 == raw.data.generated_sha256, "roads packed/JSON hash")
            var c: Dictionary = DATA.view(packed.data.chunk)
            var faces := PackedVector3Array()
            for t in DATA.count(c):
                for v in [0, 2, 1]:
                    check(DATA.scene_vertex(c,t,v) == DATA.scene_vertex(raw.data.chunk,t,v), "roads exact packed/JSON cm")
                    faces.append(DATA.scene_vertex(c,t,v))
            var body := StaticBody3D.new()
            var collider := CollisionShape3D.new()
            var shape := ConcavePolygonShape3D.new()
            shape.backface_collision = true
            shape.set_faces(faces)
            collider.shape = shape
            body.add_child(collider)
            world.add_child(body)
            var render := RENDERER.attach(packed.data.chunk, world)
            var rendered := PackedVector3Array()
            for mesh: MeshInstance3D in render.get_children():
                var arrays := mesh.mesh.surface_get_arrays(0)
                var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
                if arrays[Mesh.ARRAY_INDEX] == null: rendered.append_array(vertices)
                else:
                    for index in arrays[Mesh.ARRAY_INDEX]: rendered.append(vertices[index])
            check(rendered == faces, "all road/terrain/structure render faces equal collision")
    await physics_frame
    await physics_frame
    for id in ["underpass", "tunnel"]:
        var y := 5000 if id == "underpass" else 8000
        # Rays at center and both wheel tracks across every grade/apron/cell seam.
        for x in range(101, 9900, 100):
            for offset in [-100, 0, 100]:
                var sample: Dictionary = JSON.parse_string(bridge.surface_probe(x,y+offset,id))
                check(sample.ok, "continuous native floor query %s %d" % [id,x])
                if not sample.ok: continue
                var target := pos(x,float(sample.data.position_cm[1]),y+offset)
                var hit := ray(target+Vector3.UP*0.1,target-Vector3.UP*0.1)
                check(not hit.is_empty() and hit.position.distance_to(world.to_global(target)) < 0.0013, "actual floor contact %s %d" % [id,x])
        var h := -500 if id == "underpass" else -600
        var wall := ray(pos(6000,h+100,y),pos(6000,h+100,y+500))
        check(not wall.is_empty() and absf(world.to_local(wall.position).z-pos(6000,0,y+300).z)<0.0001, "real side wall %s" % id)
        var above := ray(pos(6000,h+100,y),pos(6000,1000,y))
        if id == "tunnel": check(not above.is_empty() and absf(world.to_local(above.position).y-pos(0,h+300,0).y)<0.0001, "tunnel ceiling contact")
        else: check(above.is_empty(), "underpass has no ceiling or uncut terrain")
    for pair in [["ground-west",2500,1000,0],["bridge",2500,1000,600],["elevated",7500,1400,700]]:
        var target := pos(pair[1],pair[3],pair[2])
        var hit := ray(target+Vector3.UP*0.05,target-Vector3.UP*0.05)
        check(not hit.is_empty() and hit.position.distance_to(world.to_global(target))<0.0001,"separate physical layer " + pair[0])
    world.queue_free()
    await process_frame
    await process_frame
    print("mapkit_roads: " + ("PASS (recipe 2, JSON/packed/render/collision, portals, grades, seams, walls, ceiling)" if failures.is_empty() else str(failures)))
    quit(0 if failures.is_empty() else 1)
'''
