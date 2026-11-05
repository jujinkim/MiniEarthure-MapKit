extends RefCounted
## Display-only street trees and flower beds, built procedurally (no external
## assets): five street-tree species in tree pits beside the curb and four
## flower-bed pieces (building-front and plaza planters, median beds with and
## without a pine). Every array comes from integer hashes, so builds are
## identical. Metres, origin at the foot; local +x runs along the road.
##
## Street trees (builtin:street_tree_ginkgo, _zelkova, _plane, _cherry,
## _pine; collision: the 14 x 140 x 14 cm trunk box, footprint the turned
## 40 cm pit square, placement.rs) reuse the park vegetation's shaders, wind,
## leaf tint, low-tier hull regrowth and weather (vegetation_meshes.gd) on
## their own leaf atlas (street_leaf_atlas.png: ginkgo fans, pine needle
## tufts, small serrate leaves, large lobed plane leaves). Each species has a
## mature variant (instance scale 0.92-1.08) and a young replacement (12 % by
## identity, scale 1, crown x 0.65, a burlap trunk wrap, a wooden tripod with
## two rails and rubber ties); the pine also a mature variant without a pit
## for median beds. A variant is a `body` (bark tube with species bark baked
## into vertex colours, branches, the crown's dark inner hull and the pit
## fixtures), `cards` (alpha-scissor leaf cards) and `far` (coarse hull, plain
## trunk and the pit as one plate) like park trees:
##   ginkgo  straight leader, narrow cone, whorls of short upswept branches,
##           grey-brown bark with vertical fissures; cast-iron grate.
##   zelkova trunk forking at 0.95 m into five ascending limbs under a broad
##           dome, smooth grey bark with orange-brown flakes; cast-iron grate.
##   plane   tall straight trunk, broad ovoid crown, camouflage bark (cream,
##           olive and grey patches); granite frame with mulch and gravel.
##   cherry  low fork at 0.85 m, four spreading limbs, umbrella crown, dark
##           bark with pale lenticel bands; frame with a liriope ring.
##   pine    gently curved trunk, four or five flat tiered needle pads, grey
##           plated bark turning red-orange above; soil mound in a stone ring.
## Guarantees (street_planting_validator): at the largest scale every crown
## vertex plus its largest sway stays within CROWN_CM (mapkit-core
## STREET_TREES) of the axis; at the smallest every vertex beyond 30 cm of the
## axis is at least STREET_CANOPY_MIN_CM (1.20 m) up, so crowns may overhang
## the carriageway above vehicles; the trunk (young: the wrap) encloses the
## trunk box up to 0.60 m (car height 0.57 m); pit fixtures stay inside a 36 cm square (38.9 cm
## at 1.08, inside the 40 cm pit) and rise to 4.5 cm, above the 3 cm sidewalk.
## A crown turns with the placement plus a quarter turn from the identity, so
## the pit stays square to the curb.
##
## Flower beds (builtin:planter 80 x 14 x 30 cm, planter_square 60 x 18 x 60,
## median_bed 300 x 8 x 80 and median_bed_pine, the bed plus a pine at its
## centre; all solid boxes): two instances per piece, a rim with its soil and
## a planting. Rims (by the 48 m world block, so neighbours match): granite
## blocks with chamfered tops, red brick with a soldier-course cap,
## exposed-aggregate concrete, or a dark green painted box on a plinth with
## feet (not in medians; median curbs are granite or concrete). Plantings (by
## identity): bedding annuals (petunia, marigold, pansy and begonia heads on
## leafy mounds), a clipped boxwood block, a royal azalea mound in magenta
## bloom, or ornamental grass with a liriope edge; median beds a clipped
## azalea or boxwood hedge with rounded ends (by block) with or without an
## edge of annuals (by identity). Rims enclose their proxy box by 5 mm and
## everything stays within the box plus 3 cm and below 45 cm (planter), 60 cm
## (plaza planter) or 25 cm (median bed). A `far` box with a planted top
## replaces both from the near range on.
##
## The renderer draws these meshes instead of the generated proxy faces
## (render_memory.DRAWN_OVER); collision and occluders never read those faces.
## A cell instances each mesh per 32 m square as MultiMeshes with leaf tint
## and wind phase (display_quality.apply_vegetation: trees as `tree`, beds as
## `planter`). build() makes one part per call for admitted streaming steps
## (render_resource_cache.prepare_mesh_set); step_shared() builds a
## process-lifetime set for cells without a resource cache.

const VEG := preload("./vegetation_meshes.gd")
const F := preload("./street_furniture_meshes.gd")
const QUALITY := preload("./display_quality.gd")
const KEY := "builtin:street_planting"
const GROUP_METRES := 32.0
## MultiMesh floats per instance: 3x4 transform and the custom colour.
const FLOATS := 16
## scripts/richer_assets.py --textures --leaves (2x2 leaf clusters, 256 px).
const LEAF_ATLAS := "textures/vegetation/street_leaf_atlas.png"
const SPECIES := ["ginkgo", "zelkova", "plane", "cherry", "pine"]
const TREE_IDS := {"builtin:street_tree_ginkgo": "ginkgo", "builtin:street_tree_zelkova": "zelkova",
	"builtin:street_tree_plane": "plane", "builtin:street_tree_cherry": "cherry", "builtin:street_tree_pine": "pine"}
const BED_IDS := {"builtin:planter": "planter", "builtin:planter_square": "planter_square",
	"builtin:median_bed": "median_bed", "builtin:median_bed_pine": "median_bed"}
const IDS := ["builtin:street_tree_ginkgo", "builtin:street_tree_zelkova", "builtin:street_tree_plane", "builtin:street_tree_cherry",
	"builtin:street_tree_pine", "builtin:planter", "builtin:planter_square", "builtin:median_bed", "builtin:median_bed_pine"]
## Crown reach from the trunk axis (cm; mapkit-core STREET_TREES, the display
## contract the importer fits crowns to), the pit square and the canopy floor
## beyond CANOPY_AXIS of the axis (STREET_TREE_PIT_CM, STREET_CANOPY_MIN_CM).
const CROWN_CM := {"ginkgo": 60, "zelkova": 125, "plane": 115, "cherry": 120, "pine": 85}
const PIT_CM := 40
const CANOPY_MIN := 1.20
const CANOPY_AXIS := 0.30
## Species at scale 1: canopy base, canopy height, crown radius before the
## reach fit. Mirrored by the distant silhouette (distant.rs STREET_TREES).
const FORMS := {"ginkgo": Vector3(1.35, 2.55, 0.55), "zelkova": Vector3(1.35, 2.10, 1.15), "plane": Vector3(1.45, 2.35, 1.05),
	"cherry": Vector3(1.35, 1.45, 1.10), "pine": Vector3(1.35, 1.55, 0.80)}
## Mature instance scale span; young replacements: share, scale 1, crown factor.
const SCALE := Vector2(0.92, 1.08)
const YOUNG_SHARE := 0.12
const YOUNG_CROWN := 0.65
## Mesh-space canopy floor: vertices beyond AXIS_LIMIT (CANOPY_AXIS at the
## largest scale) stand at least FLOOR high (CANOPY_MIN at the smallest scale,
## plus a centimetre and a half for leaf-card flutter).
const AXIS_LIMIT := CANOPY_AXIS / 1.08
const FLOOR := CANOPY_MIN / 0.92 + 0.015
## Leaf colours (sRGB), card counts and sizes, atlas cells.
const LEAVES := {"ginkgo": "7a9a3c", "zelkova": "4f7d3e", "plane": "5d8a43", "cherry": "3f6a3a", "pine": "35583c"}
const CARDS := {"ginkgo": 160, "zelkova": 190, "plane": 140, "cherry": 180, "pine": 200}
const CARD_SIZE := {"ginkgo": 0.34, "zelkova": 0.40, "plane": 0.65, "cherry": 0.45, "pine": 0.42}
const TILE := {"ginkgo": Vector2(0.0, 0.0), "pine": Vector2(0.5, 0.0), "zelkova": Vector2(0.0, 0.5), "cherry": Vector2(0.0, 0.5), "plane": Vector2(0.5, 0.5)}
## Pit fixtures per species.
const PITS := {"ginkgo": "grate", "zelkova": "grate", "plane": "mulch", "cherry": "liriope", "pine": "mound"}
## Pit frame (outer and inner half size), its top and bottom, the grate's top
## and the hole the trunk passes through.
const PIT_HALF := 0.18
const PIT_INNER := 0.155
const PIT_TOP := 0.045
const PIT_BOTTOM := -0.005
const GRATE_TOP := 0.04
const GRATE_HOLE := 0.135
## Trunk tube sides below and above LOWER_TOP; the lower tube encloses the box.
const TRUNK_SIDES := 12
const UPPER_SIDES := 8
const LOWER_TOP := 0.72
const BARK := {"ginkgo": "5e5248", "zelkova": "8a837a", "plane": "8a8b6d", "cherry": "4b3b36", "pine": "5c4f45"}
const WRAP := "a08a5e"
const STAKE := "a8865a"
const TIE := "2a2a2a"
const IRON := "3b3d40"
const GRANITE := "b3ada3"
const SOIL := "4a3828"
## Flower beds: proxy box (width along local x, height, depth), rim wall
## thickness, rim styles (0 granite, 1 brick, 2 exposed aggregate, 3 painted
## box), plantings and their height caps.
const PIECES := {"planter": Vector3(0.80, 0.14, 0.30), "planter_square": Vector3(0.60, 0.18, 0.60), "median_bed": Vector3(3.0, 0.08, 0.80)}
const RIM_STYLES := {"planter": [0, 1, 2, 3], "planter_square": [0, 1, 2, 3], "median_bed": [0, 2]}
const RIM_WALL := {"planter": 0.03, "planter_square": 0.035, "median_bed": 0.05}
const RIM_OUT := 0.005
const RIM_SKIRT := 0.02
const SOIL_DROP := 0.02
const PLANT_CAP := {"planter": 0.45, "planter_square": 0.60, "median_bed": 0.25}
const PLANTINGS := 4
## Planters: annuals, boxwood, azalea, grass. Median beds: azalea with and
## without an edge of annuals, boxwood with and without.
const PLANTING_LEAVES := {"planter": ["4f8a3e", "3d6b3a", "4a6e38", "8aa060"], "median_bed": ["4a6e38", "4a6e38", "3d6b3a", "3d6b3a"]}
## Bedding annuals: petunia magenta and white, marigold orange and yellow,
## pansy violet, begonia red; royal azalea magenta.
const FLOWERS := ["c8287a", "f2f0ea", "f08418", "f5c81e", "6b3fa6", "c8202c"]
const AZALEA := "d0307e"
## Far planted tops and granite sides (sRGB; distant.rs uses the same).
const FAR_SIDE := "aaa69e"
const FAR_TOP := "567046"
## World block (cm) that picks a rim style and a median hedge.
const BLOCK_CM := 4800
## Memory allowance of one shared set (render_resource_cache): the measured
## vertices at <= 56 B and indices at 4 B in CPU arrays and GPU buffers
## (street_planting_validator), the 256 px RGBA leaf atlas with mipmaps as
## decoded image and texture (350 KiB each) and 128 KiB for the two materials
## and one mesh's build arrays.
const SHARED_BYTES := 4 * 1024 * 1024
static var STEPS: PackedStringArray = _steps()
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}
static var _crowns: Dictionary = {}
static var _shapes: Dictionary = {}

