extends Node3D
## Public visual consumer. Receives resolved state; never advances a simulation.
const SKY_SHADER := preload("./atmosphere_sky.gdshader")
const PROFILE := preload("./environment_profile.gd")
var settings: Environment
var sun: DirectionalLight3D
var moon: DirectionalLight3D
var sky_material: ShaderMaterial
var lights: Array[SpotLight3D] = []
var precipitation: GPUParticles3D
var particle_material: ParticleProcessMaterial
var _last_update := -1
var _wet := 0.0
var _snow := 0.0
var _clouds := 0.15
var _context: RefCounted
var _light_candidates: Array = []
var _scan_at := 0
var low := false
var _night_lights := false
var display_distance := 0.0 # Opt-in scene metres; zero preserves Editor/default weather fog.
var _weather_fog_density := 0.0008
var _weather_fog_energy := 0.6

func set_display_distance(distance: float) -> void:
	if not is_finite(distance) or distance < 0.0: return
	if is_equal_approx(display_distance, distance): return
	display_distance = distance
	_apply_distance_fog()

func _apply_distance_fog() -> void:
	if settings == null: return
	var enabled := display_distance > 0.0
	settings.fog_mode = Environment.FOG_MODE_DEPTH if enabled else Environment.FOG_MODE_EXPONENTIAL
	settings.fog_density = 1.0 if enabled else _weather_fog_density
	settings.fog_aerial_perspective = 1.0 if enabled else 0.0
	settings.fog_sky_affect = 0.0 if enabled else 1.0
	settings.fog_light_energy = 1.0 if enabled else _weather_fog_energy
	if enabled:
		settings.fog_depth_begin = display_distance * 0.60
		settings.fog_depth_end = display_distance * 0.95
		settings.fog_depth_curve = 0.7

func configure(environment: Environment, key: DirectionalLight3D, low_quality: bool) -> void:
	settings = environment
	sun = key
	low = low_quality
	settings.background_mode = Environment.BG_SKY
	settings.sky = Sky.new()
	settings.sky.radiance_size = Sky.RADIANCE_SIZE_32
	settings.sky.process_mode = Sky.PROCESS_MODE_INCREMENTAL
	sky_material = ShaderMaterial.new()
	sky_material.shader = SKY_SHADER
	settings.sky.sky_material = sky_material
	settings.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	settings.tonemap_mode = Environment.TONE_MAPPER_ACES
	settings.fog_enabled = true
	settings.fog_density = 0.0008
	settings.glow_enabled = false
	sun.directional_shadow_max_distance = 96.0 if low else 160.0
	moon = DirectionalLight3D.new()
	moon.light_color = Color(0.49,0.62,0.83)
	add_child(moon)
	for index in (4 if low else 8):
		var light := SpotLight3D.new()
		light.spot_range = 24.0
		light.spot_angle = 48.0
		light.light_color = Color(1.0,0.84,0.61)
		light.visible = false
		light.shadow_enabled = index == 0
		add_child(light)
		lights.append(light)
	precipitation = GPUParticles3D.new()
	precipitation.amount = 350 if low else 1100
	precipitation.lifetime = 6.0
	precipitation.visibility_aabb = AABB(Vector3(-15,-15,-15),Vector3(30,40,30))
	precipitation.emitting = false
	particle_material = ParticleProcessMaterial.new()
	particle_material.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_BOX
	particle_material.emission_box_extents = Vector3(12,1,12)
	particle_material.direction = Vector3(0,-1,0)
	particle_material.spread = 6.0
	particle_material.gravity = Vector3(0,-2,0)
	particle_material.initial_velocity_min = 10
	particle_material.initial_velocity_max = 14
	precipitation.process_material = particle_material
	var mesh := QuadMesh.new()
	mesh.size = Vector2(0.025,0.25)
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.56,0.65,0.70,0.4)
	material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	material.billboard_mode = BaseMaterial3D.BILLBOARD_ENABLED
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mesh.material = material
	precipitation.draw_pass_1 = mesh
	add_child(precipitation)

