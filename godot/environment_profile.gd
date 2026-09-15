extends RefCounted
## Public, rendering-independent environment math. No clock, I/O or game authority.
const VERSION := 1
const CONCEPTS := ["polar", "metropolis", "countryside", "middle-eastern", "desert", "jungle", "southeast-asian"]
const WEIGHTS := [[25,35,0,40], [45,30,20,5], [45,25,25,5], [65,25,10,0], [80,18,2,0], [25,30,45,0], [30,25,45,0]]

static func defaults(concept := "metropolis") -> Dictionary:
	if concept not in CONCEPTS: concept = "metropolis"
	return {"version": VERSION, "concept": concept, "latitude_mdeg": 37500, "longitude_mdeg": 127000,
		"utc_offset_minutes": 540, "sunrise_minutes": 360, "sunset_minutes": 1080, "regions": [], "lights": []}

static func weights(concept: String) -> Array:
	var index := CONCEPTS.find(concept)
	return WEIGHTS[maxi(0, index)].duplicate() if index >= 0 else WEIGHTS[1].duplicate()

static func concept_at(profile: Dictionary, point: Vector2) -> String:
	for region: Dictionary in profile.get("regions", []):
		var polygon := PackedVector2Array()
		for vertex: Array in region.polygon: polygon.append(Vector2(vertex[0], vertex[1]) / 100.0)
		if Geometry2D.is_point_in_polygon(point, polygon): return str(region.concept)
	return str(profile.get("concept", "metropolis"))

static func stable_unit(identity: String) -> float:
	# 28 exact integer bits; independent of process, object allocation and load order.
	return float(identity.sha256_text().substr(0, 7).hex_to_int()) / 268435456.0

static func building_cutoff(map_id: String, object_id: String, night: int) -> float:
	var seed := (map_id + "/" + object_id).sha256_text().substr(0,6).hex_to_int()
	return float((seed ^ (((night+100000)*31337)&0xffffffff)) % 241) * 60.0

static func building_on(seconds: float, cutoff: float, sun: Dictionary, sunset_minutes := 1080) -> bool:
	if float(sun.altitude) >= 0.0: return false
	var day_second := fposmod(seconds, 86400.0)
	# Midnight belongs to the preceding evening. During polar night repeat daily.
	return day_second < cutoff or day_second >= float(sunset_minutes) * 60.0

static func night_index(seconds: float) -> int:
	return floori((seconds - 12.0 * 3600.0) / 86400.0)

static func _direction(altitude: float, azimuth: float) -> Vector3:
	return Vector3(sin(azimuth) * cos(altitude), sin(altitude), -cos(azimuth) * cos(altitude))

static func celestial(seconds: float, config: Dictionary) -> Dictionary:
	var hour := fposmod(seconds, 86400.0) / 3600.0
	var sun_alt: float
	var sun_az: float
	var moon_alt: float
	var moon_az: float
	var phase := 1.0
	if config.get("celestial_mode", "simple") == "astronomical":
		# Low-cost orbital elements (J2000); angles evaluated offline in radians.
		var utc := float(config.get("date_epoch", 1774051200)) + seconds - float(config.get("utc_offset_minutes", 540)) * 60.0
		var d := utc / 86400.0 + 2440587.5 - 2451545.0
		var latitude := deg_to_rad(float(config.get("latitude_mdeg", 37500)) / 1000.0)
		var longitude := deg_to_rad(float(config.get("longitude_mdeg", 127000)) / 1000.0)
		var obliquity := deg_to_rad(23.4393 - 0.0000003563 * d)
		var mean_anomaly := deg_to_rad(fposmod(357.5291 + 0.98560028 * d, 360.0))
		var sun_long := deg_to_rad(fposmod(280.4665 + 0.98564736 * d, 360.0)) + deg_to_rad(1.9148) * sin(mean_anomaly) + deg_to_rad(0.0200) * sin(2.0 * mean_anomaly)
		var sidereal := deg_to_rad(fposmod(280.16 + 360.9856235 * d, 360.0)) + longitude
		var sun_pos := _horizontal(sun_long, 0.0, obliquity, sidereal, latitude)
		sun_alt = sun_pos.x
		sun_az = sun_pos.y
		var lunar_anomaly := deg_to_rad(fposmod(134.963 + 13.064993 * d, 360.0))
		var lunar_long := deg_to_rad(fposmod(218.316 + 13.176396 * d, 360.0)) + deg_to_rad(6.289) * sin(lunar_anomaly)
		var lunar_lat := deg_to_rad(5.128) * sin(deg_to_rad(fposmod(93.272 + 13.229350 * d, 360.0)))
		var moon_pos := _horizontal(lunar_long, lunar_lat, obliquity, sidereal, latitude)
		moon_alt = moon_pos.x
		moon_az = moon_pos.y
		phase = (1.0 - cos(lunar_long - sun_long)) * 0.5
	else:
		var rise := float(config.get("sunrise_minutes", 360)) / 60.0
		var setting := float(config.get("sunset_minutes", 1080)) / 60.0
		var length := fposmod(setting - rise, 24.0)
		var elapsed := fposmod(hour - rise, 24.0)
		var angle := PI * elapsed / length if elapsed <= length else PI + PI * (elapsed - length) / (24.0 - length)
		sun_alt = asin(sin(angle) * 0.85)
		sun_az = angle - PI * 0.5
		moon_alt = -sun_alt
		moon_az = sun_az + PI
	return {"altitude": sun_alt, "azimuth": sun_az, "sun_direction": _direction(sun_alt, sun_az),
		"moon_altitude": moon_alt, "moon_direction": _direction(moon_alt, moon_az), "moon_phase": phase,
		"daylight": smoothstep(deg_to_rad(-6.0), deg_to_rad(12.0), sun_alt)}

