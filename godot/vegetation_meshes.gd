extends RefCounted
## Display-only park vegetation, built procedurally (no external assets): three
## city-tree variants, three shrub sizes and one grass tuft with optional flower
## heads. Every array comes from integer hashes, so builds are identical.
##
## A tree or shrub has three meshes: `body` (a tapered trunk with root flare
## and two or three branches forking below the crown, plus the crown's dark
## inner hull with vertex ambient occlusion), `cards` (alpha-scissor leaf cards
## on the generated leaf atlas standing outside that hull and forming the
## silhouette) and `far` (a coarse lighter hull and a plain trunk). On low the
## body's hull grows back to the full crown and brightens (shader
## `cards_hidden`), so close trees stay lobed without cards.
##
## Collision is unchanged: the 14 x 140 x 14 cm trunk box (placement.rs) stays
## inside the trunk at every instance scale (0.85-1.15). The horizontal reach
## of every crown vertex at the largest scale stays inside the generated
## footprint (CITY_TREE_RADIUS 120 cm for trees; the baseline 42 cm for shrubs),
## so crowns never reach into facades, fences or lanes the placement keeps clear.
##
## begin()/advance() build one part per call for admitted streaming steps
## (render_resource_cache.prepare_vegetation); step_shared() advances a
## process-lifetime set one part per call for cells without a resource cache.

const SHADER := preload("./vegetation_leaf.gdshader")
## Preloaded and bound once when the renderer scripts load (_warm), so their
## compilation never falls into an admitted streaming step.
const SHADERS := {"body": SHADER, "cards": preload("./vegetation_leaf_cards.gdshader"), "grass": preload("./vegetation_grass.gdshader")}
static var _warm_materials: Array = _warm()

static func _warm() -> Array:
	var result := []
	for kind: String in SHADERS:
		var warm := ShaderMaterial.new()
		warm.shader = SHADERS[kind]
		warm.set_shader_parameter("wind_strength", 0.0)
		result.append(warm)
	return result
## scripts/richer_assets.py --textures --leaves (2x2 leaf clusters, 256 px).
const LEAF_ATLAS := "textures/vegetation/leaf_atlas.png"
## City trees at scale 1 (metres): canopy base, canopy height, crown radius
## before the reach fit. Round, columnar and spreading; mirrored by the distant
## silhouette (mapkit-package distant.rs CITY_TREES).
const TREES := [Vector3(1.5, 2.3, 1.0), Vector3(1.45, 2.9, 0.8), Vector3(1.7, 1.8, 1.15)]
const TREE_SCALE := Vector2(0.85, 1.15)
## Largest horizontal reach of a tree at scale 1: 120 cm at the largest scale.
const TREE_REACH := 1.2 / 1.15
## Leaf colours (sRGB) of the tree variants; placements vary hue and brightness.
const TREE_LEAVES := ["4f7a3a", "3f6c40", "5d7d36"]
## Shrubs: radius, height (metres) and sRGB leaf colour of the three sizes.
const SHRUBS := [Vector2(0.36, 0.58), Vector2(0.40, 0.72), Vector2(0.31, 0.46)]
const SHRUB_SCALE := Vector2(0.9, 1.1)
const SHRUB_REACH := 0.42 / 1.1
const SHRUB_LEAVES := ["4d7a40", "5a7f3c", "3f6b43"]
const BARK := "5a4433"
## The near hull sits at this share of the crown so cards stand outside it.
const SHELL := 0.8
## Memory allowance of one shared set (render_resource_cache): about 9,000
## vertices at <= 56 B in CPU arrays and GPU buffers, three materials (the
## shaders are process-lifetime script constants), and the
## 256 px RGBA leaf atlas with mipmaps as decoded image and texture (350 KiB
## each), with build overlap. Measured arrays stay below half of it.
const SHARED_BYTES := 2 * 1024 * 1024
const KINDS := ["body", "cards", "far"]
static var STEPS: PackedStringArray = _steps()
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}
static var _crowns: Dictionary = {}

