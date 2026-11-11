extends RefCounted
## Display only. Call advance() per admitted batch; the owner controls frame budgets.
const ASSETS := preload("./asset_library.gd")
const APPEARANCE := preload("./regional_appearance.gd")
const DATA := preload("./chunk_data.gd")
const COLORS := {"asphalt": Color("30343b"), "concrete": Color("b7b8b0"),
	"dirt": Color("927456"), "gravel": Color("888477"), "grass": Color("738664")}
const BUILDING_BASE := {"brick": Color("b46c50"), "wood": Color("997347"), "concrete": Color("b7b8b0")}
const BUILDING_TINT := {"residential": Color("e8d6bd"), "commercial": Color("9fb4c4"), "industrial": Color("8b8f93"), "public": Color("d9c89a")}
const URBAN_SURFACE := preload("./urban_surface.gdshader")
const FACADE_PALETTE := preload("./facade_palette.json")
const ENVIRONMENT_LAYERS: Array = preload("./environment_materials.gd").FACADE_LAYERS
const PLAN := preload("./render_memory.gd")
const QUALITY := preload("./display_quality.gd")
const INSTANCES := preload("./render_instances.gd")
const VEGETATION := preload("./vegetation_meshes.gd")
const GRASS := preload("./grass_scatter.gd")
const GRASS_FIELD := preload("./grass_field.gd")
## Street-life walkers and pigeons (presentation walk lanes and ambience,
## walk_lanes.rs): registered with the cache's street_life_field.gd last.
const STREET_LIFE := preload("./street_life_field.gd")
const FURNITURE := preload("./street_furniture_meshes.gd")
const SIGNALS := preload("./street_signal_meshes.gd")
## Subway entrances and station signs: transit_meshes.gd groups (TRANSIT.KINDS).
const TRANSIT := preload("./transit_meshes.gd")
## Road signs and overhead guide signs: road_sign_meshes.gd groups (ROAD_SIGNS.KINDS).
const ROAD_SIGNS := preload("./road_sign_meshes.gd")
## Street trees and flower beds: street_planting_meshes.gd groups (STREET.IDS).
const STREET := preload("./street_planting_meshes.gd")
## Facade and rooftop fixtures of generated buildings (presentation
## facade_fixtures, facade_fixtures.rs and roof_fixtures.rs): blade signs and
## air-conditioner units on walls, penthouses, tanks, condensers, rims, solar
## rows and masts on flat roofs; facade_fixture_meshes.gd groups after the objects.
const FIXTURES := preload("./facade_fixture_meshes.gd")
## City fences, walls and hedges (presentation barrier_runs and
## barrier_posts, barrier_details.rs): barrier_detail_meshes.gd groups after
## the objects, drawn inside the collision prisms whose faces are hidden.
const BARRIERS := preload("./barrier_detail_meshes.gd")
## City streetlights (builtin:city_streetlight_*): city_lamp_meshes.gd groups
## (CITY_LAMPS.IDS) and the presentation's night lamp_map on road and
## sidewalk materials.
const CITY_LAMPS := preload("./city_lamp_meshes.gd")
const TRIANGLES_PER_BATCH := PLAN.TRIANGLES_PER_BATCH
# Triple the lit ground width at the same mounting height (tan of the cone angle).
const STREETLIGHT_SPREAD := 3.0
const STREETLIGHT_ANGLE := rad_to_deg(atan(tan(deg_to_rad(48.0)) * STREETLIGHT_SPREAD))
static var _advance_usec := 0
static var _release_usec := 0
static var _steps := 0
static var _phase_usec: Dictionary = {}
static var _phase_peak_usec: Dictionary = {}

const BUILTINS := ["builtin:tree", "builtin:fence", "builtin:streetlight", "builtin:signal_h", "builtin:signal_v",
	"builtin:bus_shelter", "builtin:subway_entrance", "builtin:station_sign", "builtin:bench", "builtin:shrub", "builtin:city_tree",
	"builtin:sign_speed_30", "builtin:sign_speed_40", "builtin:sign_speed_50", "builtin:sign_speed_60", "builtin:sign_speed_80",
	"builtin:sign_speed_100", "builtin:sign_crossing", "builtin:sign_stop", "builtin:sign_yield", "builtin:sign_children", "builtin:guide_sign",
	"builtin:street_tree_ginkgo", "builtin:street_tree_zelkova", "builtin:street_tree_plane", "builtin:street_tree_cherry", "builtin:street_tree_pine",
	"builtin:planter", "builtin:planter_square", "builtin:median_bed", "builtin:median_bed_pine",
	"builtin:city_streetlight_75", "builtin:city_streetlight_135", "builtin:city_streetlight_195", "builtin:city_streetlight_255",
	"builtin:city_streetlight_315", "builtin:city_streetlight_375",
	"builtin:city_streetlight_twin_75", "builtin:city_streetlight_twin_135"]
## City trees (three street variants over the 140 cm collision trunk, scaled
## 0.85–1.15, or three wide park variants for zone footprints of 160–200 cm)
## and shrubs (three sizes) are display-only vegetation_meshes.gd meshes chosen
## and turned by the object identity; a cell instances them per mesh set,
## variant and 32 m group with near (cards, branches) and far (coarse lobes)
## MultiMeshes.
const VEGETATION_IDS := {"builtin:city_tree": "tree", "builtin:shrub": "shrub"}
const VEGETATION_KEY := "builtin:vegetation"
const VEGETATION_GROUP_METRES := 32.0
## Generated details the renderer draws as display-only instance groups
## (collision stays the generated boxes): city trees and shrubs (VEGETATION_IDS, vegetation groups),
## streetlights and benches (street_furniture_meshes.gd, FURNITURE.KINDS),
## traffic signals and bus shelters (street_signal_meshes.gd, SIGNALS.KINDS),
## subway entrances and station signs (transit_meshes.gd, TRANSIT.KINDS),
## road signs and guide signs (road_sign_meshes.gd, ROAD_SIGNS.KINDS),
## street trees and flower beds (street_planting_meshes.gd, STREET.IDS),
## city streetlights (city_lamp_meshes.gd, CITY_LAMPS.IDS);
## generated barrier prisms likewise draw as fences, walls and hedges
## (barrier_detail_meshes.gd, BARRIERS) from the native presentation lists.
## `_street_detail` builds one city tree, shrub, street tree or planter as
## single nodes for the lease and placement checks of
## tests/street_detail_lease_validator.gd only.
const STREET_DETAILS := {"builtin:shrub": true, "builtin:city_tree": true, "builtin:street_tree_ginkgo": true, "builtin:street_tree_zelkova": true,
	"builtin:street_tree_plane": true, "builtin:street_tree_cherry": true, "builtin:street_tree_pine": true, "builtin:planter": true, "builtin:median_bed_pine": true}

static func _street_detail(id: String, object: Dictionary, radius_cm := 0) -> Node3D:
	var anchor := Node3D.new()
	anchor.set_meta("mapkit_asset_id", id)
	anchor.set_meta("mapkit_object_id", str(object.id))
	anchor.position = scene_position(object.position)
	anchor.rotation.y = float(int(object.quarter_turns)) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0)
	if STREET.IDS.has(id):
		# Street trees and flower beds: their near meshes as single nodes.
		var placed := object.duplicate()
		placed.asset_id = id
		for node: MeshInstance3D in STREET.detail_nodes(placed, anchor.transform): anchor.add_child(node)
		return anchor
	match id:
		_:
			# Single-node city tree or shrub (validator helper; cells instance
			# them): the near body and cards, the instance path's placement and
			# ranges, and the material's default leaf tint.
			var placement := vegetation_placement(object, id, radius_cm)
			var meshes := VEGETATION.shared()
			for part in ["body", "cards"]:
				var mesh: ArrayMesh = meshes[placement.mesh + "_" + part][placement.variant]
				if mesh.get_surface_count() == 0: continue
				var plant := MeshInstance3D.new()
				plant.name = "Vegetation_" + part
				plant.mesh = mesh
				plant.transform = anchor.transform.affine_inverse() * (placement.transform as Transform3D)
				plant.set_meta("mapkit_static_detail", true)
				plant.set_meta("mapkit_decoration", true)
				plant.set_meta("mapkit_vegetation_lod", part)
				plant.set_meta("mapkit_vegetation_kind", placement.kind)
				QUALITY.apply_node(plant, QUALITY.active())
				anchor.add_child(plant)
	return anchor

