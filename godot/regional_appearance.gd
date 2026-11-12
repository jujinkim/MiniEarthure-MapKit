extends RefCounted
## Shared catalogue lookup and display-only native regional batches.
const CATALOG := preload("./regional_catalog.json")
const GROUNDS := ["asphalt","concrete","pavers","stone_paving","gravel","compacted_earth","sand","rock","managed_lawn","grassland","scrub","forest_floor","cropland","orchard","wetland","snow","ice"]
static func entry(key: String) -> Dictionary:
	var p := key.split("/")
	if p[0] == "ground" and p.size() == 2: return CATALOG.data.ground.get(p[1], {})
	if p[0] == "road" and p.size() == 3: return CATALOG.data.ground.get(p[2], {})
	if p[0] == "building" and p.size() in [4,6]: return CATALOG.data.materials.get(p[3], {})
	if p[0] == "barrier" and p.size() == 3: return CATALOG.data.materials.get(p[2], {})
	if p[0] in ["material","roof"] and p.size() == 2: return CATALOG.data.materials.get(p[1], {})
	return {}
## Natural cover whose steep faces may show bare rock (mapkit-core appearance::vegetated_ground).
const VEGETATED := ["managed_lawn","grassland","scrub","forest_floor","cropland","orchard","wetland"]
## Mean sRGB of a style. `design` (environment_profile.gd ground_design) selects
## the catalogue's terrain_ground, then climate_ground palette for ground and
## road styles, as mapkit-core appearance::ground_color_for does at distance.
static func color(key: String, design: Dictionary = {}) -> Color:
	var record := entry(key)
	var rgb: Array = record.get("roof_color" if key.begins_with("roof/") else "color", [128,128,128])
	if not design.is_empty() and not record.is_empty() and (key.begins_with("ground/") or key.begins_with("road/")):
		var kind := key.get_slice("/",key.get_slice_count("/")-1)
		var climate: Dictionary = CATALOG.data.get("climate_ground",{}).get(str(design.get("climate","")),{})
		var terrain: Dictionary = CATALOG.data.get("terrain_ground",{}).get(str(design.get("terrain","")),{})
		rgb = terrain.get(kind,climate.get(kind,rgb))
	return Color(float(rgb[0])/255.0,float(rgb[1])/255.0,float(rgb[2])/255.0)
## Bare rock on steep vegetated cover (shader cover_slope_weight/slope_rock_color,
## near ground and distant role -3) from the catalogue's terrain_slope_rock; a
## terrain without an entry binds weight 0 and keeps the cover's colour.
static func bind_slope_rock(material: ShaderMaterial, design: Dictionary) -> void:
	var slope: Dictionary = CATALOG.data.get("terrain_slope_rock",{}).get(str(design.get("terrain","")),{})
	material.set_shader_parameter("cover_slope_weight",float(slope.get("weight_per_mille",0))/1000.0)
	if slope.has("color"):
		var rgb: Array = slope.color
		material.set_shader_parameter("slope_rock_color",Color(float(rgb[0])/255.0,float(rgb[1])/255.0,float(rgb[2])/255.0))
static func facade_kind(key: String) -> int:
	var p := key.split("/")
	return int(CATALOG.data.building_types[p[2]].facade_kind) if p[0] == "building" and p.size() in [4,6] else 0
static func bind(material: ShaderMaterial, key: String, design: Dictionary = {}) -> void:
	var record := entry(key)
	material.set_shader_parameter("roughness",float(record.get("roughness_per_mille",900))/1000.0)
	material.set_shader_parameter("metallic",float(record.get("metallic_per_mille",0))/1000.0)
	if key.begins_with("ground/"):
		material.set_shader_parameter("regional_ground",GROUNDS.find(key.get_slice("/",1))+1)
		if key == "ground/ice": material.set_shader_parameter("roughness",0.22)
		elif key == "ground/wetland": material.set_shader_parameter("roughness",0.7)
		if key.get_slice("/",1) in VEGETATED:
			bind_slope_rock(material,design)
			# Arid cover is dry ground: no green lawn patches or clover.
			if str(design.get("climate","")) == "arid": material.set_shader_parameter("lawn",false)
	if not key.begins_with("building/"): return
	var architecture := key.get_slice("/",1)
	material.set_shader_parameter("regional_arch",["venetian","middle_eastern","european_stone","korean_traditional","japanese_traditional","chinese_courtyard","cycladic","polar_station"].find(architecture)+1)
	material.set_shader_parameter("regional_trim",color("material/blue_tile"))
	material.set_shader_parameter("facade_palette",false)
	material.set_shader_parameter("facade_finish_count",0)