static func _steps() -> PackedStringArray:
	var result := PackedStringArray(["atlas", "material_body", "material_cards"])
	for species: String in SPECIES:
		for variant in 2:
			for part in ["shape", "crown", "body", "cards"]: result.append("tree_%s_%d_%s" % [species, variant, part])
			# The median-bed pine shares the mature pine's crown, built before
			# that crown is released with its far mesh.
			if species == "pine" and variant == 0: result.append("tree_pine_2_body")
			result.append("tree_%s_%d_far" % [species, variant])
	for piece: String in PIECES:
		for style: int in RIM_STYLES[piece]: result.append("rim_%s_%d" % [piece, style])
		for planting in PLANTINGS: result.append("planting_%s_%d" % [piece, planting])
		result.append("far_" + piece)
	return result

static func linear(srgb: String, shade := 1.0) -> Color:
	var c := Color(srgb).srgb_to_linear()
	return Color(c.r * shade, c.g * shade, c.b * shade, 0.0)

static func _identity_unit(id: Variant) -> float:
	return float(str(id).sha256_text().substr(0, 6).hex_to_int()) / 16777216.0

# Shared set --------------------------------------------------------------------

static func _atlas() -> Texture2D:
	var path: String = (VEG.SHADER as Resource).resource_path.get_base_dir().path_join(LEAF_ATLAS)
	var source := ResourceLoader.load(path, "Texture2D", ResourceLoader.CACHE_MODE_IGNORE) as Texture2D
	var image := source.get_image() if source != null else null
	if image == null or image.is_empty(): return null # cards are optional decoration
	if image.is_compressed(): image.decompress()
	image.convert(Image.FORMAT_RGBA8)
	image.generate_mipmaps()
	return ImageTexture.create_from_image(image)

## One part per call (the atlas, a material or a mesh; a tree's shape and
## crown fit are steps of their own); true when the whole set is in
## state.result. `lease` (track(Object)) receives every created resource.
## The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {"materials": {}}
		state.atlas = null
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "street_planting_" + step
	var result: Dictionary = state.result
	if step == "atlas":
		state.atlas = _atlas()
		if state.atlas != null and lease != null: lease.track(state.atlas)
	elif step.begins_with("material_"):
		var kind := step.trim_prefix("material_")
		var made: ShaderMaterial = VEG.material(kind, state.atlas, context)
		made.resource_name = "mk_street_planting_" + kind
		result.materials[kind] = made
		if lease != null: lease.track(made)
		if kind == "cards": state.atlas = null
	elif step.begins_with("tree_"):
		var species := step.get_slice("_", 1)
		var variant := int(step.get_slice("_", 2))
		var part := step.get_slice("_", 3)
		if part == "shape": _shapes["%s_%d" % [species, variant]] = shape(species, variant)
		elif part == "crown": crown(species, variant)
		else:
			var mesh := tree_mesh(species, part, variant, result.materials)
			if lease != null: lease.track(mesh)
			result["%s_%s_%d" % [species, part, variant]] = mesh
	else:
		var kind := step.get_slice("_", 0)
		var rest := step.trim_prefix(kind + "_")
		var mesh: ArrayMesh
		var name := ""
		if kind == "far":
			mesh = bed_far(rest, result.materials)
			name = rest + "_far"
		else:
			var index := int(rest.get_slice("_", rest.get_slice_count("_") - 1))
			var piece := rest.trim_suffix("_%d" % index)
			mesh = rim(piece, index, result.materials) if kind == "rim" else planting(piece, index, result.materials)
			name = "%s_%s_%d" % [piece, kind, index]
		if lease != null: lease.track(mesh)
		result[name] = mesh
	state.step += 1
	if state.step < STEPS.size(): return false
	# The median-bed pine draws the mature pine's cards and far mesh.
	result["pine_cards_2"] = result["pine_cards_0"]
	result["pine_far_2"] = result["pine_far_0"]
	state.erase("atlas")
	result.make_read_only()
	return true

## Process-lifetime set for cells without a resource cache (standalone
## previews), one part per call; the set once complete, else {}.
static func step_shared() -> Dictionary:
	if not _shared.is_empty(): return _shared
	if build(_shared_job):
		_shared = _shared_job.result
		_shared_job = {}
	return _shared

## The whole process-lifetime set at once (validators and single-node previews).
static func shared() -> Dictionary:
	while step_shared().is_empty(): pass
	return _shared

## Claims the shared set as a cell begins (chunk_renderer.begin, before any
## upload) when it holds street planting and has a resource cache; false when
## the cache refuses it (the cell fails: these pieces would lose their only
## visible body).
static func claim(job: Dictionary, objects: Array) -> bool:
	if job.get("resources") == null or job.claims.has(KEY): return true
	for object: Dictionary in objects:
		if not str(object.asset_id) in IDS: continue
		var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
		if key.is_empty(): return false
		job.claims[KEY] = key
		return true
	return true

## Shared set for a renderer job: 1 ready, 0 another step is needed, -2 it is
## unavailable, which fails the cell (chunk_renderer).
static func ready(job: Dictionary) -> int:
	if job.has("street_planting"): return 1
	job.last_phase = "street_planting_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.street_planting = made
		return 0
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.street_planting = made
	return 0

# Placement ---------------------------------------------------------------------

## Display pieces of one object: kind (`tree` or `planter`, the display
## quality kind), the meshes drawn per LOD (body, cards, far), the scene
## transform and the instance data (leaf tint and wind phase). A street tree
## is one piece, a planter or median bed two (rim and planting), a median bed
## with a pine three. Planters take the placement transform exactly.
static func pieces(object: Dictionary) -> Array:
	var id := str(object.asset_id)
	var pick := _identity_unit(object.id)
	if TREE_IDS.has(id): return [tree_piece(TREE_IDS[id], object, pick, false)]
	var piece: String = BED_IDS[id]
	var p: Array = object.position
	var block := Vector2i(floori(float(p[0]) / BLOCK_CM), floori(float(p[2]) / BLOCK_CM))
	var styles: Array = RIM_STYLES[piece]
	var style: int = styles[int(VEG.unit(block.x, block.y, 53) * styles.size()) % styles.size()]
	var kind := 0
	if piece == "median_bed": kind = (0 if VEG.unit(block.x, block.y, 59) < 0.5 else 2) + (0 if fmod(pick * 11.0, 1.0) < 0.5 else 1)
	else: kind = int(fmod(pick * 7.0, 1.0) * float(PLANTINGS)) % PLANTINGS
	var leaves: String = PLANTING_LEAVES["median_bed" if piece == "median_bed" else "planter"][kind]
	var transform := F.placement(object)
	var result := [{"kind": "planter", "lods": {"body": ["%s_rim_%d" % [piece, style]], "far": [piece + "_far"]}, "transform": transform, "custom": Color(0.0, 0.0, 0.0, 1.0)},
		{"kind": "planter", "lods": {"body": ["%s_planting_%d" % [piece, kind]]}, "transform": transform, "custom": VEG.leaf_custom(leaves, pick)}]
	if id == "builtin:median_bed_pine": result.append(tree_piece("pine", object, pick, true))
	return result

## A street tree's variant (0 mature, 1 young, 2 the median-bed pine), scale
## and turn from its identity: the crown turns with the placement plus a
## quarter turn, so the pit stays square to the curb.
static func tree_piece(species: String, object: Dictionary, pick: float, bed: bool) -> Dictionary:
	var young := not bed and fmod(pick * 11.0, 1.0) < YOUNG_SHARE
	var variant := 2 if bed else (1 if young else 0)
	var scale := 1.0 if young else SCALE.x + (SCALE.y - SCALE.x) * fmod(pick * 7.0, 1.0)
	var quarter := int(fmod(pick * 13.0, 1.0) * 4.0) % 4
	var yaw := float(int(object.quarter_turns) + quarter) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0)
	var p: Array = object.position
	var transform := Transform3D(Basis(Vector3.UP, yaw).scaled(Vector3.ONE * scale), Vector3(float(p[0]), float(p[1]), -float(p[2])) * 0.01)
	return {"kind": "tree", "species": species, "variant": variant, "scale": scale,
		"lods": {"body": ["%s_body_%d" % [species, variant]], "cards": ["%s_cards_%d" % [species, variant]], "far": ["%s_far_%d" % [species, variant]]},
		"transform": transform, "custom": VEG.leaf_custom(LEAVES[species], pick)}

## One admitted step per object: its pieces join the cell's groups of their
## meshes and 32 m square, attached one group per step after the objects.
static func plan(job: Dictionary, object: Dictionary) -> void:
	if not job.has("street_plan"): job.street_plan = {}
	for piece: Dictionary in pieces(object):
		var transform: Transform3D = piece.transform
		var o := transform.origin
		var key := "%s/%d,%d" % [piece.lods.body[0], floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
		if not job.street_plan.has(key): job.street_plan[key] = {"kind": piece.kind, "lods": piece.lods, "parts": [], "ids": PackedStringArray()}
		var b := transform.basis
		var c: Color = piece.custom
		job.street_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z, c.r, c.g, c.b, c.a]))
		job.street_plan[key].ids.append(str(object.id))

## The MultiMeshes of one planned group (one per mesh and LOD; display
## quality sets ranges and shadows from `mapkit_vegetation_lod` and
## `mapkit_vegetation_kind`). The cell lease carries the nodes and instance
## buffers; meshes and materials belong to the shared set. mapkit_object_ids
## lists the instances' object ids (editor picking).
static func attach_group(job: Dictionary) -> void:
	var key: String = job.street_plan.keys()[0]
	var group: Dictionary = job.street_plan[key]
	job.street_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in group.lods:
		for name: String in group.lods[lod]:
			var mesh: Mesh = job.street_planting[name]
			if mesh.get_surface_count() == 0: continue
			var multi := MultiMesh.new()
			multi.transform_format = MultiMesh.TRANSFORM_3D
			multi.use_custom_data = true
			multi.mesh = mesh
			multi.instance_count = buffer.size() / FLOATS
			multi.buffer = buffer
			var node := MultiMeshInstance3D.new()
			node.name = "StreetPlanting_%s_%s_%d" % [name, lod, job.root.get_child_count()]
			node.multimesh = multi
			# Wind moves leaves a few centimetres past the mesh bounds.
			node.extra_cull_margin = 0.3
			node.set_meta("mapkit_static_detail", true)
			node.set_meta("mapkit_decoration", false)
			node.set_meta("mapkit_vegetation_lod", lod)
			node.set_meta("mapkit_vegetation_kind", group.kind)
			node.set_meta("mapkit_object_ids", group.ids)
			QUALITY.apply_node(node, quality)
			if job.get("lease") != null:
				job.lease.track(node)
				job.lease.track(multi)
			job.root.add_child(node)