## Display placement of a city tree or shrub from its object identity: kind
## (tree or shrub), mesh set (vegetation_meshes PLANTS), variant, scene
## transform (origin at the trunk foot) and the instance data (leaf tint and
## wind phase). A city tree's crown fits its placement footprint `radius_cm`
## (0: CITY_TREE_RADIUS 120 cm; vegetation_meshes.tree_set) and its variant
## (and so its leaf colour family) is `city_tree_variant` (stands); a shrub's
## variant, and every scale, yaw and per-tree leaf hue/brightness jitter, follow
## the identity alone. Collision is the generated trunk box only.
static func vegetation_placement(object: Dictionary, id := "", radius_cm := 0, style := "") -> Dictionary:
	var pick := _identity_unit(object.id)
	var tree := (id if not id.is_empty() else str(object.get("asset_id", "builtin:city_tree"))) == "builtin:city_tree"
	var variant := city_tree_variant(str(object.id), object.position) if tree else int(pick * 3.0) % 3
	var set := VEGETATION.tree_set(radius_cm if radius_cm > 0 else 120) if tree else {"plant": "shrub", "factor": 1.0, "span": VEGETATION.SHRUB_SCALE}
	var regional := VEGETATION.regional_set(style,tree,radius_cm if radius_cm > 0 else 120)
	if not regional.is_empty(): set = regional
	var span: Vector2 = set.span
	var scale: float = float(set.factor) * (span.x + (span.y - span.x) * fmod(pick * (7.0 if tree else 11.0), 1.0))
	var basis := Basis(Vector3.UP, TAU * fmod(pick * (13.0 if tree else 5.0), 1.0)).scaled(Vector3.ONE * scale)
	var leaves: String = (VEGETATION.TREE_LEAVES if tree else VEGETATION.SHRUB_LEAVES)[variant]
	var custom := VEGETATION.leaf_custom(leaves,pick)
	if not regional.is_empty():
		var rgb: Array = VEGETATION.REGIONAL.data[set.plant].color
		custom = Color(float(rgb[0])/255.0,float(rgb[1])/255.0,float(rgb[2])/255.0).srgb_to_linear();custom.a = 1.0+fmod(pick*29.0,1.0)*0.999
	return {"kind": "tree" if tree else "shrub", "mesh": set.plant, "variant": variant, "transform": Transform3D(basis, scene_position(object.position)),
		"custom": custom}

## Canopy variant of a city tree, integer-only so the distant proxy
## (mapkit-package distant.rs `city_tree_variant`) always matches: a tree whose
## identity byte sha256(id)[6:8] is at least CITY_TREE_OWN_FROM (90 of 256, about
## 35%) takes its own sha256(id)[0:6] % 3; the others take their stand's draw,
## sha256("stand|sx|sz")[0:6] % 3 of the CITY_TREE_STAND_CM square (map cm,
## floored, from the object's map position) they stand in.
const CITY_TREE_STAND_CM := 2000
const CITY_TREE_OWN_FROM := 166
static func city_tree_variant(object_id: String, position: Array) -> int:
	var own := object_id.sha256_text()
	if own.substr(6, 2).hex_to_int() >= CITY_TREE_OWN_FROM: return own.substr(0, 6).hex_to_int() % 3
	var sx := floori(float(position[0]) / float(CITY_TREE_STAND_CM))
	var sz := floori(float(position[2]) / float(CITY_TREE_STAND_CM))
	return ("stand|%d|%d" % [sx, sz]).sha256_text().substr(0, 6).hex_to_int() % 3

## Placement footprint radius (cm) of a cell's city tree: the native
## presentation lists zone trees wider than 120 cm (`city_tree_radii`, object
## id -> radius); every other city tree, and any chunk without that list, 120.
static func city_tree_radius(chunk: Dictionary, object: Dictionary) -> int:
	var radii: Variant = chunk.get("presentation", {}).get("city_tree_radii", {})
	return int(radii.get(str(object.id), 120)) if radii is Dictionary else 120

## Mesh set, variant and scene transform of a city tree (shared with `_street_detail`).
static func city_canopy_transform(object: Dictionary, radius_cm := 0) -> Dictionary:
	var placement := vegetation_placement(object, "builtin:city_tree", radius_cm)
	return {"mesh": placement.mesh, "variant": placement.variant, "transform": placement.transform}

## One admitted step per city tree or shrub: its instance joins the cell's
## group of its mesh set, variant and 32 m square (body, cards and far
## MultiMeshes per group, attached one group per step after the object pass).
static func _plan_vegetation(job: Dictionary, object: Dictionary) -> void:
	var tree := str(object.get("asset_id", "")) == "builtin:city_tree"
	var placement := vegetation_placement(object, "", city_tree_radius(job.get("chunk", {}), object) if tree else 0,str(job.get("chunk",{}).get("presentation",{}).get("vegetation_styles",{}).get(str(object.id),"")))
	var transform: Transform3D = placement.transform
	var origin := transform.origin
	var key := "%s/%d/%d,%d" % [placement.mesh, placement.variant, floori(origin.x / VEGETATION_GROUP_METRES), floori(origin.z / VEGETATION_GROUP_METRES)]
	if not job.has("vegetation_plan"): job.vegetation_plan = {}
	if not job.vegetation_plan.has(key): job.vegetation_plan[key] = {"kind": placement.kind, "mesh": placement.mesh, "variant": placement.variant, "parts": []}
	var b := transform.basis
	var c: Color = placement.custom
	job.vegetation_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, origin.x, b.x.y, b.y.y, b.z.y, origin.y, b.x.z, b.y.z, b.z.z, origin.z, c.r, c.g, c.b, c.a]))

## Shared vegetation meshes for this job: 1 ready, 0 another step is needed,
## -1 refused (the cell fails).
static func _vegetation_ready(job: Dictionary) -> int:
	if job.has("vegetation"): return 1
	job.last_phase = "vegetation_preparation"
	if job.get("resources") == null:
		# No cache (standalone previews, older recipes): the process-lifetime
		# set, one part per step like the cached build.
		var meshes := VEGETATION.step_shared()
		if not meshes.is_empty(): job.vegetation = meshes
		return 0
	if not job.claims.has(VEGETATION_KEY):
		var key: String = job.resources.claim_vegetation()
		if key.is_empty():
			fail(job, "E_MEMORY_BUDGET", "Shared vegetation exceeds memory allowance")
			return -1
		job.claims[VEGETATION_KEY] = key
	if job.resources.prepare_vegetation(job.claims[VEGETATION_KEY]):
		var meshes: Dictionary = job.resources.vegetation(job.claims[VEGETATION_KEY])
		if meshes.is_empty():
			fail(job, "E_RENDER_ASSET", "Shared vegetation is unavailable")
			return -1
		job.vegetation = meshes
	return 0

## One MultiMesh of shared vegetation; display quality sets its range and
## shadows (`mapkit_vegetation_lod`: body, cards or far). The cell lease
## carries the node and instance buffer only: meshes and materials belong to
## the shared set. A mesh without surfaces (no leaf atlas) adds nothing.
static func _vegetation_node(job: Dictionary, mesh: Mesh, buffer: PackedFloat32Array, lod: String, kind: String, quality: Dictionary) -> void:
	if mesh.get_surface_count() == 0: return
	var multi := MultiMesh.new()
	multi.transform_format = MultiMesh.TRANSFORM_3D
	multi.use_custom_data = true
	multi.mesh = mesh
	multi.instance_count = buffer.size() / VEGETATION.FLOATS
	multi.buffer = buffer
	var node := MultiMeshInstance3D.new()
	node.name = "Vegetation_%s_%s_%d" % [kind, lod, job.root.get_child_count()]
	node.multimesh = multi
	# Wind moves leaves a few centimetres past the mesh bounds.
	node.extra_cull_margin = 0.3
	node.set_meta("mapkit_static_detail", true)
	node.set_meta("mapkit_decoration", true)
	node.set_meta("mapkit_vegetation_lod", lod)
	node.set_meta("mapkit_vegetation_kind", kind)
	QUALITY.apply_node(node, quality)
	if job.get("lease") != null:
		job.lease.track(node)
		job.lease.track(multi)
	job.root.add_child(node)

