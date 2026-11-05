extends RefCounted
## Display-only road signs, built procedurally (no external assets, fonts,
## real text or logos): post-mounted regulatory and warning plates in generic
## traffic-control shapes and an overhead cantilever direction board. Digits
## are seven-segment bars; names are glyphs of the invented storefront script
## (facade_signs.gdshaderinc: 3-5 strokes on a 3 x 4 lattice, fixed bits per
## sign), drawn as geometry. Each kind has `near` parts (full detail) and a
## simple `far` mesh: speed limits share one body (near and far) under their
## own digits, guide signs one frame under their board layout (parts()).
## Materials are one shared set: painted metal (posts,
## brackets, aluminium backs and edges) and the printed faces, vertex tinted
## and lit softly at night through the street-lamp role (atmosphere_surface
## lighting_role 2, standing in for retroreflection; printed colours stay at
## about 0.8 so the glow never clips).
##
## Metres, origin at the foot, local -z is the printed front: the importer
## turns local map +y (scene -z) against the traffic heading
## (osm_furniture.road_signs), so local +x points over the road under
## right-hand driving.
##
## Pole signs (builtin:sign_speed_30/40/50/60/80/100, sign_crossing,
## sign_stop, sign_yield, sign_children; collision: the 6 x 104 x 6 cm post
## box, placement.rs): a foot sleeve (reaching SKIRT below the foot), a
## slightly tapered round post with a domed cap, a back channel and two band
## clamps holding an aluminium plate whose rolled edge and plain back carry a
## stiffener rail. Printed faces are tiled coplanar regions (a ring and its
## disc, a band and its field) with symbols PROUD in front: speed limits a
## white rim, red ring and white disc with black digits; the crossing sign a
## blue chamfered square with a white border, a walking figure and zebra
## bars; stop a red inverted triangle with a white border and two white
## glyphs; yield the same triangle with a wide red band, white field and two
## red glyphs; children protection a yellow triangle with a red band, two
## black child figures holding hands and a white supplementary plate with a
## dark border and a run of four glyphs. Everything below 0.66 m stays inside
## the post box; plates stay within 16 cm of the post across x (the
## importer's footprint) and 6 cm along z.
##
## Guide sign (builtin:guide_sign; collision: the 10 x 250 x 10 cm pole box):
## a base plate with anchor bolts and a concrete collar, a tapered round pole
## with a cap, two cantilever pipes (2.44 m and 1.725 m up, end caps, a
## diagonal web every 0.43 m, gussets at the pole) and a 1.70 x 0.60 m board
## from 1.78 m up in front of them with Z-bar stiffeners and brackets with
## U-bolts on its back: a blue field with a white border, white arrows, glyph
## runs and white route shields with two blue digits. Four board layouts
## (VARIANTS) are chosen by the object identity: a junction diagram, straight
## with a right branch, left with straight, and an overhead lane board; the
## frame and blank board are one part, each layout's print another. The far
## mesh keeps the open truss as two arm bars and one diagonal. Only the pole
## stands below 1.70 m; the arms and board reach 2.58 m along +x (the
## importer's 2.6 m guide reach), clear of vehicles.
##
## The renderer draws these meshes instead of the generated proxy faces
## (render_memory.DRAWN_OVER); collision and occluders never read those faces.
## A cell instances each kind (and guide layout) per 32 m group as one near
## MultiMesh per part and a far MultiMesh with street_furniture_meshes' range
## metadata (display_quality.apply_furniture: near within furniture_near, the
## far mesh beyond and always drawn like the proxy it replaces, low shows the
## far mesh only, shadows on every tier). build() makes one part per call for
## admitted streaming steps (render_resource_cache.prepare_mesh_set);
## step_shared() builds a process-lifetime set for cells without a resource
## cache.

const QUALITY := preload("./display_quality.gd")
const F := preload("./street_furniture_meshes.gd")
const KINDS := {"builtin:sign_speed_30": "speed_30", "builtin:sign_speed_40": "speed_40", "builtin:sign_speed_50": "speed_50",
	"builtin:sign_speed_60": "speed_60", "builtin:sign_speed_80": "speed_80", "builtin:sign_speed_100": "speed_100",
	"builtin:sign_crossing": "crossing", "builtin:sign_stop": "stop", "builtin:sign_yield": "yield",
	"builtin:sign_children": "children", "builtin:guide_sign": "guide"}
const POLE_KINDS := ["speed_30", "speed_40", "speed_50", "speed_60", "speed_80", "speed_100", "crossing", "stop", "yield", "children"]
## Board layouts chosen by object identity.
const VARIANTS := {"guide": 4}
const LODS := ["near", "far"]
const KEY := "builtin:road_signs"
const GROUP_METRES := 32.0
## Collision boxes (placement.rs builtin), metres.
const POST_PROXY := AABB(Vector3(-0.03, 0.0, -0.03), Vector3(0.06, 1.04, 0.06))
const GUIDE_PROXY := AABB(Vector3(-0.05, 0.0, -0.05), Vector3(0.1, 2.5, 0.1))
## Sleeves and footings reach this far below the foot (sloping ground).
const SKIRT := 0.04
## Pole plates: printed front and aluminium back planes (local z).
const FRONT := -0.034
const BACK := -0.028
## Symbols stand this far in front of the face they are printed on.
const PROUD := 0.0015
## Guide board: printed front, back plane, extent and the lowest display part.
const BOARD_FRONT := -0.06
const BOARD_BACK := -0.05
const BOARD := Rect2(0.82, 1.78, 1.7, 0.6)
const CLEAR_HEIGHT := 1.70
const LOWER_ARM := 1.725
## Segments of a near plate circle.
const CIRCLE := 24
## Printed colours (sRGB, shaded to about 0.8 so the night glow never clips).
const PRINT := 0.8
const WHITE := "f4f5f0"
const RED := "c8202a"
const BLUE := "1d4f9c"
const YELLOW := "f2c21b"
const BLACK := "141414"
## Paint (sRGB).
const POST := "8d949a"
const ALUMINIUM := "b9bec1"
const CLAMP := "6f767b"
const CONCRETE := "a39f96"
const SURFACES := {"sign": {"tile": "", "metallic": 0.0, "roughness": 0.45}}
## Glyph bits per sign (fixed constants: not text).
const GLYPHS := {"stop": 0x5a17, "yield": 0x3c41, "children": 0x77e2, "guide": 0x1b93, "shield": 0}
const STEPS := ["material_paint", "material_sign", "speed_near", "speed_far", "speed_30_digits", "speed_40_digits",
	"speed_50_digits", "speed_60_digits", "speed_80_digits", "speed_100_digits", "crossing_near", "crossing_far",
	"stop_near", "stop_far", "yield_near", "yield_far", "children_near", "children_far",
	"guide_near", "guide_far", "guide_board_0", "guide_board_1", "guide_board_2", "guide_board_3"]