## Single nodes of one object's near meshes (body and cards), transformed
## relative to `anchor` (chunk_renderer._street_detail, validators).
static func detail_nodes(object: Dictionary, anchor: Transform3D) -> Array:
	var meshes := shared()
	var result := []
	for piece: Dictionary in pieces(object):
		for lod in ["body", "cards"]:
			for name: String in piece.lods.get(lod, []):
				var mesh: ArrayMesh = meshes[name]
				if mesh.get_surface_count() == 0: continue
				var node := MeshInstance3D.new()
				node.name = "StreetPlanting_" + name
				node.mesh = mesh
				node.transform = anchor.affine_inverse() * (piece.transform as Transform3D)
				node.set_meta("mapkit_static_detail", true)
				node.set_meta("mapkit_decoration", false)
				node.set_meta("mapkit_vegetation_lod", lod)
				node.set_meta("mapkit_vegetation_kind", piece.kind)
				QUALITY.apply_node(node, QUALITY.active())
				result.append(node)
	return result

# Geometry helpers -----------------------------------------------------------------

## Convex polygon facing `normal` (front faces wind clockwise as seen along
## -normal, Godot's convention, whatever the point order).
static func face(b: VEG.Builder, points: Array, normal: Vector3, color: Color, sway := 0.0, colors := []) -> void:
	var first := b.vertices.size()
	var newell := Vector3.ZERO
	for i in points.size():
		var p: Vector3 = points[i]
		var q: Vector3 = points[(i + 1) % points.size()]
		newell += Vector3((p.y - q.y) * (p.z + q.z), (p.z - q.z) * (p.x + q.x), (p.x - q.x) * (p.y + q.y))
		b.add(p, normal, colors[i] if not colors.is_empty() else color, Vector2.ZERO, Vector2(sway, 0.0))
	var flip := newell.dot(normal) > 0.0
	for i in range(1, points.size() - 1):
		if flip: b.triangle(first, first + i + 1, first + i)
		else: b.triangle(first, first + i, first + i + 1)

## Grid of quads from `origin` along `du` and `dv` (nu x nv cells) facing
## `normal`, a colour per vertex from `paint` (a Callable(point) -> Color).
static func grid(b: VEG.Builder, origin: Vector3, du: Vector3, dv: Vector3, nu: int, nv: int, normal: Vector3, paint: Callable) -> void:
	var first := b.vertices.size()
	for i in nu + 1:
		for j in nv + 1:
			var p := origin + du * (float(i) / nu) + dv * (float(j) / nv)
			b.add(p, normal, paint.call(p), Vector2.ZERO, Vector2.ZERO)
	var flip := du.cross(dv).dot(normal) > 0.0
	for i in nu:
		for j in nv:
			var a := first + i * (nv + 1) + j
			var c := a + nv + 1
			if flip:
				b.triangle(a, c + 1, c); b.triangle(a, a + 1, c + 1)
			else:
				b.triangle(a, c, c + 1); b.triangle(a, c + 1, a + 1)

## Block from `low` to `high` (axis aligned) with its top edges chamfered by
## `chamfer`, no bottom; side faces as grids of `cells` (along the longest
## side, up), coloured per vertex by `paint`. Bit k of `sides` keeps side k
## (-z, +x, +z, -x); a dropped side (abutting the next block) has no face and
## its top edge stays square.
static func block(b: VEG.Builder, low: Vector3, high: Vector3, chamfer: float, paint: Callable, cells := Vector2i(1, 1), sides := 15) -> void:
	var top := high.y
	var cut := []
	for k in 4: cut.append(chamfer if (sides >> k) & 1 == 1 else 0.0)
	var tops := [Vector3(low.x + cut[3], top, low.z + cut[0]), Vector3(high.x - cut[1], top, low.z + cut[0]),
		Vector3(high.x - cut[1], top, high.z - cut[2]), Vector3(low.x + cut[3], top, high.z - cut[2])]
	var corners := [Vector3(low.x, 0, low.z), Vector3(high.x, 0, low.z), Vector3(high.x, 0, high.z), Vector3(low.x, 0, high.z)]
	face(b, tops, Vector3.UP, Color.BLACK, 0.0, [paint.call(corners[0] + Vector3.UP * top), paint.call(corners[1] + Vector3.UP * top), paint.call(corners[2] + Vector3.UP * top), paint.call(corners[3] + Vector3.UP * top)])
	var normals := [Vector3.FORWARD, Vector3.RIGHT, Vector3.BACK, Vector3.LEFT]
	var longest := maxf(maxf(high.x - low.x, high.z - low.z), 0.001)
	var under := top - chamfer
	for k in 4:
		if (sides >> k) & 1 == 0: continue
		var a: Vector3 = corners[k]
		var e: Vector3 = corners[(k + 1) % 4]
		var n: Vector3 = normals[k]
		if chamfer > 0.0:
			face(b, [a + Vector3.UP * under, e + Vector3.UP * under, tops[(k + 1) % 4], tops[k]], (n + Vector3.UP).normalized(), Color.BLACK, 0.0,
				[paint.call(a + Vector3.UP * under), paint.call(e + Vector3.UP * under), paint.call(e + Vector3.UP * top), paint.call(a + Vector3.UP * top)])
		var count := maxi(1, roundi(float(cells.x) * (e - a).length() / longest))
		grid(b, a + Vector3.UP * low.y, e - a, Vector3.UP * (under - low.y), count, cells.y, n, paint)

## Side mask of segment `s` of `count` along a wall (local x when `along_x`):
## the faces between abutting segments are dropped.
static func segment_sides(s: int, count: int, along_x: bool) -> int:
	var sides := 15
	if along_x:
		if s > 0: sides &= ~8
		if s < count - 1: sides &= ~2
	else:
		if s > 0: sides &= ~1
		if s < count - 1: sides &= ~4
	return sides

## Rounded box about `centre` with half size `half` and per-axis corner radii
## `round` on a subdivided cube (`counts` cells per axis); vertices pushed
## along their normals by up to `jitter` (a hash of the position, so shared
## edges stay closed). Colour per vertex from `paint(point, normal)`; COLOR.a
## = `leaf`; sway weight from `sway_of(point)` when valid.
static func rounded(b: VEG.Builder, centre: Vector3, half: Vector3, round: Vector3, counts: Vector3i, seed: int, jitter: float, leaf: float, paint: Callable, sway_of := Callable(), skip_bottom := true) -> void:
	var core := (half - round).max(Vector3.ZERO)
	for axis in 3:
		for side in [-1.0, 1.0]:
			if skip_bottom and axis == 1 and side < 0.0: continue
			var u := (axis + 1) % 3
			var v := (axis + 2) % 3
			var nu: int = counts[u]
			var nv: int = counts[v]
			var first := b.vertices.size()
			var axis_normal := Vector3.ZERO
			axis_normal[axis] = side
			for i in nu + 1:
				for j in nv + 1:
					var c := Vector3.ZERO
					c[axis] = side * half[axis]
					c[u] = lerpf(-half[u], half[u], float(i) / nu)
					c[v] = lerpf(-half[v], half[v], float(j) / nv)
					var q := c.clamp(-core, core)
					var d := (c - q) / round.max(Vector3.ONE * 0.0001)
					var unit_d := d.normalized() if d.length() > 0.0001 else axis_normal
					var p := q + unit_d * round
					var n := (unit_d / round.max(Vector3.ONE * 0.0001)).normalized()
					var key := Vector3i(roundi(c.x * 1000.0), roundi(c.y * 1000.0), roundi(c.z * 1000.0))
					p += n * (VEG.unit(key.x * 31 + seed, key.y * 17 + seed, key.z) - 0.5) * jitter
					var color: Color = paint.call(centre + p, n)
					color.a = leaf
					b.add(centre + p, n, color, Vector2.ZERO, Vector2(sway_of.call(centre + p) if sway_of.is_valid() else 0.0, 0.0))
			for i in nu:
				for j in nv:
					var a := first + i * (nv + 1) + j
					var e := a + nv + 1
					var cross := (b.vertices[e] - b.vertices[a]).cross(b.vertices[e + 1] - b.vertices[a])
					if cross.dot(axis_normal) > 0.0:
						b.triangle(a, e + 1, e); b.triangle(a, a + 1, e + 1)
					else:
						b.triangle(a, e, e + 1); b.triangle(a, e + 1, a + 1)

## Lumpy ellipsoid mound (icosphere) about `centre`, flat at `floor`.
static func mound(b: VEG.Builder, centre: Vector3, radii: Vector3, detail: int, seed: int, floor_y: float, leaf_shade: float, sway_weight: float) -> void:
	var ico: Array = VEG.icosphere(detail)
	var points: PackedVector3Array = ico[0]
	var faces: PackedInt32Array = ico[1]
	var first := b.vertices.size()
	for i in points.size():
		var d := points[i]
		var p := centre + d * radii * (0.88 + 0.2 * VEG.unit(seed, i, 41))
		p.y = maxf(p.y, floor_y)
		var ao := lerpf(0.5, 1.0, clampf((p.y - floor_y) / maxf(radii.y * 2.0, 0.01), 0.0, 1.0)) * leaf_shade
		b.add(p, (d / radii).normalized(), Color(ao, ao, ao, 1.0), Vector2.ZERO, Vector2(sway_weight * clampf((p.y - floor_y) / maxf(radii.y * 2.0, 0.01), 0.0, 1.0), 0.0))
	for f in range(0, faces.size(), 3):
		b.triangle(first + faces[f], first + faces[f + 2], first + faces[f + 1])

## Five-petal flower head facing `normal`: a centre and five petal tips.
static func flower(b: VEG.Builder, centre: Vector3, normal: Vector3, radius: float, petals: Color, seed: int, sway_weight: float) -> void:
	var helper := Vector3.FORWARD if absf(normal.y) > 0.9 else Vector3.UP
	var u := normal.cross(helper).normalized()
	var v := normal.cross(u)
	var turn := VEG.unit(seed, 3) * TAU
	var eye := petals.lerp(linear("f2c21b"), 0.6)
	var first := b.add(centre + normal * radius * 0.25, normal, eye, Vector2.ZERO, Vector2(sway_weight, 0.0))
	for k in 5:
		var a := turn + TAU * float(k) / 5.0
		b.add(centre + (u * cos(a) + v * sin(a)) * radius * (0.85 + 0.3 * VEG.unit(seed, k, 4)), normal, petals, Vector2.ZERO, Vector2(sway_weight, 0.0))
	for k in 5:
		var p0 := b.vertices[first + 1 + k] - b.vertices[first]
		var p1 := b.vertices[first + 1 + (k + 1) % 5] - b.vertices[first]
		if p0.cross(p1).dot(normal) > 0.0: b.triangle(first, first + 1 + (k + 1) % 5, first + 1 + k)
		else: b.triangle(first, first + 1 + k, first + 1 + (k + 1) % 5)