func update_environment(state: RefCounted, camera_position: Vector3, _vehicles: Array, resources: RefCounted = null, immediate := false) -> void:
	if state == null or state.config.is_empty(): return
	var celestial: Dictionary = state.celestial()
	var day := float(celestial.daylight)
	var design := PROFILE.design_at(state.profile,Vector2(camera_position.x,-camera_position.z))
	var settlement := str(design.get("settlement","urban" if design.get("concept")=="metropolis" else "sparse"))
	var night_ambient := 0.07 if settlement == "urban" else (0.028 if settlement == "village" else 0.012)
	var now := Time.get_ticks_msec()
	var delta := 0.1 if _last_update < 0 else clampf(float(now - _last_update) / 1000.0,0.0,0.25)
	_last_update = now
	_wet = float(state.wet)*0.5 if immediate else move_toward(_wet,float(state.wet)*0.5,delta*0.2)
	_snow = float(state.snow)*0.5 if immediate else move_toward(_snow,float(state.snow)*0.5,delta*0.15)
	var cover := {"clear":0.18,"cloudy":0.83,"rain":0.97,"snow":0.94}
	_clouds = lerpf(float(cover.get(state.previous_weather,0.18)),float(cover.get(state.weather,0.18)),state.blend())
	_orient(sun,celestial.sun_direction)
	_orient(moon,celestial.moon_direction)
	sun.light_color = Color(1.0,0.68,0.43).lerp(Color(1.0,0.94,0.84),day)
	sun.light_energy = maxf(0.0,sin(float(celestial.altitude)))*1.05*(1.0-_clouds*0.55)
	moon.light_energy = maxf(0.0,sin(float(celestial.moon_altitude)))*float(celestial.moon_phase)*0.12*(1.0-_clouds*0.85)
	sun.shadow_enabled = float(celestial.altitude) > 0.0
	moon.shadow_enabled = not sun.shadow_enabled and moon.light_energy > 0.01
	settings.ambient_light_color = Color(0.23,0.32,0.48).lerp(Color(0.62,0.69,0.72),day)
	settings.ambient_light_energy = lerpf(night_ambient,0.24,day)*(1.0-_clouds*0.35)
	settings.fog_light_color = Color(0.004,0.007,0.015).lerp(Color(0.36,0.42,0.46),day)
	_weather_fog_density = lerpf(0.0008,0.006,_clouds*float(state.config.intensity))
	_weather_fog_energy = lerpf(0.04,0.6,day)
	_apply_distance_fog()
	sky_material.set_shader_parameter("sun_direction",celestial.sun_direction)
	sky_material.set_shader_parameter("moon_direction",celestial.moon_direction)
	sky_material.set_shader_parameter("daylight",day)
	sky_material.set_shader_parameter("moon_phase",celestial.moon_phase)
	sky_material.set_shader_parameter("cloud_cover",_clouds)
	sky_material.set_shader_parameter("cloud_motion",float(state.elapsed)*0.012)
	var concept := str(design.get("climate",design.get("concept","metropolis")))
	var night: int = PROFILE.night_index(state.seconds)
	var aurora_event := PROFILE.stable_unit("aurora/%s/%d" % [state.seed,night]) < float(state.config.aurora_probability)
	var duration := lerpf(20.0,60.0,PROFILE.stable_unit("aurora-duration/%s/%d" % [state.seed,night]))
	var start := 20.0 + 3.0*PROFILE.stable_unit("aurora-start/%s/%d" % [state.seed,night])
	var night_minute := fposmod(float(state.seconds)-start*3600.0,86400.0)/60.0
	var aurora_strength := sin(PI*clampf(night_minute/duration,0.0,1.0)) if night_minute < duration else 0.0
	sky_material.set_shader_parameter("aurora",aurora_strength if concept == "polar" and aurora_event else 0.0)
	if resources != null and resources.has_method("environment_context"):
		_context = resources.environment_context()
		if _context != null: _context.update(state.seconds,_wet,_snow,float(celestial.altitude),int(celestial.get("sunset_minutes",state.config.sunset_minutes)))
	_update_precipitation(state,camera_position)
	_update_static_lights(state,camera_position)

func _orient(light: DirectionalLight3D, direction: Vector3) -> void:
	light.basis = Basis.looking_at(-direction,Vector3.FORWARD if absf(direction.y)>0.99 else Vector3.UP)

func _update_precipitation(state: RefCounted, camera_position: Vector3) -> void:
	precipitation.global_position = camera_position + Vector3.UP*10.0
	var wet_weather: bool = state.weather in ["rain","snow"]
	var previous_wet: bool = state.previous_weather in ["rain","snow"]
	var strength := lerpf(1.0 if previous_wet else 0.0,1.0 if wet_weather else 0.0,state.blend())*float(state.config.intensity)
	var query := PhysicsRayQueryParameters3D.create(camera_position,camera_position+Vector3.UP*1000.0)
	query.hit_back_faces = true
	var sheltered := not get_world_3d().direct_space_state.intersect_ray(query).is_empty()
	precipitation.emitting = strength > 0.01 and not sheltered
	precipitation.amount_ratio = maxf(0.01,strength)
	var snowing: bool = (state.previous_weather if state.blend()<0.5 else state.weather) == "snow"
	particle_material.initial_velocity_min = 1.0 if snowing else 10.0
	particle_material.initial_velocity_max = 2.5 if snowing else 14.0
	particle_material.gravity = Vector3(0,-0.5 if snowing else -2.0,0)
	precipitation.draw_pass_1.size = Vector2(0.09,0.09) if snowing else Vector2(0.025,0.25)

func _update_static_lights(state: RefCounted, camera_position: Vector3) -> void:
	_night_lights = float(state.celestial().altitude) < 0.0
	if Time.get_ticks_msec() >= _scan_at:
		_scan_at = Time.get_ticks_msec()+500
		_light_candidates.clear()
		for root: Node3D in get_tree().get_nodes_in_group("mapkit_environment_cells"):
			if root.get_world_3d() != get_world_3d(): continue
			for candidate: Dictionary in root.get_meta("environment_lamps",[]):
				if candidate.position.distance_squared_to(camera_position) < 1600.0: _light_candidates.append(candidate)
		_light_candidates.sort_custom(func(a: Dictionary,b: Dictionary): return a.position.distance_squared_to(camera_position)<b.position.distance_squared_to(camera_position))
## Render poses are supplied by the consumer after vehicle and camera updates.
func update_dynamic_lights(camera_position: Vector3, vehicles: Array) -> void:
	var selected: Array = []
	for vehicle: Dictionary in vehicles:
		if selected.size() >= lights.size(): break
		var pose: Transform3D = vehicle.pose
		if not vehicle.get("primary",false) and pose.origin.distance_squared_to(camera_position)>2025.0: continue
		selected.append({"position":pose.origin + pose.basis*Vector3(0,0.22,-0.30),
			"basis":pose.basis,"energy":3.2,"range":30.0,"color":Color(1.0,0.88,0.68)})
	if _night_lights:
		for candidate: Dictionary in _light_candidates:
			if selected.size() >= lights.size(): break
			selected.append(candidate)
	for index in lights.size():
		var light := lights[index]
		light.visible = index < selected.size()
		if not light.visible: continue
		var candidate: Dictionary = selected[index]
		light.global_position = candidate.position
		light.global_basis = candidate.basis
		light.light_color = candidate.color
		light.light_energy = candidate.energy
		light.spot_range = candidate.range
