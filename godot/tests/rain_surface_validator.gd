extends SceneTree
const RENDERER := preload("../environment_renderer.gd")
const MATERIALS := preload("../environment_materials.gd")
const PROFILE := preload("../environment_profile.gd")
class State extends RefCounted:
	var config := {"intensity":1.0,"sunset_minutes":1080,"sunrise_minutes":360,"aurora_probability":0.0,"celestial_mode":"simple"}
	var profile: Dictionary=PROFILE.defaults()
	var seconds:=43200.0
	var elapsed:=0.0
	var seed:=1
	var weather:="rain"
	var previous_weather:="rain"
	var wet:=2
	var snow:=0
	func celestial() -> Dictionary:return PROFILE.celestial(seconds,config)
	func blend() -> float:return 1.0
var failed:=false
func check(ok: bool,label: String) -> void:
	if not ok:failed=true;push_error(label)
func _initialize() -> void:run.call_deferred()
func box(size:Vector3,position:Vector3)->StaticBody3D:
	var node:=StaticBody3D.new();var shape:=CollisionShape3D.new();shape.shape=BoxShape3D.new();shape.shape.size=size
	node.add_child(shape);node.position=position;root.add_child(node);return node
func disc_energy(picture:Image)->float:
	var center:=picture.get_size()/2
	var energy:=0.0
	for x in range(center.x-18,center.x+18):
		for y in range(center.y-18,center.y+18):energy+=picture.get_pixel(x,y).get_luminance()
	return energy
func run()->void:
	var floor:=box(Vector3(50,1,50),Vector3(0,-.5,0))
	var state:=State.new()
	var camera:=Camera3D.new();root.add_child(camera);camera.position=Vector3(0,2,5);camera.look_at(Vector3(0,.6,-5))
	var world_env:=WorldEnvironment.new();world_env.environment=Environment.new();root.add_child(world_env)
	var materials:=MATERIALS.new()
	var floor_mesh:=MeshInstance3D.new();floor_mesh.mesh=BoxMesh.new();floor_mesh.mesh.size=Vector3(50,1,50);floor.add_child(floor_mesh)
	var source:=StandardMaterial3D.new();source.albedo_color=Color(.25,.27,.3);source.roughness=.9
	floor_mesh.material_override=materials.surface_material(source,0)
	var output:=OS.get_environment("WEATHER_RENDER_DIR")
	if not output.is_empty():DirAccess.make_dir_recursive_absolute(output)
	for low in [false,true]:
		var sun:=DirectionalLight3D.new();root.add_child(sun)
		var renderer:=RENDERER.new();root.add_child(renderer);renderer.configure(world_env.environment,sun,low)
		for i in 3:await physics_frame
		for i in 12:
			renderer.update_environment(state,Vector3(0,2,0),[],null,true)
			renderer._process(.03)
		check(renderer.precipitation.emitting,"rain enabled")
		check(renderer._splash_age.count(1.0)<renderer._splash_age.size(),"surface splash slots used")
		check(renderer.surface_ray_count<=24,"at most two surface rays per environment update")
		check(renderer.splashes.multimesh.instance_count==(24 if low else 64),"fixed platform pool")
		var roof:=box(Vector3(40,.3,40),Vector3(0,4,0));for i in 3:await physics_frame
		renderer.update_environment(state,Vector3(0,2,0),[],null,true)
		check(not renderer.precipitation.visible and renderer._splash_age.count(1.0)==renderer._splash_age.size(),"roof suppresses rain and splashes")
		roof.free();for i in 3:await physics_frame
		for weather in ["clear","snow","rain"]:
			state.weather=weather;state.previous_weather=weather
			renderer.update_environment(state,Vector3(0,2,0),[],null,true)
			check(renderer.precipitation.emitting==(weather!="clear"),"weather transition "+weather)
			if weather!="rain":check(renderer._splash_age.count(1.0)==renderer._splash_age.size(),"dry/snow clears splashes")
		if not low and not output.is_empty():
			for hour in [12,0]:
				state.seconds=hour*3600.0
				for weather in ["clear","rain","snow"]:
					state.weather=weather;state.previous_weather=weather;state.wet=2 if weather=="rain" else 0;state.snow=2 if weather=="snow" else 0
					for i in 45:
						renderer.update_environment(state,camera.position,[],null,true)
						materials.update(state.seconds,float(state.wet)/2.0,float(state.snow)/2.0,float(state.celestial().altitude),1080)
						await process_frame
					await RenderingServer.frame_post_draw
					root.get_texture().get_image().save_png(output.path_join("%02d-%s.png"%[hour,weather]))
			# Point directly at the known sky disc; verify noon and midnight paths.
			for hour in [12,0]:
				state.seconds=hour*3600.0;state.weather="clear";state.previous_weather="clear"
				renderer.update_environment(state,camera.position,[],null,true)
				var direction:Vector3=renderer.sky_material.get_shader_parameter("sun_direction" if hour==12 else "moon_direction")
				camera.look_at(camera.position+direction,Vector3.FORWARD if absf(direction.y)>.99 else Vector3.UP)
				for i in 5:await process_frame
				await RenderingServer.frame_post_draw
				var picture:=root.get_texture().get_image()
				picture.save_png(output.path_join("disc-%02d.png"%hour))
				if hour==0:
					var full_energy:=disc_energy(picture)
					check(full_energy>20,"full moon visibly lit")
					renderer.sky_material.set_shader_parameter("sun_direction",direction.cross(Vector3.UP).normalized())
					for i in 5:await process_frame
					await RenderingServer.frame_post_draw
					picture=root.get_texture().get_image();picture.save_png(output.path_join("disc-half.png"))
					check(disc_energy(picture)>full_energy*.25 and disc_energy(picture)<full_energy*.75,"phase changes illuminated disc")
					renderer.sky_material.set_shader_parameter("cloud_cover",1.0)
					for i in 5:await process_frame
					await RenderingServer.frame_post_draw
					check(disc_energy(root.get_texture().get_image())<full_energy*.2,"cloud occludes moon")
			camera.look_at(Vector3(0,.6,-5));state.weather="rain";state.previous_weather="rain";state.wet=2;state.snow=0
		var refs: Array[WeakRef]=[weakref(renderer.splashes.multimesh),weakref(renderer.splashes.multimesh.mesh),weakref(renderer.splash_material)]
		renderer.free();sun.free();await process_frame
		check(refs.all(func(r):return r.get_ref()==null),"splash resources retire")
	floor.free();camera.free();world_env.free();materials=null;source=null
	for i in 5:await process_frame
	print("rain_surface_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
