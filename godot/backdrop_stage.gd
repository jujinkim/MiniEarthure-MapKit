extends RefCounted
## Display-only default-world scenery around a standalone track (`MapDocument.backdrop`).
## No collision, occupancy or spawn metadata; placement is seeded and stays outside
## the venue floor, so drive geometry never changes.
const VENUE := preload("./rc_venue.gd")
## Bands outside the venue floor, in metres.
const NEAR := Vector2(4.0, 16.0)
const FAR := Vector2(20.0, 48.0)
const LANDMARK := 58.0
const GROUND_MARGIN := 140.0
const MAX_SCATTER := 96
const MAX_BUILDINGS := 32
const THEMES := {
	"village": {"ground":Color("8fa45a"), "landmarks":["market-hall"], "buildings":["home-0","home-2","shop-0"], "scatter":["tree-2","tree-0","shrub","wheat"]},
	"neon_harbor": {"ground":Color("4d5a62"), "landmarks":["dock-crane"], "buildings":["urban-0","urban-2","warehouse-0","container-stack-0","container-stack-2"], "scatter":["lamp","tree-2"]},
	"deep_forest": {"ground":Color("4f6b38"), "landmarks":["ranger-lodge"], "buildings":["cedar-0","cedar-1","camp-tent"], "scatter":["beech","sapling-0","fern-patch","rock-0"]},
	"red_canyon": {"ground":Color("b97a4c"), "landmarks":["natural-arch"], "buildings":["sandstone-0","sandstone-1","sandstone-2","quarry-office"], "scatter":["cactus","desert-scrub","canyon-rubble"]},
	"snow_mountain": {"ground":Color("e3e8ec"), "landmarks":["alpine-lodge-0"], "buildings":["alpine-crag-2","alpine-crag-0","snow-fir-2"], "scatter":["snow-fir-0","snow-fir-1","snowbank","snow-fence"]},
	"machine_factory": {"ground":Color("6b6e66"), "landmarks":["factory-stack"], "buildings":["production-0","production-1","process-tank-0","pipe-rack"], "scatter":["transformer","container-0","tree-4"]},
	"sky_park": {"ground":Color("7fae5b"), "landmarks":["observation-wheel","carousel","decorative-coaster"], "buildings":["park-kiosk-0","park-pergola"], "scatter":["tree-2","tree-4","flowerbed"]},
}

static func supported(backdrop: String) -> bool:
	return THEMES.has(backdrop)

static func model(theme: String, id: String) -> PackedScene:
	var venue: Script = VENUE
	return load(venue.resource_path.get_base_dir().path_join("backdrops/%s/%s.glb" % [theme, id])) as PackedScene

## Point on the rectangle `[lo, hi]` grown by `offset`, at perimeter fraction `t`.
static func ring(lo: Vector2, hi: Vector2, offset: float, t: float) -> Vector2:
	var a := lo - Vector2.ONE*offset
	var b := hi + Vector2.ONE*offset
	var w := b.x - a.x
	var h := b.y - a.y
	var d := fposmod(t, 1.0) * 2.0 * (w + h)
	if d < w: return Vector2(a.x + d, a.y)
	d -= w
	if d < h: return Vector2(b.x, a.y + d)
	d -= h
	if d < w: return Vector2(b.x - d, b.y)
	return Vector2(a.x, b.y - (d - w))

static func build(root: Node3D, assembly: Dictionary, backdrop: String, floor_y: float, center: Vector3, extent: Vector3) -> void:
	var theme: Dictionary = THEMES[backdrop]
	root.set_meta("backdrop", backdrop)
	var ground := MeshInstance3D.new()
	ground.name = "BackdropGround"
	var plane := BoxMesh.new()
	plane.size = Vector3(extent.x + GROUND_MARGIN*2.0, 0.5, extent.z + GROUND_MARGIN*2.0)
	ground.mesh = plane
	var paint := StandardMaterial3D.new()
	paint.albedo_color = theme.ground
	paint.roughness = 0.9
	ground.material_override = paint
	ground.position = Vector3(center.x, floor_y - 0.27, center.z)
	root.add_child(ground)
	var rng := RandomNumberGenerator.new()
	rng.seed = hash([int(assembly.settings.seed), backdrop])
	var lo := Vector2(center.x - extent.x/2.0, center.z - extent.z/2.0)
	var hi := Vector2(center.x + extent.x/2.0, center.z + extent.z/2.0)
	var perimeter := 2.0 * (hi.x - lo.x + hi.y - lo.y)
	var scenes := {}
	var place := func(id: String, at: Vector2, face_center: bool, scale_range: Vector2) -> void:
		if not scenes.has(id): scenes[id] = model(backdrop, id)
		var scene: PackedScene = scenes[id]
		if scene == null: return
		var node: Node3D = scene.instantiate()
		node.position = Vector3(at.x, floor_y, at.y)
		node.rotation.y = atan2(center.x - at.x, center.z - at.y) if face_center else rng.randf() * TAU
		node.scale = Vector3.ONE * rng.randf_range(scale_range.x, scale_range.y)
		root.add_child(node)
	var scatter: Array = theme.scatter
	for i in mini(MAX_SCATTER, int(perimeter / 6.0)):
		place.call(scatter[rng.randi() % scatter.size()], ring(lo, hi, rng.randf_range(NEAR.x, NEAR.y), (float(i) + rng.randf()) / float(mini(MAX_SCATTER, int(perimeter / 6.0)))), false, Vector2(0.85, 1.2))
	var buildings: Array = theme.buildings
	var count := mini(MAX_BUILDINGS, int(perimeter / 24.0))
	for i in count:
		place.call(buildings[rng.randi() % buildings.size()], ring(lo, hi, rng.randf_range(FAR.x, FAR.y), (float(i) + 0.5 + rng.randf_range(-0.3, 0.3)) / float(maxi(count, 1))), true, Vector2(0.9, 1.1))
	var landmarks: Array = theme.landmarks
	for i in landmarks.size():
		place.call(landmarks[i], ring(lo, hi, LANDMARK, (float(i) + 0.5) / float(landmarks.size()) + 0.125), true, Vector2.ONE)