static func _steps() -> PackedStringArray:
	var result := PackedStringArray(["atlas", "material_body", "material_cards", "material_grass"])
	for plant in ["tree", "shrub"]:
		for part in KINDS:
			for variant in 3: result.append("%s_%s_%d" % [plant, part, variant])
	result.append("grass")
	return result

class Builder:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var colors := PackedColorArray()
	var uv := PackedVector2Array()
	var uv2 := PackedVector2Array()
	var indices := PackedInt32Array()
	func add(point: Vector3, normal: Vector3, color: Color, texture: Vector2, weights: Vector2) -> int:
		vertices.append(point); normals.append(normal); colors.append(color); uv.append(texture); uv2.append(weights)
		return vertices.size() - 1
	func triangle(a: int, b: int, c: int) -> void:
		indices.append(a); indices.append(b); indices.append(c)
	func commit(mesh: ArrayMesh, material: Material) -> void:
		if indices.is_empty(): return
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_NORMAL] = normals
		arrays[Mesh.ARRAY_COLOR] = colors
		arrays[Mesh.ARRAY_TEX_UV] = uv
		arrays[Mesh.ARRAY_TEX_UV2] = uv2
		arrays[Mesh.ARRAY_INDEX] = indices
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mesh.surface_set_material(mesh.get_surface_count() - 1, material)

## 32-bit integer hash (no floating point): identical on every platform.
static func hash32(value: int) -> int:
	var h := value & 0xffffffff
	h = ((h >> 16) ^ h) * 0x45d9f3b & 0xffffffff
	h = ((h >> 16) ^ h) * 0x45d9f3b & 0xffffffff
	return ((h >> 16) ^ h) & 0xffffffff

## Value in [0, 1) from up to three integers.
static func unit(a: int, b := 0, c := 0) -> float:
	return float(hash32(a * 73856093 ^ hash32(b * 19349663 ^ hash32(c * 83492791))) & 0xffffff) / 16777216.0

static func linear(srgb: String) -> Color:
	return Color(srgb).srgb_to_linear()

## Instance data of a tree or shrub placement: leaf tint (linear) varied in
## hue and brightness, and the wind phase (INSTANCE_CUSTOM.a = 1 + phase).
static func leaf_custom(srgb: String, pick: float) -> Color:
	var base := linear(srgb)
	var hue := fmod(pick * 17.0, 1.0)
	var warm := base * Color(1.18, 1.06, 0.72)
	var cool := base * Color(0.86, 1.0, 1.12)
	var tinted := cool.lerp(base, minf(hue * 2.0, 1.0)).lerp(warm, maxf(hue * 2.0 - 1.0, 0.0))
	var bright := 0.86 + 0.26 * fmod(pick * 23.0, 1.0)
	return Color(tinted.r * bright, tinted.g * bright, tinted.b * bright, 1.0 + fmod(pick * 29.0, 1.0) * 0.999)

static func begin(context: RefCounted = null) -> Dictionary:
	var result := {"materials": {}}
	for plant in ["tree", "shrub"]:
		for part in KINDS: result[plant + "_" + part] = []
	return {"step": 0, "context": context, "result": result, "atlas": null}

## One part per call; returns true when the whole set is complete. A lease
## (track(Object)) receives every created resource.
static func advance(job: Dictionary, lease: RefCounted = null) -> bool:
	if job.step >= STEPS.size(): return true
	var step: String = STEPS[job.step]
	var result: Dictionary = job.result
	if step == "atlas":
		job.atlas = _atlas()
		if job.atlas != null and lease != null: lease.track(job.atlas)
	elif step.begins_with("material_"):
		var kind := step.get_slice("_", 1)
		var made := material(kind, job.atlas, job.context)
		result.materials[kind] = made
		if lease != null: lease.track(made)
		if kind == "grass": job.atlas = null
	else:
		var mesh: ArrayMesh
		if step == "grass": mesh = grass(result.materials)
		else: mesh = plant(step.get_slice("_", 0), step.get_slice("_", 1), int(step.get_slice("_", 2)), result.materials)
		if lease != null: lease.track(mesh)
		if step == "grass": result.grass = mesh
		else: result[step.get_slice("_", 0) + "_" + step.get_slice("_", 1)].append(mesh)
	job.step += 1
	if job.step < STEPS.size(): return false
	job.erase("context")
	result.make_read_only()
	return true

