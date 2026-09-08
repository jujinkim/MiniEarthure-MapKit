"""Native static assets, shared rendering, source ownership and solid geometry."""
PROBE = '''extends SceneTree
const DATA = preload("res://addons/outer_runtime/mapkit/chunk_data.gd")
const RENDERER = preload("res://addons/outer_runtime/mapkit/chunk_renderer.gd")
var failures: Array[String] = []
func check(ok: bool, message: String) -> void:
    if not ok:
        failures.append(message)
        push_error(message)
func _initialize() -> void: run.call_deferred()
func meshes(node: Node) -> Array[MeshInstance3D]:
    var out: Array[MeshInstance3D] = []
    if node is MeshInstance3D: out.append(node)
    for child: Node in node.get_children(): out.append_array(meshes(child))
    return out
func run() -> void:
    var bridge: RefCounted = ClassDB.instantiate("MapKitBridge")
    var opened: Dictionary = JSON.parse_string(bridge.open_package(ProjectSettings.globalize_path("res://assets.memap")))
    check(opened.ok, "independent recipe-4 package")
    if not opened.ok:
        quit(1)
        return
    var raw: Dictionary = JSON.parse_string(bridge.generate_chunk(0,0))
    var packed: Dictionary = bridge.generate_chunk_occupied_packed(0,0,20000)
    check(packed.ok and raw.ok, "exact occupied generation")
    check(not packed.data.chunk.has("presentation"), "Host generation has no display data")
    check(packed.data.generated_sha256 == raw.data.generated_sha256, "JSON and packed hash")
    var original: String = raw.data.generated_sha256
    var geometry: Dictionary = DATA.view(packed.data.chunk)
    var independent: Dictionary = DATA.view(packed.data.chunk)
    geometry.asset_convex_vertices_cm[0] += 100
    check(geometry.asset_convex_vertices_cm[0] != independent.asset_convex_vertices_cm[0], "convex COW isolation")
    var decorated: Dictionary = bridge.with_presentation(packed.data)
    check(decorated.ok and decorated.data.generated_sha256 == original, "display decoration preserves generated hash")
    var display_collision: Dictionary = decorated.data.chunk.duplicate(true)
    display_collision.presentation.assets.grass = display_collision.presentation.assets.checker
    display_collision.presentation.proxy_materials["image-box"] = "grass"
    for t in DATA.count(DATA.view(display_collision)):
        var view: Dictionary = DATA.view(display_collision)
        if DATA.object_id(view,t) == "terrain": check(RENDERER.display_material_key(view,t) == "grass", "asset ID cannot shadow terrain material")
        elif DATA.object_id(view,t) == "image-box": check(RENDERER.display_material_key(view,t) == "asset:grass", "custom material has separate namespace")
    var first: Dictionary = decorated.data.chunk.presentation.assets
    first.tetra.bytes[0] = 0
    var second: Dictionary = bridge.with_presentation(bridge.generate_chunk_packed(0,0).data)
    check(second.data.chunk.presentation.assets.tetra.bytes[0] == 103, "display mutation cannot alter package snapshot")
    decorated = second
    var world := Node3D.new()
    root.add_child(world)
    var job := RENDERER.begin(decorated.data.chunk, world)
    var steps := 0
    while not RENDERER.advance(job) and steps < 1000: steps += 1
    check(job.done and job.error.is_empty(), "GLB PNG and builtins rendered: " + str(job.error))
    if not job.error.is_empty():
        world.queue_free()
        await process_frame
        quit(1)
        return
    check(job.templates.is_empty() and job.asset_materials.is_empty() and job.chunk.is_empty(), "temporary resources released at completion")
    var anchors := {}
    for node: Node in job.root.get_children():
        if node.has_meta("mapkit_object_id"): anchors[node.get_meta("mapkit_object_id")] = node
    check(anchors.size() == 2, "one owner instance per model")
    var points := DATA.prism_points(independent,0,0.125)
    var model_points := PackedVector3Array()
    for model: MeshInstance3D in meshes(anchors["tetra-a"]):
        for v: Vector3 in model.mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]:
            model_points.append(model.global_transform * v)
        check(model.mesh.surface_get_material(0).albedo_texture != null, "embedded PNG material")
    for p: Vector3 in points: check(Array(model_points).any(func(v: Vector3): return v.distance_to(p) < 0.00001), "GLB metre-axis transform equals cm collision vertex")
    for model: MeshInstance3D in meshes(anchors["tetra-b"]):
        check(model.material_override != null and model.material_override.albedo_texture != null, "declarative override texture")
        check(is_equal_approx(model.material_override.metallic,0.1), "declarative metallic")
    var images := 0
    for model: MeshInstance3D in meshes(job.root):
        if model.material_override != null and model.material_override.albedo_texture != null: images += 1
    check(images >= 2, "standalone image proxy and model override")
    # Both cells retain full convex collision at the seam. Empty sloping AABB corner stays empty.
    for x in 2:
        var c: Dictionary = DATA.view(bridge.generate_chunk_packed(x,0).data.chunk)
        for i in DATA.prism_count(c):
            if DATA.prism_id(c,i) != "tetra-a": continue
            var body := StaticBody3D.new()
            var collider := CollisionShape3D.new()
            var shape := ConvexPolygonShape3D.new()
            shape.points = DATA.prism_points(c,i,0.125)
            collider.shape = shape
            body.add_child(collider)
            world.add_child(body)
    await physics_frame
    await physics_frame
    var sphere := SphereShape3D.new()
    sphere.radius = 0.001
    var query := PhysicsShapeQueryParameters3D.new()
    query.shape = sphere
    for example: Array in [[[12020,30,6730],true],[[12810,20,6730],true],[[12800,600,7250],false]]:
        query.transform = Transform3D(Basis.IDENTITY,RENDERER.scene_position(example[0]))
        var found: bool = not world.get_world_3d().direct_space_state.intersect_shape(query).is_empty()
        check(found == example[1], "solid interior/seam and empty slope corner " + str(example))
    var weak: WeakRef = weakref(anchors["tetra-a"])
    RENDERER.cancel(job)
    RENDERER.cancel(job)
    anchors.clear()
    await process_frame
    check(weak.get_ref() == null, "render nodes returned exactly once")
    var corrupt: Dictionary = decorated.data.chunk.duplicate(true)
    corrupt.presentation.assets.tetra.bytes = PackedByteArray()
    var failed := RENDERER.begin(corrupt,world)
    while not RENDERER.advance(failed): pass
    check(not failed.error.is_empty() and not failed.done and failed.cancelled, "backend failure is never readiness")
    var cancelled := RENDERER.begin(decorated.data.chunk,world)
    RENDERER.advance(cancelled)
    RENDERER.cancel(cancelled)
    check(RENDERER.advance(cancelled) and cancelled.chunk.is_empty() and cancelled.templates.is_empty(), "partial cancellation")
    var abandoned := RENDERER.begin(decorated.data.chunk,world)
    abandoned.root.queue_free()
    await process_frame
    check(RENDERER.advance(abandoned) and abandoned.cancelled, "parent deletion prevents late attachment")
    # WebP display uses the same declarative image path, independently of source encoding.
    var image := Image.create(2,2,false,Image.FORMAT_RGBA8)
    image.fill(Color("c13b24"))
    var webp := image.save_webp_to_buffer(false)
    var sources := {"webp": {"path": "synthetic.webp", "bytes": webp, "material_json": "null"}}
    var material := RENDERER.ASSETS.material("webp",sources,{})
    check(material != null and material.albedo_texture.get_image().get_pixel(0,0).is_equal_approx(image.get_pixel(0,0)), "WebP decoded texture pixels")
    world.queue_free()
    await process_frame
    print("mapkit_assets: ", "PASS" if failures.is_empty() else "FAIL", " hash=", original, " batches=", steps)
    quit(0 if failures.is_empty() else 1)
'''