## Thin two-sided blade from `root` leaning along `out` (grass, liriope).
static func blade(b: VEG.Builder, root: Vector3, out: Vector3, length: float, width: float, color: Color, leaf: float, sway_weight: float) -> void:
	var tip := root + (Vector3.UP + out).normalized() * length
	var across := out.cross(Vector3.UP).normalized() * width * 0.5 if out.length() > 0.001 else Vector3.RIGHT * width * 0.5
	var normal := (tip - root).cross(across).normalized()
	var low := Color(color.r * 0.6, color.g * 0.6, color.b * 0.6, leaf)
	var high := Color(color.r, color.g, color.b, leaf)
	for side in [1.0, -1.0]:
		var first := b.add(root - across, normal * side, low, Vector2.ZERO, Vector2.ZERO)
		b.add(root + across, normal * side, low, Vector2.ZERO, Vector2.ZERO)
		b.add(tip, normal * side, high, Vector2.ZERO, Vector2(sway_weight, 0.0))
		var cross := (b.vertices[first + 1] - b.vertices[first]).cross(b.vertices[first + 2] - b.vertices[first])
		if cross.dot(normal * side) > 0.0: b.triangle(first, first + 2, first + 1)
		else: b.triangle(first, first + 1, first + 2)

## Tube through `points` with `radii` (open ends), vertex colours from
## `paint(point, ring, side)` (alpha 0: untinted bark), wind weight from the
## canopy base like the park trees.
static func tube(b: VEG.Builder, points: Array, radii: Array, sides: int, paint: Callable, base: float, height: float, sway_scale := 1.0) -> void:
	var first := b.vertices.size()
	for i in points.size():
		var p: Vector3 = points[i]
		var tangent: Vector3 = ((points[mini(i + 1, points.size() - 1)] as Vector3) - (points[maxi(i - 1, 0)] as Vector3)).normalized()
		var helper := Vector3.RIGHT if absf(tangent.x) < 0.9 else Vector3.FORWARD
		var n1 := tangent.cross(helper).normalized()
		var n2 := tangent.cross(n1)
		for k in sides:
			var theta := TAU * float(k) / float(sides)
			var direction := n1 * cos(theta) + n2 * sin(theta)
			var q := p + direction * float(radii[i])
			b.add(q, direction, paint.call(q, i, k), Vector2.ZERO, Vector2(VEG.sway(q.y, base, height) * sway_scale, 0.0))
	for i in points.size() - 1:
		for k in sides:
			var a := first + i * sides + k
			var c := first + i * sides + (k + 1) % sides
			b.triangle(a, a + sides, c + sides)
			b.triangle(a, c + sides, c)

# Street trees ---------------------------------------------------------------------

static func form(species: String, variant: int) -> Vector3:
	var f: Vector3 = FORMS[species]
	return Vector3(f.x, f.y * YOUNG_CROWN, f.z * YOUNG_CROWN) if variant == 1 else f

static func largest_scale(variant: int) -> float:
	return 1.0 if variant == 1 else SCALE.y

static func smallest_scale(variant: int) -> float:
	return 1.0 if variant == 1 else SCALE.x

## Largest horizontal reach (metres, mesh space) of a crown vertex that sways
## `sway_m` sideways: at the variant's largest scale it plus its sway stays
## within the species' crown reach.
static func reach_limit(species: String, variant: int, sway_m := 0.0) -> float:
	return (float(CROWN_CM[species]) / 100.0 - sway_m) / largest_scale(variant)

## Largest sideways wind sway (metres) of a vertex at mesh height `y`.
static func sway_reach(y: float, base: float, height: float, card: bool) -> float:
	return VEG.sway(y, base, height) * (VEG.SWAY_PEAK * VEG.TREE_SWAY + (VEG.FLUTTER if card else 0.0))

## Crown lobes ([centre, radii], the core first) of a species.
static func species_lobes(species: String, seed: int, base: float, height: float, radius: float) -> Array:
	var result := []
	match species:
		"ginkgo":
			# A narrow cone: lobes stacked up the leader, narrowing upward.
			result.append([Vector3(0.0, base + height * 0.42, 0.0), Vector3(radius * 0.78, height * 0.4, radius * 0.78)])
			for i in 5:
				var t := float(i) / 4.0
				var angle := VEG.unit(seed, i, 1) * TAU
				var out := radius * 0.12 * VEG.unit(seed, i, 2)
				var r := radius * (1.0 - 0.62 * t) * (0.9 + 0.12 * VEG.unit(seed, i, 3))
				var v := height * (0.2 - 0.05 * t)
				var y := clampf(base + height * (0.2 + 0.66 * t), base + v * 0.9, base + height - v)
				result.append([Vector3(cos(angle) * out, y, sin(angle) * out), Vector3(r, v, r)])
		"zelkova": result = VEG.lobes(seed, base, height, radius, 7, 1.15)
		"plane": result = VEG.lobes(seed, base, height, radius, 6, 0.9)
		"cherry":
			# Umbrella: a flat ring of lobes around a low core under a cap.
			result.append([Vector3(0.0, base + height * 0.48, 0.0), Vector3(radius * 0.62, height * 0.42, radius * 0.62)])
			var turn := VEG.unit(seed, 1) * TAU
			for i in 6:
				var angle := turn + TAU * float(i) / 6.0 + (VEG.unit(seed, i, 2) - 0.5) * 0.6
				var out := radius * (0.56 + 0.1 * VEG.unit(seed, i, 3))
				var r := radius * (0.4 + 0.08 * VEG.unit(seed, i, 4))
				var v := height * (0.26 + 0.06 * VEG.unit(seed, i, 5))
				var y := clampf(base + height * (0.4 + 0.12 * VEG.unit(seed, i, 6)), base + v * 0.9, base + height - v)
				result.append([Vector3(cos(angle) * out, y, sin(angle) * out), Vector3(r, v, r)])
			result.append([Vector3(0.0, base + height * 0.73, 0.0), Vector3(radius * 0.55, height * 0.27, radius * 0.55)])
		_:
			# Pine: a slim core and four or five flat needle pads in tiers,
			# turning round the trunk, the last one on top.
			result.append([Vector3(0.0, base + height * 0.5, 0.0), Vector3(radius * 0.26, height * 0.42, radius * 0.26)])
			var turn := VEG.unit(seed, 1) * TAU
			var pads := 4 + int(VEG.unit(seed, 2) * 2.0)
			for i in pads:
				var t := float(i) / float(pads - 1)
				var top := i == pads - 1
				var angle := turn + float(i) * 2.4
				var out := 0.0 if top else radius * (0.52 - 0.14 * t)
				var r := radius * (0.36 if top else 0.52 - 0.14 * t)
				var v := height * 0.12
				result.append([Vector3(cos(angle) * out, base + v + (height - 2.0 * v) * t, sin(angle) * out), Vector3(r, v, r * 0.85)])
	return result

## Lobes and unsqueezed hulls of a tree variant (a pure function of the
## constants), kept for its `crown` step: `hulls` lists [lobe the hull grows
## from, near shell points, far points, near and far icosphere detail]. A
## broadleaf crown is one hull round its lobes; a pine one per needle pad, so
## the tiers stay apart. The median-bed pine (2) shares the mature pine's shape.
static func shape(species: String, variant: int) -> Dictionary:
	var f := form(species, variant)
	var seed := 500 + SPECIES.find(species) * 10 + (1 if variant == 1 else 0)
	var lobe_list := species_lobes(species, seed, f.x, f.y, f.z)
	var lift := f.x + f.y * 0.08
	var hulls := []
	if species == "pine":
		for i in range(1, lobe_list.size()):
			hulls.append([lobe_list[i], VEG._hull(seed + i, [lobe_list[i]], 1, VEG.SHELL, lift), VEG._hull(seed + i, [lobe_list[i]], 0, 0.98, lift), 1, 0])
	else:
		hulls.append([lobe_list[0], VEG._hull(seed, lobe_list, 2, VEG.SHELL, lift), VEG._hull(seed, lobe_list, 1, 0.98, lift), 2, 1])
	return {"species": species, "variant": variant, "base": f.x, "height": f.y, "radius": f.z, "seed": seed, "lobes": lobe_list, "hulls": hulls}

## Shape with leaf cards and the horizontal reach fit (cached while the
## variant's meshes build; its `far` mesh releases it).
static func crown(species: String, variant: int) -> Dictionary:
	var key := "%s_%d" % [species, 0 if variant == 2 else variant]
	if _crowns.has(key): return _crowns[key]
	var data: Dictionary = _shapes.get(key, {})
	_shapes.erase(key)
	if data.is_empty(): data = shape(species, 0 if variant == 2 else variant)
	var young := variant == 1
	var base: float = data.base
	var height: float = data.height
	# Pine cards sit on the needle pads, not the slim core.
	var card_lobes: Array = data.lobes.slice(1) if species == "pine" else data.lobes
	var cards := VEG._card_list(data.seed, card_lobes, int(float(CARDS[species]) * (0.55 if young else 1.0)),
		float(CARD_SIZE[species]) * (0.85 if young else 1.0), base, height, data.radius)
	for card: Array in cards: card[2] = TILE[species]
	data.cards = cards
	# Every hull vertex with its sway, the shell grown back on low (still) and
	# every card corner with its sway and flutter inside the reach.
	var peak := VEG.SWAY_PEAK * VEG.TREE_SWAY
	var still := reach_limit(species, data.variant)
	var fit := 1.0
	for hull: Array in data.hulls:
		for part in [1, 2]:
			for p: Vector3 in hull[part]:
				var r := Vector2(p.x, p.z).length()
				if r < 0.001: continue
				fit = minf(fit, reach_limit(species, data.variant, VEG.sway(p.y, base, height) * peak) / r)
				if part == 1: fit = minf(fit, still * VEG.SHELL / r)
	for card: Array in cards:
		for p: Vector3 in card[0]:
			var r := Vector2(p.x, p.z).length()
			if r > 0.001: fit = minf(fit, reach_limit(species, data.variant, sway_reach(p.y, base, height, true)) / r)
	var squeeze := Vector3(fit, 1.0, fit)
	for hull: Array in data.hulls:
		for part in [1, 2]:
			var points: PackedVector3Array = hull[part]
			for i in points.size():
				points[i] *= squeeze
				if Vector2(points[i].x, points[i].z).length() > AXIS_LIMIT - 0.02: points[i].y = maxf(points[i].y, FLOOR)
			hull[part] = points
	for card: Array in cards:
		var corners: PackedVector3Array = card[0]
		for i in 4:
			corners[i] *= squeeze
			corners[i].y = maxf(corners[i].y, FLOOR)
		card[0] = corners
	for lobe: Array in data.lobes:
		lobe[0] = (lobe[0] as Vector3) * squeeze
		lobe[1] = (lobe[1] as Vector3) * squeeze
	data.radius = float(data.radius) * fit
	data.fit = fit
	_crowns[key] = data
	return data