## Process-lifetime set for cells without a resource cache (standalone
## previews, older recipes), like the other street details: one part per call;
## returns the set once complete, else {}.
static func step_shared() -> Dictionary:
	if not _shared.is_empty(): return _shared
	if _shared_job.is_empty(): _shared_job = begin()
	if advance(_shared_job):
		_shared = _shared_job.result
		_shared_job = {}
	return _shared

## The whole process-lifetime set at once (validators and single-node previews).
static func shared() -> Dictionary:
	while step_shared().is_empty(): pass
	return _shared

static func _atlas() -> Texture2D:
	var path: String = (SHADER as Resource).resource_path.get_base_dir().path_join(LEAF_ATLAS)
	var source := ResourceLoader.load(path, "Texture2D", ResourceLoader.CACHE_MODE_IGNORE) as Texture2D
	var image := source.get_image() if source != null else null
	if image == null or image.is_empty(): return null # cards are optional decoration
	if image.is_compressed(): image.decompress()
	image.convert(Image.FORMAT_RGBA8)
	image.generate_mipmaps()
	return ImageTexture.create_from_image(image)

static func material(kind: String, atlas: Texture2D, context: RefCounted = null) -> ShaderMaterial:
	var result := ShaderMaterial.new()
	result.shader = SHADERS[kind]
	result.resource_name = "mk_vegetation_" + kind
	if atlas != null:
		result.set_shader_parameter("leaf_atlas", atlas)
		result.set_shader_parameter("leaf_pattern", atlas)
	result.set_shader_parameter("sway_metres", 0.05 if kind == "grass" else 0.10)
	if context != null and context.has_method("bind_vegetation"): context.bind_vegetation(result)
	result.set_meta("mapkit_vegetation_shader", kind)
	result.set_meta("mapkit_vegetation", true)
	preload("./display_quality.gd").apply_material(result, preload("./display_quality.gd").active())
	return result

static var _ico := {}
## Unit icosphere (12, 42 or 162 vertices) and its faces.
static func icosphere(subdivisions: int) -> Array:
	if _ico.has(subdivisions): return _ico[subdivisions]
	var t := (1.0 + sqrt(5.0)) / 2.0
	var points := PackedVector3Array()
	for p: Vector3 in [Vector3(-1, t, 0), Vector3(1, t, 0), Vector3(-1, -t, 0), Vector3(1, -t, 0), Vector3(0, -1, t), Vector3(0, 1, t),
			Vector3(0, -1, -t), Vector3(0, 1, -t), Vector3(t, 0, -1), Vector3(t, 0, 1), Vector3(-t, 0, -1), Vector3(-t, 0, 1)]:
		points.append(p.normalized())
	var faces := PackedInt32Array([0,11,5, 0,5,1, 0,1,7, 0,7,10, 0,10,11, 1,5,9, 5,11,4, 11,10,2, 10,7,6, 7,1,8,
		3,9,4, 3,4,2, 3,2,6, 3,6,8, 3,8,9, 4,9,5, 2,4,11, 6,2,10, 8,6,7, 9,8,1])
	for _level in subdivisions:
		var middle := {}
		var next := PackedInt32Array()
		for f in range(0, faces.size(), 3):
			var mids := []
			for e in 3:
				var a := faces[f + e]
				var b := faces[f + (e + 1) % 3]
				var key := Vector2i(mini(a, b), maxi(a, b))
				if not middle.has(key):
					points.append(((points[a] + points[b]) * 0.5).normalized())
					middle[key] = points.size() - 1
				mids.append(middle[key])
			next.append_array([faces[f], mids[0], mids[2], faces[f + 1], mids[1], mids[0], faces[f + 2], mids[2], mids[1], mids[0], mids[1], mids[2]])
		faces = next
	_ico[subdivisions] = [points, faces]
	return _ico[subdivisions]

