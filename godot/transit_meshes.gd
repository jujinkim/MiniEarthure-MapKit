extends RefCounted
## Display-only transit details, built procedurally (no external assets, no
## real operator's logo, typeface or line colours): the builtin subway entrance,
## a glazed city metro entrance over a stair, and the builtin station sign, a
## free-standing pylon. Each has a `near` mesh (full detail) and a simple `far`
## mesh; materials are shared painted steel, stone cladding, a vertex-tinted
## night glow (atmosphere_surface lighting_role 2: lit signs, light strips and
## the stair tunnel's soft interior light) and translucent glass.
##
## Local metres, origin at the foot, local -z is the front: the importer turns
## both to face the nearest road (osm_furniture.py: local map +y, scene -z).
## Everything stays inside the unchanged collision boxes (placement.rs:
## entrance 80 x 110 x 120 cm, sign 24 x 180 x 24 cm) except the SKIRT below
## the foot. The renderer draws these
## meshes instead of the generated proxy faces (render_memory.hidden_objects);
## collision and occluders never read those faces.
##
## Subway entrance: stone-clad side and back parapets with a dark base course
## and coping on a closed stone slab (nothing inside lies below FLOOR, so
## sloping ground stays hidden); two flood-guard steps up to a landing, three
## shallow steps down to the well floor, then the stair's fake depth: treads
## and inner walls fading from the landing's soft light to black, receding
## lit nosings ever closer, handrails pitching down into a tunnel mouth in the
## rear block (dark hood top) whose ceiling slopes down into darkness (no hole
## in the ground). Steel posts, beams, gutters and five arch ribs carry a
## translucent barrel-vault roof and side and back glass. The front sign band
## carries a lit pictogram badge (an original train front), abstract name
## bars, a line badge and a line-colour stripe. By night the landing, steps
## and nosings glow softly, lamp strips run under the copings and the ridge,
## and a lamp sits inside the tunnel mouth.
##
## Station sign: a stone plinth, a bevelled steel pylon with a glowing line
## stripe and a name plate of abstract text bars, and a four-sided light box
## (pictogram, name bars and line stripe on a softly backlit panel) under a cap.
##
## A cell instances both kinds per 32 m group as near and far MultiMeshes with
## street_furniture_meshes' range metadata (display_quality.apply_furniture:
## near within furniture_near, the far mesh beyond and always drawn like the
## proxy it replaces, low shows the far mesh only, shadows on every tier).
## build() makes one part per call for admitted streaming steps
## (render_resource_cache.prepare_mesh_set); step_shared() builds a
## process-lifetime set for cells without a resource cache.

const QUALITY := preload("./display_quality.gd")
const F := preload("./street_furniture_meshes.gd")
const KINDS := {"builtin:subway_entrance": "subway_entrance", "builtin:station_sign": "station_sign"}
const LODS := ["near", "far"]
const KEY := "builtin:transit_details"
const GROUP_METRES := 32.0
## Collision boxes (placement.rs builtin), metres.
const ENTRANCE_PROXY := AABB(Vector3(-0.4, 0.0, -0.6), Vector3(0.8, 1.1, 1.2))
const SIGN_PROXY := AABB(Vector3(-0.12, 0.0, -0.12), Vector3(0.24, 1.8, 0.24))
## Colours (sRGB). LINE is an original teal, not any operator's line colour.
const STEEL := "38424c"
const RAIL := "b4bcc1"
const STONE := "aaa496"
const BASE := "6c675f"
const COPING := "cdc8bb"
const TREAD := "8f8c85"
const NOSING := "d6ae3c"
const BAND := "1d2a39"
const WHITE := "f3f5f1"
const LINE := "1aa39a"
const INK := "12323a"
const LAMP := "fff1d6"
## Tunnel interior: dim warm grey at the mouth to black inside (linear), so it
## reads as shadow by day and as a faint lit passage by night (emission 0.9x).
const TUNNEL_MOUTH := Color(0.07, 0.064, 0.055)
const TUNNEL_DEEP := Color(0.008, 0.008, 0.008)
## Rear block top over the tunnel: dark stone, part of the well from above.
const HOOD := "4a463f"
const GLASS := Color(0.6, 0.76, 0.8, 0.3)
const SURFACES := {"paint": {"tile": "metal", "metallic": 0.35, "roughness": 0.5},
	"stone": {"tile": "stone", "metallic": 0.0, "roughness": 0.8},
	"glow": {"tile": "", "metallic": 0.0, "roughness": 0.35}}
const STEPS := ["material_paint", "material_stone", "material_glow", "material_glass", "subway_entrance_near", "subway_entrance_far", "station_sign_near", "station_sign_far"]
## Entrance levels: two flood-guard risers up from the street to the landing,
## three shallow steps down to the well floor. The interior is a closed stone
## slab: nothing inside the parapets lies below FLOOR, so ground relief under
## the 80 x 120 cm footprint up to 7 cm (city cards: at most 6.5-7.1 cm) stays
## hidden; walls, the front riser and the plinth reach SKIRT below the foot so
## a downhill side never shows a gap.
const RISE := 0.053
const LANDING := 2.0 * RISE
const DROP := 0.012
const FLOOR := LANDING - 3.0 * DROP
const SKIRT := 0.06
## Landing back edge, down-step edges and the tunnel mouth (local z).
const WELL_EDGE := -0.44
const DOWN_EDGES := [-0.44, -0.385, -0.335, -0.29]
const MOUTH := 0.12
## Spacing of stacked decal layers (lit inlays, nosings, joints): at least
## 2.5 mm so they never depth-fight at chase distances.
const LAYER := 0.0025
## Well shade (factor of TREAD, linear) by depth z: the landing's soft light
## fading to <= 0.05 linear before the mouth and near black inside.
const WELL_SHADE := [[-0.545, 0.62], [-0.29, 0.4], [-0.15, 0.19], [-0.05, 0.11], [0.05, 0.06], [MOUTH, 0.035], [0.56, 0.03]]
## Memory allowance of one shared set (render_resource_cache): about 4,500
## vertices at <= 56 B and 7,500 indices at 4 B in CPU arrays and GPU buffers
## (0.57 MB; transit_detail_validator measures them), 64 KiB for the four
## materials and one mesh's build arrays.
const SHARED_BYTES := 768 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