static func batches(p: Dictionary, cancelled: Callable = Callable()) -> Array:
	var keys: PackedStringArray = p.get("regional_keys",PackedStringArray())
	var vertices: PackedVector3Array = p.get("regional_vertices",PackedVector3Array())
	var normals: PackedVector3Array = p.get("regional_normals",PackedVector3Array())
	var seeds: PackedVector2Array = p.get("regional_seeds",PackedVector2Array())
	var tops: PackedFloat32Array = p.get("regional_tops",PackedFloat32Array())
	var groups := {}
	for i in keys.size():
		if i % 256 == 0 and cancelled.is_valid() and cancelled.call(): return []
		var point := (vertices[i*3]+vertices[i*3+1]+vertices[i*3+2])/3.0
		var key := keys[i]+"|%d,%d,%d" % [floori(point.x/32.),floori(point.y/32.),floori(point.z/32.)]
		if not groups.has(key): groups[key] = PackedInt32Array()
		groups[key].append(i)
	var result: Array = []
	for key: String in groups:
		var indices: PackedInt32Array = groups[key]
		for first in range(0,indices.size(),256):
			if cancelled.is_valid() and cancelled.call(): return []
			var batch := {"key":key.get_slice("|",0),"vertices":PackedVector3Array(),"normals":PackedVector3Array(),"uv":PackedVector2Array(),"uv2":PackedVector2Array(),"custom0":PackedFloat32Array()}
			var facade := key.begins_with("building/")
			for i in range(first,mini(first+256,indices.size())):
				for c in 3:
					var index := indices[i]*3+c
					batch.vertices.append(vertices[index]); batch.normals.append(normals[index])
					if facade: batch.uv2.append(seeds[index]); batch.custom0.append(tops[index])
					else: batch.uv.append(Vector2(vertices[index].x,vertices[index].z))
			result.append(batch)
	return result

static func required_tiles(chunk: Dictionary) -> Array:
	var kinds := {}
	var p: Dictionary = chunk.get("presentation",{})
	var data := preload("./chunk_data.gd")
	for i in data.count(chunk):
		var key: String = data.material_key(chunk,i)
		var styles: Dictionary = p.get("appearance_styles",{})
		key = str(styles.get(data.object_id(chunk,i),key))
		var roads: PackedStringArray = p.get("road_materials",PackedStringArray())
		if i < roads.size() and not roads[i].is_empty(): key = roads[i]
		var record := entry(key)
		if not record.is_empty(): kinds[str(record.detail)] = true
		elif key.begins_with("road:"): kinds[str(p.get("road_styles",{}).get(key,{}).get("detail_kind","asphalt"))] = true
		else:
			var kind: String = {"dirt":"earth","gravel":"stone"}.get(key,key.get_slice(":",0))
			if kind in preload("./environment_materials.gd").TILE_KINDS: kinds[kind] = true
			if key.contains(":") and kind in ["brick","wood","concrete"]:
				for layer: String in preload("./environment_materials.gd").FACADE_LAYERS: kinds["facade:"+layer] = true
	for key: String in p.get("regional_keys",PackedStringArray()): kinds[str(entry(key).detail)] = true
	if not chunk.get("objects",[]).is_empty():
		for kind in ["wood","metal","stone","leaf"]: kinds[kind] = true
	if not p.get("assets",{}).is_empty():
		for kind: String in preload("./environment_materials.gd").TILE_KINDS: kinds[kind] = true
	return kinds.keys()

static func batch_count(p: Dictionary) -> int:
	var groups := {}
	var keys: PackedStringArray = p.get("regional_keys",PackedStringArray())
	var vertices: PackedVector3Array = p.get("regional_vertices",PackedVector3Array())
	for i in keys.size():
		var point := (vertices[i*3]+vertices[i*3+1]+vertices[i*3+2])/3.0
		var key := keys[i]+"|%d,%d,%d" % [floori(point.x/32.),floori(point.y/32.),floori(point.z/32.)]
		groups[key] = int(groups.get(key,0))+1
	var count := 0
	for size: int in groups.values(): count += ceili(float(size)/256.)
	return count