## Trunk tubes of a variant as lists of axis rings ([centre, radius]):
## mature trees one tube (TRUNK_SIDES) from below the pit into the crown;
## young ones the burlap wrap (TRUNK_SIDES) closing onto the stem and the
## stem (UPPER_SIDES). The rings up to LOWER_TOP enclose the trunk box.
static func trunk(species: String, variant: int) -> Array:
	var f := form(species, variant)
	var base := f.x
	var h := f.y
	var lower := []
	var upper := []
	if variant == 1:
		for y in [-0.06, 0.0, 0.12, 0.24, 0.36, 0.48, 0.6]: lower.append(Vector2(y, 0.112))
		lower.append(Vector2(0.625, 0.05))
		upper = [Vector2(0.6, 0.048), Vector2(0.9, 0.045), Vector2(1.2, 0.042), Vector2(base, 0.04), Vector2(base + h * 0.4, 0.028), Vector2(base + h * 0.85, 0.01)]
	else:
		lower = [Vector2(-0.12, 0.15), Vector2(0.03, 0.125), Vector2(0.15, 0.124), Vector2(0.3, 0.122), Vector2(0.45, 0.12), Vector2(0.6, 0.119), Vector2(LOWER_TOP, 0.118)]
		match species:
			"ginkgo": upper = [Vector2(0.66, 0.112), Vector2(0.82, 0.108), Vector2(0.98, 0.104), Vector2(1.14, 0.1), Vector2(base, 0.092), Vector2(base + h * 0.3, 0.068), Vector2(base + h * 0.6, 0.042), Vector2(base + h * 0.93, 0.012)]
			"plane": upper = [Vector2(0.66, 0.112), Vector2(0.82, 0.11), Vector2(0.98, 0.108), Vector2(1.14, 0.106), Vector2(1.3, 0.104), Vector2(base, 0.1), Vector2(base + h * 0.3, 0.075), Vector2(base + h * 0.6, 0.045), Vector2(base + h * 0.85, 0.015)]
			"zelkova": upper = [Vector2(0.66, 0.112), Vector2(0.8, 0.112), Vector2(0.95, 0.114), Vector2(1.1, 0.07), Vector2(base + h * 0.35, 0.04)]
			"cherry": upper = [Vector2(0.66, 0.112), Vector2(0.78, 0.115), Vector2(0.88, 0.09), Vector2(1.0, 0.06), Vector2(base + h * 0.25, 0.03)]
			_:
				lower = [Vector2(-0.12, 0.155), Vector2(0.03, 0.132), Vector2(0.15, 0.131), Vector2(0.3, 0.13), Vector2(0.45, 0.129), Vector2(0.6, 0.128), Vector2(LOWER_TOP, 0.128)]
				upper = [Vector2(0.66, 0.118), Vector2(0.82, 0.112), Vector2(0.98, 0.108), Vector2(1.14, 0.104), Vector2(1.3, 0.1), Vector2(base + h * 0.35, 0.07), Vector2(base + h * 0.7, 0.04), Vector2(base + h - 0.05, 0.012)]
	var lean := pine_lean() if species == "pine" and variant != 1 else Vector3.ZERO
	var rings := func(list: Array) -> Array:
		var out := []
		for ring: Vector2 in list: out.append([Vector3(0.0, ring.x, 0.0) + lean * pine_offset(ring.x), ring.y])
		return out
	if variant == 1: return [rings.call(lower), rings.call(upper)]
	var whole := lower.duplicate()
	for ring: Vector2 in upper:
		if ring.x > LOWER_TOP + 0.01: whole.append(ring)
	return [rings.call(whole)]

## Direction the pine trunk curves toward.
static func pine_lean() -> Vector3:
	var a := VEG.unit(SPECIES.find("pine") * 10 + 500, 7) * TAU
	return Vector3(cos(a), 0.0, sin(a))

## Sideways offset (metres) of the curved pine trunk at height `y`: at most
## 6 cm at 1.4 m.
static func pine_offset(y: float) -> float:
	var t := clampf(y / 1.4, 0.0, 1.0)
	return 0.06 * t * t + maxf(0.0, y - 1.4) * 0.03

## Bark colour (linear, alpha 0) of a trunk vertex: species pattern, a young
## stem lighter and smoother, darker toward the ground.
static func bark(species: String, young: bool, p: Vector3, ring: int, side: int, seed: int) -> Color:
	var noise := VEG.unit(seed, ring * 32 + side, 33)
	var c := linear(BARK[species])
	match species:
		"ginkgo":
			# Vertical fissures every third side.
			c = linear(BARK[species], (0.68 if side % 3 == 0 else 1.0) * (0.92 + 0.16 * noise))
		"zelkova":
			# Smooth grey with orange-brown flake patches.
			c = linear("a0623c" if VEG.unit(seed, ring / 2 * 32 + side / 2, 34) < 0.2 else BARK[species], 0.94 + 0.12 * noise)
		"plane":
			# Camouflage: cream, olive and grey patches.
			var patch := VEG.unit(seed, ring / 2 * 64 + (side + ring % 2) / 2, 35)
			c = linear("c8c2a8" if patch < 0.34 else ("8a8b6d" if patch < 0.7 else "6b6656"))
		"cherry":
			# Dark bark with pale horizontal lenticel bands.
			c = linear("7d6a5e" if ring % 3 == 1 and p.y < 1.3 else BARK[species], 0.92 + 0.14 * noise)
		_:
			# Grey plated below, red-orange above.
			c = linear(BARK[species]).lerp(linear("93603f"), clampf((p.y - 0.5) / 0.9, 0.0, 1.0)) * (0.78 if noise < 0.3 else 1.0)
	if young: c = c.lerp(linear("8a7a68"), 0.35)
	var shade := lerpf(0.62, 1.0, clampf(p.y / 1.0, 0.0, 1.0))
	return Color(c.r * shade, c.g * shade, c.b * shade, 0.0)

## Branches of a variant: [points, radii] tubes (five sides) under the crown.
static func branches(species: String, variant: int, data: Dictionary) -> Array:
	var base: float = data.base
	var h: float = data.height
	var radius: float = data.radius
	var seed: int = data.seed
	var turn := VEG.unit(seed, 9) * TAU
	var result := []
	if variant == 1:
		# Young: three thin twigs into the crown.
		for i in 3:
			var a := turn + TAU * float(i) / 3.0
			var start := Vector3(0.0, base + h * (0.12 + 0.12 * i), 0.0)
			result.append([[start, start + Vector3(cos(a) * radius * 0.5, h * 0.3, sin(a) * radius * 0.5)], [0.018, 0.006]])
		return result
	match species:
		"ginkgo":
			# Three whorls of three short upswept branches.
			for w in 3:
				var y := base + h * (0.14 + 0.24 * w)
				var reach := radius * (1.0 - 0.62 * (0.14 + 0.24 * w)) * 0.7
				for i in 3:
					var a := turn + TAU * float(i) / 3.0 + 0.7 * w
					var d := Vector3(cos(a), 0.0, sin(a))
					var start := Vector3(0.0, y, 0.0)
					var end := start + d * reach + Vector3.UP * (0.1 + 0.4 * reach)
					result.append([[start, start.lerp(end, 0.5) + Vector3.UP * 0.03, end], [0.032, 0.02, 0.008]])
		"zelkova":
			# Five ascending limbs from the fork (vase).
			for i in 5:
				var a := turn + TAU * float(i) / 5.0 + (VEG.unit(seed, i, 10) - 0.5) * 0.5
				var d := Vector3(cos(a), 0.0, sin(a))
				var start := Vector3(0.0, 0.9 + 0.04 * i, 0.0)
				result.append([[start, d * 0.15 + Vector3.UP * 1.42, d * radius * 0.55 + Vector3.UP * (base + h * 0.32), d * radius * 0.72 + Vector3.UP * (base + h * 0.55)], [0.075, 0.06, 0.038, 0.014]])
		"cherry":
			# Four limbs spreading from the low fork.
			for i in 4:
				var a := turn + TAU * float(i) / 4.0 + (VEG.unit(seed, i, 10) - 0.5) * 0.6
				var d := Vector3(cos(a), 0.0, sin(a))
				var start := Vector3(0.0, 0.8 + 0.03 * i, 0.0)
				result.append([[start, d * 0.17 + Vector3.UP * 1.36, d * radius * 0.62 + Vector3.UP * (base + h * 0.3), d * radius * 0.8 + Vector3.UP * (base + h * 0.45)], [0.07, 0.055, 0.034, 0.012]])
		"plane":
			for i in 3:
				var a := turn + TAU * float(i) / 3.0
				var d := Vector3(cos(a), 0.0, sin(a))
				var start := Vector3(0.0, base + h * 0.1 + 0.12 * i, 0.0)
				result.append([[start, start + d * radius * 0.25 + Vector3.UP * 0.25, start + d * radius * 0.6 + Vector3.UP * 0.55], [0.05, 0.035, 0.014]])
		_:
			# Pine: one branch out to each pad but the top one.
			var lobes: Array = data.lobes
			var lean := pine_lean()
			for i in range(1, lobes.size() - 1):
				var pad: Vector3 = lobes[i][0]
				var y := maxf(pad.y - 0.15, base + 0.02)
				var start := Vector3(0.0, y, 0.0) + lean * pine_offset(y)
				var end := Vector3(pad.x * 0.75, pad.y, pad.z * 0.75)
				result.append([[start, start.lerp(end, 0.5) + Vector3.UP * 0.04, end], [0.035, 0.022, 0.01]])
	return result