## Lobes of a crown: [centre, radii] in mesh space. `seed` separates kinds.
## The top lobe sits on the axis, so no crown has a notch in its top.
static func lobes(seed: int, base: float, height: float, radius: float, count: int, spread: float) -> Array:
	var result := []
	var centre := Vector3(0.0, base + height * 0.5, 0.0)
	result.append([centre, Vector3(radius * 0.7, height * 0.42, radius * 0.7)])
	var turn := unit(seed, 1) * TAU
	for i in range(1, count):
		var top := i == count - 1
		var angle := turn + TAU * float(i) / float(count - 1) + (unit(seed, i, 2) - 0.5) * 0.9
		var out := (0.08 if top else spread * (0.38 + 0.16 * unit(seed, i, 3))) * radius
		var y := base + height * (0.72 if top else 0.36 + 0.24 * unit(seed, i, 4))
		var r := radius * (0.58 if top else 0.46 + 0.12 * unit(seed, i, 5))
		var v := height * (0.28 if top else 0.22 + 0.08 * unit(seed, i, 6))
		# Keep every lobe inside the crown's [base, base + height] slab.
		y = clampf(y, base + v * 0.9, base + height - v)
		result.append([Vector3(cos(angle) * out, y, sin(angle) * out), Vector3(r, v, r)])
	return result

## Sway weight measured from the canopy base: 0 on the trunk, 1 at the top.
static func sway(y: float, base: float, height: float) -> float:
	return pow(clampf((y - base + 0.3) / (height + 0.3), 0.0, 1.0), 1.3)

## Ambient occlusion of a crown point: dark underside and interior.
static func crown_ao(p: Vector3, base: float, height: float, radius: float) -> float:
	var h := clampf((p.y - base) / height, 0.0, 1.0)
	var radial := clampf(Vector2(p.x, p.z).length() / radius, 0.0, 1.2)
	return lerpf(0.34, 1.0, clampf(0.12 + 0.58 * h + 0.45 * radial, 0.0, 1.0))

## Crown hull points: an icosphere around the core lobe pushed out to the
## farthest lobe surface along each direction (times `share`), so overlapping
## lobes become one lumpy mass without interior faces. The underside is lifted
## to `lift` so branches show beneath it.
static func _hull(seed: int, crown: Array, detail: int, share: float, lift: float) -> PackedVector3Array:
	var unit_points: PackedVector3Array = icosphere(detail)[0]
	var origin: Vector3 = crown[0][0]
	var points := PackedVector3Array()
	for v in unit_points.size():
		var direction := unit_points[v]
		var reach := 0.0
		for lobe: Array in crown:
			# Far intersection of the ray origin + direction * t with the ellipsoid.
			var radii: Vector3 = lobe[1]
			var q: Vector3 = (origin - (lobe[0] as Vector3)) / radii
			var e := direction / radii
			var a := e.dot(e)
			var b := 2.0 * q.dot(e)
			var c := q.dot(q) - 1.0
			var disc := b * b - 4.0 * a * c
			if disc >= 0.0: reach = maxf(reach, (-b + sqrt(disc)) / (2.0 * a))
		var p := origin + direction * reach * share * (0.95 + 0.1 * unit(seed, v, 7))
		p.y = maxf(p.y, lerpf(origin.y, lift, share))
		points.append(p)
	return points

