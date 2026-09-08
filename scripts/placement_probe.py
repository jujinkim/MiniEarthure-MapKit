"""Independent recipe-3 native generation, shared rendering and solid collision."""
PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
var failures: Array[String] = []
var world: Node3D
func check(ok: bool, message: String) -> void:
    if not ok:
        failures.append(message)
        push_error(message)
func pos(x: float, h: float, y: float) -> Vector3: return Vector3(x, h, -y) * 0.00125
func occupied(point: Vector3) -> bool:
    var query := PhysicsShapeQueryParameters3D.new()
    var sphere := SphereShape3D.new()
    sphere.radius = 0.005
    query.shape = sphere
    query.transform = Transform3D(Basis.IDENTITY, point)
    return not world.get_world_3d().direct_space_state.intersect_shape(query).is_empty()
func _initialize() -> void: run.call_deferred()
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    var opened: Dictionary = JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://placement.memap")))
    check(opened.ok, "third-party recipe-3 load")
    if not opened.ok:
        quit(1)
        return
    check(JSON.parse_string(bridge.cell_window(0,0)).data.recipe_version == 3, "opened recipe metadata")
    world = Node3D.new()
    root.add_child(world)
    var identities := {}
    var styles := {}
    for y in 2:
        for x in 2:
            var packed: Dictionary = bridge.generate_chunk_occupied_packed(x,y,20000)
            var raw: Dictionary = JSON.parse_string(bridge.generate_chunk(x,y))
            check(packed.ok and raw.ok, "recipe-3 generation")
            if not packed.ok or not raw.ok:
                quit(1)
                return
            check(packed.data.generated_sha256 == raw.data.generated_sha256,"JSON/packed exact hash")
            var c: Dictionary = DATA.view(packed.data.chunk)
            var faces := PackedVector3Array()
            for t in DATA.count(c):
                var key: String = DATA.material_key(c,t)
                check(key == DATA.material_key(raw.data.chunk,t),"shared packed/JSON material and usage")
                styles[key] = true
                for v in [0,2,1]: faces.append(DATA.scene_vertex(c,t,v))
            check(DATA.prism_count(c) == DATA.prism_count(raw.data.chunk),"exact solid count")
            for i in DATA.prism_count(c):
                check(DATA.prism_points(c,i,0.125) == DATA.prism_points(raw.data.chunk,i,0.125),"exact six-vertex solid transfer")
                var shape := ConvexPolygonShape3D.new()
                shape.points = DATA.prism_points(c,i,0.125)
                var collider := CollisionShape3D.new()
                collider.shape = shape
                var body := StaticBody3D.new()
                body.add_child(collider)
                world.add_child(body)
            var render := RENDERER.attach(packed.data.chunk,world)
            var rendered := PackedVector3Array()
            for mesh: MeshInstance3D in render.get_children():
                if not mesh.mesh is ArrayMesh: continue # center-owned tree canopy
                var arrays := mesh.mesh.surface_get_arrays(0)
                var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
                if arrays[Mesh.ARRAY_INDEX] == null: rendered.append_array(vertices)
                else:
                    for index in arrays[Mesh.ARRAY_INDEX]: rendered.append(vertices[index])
            check(rendered == faces,"all generated building/prop/sidewalk render faces equal canonical collision faces")
            for object: Dictionary in c.objects:
                check(not identities.has(object.id),"unique tree/repetition/manual object ownership")
                identities[object.id] = true
            var cost: Dictionary = JSON.parse_string(bridge.estimate_chunk(x,y)).data
            check(cost.building_prisms >= DATA.prism_count(c),"prism pre-allocation bound")
            var info: Dictionary = JSON.parse_string(bridge.chunk_archive_info(x,y)).data
            var saved: Dictionary = bridge.generate_chunk_archived(x,y,info.max_bytes)
            check(saved.ok and saved.data.has("archive"),"new primitive archive within allowance")
            if saved.ok and saved.data.has("archive"):
                var restored: Dictionary = bridge.restore_chunk_archive(x,y,saved.data.archive)
                check(restored.ok and restored.data.generated_sha256 == packed.data.generated_sha256,"new primitive cache roundtrip")
                check(DATA.view(restored.data.chunk).building_prism_vertices_cm == c.building_prism_vertices_cm,"cache preserves exact convex interiors")
            if DATA.prism_count(c)>0:
                var changed: PackedInt64Array = c.building_prism_vertices_cm
                var before: int = changed[0]
                changed[0] += 999
                check(DATA.view(packed.data.chunk).building_prism_vertices_cm[0] == before,"convex view mutation isolation")
                var solid: Dictionary = packed.data.occupancy.view()
                check(solid.shape_kinds.has(2) and solid.slope_tops_cm.size()>0,"exact sloped occupied sidecar")
    await physics_frame
    await physics_frame
    check(occupied(pos(2500,400,2000)),"building interior physically solid")
    check(not occupied(pos(5000,400,4000)),"concave notch physically empty")
    check(occupied(pos(12900,1200,10000)),"space below ridge physically solid")
    check(not occupied(pos(12050,1200,10000)),"space above roof slope remains empty")
    check(occupied(pos(12795,400,10000)) and occupied(pos(12805,400,10000)),"solid seam has no gap")
    check(styles.has("wood:public") and styles.has("brick:residential") and styles.has("concrete:commercial"),"authored usage/material reaches common renderer")
    var cancelled := RENDERER.begin(bridge.generate_chunk_packed(0,0).data.chunk,world)
    RENDERER.advance(cancelled)
    RENDERER.cancel(cancelled)
    check(RENDERER.advance(cancelled) and cancelled.chunk.is_empty(),"cancel releases geometry ownership")
    world.queue_free()
    await process_frame
    await process_frame
    print("mapkit_placement: " + ("PASS (recipe 3, material/roof, solid interiors, concavity/seams, props, ownership, archive)" if failures.is_empty() else str(failures)))
    quit(0 if failures.is_empty() else 1)
'''
