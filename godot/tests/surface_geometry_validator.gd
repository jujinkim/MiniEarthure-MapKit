extends SceneTree
const GEOMETRY := preload("../gimmick_geometry.gd")
var failed := false
func check(ok: bool, message: String) -> void:
	if not ok: failed = true; push_error(message)
func _initialize() -> void: run.call_deferred()
func run() -> void:
	var fixture: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://addons/mapkit/godot/tests/haeon_surface_fixture.json"))
	var world := Node3D.new()
	root.add_child(world)
	var index := 0
	for g: Dictionary in fixture.gimmicks:
		var visual := GEOMETRY.visual(g)
		world.add_child(visual)
		visual.position = Vector3((index % 4)*8,0,-(index/4)*10)
		for child: MeshInstance3D in visual.get_children():
			var arrays := child.mesh.surface_get_arrays(0)
			var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
			var normals: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
			var center := Vector3.ZERO
			for v in vertices: center += v
			center /= vertices.size()
			for i in range(0,vertices.size(),3):
				var front := (vertices[i+2]-vertices[i]).cross(vertices[i+1]-vertices[i]).normalized()
				check(front.dot(vertices[i]-center)>0.0001, g.id+": outward clockwise face")
				check(front.dot(normals[i])>0.99,g.id+": matching normal")
			check(child.material_override != null,g.id+": solid material")
		index += 1
	if DisplayServer.get_name() != "headless":
		var light := DirectionalLight3D.new(); world.add_child(light); light.rotation_degrees=Vector3(-55,-25,0)
		var camera := Camera3D.new(); world.add_child(camera)
		camera.position=Vector3(30,26,25); camera.look_at(Vector3(11,0,-10)); camera.current=true
		await process_frame; await process_frame
		await RenderingServer.frame_post_draw
		var path := OS.get_environment("MAPKIT_SURFACE_CAPTURE")
		if not path.is_empty(): root.get_texture().get_image().save_png(path)
	world.free()
	print("surface_geometry_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