static func linear(srgb: String, shade := 1.0) -> Color:
	return F.linear(srgb, shade)

## Rectangle facing `normal` (u: the viewer's right, v: up; horizontal
## faces use x and -z), half size `half`, corners cut by `cut`.
static func rect(b: F.Builder, centre: Vector3, normal: Vector3, half: Vector2, color: Color, cut := 0.0) -> void:
	var u := _right(normal)
	var v := normal.cross(u).normalized()
	var points := []
	if cut <= 0.0:
		for corner: Vector2 in [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)]:
			points.append(centre + u * half.x * corner.x + v * half.y * corner.y)
	else:
		for corner: Vector2 in [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)]:
			points.append(centre + u * (half.x - cut) * corner.x + v * half.y * corner.y)
			points.append(centre + u * half.x * corner.x + v * (half.y - cut) * corner.y)
	F.polygon(b, points, normal, color)

static func _right(normal: Vector3) -> Vector3:
	return (Vector3.UP.cross(normal) if absf(normal.y) < 0.9 else Vector3.RIGHT).normalized()

## Quad with one colour per corner (corners in order around it).
static func shade_quad(b: F.Builder, corners: Array, normal: Vector3, colors: Array) -> void:
	var first := b.vertices.size()
	for index in 4: b.add(corners[index], normal, colors[index])
	b.triangle(first, first + 1, first + 2)
	b.triangle(first, first + 2, first + 3)

## Original metro pictogram, `size` m square, on a face: a line-colour badge
## with chamfered corners holding a white train front (rounded cab, dark
## windscreen, two lamps, splayed skirts over a rail). Layers step LAYER out.
static func pictogram(b: F.Builder, centre: Vector3, normal: Vector3, size: float, white_shade := 0.9) -> void:
	var u := _right(normal)
	var v := normal.cross(u).normalized()
	var at := func(x: float, y: float, layer: int) -> Vector3: return centre + (u * x + v * y) * size + normal * (LAYER * float(layer))
	var white := linear(WHITE, white_shade)
	var ink := linear(INK)
	rect(b, at.call(0.0, 0.0, 0), normal, Vector2(0.5, 0.5) * size, linear(LINE), 0.12 * size)
	var cab := []
	for point: Vector2 in [Vector2(-0.27, -0.2), Vector2(0.27, -0.2), Vector2(0.27, 0.2), Vector2(0.16, 0.33), Vector2(-0.16, 0.33), Vector2(-0.27, 0.2)]:
		cab.append(at.call(point.x, point.y, 1))
	F.polygon(b, cab, normal, white)
	rect(b, at.call(0.0, 0.12, 2), normal, Vector2(0.19, 0.085) * size, ink, 0.03 * size)
	for side: float in [-1.0, 1.0]:
		rect(b, at.call(0.155 * side, -0.09, 2), normal, Vector2(0.045, 0.035) * size, ink)
		F.polygon(b, [at.call(0.14 * side, -0.2, 1), at.call(0.22 * side, -0.2, 1), at.call(0.31 * side, -0.33, 1), at.call(0.23 * side, -0.33, 1)], normal, white)
	rect(b, at.call(0.0, -0.37, 1), normal, Vector2(0.36, 0.025) * size, white)

## Abstract station name: a row of four glyph blocks over a thinner subtitle.
static func name_bars(b: F.Builder, centre: Vector3, normal: Vector3, half: Vector2, color: Color) -> void:
	var u := _right(normal)
	var v := normal.cross(u).normalized()
	var widths := [0.21, 0.25, 0.21, 0.25]
	var x := -1.0
	for index in widths.size():
		var w: float = widths[index] * 2.0
		var c := centre + u * half.x * (x + w * 0.5) + v * half.y * 0.32
		rect(b, c, normal, Vector2(half.x * w * 0.42, half.y * 0.5), color)
		x += w + 0.05
	var y := -0.62
	x = -1.0
	for w: float in [0.5, 0.36, 0.62, 0.38]:
		rect(b, centre + u * half.x * (x + w * 0.5) + v * half.y * y, normal, Vector2(half.x * w * 0.46, half.y * 0.18), color)
		x += w + 0.04