## Memory allowance of one shared set (render_resource_cache): about 6,600
## vertices at <= 56 B and 14,600 indices at 4 B in CPU arrays and GPU
## buffers (0.86 MB; road_sign_validator measures them), 64 KiB for the two
## materials and one mesh's build arrays.
const SHARED_BYTES := 960 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

static func linear(srgb: String, shade := 1.0) -> Color:
	return F.linear(srgb, shade)

static func ink(srgb: String) -> Color:
	return F.linear(srgb, PRINT)

## Variant of a guide board (0..count-1) from the object identity.
static func variant(object: Dictionary, count: int) -> int:
	return int(float(str(object.id).sha256_text().substr(0, 6).hex_to_int()) / 16777216.0 * float(count)) % count

# Faces ------------------------------------------------------------------------

## A printed face: `centre`, its outward `normal` (horizontal) and the
## viewer's right; points are (u right, v up) in metres.
static func face(centre: Vector3, normal: Vector3) -> Dictionary:
	return {"centre": centre, "normal": normal, "right": Vector3.UP.cross(normal).normalized()}

static func at(f: Dictionary, p: Vector2, layer := 0.0) -> Vector3:
	return (f.centre as Vector3) + (f.right as Vector3) * p.x + Vector3.UP * p.y + (f.normal as Vector3) * (PROUD * layer)

## Convex polygon on a face, `layer` PROUD steps in front.
static func fill(b: F.Builder, f: Dictionary, points: Array, layer: float, color: Color) -> void:
	var placed := []
	for p: Vector2 in points: placed.append(at(f, p, layer))
	F.polygon(b, placed, f.normal, color)

## Band between two polygons with matching vertices (outer and inset inner).
static func band(b: F.Builder, f: Dictionary, outer: Array, inner: Array, layer: float, color: Color) -> void:
	for k in outer.size():
		var n := (k + 1) % outer.size()
		var first := b.vertices.size()
		for p: Vector2 in [outer[k], outer[n], inner[n], inner[k]]: b.add(at(f, p, layer), f.normal, color)
		b.triangle(first, first + 1, first + 2)
		b.triangle(first, first + 2, first + 3)

## Stroke from `a` to `c` of width `width` with square ends (half a width past
## each end when `capped`).
static func stroke(b: F.Builder, f: Dictionary, a: Vector2, c: Vector2, width: float, layer: float, color: Color, capped := true) -> void:
	var along := (c - a).normalized() if c != a else Vector2.RIGHT
	var across := Vector2(-along.y, along.x) * width * 0.5
	var reach := along * width * 0.5 if capped else Vector2.ZERO
	fill(b, f, [a - reach - across, c + reach - across, c + reach + across, a - reach + across], layer, color)

## Rim of a plate outline from its front back to `depth` (outward normals)
## and its back face.
static func plate(b: F.Builder, f: Dictionary, points: Array, depth: float, color: Color, back := true) -> void:
	var count := points.size()
	var centre := Vector2.ZERO
	for p: Vector2 in points: centre += p
	centre /= float(count)
	var rear := (f.normal as Vector3) * -depth
	for k in count:
		var p: Vector2 = points[k]
		var q: Vector2 = points[(k + 1) % count]
		var edge := q - p
		var out := Vector2(edge.y, -edge.x).normalized()
		if out.dot((p + q) * 0.5 - centre) < 0.0: out = -out
		var normal := (f.right as Vector3) * out.x + Vector3.UP * out.y
		var first := b.vertices.size()
		for point: Vector3 in [at(f, p), at(f, q), at(f, q) + rear, at(f, p) + rear]: b.add(point, normal, color)
		b.triangle(first, first + 1, first + 2)
		b.triangle(first, first + 2, first + 3)
	if back:
		var placed := []
		for p: Vector2 in points: placed.append(at(f, p) + rear)
		F.polygon(b, placed, -(f.normal as Vector3), color)

# Outlines ---------------------------------------------------------------------

static func circle(radius: float, sides: int, centre := Vector2.ZERO) -> Array:
	var points := []
	for k in sides: points.append(centre + Vector2.from_angle(TAU * (float(k) + 0.5) / float(sides)) * radius)
	return points

## Equilateral triangle of `side` about its centroid, apex up (or down).
static func triangle(side: float, down: bool) -> Array:
	var r := side / sqrt(3.0)
	var s := -1.0 if down else 1.0
	return [Vector2(0.0, r * s), Vector2(-side * 0.5, -r * 0.5 * s), Vector2(side * 0.5, -r * 0.5 * s)]

## Axis-aligned rectangle (centre, half size) with corners cut by `cut`.
static func chamfered(centre: Vector2, half: Vector2, cut: float) -> Array:
	if cut <= 0.0:
		return [centre + Vector2(-half.x, -half.y), centre + Vector2(half.x, -half.y), centre + Vector2(half.x, half.y), centre + Vector2(-half.x, half.y)]
	return [centre + Vector2(-half.x + cut, -half.y), centre + Vector2(half.x - cut, -half.y), centre + Vector2(half.x, -half.y + cut),
		centre + Vector2(half.x, half.y - cut), centre + Vector2(half.x - cut, half.y), centre + Vector2(-half.x + cut, half.y),
		centre + Vector2(-half.x, half.y - cut), centre + Vector2(-half.x, -half.y + cut)]