## Leaf cards: [corners, normal, atlas cell, phase] on the lobe surfaces just
## outside the near hull, facing outward with a random tilt and roll.
static func _card_list(seed: int, crown: Array, count: int, size: float, base: float, height: float, radius: float) -> Array:
	var centre := Vector3(0.0, base + height * 0.5, 0.0)
	var extent := Vector3(radius, height * 0.5, radius)
	var cards := []
	for card in count:
		# A point on the crown's outer surface: up to four tries for one that
		# no other lobe covers, biased to the sides and underside the camera sees.
		var p := Vector3.ZERO
		for attempt in 4:
			var lobe_index := int(unit(seed, card * 4 + attempt, 11) * crown.size()) % crown.size()
			var lobe: Array = crown[lobe_index]
			var z := lerpf(-0.85, 0.75, unit(seed, card * 4 + attempt, 12))
			var angle := unit(seed, card * 4 + attempt, 13) * TAU
			var direction := Vector3(cos(angle) * sqrt(1.0 - z * z), z, sin(angle) * sqrt(1.0 - z * z))
			p = lobe[0] + direction * (lobe[1] as Vector3) * (0.9 + 0.14 * unit(seed, card, 14))
			# Cards keep above the crown's lifted underside: branches show.
			p.y = maxf(p.y, base + height * 0.1 + size * 0.25)
			var covered := false
			for other in crown.size():
				if other != lobe_index and ((p - crown[other][0]) / (crown[other][1] as Vector3)).length_squared() < 0.75: covered = true
			if not covered: break
		var outward := ((p - centre) / extent).normalized()
		var normal := outward.lerp(Vector3(unit(seed, card, 15) - 0.5, unit(seed, card, 16) - 0.4, unit(seed, card, 17) - 0.5), 0.45).normalized()
		var helper := Vector3.UP if absf(normal.y) < 0.9 else Vector3.RIGHT
		var u := normal.cross(helper).normalized()
		var v := normal.cross(u)
		var roll := unit(seed, card, 18) * TAU
		var half := size * (0.85 + 0.3 * unit(seed, card, 19)) * 0.5
		var a := (u * cos(roll) + v * sin(roll)) * half
		var b := (v * cos(roll) - u * sin(roll)) * half
		var corners := PackedVector3Array([p - a - b, p + a - b, p + a + b, p - a + b])
		for corner in 4: corners[corner].y = maxf(corners[corner].y, base - 0.05)
		cards.append([corners, normal, Vector2(float(card % 2), float((card / 2) % 2)) * 0.5, unit(seed, card, 21)])
	return cards

## Shape of one tree or shrub variant, cached per process (pure function of
## the constants): lobes, near and far hull points and leaf cards, squeezed
## horizontally so no vertex reaches past the kind's reach limit.
static func crown(plant: String, variant: int) -> Dictionary:
	var key := "%s_%d" % [plant, variant]
	if _crowns.has(key): return _crowns[key]
	var tree := plant == "tree"
	var base: float
	var height: float
	var radius: float
	var lobe_list: Array
	var seed: int
	if tree:
		var shape: Vector3 = TREES[variant]
		base = shape.x; height = shape.y; radius = shape.z
		seed = 101 + variant
		lobe_list = lobes(seed, base, height, radius, 6 if variant > 0 else 5, 1.15 if variant == 2 else (0.7 if variant == 1 else 1.0))
	else:
		var shape: Vector2 = SHRUBS[variant]
		base = -0.08; height = shape.y + 0.08; radius = shape.x
		seed = 201 + variant
		lobe_list = lobes(seed, base, height, radius, 4 if variant < 2 else 3, 1.1)
	var lift := base + height * 0.08 if tree else base
	var data := {"base": base, "height": height, "radius": radius, "seed": seed, "lobes": lobe_list,
		"body": _hull(seed, lobe_list, 2 if tree else 1, SHELL, lift), "far": _hull(seed, lobe_list, 1, 0.98, lift),
		"cards": _card_list(seed, lobe_list, (150 + 12 * variant) if tree else (26 + 6 * variant), 0.58 if tree else 0.22, base, height, radius)}
	# Horizontal fit: the widest vertex of the full crown (far hull, the shell
	# grown back on low, or a card corner) at the reach limit.
	var widest := 0.0
	for p in data.far: widest = maxf(widest, Vector2(p.x, p.z).length())
	for p in data.body: widest = maxf(widest, Vector2(p.x, p.z).length() / SHELL)
	for card: Array in data.cards:
		for p in card[0] as PackedVector3Array: widest = maxf(widest, Vector2(p.x, p.z).length())
	var fit := minf(1.0, (TREE_REACH if tree else SHRUB_REACH) / widest)
	var squeeze := Vector3(fit, 1.0, fit)
	for hull in ["body", "far"]:
		var points: PackedVector3Array = data[hull]
		for i in points.size(): points[i] *= squeeze
		data[hull] = points
	for card: Array in data.cards:
		var corners: PackedVector3Array = card[0]
		for i in 4: corners[i] *= squeeze
		card[0] = corners
	for lobe: Array in lobe_list:
		lobe[0] = (lobe[0] as Vector3) * squeeze
		lobe[1] = (lobe[1] as Vector3) * squeeze
	data.radius = radius * fit
	data.fit = fit
	_crowns[key] = data
	return data