## Subway entrance: surfaces paint, stone, glow and glass.
static func subway_entrance(lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var stone := F.Builder.new()
	var glow := F.Builder.new()
	var glass := F.Builder.new()
	if lod == "near":
		_entrance_shell(stone, glow)
		_entrance_stairs(stone, glow)
		_entrance_canopy(paint, glass, glow)
		_entrance_rails(paint)
	else:
		_entrance_far(paint, stone, glow, glass)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	stone.commit(mesh, materials.get("stone"))
	glow.commit(mesh, materials.get("glow"))
	glass.commit(mesh, materials.get("glass"))
	mesh.resource_name = "mk_subway_entrance_" + lod
	return mesh

## Far entrance: the same closed slab, steps, darkening well, dark hood and
## mouth, posts and band in boxes, and translucent glass walls and roof.
static func _entrance_far(paint: F.Builder, stone: F.Builder, glow: F.Builder, glass: F.Builder) -> void:
	var steel := linear(STEEL)
	for s: float in [-1.0, 1.0]:
		F.box(stone, Vector3(0.3625 * s, (0.42 - SKIRT) * 0.5, 0.0), Vector3(0.0325, (0.42 + SKIRT) * 0.5, 0.6), 0.0, linear(STONE), Basis(), 0.7)
		rect(glass, Vector3(0.365 * s, 0.7, 0.0), Vector3.RIGHT * s, Vector2(0.56, 0.27), Color.WHITE)
	F.box(stone, Vector3(0.0, (0.42 - SKIRT) * 0.5, 0.5875), Vector3(0.33, (0.42 + SKIRT) * 0.5, 0.0125), 0.0, linear(STONE), Basis(), 0.7)
	rect(glass, Vector3(0.0, 0.7, 0.575), Vector3.BACK, Vector2(0.35, 0.27), Color.WHITE)
	F.box(stone, Vector3(0.0, (RISE - SKIRT) * 0.5, -0.5725), Vector3(0.33, (RISE + SKIRT) * 0.5, 0.0275), 0.0, linear(TREAD))
	F.box(glow, Vector3(0.0, (LANDING - SKIRT) * 0.5, -0.4925), Vector3(0.33, (LANDING + SKIRT) * 0.5, 0.0525), 0.0, _well_shade(-0.545))
	# Well floor from the landing to the mouth, darkening inward.
	var edges := [WELL_EDGE, -0.15, 0.0, MOUTH]
	for index in edges.size() - 1:
		var a: float = edges[index]
		var c: float = edges[index + 1]
		shade_quad(glow, [Vector3(-0.33, FLOOR, a), Vector3(0.33, FLOOR, a), Vector3(0.33, FLOOR, c), Vector3(-0.33, FLOOR, c)], Vector3.UP,
			[_well_shade(a), _well_shade(a), _well_shade(c), _well_shade(c)])
	# Rear block: dark hood, clad lintel and the mouth fading to black.
	F.box(stone, Vector3(0.0, (0.34 + FLOOR) * 0.5, 0.3475), Vector3(0.33, (0.34 - FLOOR) * 0.5, 0.2275), 0.0, linear(HOOD))
	rect(stone, Vector3(0.0, 0.31, MOUTH - LAYER), Vector3.FORWARD, Vector2(0.33, 0.03), linear(STONE))
	shade_quad(glow, [Vector3(-0.3, FLOOR, MOUTH - LAYER), Vector3(0.3, FLOOR, MOUTH - LAYER), Vector3(0.3, 0.28, MOUTH - LAYER), Vector3(-0.3, 0.28, MOUTH - LAYER)], Vector3.FORWARD,
		[TUNNEL_DEEP, TUNNEL_DEEP, TUNNEL_MOUTH, TUNNEL_MOUTH])
	# Canopy: posts, eaves, translucent two-pane roof.
	for post: Vector2 in [Vector2(-0.365, -0.57), Vector2(0.365, -0.57), Vector2(-0.365, 0.575), Vector2(0.365, 0.575)]:
		F.box(paint, Vector3(post.x, 0.7, post.y), Vector3(0.014, 0.28, 0.014), 0.0, steel)
	for s: float in [-1.0, 1.0]:
		F.box(paint, Vector3(0.365 * s, 0.98, 0.0225), Vector3(0.016, 0.015, 0.5675), 0.0, steel)
		var eave := Vector3(0.38 * s, 1.0, 0.0)
		var ridge := Vector3(0.0, 1.085, 0.0)
		var normal := Vector3(0.085 * s, 0.38, 0.0).normalized()
		shade_quad(glass, [eave + Vector3(0, 0, -0.545), ridge + Vector3(0, 0, -0.545), ridge + Vector3(0, 0, 0.592), eave + Vector3(0, 0, 0.592)], normal,
			[Color.WHITE, Color.WHITE, Color.WHITE, Color.WHITE])
	# Sign band (face z -0.5895) and its lit inlays one layer proud.
	F.box(paint, Vector3(0.0, 0.93, -0.5705), Vector3(0.395, 0.07, 0.019), 0.0, linear(BAND))
	var inlay := -0.5895 - LAYER
	rect(glow, Vector3(0.31, 0.935, inlay), Vector3.FORWARD, Vector2(0.05, 0.05), linear(LINE))
	rect(glow, Vector3(-0.03, 0.945, inlay), Vector3.FORWARD, Vector2(0.2, 0.025), linear(WHITE, 0.85))
	rect(glow, Vector3(0.0, 0.874, inlay), Vector3.FORWARD, Vector2(0.388, 0.008), linear(LINE))

## Side and back parapets (base course reaching SKIRT below the foot, clad
## body with joints, coping), the inner wall faces darkening down into the
## well, a lamp strip under each coping, and the rear block (dark hood top,
## clad lintel and jambs) whose front frames the tunnel mouth.
static func _entrance_shell(stone: F.Builder, glow: F.Builder) -> void:
	var clad := linear(STONE)
	var joint := linear(STONE, 0.72)
	for s: float in [-1.0, 1.0]:
		# Clad body 0.336-0.395: its inner face sits behind the shaded wall quads.
		F.box(stone, Vector3(0.3655 * s, (0.04 - SKIRT) * 0.5, 0.0), Vector3(0.0295, (0.04 + SKIRT) * 0.5, 0.6), 0.0, linear(BASE))
		F.box(stone, Vector3(0.3655 * s, 0.215, 0.0), Vector3(0.0295, 0.175, 0.6), 0.0, clad, Basis(), 0.88)
		F.box(stone, Vector3(0.3625 * s, 0.405, 0.0), Vector3(0.036, 0.015, 0.598), 0.004, linear(COPING))
		# Cladding joints on the outer face: panels 20 cm wide in two courses.
		var outer := Vector3.RIGHT * s
		for z: float in [-0.4, -0.2, 0.0, 0.2, 0.4]:
			rect(stone, Vector3((0.395 + LAYER) * s, 0.215, z), outer, Vector2(0.002, 0.175), joint)
		rect(stone, Vector3((0.395 + LAYER) * s, 0.215, 0.0), outer, Vector2(0.6, 0.002), joint)
		# Inner wall over the stair: stone at the coping, falling into the
		# well's shade at its foot, deeper toward the mouth.
		var inner := Vector3.LEFT * s
		var cuts := [-0.6, WELL_EDGE, -0.29, -0.15, -0.05, 0.05, MOUTH]
		for index in cuts.size() - 1:
			var a: float = cuts[index]
			var c: float = cuts[index + 1]
			var top_a := _wall_top(a)
			var top_c := _wall_top(c)
			shade_quad(stone, [Vector3(0.33 * s, 0.0, a), Vector3(0.33 * s, 0.39, a), Vector3(0.33 * s, 0.39, c), Vector3(0.33 * s, 0.0, c)], inner,
				[_wall_foot(a), top_a, top_c, _wall_foot(c)])
		# Lamp strip under the coping, lit at night.
		F.box(glow, Vector3(0.324 * s, 0.372, -0.2), Vector3(0.005, 0.006, 0.31), 0.0, linear(LAMP, 0.5))
	# Back parapet (outer face z 0.5955).
	F.box(stone, Vector3(0.0, (0.04 - SKIRT) * 0.5, 0.584), Vector3(0.336, (0.04 + SKIRT) * 0.5, 0.0115), 0.0, linear(BASE))
	F.box(stone, Vector3(0.0, 0.215, 0.584), Vector3(0.336, 0.175, 0.0115), 0.0, clad, Basis(), 0.88)
	F.box(stone, Vector3(0.0, 0.405, 0.583), Vector3(0.3265, 0.015, 0.0155), 0.004, linear(COPING))
	for x: float in [-0.2, 0.0, 0.2]: rect(stone, Vector3(x, 0.215, 0.5955 + LAYER), Vector3.BACK, Vector2(0.002, 0.175), joint)
	# Rear block over the tunnel: dark hood top, front frame (lintel and jambs).
	rect(stone, Vector3(0.0, 0.34, 0.3475), Vector3.UP, Vector2(0.336, 0.2275), linear(HOOD))
	rect(stone, Vector3(0.0, 0.31, MOUTH), Vector3.FORWARD, Vector2(0.33, 0.03), clad)
	F.box(stone, Vector3(0.0, 0.3, MOUTH - 0.005), Vector3(0.31, 0.012, 0.008), 0.003, linear(COPING))
	for s: float in [-1.0, 1.0]:
		rect(stone, Vector3(0.315 * s, (0.28 + FLOOR) * 0.5, MOUTH), Vector3.FORWARD, Vector2(0.015, (0.28 - FLOOR) * 0.5), clad)

static func _wall_top(z: float) -> Color:
	return linear(STONE, lerpf(1.0, 0.55, clampf((z + 0.44) / 0.56, 0.0, 1.0)))

static func _wall_foot(z: float) -> Color:
	return linear(STONE, 0.75) if z <= -0.545 else F.shaded(_well_shade(z), 1.1)

## Front steps up (stone, lit nosings), the landing and three shallow steps
## down (glow: softly lit by night), the well floor with receding nosings
## fading to black, and the tunnel interior with its sloping ceiling and lamp.
static func _entrance_stairs(stone: F.Builder, glow: F.Builder) -> void:
	var tread := linear(TREAD)
	var nosing := linear(NOSING, 0.8)
	var half := 0.33
	# Flood guard: the front riser reaches SKIRT below the foot.
	rect(stone, Vector3(0.0, (RISE - SKIRT) * 0.5, -0.6), Vector3.FORWARD, Vector2(half, (RISE + SKIRT) * 0.5), linear(TREAD, 0.78))
	rect(stone, Vector3(0.0, RISE, -0.5725), Vector3.UP, Vector2(half, 0.0275), tread)
	rect(stone, Vector3(0.0, (RISE + LANDING) * 0.5, -0.545), Vector3.FORWARD, Vector2(half, RISE * 0.5), linear(TREAD, 0.78))
	for edge: Vector2 in [Vector2(-0.6, RISE), Vector2(-0.545, LANDING)]:
		rect(glow, Vector3(0.0, edge.y + LAYER, edge.x + 0.009), Vector3.UP, Vector2(half - 0.004, 0.008), nosing)
	# Landing and the steps down, each a softly lit tread with a lit nosing
	# on its drop edge; the risers face away from the street.
	shade_quad(glow, [Vector3(-half, LANDING, -0.545), Vector3(half, LANDING, -0.545), Vector3(half, LANDING, WELL_EDGE), Vector3(-half, LANDING, WELL_EDGE)], Vector3.UP,
		[_well_shade(-0.545), _well_shade(-0.545), _well_shade(WELL_EDGE), _well_shade(WELL_EDGE)])
	for index in 3:
		var z0: float = DOWN_EDGES[index]
		var z1: float = DOWN_EDGES[index + 1]
		var top := LANDING - DROP * float(index)
		var y := top - DROP
		rect(glow, Vector3(0.0, (top + y) * 0.5, z0), Vector3.BACK, Vector2(half, DROP * 0.5), F.shaded(_well_shade(z0), 0.6))
		shade_quad(glow, [Vector3(-half, y, z0), Vector3(half, y, z0), Vector3(half, y, z1), Vector3(-half, y, z1)], Vector3.UP,
			[_well_shade(z0), _well_shade(z0), _well_shade(z1), _well_shade(z1)])
		rect(glow, Vector3(0.0, top + LAYER, z0 - 0.009), Vector3.UP, Vector2(half - 0.004, 0.008), nosing.lerp(_well_shade(z0), 0.25 * float(index)))
	# The well: floor shading to black, painted nosings receding ever closer
	# and darker (four, 2 cm wide, so they stay solid lines even seen almost
	# edge-on from the road).
	var stops := [-0.29, -0.15, -0.05, 0.05, MOUTH, 0.56]
	for index in stops.size() - 1:
		var a: float = stops[index]
		var c: float = stops[index + 1]
		shade_quad(glow, [Vector3(-half, FLOOR, a), Vector3(half, FLOOR, a), Vector3(half, FLOOR, c), Vector3(-half, FLOOR, c)], Vector3.UP,
			[_well_shade(a), _well_shade(a), _well_shade(c), _well_shade(c)])
	var receding := [-0.235, -0.165, -0.11, -0.065]
	for index in receding.size():
		var z: float = receding[index]
		var edge := _well_shade(z).lerp(nosing, 0.45 * (1.0 - float(index) / float(receding.size())))
		rect(glow, Vector3(0.0, FLOOR + LAYER, z), Vector3.UP, Vector2(half - 0.004, 0.01), edge)
	# Tunnel: sloping ceiling and side walls from the mouth inward.
	var ceiling := func(z: float) -> float: return lerpf(0.28, FLOOR + 0.012, clampf((z - MOUTH) / 0.44, 0.0, 1.0))
	for index in 4:
		var a := MOUTH + 0.11 * float(index)
		var c := a + 0.11
		var ta := _tunnel_shade(a)
		var tc := _tunnel_shade(c)
		var ya: float = ceiling.call(a)
		var yc: float = ceiling.call(c)
		var down := Vector3(0.0, -0.44, -(0.28 - FLOOR)).normalized()
		shade_quad(glow, [Vector3(-0.3, ya, a), Vector3(0.3, ya, a), Vector3(0.3, yc, c), Vector3(-0.3, yc, c)], down, [ta, ta, tc, tc])
		for s: float in [-1.0, 1.0]:
			shade_quad(glow, [Vector3(0.3 * s, FLOOR, a), Vector3(0.3 * s, ya, a), Vector3(0.3 * s, yc, c), Vector3(0.3 * s, FLOOR, c)], Vector3.LEFT * s, [ta, ta, tc, tc])
	rect(glow, Vector3(0.0, FLOOR + 0.0065, 0.559), Vector3.FORWARD, Vector2(0.3, 0.0065), TUNNEL_DEEP)
	# Lamp under the lintel inside the mouth.
	F.box(glow, Vector3(0.0, 0.266, MOUTH + 0.01), Vector3(0.2, 0.008, 0.008), 0.0, linear(LAMP, 0.4))

## Well floor and lit tread tint at depth z (linear), from WELL_SHADE.
static func _well_shade(z: float) -> Color:
	var factor: float = WELL_SHADE[-1][1]
	for index in WELL_SHADE.size() - 1:
		var a: Array = WELL_SHADE[index]
		var b: Array = WELL_SHADE[index + 1]
		if z <= float(b[0]):
			factor = lerpf(float(a[1]), float(b[1]), clampf((z - float(a[0])) / (float(b[0]) - float(a[0])), 0.0, 1.0))
			break
	return linear(TREAD, factor)

## Tunnel walls and ceiling: TUNNEL_MOUTH at the mouth to black 25 cm inside.
static func _tunnel_shade(z: float) -> Color:
	return TUNNEL_MOUTH.lerp(TUNNEL_DEEP, clampf((z - MOUTH) / 0.25, 0.0, 1.0))

## Stair floor height at depth z (landing, the steps down, the well floor).
static func _floor_at(z: float) -> float:
	if z < WELL_EDGE: return LANDING
	for index in 3:
		if z < float(DOWN_EDGES[index + 1]): return LANDING - DROP * float(index + 1)
	return FLOOR

## Steel posts, glazing rails, beams, gutters, sign band, arch ribs, ridge,
## the translucent vault and walls, and the sign band's lit inlays.
static func _entrance_canopy(paint: F.Builder, glass: F.Builder, glow: F.Builder) -> void:
	var steel := linear(STEEL)
	for post: Vector2 in [Vector2(-0.365, -0.57), Vector2(0.365, -0.57), Vector2(-0.365, 0.0), Vector2(0.365, 0.0), Vector2(-0.365, 0.575), Vector2(0.365, 0.575)]:
		var top := 0.86 if post.y < -0.5 else 0.985
		F.box(paint, Vector3(post.x, (0.42 + top) * 0.5, post.y), Vector3(0.014, (top - 0.42) * 0.5, 0.014), 0.003, steel)
	for s: float in [-1.0, 1.0]:
		F.box(paint, Vector3(0.365 * s, 0.428, 0.0), Vector3(0.01, 0.008, 0.57), 0.0, steel)
		F.box(paint, Vector3(0.365 * s, 0.98, 0.0225), Vector3(0.016, 0.015, 0.5675), 0.003, steel)
		F.box(paint, Vector3(0.383 * s, 1.0, 0.0225), Vector3(0.012, 0.008, 0.5675), 0.002, linear(STEEL, 0.85))
		F.box(paint, Vector3(0.365 * s, 0.7, 0.29), Vector3(0.006, 0.27, 0.006), 0.0, steel)
		F.box(paint, Vector3(0.365 * s, 0.7, -0.29), Vector3(0.006, 0.27, 0.006), 0.0, steel)
		rect(glass, Vector3(0.365 * s, 0.7, 0.0), Vector3.RIGHT * s, Vector2(0.56, 0.27), Color.WHITE)
	F.box(paint, Vector3(0.0, 0.428, 0.575), Vector3(0.35, 0.008, 0.01), 0.0, steel)
	F.box(paint, Vector3(0.0, 0.98, 0.575), Vector3(0.35, 0.015, 0.016), 0.003, steel)
	F.box(paint, Vector3(0.0, 0.7, 0.575), Vector3(0.006, 0.27, 0.006), 0.0, steel)
	rect(glass, Vector3(0.0, 0.7, 0.575), Vector3.BACK, Vector2(0.35, 0.27), Color.WHITE)
	# Sign band (face z -0.5895).
	F.box(paint, Vector3(0.0, 0.93, -0.5705), Vector3(0.395, 0.07, 0.019), 0.006, linear(BAND))
	F.box(paint, Vector3(0.0, 0.856, -0.5705), Vector3(0.39, 0.004, 0.015), 0.0, linear(STEEL, 0.8))
	# Lit inlays one LAYER proud of the band face, layers outward.
	var face := Vector3.FORWARD
	var inlay := -0.5895 - LAYER
	pictogram(glow, Vector3(0.315, 0.935, inlay), face, 0.1)
	name_bars(glow, Vector3(-0.015, 0.935, inlay), face, Vector2(0.2, 0.045), linear(WHITE, 0.85))
	rect(glow, Vector3(-0.32, 0.935, inlay), face, Vector2(0.042, 0.042), linear(LINE), 0.016)
	rect(glow, Vector3(-0.32, 0.935, inlay - LAYER), face, Vector2(0.007, 0.026), linear(WHITE, 0.85))
	rect(glow, Vector3(0.0, 0.874, inlay), face, Vector2(0.388, 0.008), linear(LINE))
	# Vault: arch ribs, ridge, translucent roof strip.
	var arc := func(x: float, lift: float) -> float: return 1.0 + (0.081 + lift) * (1.0 - (x / 0.38) * (x / 0.38))
	for z: float in [-0.54, -0.27, 0.0, 0.27, 0.585]:
		var centres := []
		var radii := []
		for step in 9:
			var x := lerpf(-0.38, 0.38, float(step) / 8.0)
			centres.append(Vector3(x, arc.call(x, 0.004), z))
			radii.append(0.008)
		F.tube(paint, centres, radii, 4, steel, true, true)
	F.tube(paint, [Vector3(0.0, 1.085, -0.54), Vector3(0.0, 1.085, 0.0), Vector3(0.0, 1.085, 0.585)], [0.007, 0.007, 0.007], 4, steel, true, true)
	for step in 8:
		var xa := lerpf(-0.38, 0.38, float(step) / 8.0)
		var xb := lerpf(-0.38, 0.38, float(step + 1) / 8.0)
		var ya: float = arc.call(xa, 0.0)
		var yb: float = arc.call(xb, 0.0)
		var normal := Vector3(-(yb - ya), xb - xa, 0.0).normalized()
		shade_quad(glass, [Vector3(xa, ya, -0.545), Vector3(xb, yb, -0.545), Vector3(xb, yb, 0.592), Vector3(xa, ya, 0.592)], normal, [Color.WHITE, Color.WHITE, Color.WHITE, Color.WHITE])
	# Luminaire hanging under the ridge (1.2 cm tall so it stays solid at range).
	F.box(glow, Vector3(0.0, 1.05, 0.02), Vector3(0.012, 0.006, 0.5), 0.0, linear(LAMP, 0.85))
	F.box(paint, Vector3(0.0, 1.06, 0.02), Vector3(0.006, 0.004, 0.5), 0.0, linear(STEEL, 0.9))

## Stainless handrails on the side walls and a centre rail on its posts, 27
## cm over the front steps and landing, then pitched down toward the mouth as
## if following a steep flight, with wall brackets.
static func _entrance_rails(paint: F.Builder) -> void:
	var rail := linear(RAIL)
	var path := [Vector2(-0.585, RISE + 0.27), Vector2(-0.545, LANDING + 0.27), Vector2(WELL_EDGE, LANDING + 0.27), Vector2(MOUTH - 0.01, FLOOR + 0.12)]
	for s: float in [-1.0, 1.0]:
		var centres := []
		var radii := []
		for point: Vector2 in path:
			centres.append(Vector3(0.317 * s, point.y, point.x))
			radii.append(0.0065)
		F.tube(paint, centres, radii, 6, rail, true, true)
		for z: float in [-0.5, -0.3, -0.1, 0.06]:
			var y := _rail_height(path, z)
			F.box(paint, Vector3(0.3245 * s, y - 0.008, z), Vector3(0.0055, 0.0025, 0.0025), 0.0, rail)
	var middle := [Vector2(WELL_EDGE + 0.02, LANDING + 0.27), Vector2(MOUTH - 0.03, FLOOR + 0.12)]
	F.tube(paint, middle.map(func(point: Vector2) -> Vector3: return Vector3(0.0, point.y, point.x)), [0.0065, 0.0065], 6, rail, true, true)
	for z: float in [-0.4, -0.2, 0.0]:
		# Posts stand on the tread below them.
		var floor_y := _floor_at(z)
		var top := _rail_height(middle, z)
		F.box(paint, Vector3(0.0, (floor_y + top) * 0.5, z), Vector3(0.005, (top - floor_y) * 0.5, 0.005), 0.0, rail)

static func _rail_height(path: Array, z: float) -> float:
	for index in path.size() - 1:
		var a: Vector2 = path[index]
		var b: Vector2 = path[index + 1]
		if z <= b.x: return lerpf(a.y, b.y, clampf((z - a.x) / (b.x - a.x), 0.0, 1.0))
	return (path[-1] as Vector2).y

## Station sign: surfaces paint, stone and glow.
static func station_sign(lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var stone := F.Builder.new()
	var glow := F.Builder.new()
	var steel := linear("2f3a46")
	var faces := [Vector3.FORWARD, Vector3.BACK, Vector3.LEFT, Vector3.RIGHT]
	# Backlit panels at 0.62: a soft glow at night instead of a clipped white.
	var panel := linear(WHITE, 0.62)
	if lod == "near":
		F.box(stone, Vector3(0.0, (0.06 - SKIRT) * 0.5, 0.0), Vector3(0.115, (0.06 + SKIRT) * 0.5, 0.115), 0.006, linear("8d887f"), Basis(), 0.75)
		F.box(stone, Vector3(0.0, 0.07, 0.0), Vector3(0.095, 0.01, 0.095), 0.003, linear(COPING))
		F.box(paint, Vector3(0.0, 0.68, 0.0), Vector3(0.08, 0.6, 0.05), 0.016, steel, Basis(), 0.8)
		F.box(paint, Vector3(0.0, 1.29, 0.0), Vector3(0.07, 0.012, 0.045), 0.003, linear(STEEL, 0.8))
		# Light box body (faces 0.104) under its panels one LAYER proud.
		F.box(paint, Vector3(0.0, 1.54, 0.0), Vector3(0.104, 0.245, 0.104), 0.008, steel)
		F.box(paint, Vector3(0.0, 1.79, 0.0), Vector3(0.114, 0.008, 0.114), 0.003, linear(STEEL, 1.15))
		for face: Vector3 in faces:
			var c := face * (0.104 + LAYER)
			rect(glow, c + Vector3.UP * 1.5425, face, Vector2(0.094, 0.2225), panel, 0.006)
			pictogram(glow, c + face * LAYER + Vector3.UP * 1.635, face, 0.15)
			name_bars(glow, c + face * LAYER + Vector3.UP * 1.46, face, Vector2(0.08, 0.035), linear(INK))
			rect(glow, c + face * LAYER + Vector3.UP * 1.375, face, Vector2(0.094, 0.018), linear(LINE))
		for face: Vector3 in [Vector3.FORWARD, Vector3.BACK]:
			var c := face * (0.05 + LAYER)
			rect(paint, c + Vector3.UP * 1.1, face, Vector2(0.066, 0.15), linear(BAND), 0.01)
			name_bars(glow, c + face * LAYER + Vector3.UP * 1.14, face, Vector2(0.052, 0.05), linear(WHITE, 0.85))
			rect(glow, c + face * LAYER + Vector3.UP * 1.02, face, Vector2(0.052, 0.006), linear(LINE))
			rect(glow, c + Vector3.UP * 0.53, face, Vector2(0.008, 0.38), linear(LINE))
	else:
		F.box(stone, Vector3(0.0, (0.08 - SKIRT) * 0.5, 0.0), Vector3(0.115, (0.08 + SKIRT) * 0.5, 0.115), 0.0, linear("8d887f"))
		F.box(paint, Vector3(0.0, 0.68, 0.0), Vector3(0.08, 0.6, 0.05), 0.0, steel)
		F.box(paint, Vector3(0.0, 1.545, 0.0), Vector3(0.104, 0.25, 0.104), 0.0, steel)
		for face: Vector3 in faces:
			rect(glow, face * (0.104 + LAYER) + Vector3.UP * 1.56, face, Vector2(0.094, 0.2), panel)
			rect(glow, face * (0.104 + 2.0 * LAYER) + Vector3.UP * 1.62, face, Vector2(0.07, 0.07), linear(LINE))
			rect(glow, face * (0.104 + 2.0 * LAYER) + Vector3.UP * 1.39, face, Vector2(0.094, 0.018), linear(LINE))
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	stone.commit(mesh, materials.get("stone"))
	glow.commit(mesh, materials.get("glow"))
	mesh.resource_name = "mk_station_sign_" + lod
	return mesh

## Shared material of `kind`: paint, stone and glow use the environment
## context's surface shader (weather, detail tiles; glow is the street-lamp
## night role with vertex tints) when given, else a plain material for
## standalone previews; glass is one translucent standard material.
static func material(kind: String, context: RefCounted = null) -> Material:
	if kind == "glass":
		var pane := StandardMaterial3D.new()
		pane.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		pane.albedo_color = GLASS
		pane.metallic = 0.2
		pane.metallic_specular = 0.7
		pane.roughness = 0.06
		pane.cull_mode = BaseMaterial3D.CULL_DISABLED
		pane.resource_name = "mk_transit_glass"
		return pane
	var surface: Dictionary = SURFACES[kind]
	if context != null and context.has_method("generated_material"):
		var made: ShaderMaterial = context.generated_material(Color.WHITE, str(surface.tile), float(surface.metallic), float(surface.roughness))
		made.set_shader_parameter("vertex_tinted", true)
		made.set_shader_parameter("lighting_role", 2 if kind == "glow" else 0)
		made.resource_name = "mk_transit_" + kind
		return made
	var plain := StandardMaterial3D.new()
	plain.vertex_color_use_as_albedo = true
	plain.metallic = float(surface.metallic)
	plain.roughness = float(surface.roughness)
	plain.cull_mode = BaseMaterial3D.CULL_DISABLED
	plain.resource_name = "mk_transit_" + kind
	return plain

## One part per call (a material or a mesh); true when the whole set is in
## state.result. `lease` (track(Object)) receives every created resource.
## The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {"materials": {}}
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "transit_" + step
	var result: Dictionary = state.result
	if step.begins_with("material_"):
		var made := material(step.trim_prefix("material_"), context)
		result.materials[step.trim_prefix("material_")] = made
		if lease != null: lease.track(made)
	else:
		var lod := step.get_slice("_", step.get_slice_count("_") - 1)
		var mesh := subway_entrance(lod, result.materials) if step.begins_with("subway_entrance") else station_sign(lod, result.materials)
		if lease != null: lease.track(mesh)
		result[step] = mesh
	state.step += 1
	if state.step < STEPS.size(): return false
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

## Claims the shared set as a cell begins (chunk_renderer.begin, before any
## upload) when it holds subway entrances or station signs and has a resource
## cache; false when the cache refuses it (the cell fails before uploading
## anything: its entrances and signs would lose their only body).
static func claim(job: Dictionary, objects: Array) -> bool:
	if job.get("resources") == null or job.claims.has(KEY): return true
	for object: Dictionary in objects:
		if not KINDS.has(str(object.asset_id)): continue
		var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
		if key.is_empty(): return false
		job.claims[KEY] = key
		return true
	return true

## Shared set for a renderer job: 1 ready, 0 another step is needed, -2 it is
## unavailable, which fails the cell (chunk_renderer): its entrances and signs
## would lose their only body. A refused claim already failed the cell in
## begin().
static func ready(job: Dictionary) -> int:
	if job.has("transit"): return 1
	job.last_phase = "transit_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.transit = made
		return 0
	# begin() claimed the set (claim) or failed the cell.
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.transit = made
	return 0

## One admitted step per object: its transform (street_furniture_meshes
## placement: position, quarter turns, yaw offset) and id join the cell's group
## of its kind and 32 m square, attached one group per step after the objects.
static func plan(job: Dictionary, object: Dictionary) -> void:
	var kind: String = KINDS[str(object.asset_id)]
	var transform := F.placement(object)
	var o := transform.origin
	var key := "%s/%d,%d" % [kind, floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
	if not job.has("transit_plan"): job.transit_plan = {}
	if not job.transit_plan.has(key): job.transit_plan[key] = {"kind": kind, "parts": [], "ids": PackedStringArray()}
	var b := transform.basis
	job.transit_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z]))
	job.transit_plan[key].ids.append(str(object.id))

## Near and far MultiMeshes of one planned group. The cell lease carries the
## nodes and instance buffers; meshes and materials belong to the shared set.
## mapkit_object_ids lists the instances' object ids (editor picking, as for
## street furniture).
static func attach_group(job: Dictionary) -> void:
	var key: String = job.transit_plan.keys()[0]
	var group: Dictionary = job.transit_plan[key]
	job.transit_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in LODS:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.mesh = job.transit[group.kind + "_" + lod]
		multi.instance_count = buffer.size() / 12
		multi.buffer = buffer
		var node := MultiMeshInstance3D.new()
		node.name = "Transit_%s_%s_%d" % [group.kind, lod, job.root.get_child_count()]
		node.multimesh = multi
		node.set_meta("mapkit_static_detail", true)
		node.set_meta("mapkit_decoration", false)
		node.set_meta("mapkit_furniture_lod", lod)
		node.set_meta("mapkit_furniture_kind", group.kind)
		node.set_meta("mapkit_object_ids", group.ids)
		QUALITY.apply_node(node, quality)
		if job.get("lease") != null:
			job.lease.track(node)
			job.lease.track(multi)
		job.root.add_child(node)