## Convex polygon moved inward by `distance` (each edge's line offset, then
## consecutive lines intersected): vertices keep their correspondence.
static func inset(points: Array, distance: float) -> Array:
	var count := points.size()
	var area := 0.0
	for k in count: area += (points[k] as Vector2).cross(points[(k + 1) % count])
	var turn := 1.0 if area > 0.0 else -1.0
	var lines := []
	for k in count:
		var p: Vector2 = points[k]
		var q: Vector2 = points[(k + 1) % count]
		var inward := Vector2(-(q - p).y, (q - p).x).normalized() * turn
		lines.append([p + inward * distance, (q - p).normalized()])
	var result := []
	for k in count:
		var a: Array = lines[(k - 1 + count) % count]
		var c: Array = lines[k]
		var denominator := (a[1] as Vector2).cross(c[1])
		if absf(denominator) < 1e-9:
			result.append(c[0])
			continue
		var t := ((c[0] as Vector2) - (a[0] as Vector2)).cross(c[1]) / denominator
		result.append((a[0] as Vector2) + (a[1] as Vector2) * t)
	return result

## Corners of a convex polygon rounded with `radius` (`segments` per arc);
## rounded(inset(p, d), r - d) keeps rounded(p, r)'s arc centres, so the two
## match vertex for vertex as band() needs.
static func rounded(points: Array, radius: float, segments := 3) -> Array:
	var count := points.size()
	var result := []
	var core := inset(points, radius)
	for k in count:
		var p: Vector2 = points[k]
		var before: Vector2 = points[(k - 1 + count) % count]
		var after: Vector2 = points[(k + 1) % count]
		var n0 := _outward(before, p, core[k])
		var n1 := _outward(p, after, core[k])
		var a0 := n0.angle()
		var sweep := wrapf(n1.angle() - a0, -PI, PI)
		for s in segments + 1:
			result.append((core[k] as Vector2) + Vector2.from_angle(a0 + sweep * float(s) / float(segments)) * maxf(radius, 0.0))
	return result

static func _outward(p: Vector2, q: Vector2, inside: Vector2) -> Vector2:
	var normal := Vector2((q - p).y, -(q - p).x).normalized()
	return -normal if normal.dot(inside - p) > 0.0 else normal

## Rounded outline of `points` grown back out by `radius`: the outline whose
## inset by `d` is rounded(inset(points, d), radius - d).
static func outline(points: Array, radius: float, depth: float, segments := 3) -> Array:
	return rounded(inset(points, depth), maxf(radius - depth, 0.0005), segments)

# Symbols ----------------------------------------------------------------------

## Seven-segment bars: a top, b upper right, c lower right, d bottom, e lower
## left, f upper left, g middle.
const SEGMENTS := ["abcdef", "bc", "abged", "abgcd", "fgbc", "afgcd", "afgedc", "abc", "abcdefg", "abcdfg"]

## Digit `value` in a cell of `size` (width, height) centred at `centre`,
## bars `width` thick with pointed (mitred) ends and small gaps (`plain`:
## square-ended bars, half the triangles, for small print).
static func digit(b: F.Builder, f: Dictionary, value: int, centre: Vector2, size: Vector2, width: float, layer: float, color: Color, plain := false) -> void:
	var hx := size.x * 0.5 - width * 0.5
	var hy := size.y * 0.5 - width * 0.5
	var gap := width * 0.12
	var bars := {"a": [Vector2(-hx, hy), Vector2(hx, hy)], "g": [Vector2(-hx, 0.0), Vector2(hx, 0.0)], "d": [Vector2(-hx, -hy), Vector2(hx, -hy)],
		"f": [Vector2(-hx, hy), Vector2(-hx, 0.0)], "b": [Vector2(hx, hy), Vector2(hx, 0.0)],
		"e": [Vector2(-hx, 0.0), Vector2(-hx, -hy)], "c": [Vector2(hx, 0.0), Vector2(hx, -hy)]}
	for key: String in SEGMENTS[value]:
		var p: Vector2 = centre + bars[key][0]
		var q: Vector2 = centre + bars[key][1]
		var along := (q - p).normalized()
		var across := Vector2(-along.y, along.x) * width * 0.5
		var tip := along * gap
		var shoulder := along * (gap + width * 0.5)
		if plain:
			fill(b, f, [p + shoulder - across, q - shoulder - across, q - shoulder + across, p + shoulder + across], layer, color)
			continue
		fill(b, f, [p + tip, p + shoulder + across, q - shoulder + across, q - tip, q - shoulder - across, p + shoulder - across], layer, color)

## A number centred at `centre` (cells of `size`, `spacing` apart).
static func number(b: F.Builder, f: Dictionary, value: int, centre: Vector2, size: Vector2, width: float, spacing: float, layer: float, color: Color, plain := false) -> void:
	var text := str(value)
	var span := float(text.length() - 1) * spacing
	for k in text.length():
		digit(b, f, int(text[k]), centre + Vector2(-span * 0.5 + spacing * float(k), 0.0), size, width, layer, color, plain)

## The storefront script's hash (facade_pcg), 32-bit.
static func pcg(value: int) -> int:
	var s := (value * 747796405 + 2891336453) & 0xFFFFFFFF
	var w := (((s >> ((s >> 28) + 4)) ^ s) * 277803737) & 0xFFFFFFFF
	return ((w >> 22) ^ w) & 0xFFFFFFFF

const GLYPH_STEPS := [Vector2i(1, 0), Vector2i(0, 1), Vector2i(1, 1), Vector2i(1, -1), Vector2i(0, 2), Vector2i(2, 0)]
const GLYPH_ADVANCE := 1.04
const GLYPH_STROKE := 0.16

