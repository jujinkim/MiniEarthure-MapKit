extends SceneTree
const RENDER := preload("../chunk_renderer.gd")
const MATERIALS := preload("../environment_materials.gd")
var failed := false
func _initialize() -> void: run.call_deferred()
func check(ok: bool, label: String) -> void:
	if not ok: failed=true;push_error(label)
func pixel(image:Image,camera:Camera3D,p:Vector3)->Color:
	var screen:=camera.unproject_position(p)
	return image.get_pixel(roundi(screen.x),roundi(screen.y))
func run()->void:
	root.size=Vector2i(640,640)
	var world:=Node3D.new();root.add_child(world)
	var environment:=WorldEnvironment.new();environment.environment=Environment.new();world.add_child(environment)
	environment.environment.background_mode=Environment.BG_COLOR
	environment.environment.ambient_light_source=Environment.AMBIENT_SOURCE_COLOR
	environment.environment.ambient_light_color=Color.WHITE;environment.environment.ambient_light_energy=1.0
	var context:=MATERIALS.new()
	var paths:=PackedVector4Array();paths.resize(128);paths[0]=Vector4(2,-6,2,6)
	var metrics:=PackedVector4Array();metrics.resize(128);metrics[0]=Vector4(4,0,2.4,12)
	var borders:=PackedVector4Array();borders.resize(128);borders[0]=Vector4(0,-6,0,6);borders[1]=Vector4(4,6,4,-6)
	var style:Dictionary={"surface":0,"marked":true,"snow_retention_percent":15.0,"base_color":Color(39/255.0,43/255.0,49/255.0),"road_paths":paths,"road_metrics":metrics,"road_borders":borders,"path_count":1,"edge_count":2,"lanes":2,"center_line":true,"edge_lines":true}
	var road:=RENDER.surface_material("road:test",{"urban_surfaces":true,"road_styles":{"road:test":style}},RENDER.wet_urban_shader()) as ShaderMaterial
	road.set_shader_parameter("environment_enabled",true);road.set_shader_parameter("environment_data",context.texture)
	check(float(road.get_shader_parameter("snow_retention_percent"))==15,"retention reaches shader unchanged")
	var source:=StandardMaterial3D.new();source.resource_name="mk_grass";source.albedo_color=Color(.85,.89,.91)
	for i in 2:
		var mesh:=MeshInstance3D.new();mesh.mesh=PlaneMesh.new();mesh.mesh.size=Vector2(4,12);mesh.position=Vector3(2+4*i,0,0);mesh.material_override=road if i==0 else context.surface_material(source,0);world.add_child(mesh)
	var camera:=Camera3D.new();world.add_child(camera);camera.position=Vector3(4,12,0);camera.look_at(Vector3(4,0,0),Vector3.FORWARD);camera.projection=Camera3D.PROJECTION_ORTHOGONAL;camera.size=12;camera.current=true
	var output:=OS.get_environment("SNOW_RENDER_DIR")
	if not output.is_empty():DirAccess.make_dir_recursive_absolute(output)
	for stage in 3:
		context.update(43200,0,float(stage)*.5,1,1080)
		await process_frame;await process_frame;await RenderingServer.frame_post_draw
		var image:=root.get_texture().get_image()
		var asphalt:=pixel(image,camera,Vector3(1,0,0));var snow:=pixel(image,camera,Vector3(6,0,0));var yellow:=pixel(image,camera,Vector3(2.07,0,0));var white:=pixel(image,camera,Vector3(.16,0,0))
		check(snow.get_luminance()-asphalt.get_luminance()>.2,"snow/road contrast stage "+str(stage))
		check(yellow.r-yellow.b>.15 and yellow.get_luminance()>asphalt.get_luminance()+.15,"yellow center preserved stage "+str(stage))
		check(white.get_luminance()>asphalt.get_luminance()+.25,"white edge preserved stage "+str(stage))
		if not output.is_empty():image.save_png(output.path_join("snow-stage-%d.png"%stage))
		print("SNOW_RENDER ",stage," asphalt=",asphalt," snow=",snow," yellow=",yellow," white=",white)
	world.free();await process_frame
	print("snow_surface_validator: ","FAIL" if failed else "PASS");quit(1 if failed else 0)