static func _attach_vegetation_group(job: Dictionary) -> void:
	var key: String = job.vegetation_plan.keys()[0]
	var group: Dictionary = job.vegetation_plan[key]
	job.vegetation_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	var meshes: Dictionary = job.vegetation
	for part in VEGETATION.KINDS: _vegetation_node(job, meshes[group.mesh + "_" + part][group.variant], buffer, part, group.kind, quality)

## Grass ground after the objects: index the grass batches in bounded steps,
## then register them with the camera-local grass field (`grass_cell`,
## released by cancel). A refused field skips grass. Returns whether this
## call did work.
static func _grass_step(job: Dictionary) -> bool:
	if job.get("grass_done", false): return false
	if not job.has("grass"):
		var parts: Array = job.get("grass_parts", [])
		if parts.is_empty() or int(job.get("grass_level", 0)) <= 0:
			job.grass_done = true
			job.erase("grass_parts")
			return false
		job.last_phase = "grass_index"
		job.grass = GRASS.begin(parts)
		job.erase("grass_parts")
		return true
	job.last_phase = "grass_index"
	if not GRASS.advance(job.grass): return true
	job.grass_done = true
	var record := GRASS.record(job.grass)
	job.erase("grass")
	var cell: RefCounted = GRASS_FIELD.register(record, job.root, job.get("resources"), job.grass_tint, job.materials.get("grass")) if not record.is_empty() else null
	if cell != null:
		job.grass_cell = cell
		if job.get("lease") != null: job.lease.track(cell)
	return true

## Street life after the grass: register the cell's walk lanes, ambience and
## pigeon anchors with the cache's street-life field (`street_life_cell`,
## released by cancel), whatever the live switch (the field gates spawning
## and drawing). Without a cache or on low nothing registers. Returns whether
## this call did work.
static func _street_life_step(job: Dictionary) -> bool:
	if job.get("street_life_done", false): return false
	job.street_life_done = true
	if job.get("resources") == null: return false
	var record := STREET_LIFE.record(job.chunk)
	if record.is_empty(): return false
	job.last_phase = "street_life"
	var cell: RefCounted = STREET_LIFE.register(record, job.root, job.resources)
	if cell != null:
		job.street_life_cell = cell
		if job.get("lease") != null: job.lease.track(cell)
	return true

## Stable display-only value in [0, 1) from an object identity.
static func _identity_unit(id: Variant) -> float:
	return float(str(id).sha256_text().substr(0, 6).hex_to_int()) / 16777216.0

static func scene_position(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), -float(value[2])) * 0.01

## The caller owns admission policy. A lease implements track(Object) and seal().
## No game dependency: small standalone editor previews may omit admission.

static func begin(chunk: Dictionary, parent: Node3D, reserve: Callable = Callable(), planned_bytes: int = 0, resources: RefCounted = null) -> Dictionary:
	var view := DATA.view(chunk)
	var shared := PLAN.shared_bytes(view) if resources != null else 0
	# Grass blades follow the grass field tier at attachment (display_quality
	# grass_level: the cache texture tier); low and polar ground keep none.
	# The worker estimate (no display tier) holds the cell's grass reservation
	# (render_memory.grass_bytes); a cell registering no grass returns it when
	# its prepared batches count the grass triangles (an unbatched cell keeps
	# it: counting would need a main-thread triangle pass here). Medium and
	# high keep the whole estimate: the Client pins a cell's lease to it there.
	var climate: Variant = JSON.parse_string(str(view.get("presentation", {}).get("environment_json", "{}")))
	var grass_level := QUALITY.grass_level(QUALITY.active()) if not (climate is Dictionary and str(climate.get("climate", "")) == "polar") else 0
	if grass_level == 0:
		var unused := PLAN.grass_bytes(PLAN.batch_grass_triangles(view))
		if unused > 0 and planned_bytes - shared >= unused + 65536: planned_bytes -= unused
	# Older validated recipes have no per-asset immutable sharing metadata.
	# Keep their complete per-job reservation and local importer ownership.
	if resources != null and shared == 0 and not view.get("presentation", {}).get("assets", {}).is_empty():
		resources = null
	# Street life registers only with a cache from medium (street_life_field):
	# otherwise the cell returns its graph reservation (render_memory.street_life_bytes).
	if resources == null or int(resources.quality_profile.get("level", 1)) <= 0:
		var idle := PLAN.street_life_bytes(view)
		if idle > 0 and planned_bytes - shared >= idle + 65536: planned_bytes -= idle
	var lease: RefCounted
	if reserve.is_valid():
		var bytes := planned_bytes - shared
		if bytes > 0 and bytes <= 64 * 1024 * 1024 * 1024: lease = reserve.call(bytes)
		if lease == null:
			return {"root": null, "chunk": {}, "done": true, "cancelled": true,
				"materials": {}, "asset_materials": {}, "templates": {}, "lease": null,
				"error": {"code": "E_MEMORY_BUDGET", "message": "Display allocation exceeds memory allowance"}}
	var root := Node3D.new()
	root.set_meta("mapkit_render_root", true)
	root.add_to_group("mapkit_quality_targets")
	if lease != null: lease.track(root)
	root.name = "MapCell_%s_%s" % [chunk.cell.x, chunk.cell.y]
	parent.add_child(root)
	var job := {"root": root, "chunk": view, "lease": lease, "grass_parts": [], "display_lease": lease, "grind": 0, "grind_records": preload("./grind_geometry.gd").records(view), "triangle": 0, "water": 0, "water_face": 0, "water_records": preload("./water_query.gd").records(view), "object": 0, "done": false, "cancelled": false, "materials": {}, "asset_materials": {}, "templates": {}, "error": {}, "resources": resources, "claims": {}, "instances": {}, "counts": {}, "borrowed_materials": {}, "steps": 0, "peak_step_usec": 0}
	job.grass_level = grass_level
	job.grass_tint = ground_color("grass", view.get("presentation", {})).srgb_to_linear() * Color(0.92, 1.0, 0.86)
	if resources != null:
		resources.environment_profile = preload("./environment_profile.gd").defaults()
		var environment_json := str(view.get("presentation",{}).get("environment_json",""))
		if not environment_json.is_empty():
			var profile: Variant = JSON.parse_string(environment_json)
			if profile is Dictionary: resources.environment_profile = profile
		var sources: Dictionary = view.get("presentation", {}).get("assets", {})
		for id: String in sources:
			var key: String = resources.claim(id, sources)
			if key.is_empty():
				fail(job, "E_MEMORY_BUDGET", "Shared display resources exceed memory allowance: %s (%d bytes cached)" % [id,resources.bytes()])
				return job
			job.claims[id] = key
		if not resources.require_environment_tiles(APPEARANCE.required_tiles(view)) or not resources.begin_environment():
			fail(job, "E_MEMORY_BUDGET", "Shared material tiles exceed memory allowance")
			return job
		var lamp_assets := {}
		var has_light_bindings: bool = not resources.environment_profile.get("lights",[]).is_empty()
		for object: Dictionary in view.objects:
			var id := str(object.asset_id)
			if has_light_bindings: lamp_assets[id] = true
			if id == "builtin:tree" and not job.claims.has(id):
				var key: String = resources.claim_tree_canopy()
				if key.is_empty():
					fail(job,"E_MEMORY_BUDGET","Shared tree canopy exceeds memory allowance")
					return job
				job.claims[id] = key
			if not id.begins_with("builtin:") and str(sources.get(id,{}).get("path","")).ends_with(".glb"):
				var group := _object_group(object)
				job.counts[group] = int(job.counts.get(group, 0)) + 1
		job.lamp_bindings = _lamp_bindings(resources.environment_profile,lamp_assets)
		# Street furniture claims its shared set before any upload.
		if not FURNITURE.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared street furniture exceeds memory allowance")
			return job
		# Traffic signals and bus shelters likewise.
		if not SIGNALS.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared signals and shelters exceed memory allowance")
			return job
		# Subway entrances and station signs likewise.
		if not TRANSIT.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared transit details exceed memory allowance")
			return job
		# Road signs likewise.
		if not ROAD_SIGNS.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared road signs exceed memory allowance")
			return job
		# Street trees and flower beds likewise.
		if not STREET.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared street planting exceeds memory allowance")
			return job
		# City streetlights likewise.
		if not CITY_LAMPS.claim(job, view.objects):
			fail(job,"E_MEMORY_BUDGET","Shared city streetlights exceed memory allowance")
			return job
		# City barrier detail likewise; a refused claim fails the cell (the
		# prism faces are hidden).
		if not BARRIERS.claim(job):
			fail(job,"E_MEMORY_BUDGET","Shared barrier details exceed memory allowance")
			return job
		# Facade fixtures likewise; a refused claim only skips them.
		FIXTURES.claim(job)
	return job