## One glyph of `bits` (the storefront script: 3-5 strokes between
## neighbouring points of a 3 x 4 lattice, a dot one time in five) in its
## 0.8 x 0.9 box of `scale` metres per glyph unit, top-left at `origin`.
static func glyph(b: F.Builder, f: Dictionary, bits: int, origin: Vector2, scale: float, layer: float, color: Color) -> void:
	var point := func(i: int, j: int) -> Vector2: return origin + Vector2(float(i) * 0.4, -float(j) * 0.3) * scale
	var count := 3 + pcg(bits ^ 1759714724) % 3
	for k in count:
		var h := pcg((bits + k * 2654435769) & 0xFFFFFFFF)
		var step: Vector2i = GLYPH_STEPS[(h >> 16) % 6]
		var i := mini(h % 3, 2 - step.x)
		var j := (h >> 8) % 4
		j = mini(j, 3 - step.y) if step.y > 0 else maxi(j, -step.y)
		stroke(b, f, point.call(i, j), point.call(i + step.x, j + step.y), GLYPH_STROKE * scale, layer, color)
	var dot := pcg(bits ^ 742938285)
	if dot % 5 == 0:
		var c: Vector2 = point.call((dot >> 8) % 3, (dot >> 12) % 4)
		fill(b, f, chamfered(c, Vector2.ONE * 0.1 * scale, 0.03 * scale), layer, color)

## A run of `count` glyphs `height` m tall centred at `centre`.
static func run(b: F.Builder, f: Dictionary, bits: int, count: int, centre: Vector2, height: float, layer: float, color: Color) -> void:
	var scale := height / 0.9
	var width := (float(count - 1) * GLYPH_ADVANCE + 0.8) * scale
	var origin := centre + Vector2(-width * 0.5, height * 0.5)
	for g in count:
		var seed_bits := pcg(bits ^ ((g * 668265263 + 374761393) & 0xFFFFFFFF))
		glyph(b, f, seed_bits, origin + Vector2(float(g) * GLYPH_ADVANCE * scale, 0.0), scale, layer, color)

## Run width (metres) of `count` glyphs `height` tall.
static func run_width(count: int, height: float) -> float:
	return (float(count - 1) * GLYPH_ADVANCE + 0.8) * height / 0.9

## Arrow along `points` (a polyline) of stem `width`, ending in a triangular
## head `head` long and `spread` wide.
static func arrow(b: F.Builder, f: Dictionary, points: Array, width: float, head: float, spread: float, layer: float, color: Color) -> void:
	var tip: Vector2 = points[-1]
	var last: Vector2 = points[-2]
	var along := (tip - last).normalized()
	var base := tip - along * head
	for k in points.size() - 1:
		var p: Vector2 = points[k]
		var q: Vector2 = base if k == points.size() - 2 else points[k + 1]
		# Flat stem ends; a square fills each bend; the head starts at the stem end.
		stroke(b, f, p, q, width, layer, color, false)
		if k > 0: fill(b, f, chamfered(p, Vector2.ONE * width * 0.5, 0.0), layer, color)
	var across := Vector2(-along.y, along.x) * spread * 0.5
	fill(b, f, [tip, base + across, base - across], layer, color)

## Walking figure (crossing sign) at `centre`, about `size` m tall.
static func walker(b: F.Builder, f: Dictionary, centre: Vector2, size: float, layer: float, color: Color) -> void:
	var p := func(x: float, y: float) -> Vector2: return centre + Vector2(x, y) * size
	fill(b, f, circle(0.13 * size, 8, p.call(0.05, 0.4)), layer, color)
	stroke(b, f, p.call(0.03, 0.24), p.call(-0.04, -0.06), 0.17 * size, layer, color, false)
	stroke(b, f, p.call(-0.04, -0.06), p.call(0.2, -0.42), 0.12 * size, layer, color)
	stroke(b, f, p.call(-0.04, -0.06), p.call(-0.26, -0.4), 0.12 * size, layer, color)
	stroke(b, f, p.call(0.02, 0.2), p.call(0.24, -0.02), 0.09 * size, layer, color)
	stroke(b, f, p.call(0.02, 0.2), p.call(-0.2, 0.04), 0.09 * size, layer, color)

## Child figure: a head over a trapezoid body on two legs, `height` m tall.
static func child(b: F.Builder, f: Dictionary, foot: Vector2, height: float, layer: float, color: Color) -> void:
	var p := func(x: float, y: float) -> Vector2: return foot + Vector2(x, y) * height
	fill(b, f, circle(0.13 * height, 8, p.call(0.0, 0.86)), layer, color)
	fill(b, f, [p.call(-0.1, 0.7), p.call(0.1, 0.7), p.call(0.19, 0.28), p.call(-0.19, 0.28)], layer, color)
	stroke(b, f, p.call(-0.08, 0.29), p.call(-0.1, 0.02), 0.09 * height, layer, color)
	stroke(b, f, p.call(0.08, 0.29), p.call(0.1, 0.02), 0.09 * height, layer, color)

# Posts ------------------------------------------------------------------------

## Post with a foot sleeve and a domed cap, inside the 6 cm box; band clamps
## at `clamps` (heights) and a back channel from `low` to `high`.
static func post(paint: F.Builder, near: bool, clamps: Array, low: float, high: float) -> void:
	var color := linear(POST)
	if not near:
		F.box(paint, Vector3(0.0, (1.04 - SKIRT) * 0.5, 0.0), Vector3(0.02, (1.04 + SKIRT) * 0.5, 0.02), 0.0, color)
		return
	F.tube(paint, [Vector3(0.0, -SKIRT, 0.0), Vector3(0.0, 0.0, 0.0), Vector3(0.0, 0.03, 0.0)], [0.026, 0.026, 0.0255], 12, linear(POST, 0.78), false, true)
	F.tube(paint, [Vector3(0.0, 0.03, 0.0), Vector3(0.0, 0.5, 0.0), Vector3(0.0, 1.035, 0.0)], [0.021, 0.02, 0.019], 12, color)
	F.tube(paint, [Vector3(0.0, 1.03, 0.0), Vector3(0.0, 1.038, 0.0), Vector3(0.0, 1.045, 0.0)], [0.0195, 0.016, 0.007], 12, linear(POST, 0.95), false, true)
	F.box(paint, Vector3(0.0, (low + high) * 0.5, -0.02425), Vector3(0.006, (high - low) * 0.5, 0.00375), 0.0, linear(CLAMP))
	for y: float in clamps:
		F.tube(paint, [Vector3(0.0, y - 0.008, 0.0), Vector3(0.0, y, 0.0), Vector3(0.0, y + 0.008, 0.0)], [0.0235, 0.0235, 0.0235], 12, linear(CLAMP), true, true)
		F.box(paint, Vector3(0.0, y, -0.024), Vector3(0.011, 0.008, 0.004), 0.0, linear(CLAMP))