## Hull surface from points: averaged face normals bent toward the crown
## centre's ellipsoid normal, vertex AO. The near shell (under the cards) is
## the dark gap colour; UV marks its shell: x = core lobe height, y = the
## growth factor back to the full crown used when cards are hidden.
static func _crown_surface(builder: Builder, data: Dictionary, points: PackedVector3Array, detail: int, sway_scale: float, shell: bool) -> void:
	var faces: PackedInt32Array = icosphere(detail)[1]
	var base: float = data.base
	var height: float = data.height
	var radius: float = data.radius
	var seed: int = data.seed
	var centre := Vector3(0.0, base + height * 0.5, 0.0)
	var extent := Vector3(radius, height * 0.5, radius)
	var origin: Vector3 = data.lobes[0][0]
	var first := builder.vertices.size()
	var normals := PackedVector3Array()
	normals.resize(points.size())
	for f in range(0, faces.size(), 3):
		var n := (points[faces[f + 1]] - points[faces[f]]).cross(points[faces[f + 2]] - points[faces[f]])
		for k in 3: normals[faces[f + k]] += n
	for v in points.size():
		var p := points[v]
		var bent := normals[v].normalized().lerp(((p - centre) / extent).normalized(), 0.4).normalized()
		var ao := crown_ao(p, base, height, radius) * ((0.62 if shell else 0.9) + 0.12 * unit(seed, v, 8))
		var marks := Vector2(origin.y, 1.0 / SHELL) if shell else Vector2.ZERO
		builder.add(p, bent, Color(ao, ao, ao, 1.0), marks, Vector2(sway(p.y, base, height) * sway_scale, 0.0))
	for f in range(0, faces.size(), 3):
		builder.triangle(first + faces[f], first + faces[f + 2], first + faces[f + 1])

static func _cards(builder: Builder, data: Dictionary, sway_scale: float) -> void:
	var base: float = data.base
	var height: float = data.height
	var radius: float = data.radius
	var centre := Vector3(0.0, base + height * 0.5, 0.0)
	var extent := Vector3(radius, height * 0.5, radius)
	var uvs := [Vector2(0.0, 1.0), Vector2(1.0, 1.0), Vector2(1.0, 0.0), Vector2(0.0, 0.0)]
	var index := 0
	for card: Array in data.cards:
		var corners: PackedVector3Array = card[0]
		var normal: Vector3 = card[1]
		var cell: Vector2 = card[2]
		var first := builder.vertices.size()
		for corner in 4:
			var q := corners[corner]
			var bent := ((q - centre) / extent).normalized().lerp(normal, 0.35).normalized()
			var ao := crown_ao(q, base, height, radius) * (0.95 + 0.1 * unit(data.seed, index * 4 + corner, 20))
			# UV2.y = 1 + the card's flutter phase: the whole card moves together.
			builder.add(q, bent, Color(ao, ao, ao, 1.0), cell + uvs[corner] * 0.5, Vector2(sway(q.y, base, height) * sway_scale, 1.0 + 0.999 * float(card[3])))
		builder.triangle(first, first + 1, first + 2)
		builder.triangle(first, first + 2, first + 3)
		index += 1