static func advance(job: Dictionary) -> bool:
	var started := Time.get_ticks_usec()
	job.last_phase = "retirement"
	var done := _advance(job)
	var phase: String = job.last_phase
	var elapsed := Time.get_ticks_usec() - started
	_phase_usec[phase] = int(_phase_usec.get(phase,0)) + elapsed
	_phase_peak_usec[phase] = maxi(int(_phase_peak_usec.get(phase,0)),elapsed)
	_advance_usec += elapsed
	_steps += 1
	job.steps = int(job.get("steps", 0)) + 1
	job.peak_step_usec = maxi(int(job.get("peak_step_usec", 0)), Time.get_ticks_usec() - started)
	return done

static func _advance(job: Dictionary) -> bool:
	if job.done or job.cancelled:
		return true
	if not is_instance_valid(job.root) or job.root.is_queued_for_deletion():
		job.cancelled = true
		dispose(job)
		return true
	if job.get("resources") != null and not job.get("environment_ready",false):
		job.last_phase = "environment_preparation"
		if not job.resources.prepare_environment(): return false
		if job.resources.environment_context() == null: return fail(job,"E_RENDER_ASSET","Shared material preparation failed")
		job.environment_ready = true
		return false
	if int(job.grind) < job.grind_records.size():
		job.last_phase = "special_upload"
		job.root.add_child(preload("./grind_geometry.gd").visual(job.grind_records[job.grind], job.lease, JSON.parse_string(job.chunk.get("presentation",{}).get("cell_bounds_json","{}"))))
		job.grind += 1
		return false
	var chunk: Dictionary = job.chunk
	var presentation: Dictionary = chunk.get("presentation", {})
	var sources: Dictionary = presentation.get("assets", {})
	job.last_phase = "mesh_upload"
	var offset := int(job.triangle)
	if chunk.has("render_batches"):
		if offset < chunk.render_batches.size(): return _prepared_batch(job, chunk.render_batches[offset])
		offset = DATA.count(chunk)
	if offset < DATA.count(chunk):
		var skipped := 0
		if not job.has("hidden_objects"): job.hidden_objects = PLAN.hidden_objects(chunk)
		while offset < DATA.count(chunk) and DATA.object_id(chunk, offset) in job.hidden_objects and skipped < TRIANGLES_PER_BATCH:
			offset += 1
			skipped += 1
		if skipped > 0:
			job.triangle = offset
			return false
		var key := display_material_key(chunk, offset)
		var end := mini(offset + TRIANGLES_PER_BATCH, DATA.count(chunk))
		var first := offset
		while offset < end and display_material_key(chunk, offset) == key and not DATA.object_id(chunk, offset) in job.hidden_objects:
			offset += 1
		var mesh := MeshInstance3D.new()
		if chunk.has("scene_vertices"):
			var arrays := []
			arrays.resize(Mesh.ARRAY_MAX)
			arrays[Mesh.ARRAY_VERTEX] = chunk.scene_vertices.slice(first * 3, offset * 3)
			arrays[Mesh.ARRAY_NORMAL] = chunk.scene_normals.slice(first * 3, offset * 3)
			# Same attributes as worker-prepared batches (render_memory.facade_arrays).
			var facade: int = PLAN.facade_arrays(key) if chunk.get("facade_seeds", PackedVector2Array()).size() == chunk.wall_uv.size() else 0
			if facade == 0: arrays[Mesh.ARRAY_TEX_UV] = chunk.wall_uv.slice(first * 3, offset * 3)
			else: arrays[Mesh.ARRAY_TEX_UV2] = chunk.facade_seeds.slice(first * 3, offset * 3)
			if facade == 1 and chunk.get("facade_tops", PackedFloat32Array()).size() == chunk.wall_uv.size(): arrays[Mesh.ARRAY_CUSTOM0] = chunk.facade_tops.slice(first * 3, offset * 3)
			if key == "concrete":
				var framed := PLAN.sidewalk_arrays(chunk, PackedInt32Array(range(first, offset)), arrays[Mesh.ARRAY_TEX_UV])
				if not framed.is_empty(): arrays[Mesh.ARRAY_TEX_UV] = framed[0]; arrays[Mesh.ARRAY_TEX_UV2] = framed[1]
			if key == "grass" and int(job.get("grass_level", 0)) > 0: job.grass_parts.append(arrays[Mesh.ARRAY_VERTEX])
			var prepared := ArrayMesh.new()
			prepared.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, FACADE_TOP_FORMAT if arrays[Mesh.ARRAY_CUSTOM0] != null else 0)
			mesh.mesh = prepared
			_prepare_batch_occluder(prepared, key, arrays[Mesh.ARRAY_VERTEX])
		else:
			var surface := SurfaceTool.new()
			surface.begin(Mesh.PRIMITIVE_TRIANGLES)
			for triangle in range(first, offset):
				var normal := (DATA.scene_vertex(chunk, triangle, 1) - DATA.scene_vertex(chunk, triangle, 0)).cross(DATA.scene_vertex(chunk, triangle, 2) - DATA.scene_vertex(chunk, triangle, 0)).abs()
				for index in [0, 2, 1]:
					var point := DATA.scene_vertex(chunk, triangle, index)
					var uv := Vector2(point.x, point.z)
					if normal.y < maxf(normal.x, normal.z):
						uv = Vector2(point.z, point.y) if normal.x > normal.z else Vector2(point.x, point.y)
					surface.set_uv(uv)
					surface.add_vertex(point)
			surface.generate_normals()
			mesh.mesh = surface.commit()
		if not job.materials.has(key):
			if presentation.get("urban_surfaces", false) and not job.has("urban_shader"):
				job.urban_shader = job.resources.urban_shader() if job.get("resources") != null else wet_urban_shader()
				if job.urban_shader == null:
					mesh.free()
					return fail(job, "E_MEMORY_BUDGET", "Shared surface shader exceeds memory allowance")
			var material: Material
			if key.begins_with("asset:"):
				var id := key.substr(6)
				material = job.resources.material(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.material(id, sources, job.asset_materials)
				if material != null and job.get("resources") != null: job.borrowed_materials[material.get_instance_id()] = true
				if material != null: material = _environment_surface(job, material)
			else: material = _generated_material(job, key, presentation, true)
			if material == null:
				mesh.free()
				return fail(job, "E_RENDER_ASSET", "Validated image could not be displayed")
			job.materials[key] = material
		mesh.material_override = job.materials[key]
		_wet_bank(job, mesh, key)
		mesh.set_meta("mapkit_occluder", occluding_surface(key))
		track_resources(job, mesh)
		job.root.add_child(mesh)
		job.triangle = offset
	elif not chunk.has("render_batches") and not job.get("regional_done",false):
		if not job.has("regional_batches"): job.regional_batches = APPEARANCE.batches(presentation); job.regional_cursor = 0
		if job.regional_cursor < job.regional_batches.size():
			var saved: int = job.triangle
			_prepared_batch(job,job.regional_batches[job.regional_cursor])
			job.triangle = saved
			job.regional_cursor += 1
		else: job.regional_done = true; job.erase("regional_batches")
		return false
	elif int(job.water) < job.water_records.size():
		job.last_phase = "water_upload"
		var record: Dictionary = job.water_records[job.water]
		var count := mini(TRIANGLES_PER_BATCH, record.surface.size()-int(job.water_face))
		if count > 0:
			var mesh := preload("./water_renderer.gd").mesh(record, int(job.water_face), count)
			track_resources(job, mesh)
			job.root.add_child(mesh)
		job.water_face += count
		if int(job.water_face) >= record.surface.size(): job.water += 1; job.water_face = 0
	elif int(job.object) < chunk.objects.size():
		job.last_phase = "instance_upload"
		var finish := mini(int(job.object) + 1, chunk.objects.size())
		while int(job.object) < finish:
			var object: Dictionary = chunk.objects[job.object]
			job.object += 1
			var id := str(object.asset_id)
			if not id.begins_with("builtin:"):
				if not sources.has(id): return fail(job, "E_RENDER_ASSET", "Presentation bytes are missing")
				if not str(sources[id].path).ends_with(".glb"): continue # textured proxy faces
				if not job.templates.has(id):
					job.last_phase = "template_preparation"
					if job.get("resources") != null and not job.resources.prepare_template(job.claims[id],id,sources):
						job.object -= 1
						return false
					var template: Node3D = job.resources.template(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.template(id, sources, job.asset_materials)
					if template == null: return fail(job, "E_RENDER_ASSET", "Validated GLB could not be displayed")
					if job.get("resources") == null: track_resources(job, template)
					job.templates[id] = template
					# Import and scene attachment are different budgeted steps.
					job.object -= 1
					return false
				if job.get("resources") != null:
					var group := _object_group(object)
					if not job.instances.has(group):
						job.last_phase = "multimesh_setup"
						job.instances[group] = INSTANCES.begin_parts(job.resources.instance_parts(job.claims[id]), int(job.counts.get(group, 0)), job.root, job.lease, bool(sources[id].get("solid",true)))
						job.object -= 1
						return false
					if not job.instances[group].is_empty():
						var transform := Transform3D(Basis(Vector3.UP, (float(object.quarter_turns) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0))), scene_position(object.position))
						INSTANCES.append(job.instances[group], transform, str(object.id), str(presentation.get("map_id","")))
						_environment_lamp(job,id,transform)
						continue
				var instance: Node3D = job.templates[id].duplicate(0)
				var seed_value := float((str(presentation.get("map_id",""))+"/"+str(object.id)).sha256_text().substr(0,6).hex_to_int())/16777215.0
				# One synchronous placement shares quality; refresh on the next call.
				var quality := QUALITY.active()
				var pending: Array = [instance]
				while not pending.is_empty():
					var piece: Node3D = pending.pop_back()
					if piece is MeshInstance3D:
						if bool(sources[id].get("solid",true)): piece.set_meta("mapkit_decoration",false)
						_seed_fallback_materials(piece,seed_value,job.lease)
						if piece.get_meta("mapkit_occluder",false):
							var occluder := QUALITY.occluder(piece.mesh,job.lease,quality)
							if occluder != null: piece.add_child(occluder)
					pending.append_array(piece.get_children())
				var anchor := Node3D.new()
				anchor.set_meta("mapkit_asset_id", id)
				anchor.set_meta("mapkit_object_id", str(object.id))
				anchor.position = scene_position(object.position)
				# glTF metres: x-right, y-up, z-back; local map y points forward.
				anchor.rotation.y = (float(object.quarter_turns) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0))
				anchor.scale = Vector3.ONE
				_environment_lamp(job,id,anchor.transform)
				QUALITY.apply_tree(instance, quality)
				anchor.add_child(instance)
				job.root.add_child(anchor)
				track_instance_nodes(job.lease, anchor)
				continue
			if id not in BUILTINS:
				return fail(job, "E_RENDER_ASSET", "Unknown built-in asset")
			if FURNITURE.KINDS.has(id):
				var furnished := FURNITURE.ready(job)
				if furnished == -3: continue
				if furnished < 0: return fail(job, "E_RENDER_ASSET", "Shared street furniture is unavailable")
				if furnished == 0:
					job.object -= 1
					return false
				FURNITURE.plan(job, object)
				continue
			if SIGNALS.KINDS.has(id):
				var signalled := SIGNALS.ready(job)
				if signalled < 0: return fail(job, "E_RENDER_ASSET", "Shared signals and shelters are unavailable")
				if signalled == 0:
					job.object -= 1
					return false
				SIGNALS.plan(job, object)
				continue
			if TRANSIT.KINDS.has(id):
				var transit := TRANSIT.ready(job)
				if transit < 0: return fail(job, "E_RENDER_ASSET", "Shared transit details are unavailable")
				if transit == 0:
					job.object -= 1
					return false
				TRANSIT.plan(job, object)
				continue
			if ROAD_SIGNS.KINDS.has(id):
				var signs := ROAD_SIGNS.ready(job)
				if signs < 0: return fail(job, "E_RENDER_ASSET", "Shared road signs are unavailable")
				if signs == 0:
					job.object -= 1
					return false
				ROAD_SIGNS.plan(job, object)
				continue
			if STREET.IDS.has(id):
				var planted := STREET.ready(job)
				if planted < 0: return fail(job, "E_RENDER_ASSET", "Shared street planting is unavailable")
				if planted == 0:
					job.object -= 1
					return false
				STREET.plan(job, object)
				continue
			if CITY_LAMPS.IDS.has(id):
				var lit := CITY_LAMPS.ready(job)
				if lit < 0: return fail(job, "E_RENDER_ASSET", "Shared city streetlights are unavailable")
				if lit == 0:
					job.object -= 1
					return false
				CITY_LAMPS.plan(job, object)
				continue
			if VEGETATION_IDS.has(id):
				var ready := _vegetation_ready(job)
				if ready < 0: return true
				if ready == 0:
					job.object -= 1
					return false
				_plan_vegetation(job, object)
				continue
			if id != "builtin:tree": continue
			var canopy_mesh: Mesh
			if job.get("resources") != null:
				if not job.get("canopy_ready",false):
					job.last_phase = "canopy_preparation"
					job.canopy_ready = job.resources.prepare_tree_canopy(job.claims[id])
					job.object -= 1
					return false
				canopy_mesh = job.resources.tree_canopy(job.claims[id])
				if canopy_mesh == null: return fail(job,"E_RENDER_ASSET","Shared tree canopy is unavailable")
			else:
				canopy_mesh = QUALITY.prepare_lods(preload("./render_resource_cache.gd").tree_canopy_source())
			var canopy := MeshInstance3D.new()
			canopy.mesh = canopy_mesh
			canopy.set_meta("mapkit_decoration",true)
			canopy.position = scene_position(object.position) + Vector3.UP * 4.8
			# A new private binding receives quality once in track_resources below,
			# in this same admitted call, before the node becomes visible.
			canopy.material_override = _leaf_material(job, true)
			track_resources(job, canopy, job.get("resources") != null)
			job.root.add_child(canopy)
	elif not job.get("furniture_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		FURNITURE.attach_group(job)
	elif not job.get("signal_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		SIGNALS.attach_group(job)
	elif not job.get("transit_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		TRANSIT.attach_group(job)
	elif not job.get("road_sign_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		ROAD_SIGNS.attach_group(job)
	elif not job.get("street_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		STREET.attach_group(job)
	elif not job.get("city_lamp_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		CITY_LAMPS.attach_group(job)
	elif not job.get("vegetation_plan", {}).is_empty():
		job.last_phase = "instance_upload"
		_attach_vegetation_group(job)
	elif BARRIERS.step(job):
		if job.get("barriers_failed", false): return fail(job, "E_RENDER_ASSET", "Shared barrier details are unavailable")
	elif FIXTURES.step(job): pass
	elif _grass_step(job): pass
	elif _street_life_step(job): pass
	else:
		job.done = true
		dispose(job)
	return job.done

static func _prepared_batch(job: Dictionary, batch: Dictionary) -> bool:
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = batch.vertices
	arrays[Mesh.ARRAY_NORMAL] = batch.normals
	if not batch.uv.is_empty(): arrays[Mesh.ARRAY_TEX_UV] = batch.uv
	if not batch.get("uv2", PackedVector2Array()).is_empty(): arrays[Mesh.ARRAY_TEX_UV2] = batch.uv2
	if not batch.get("custom0", PackedFloat32Array()).is_empty(): arrays[Mesh.ARRAY_CUSTOM0] = batch.custom0
	var mesh := MeshInstance3D.new()
	var resource := ArrayMesh.new()
	resource.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], {}, FACADE_TOP_FORMAT if arrays[Mesh.ARRAY_CUSTOM0] != null else 0)
	mesh.mesh = resource
	var key: String = batch.key
	if key == "grass" and int(job.get("grass_level", 0)) > 0: job.grass_parts.append(batch.vertices)
	_prepare_batch_occluder(resource, key, batch.vertices)
	if not job.materials.has(key):
		var presentation: Dictionary = job.chunk.get("presentation", {})
		var sources: Dictionary = presentation.get("assets", {})
		if presentation.get("urban_surfaces", false) and not job.has("urban_shader"):
			job.urban_shader = job.resources.urban_shader() if job.get("resources") != null else wet_urban_shader()
			if job.urban_shader == null:
				mesh.free()
				return fail(job, "E_MEMORY_BUDGET", "Shared surface shader exceeds memory allowance")
		var material: Material
		if key.begins_with("asset:"):
			var id := key.substr(6)
			material = job.resources.material(job.claims[id], id, sources) if job.get("resources") != null else ASSETS.material(id, sources, job.asset_materials)
			if material != null and job.get("resources") != null: job.borrowed_materials[material.get_instance_id()] = true
			if material != null: material = _environment_surface(job, material)
		else:
			material = _generated_material(job, key, presentation, true)
		if material == null:
			mesh.free()
			return fail(job, "E_RENDER_ASSET", "Validated image could not be displayed")
		job.materials[key] = material
	mesh.material_override = job.materials[key]
	_wet_bank(job, mesh, key)
	mesh.set_meta("mapkit_occluder", occluding_surface(key))
	track_resources(job, mesh)
	job.root.add_child(mesh)
	job.triangle += 1
	return false

## Earth meshes of a cell with water get its highest water surface (mesh space,
## metres) so the ground shader darkens the wet bank; real-city basins are earth.
static func _wet_bank(job: Dictionary, mesh: GeometryInstance3D, key: String) -> void:
	if key != "dirt": return
	if not job.has("shore_level"):
		var level := -100000.0
		for record: Dictionary in job.water_records:
			level = maxf(level, float(record.body.surface_cm) * .01)
		job.shore_level = level
	if float(job.shore_level) > -10000.0 and mesh.material_override is ShaderMaterial:
		# Generated dirt materials belong to this cell. A material parameter
		# avoids allocating a global instance block for every distant/near mesh.
		mesh.material_override.set_shader_parameter("shore_level_m", float(job.shore_level))

## An occluding batch derives its occluder faces from the vertex array it was
## built from: reading a just-uploaded surface back waits for the GPU (Metal,
## Vulkan) inside the attachment budget. The faces are identical; the shape is
## leased once by QUALITY.occluder() when the node is tracked.
static func _prepare_batch_occluder(mesh: ArrayMesh, key: String, vertices: PackedVector3Array) -> void:
	if occluding_surface(key): QUALITY.prepare_occluder(mesh, null, [[vertices, null]])

static func display_material_key(chunk: Dictionary, triangle: int) -> String:
	return PLAN.material_key(chunk, triangle)

## Weak references extend the charge when a trusted consumer retains a mesh,
## material or texture after its scene node is destroyed. Templates share these
## resources with duplicate(0) instances; the optional session cache owns shared leases.
static func track_instance_nodes(lease: RefCounted, node: Node) -> void:
	if lease == null: return
	lease.track(node)
	for child: Node in node.get_children(): track_instance_nodes(lease, child)

static func track_resources(job: Dictionary, node: Node, shared_mesh := false) -> void:
	# One synchronous traversal uses one snapshot; the next admitted call reads
	# current quality again. Lease tracking must not change display policy.
	_track_resources(job, node, shared_mesh, QUALITY.active())

static func _track_resources(job: Dictionary, node: Node, shared_mesh: bool, quality: Dictionary) -> void:
	QUALITY.apply_node(node, quality)
	if node is MeshInstance3D and node.get_meta("mapkit_occluder", false) and node.get_child_count() == 0 and QUALITY.node_can_occlude(node):
		var occluder := QUALITY.occluder(node.mesh, job.get("lease"), quality)
		if occluder != null: node.add_child(occluder)
	if job.get("lease") == null: return
	if node is MeshInstance3D:
		# A cached canopy or process-lifetime street detail must not keep every
		# cell's private reservation alive; the node carries the charge.
		var detail: bool = node.get_meta("mapkit_static_detail", false)
		if shared_mesh or detail: job.lease.track(node)
		else: job.lease.track(node.mesh)
		if not detail:
			track_material(job, node.material_override)
			for i in node.mesh.get_surface_count():
				track_material(job, node.mesh.surface_get_material(i))
				track_material(job, node.get_surface_override_material(i))
	for child: Node in node.get_children(): _track_resources(job, child, false, quality)

static func track_material(job: Dictionary, material: Material) -> void:
	if material == null: return
	if job.get("borrowed_materials", {}).has(material.get_instance_id()): return
	job.lease.track(material)
	if material is ShaderMaterial and job.get("resources") == null: job.lease.track(material.shader)
	if material is BaseMaterial3D:
		for slot in BaseMaterial3D.TEXTURE_MAX:
			var texture: Texture2D = material.get_texture(slot)
			if texture != null: job.lease.track(texture)

static func dispose(job: Dictionary) -> void:
	if job.get("resources") != null:
		for key: String in job.get("claims", {}).values(): job.resources.release(key)
	else:
		for template: Node in job.templates.values():
			if is_instance_valid(template): template.free()
	job.claims = {}
	job.resources = null
	job.instances = {}
	job.counts = {}
	job.borrowed_materials = {}
	job.templates = {}
	job.asset_materials = {}
	job.materials = {}
	job.chunk = {}
	job.erase("lamp_bindings")
	job.erase("urban_shader")
	job.erase("regional_batches")
	for key in ["vegetation", "vegetation_plan", "grass", "grass_parts", "furniture", "furniture_plan", "signals", "signal_plan", "transit", "transit_plan", "road_signs", "road_sign_plan", "street_planting", "street_plan", "city_lamps", "city_lamp_plan", "lamp_texture", "fixtures", "fixture_plan", "barrier_set", "barrier_plan", "barrier_cursor", "barrier_low", "barrier_owners", "barriers_done", "barriers_failed", "hidden_objects"]: job.erase(key)
	if job.get("lease") != null:
		job.lease.seal()
		job.lease = null

static func fail(job: Dictionary, code: String, message: String) -> bool:
	job.error = {"code": code, "message": message}
	cancel(job)
	return true

static func material_color(key: String) -> Color:
	if not APPEARANCE.entry(key).is_empty(): return APPEARANCE.color(key)
	if key.begins_with("builtin:"):
		return {"builtin:tree": Color("745138"), "builtin:city_tree": Color("6b4c36"), "builtin:fence": Color("9b764f"), "builtin:streetlight": Color("555e63"),
			"builtin:signal_h": Color("4a5055"), "builtin:signal_v": Color("4a5055"), "builtin:bus_shelter": Color("7aa0b8"),
			"builtin:subway_entrance": Color("3f6fa8"), "builtin:station_sign": Color("2f7fd0"),
			"builtin:guide_sign": Color("1d4f9c"), "builtin:planter": Color("aaa69e"), "builtin:planter_square": Color("aaa69e"),
			"builtin:median_bed": Color("aaa69e"), "builtin:median_bed_pine": Color("aaa69e")}.get(key, Color("8d949a") if key.begins_with("builtin:sign_") or key.begins_with("builtin:city_streetlight") else (Color("5e5248") if key.begins_with("builtin:street_tree_") else Color.GRAY))
	if key.begins_with("barrier:"):
		return {"barrier:hedge": Color("486447"), "barrier:fence": Color("9b764f"), "barrier:wall": Color("b7b8b0")}.get(key, Color.GRAY)
	if not key.contains(":"):
		return COLORS.get(key, Color.GRAY)
	var fields := key.split(":")
	var base: Color = BUILDING_BASE.get(fields[0], Color.GRAY)
	var tint: Color = BUILDING_TINT.get(fields[1], Color.WHITE)
	# Same twelve colours as the distant mesh (mapkit-package distant.rs).
	return base.lerp(tint, 0.6 if fields[0] == "concrete" else 0.3)

## Generated building walls: 1 apartment, 2 house, 3 commercial, 4 industrial, 5 public; 0 none.
static func facade_kind(key: String) -> int:
	if key.begins_with("building/"): return APPEARANCE.facade_kind(key)
	var fields := key.split(":")
	if fields.size() != 2 or not BUILDING_BASE.has(fields[0]): return 0
	match fields[1]:
		"residential": return 1 if fields[0] == "concrete" else 2
		"commercial": return 3
		"industrial": return 4
		"public": return 5
	return 0

## Facade batches carry their solids' offset top (eaves) height in CUSTOM0.x
## (packed `facade_tops`, negated on triangles rising above it): commercial
## storeys fit below it, and every kind draws its parapet coping there on walls
## that end at it. Flat roof faces carry their solid's roof frame there instead
## (4000 + the angle, read above 3500), which roof finishes run square to.
const FACADE_TOP_FORMAT := Mesh.ARRAY_CUSTOM_R_FLOAT << Mesh.ARRAY_FORMAT_CUSTOM0_SHIFT

## Floor pitch follows the authored map scale (3 m ÷ D) published by presentation.
static func bind_facade(material: Material, key: String, presentation: Dictionary) -> void:
	var kind := facade_kind(key)
	if kind == 0 or material is not ShaderMaterial: return
	material.set_shader_parameter("facade_kind", kind)
	material.set_shader_parameter("floor_pitch", float(presentation.get("facade_floor_cm", 300)) / 100.0)
	material.set_shader_parameter("lighting_role", 1)
	material.set_shader_parameter("signs_enabled", kind == 3)
	if key.begins_with("building/"):
		APPEARANCE.bind(material,key)
		return
	var palette := facade_palette(key)
	material.set_shader_parameter("facade_body", palette.body)
	material.set_shader_parameter("facade_roof", palette.roof)
	material.set_shader_parameter("facade_accent", palette.accent)
	material.set_shader_parameter("facade_own_share", palette.own_share)
	material.set_shader_parameter("facade_palette", true)
	var finishes := facade_finishes(key)
	material.set_shader_parameter("facade_finish", finishes.finish)
	material.set_shader_parameter("facade_finish_look", finishes.look)
	material.set_shader_parameter("facade_finish_count", finishes.names.size())

## Wall finishes of a generated building key (`facade_palette.json` finishes) in
## shader slots: finish (tile layer or -1, tile floors, roughness, metallic) and
## look (specular, curtain wall, cumulative weight). Curtain walls only on
## commercial keys; keys without weights (wood) have none.
static var _facade_finishes := {}
static func facade_finishes(key: String) -> Dictionary:
	if _facade_finishes.has(key): return _facade_finishes[key]
	var fields := key.split(":")
	var table: Dictionary = FACADE_PALETTE.data.get("finishes", {})
	var weights: Dictionary = table.get("weights", {}).get(fields[0] if fields[0] in ["brick", "wood"] else key, {})
	var result := {"names": PackedStringArray(), "finish": PackedVector4Array(), "look": PackedVector4Array()}
	var total := 0.0
	for name: String in weights:
		var kind: Dictionary = table.kinds[name]
		if kind.get("curtain_wall", false) and facade_kind(key) != 3: continue
		total += float(weights[name])
	var cumulative := 0.0
	for name: String in weights:
		var kind: Dictionary = table.kinds[name]
		if result.names.size() == 6 or (kind.get("curtain_wall", false) and facade_kind(key) != 3) or total <= 0.0: continue
		cumulative += float(weights[name])
		result.names.append(name)
		result.finish.append(Vector4(float(ENVIRONMENT_LAYERS.find(str(kind.get("tile", "")))), float(kind.tile_floors), float(kind.roughness), float(kind.metallic)))
		result.look.append(Vector4(float(kind.specular), 1.0 if kind.get("curtain_wall", false) else 0.0, cumulative / total, 0.0))
	result.make_read_only()
	_facade_finishes[key] = result
	return result

## Record palettes of a generated building key (sRGB 0..1), the same entries the
## distant mesh uses (`facade_palette.json`, mapkit-package distant.rs), and the
## key's own-look share (`own_share`: the key's entry, else the default).
static var _facade_palettes := {}
static func facade_palette(key: String) -> Dictionary:
	if _facade_palettes.has(key): return _facade_palettes[key]
	var fields := key.split(":")
	var table: Dictionary = FACADE_PALETTE.data
	var brick_or_wood := fields[0] in ["brick", "wood"]
	var usage: String = fields[1] if fields.size() > 1 and fields[1] in ["commercial", "industrial", "public"] else "residential"
	var unit := func(rows: Array) -> PackedVector3Array:
		var out := PackedVector3Array()
		for rgb: Array in rows: out.append(Vector3(float(rgb[0]), float(rgb[1]), float(rgb[2])) / 255.0)
		return out
	var palette := {"body": unit.call(table.body[fields[0] if brick_or_wood else "concrete:" + usage]),
		"roof": unit.call(table.roof[fields[0] if brick_or_wood else "concrete"]), "accent": unit.call(table.accent),
		"own_share": float(table.own_share.get(key, table.own_share.default))}
	palette.make_read_only()
	_facade_palettes[key] = palette
	return palette

## Authored climate changes static ground colour only, never surface traction.
static func ground_color(key: String, presentation: Dictionary) -> Color:
	if key != "grass": return material_color(key)
	var profile: Variant = JSON.parse_string(str(presentation.get("environment_json", "{}")))
	if not profile is Dictionary: return material_color(key)
	if profile.has("ground_color"):
		var tint: Array=profile.ground_color
		return Color(float(tint[0])/255.0,float(tint[1])/255.0,float(tint[2])/255.0)
	return {"polar":Color("d9e4e8"),"arid":Color("b89a6a"),"tropical":Color("526b40")}.get(str(profile.get("climate", "")),material_color(key))

static func cancel(job: Dictionary) -> void:
	var started := Time.get_ticks_usec()
	job.cancelled = true
	if job.has("grass_cell"):
		job.grass_cell.release()
		job.erase("grass_cell")
	if job.has("street_life_cell"):
		job.street_life_cell.release()
		job.erase("street_life_cell")
	dispose(job)
	if is_instance_valid(job.root) and not job.root.is_queued_for_deletion():
		var parent: Node = job.root.get_parent()
		if parent != null:
			parent.remove_child(job.root)
		job.root.queue_free()
	_release_usec += Time.get_ticks_usec() - started

static func diagnostics() -> Dictionary:
	return {"advance_usec": _advance_usec, "release_usec": _release_usec, "steps": _steps, "phase_usec":_phase_usec.duplicate(), "phase_peak_usec":_phase_peak_usec.duplicate()}

static func attach(chunk: Dictionary, parent: Node3D) -> Node3D:
	# Synchronous convenience for existing small editor previews; same renderer source.
	var job := begin(chunk, parent)
	while not advance(job):
		pass
	return job.root

static func _leaf_material(job: Dictionary, defer_quality := false) -> Material:
	var context: RefCounted = job.resources.material_context() if job.get("resources") != null else null
	if context != null:
		# Leaves keep the original source roughness, not the generated-face value.
		# The shared shader already uses the wrapper's double-sided rendering.
		return context.generated_material(Color("486447"),"leaf",0.0,1.0,defer_quality)
	var leaf := StandardMaterial3D.new()
	leaf.albedo_color = Color("486447")
	leaf.resource_name = "mk_leaf"
	return leaf

## Only fresh attachment bindings defer quality to track_resources in the same
## admitted call. Direct callers receive a complete material; reused bindings
## still receive unconditional node quality updates.
static func _generated_material(job: Dictionary, key: String, presentation: Dictionary, defer_quality := false) -> Material:
	var context: RefCounted = job.resources.material_context() if job.get("resources") != null else null
	if context != null and not key.begins_with("rc:") and not _urban_surface(key,presentation):
		var generated: ShaderMaterial = context.generated_material(ground_color(key,presentation),detail_kind(key),0.8 if key == "safety:metal" else 0.0,0.9,defer_quality)
		if not APPEARANCE.entry(key).is_empty(): APPEARANCE.bind(generated,key)
		generated.set_shader_parameter("regional_scale",300.0/float(presentation.get("facade_floor_cm",300)))
		bind_facade(generated,key,presentation)
		if facade_kind(key) == 3: context.bind_signs(generated)
		return generated
	var material := surface_material(key,presentation,job.get("urban_shader"))
	# Roads and sidewalks take the cell's night lamp pools (lamp_map).
	if _urban_surface(key,presentation): CITY_LAMPS.bind_light_map(job,material,presentation)
	if material is StandardMaterial3D:
		material.albedo_color = ground_color(key,presentation)
		material.roughness = 0.9
		material.cull_mode = BaseMaterial3D.CULL_DISABLED
		material.resource_name = "mk_"+detail_kind(key)
	var bound := _environment_surface(job,material,defer_quality)
	bind_facade(bound,key,presentation)
	return bound

static func _urban_surface(key: String, presentation: Dictionary) -> bool:
	return presentation.get("urban_surfaces",false) and (key.begins_with("road:") or key in ["asphalt","concrete","ground/asphalt","ground/concrete","ground/pavers","ground/stone_paving"])

static func surface_material(key: String, presentation: Dictionary, shader: Shader) -> Material:
	if key.begins_with("rc:"): return preload("./rc_venue.gd").material(key, int(presentation.track_seed))
	if key == "safety:metal":
		var metal := StandardMaterial3D.new()
		metal.albedo_color = Color(0.58, 0.63, 0.68)
		metal.metallic = 0.8
		metal.roughness = 0.3
		return metal
	if not _urban_surface(key,presentation): return StandardMaterial3D.new()
	var material := ShaderMaterial.new()
	material.shader = shader
	var style: Dictionary = presentation.get("road_styles", {}).get(key, {})
	var concrete := key == "concrete" or int(style.get("surface", 0)) == 1
	material.set_shader_parameter("base_color", COLORS[DATA.SURFACES[int(style.get("surface", 1 if concrete else 0))]])
	material.set_shader_parameter("paving", concrete)
	material.set_shader_parameter("marked", bool(style.get("marked", false)))
	for name: String in style:
		if name not in ["surface","detail_kind"]: material.set_shader_parameter(name, style[name])
	if style.has("detail_kind"): material.set_meta("regional_detail",style.detail_kind)
	if key.begins_with("ground/"):
		material.set_shader_parameter("base_color",APPEARANCE.color(key))
		material.set_shader_parameter("appearance_mode",1 if key == "ground/asphalt" else 2)
		material.set_shader_parameter("paving",key != "ground/asphalt")
		material.set_shader_parameter("slabs",key == "ground/concrete")
		material.set_meta("regional_detail",detail_kind(key))
	return material

static func _lamp_bindings(profile: Dictionary, assets: Dictionary) -> Dictionary:
	# Cell-local value records: another cell may replace the shared cache profile
	# between placements. Only placed assets consume the per-object allowance.
	var result := {}
	for binding: Dictionary in profile.get("lights",[]):
		if not assets.has(str(binding.asset_id)) or binding.get("bulb_materials",[]).is_empty(): continue
		var point: Array = binding.position_cm
		var rgb: Array = binding.color
		var lamp := {"offset_cm":Vector3(point[0],point[1],-point[2]),"basis":Basis.looking_at(Vector3.DOWN,Vector3.FORWARD),
			"range":float(binding.range_cm)*0.01*STREETLIGHT_SPREAD,"angle":STREETLIGHT_ANGLE,
			"energy":2.0,"attenuation":0.0,"angle_attenuation":0.25,
			"color":Color(float(rgb[0])/255.0,float(rgb[1])/255.0,float(rgb[2])/255.0)}
		lamp.make_read_only()
		result[str(binding.asset_id)] = lamp
	return result

static func _environment_lamp(job: Dictionary, id: String, placement: Transform3D) -> void:
	var bindings: Dictionary = job.get("lamp_bindings",{})
	if not bindings.has(id): return
	var lamp: Dictionary = bindings[id].duplicate()
	# Reuse the visual placement without changing rotation-before-cm conversion.
	lamp.position = placement.origin+placement.basis*lamp.offset_cm*0.01
	lamp.erase("offset_cm")
	var lamps: Array = job.root.get_meta("environment_lamps",[])
	lamps.append(lamp)
	job.root.set_meta("environment_lamps",lamps)
	job.root.add_to_group("mapkit_environment_cells")

## Only newly generated urban/RC bindings may defer detail quality. Borrowed
## assets and imported wrappers keep their normal preparation and live updates.
static func _environment_surface(job: Dictionary, material: Material, defer_quality := false) -> Material:
	if job.get("resources") == null: return material
	var context: RefCounted = job.resources.material_context()
	if context == null: return material
	if material is ShaderMaterial and material.shader == preload("./rc_surface.gdshader"):
		material.set_shader_parameter("environment_enabled",true)
		material.set_shader_parameter("environment_data",context.texture)
		context.bind_detail(material,str(material.get_meta("rc_detail_kind")),not defer_quality)
		return material
	if material is ShaderMaterial and material.shader == job.get("urban_shader"):
		material.set_shader_parameter("environment_enabled",true)
		material.set_shader_parameter("environment_data",context.texture)
		context.bind_detail(material,str(material.get_meta("regional_detail","concrete" if material.get_shader_parameter("paving") else "asphalt")),not defer_quality)
		return material
	return context.surface_material(material,0)

static func wet_urban_shader() -> Shader:
	var result := Shader.new()
	result.code = URBAN_SURFACE.code.replace('#include "wet_surface.gdshaderinc"',preload("./wet_surface.gdshaderinc").code).replace('#include "material_detail.gdshaderinc"',preload("./material_detail.gdshaderinc").code)
	return result

static func _object_group(object: Dictionary) -> String:
	var p := scene_position(object.position)
	return "%s/%d,%d,%d" % [object.asset_id,floori(p.x/32.0),floori(p.y/32.0),floori(p.z/32.0)]

static func _seed_fallback_materials(piece: MeshInstance3D, seed_value: float, lease: RefCounted) -> void:
	# Only exceptional overlays/unsupported instancing need a private parameter
	# binding. Shader code, mesh buffers and texture resources remain shared.
	if piece.mesh==null: return
	var copied := {}
	for index in range(-1,piece.mesh.get_surface_count()):
		var material: Material=piece.material_override if index==-1 else piece.get_active_material(index)
		if material is not ShaderMaterial or not material.get_meta("mapkit_opaque",false): continue
		if not copied.has(material):
			var replacement: ShaderMaterial=material.duplicate(false)
			replacement.set_shader_parameter("building_seed",seed_value)
			copied[material]=replacement
			if lease!=null: lease.track(replacement)
		if index==-1: piece.material_override=copied[material];break
		piece.set_surface_override_material(index,copied[material])

static func detail_kind(key: String) -> String:
	var appearance := APPEARANCE.entry(key)
	if not appearance.is_empty(): return str(appearance.detail)
	return {"dirt":"earth","gravel":"stone","builtin:tree":"wood","builtin:city_tree":"wood","builtin:fence":"wood","builtin:streetlight":"metal","builtin:signal_h":"metal","builtin:signal_v":"metal","builtin:bus_shelter":"metal","builtin:subway_entrance":"metal","builtin:station_sign":"metal","builtin:guide_sign":"metal","safety:metal":"metal","barrier:hedge":"leaf","barrier:fence":"wood","barrier:wall":"stone","builtin:planter":"stone","builtin:planter_square":"stone","builtin:median_bed":"stone","builtin:median_bed_pine":"stone"}.get(key,"metal" if key.begins_with("builtin:sign_") or key.begins_with("builtin:city_streetlight") else ("wood" if key.begins_with("builtin:street_tree_") else key.get_slice(":",0)))

static func occluding_surface(key: String) -> bool:
	return key == "rc:wall" or key.begins_with("building") or key.begins_with("urban:wall") or (key.contains(":") and key.get_slice(":",0) in ["brick","wood","concrete"])