## Pit fixtures (untinted, still): a granite frame round a cast-iron grate
## with radial slots, mulch and gravel, a liriope ring, or a soil mound in a
## ring of stones.
static func pit(b: VEG.Builder, kind: String, seed: int) -> void:
	if kind != "mound":
		# Four frame stones: top, outer chamfer and side, inner side.
		for k in 4:
			var shade := 0.9 + 0.14 * VEG.unit(seed, k, 50)
			var stone := linear(GRANITE, shade)
			var turn := Basis(Vector3.UP, -PI / 2.0 * k)
			var o := [Vector3(-PIT_HALF, 0, -PIT_HALF), Vector3(PIT_HALF, 0, -PIT_HALF)]
			var i := [Vector3(-PIT_INNER, 0, -PIT_INNER), Vector3(PIT_INNER, 0, -PIT_INNER)]
			var edge := PIT_HALF - 0.004
			var e := [Vector3(-edge, 0, -edge), Vector3(edge, 0, -edge)]
			var up := Vector3.UP
			face(b, [turn * (i[0] + up * PIT_TOP), turn * (i[1] + up * PIT_TOP), turn * (e[1] + up * PIT_TOP), turn * (e[0] + up * PIT_TOP)], Vector3.UP, stone)
			face(b, [turn * (e[0] + up * PIT_TOP), turn * (e[1] + up * PIT_TOP), turn * (o[1] + up * (PIT_TOP - 0.004)), turn * (o[0] + up * (PIT_TOP - 0.004))], (turn * Vector3(0, 1, -1)).normalized(), stone)
			face(b, [turn * (o[0] + up * (PIT_TOP - 0.004)), turn * (o[1] + up * (PIT_TOP - 0.004)), turn * (o[1] + up * PIT_BOTTOM), turn * (o[0] + up * PIT_BOTTOM)], turn * Vector3.FORWARD, linear(GRANITE, shade * 0.85))
			face(b, [turn * (i[0] + up * PIT_TOP), turn * (i[1] + up * PIT_TOP), turn * (i[1] + up * (GRATE_TOP - 0.012)), turn * (i[0] + up * (GRATE_TOP - 0.012))], turn * Vector3.BACK, linear(GRANITE, shade * 0.7))
	match kind:
		"grate":
			# A cast-iron plate from the trunk hole to the frame with 24 radial
			# slots widening outward over dark soil.
			var first := b.vertices.size()
			for k in 24:
				var a := TAU * float(k) / 24.0
				b.add(_ray(a, GRATE_HOLE, GRATE_TOP), Vector3.UP, linear(IRON, 0.85 + 0.2 * VEG.unit(seed, k, 51)), Vector2.ZERO, Vector2.ZERO)
				b.add(_square(a, PIT_INNER, GRATE_TOP), Vector3.UP, linear(IRON, 0.9 + 0.2 * VEG.unit(seed, k, 52)), Vector2.ZERO, Vector2.ZERO)
			_ring(b, first, 24)
			for k in 24:
				var a := TAU * (float(k) + 0.5) / 24.0
				var d := Vector3(cos(a), 0.0, sin(a))
				var t := Vector3(-sin(a), 0.0, cos(a))
				var outer := Vector2(_square(a, PIT_INNER, 0.0).x, _square(a, PIT_INNER, 0.0).z).length() - 0.007
				var y := GRATE_TOP + 0.0008
				face(b, [d * 0.143 - t * 0.002 + Vector3.UP * y, d * 0.143 + t * 0.002 + Vector3.UP * y, d * outer + t * 0.0035 + Vector3.UP * y, d * outer - t * 0.0035 + Vector3.UP * y], Vector3.UP, linear("1a1714"))
		"mulch", "liriope":
			# Bark mulch and gravel (plane) or dark soil (liriope).
			var colors := ["6b4a32", "8a6a4a", "9a958c"] if kind == "mulch" else [SOIL, "3c2e22", SOIL]
			var first := b.vertices.size()
			for k in 16:
				var a := TAU * float(k) / 16.0
				for r in 2:
					var p := _ray(a, GRATE_HOLE - 0.01, 0.036) if r == 0 else _square(a, PIT_INNER, 0.038)
					b.add(p, Vector3.UP, linear(colors[int(VEG.unit(seed, k * 2 + r, 52) * 3.0) % 3], 0.85 + 0.2 * VEG.unit(seed, k, 53)), Vector2.ZERO, Vector2.ZERO)
			_ring(b, first, 16)
			if kind == "liriope":
				# Eight tufts of arching strap leaves round the trunk.
				for k in 8:
					var a := TAU * (float(k) + 0.5) / 8.0
					var root := _ray(a, 0.14, 0.036)
					for blade_index in 3:
						var out := Vector3(cos(a), 0.0, sin(a)).rotated(Vector3.UP, (float(blade_index) - 1.0) * 0.6) * 0.5
						blade(b, root, out, 0.045 + 0.01 * VEG.unit(seed, k, blade_index), 0.012, linear("2f4a2a", 0.9 + 0.2 * VEG.unit(seed, k, 54)), 0.0, 0.02)
		"mound":
			# Soil mound rising to the trunk inside a ring of ten stones.
			var first := b.vertices.size()
			for k in 12:
				var a := TAU * float(k) / 12.0
				b.add(_ray(a, 0.15, 0.034), Vector3(cos(a), 2.0, sin(a)).normalized(), linear(SOIL, 0.9), Vector2.ZERO, Vector2.ZERO)
				b.add(_ray(a, 0.12, 0.06), Vector3(cos(a), 3.0, sin(a)).normalized(), linear("5a4632"), Vector2.ZERO, Vector2.ZERO)
			for k in 12:
				var a := first + k * 2
				var c := first + ((k + 1) % 12) * 2
				b.triangle(a, c, c + 1); b.triangle(a, c + 1, a + 1)
			for k in 10:
				var a := TAU * (float(k) + VEG.unit(seed, k, 55) * 0.3) / 10.0
				var centre := _ray(a, 0.155, 0.036)
				var start := b.vertices.size()
				var ico: Array = VEG.icosphere(0)
				var points: PackedVector3Array = ico[0]
				var faces: PackedInt32Array = ico[1]
				var grey := linear("9c968c", 0.8 + 0.3 * VEG.unit(seed, k, 56))
				for p in points:
					var q := centre + p * Vector3(0.02, 0.016, 0.02) * (0.85 + 0.25 * VEG.unit(seed, k, 57))
					q.y = maxf(q.y, 0.03)
					b.add(q, p, grey, Vector2.ZERO, Vector2.ZERO)
				for f in range(0, faces.size(), 3): b.triangle(start + faces[f], start + faces[f + 2], start + faces[f + 1])

## Upward band between pairs of vertices (inner, outer) round a ring.
static func _ring(b: VEG.Builder, first: int, count: int) -> void:
	for k in count:
		var a := first + k * 2
		var c := first + ((k + 1) % count) * 2
		b.triangle(a, c + 1, c); b.triangle(a, a + 1, c + 1)

static func _ray(angle: float, radius: float, y: float) -> Vector3:
	return Vector3(cos(angle) * radius, y, sin(angle) * radius)

## Where the ray at `angle` leaves the square of half size `half`.
static func _square(angle: float, half: float, y: float) -> Vector3:
	var d := Vector2(cos(angle), sin(angle))
	var t := half / maxf(absf(d.x), absf(d.y))
	return Vector3(d.x * t, y, d.y * t)

## Young tree stakes: three stakes from inside the pit (0.16 m out)
## leaning inward to the stem, two rails between them at 0.35 and 0.75 m and
## rubber ties to the stem at 1.0 m.
static func stakes(b: VEG.Builder, seed: int) -> void:
	var feet := []
	var tops := []
	for i in 3:
		var a := PI / 6.0 + TAU * float(i) / 3.0
		var d := Vector3(cos(a), 0.0, sin(a))
		feet.append(d * 0.16 + Vector3.DOWN * 0.03)
		tops.append(d * 0.062 + Vector3.UP * 1.1)
	var wood := func(p: Vector3, _r: int, side: int) -> Color:
		return linear(STAKE, (0.82 + 0.12 * float(side % 2)) * lerpf(0.7, 1.0, clampf(p.y, 0.0, 1.0)))
	for i in 3: tube(b, [feet[i], tops[i]], [0.015, 0.014], 5, wood, 9.0, 1.0, 0.0)
	for level in [0.35, 0.75]:
		for i in 3:
			var a: Vector3 = (feet[i] as Vector3).lerp(tops[i], (level + 0.03) / 1.13)
			var c: Vector3 = (feet[(i + 1) % 3] as Vector3).lerp(tops[(i + 1) % 3], (level + 0.03) / 1.13)
			tube(b, [a, c], [0.011, 0.011], 5, wood, 9.0, 1.0, 0.0)
	var tie := func(_p: Vector3, _r: int, _s: int) -> Color: return linear(TIE)
	for i in 3:
		var stake: Vector3 = (feet[i] as Vector3).lerp(tops[i], 1.03 / 1.13)
		tube(b, [Vector3(stake.x * 0.4, stake.y, stake.z * 0.4), stake], [0.008, 0.008], 4, tie, 9.0, 1.0, 0.0)
	tube(b, [Vector3(0, 0.99, 0), Vector3(0, 1.02, 0)], [0.05, 0.05], 8, tie, 9.0, 1.0, 0.0)

## Hull surfaces of a crown (`part` 1 the near shell, 2 the far hull), each
## shell growing back about the axis at its lobe's height on low.
static func _crowns_surface(builder: VEG.Builder, data: Dictionary, part: int) -> void:
	for hull: Array in data.hulls:
		var single := {"base": data.base, "height": data.height, "radius": data.radius, "seed": data.seed, "lobes": [hull[0]]}
		VEG._crown_surface(builder, single, hull[part], hull[2 + part], 1.0, part == 1)

static func tree_mesh(species: String, part: String, variant: int, materials: Dictionary) -> ArrayMesh:
	var data := crown(species, variant)
	var base: float = data.base
	var height: float = data.height
	var seed: int = data.seed
	var young := variant == 1
	var builder := VEG.Builder.new()
	var mesh := ArrayMesh.new()
	var paint := func(p: Vector3, ring: int, side: int) -> Color: return bark(species, young, p, ring, side, seed)
	match part:
		"body":
			var rings := trunk(species, variant)
			for tube_index in rings.size():
				var points := []
				var radii := []
				for ring: Array in rings[tube_index]:
					points.append(ring[0]); radii.append(ring[1])
				if young and tube_index == 0:
					# Burlap wrap: diagonal bands of the weave.
					var wrap := func(p: Vector3, ring: int, side: int) -> Color:
						return linear(WRAP, (0.78 if (side + ring) % 2 == 0 else 1.0) * (0.9 + 0.12 * VEG.unit(seed, ring, side)) * (1.0 if p.y < 0.61 else 0.75))
					tube(builder, points, radii, TRUNK_SIDES, wrap, base, height)
				else:
					tube(builder, points, radii, TRUNK_SIDES if tube_index == 0 else UPPER_SIDES, paint, base, height)
			for limb: Array in branches(species, variant, data):
				tube(builder, limb[0], limb[1], 5, paint, base, height)
			if young: stakes(builder, seed)
			if variant != 2: pit(builder, PITS[species], seed)
			_crowns_surface(builder, data, 1)
			builder.commit(mesh, materials.get("body"))
		"cards":
			VEG._cards(builder, data, 1.0)
			builder.commit(mesh, materials.get("cards"))
		_:
			var radii := [0.11, 0.05, 0.035] if young else [0.13, 0.1, 0.07]
			tube(builder, [Vector3(0.0, -0.1, 0.0), Vector3(0.0, base - 0.1, 0.0), Vector3(0.0, base + 0.3, 0.0)], radii, 6, paint, base, height)
			_crowns_surface(builder, data, 2)
			if variant != 2:
				var plate: Color = {"grate": linear(IRON), "mulch": linear("6b4a32"), "liriope": linear("2f4a2a"), "mound": linear(SOIL)}[PITS[species]]
				face(builder, [Vector3(-PIT_HALF, PIT_TOP, -PIT_HALF), Vector3(PIT_HALF, PIT_TOP, -PIT_HALF), Vector3(PIT_HALF, PIT_TOP, PIT_HALF), Vector3(-PIT_HALF, PIT_TOP, PIT_HALF)], Vector3.UP, plate)
			builder.commit(mesh, materials.get("body"))
	mesh.resource_name = "mk_street_tree_%s_%s_%d" % [species, part, variant]
	# `far` is the variant's last part: its crown is not kept for the process
	# lifetime (a later build recomputes it identically).
	if part == "far": _crowns.erase("%s_%d" % [species, variant])
	return mesh

