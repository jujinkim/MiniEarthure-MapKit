extends SceneTree
## Golden-hour layer (sky_hour.gd): per-table keys, the golden weight's plateau
## and cut-offs, continuous sampling, the shader include mirroring the GDScript
## tables, and identity of every renderer output where the weight is 0 (24° and
## up, below -6°, cloudy cover), compared with the formulas the layer wraps.
const SKY_HOUR := preload("../sky_hour.gd")
const GRADE := preload("../color_grade.gd")
const RENDERER := preload("../environment_renderer.gd")
const PROFILE := preload("../environment_profile.gd")
const TABLES := ["LOW_SUN","LOW_AMBIENT","ZENITH","HORIZON","DUSK","HALO","CLOUD_LIT"]
const SHADER_TABLES := ["ZENITH","HORIZON","DUSK","HALO","CLOUD_LIT"]
class State extends RefCounted:
	var config := {"intensity":1.0,"sunset_minutes":1080,"sunrise_minutes":360,"aurora_probability":0.0,"celestial_mode":"simple"}
	var profile: Dictionary = PROFILE.defaults()
	var seconds := 43200.0
	var elapsed := 0.0
	var seed := 1
	var weather := "clear"
	var previous_weather := "clear"
	var wet := 0
	var snow := 0
	var degrees := 58.0
	func celestial() -> Dictionary:
		var result: Dictionary = PROFILE.celestial(seconds,config)
		var altitude := deg_to_rad(degrees)
		result.altitude = altitude
		result.sun_direction = Vector3(0.0,sin(altitude),-cos(altitude))
		result.daylight = smoothstep(deg_to_rad(-6.0),deg_to_rad(12.0),altitude)
		return result
	func blend() -> float: return 1.0
var failed := false
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func _initialize() -> void: run.call_deferred()
func near(a: Color, b: Color, tolerance := 1e-6) -> bool:
	return absf(a.r-b.r)<=tolerance and absf(a.g-b.g)<=tolerance and absf(a.b-b.b)<=tolerance
func keys(table: String) -> Array: return (SKY_HOUR as Script).get_script_constant_map()[table+"_KEYS"]
func colors(table: String) -> Array: return (SKY_HOUR as Script).get_script_constant_map()[table]
func vector(color: Color) -> Vector3: return Vector3(color.r,color.g,color.b)