## Aluminium plate of `points` (on face `f`): rolled edge, back face and a
## horizontal stiffener rail behind it.
static func backing(paint: F.Builder, f: Dictionary, points: Array, near: bool) -> void:
	plate(paint, f, points, BACK - FRONT, linear(ALUMINIUM))
	if not near: return
	var low := INF
	var high := -INF
	var left := INF
	var right := -INF
	for p: Vector2 in points:
		low = minf(low, p.y)
		high = maxf(high, p.y)
		left = minf(left, p.x)
		right = maxf(right, p.x)
	# At the middle of its height, inside the outline (triangles narrow).
	var middle := (low + high) * 0.5
	var inside := INF
	for k in points.size():
		var p: Vector2 = points[k]
		var q: Vector2 = points[(k + 1) % points.size()]
		if (p.y - middle) * (q.y - middle) <= 0.0 and p.y != q.y: inside = minf(inside, absf(lerpf(p.x, q.x, (middle - p.y) / (q.y - p.y))))
	var span := minf((right - left) * 0.35, inside - 0.015)
	F.box(paint, Vector3(0.0, (f.centre as Vector3).y + middle, BACK + 0.0018), Vector3(span, 0.006, 0.0018), 0.0, linear(ALUMINIUM, 0.85))

# Pole signs -------------------------------------------------------------------

## Plate centre height and outline (face coordinates about the centre) of a
## pole sign kind.
static func plate_centre(kind: String) -> float:
	match kind:
		"crossing": return 0.905
		"stop", "yield": return 0.9
		"children": return 0.8636
	return 0.905