static func _horizontal(longitude: float, latitude: float, tilt: float, sidereal: float, observer_lat: float) -> Vector2:
	var ra := atan2(sin(longitude) * cos(tilt) - tan(latitude) * sin(tilt), cos(longitude))
	var declination := asin(sin(latitude) * cos(tilt) + cos(latitude) * sin(tilt) * sin(longitude))
	var hour_angle := sidereal - ra
	var altitude := asin(clampf(sin(observer_lat) * sin(declination) + cos(observer_lat) * cos(declination) * cos(hour_angle), -1.0, 1.0))
	return Vector2(altitude, atan2(sin(hour_angle), cos(hour_angle) * sin(observer_lat) - tan(declination) * cos(observer_lat)) + PI)

static func lighting_sunset(seconds: float, config: Dictionary) -> int:
	if config.get("celestial_mode", "simple") != "astronomical": return int(config.get("sunset_minutes",1080))
	var day := floorf(seconds/86400.0)*86400.0
	var before: float = celestial(day,config).altitude
	for hour in range(1,25):
		var after: float = celestial(day+hour*3600.0,config).altitude
		if before >= 0.0 and after < 0.0:
			var lower := float((hour-1)*3600)
			var upper := float(hour*3600)
			for iteration in 12:
				var middle := (lower+upper)*0.5
				if float(celestial(day+middle,config).altitude)>=0.0: lower=middle
				else: upper=middle
			return roundi(upper/60.0)%1440
		before=after
	return 1080 # Daily occupied-hours schedule during polar night.

static func design_defaults(concept: String) -> Dictionary:
	var values: Array = {"polar":["timber","polar","sparse"],"metropolis":["modern","temperate","urban"],
		"countryside":["rural","temperate","village"],"middle-eastern":["adobe","arid","urban"],
		"desert":["adobe","arid","wilderness"],"jungle":["tropical","tropical","wilderness"],
		"southeast-asian":["tropical","tropical","village"]}.get(concept,["modern","temperate","urban"])
	return {"architecture":values[0],"climate":values[1],"settlement":values[2]}

static func design_at(profile: Dictionary, point: Vector2) -> Dictionary:
	var design := design_defaults(str(profile.get("concept","metropolis")))
	for key: String in profile:
		if key not in ["architecture","climate","settlement"] or not str(profile[key]).is_empty(): design[key]=profile[key]
	for region: Dictionary in profile.get("regions", []):
		var polygon := PackedVector2Array()
		for vertex: Array in region.polygon: polygon.append(Vector2(vertex[0], vertex[1]) / 100.0)
		if Geometry2D.is_point_in_polygon(point,polygon):
			design.merge(design_defaults(str(region.concept)),true)
			for key: String in region:
				if key not in ["architecture","climate","settlement"] or not str(region[key]).is_empty(): design[key]=region[key]
			break
	return design