func run() -> void:
	tables()
	weight()
	include()
	await renderer()
	print("sky_hour_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

func tables() -> void:
	for table: String in TABLES:
		var k := keys(table)
		check(k.size()>=2 and k.size()==colors(table).size(),"%s keys match its colours" % table)
		for index in range(1,k.size()): check(float(k[index])>float(k[index-1]),"%s keys ascend" % table)
		for index in k.size():
			check(near(SKY_HOUR.sample(k,colors(table),float(k[index])),colors(table)[index]),"%s hits key %d" % [table,index])
			var at := float(k[index])
			# Continuous across each key: a 1e-3° step moves less than the steepest slope allows.
			check(near(SKY_HOUR.sample(k,colors(table),at-1e-3),SKY_HOUR.sample(k,colors(table),at+1e-3),1e-3),"%s continuous at key %d" % [table,index])
		check(SKY_HOUR.sample(k,colors(table),float(k[0])-30.0)==colors(table)[0] and SKY_HOUR.sample(k,colors(table),float(k[k.size()-1])+30.0)==colors(table)[k.size()-1],"%s clamps outside its keys" % table)
	check(SKY_HOUR.DUSK[0]==SKY_HOUR.DUSK_COLOR,"DUSK -6° key equals today's dusk colour")

func weight() -> void:
	for deg in [0.0,1.0,3.0,5.5,6.0]: check(SKY_HOUR.golden(deg,0.18)==1.0,"golden plateau of 1 at %s°" % deg)
	for deg in [24.0,30.0,36.9,58.2,90.0,-6.0,-7.0,-30.0,-90.0]: check(SKY_HOUR.golden(deg,0.18)==0.0,"golden 0 at %s°" % deg)
	for cover in [0.80,0.83,0.94,0.97,1.0]:
		for deg in [0.0,3.0,6.0,12.0]: check(SKY_HOUR.golden(deg,cover)==0.0,"golden 0 under cover %s" % cover)
	var previous := SKY_HOUR.golden(-8.0,0.18)
	for step in 400:
		var deg := -8.0+step*0.1
		var value := SKY_HOUR.golden(deg,0.18)
		check(value>=0.0 and value<=1.0 and absf(value-previous)<0.03,"golden bounded and continuous at %.1f°" % deg)
		previous = value
	# Identity of every table blend wherever the weight is 0.
	var probe := Color(0.3141,0.2718,0.1618)
	for case: Array in [[24.0,0.18],[37.0,0.18],[58.0,0.18],[-7.0,0.18],[3.0,0.83],[0.0,0.97]]:
		var g := SKY_HOUR.golden(case[0],case[1])
		for table: String in TABLES:
			check(SKY_HOUR.blend(probe,keys(table),colors(table),case[0],g)==probe,"%s identity at %s° cover %s" % [table,case[0],case[1]])
		check(SKY_HOUR.DAY_FOG.lerp(SKY_HOUR.fog(case[0]),g)==SKY_HOUR.DAY_FOG,"fog identity at %s° cover %s" % [case[0],case[1]])
	check(near(SKY_HOUR.fog(12.0),SKY_HOUR.DAY_FOG,1e-6),"fog target meets the day fog at the 12° horizon key")

## The include's constants mirror sky_hour.gd; the sky shader defaults equal its day colours.
func include() -> void:
	var code: String = preload("../sky_hour.gdshaderinc").code
	var number := RegEx.create_from_string("-?[0-9]+\\.?[0-9]*")
	for table: String in SHADER_TABLES:
		var key_match := RegEx.create_from_string("const float SKY_HOUR_%s_KEYS\\[(\\d+)\\] = \\{([^}]*)\\};" % table).search(code)
		var color_match := RegEx.create_from_string("const vec3 SKY_HOUR_%s\\[(\\d+)\\] = \\{([^}]*)\\};" % table).search(code)
		check(key_match != null and color_match != null,"include declares %s" % table)
		if key_match == null or color_match == null: continue
		var parsed_keys := number.search_all(key_match.get_string(2)).map(func(m: RegExMatch): return m.get_string().to_float())
		check(int(key_match.get_string(1))==parsed_keys.size() and parsed_keys.size()==keys(table).size(),"include %s key count" % table)
		for index in mini(parsed_keys.size(),keys(table).size()): check(is_equal_approx(parsed_keys[index],float(keys(table)[index])),"include %s key %d" % [table,index])
		var triples := RegEx.create_from_string("vec3\\(([^)]*)\\)").search_all(color_match.get_string(2))
		check(int(color_match.get_string(1))==triples.size() and triples.size()==colors(table).size(),"include %s colour count" % table)
		for index in mini(triples.size(),colors(table).size()):
			var parts: PackedStringArray = triples[index].get_string(1).split(",")
			check(parts.size()==3 and near(Color(parts[0].to_float(),parts[1].to_float(),parts[2].to_float()),colors(table)[index]),"include %s colour %d" % [table,index])
	for pair: Array in [["SKY_HOUR_DAY_ZENITH",SKY_HOUR.DAY_ZENITH],["SKY_HOUR_DAY_HORIZON",SKY_HOUR.DAY_HORIZON],["SKY_HOUR_CLOUD_DAY",SKY_HOUR.CLOUD_DAY]]:
		var found := RegEx.create_from_string("const vec3 %s = vec3\\(([^)]*)\\);" % pair[0]).search(code)
		var parts: PackedStringArray = found.get_string(1).split(",") if found != null else PackedStringArray()
		check(parts.size()==3 and near(Color(parts[0].to_float(),parts[1].to_float(),parts[2].to_float()),pair[1]),"include %s" % pair[0])
	check(code.contains("(1.0-smoothstep(6.0,24.0,deg))*smoothstep(-6.0,0.0,deg)*(1.0-smoothstep(0.45,0.80,cover))"),"include golden weight mirrors sky_hour.golden")
	var sky: String = preload("../atmosphere_sky.gdshader").code
	for pair: Array in [["day_zenith",SKY_HOUR.DAY_ZENITH],["day_horizon",SKY_HOUR.DAY_HORIZON],["dusk_color",SKY_HOUR.DUSK_COLOR],["halo_color",SKY_HOUR.HALO_COLOR],["cloud_day",SKY_HOUR.CLOUD_DAY]]:
		var found := RegEx.create_from_string("uniform vec3 %s = vec3\\(([^)]*)\\);" % pair[0]).search(sky)
		var parts: PackedStringArray = found.get_string(1).split(",") if found != null else PackedStringArray()
		check(parts.size()==3 and near(Color(parts[0].to_float(),parts[1].to_float(),parts[2].to_float()),pair[1]),"sky shader default %s" % pair[0])
	var surface: String = preload("../atmosphere_surface.gdshader").code
	check(surface.contains('#include "sky_hour.gdshaderinc"') and surface.contains("facade_sky(rise,weather.z,clock.z)"),"glass reflection reads the sun altitude through the include")

## Renderer outputs against the pre-layer formulas wherever golden is 0, and a
## warmer sun, sky and grade at a clear 3° sun.
func renderer() -> void:
	var world := WorldEnvironment.new(); world.environment = Environment.new(); root.add_child(world)
	var sun := DirectionalLight3D.new(); root.add_child(sun)
	var view := RENDERER.new(); root.add_child(view); view.configure(world.environment,sun,false)
	for frame in 2: await physics_frame
	var state := State.new()
	var design := PROFILE.design_at(state.profile,Vector2.ZERO)
	var settlement := str(design.get("settlement","urban" if design.get("concept")=="metropolis" else "sparse"))
	var night_color := Color(0.42,0.49,0.63) if settlement == "urban" else Color(0.23,0.32,0.48)
	var reference := GRADE.new()
	var stored := Environment.new()
	for case: Array in [[24.0,"clear"],[37.0,"clear"],[58.0,"clear"],[-7.0,"clear"],[3.0,"cloudy"],[0.0,"rain"],[2.0,"snow"]]:
		state.degrees = case[0]; state.weather = case[1]; state.previous_weather = case[1]
		view.update_environment(state,Vector3(0,2,0),[],null,true)
		var label := "%s° %s" % case
		var celestial := state.celestial()
		var day := float(celestial.daylight)
		var snow := float(state.snow)*0.5
		var high_sun := smoothstep(0.0,0.65,maxf(0.0,sin(float(celestial.altitude))))
		check(sun.light_color==Color(1.0,0.85,0.66).lerp(Color(1.0,0.98,0.93),high_sun).lerp(Color(0.92,0.96,1.0),snow*0.65),"sun colour unchanged at "+label)
		check(world.environment.ambient_light_color==night_color.lerp(Color(0.72,0.73,0.66),day),"ambient unchanged at "+label)
		# Stored through the same float property, so the comparison is exact.
		stored.adjustment_saturation = lerpf(1.0,.98,day*(1.0-snow*2.0))
		check(world.environment.adjustment_saturation==stored.adjustment_saturation,"saturation unchanged at "+label)
		check(world.environment.fog_light_color==Color(0.004,0.007,0.015).lerp(Color(0.48,0.49,0.46),day),"fog unchanged at "+label)
		for pair: Array in [["day_zenith",Vector3(0.16,0.28,0.40)],["day_horizon",Vector3(0.58,0.57,0.51)],["dusk_color",Vector3(0.48,0.21,0.055)],["halo_color",Vector3(0.38,0.25,0.12)],["cloud_day",Vector3(0.27,0.30,0.34)]]:
			check(view.sky_material.get_shader_parameter(pair[0])==pair[1],"sky %s unchanged at %s" % [pair[0],label])
		reference.update(day,clampf(snow*2.0,0.0,1.0))
		check(view.color_grade.texture.gradient.colors==reference.texture.gradient.colors,"grade unchanged at "+label)
	state.degrees = 3.0; state.weather = "clear"; state.previous_weather = "clear"
	view.update_environment(state,Vector3(0,2,0),[],null,true)
	var celestial := state.celestial()
	var high_sun := smoothstep(0.0,0.65,maxf(0.0,sin(float(celestial.altitude))))
	var plain := Color(1.0,0.85,0.66).lerp(Color(1.0,0.98,0.93),high_sun)
	check(sun.light_color.b<plain.b-0.1 and near(sun.light_color,SKY_HOUR.sample(SKY_HOUR.LOW_SUN_KEYS,SKY_HOUR.LOW_SUN,3.0),1e-5),"clear 3° sun takes the low-sun colour")
	var horizon: Vector3 = view.sky_material.get_shader_parameter("day_horizon")
	check(horizon.x-horizon.z>0.25,"clear 3° horizon turns warm")
	check(world.environment.adjustment_saturation>lerpf(1.0,.98,float(celestial.daylight))+0.04,"clear 3° saturation lifts")
	reference.update(float(celestial.daylight),0.0)
	var top: Color = view.color_grade.texture.gradient.colors[GRADE.STOPS.size()-1]
	var plain_top: Color = reference.texture.gradient.colors[GRADE.STOPS.size()-1]
	check(top.b<plain_top.b-0.03 and top.r>=plain_top.r,"clear 3° grade warms the highlight")
	view.free(); sun.free(); world.free()
	for frame in 3: await process_frame