static func pole_sign(kind: String, lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var signs := F.Builder.new()
	var near := lod == "near"
	var y := plate_centre(kind)
	var f := face(Vector3(0.0, y, FRONT), Vector3.FORWARD)
	var white := ink(WHITE)
	var red := ink(RED)
	var shape: Array
	match kind:
		"crossing":
			shape = rounded(chamfered(Vector2.ZERO, Vector2(0.12, 0.12), 0.0), 0.018, 3) if near else chamfered(Vector2.ZERO, Vector2(0.12, 0.12), 0.0)
		"stop", "yield":
			shape = rounded(triangle(0.3, true), 0.014, 3) if near else triangle(0.3, true)
		"children":
			shape = rounded(triangle(0.3, false), 0.014, 3) if near else triangle(0.3, false)
		_:
			shape = circle(0.13, CIRCLE if near else 8)
	backing(paint, f, shape, near)
	var low := y - 0.12
	var high := y + 0.1
	var clamps := [y - 0.06, y + 0.06]
	if kind.begins_with("speed"):
		# The body shared by every limit; digits are their own parts (digits()).
		var rim := circle(0.126, CIRCLE if near else 8)
		var disc := circle(0.103, CIRCLE if near else 8)
		if near: band(signs, f, shape, rim, 0.0, white)
		band(signs, f, rim if near else shape, disc, 0.0, red)
		fill(signs, f, disc, 0.0, white)
		if not near: fill(signs, f, chamfered(Vector2.ZERO, Vector2(0.055, 0.04), 0.0), 1.0, linear(BLACK, 0.25 * PRINT))
	elif kind == "crossing":
		var blue := ink(BLUE)
		if near:
			var edge := rounded(inset(chamfered(Vector2.ZERO, Vector2(0.12, 0.12), 0.0), 0.008), 0.01, 3)
			var inner := rounded(inset(chamfered(Vector2.ZERO, Vector2(0.12, 0.12), 0.0), 0.018), 0.0005, 3)
			band(signs, f, shape, edge, 0.0, blue)
			band(signs, f, edge, inner, 0.0, white)
			fill(signs, f, inner, 0.0, blue)
			walker(signs, f, Vector2(-0.005, 0.03), 0.13, 1.0, white)
			for k in 4: fill(signs, f, chamfered(Vector2(-0.06 + 0.04 * float(k), -0.068), Vector2(0.011, 0.016), 0.0), 1.0, white)
		else:
			var inner := inset(shape, 0.016)
			band(signs, f, shape, inner, 0.0, white)
			fill(signs, f, inner, 0.0, blue)
	elif kind == "stop" or kind == "yield":
		var stop := kind == "stop"
		var width := 0.012 if stop else 0.035
		var inner := outline(triangle(0.3, true), 0.014, width, 3) if near else inset(shape, width)
		band(signs, f, shape, inner, 0.0, white if stop else red)
		fill(signs, f, inner, 0.0, red if stop else white)
		if near: run(signs, f, GLYPHS[kind], 2, Vector2(0.0, 0.018), 0.05 if stop else 0.042, 1.0, white if stop else red)
		low = y - 0.15
		high = y + 0.07
		clamps = [y - 0.1, y + 0.05]
	else:
		var inner := outline(triangle(0.3, false), 0.014, 0.03, 3) if near else inset(shape, 0.03)
		band(signs, f, shape, inner, 0.0, red)
		fill(signs, f, inner, 0.0, ink(YELLOW))
		if near:
			var black := ink(BLACK)
			child(signs, f, Vector2(-0.022, -0.046), 0.088, 1.0, black)
			child(signs, f, Vector2(0.024, -0.046), 0.074, 1.0, black)
			stroke(signs, f, Vector2(-0.006, -0.002), Vector2(0.01, -0.008), 0.007, 1.0, black)
		# Supplementary plate under the triangle.
		var plate_f := face(Vector3(0.0, 0.735, FRONT), Vector3.FORWARD)
		var board := chamfered(Vector2.ZERO, Vector2(0.1, 0.03), 0.004)
		backing(paint, plate_f, board, near)
		if near:
			var frame := inset(board, 0.004)
			band(signs, plate_f, board, frame, 0.0, ink(BLACK))
			fill(signs, plate_f, frame, 0.0, white)
			run(signs, plate_f, GLYPHS.children, 4, Vector2.ZERO, 0.032, 1.0, ink(BLACK))
		else:
			fill(signs, plate_f, board, 0.0, white)
		low = 0.72
		high = y + 0.06
		clamps = [0.735, y]
	post(paint, near, clamps, low, high)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	signs.commit(mesh, materials.get("sign"))
	mesh.resource_name = "mk_sign_%s_%s" % [kind, lod]
	return mesh

## Digits of a speed limit (surface sign only), PROUD in front of the
## shared speed body.
static func digits(value: int, materials: Dictionary) -> ArrayMesh:
	var signs := F.Builder.new()
	var f := face(Vector3(0.0, plate_centre("speed"), FRONT), Vector3.FORWARD)
	if value >= 100: number(signs, f, value, Vector2.ZERO, Vector2(0.042, 0.09), 0.012, 0.052, 1.0, ink(BLACK))
	else: number(signs, f, value, Vector2.ZERO, Vector2(0.05, 0.09), 0.013, 0.064, 1.0, ink(BLACK))
	var mesh := ArrayMesh.new()
	signs.commit(mesh, materials.get("sign"))
	mesh.resource_name = "mk_sign_speed_%d_digits" % value
	return mesh

# Guide sign -------------------------------------------------------------------

## Overhead guide sign: pole, arms and the blank printed board (the board
## layouts are their own parts, board()).
static func guide(lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var signs := F.Builder.new()
	var near := lod == "near"
	var pole := linear(POST)
	var steel := linear(POST, 0.92)
	var centre := BOARD.get_center()
	var f := face(Vector3(centre.x, centre.y, BOARD_FRONT), Vector3.FORWARD)
	var half := BOARD.size * 0.5
	var shape := rounded(chamfered(Vector2.ZERO, half, 0.0), 0.03, 3 if near else 1)
	plate(paint, f, shape, BOARD_BACK - BOARD_FRONT, linear(ALUMINIUM))
	var blue := ink(BLUE)
	var white := ink(WHITE)
	if near:
		# Base plate with anchor bolts and a concrete collar inside the 10 cm box.
		F.box(paint, Vector3(0.0, (0.03 - SKIRT) * 0.5, 0.0), Vector3(0.049, (0.03 + SKIRT) * 0.5, 0.049), 0.004, linear(CONCRETE), Basis(), 0.8)
		F.box(paint, Vector3(0.0, 0.036, 0.0), Vector3(0.046, 0.006, 0.046), 0.003, steel)
		for sx: float in [-1.0, 1.0]:
			for sz: float in [-1.0, 1.0]:
				F.tube(paint, [Vector3(0.036 * sx, 0.042, 0.036 * sz), Vector3(0.036 * sx, 0.05, 0.036 * sz)], [0.0045, 0.0045], 6, linear(POST, 0.7), false, true)
		F.tube(paint, [Vector3(0.0, 0.04, 0.0), Vector3(0.0, 0.07, 0.0)], [0.046, 0.044], 16, linear(POST, 0.85), false, false)
		F.tube(paint, [Vector3(0.0, 0.06, 0.0), Vector3(0.0, 1.3, 0.0), Vector3(0.0, 2.47, 0.0)], [0.042, 0.039, 0.036], 16, pole)
		F.tube(paint, [Vector3(0.0, 2.465, 0.0), Vector3(0.0, 2.49, 0.0), Vector3(0.0, 2.5, 0.0)], [0.039, 0.037, 0.016], 16, linear(POST, 0.95), false, true)
		# Cantilever pipes with end caps, collars, a diagonal web and gussets.
		for arm: Vector2 in [Vector2(2.44, 0.022), Vector2(LOWER_ARM, 0.018)]:
			F.tube(paint, [Vector3(0.03, arm.x, 0.0), Vector3(1.3, arm.x, 0.0), Vector3(2.56, arm.x, 0.0)], [arm.y, arm.y, arm.y], 10, steel)
			F.tube(paint, [Vector3(2.555, arm.x, 0.0), Vector3(2.57, arm.x, 0.0), Vector3(2.58, arm.x, 0.0)], [arm.y + 0.003, arm.y + 0.003, arm.y * 0.4], 10, linear(POST, 0.85), true, true)
			F.tube(paint, [Vector3(0.0, arm.x - 0.03, 0.0), Vector3(0.0, arm.x, 0.0), Vector3(0.0, arm.x + 0.03, 0.0)], [0.045, 0.045, 0.045], 16, linear(POST, 0.88), true, true)
		var x := 0.16
		var up := true
		while x < 2.5:
			var nx := minf(x + 0.43, 2.52)
			var a := Vector3(x, LOWER_ARM if up else 2.44, 0.0)
			var c := Vector3(nx, 2.44 if up else LOWER_ARM, 0.0)
			F.tube(paint, [a, c], [0.008, 0.008], 6, steel)
			F.tube(paint, [Vector3(x, LOWER_ARM, 0.0), Vector3(x, 2.44, 0.0)], [0.007, 0.007], 6, steel)
			x = nx
			up = not up
		# Gussets: plates above the lower pipe and under the upper one.
		F.polygon(paint, [Vector3(0.04, 1.735, 0.0), Vector3(0.22, 1.735, 0.0), Vector3(0.04, 1.9, 0.0)], Vector3.BACK, steel)
		F.polygon(paint, [Vector3(0.04, 1.735, 0.0), Vector3(0.22, 1.735, 0.0), Vector3(0.04, 1.9, 0.0)], Vector3.FORWARD, steel)
		F.polygon(paint, [Vector3(0.04, 2.425, 0.0), Vector3(0.22, 2.425, 0.0), Vector3(0.04, 2.27, 0.0)], Vector3.BACK, steel)
		F.polygon(paint, [Vector3(0.04, 2.425, 0.0), Vector3(0.22, 2.425, 0.0), Vector3(0.04, 2.27, 0.0)], Vector3.FORWARD, steel)
		# Board back: Z-bar stiffeners and U-bolt clamps to the pipes.
		for y: float in [1.86, 2.08, 2.3]:
			F.box(paint, Vector3(centre.x, y, BOARD_BACK + 0.004), Vector3(half.x - 0.05, 0.012, 0.004), 0.0, linear(ALUMINIUM, 0.82))
		# Brackets from each pipe to the board edge behind it, U-bolts round the pipes.
		for bx: float in [1.1, 2.25]:
			for arm: Vector2 in [Vector2(2.44, 0.022), Vector2(LOWER_ARM, 0.018)]:
				var edge_y := BOARD.end.y - 0.03 if arm.x > 2.0 else BOARD.position.y + 0.03
				F.box(paint, Vector3(bx, (arm.x + edge_y) * 0.5, BOARD_BACK * 0.5), Vector3(0.012, absf(arm.x - edge_y) * 0.5 + 0.008, -BOARD_BACK * 0.5), 0.002, linear(CLAMP))
				F.tube(paint, [Vector3(bx, arm.x, -0.012), Vector3(bx, arm.x, 0.012)], [arm.y + 0.003, arm.y + 0.003], 8, linear(CLAMP), true, true)
		# Board front: blue margin, white border, blue field and the layout.
		var edge := rounded(inset(chamfered(Vector2.ZERO, half, 0.0), 0.015), 0.015, 3)
		var inner := rounded(inset(chamfered(Vector2.ZERO, half, 0.0), 0.027), 0.003, 3)
		band(signs, f, shape, edge, 0.0, blue)
		band(signs, f, edge, inner, 0.0, white)
		fill(signs, f, inner, 0.0, blue)
	else:
		F.tube(paint, [Vector3(0.0, -SKIRT, 0.0), Vector3(0.0, 2.5, 0.0)], [0.042, 0.036], 6, pole, false, true)
		# Two arm bars (as the near pipes) and one diagonal: the truss stays open.
		F.box(paint, Vector3(1.3, 2.44, 0.0), Vector3(1.27, 0.02, 0.02), 0.0, steel)
		F.box(paint, Vector3(1.3, LOWER_ARM, 0.0), Vector3(1.27, 0.018, 0.018), 0.0, steel)
		F.bar(paint, Vector3(0.16, LOWER_ARM, 0.0), Vector3(0.8, 2.44, 0.0), Vector3.FORWARD, 0.016, 0.016, 0.0, steel)
		var inner := inset(shape, 0.025)
		band(signs, f, shape, inner, 0.0, white)
		fill(signs, f, inner, 0.0, blue)
		fill(signs, f, chamfered(Vector2(-0.45, 0.0), Vector2(0.025, 0.17), 0.0), 1.0, white)
		fill(signs, f, chamfered(Vector2(0.4, 0.0), Vector2(0.2, 0.025), 0.0), 1.0, white)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	signs.commit(mesh, materials.get("sign"))
	mesh.resource_name = "mk_guide_sign_" + lod
	return mesh

## Arrows, runs and shields of board layout `layout` (surface sign only),
## PROUD in front of the guide board.
static func board(layout: int, materials: Dictionary) -> ArrayMesh:
	var signs := F.Builder.new()
	var centre := BOARD.get_center()
	board_layout(signs, face(Vector3(centre.x, centre.y, BOARD_FRONT), Vector3.FORWARD), layout, BOARD.size * 0.5)
	var mesh := ArrayMesh.new()
	signs.commit(mesh, materials.get("sign"))
	mesh.resource_name = "mk_guide_board_%d" % layout
	return mesh

## Arrows, glyph runs and the route shield of a board layout (face
## coordinates about the board centre; u is the driver's right).
static func board_layout(signs: F.Builder, f: Dictionary, layout: int, half: Vector2) -> void:
	var white := ink(WHITE)
	var bits := int(GLYPHS.guide) + layout * 977
	var stem := 0.04
	var head := 0.07
	var spread := 0.11
	var text := 0.075
	match layout:
		0:
			# Junction diagram: a stem up to straight on, branches left and right.
			arrow(signs, f, [Vector2(0.0, -0.24), Vector2(0.0, 0.22)], stem, head, spread, 1.0, white)
			arrow(signs, f, [Vector2(0.0, -0.02), Vector2(-0.3, -0.02)], stem, head, spread, 1.0, white)
			arrow(signs, f, [Vector2(0.0, -0.02), Vector2(0.3, -0.02)], stem, head, spread, 1.0, white)
			run(signs, f, bits, 3, Vector2(-0.56, -0.02), text, 1.0, white)
			run(signs, f, bits + 1, 4, Vector2(0.58, -0.02), text, 1.0, white)
			run(signs, f, bits + 2, 3, Vector2(0.24, 0.19), text * 0.85, 1.0, white)
			shield(signs, f, Vector2(-0.56, 0.18), 1 + layout)
		1:
			# Straight on (left half) and a branch to the right.
			arrow(signs, f, [Vector2(-0.62, -0.24), Vector2(-0.62, 0.22)], stem, head, spread, 1.0, white)
			run(signs, f, bits, 4, Vector2(-0.33, 0.06), text, 1.0, white)
			shield(signs, f, Vector2(-0.4, -0.15), 4)
			arrow(signs, f, [Vector2(0.18, -0.24), Vector2(0.18, -0.02), Vector2(0.48, -0.02)], stem, head, spread, 1.0, white)
			run(signs, f, bits + 1, 3, Vector2(0.53, 0.16), text, 1.0, white)
		2:
			# A branch to the left and straight on (right half).
			arrow(signs, f, [Vector2(-0.18, -0.24), Vector2(-0.18, -0.02), Vector2(-0.48, -0.02)], stem, head, spread, 1.0, white)
			run(signs, f, bits, 3, Vector2(-0.53, 0.16), text, 1.0, white)
			arrow(signs, f, [Vector2(0.62, -0.24), Vector2(0.62, 0.22)], stem, head, spread, 1.0, white)
			run(signs, f, bits + 1, 4, Vector2(0.33, 0.06), text, 1.0, white)
			shield(signs, f, Vector2(0.4, -0.15), 5)
		_:
			# Overhead lane board: an arrow over each lane, a run beside it and
			# a white divider between the lanes.
			fill(signs, f, chamfered(Vector2.ZERO, Vector2(0.006, half.y - 0.06), 0.0), 1.0, white)
			for s: float in [-1.0, 1.0]:
				arrow(signs, f, [Vector2(0.62 * s, -0.22), Vector2(0.62 * s, 0.2)], stem, head, spread, 1.0, white)
				run(signs, f, bits + int(s > 0.0), 3 if s < 0.0 else 4, Vector2(0.3 * s, 0.07), text, 1.0, white)
			shield(signs, f, Vector2(-0.3, -0.14), 7)
			shield(signs, f, Vector2(0.3, -0.14), 9)

## White route shield (rounded rectangle) with a two-digit blue number.
static func shield(signs: F.Builder, f: Dictionary, centre: Vector2, seed_value: int) -> void:
	var outer := rounded(chamfered(centre, Vector2(0.06, 0.042), 0.0), 0.014, 2)
	fill(signs, f, outer, 1.0, ink(WHITE))
	var value := 10 + pcg(seed_value * 7919 + 13) % 90
	number(signs, f, value, centre, Vector2(0.03, 0.054), 0.009, 0.038, 2.0, ink(BLUE))

# Materials, build and instancing ----------------------------------------------

## Shared material of `kind`: paint (street_furniture_meshes painted metal) or
## sign (printed faces: the environment surface with vertex tints, lit softly
## at night through lighting_role 2). Without an environment context
## (standalone previews) plain materials.
static func material(kind: String, context: RefCounted = null) -> Material:
	var made: Material
	if kind == "paint":
		made = F.material("paint", context)
	elif context != null and context.has_method("generated_material"):
		var lit: ShaderMaterial = context.generated_material(Color.WHITE, str(SURFACES.sign.tile), float(SURFACES.sign.metallic), float(SURFACES.sign.roughness))
		lit.set_shader_parameter("vertex_tinted", true)
		lit.set_shader_parameter("lighting_role", 2)
		made = lit
	else:
		var plain := StandardMaterial3D.new()
		plain.vertex_color_use_as_albedo = true
		plain.roughness = float(SURFACES.sign.roughness)
		plain.cull_mode = BaseMaterial3D.CULL_DISABLED
		made = plain
	made.resource_name = "mk_road_sign_" + kind
	return made

## One part per call (a material or a mesh); true when the whole set is in
## state.result. `lease` (track(Object)) receives every created resource.
## The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {"materials": {}}
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "road_signs_" + step
	var result: Dictionary = state.result
	if step.begins_with("material_"):
		var made := material(step.trim_prefix("material_"), context)
		result.materials[step.trim_prefix("material_")] = made
		if lease != null: lease.track(made)
	else:
		var mesh: ArrayMesh
		if step.begins_with("guide_board_"): mesh = board(int(step.trim_prefix("guide_board_")), result.materials)
		elif step.begins_with("guide_"): mesh = guide(step.trim_prefix("guide_"), result.materials)
		elif step.ends_with("_digits"): mesh = digits(int(step.trim_prefix("speed_").trim_suffix("_digits")), result.materials)
		else:
			var lod := step.get_slice("_", step.get_slice_count("_") - 1)
			mesh = pole_sign(step.trim_suffix("_" + lod), lod, result.materials)
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
## upload) when it holds road signs and has a resource cache; false when the
## cache refuses it (the cell fails before uploading anything: its signs
## would lose their only body).
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
## unavailable, which fails the cell (chunk_renderer). A refused claim
## already failed the cell in begin().
static func ready(job: Dictionary) -> int:
	if job.has("road_signs"): return 1
	job.last_phase = "road_sign_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.road_signs = made
		return 0
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.road_signs = made
	return 0

## Mesh parts drawn per LOD for a kind and guide layout: speed limits share
## one body (near and far) under their own digits; guide signs one frame
## under their board layout.
static func parts(kind: String, layout := 0) -> Dictionary:
	if kind.begins_with("speed_"): return {"near": ["speed_near", kind + "_digits"], "far": ["speed_far"]}
	if kind == "guide": return {"near": ["guide_near", "guide_board_%d" % layout], "far": ["guide_far"]}
	return {"near": [kind + "_near"], "far": [kind + "_far"]}

## One admitted step per object: its transform (street_furniture_meshes
## placement) and id join the cell's group of its kind, variant and 32 m
## square, attached one group per step after the objects.
static func plan(job: Dictionary, object: Dictionary) -> void:
	var kind: String = KINDS[str(object.asset_id)]
	var layout := variant(object, int(VARIANTS[kind])) if VARIANTS.has(kind) else 0
	var transform := F.placement(object)
	var o := transform.origin
	var key := "%s/%d/%d,%d" % [kind, layout, floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
	if not job.has("road_sign_plan"): job.road_sign_plan = {}
	if not job.road_sign_plan.has(key): job.road_sign_plan[key] = {"kind": kind, "meshes": parts(kind, layout), "parts": [], "ids": PackedStringArray()}
	var b := transform.basis
	job.road_sign_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z]))
	job.road_sign_plan[key].ids.append(str(object.id))

## Near and far MultiMeshes of one planned group (one per part). The cell lease carries the
## nodes and instance buffers; meshes and materials belong to the shared set.
## mapkit_object_ids lists the instances' object ids (editor picking).
static func attach_group(job: Dictionary) -> void:
	var key: String = job.road_sign_plan.keys()[0]
	var group: Dictionary = job.road_sign_plan[key]
	job.road_sign_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in LODS:
		for mesh_key: String in group.meshes[lod]:
			var multi := MultiMesh.new()
			multi.transform_format = MultiMesh.TRANSFORM_3D
			multi.mesh = job.road_signs[mesh_key]
			multi.instance_count = buffer.size() / 12
			multi.buffer = buffer
			var node := MultiMeshInstance3D.new()
			node.name = "RoadSigns_%s_%d" % [mesh_key, job.root.get_child_count()]
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