## Tapered tube through `points` with `radii`; root flare buttresses on the
## first two rings when `flare` is set. Open ends (below ground, in the crown).
static func _tube(builder: Builder, seed: int, points: Array, radii: Array, sides: int, bark: Color, flare: bool, base: float, height: float) -> void:
	var first := builder.vertices.size()
	var turn := unit(seed, 31) * TAU
	for i in points.size():
		var p: Vector3 = points[i]
		var tangent: Vector3 = ((points[mini(i + 1, points.size() - 1)] as Vector3) - (points[maxi(i - 1, 0)] as Vector3)).normalized()
		var helper := Vector3.RIGHT if absf(tangent.x) < 0.9 else Vector3.FORWARD
		var n1 := tangent.cross(helper).normalized()
		var n2 := tangent.cross(n1)
		var ground := clampf(p.y / 1.2, 0.0, 1.0)
		var inside := 0.72 if p.y > base + 0.3 else 1.0
		for k in sides:
			var theta := TAU * float(k) / float(sides)
			var radius: float = radii[i]
			if flare and i < 2: radius *= 1.0 + (0.55 if i == 0 else 0.3) * pow(maxf(0.0, cos(4.0 * theta + turn)), 2.0)
			var direction := n1 * cos(theta) + n2 * sin(theta)
			var shade := lerpf(0.55, 1.0, ground) * inside * (0.9 + 0.15 * unit(seed, i * 16 + k, 32))
			builder.add(p + direction * radius, direction, Color(bark.r * shade, bark.g * shade, bark.b * shade, 0.0), Vector2.ZERO, Vector2(sway(p.y, base, height), 0.0))
	for i in points.size() - 1:
		for k in sides:
			var a := first + i * sides + k
			var b := first + i * sides + (k + 1) % sides
			builder.triangle(a, a + sides, b + sides)
			builder.triangle(a, b + sides, b)

## Trunk rings (mesh y, radius): the octagon's inscribed radius times the
## smallest instance scale (0.85) stays >= 0.10 m up to 1.7 m, so the 14 cm
## collision box (half diagonal 0.099 m) never shows through below 1.4 m.
const TRUNK := [Vector2(-0.12, 0.26), Vector2(0.06, 0.19), Vector2(0.35, 0.15), Vector2(1.0, 0.135), Vector2(1.7, 0.128)]
## Lowest mesh height of a branch fork (below the crown's lifted underside).
const FORK := 1.12

static func plant(kind: String, part: String, variant: int, materials: Dictionary) -> ArrayMesh:
	var data := crown(kind, variant)
	var tree := kind == "tree"
	var base: float = data.base
	var height: float = data.height
	var seed: int = data.seed
	# Shrubs sway less than crowns: 40 % of the weight at the same height.
	var sway_scale := 1.0 if tree else 0.4
	var builder := Builder.new()
	var bark := linear(BARK)
	var mesh := ArrayMesh.new()
	match part:
		"body":
			if tree:
				var points := []
				var radii := []
				for ring: Vector2 in TRUNK:
					points.append(Vector3(0.0, ring.x, 0.0)); radii.append(ring.y)
				points.append(Vector3(0.0, base + height * 0.45, 0.0)); radii.append(0.07)
				points.append(Vector3(0.0, base + height * 0.75, 0.0)); radii.append(0.035)
				_tube(builder, seed, points, radii, 8, bark, true, base, height)
				# Branches fork below the crown and climb out under its lifted
				# underside into the side lobes.
				for b in (2 if variant == 1 else 3):
					var lobe: Array = data.lobes[1 + b]
					var start := Vector3(0.0, FORK + 0.13 * float(b), 0.0)
					var target: Vector3 = lobe[0]
					var end := Vector3(target.x * 0.8, base + height * 0.3, target.z * 0.8)
					var middle := Vector3(target.x * 0.55, lerpf(start.y, end.y, 0.4), target.z * 0.55)
					_tube(builder, seed * 7 + b, [start, middle, end], [0.07, 0.05, 0.025], 5, bark, false, base, height)
			_crown_surface(builder, data, data.body, 2 if tree else 1, sway_scale, true)
			builder.commit(mesh, materials.get("body"))
		"cards":
			_cards(builder, data, sway_scale)
			builder.commit(mesh, materials.get("cards"))
		_:
			if tree:
				_tube(builder, seed, [Vector3(0.0, -0.1, 0.0), Vector3(0.0, 1.7, 0.0), Vector3(0.0, base + 0.3, 0.0)], [0.17, 0.145, 0.1], 6, bark, false, base, height)
			_crown_surface(builder, data, data.far, 1, sway_scale, false)
			builder.commit(mesh, materials.get("body"))
	mesh.resource_name = "mk_%s_%s_%d" % ["city_tree" if tree else "shrub", part, variant]
	return mesh