# Flower beds ------------------------------------------------------------------------

## Outer half size (x, z) and top of a piece's rim, the wall thickness and
## the soil level.
static func bed_frame(piece: String) -> Dictionary:
	var size: Vector3 = PIECES[piece]
	var wall: float = RIM_WALL[piece]
	var top := size.y + RIM_OUT
	return {"half": Vector2(size.x * 0.5 + RIM_OUT, size.z * 0.5 + RIM_OUT), "top": top, "wall": wall, "soil": top - SOIL_DROP,
		"inner": Vector2(size.x * 0.5 + RIM_OUT - wall, size.z * 0.5 + RIM_OUT - wall)}

## Rim walls of a piece in `style` and its soil.
static func rim(piece: String, style: int, materials: Dictionary) -> ArrayMesh:
	var frame := bed_frame(piece)
	var half: Vector2 = frame.half
	var top: float = frame.top
	var wall: float = frame.wall
	var soil: float = frame.soil
	var seed := 700 + style * 10 + PIECES.keys().find(piece)
	var b := VEG.Builder.new()
	# Walls: two long ones (local x) over the full length, two short ones between.
	var walls := [[Vector3(-half.x, -RIM_SKIRT, -half.y), Vector3(half.x, top, -half.y + wall), 2.0 * half.x],
		[Vector3(-half.x, -RIM_SKIRT, half.y - wall), Vector3(half.x, top, half.y), 2.0 * half.x],
		[Vector3(-half.x, -RIM_SKIRT, -half.y + wall), Vector3(-half.x + wall, top, half.y - wall), 2.0 * (half.y - wall)],
		[Vector3(half.x - wall, -RIM_SKIRT, -half.y + wall), Vector3(half.x, top, half.y - wall), 2.0 * (half.y - wall)]]
	match style:
		0:
			# Granite blocks (median: curb stones about 30 cm long), chamfered.
			for w in 4:
				var low: Vector3 = walls[w][0]
				var high: Vector3 = walls[w][1]
				var along_x := w < 2
				var count := maxi(1, roundi(float(walls[w][2]) / (0.3 if piece == "median_bed" else 0.27)))
				for s in count:
					var shade := 0.88 + 0.16 * VEG.unit(seed, w * 32 + s, 60)
					var stone := func(p: Vector3) -> Color: return linear(GRANITE, shade * (0.8 if p.y < top - 0.01 else 1.0) * (0.96 + 0.06 * VEG.unit(seed, roundi(p.x * 500.0), roundi(p.z * 500.0))))
					var a := low
					var c := high
					if along_x:
						a.x = lerpf(low.x, high.x, float(s) / count); c.x = lerpf(low.x, high.x, float(s + 1) / count)
					else:
						a.z = lerpf(low.z, high.z, float(s) / count); c.z = lerpf(low.z, high.z, float(s + 1) / count)
					block(b, a, c, 0.005, stone, Vector2i(1, 1), segment_sides(s, count, along_x))
		1:
			# Brick: mortar walls with stretcher bricks proud of the outer
			# faces and a soldier-course cap of bricks on edge.
			var cap := 0.03
			var mortar := func(_p: Vector3) -> Color: return linear("a89c8c")
			for w in 4:
				var high: Vector3 = walls[w][1]
				block(b, walls[w][0], Vector3(high.x, top, high.z), 0.0, mortar)
				var low: Vector3 = walls[w][0]
				var along_x := w < 2
				var length: float = walls[w][2]
				var count := maxi(2, roundi(length / 0.06))
				# Soldier bricks: their tops and outer faces over the mortar.
				var outward := Vector3(0, 0, -1 if w == 0 else 1) if along_x else Vector3(-1 if w == 2 else 1, 0, 0)
				for s in count:
					var u0 := float(s) / count + 0.004 / length
					var u1 := float(s + 1) / count - 0.004 / length
					var a := Vector3(lerpf(low.x, high.x, u0) if along_x else low.x - 0.0015, top + 0.001, low.z - 0.0015 if along_x else lerpf(low.z, high.z, u0))
					var c := Vector3(lerpf(low.x, high.x, u1) if along_x else high.x + 0.0015, top + 0.001, high.z + 0.0015 if along_x else lerpf(low.z, high.z, u1))
					var tone := linear("a8503a" if VEG.unit(seed, w * 64 + s, 61) < 0.7 else "8a3f2e", 0.88 + 0.2 * VEG.unit(seed, w * 64 + s, 62))
					face(b, [a, Vector3(c.x, a.y, a.z), c, Vector3(a.x, a.y, c.z)], Vector3.UP, tone)
					var plane := (a.z if w == 0 else c.z) if along_x else (a.x if w == 2 else c.x)
					var p0 := Vector3(a.x, top - cap, plane) if along_x else Vector3(plane, top - cap, a.z)
					var p1 := Vector3(c.x, top - cap, plane) if along_x else Vector3(plane, top - cap, c.z)
					face(b, [p0, p1, p1 + Vector3.UP * (cap + 0.001), p0 + Vector3.UP * (cap + 0.001)], outward, linear("a8503a", 0.8) * (tone.r / linear("a8503a").r))
			# Stretcher courses on the outer faces (running bond).
			var courses := 3
			var course_h := (top - cap + RIM_SKIRT) / courses
			for w in 4:
				var low: Vector3 = walls[w][0]
				var high: Vector3 = walls[w][1]
				var along_x := w < 2
				var outward := Vector3(0, 0, -1 if w == 0 else 1) if along_x else Vector3(-1 if w == 2 else 1, 0, 0)
				var length := (high.x - low.x) if along_x else (high.z - low.z) + 2.0 * wall
				var start := (low.x if along_x else low.z - wall)
				var plane := (low.z if w == 0 else high.z) if along_x else (low.x if w == 2 else high.x)
				var count := maxi(2, roundi(length / 0.1))
				for course in courses:
					var y0 := -RIM_SKIRT + course * course_h + 0.0025
					var y1 := y0 + course_h - 0.005
					var offset := 0.5 if course % 2 == 1 else 0.0
					for s in count + 1:
						var u0 := clampf((float(s) - offset) / count, 0.0, 1.0) * length + start + 0.0025
						var u1 := clampf((float(s) + 1.0 - offset) / count, 0.0, 1.0) * length + start - 0.0025
						if u1 <= u0 + 0.004: continue
						var lift := outward * 0.0015
						var p := [Vector3(u0, y0, plane) if along_x else Vector3(plane, y0, u0), Vector3(u1, y0, plane) if along_x else Vector3(plane, y0, u1),
							Vector3(u1, y1, plane) if along_x else Vector3(plane, y1, u1), Vector3(u0, y1, plane) if along_x else Vector3(plane, y1, u0)]
						for k in 4: p[k] += lift
						var brick := linear("b05a40" if VEG.unit(seed, w * 997 + course * 31 + s, 63) < 0.65 else "92452f", 0.85 + 0.25 * VEG.unit(seed, w * 997 + course * 31 + s, 64))
						face(b, p, outward, brick)
		2:
			# Exposed-aggregate concrete: speckled grids, small chamfer.
			var speckle := func(p: Vector3) -> Color:
				var n := VEG.unit(roundi(p.x * 300.0) + seed, roundi(p.y * 300.0), roundi(p.z * 300.0))
				return linear("bdb6a8" if n < 0.55 else ("8f897e" if n < 0.85 else "d8d0c0"), 0.9 + 0.12 * n)
			for w in 4:
				var length: float = walls[w][2]
				block(b, walls[w][0], walls[w][1], 0.006, speckle, Vector2i(maxi(2, roundi(length / 0.09)), 2))
		_:
			# Painted box: dark green walls with vertical seams, a top lip, a
			# dark plinth with lighter feet at the corners.
			var paint := func(p: Vector3) -> Color: return linear("2f4a3a", 0.8 if p.y < 0.02 else 1.0)
			for w in 4: block(b, walls[w][0], walls[w][1], 0.003, paint)
			# A top lip round the rim, 4 mm proud.
			var lip := 0.004
			var green := func(_p: Vector3) -> Color: return linear("355542")
			block(b, Vector3(-half.x - lip, top - 0.012, -half.y - lip), Vector3(half.x + lip, top + 0.002, -half.y + wall), 0.002, green)
			block(b, Vector3(-half.x - lip, top - 0.012, half.y - wall), Vector3(half.x + lip, top + 0.002, half.y + lip), 0.002, green)
			block(b, Vector3(-half.x - lip, top - 0.012, -half.y + wall), Vector3(-half.x + wall, top + 0.002, half.y - wall), 0.002, green)
			block(b, Vector3(half.x - wall, top - 0.012, -half.y + wall), Vector3(half.x + lip, top + 0.002, half.y - wall), 0.002, green)
			var proud := 0.001
			for side in [-1.0, 1.0]:
				var z: float = side * (half.y + proud)
				var x: float = side * (half.x + proud)
				for t in [-1.0 / 3.0, 1.0 / 3.0]:
					var u: float = t * half.x
					face(b, [Vector3(u - 0.002, 0.02, z), Vector3(u + 0.002, 0.02, z), Vector3(u + 0.002, top - 0.012, z), Vector3(u - 0.002, top - 0.012, z)], Vector3(0, 0, side), linear("1e3328"))
				# Dark plinth band.
				face(b, [Vector3(-half.x, -RIM_SKIRT, z), Vector3(half.x, -RIM_SKIRT, z), Vector3(half.x, 0.02, z), Vector3(-half.x, 0.02, z)], Vector3(0, 0, side), linear("2a2c2a"))
				face(b, [Vector3(x, -RIM_SKIRT, -half.y), Vector3(x, -RIM_SKIRT, half.y), Vector3(x, 0.02, half.y), Vector3(x, 0.02, -half.y)], Vector3(side, 0, 0), linear("2a2c2a"))
			# Feet at the corners, in front of the plinth.
			var foot := func(_p: Vector3) -> Color: return linear("4a4e4a")
			for cx in [-1.0, 1.0]:
				for cz in [-1.0, 1.0]:
					var x0: float = half.x - 0.04 if cx > 0.0 else -half.x - 0.002
					var z0: float = half.y - 0.04 if cz > 0.0 else -half.y - 0.002
					block(b, Vector3(x0, -RIM_SKIRT, z0), Vector3(x0 + 0.042, 0.02, z0 + 0.042), 0.0, foot)
	# Soil with crumbs of shade.
	var inner: Vector2 = frame.inner
	grid(b, Vector3(-inner.x, soil, -inner.y), Vector3(2.0 * inner.x, 0, 0), Vector3(0, 0, 2.0 * inner.y), maxi(2, roundi(inner.x / 0.1)), 2, Vector3.UP,
		func(p: Vector3) -> Color: return linear(SOIL, 0.75 + 0.35 * VEG.unit(roundi(p.x * 200.0), roundi(p.z * 200.0), seed)))
	var mesh := ArrayMesh.new()
	b.commit(mesh, materials.get("body"))
	mesh.resource_name = "mk_bed_%s_rim_%d" % [piece, style]
	return mesh

