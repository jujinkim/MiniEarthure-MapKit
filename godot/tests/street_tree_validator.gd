extends SceneTree
const ASSETS := preload("../asset_library.gd")
const INSTANCES := preload("../render_instances.gd")
func _initialize()->void:run.call_deferred()
func run()->void:
	var source:={"tree":{"path":"tree.glb","material_json":"null","bytes":FileAccess.get_file_as_bytes(OS.get_environment("TREE_FIXTURE_GLB"))}}
	var tree:=ASSETS.template("tree",source,{})
	if tree==null:push_error("tree failed to import");quit(1);return
	var world:=Node3D.new();root.add_child(world);world.add_child(tree)
	var env:=WorldEnvironment.new();env.environment=Environment.new();env.environment.background_mode=Environment.BG_COLOR;env.environment.background_color=Color(.4,.5,.6);env.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR;env.environment.ambient_light_color=Color.WHITE;env.environment.ambient_light_energy=.6;world.add_child(env)
	var sun:=DirectionalLight3D.new();sun.rotation_degrees=Vector3(-45,-30,0);world.add_child(sun)
	var group:=INSTANCES.begin(tree,2,world,null)
	INSTANCES.append(group,Transform3D(Basis.IDENTITY,Vector3(10,0,0)),"a")
	INSTANCES.append(group,Transform3D(Basis.IDENTITY,Vector3(-10,0,0)),"b")
	var camera:=Camera3D.new();camera.position=Vector3(10,9,24);world.add_child(camera);camera.look_at(Vector3(0,6,0))
	for i in 5:await process_frame
	await RenderingServer.frame_post_draw
	var picture:=root.get_texture().get_image()
	var failed:=false
	for x in [-10,0,10]:
		var center:=Vector2i(camera.unproject_position(Vector3(x,8.5,0)))
		var greens:=0
		for px in range(center.x-24,center.x+24):
			for py in range(center.y-24,center.y+24):
				var color:=picture.get_pixel(px,py)
				if color.g>color.r*1.1 and color.g>color.b*1.1 and color.g>.2:greens+=1
		if greens<100:failed=true;push_error("single and instanced foliage retain green tint: "+str([x,greens]))
	picture.save_png(OS.get_environment("TREE_RENDER_OUT"))
	world.free();group.clear();for i in 5:await process_frame
	print("street_tree_validator: ","FAIL" if failed else "PASS");quit(1 if failed else 0)