## Grass tuft: twenty-four curved blades in a compact 0.2 m clump (three
## triangles each) and two tilted flower heads on the tallest stems, collapsed
## unless the instance carries a flower. Blades are tinted by the instance and
## the lawn patches; their roots match the lawn underneath.
const GRASS_BLADES := 24
static func grass(materials: Dictionary) -> ArrayMesh:
	var builder := Builder.new()
	var seed := 301
	for blade in GRASS_BLADES:
		var angle := unit(seed, blade, 1) * TAU
		var root := Vector3(cos(angle), 0.0, sin(angle)) * 0.2 * sqrt(unit(seed, blade, 2))
		var tall := blade >= GRASS_BLADES - 2
		var length := 0.2 if tall else 0.06 + 0.1 * unit(seed, blade, 3)
		var lean_angle := angle + (unit(seed, blade, 4) - 0.5) * 1.2
		var lean := Vector3(cos(lean_angle), 0.0, sin(lean_angle)) * (0.03 if tall else 0.03 + 0.06 * unit(seed, blade, 5))
		var facing := unit(seed, blade, 6) * TAU
		var side := Vector3(cos(facing), 0.0, sin(facing))
		var normal := side.cross(Vector3.UP).normalized().lerp(Vector3.UP, 0.6).normalized()
		var width := 0.01 if tall else 0.018
		var shade := 0.92 + 0.1 * unit(seed, blade, 7)
		var rows := [[0.0, width, 0.82], [0.55, width * 0.7, 0.95], [1.0, 0.0, 1.08]]
		var first := builder.vertices.size()
		for row: Array in rows:
			var t: float = row[0]
			var centre := root + Vector3(0.0, length * t, 0.0) + lean * t * t
			var half: float = row[1] * 0.5
			var c: float = row[2] * shade
			var weights := Vector2(t * t, 0.5 * t)
			if half > 0.0:
				builder.add(centre - side * half, normal, Color(c, c, c, 1.0), Vector2.ZERO, weights)
				builder.add(centre + side * half, normal, Color(c, c, c, 1.0), Vector2.ZERO, weights)
			else:
				builder.add(centre, normal, Color(c, c, c, 1.0), Vector2.ZERO, weights)
		builder.triangle(first, first + 1, first + 3)
		builder.triangle(first, first + 3, first + 2)
		builder.triangle(first + 2, first + 3, first + 4)
		if tall:
			# Five-petal head tilted 50 degrees toward the stem's lean, so it
			# shows its face from eye height (UV2.y = -1 marks petals: flower
			# colour, no flutter).
			var tip := root + Vector3(0.0, length + 0.01, 0.0) + lean
			var facing_out := Vector3(cos(lean_angle), 0.0, sin(lean_angle))
			var up := Vector3.UP.rotated(facing_out.cross(Vector3.UP).normalized(), -deg_to_rad(50.0))
			var across := facing_out.cross(Vector3.UP).normalized()
			var along := up.cross(across).normalized()
			var centre_index := builder.add(tip + up * 0.008, up, Color(0.95, 0.9, 0.55, 0.0), Vector2.ZERO, Vector2(1.0, -1.0))
			for petal in 5:
				var a := TAU * float(petal) / 5.0 + angle
				builder.add(tip + (across * cos(a) + along * sin(a)) * 0.045, up, Color(1.0, 1.0, 1.0, 0.0), Vector2.ZERO, Vector2(1.0, -1.0))
			for petal in 5:
				builder.triangle(centre_index, centre_index + 1 + (petal + 1) % 5, centre_index + 1 + petal)
	var mesh := ArrayMesh.new()
	builder.commit(mesh, materials.get("grass"))
	mesh.resource_name = "mk_grass_tuft"
	return mesh