## Planting of a piece (see PLANTING_LEAVES): leaves tinted per instance
## (COLOR.a 1, ambient occlusion in the vertex colour), flowers and liriope
## untinted.
static func planting(piece: String, kind: int, materials: Dictionary) -> ArrayMesh:
	var frame := bed_frame(piece)
	var inner: Vector2 = frame.inner
	var soil: float = frame.soil
	var seed := 800 + kind * 10 + PIECES.keys().find(piece)
	var b := VEG.Builder.new()
	var ao := func(p: Vector3, _n: Vector3) -> Color:
		var shade := lerpf(0.5, 1.0, clampf((p.y - soil) / 0.15, 0.0, 1.0))
		return Color(shade, shade, shade, 1.0)
	if piece == "median_bed":
		# A clipped hedge with rounded ends, with or without an edge of annuals.
		var azalea := kind < 2
		var edge := kind % 2 == 0
		var depth := inner.y - (0.085 if edge else 0.03)
		var hedge := Vector3(inner.x - 0.03, 0.065, depth)
		rounded(b, Vector3(0.0, soil + hedge.y, 0.0), hedge, Vector3(0.15, 0.05, 0.12), Vector3i(18, 2, 3), seed, 0.012, 1.0, ao,
			func(p: Vector3) -> float: return 0.06 * clampf((p.y - soil) / 0.13, 0.0, 1.0))
		if azalea:
			for i in 40:
				var x := lerpf(-hedge.x + 0.1, hedge.x - 0.1, (float(i) + VEG.unit(seed, i, 70)) / 40.0)
				var z := (VEG.unit(seed, i, 71) - 0.5) * 2.0 * (depth - 0.06)
				var y := soil + 2.0 * hedge.y + 0.004
				flower(b, Vector3(x, y, z), Vector3(VEG.unit(seed, i, 72) - 0.5, 2.5, VEG.unit(seed, i, 73) - 0.5).normalized(), 0.022, linear(AZALEA, 0.9 + 0.2 * VEG.unit(seed, i, 74)), seed * 97 + i, 0.06)
		if edge:
			for side in [-1.0, 1.0]:
				var z: float = side * (inner.y - 0.045)
				rounded(b, Vector3(0.0, soil + 0.02, z), Vector3(inner.x - 0.04, 0.02, 0.03), Vector3(0.03, 0.02, 0.03), Vector3i(10, 1, 1), seed + 5, 0.008, 1.0, ao)
				for i in 16:
					var color: String = FLOWERS[(int(VEG.unit(seed, i / 3, 75) * FLOWERS.size()) + (1 if side > 0 else 0)) % FLOWERS.size()]
					var x := lerpf(-inner.x + 0.08, inner.x - 0.08, (float(i) + 0.5) / 16.0)
					flower(b, Vector3(x, soil + 0.045, z + (VEG.unit(seed, i, 76) - 0.5) * 0.03), Vector3(0, 1, 0), 0.018, linear(color), seed * 31 + i + int(side) * 7, 0.05)
	else:
		var square := piece == "planter_square"
		match kind:
			0:
				# Bedding annuals: leafy mounds in clumps, each in one colour.
				var columns := 3 if square else 4
				var rows := 3 if square else 2
				for i in columns:
					for j in rows:
						var x := lerpf(-inner.x, inner.x, (float(i) + 0.5 + (0.25 if j % 2 == 1 else -0.25) * (0.0 if square else 1.0)) / columns)
						var z := lerpf(-inner.y, inner.y, (float(j) + 0.5) / rows)
						var r := minf(inner.x / columns, inner.y / rows) * 1.05
						var clump := i * 7 + j
						mound(b, Vector3(x, soil + 0.02, z), Vector3(r, 0.05, r), 0, seed + clump, soil, 1.0, 0.03)
						var color := linear(FLOWERS[int(VEG.unit(seed, clump, 77) * FLOWERS.size()) % FLOWERS.size()])
						for k in 5:
							var a := TAU * (float(k) + VEG.unit(seed, clump, k)) / 5.0
							var d := r * (0.25 + 0.45 * VEG.unit(seed, clump * 5 + k, 78))
							var p := Vector3(x + cos(a) * d, soil + 0.065 - d * 0.25, z + sin(a) * d)
							flower(b, p, Vector3(cos(a) * 0.5, 2.0, sin(a) * 0.5).normalized(), 0.02, color, seed * 13 + clump * 5 + k, 0.05)
			1:
				# Clipped boxwood block.
				var height := 0.1 if square else 0.075
				rounded(b, Vector3(0.0, soil + height, 0.0), Vector3(inner.x - 0.01, height, inner.y - 0.01), Vector3.ONE * 0.04, Vector3i(8 if not square else 5, 3, 3 if not square else 5), seed, 0.01, 1.0, ao,
					func(p: Vector3) -> float: return 0.04 * clampf((p.y - soil) / 0.2, 0.0, 1.0))
			2:
				# Royal azalea: overlapping lumpy mounds in magenta bloom.
				var count := 5 if square else 4
				var directions: PackedVector3Array = VEG.icosphere(1)[0]
				for i in count:
					var centre := Vector3(lerpf(-inner.x * 0.6, inner.x * 0.6, float(i) / (count - 1)), soil + 0.03, (VEG.unit(seed, i, 85) - 0.5) * 0.04)
					var radii := Vector3(inner.x * 0.36, 0.085, inner.y * 0.88)
					if square:
						var a := TAU * float(i) / (count - 1)
						centre = Vector3(cos(a) * inner.x * 0.42, soil + 0.03, sin(a) * inner.y * 0.42) if i < count - 1 else Vector3(0.0, soil + 0.05, 0.0)
						radii = Vector3(inner.x * 0.5, 0.1, inner.y * 0.5)
					mound(b, centre, radii, 1, seed + i * 13, soil, 1.0, 0.04)
					for k in directions.size():
						var d := directions[k]
						if d.y < 0.25 or VEG.unit(seed + i, k, 79) > 0.45: continue
						var n := (d / radii).normalized()
						flower(b, centre + d * radii * 0.98 + n * 0.004, n, 0.02, linear(AZALEA, 0.85 + 0.25 * VEG.unit(seed + i, k, 80)), seed * 7 + i * 50 + k, 0.04)
			_:
				# Ornamental grass in the middle, a liriope edge along the sides.
				var tufts := 4 if square else 3
				for t in tufts:
					var centre := Vector3(0.0, soil, 0.0)
					if square:
						var a := TAU * (float(t) + 0.5) / tufts
						centre = Vector3(cos(a) * inner.x * 0.35, soil, sin(a) * inner.y * 0.35)
					else:
						centre.x = lerpf(-inner.x * 0.55, inner.x * 0.55, float(t) / (tufts - 1))
					for k in 7:
						var a := TAU * (float(k) + VEG.unit(seed, t * 8 + k, 81)) / 7.0
						var out := Vector3(cos(a), 0.0, sin(a)) * (0.35 + 0.2 * VEG.unit(seed, t * 8 + k, 82))
						var length := (0.3 if square else 0.24) + 0.05 * VEG.unit(seed, t * 8 + k, 83)
						var c := Color(0.85 + 0.15 * VEG.unit(seed, t, k), 0.85 + 0.15 * VEG.unit(seed, t, k), 0.85, 1.0)
						blade(b, centre + Vector3(cos(a), 0, sin(a)) * 0.015, out, length, 0.014, c, 1.0, 0.18)
				var liriope := linear("2a4426")
				var count := 8 if square else 6
				for side in [-1.0, 1.0]:
					for i in count:
						var x := lerpf(-inner.x + 0.04, inner.x - 0.04, (float(i) + 0.5) / count)
						var root := Vector3(x, soil, side * (inner.y - 0.025))
						if square and i % 2 == 1: root = Vector3(side * (inner.x - 0.025), soil, x)
						for k in 3:
							var out := (Vector3(0, 0, side) if not (square and i % 2 == 1) else Vector3(side, 0, 0)).rotated(Vector3.UP, (float(k) - 1.0) * 0.7) * 0.8
							blade(b, root, out, 0.06 + 0.02 * VEG.unit(seed, i * 3 + k, 84), 0.012, Color(liriope.r * (0.9 + 0.2 * VEG.unit(seed, i, k)), liriope.g, liriope.b, 0.0), 0.0, 0.02)
	var mesh := ArrayMesh.new()
	b.commit(mesh, materials.get("body"))
	mesh.resource_name = "mk_bed_%s_planting_%d" % [piece, kind]
	return mesh

## Far mesh of a piece: the rim box in granite with a planted top inset 2 cm
## (the distant colours, distant.rs).
static func bed_far(piece: String, materials: Dictionary) -> ArrayMesh:
	var frame := bed_frame(piece)
	var half: Vector2 = frame.half
	var top: float = frame.top
	var b := VEG.Builder.new()
	var side := linear(FAR_SIDE)
	var green := linear(FAR_TOP)
	block(b, Vector3(-half.x, -RIM_SKIRT, -half.y), Vector3(half.x, top, half.y), 0.0, func(p: Vector3) -> Color: return side if p.y < top - 0.001 else side * 0.9)
	var plants := {"planter": 0.06, "planter_square": 0.09, "median_bed": 0.11}[piece] as float
	block(b, Vector3(-half.x + 0.02, top - 0.01, -half.y + 0.02), Vector3(half.x - 0.02, top + plants, half.y - 0.02), 0.015, func(p: Vector3) -> Color: return green * (1.0 if p.y > top + plants - 0.001 else 0.8))
	var mesh := ArrayMesh.new()
	b.commit(mesh, materials.get("body"))
	mesh.resource_name = "mk_bed_%s_far" % piece
	return mesh
