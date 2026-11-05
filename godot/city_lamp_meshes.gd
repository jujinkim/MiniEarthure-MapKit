extends RefCounted
## Display-only city streetlights (mapkit-core CITY_STREETLIGHTS:
## builtin:city_streetlight_75/135/195/255/315/375 and _twin_75/135), built
## procedurally (no external assets): a Korean arterial lamp on a 330 cm map
## pole (about 10 m real at 1:3) whose arm carries an LED cobra head over the
## lanes. Collision is the solid 12 cm pole box only (placement.rs); every
## part here is display-only and stays inside the contract's envelope
## (CITY_STREETLIGHT_ENVELOPE_CM: local x from -0.25 m, -(reach + 0.2) on
## twins, to reach + 0.2 m; |z| <= 0.08 m; y from -0.08 m, a DEM skirt, to
## 3.45 m), with nothing beyond x = 0.25 m below 2.5 m.
##
## Metres, origin at the foot, local +x over the road (the arm), z along the
## road. Near mesh: a concrete footing skirt below the foot, a base plate with
## four anchor nuts, a square base cover over the whole pole box up to 0.6 m
## (above car height) with an access door (lock, screws) facing the road
## side, a tapered round
## or octagonal shaft with a section collar, a ribbed anti-poster sheet and
## the management-number plate (white, yellow band, digits); a bent mast
## (curving out of the shaft top) or a straight arm with a pole cap, clamp
## collars and a diagonal brace; the LED head: die-cast housing lofted from
## the spigot sleeve to a rounded nose, heat-sink fins and a photocell on
## top, and the flat lens underneath (CITY_STREETLIGHT_HEAD_CM 310 cm) with
## twelve LED modules. Optional parts by design: banner brackets with a bowed
## double-sided street banner, a pedestrian arm with its own small head
## toward the pole's back (single lamps), a pole-mounted control box with its
## conduit. Twins carry a second arm and head at -x. The far mesh (about 30
## triangles, 50 with the twin side) is a hexagonal pole, an arm bar and a
## head box with its lens; the halo mesh one camera-facing quad per head.
##
## One near, far and halo mesh serve all eight ids: the arm reach is a
## per-instance value (INSTANCE_CUSTOM.x, cm, from the id), and the vertex
## shader (city_lamp.gdshader) moves head vertices by reach - REF_REACH and
## stretches the straight arm span between its fixed root and the head
## (UV2.x: the share of the stretch, negative on the twin side); parts of
## other designs collapse to a point (UV.y: the part, see keep()). Design
## choices are integer hashes: the street design (arm style, shaft, paint,
## banners and their palette, pedestrian arms) from the lamp's 256 m world
## square and road axis, so a street's lamps inside one square match on both
## sides (rows change design at square edges, see design()); control boxes
## and missing banners per lamp from its object identity.
##
## A cell instances every lamp of a 16 m square into one group: near, far
## and halo MultiMeshes (display_quality.apply_city_lamp: the near mesh
## within streetlight_near + FURNITURE_REACH, none on low; the far mesh
## beyond, to decoration_distance + FURNITURE_REACH; halos from medium, as
## far as the far mesh, and hidden by day: display_quality.set_night_lights
## toggles the HALO_GROUP nodes as the sun crosses the horizon, so they
## issue no daytime draws). Lens and halos switch on after sunset;
## the ground pools come from the presentation's lamp_map (mapkit-godot
## lamp_map.rs) bound on the cell's road and sidewalk materials
## (bind_light_map, urban_surface.gdshader).

const QUALITY := preload("./display_quality.gd")
const KEY := "builtin:city_lamps"
## Mirror of mapkit-core CITY_STREETLIGHTS: id -> [arm reach cm, twin]
## (mapkit-godot lamp_map.rs tests compare the literals).
const IDS := {"builtin:city_streetlight_75": [75, false], "builtin:city_streetlight_135": [135, false],
	"builtin:city_streetlight_195": [195, false], "builtin:city_streetlight_255": [255, false],
	"builtin:city_streetlight_315": [315, false], "builtin:city_streetlight_375": [375, false],
	"builtin:city_streetlight_twin_75": [75, true], "builtin:city_streetlight_twin_135": [135, true]}
const LODS := ["near", "far", "halo"]
const GROUP_METRES := 16.0
const FLOATS := 16
## Contract (placement.rs): pole box, lens height, envelope [back, tip, side,
## top] in cm.
const POLE_CM := 12
const HEIGHT_CM := 330
const HEAD_CM := 310
const ENVELOPE_CM := [25, 20, 8, 345]
## Lowest display vertex: the footing skirt that hides cut and fill at the
## DEM foot.
const SKIRT := 0.08
## Square base cover (m): half side (over the 6 cm half pole box) and top
## (above the car's 57 cm map height).
const BASE_HALF := 0.0608
const BASE_TOP := 0.6
## Reach (m) the meshes are built at; the shader moves heads by reach - REF.
const REF_REACH := 1.35
const MAX_REACH := 3.75
## Arm root and spigot (m): the bent mast leaves its bend at x = BEND, the
## straight arm the pole at x = 0.02; both end SPIGOT before the head centre.
const BEND := 0.24
const SPIGOT := 0.155
const HEAD_Y := 3.10
## Part codes (UV.y) and design flags (INSTANCE_CUSTOM.y bits).
const PART_ALL := 0
const PART_BENT := 1
const PART_STRAIGHT := 2
const PART_BANNER := 3
const PART_PEDESTRIAN := 4
const PART_BOX := 5
const PART_TWIN_BENT := 6
const PART_TWIN_STRAIGHT := 7
const PART_ROUND := 8
const PART_OCTAGONAL := 9
const PART_TWIN := 10
const FLAG_TWIN := 1
const FLAG_STRAIGHT := 2
const FLAG_BANNER := 4
const FLAG_PEDESTRIAN := 8
const FLAG_BOX := 16
const FLAG_OCTAGONAL := 32
## Surface roles (UV.x, city_lamp.gdshader).
const ROLE_PAINT := 0
const ROLE_FIXED := 1
const ROLE_LENS := 2
const ROLE_HOUSING := 3
const ROLE_SHEET := 4
const ROLE_BANNER := 5
## Street design shares: straight arms, octagonal shafts, banner streets (and
## lamps of a banner street that carry one), pedestrian-arm streets, control
## boxes; paint shares galvanised, dark grey, bronze, warm white.
const STRAIGHT_SHARE := 0.22
const OCTAGONAL_SHARE := 0.35
const BANNER_STREET_SHARE := 0.3
const BANNER_LAMP_SHARE := 0.85
const PEDESTRIAN_SHARE := 0.3
const BOX_SHARE := 0.4
const PAINT_SHARES := [0.45, 0.75, 0.9, 1.0]
const DESIGN_SQUARE_CM := 25600
## LED light (about 4000 K, contract): sRGB FFEDD6.
const LED := Color("ffedd6")
## Memory allowance of one shared set (render_resource_cache): about 2,050
## vertices at <= 64 B and 5,920 indices at 4 B in CPU arrays and GPU buffers
## (0.31 MB, city_lamp_validator measures them) plus 128 KiB for the two
## materials and shaders and one mesh's build arrays.
const SHARED_BYTES := 512 * 1024
const STEPS := ["material", "halo_material", "near", "far", "halo"]
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

## Vertices with their role and part (UV), stretch share (UV2.x) and a free
## channel (UV2.y), a colour (sRGB tint, shade or mask, by role).
class Builder:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var colors := PackedColorArray()
	var uvs := PackedVector2Array()
	var uv2s := PackedVector2Array()
	var indices := PackedInt32Array()
	var role := 0
	var part := 0
	var color := Color.WHITE
	## Stretch: 0 none, 1 the head (share = side), 2 a span from `span.x` to
	## `span.y` along |x| (share = side x the fraction along it).
	var stretch := 0
	var side := 1.0
	var span := Vector2.ZERO
	func share(point: Vector3) -> float:
		if stretch == 1: return side
		if stretch == 2: return side * clampf((absf(point.x) - span.x) / maxf(span.y - span.x, 1e-6), 0.0, 1.0)
		return 0.0
	func add(point: Vector3, normal: Vector3, tint := Color(-1, 0, 0), extra := 0.0) -> int:
		vertices.append(point); normals.append(normal.normalized())
		colors.append(color if tint.r < 0.0 else tint)
		uvs.append(Vector2(role, part)); uv2s.append(Vector2(share(point), extra))
		return vertices.size() - 1
	## Front faces wind clockwise seen from their normal side (Godot).
	func triangle(a: int, b: int, c: int) -> void:
		var facing := (vertices[b] - vertices[a]).cross(vertices[c] - vertices[a])
		if facing.dot(normals[a] + normals[b] + normals[c]) > 0.0:
			indices.append(a); indices.append(c); indices.append(b)
		else:
			indices.append(a); indices.append(b); indices.append(c)
	func quad(a: int, b: int, c: int, d: int) -> void:
		triangle(a, b, c); triangle(a, c, d)
	func commit(mesh: ArrayMesh, material: Material) -> void:
		if indices.is_empty(): return
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_NORMAL] = normals
		arrays[Mesh.ARRAY_COLOR] = colors
		arrays[Mesh.ARRAY_TEX_UV] = uvs
		arrays[Mesh.ARRAY_TEX_UV2] = uv2s
		arrays[Mesh.ARRAY_INDEX] = indices
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mesh.surface_set_material(mesh.get_surface_count() - 1, material)

## Convex planar polygon (any vertex order) facing `normal`.
static func polygon(b: Builder, points: Array, normal: Vector3) -> void:
	var centre := Vector3.ZERO
	for point: Vector3 in points: centre += point
	centre /= float(points.size())
	var u: Vector3 = ((points[0] as Vector3) - centre).normalized()
	var v := normal.cross(u)
	var ordered := points.duplicate()
	ordered.sort_custom(func(p: Vector3, q: Vector3) -> bool: return atan2((p - centre).dot(v), (p - centre).dot(u)) < atan2((q - centre).dot(v), (q - centre).dot(u)))
	var first := b.vertices.size()
	for point: Vector3 in ordered: b.add(point, normal)
	for index in range(1, ordered.size() - 1): b.triangle(first, first + index, first + index + 1)

## Box of half extents `half` at `centre`, turned by `basis`; `skip` lists
## face normals (local) to leave out.
static func box(b: Builder, centre: Vector3, half: Vector3, basis := Basis(), skip: Array = []) -> void:
	for axis in 3:
		for s in [-1.0, 1.0]:
			var n := Vector3.ZERO
			n[axis] = s
			if skip.has(n): continue
			var points := []
			for p in [-1.0, 1.0]:
				for q in [-1.0, 1.0]:
					var local := Vector3.ZERO
					local[axis] = s * half[axis]
					local[(axis + 1) % 3] = p * half[(axis + 1) % 3]
					local[(axis + 2) % 3] = q * half[(axis + 2) % 3]
					points.append(centre + basis * local)
			polygon(b, points, basis * n)

## Smooth surface through `rings` (equal point counts), outward normals from
## each ring's centre; optional flat caps facing away from the next ring.
static func loft(b: Builder, rings: Array, cap_start := false, cap_end := false) -> void:
	var count: int = (rings[0] as Array).size()
	var starts := []
	for index in rings.size():
		var ring: Array = rings[index]
		var centre := _centre(ring)
		var axis := (_centre(rings[mini(index + 1, rings.size() - 1)]) - _centre(rings[maxi(index - 1, 0)])).normalized()
		starts.append(b.vertices.size())
		for point: Vector3 in ring:
			var out := point - centre
			b.add(point, (out - axis * out.dot(axis)).normalized())
	for index in rings.size() - 1:
		for k in count:
			b.quad(starts[index] + k, starts[index] + (k + 1) % count, starts[index + 1] + (k + 1) % count, starts[index + 1] + k)
	if cap_start: polygon(b, rings[0], (_centre(rings[0]) - _centre(rings[1])).normalized())
	if cap_end: polygon(b, rings[-1], (_centre(rings[-1]) - _centre(rings[-2])).normalized())

static func _centre(ring: Array) -> Vector3:
	var centre := Vector3.ZERO
	for point: Vector3 in ring: centre += point
	return centre / float(ring.size())

## Ring of `sides` points round `centre` across `forward`; `radii` may vary
## per point (ribs). `phase` turns the first point.
static func ring(centre: Vector3, forward: Vector3, radius: float, sides: int, phase := 0.5, ribs := 0.0) -> Array:
	var normal := (Vector3.RIGHT if absf(forward.dot(Vector3.RIGHT)) < 0.9 else Vector3.BACK).cross(forward).normalized()
	var across := forward.cross(normal)
	var points := []
	for k in sides:
		var angle := TAU * (float(k) + phase) / float(sides)
		points.append(centre + (normal * cos(angle) + across * sin(angle)) * (radius + (ribs if k % 2 == 0 else 0.0)))
	return points

## Round tube along `centres`, one radius per centre.
static func tube(b: Builder, centres: Array, radii: Array, sides: int, cap_start := false, cap_end := false) -> void:
	var rings := []
	for index in centres.size():
		var forward: Vector3 = ((centres[mini(index + 1, centres.size() - 1)] as Vector3) - (centres[maxi(index - 1, 0)] as Vector3)).normalized()
		rings.append(ring(centres[index], forward, float(radii[index]), sides))
	loft(b, rings, cap_start, cap_end)

## Vertical prism with flat faces (octagonal shaft) from (y0, r0) to (y1, r1),
## radii to the face centres.
static func prism(b: Builder, sides: int, y0: float, y1: float, r0: float, r1: float) -> void:
	for k in sides:
		var a := TAU * float(k) / float(sides)
		var c := TAU * float(k + 1) / float(sides)
		var mid := (a + c) * 0.5
		var scale := 1.0 / cos(PI / float(sides))
		var n := Vector3(cos(mid), (r0 - r1) / (y1 - y0), sin(mid)).normalized()
		var first := b.vertices.size()
		for point in [Vector3(cos(a) * r0 * scale, y0, sin(a) * r0 * scale), Vector3(cos(c) * r0 * scale, y0, sin(c) * r0 * scale),
				Vector3(cos(c) * r1 * scale, y1, sin(c) * r1 * scale), Vector3(cos(a) * r1 * scale, y1, sin(a) * r1 * scale)]:
			b.add(point, n)
		b.quad(first, first + 1, first + 2, first + 3)

## Shaft radius (m, to the face centres) at height y: a taper from 0.046 at
## the base-cover chamfer to 0.027 at the top.
static func shaft_radius(y: float) -> float:
	return lerpf(0.046, 0.027, clampf((y - 0.34) / (2.92 - 0.34), 0.0, 1.0))

## Paint shades (vertex colour of ROLE_PAINT, a multiplier of the design's
## paint) and fixed colours (sRGB).
const FOOTING := Color("a3a097")
const DARK := Color("2b2d2f")
const PLATE := Color("efeee8")
const PLATE_BAND := Color("efc21a")
const BOX_GREY := Color("a9aca9")
const PHOTOCELL := Color("3a3d40")

## Rounded-rectangle section across x at `x`: half width `half` (z), from
## `bottom` to `top`, corners cut by `cut`.
static func section(x: float, half: float, bottom: float, top: float, cut: float) -> Array:
	return [Vector3(x, bottom, -half + cut), Vector3(x, bottom, half - cut), Vector3(x, bottom + cut, half), Vector3(x, top - cut, half),
		Vector3(x, top, half - cut), Vector3(x, top, -half + cut), Vector3(x, top - cut, -half), Vector3(x, bottom + cut, -half)]

## The LED head at side `s` (+1 the arm's side, -1 a twin's), centred at
## x = s * REF_REACH: housing, fins, photocell, lens and modules.
static func head(b: Builder, s: float, part: int) -> void:
	b.part = part
	b.stretch = 1
	b.side = s
	var c := s * REF_REACH
	var at := func(dx: float) -> float: return c + s * dx
	b.role = ROLE_HOUSING
	b.color = Color(1, 1, 1)
	var rings := [section(at.call(-0.168), 0.024, 3.132, 3.178, 0.01), section(at.call(-0.135), 0.05, 3.104, 3.181, 0.016),
		section(at.call(-0.085), 0.064, 3.1, 3.178, 0.018), section(at.call(0.08), 0.064, 3.1, 3.172, 0.018),
		section(at.call(0.145), 0.052, 3.103, 3.163, 0.016), section(at.call(0.172), 0.034, 3.11, 3.15, 0.012)]
	loft(b, rings, true, true)
	# Heat-sink fins across the top and a photocell knob behind them.
	b.color = Color(0.86, 0.86, 0.86)
	for k in 5:
		box(b, Vector3(at.call(-0.06 + 0.03 * k), 3.183, 0.0), Vector3(0.003, 0.009, 0.044), Basis(), [Vector3.DOWN])
	b.role = ROLE_FIXED
	b.color = PHOTOCELL
	tube(b, [Vector3(at.call(-0.115), 3.176, 0.0), Vector3(at.call(-0.115), 3.2, 0.0)], [0.011, 0.009], 8, false, true)
	# The flat lens a hair under the housing, and twelve LED modules on it
	# (two rows of six) with dark gaps between: lens shade 1 lit, 0.18 gaps.
	b.role = ROLE_LENS
	b.color = Color(0.18, 0, 0)
	polygon(b, [Vector3(at.call(-0.115), 3.0985, -0.05), Vector3(at.call(-0.115), 3.0985, 0.05), Vector3(at.call(0.135), 3.0985, 0.05), Vector3(at.call(0.135), 3.0985, -0.05)], Vector3.DOWN)
	b.color = Color(1, 0, 0)
	for row in 2:
		for k in 6:
			var x0: float = at.call(-0.105 + 0.04 * k)
			var x1: float = at.call(-0.105 + 0.04 * k + 0.033)
			var z0 := -0.044 + 0.046 * row
			polygon(b, [Vector3(x0, 3.0975, z0), Vector3(x0, 3.0975, z0 + 0.042), Vector3(x1, 3.0975, z0 + 0.042), Vector3(x1, 3.0975, z0)], Vector3.DOWN)
	b.stretch = 0

## Bent mast at side `s`: out of the shaft top through a quarter bend of
## radius BEND, then level to the spigot inside the head sleeve.
static func bent_arm(b: Builder, s: float, part: int) -> void:
	b.part = part
	b.role = ROLE_PAINT
	b.color = Color(1, 1, 1)
	var centres := []
	var radii := []
	for k in 9:
		var angle := PI - float(k) / 8.0 * PI * 0.5
		centres.append(Vector3(s * (BEND + cos(angle) * BEND), 2.92 + sin(angle) * BEND, 0.0))
		radii.append(lerpf(0.027, 0.022, float(k) / 8.0))
	b.stretch = 0
	tube(b, centres, radii, 10)
	b.stretch = 2
	b.side = s
	b.span = Vector2(BEND, REF_REACH - SPIGOT)
	tube(b, [Vector3(s * BEND, 3.16, 0.0), Vector3(s * lerpf(BEND, REF_REACH - SPIGOT, 0.5), 3.159, 0.0), Vector3(s * (REF_REACH - SPIGOT + 0.02), 3.157, 0.0)], [0.022, 0.02, 0.018], 10, false, true)
	b.stretch = 0

## Straight arm at side `s`: a clamp collar at 3.12 m, the arm rising 4 cm to
## the spigot and a diagonal brace from a lower clamp to 45 % of the span.
static func straight_arm(b: Builder, s: float, part: int) -> void:
	b.part = part
	b.role = ROLE_PAINT
	b.color = Color(0.92, 0.92, 0.92)
	b.stretch = 0
	tube(b, [Vector3(0, 3.095, 0), Vector3(0, 3.145, 0)], [0.033, 0.033], 12, true, true)
	tube(b, [Vector3(0, 2.775, 0), Vector3(0, 2.815, 0)], [0.031, 0.031], 12, true, true)
	b.color = Color(1, 1, 1)
	b.stretch = 2
	b.side = s
	b.span = Vector2(0.02, REF_REACH - SPIGOT)
	var tip := REF_REACH - SPIGOT + 0.02
	tube(b, [Vector3(s * 0.02, 3.12, 0.0), Vector3(s * lerpf(0.02, tip, 0.5), 3.14, 0.0), Vector3(s * tip, 3.157, 0.0)], [0.021, 0.0195, 0.018], 10, false, true)
	var reach_x := lerpf(0.02, REF_REACH - SPIGOT, 0.45)
	tube(b, [Vector3(s * 0.02, 2.795, 0.0), Vector3(s * lerpf(0.02, reach_x, 0.5), lerpf(2.795, 3.125, 0.5), 0.0), Vector3(s * reach_x, 3.125, 0.0)], [0.012, 0.011, 0.011], 8)
	b.stretch = 0

## Near mesh (every part of every design; the shader keeps the instance's).
static func near_mesh(material: Material) -> ArrayMesh:
	var b := Builder.new()
	b.part = PART_ALL
	# Footing skirt under the foot (hides DEM cut and fill), base plate, nuts.
	b.role = ROLE_FIXED
	b.color = FOOTING
	var skirt := []
	for y in [-SKIRT, 0.002]: skirt.append(ring(Vector3(0, y, 0), Vector3.UP, 0.059, 8, 0.0))
	loft(b, skirt)
	b.role = ROLE_PAINT
	b.color = Color(0.8, 0.8, 0.8)
	box(b, Vector3(0, 0.006, 0), Vector3(0.057, 0.006, 0.057), Basis(), [Vector3.DOWN])
	b.color = Color(0.62, 0.62, 0.62)
	for sx in [-1.0, 1.0]:
		for sz in [-1.0, 1.0]:
			box(b, Vector3(sx * 0.045, 0.016, sz * 0.045), Vector3(0.0065, 0.0045, 0.0065), Basis(Vector3.UP, PI / 4.0), [Vector3.DOWN])
	# Square base cover over the whole collision box (BASE_HALF >= half the
	# 12 cm pole box) up to BASE_TOP, above the car's 57 cm, so a car stops
	# against a visible face from every side; a stepped cap round the shaft;
	# the access door facing the road side with lock and screws.
	b.color = Color(0.95, 0.95, 0.95)
	box(b, Vector3(0, (0.012 + BASE_TOP) * 0.5, 0), Vector3(BASE_HALF, (BASE_TOP - 0.012) * 0.5, BASE_HALF), Basis(), [Vector3.DOWN])
	box(b, Vector3(0, BASE_TOP + 0.007, 0), Vector3(0.052, 0.007, 0.052), Basis(), [Vector3.DOWN])
	b.color = Color(0.88, 0.88, 0.88)
	box(b, Vector3(BASE_HALF + 0.0005, 0.19, 0), Vector3(0.0005, 0.075, 0.023), Basis(), [Vector3(-1, 0, 0)])
	b.role = ROLE_FIXED
	b.color = DARK
	box(b, Vector3(BASE_HALF + 0.0012, 0.2, 0.013), Vector3(0.0002, 0.006, 0.004), Basis(), [Vector3(-1, 0, 0)])
	for y in [0.125, 0.255]: box(b, Vector3(BASE_HALF + 0.0012, y, -0.014), Vector3(0.0002, 0.0028, 0.0028), Basis(), [Vector3(-1, 0, 0)])
	# Shafts: round (12 sides) or octagonal, a section collar at 1.6 m.
	b.role = ROLE_PAINT
	b.color = Color(1, 1, 1)
	b.part = PART_ROUND
	tube(b, [Vector3(0, BASE_TOP, 0), Vector3(0, 1.6, 0), Vector3(0, 2.92, 0)], [shaft_radius(BASE_TOP), shaft_radius(1.6), shaft_radius(2.92)], 12)
	b.part = PART_OCTAGONAL
	prism(b, 8, BASE_TOP, 2.92, shaft_radius(BASE_TOP), shaft_radius(2.92))
	b.part = PART_ALL
	b.color = Color(0.9, 0.9, 0.9)
	tube(b, [Vector3(0, 1.585, 0), Vector3(0, 1.615, 0)], [shaft_radius(1.6) * 1.1 + 0.004, shaft_radius(1.6) * 1.1 + 0.004], 12, true, true)
	# Ribbed anti-poster sheet (24 facets alternating 2 mm) round either shaft.
	b.role = ROLE_SHEET
	b.color = Color(1, 1, 1)
	var sheet := []
	for y in [0.64, 0.78, 0.92]: sheet.append(ring(Vector3(0, y, 0), Vector3.UP, shaft_radius(y) * 1.09 + 0.002, 24, 0.5, 0.002))
	loft(b, sheet, true, true)
	# Management-number plate facing the road side: white, a yellow band and
	# three digit blocks.
	b.role = ROLE_FIXED
	var face := shaft_radius(1.0) * 1.09 + 0.001
	b.color = PLATE
	box(b, Vector3(face + 0.002, 1.0, 0), Vector3(0.002, 0.036, 0.017), Basis(), [Vector3(-1, 0, 0)])
	b.color = PLATE_BAND
	polygon(b, [Vector3(face + 0.0042, 1.021, -0.016), Vector3(face + 0.0042, 1.021, 0.016), Vector3(face + 0.0042, 1.031, 0.016), Vector3(face + 0.0042, 1.031, -0.016)], Vector3.RIGHT)
	b.color = DARK
	for k in 3:
		var z := -0.0095 + 0.0095 * k
		polygon(b, [Vector3(face + 0.0042, 0.982, z - 0.0035), Vector3(face + 0.0042, 0.982, z + 0.0035), Vector3(face + 0.0042, 1.006, z + 0.0035), Vector3(face + 0.0042, 1.006, z - 0.0035)], Vector3.RIGHT)
	# Arms: the bent mast or the straight arm with its pole cap; twins mirror.
	bent_arm(b, 1.0, PART_BENT)
	bent_arm(b, -1.0, PART_TWIN_BENT)
	b.part = PART_STRAIGHT
	b.role = ROLE_PAINT
	b.color = Color(1, 1, 1)
	tube(b, [Vector3(0, 2.9, 0), Vector3(0, 3.21, 0)], [0.027, 0.0255], 12, false, false)
	loft(b, [ring(Vector3(0, 3.21, 0), Vector3.UP, 0.0285, 12), ring(Vector3(0, 3.226, 0), Vector3.UP, 0.0285, 12), ring(Vector3(0, 3.236, 0), Vector3.UP, 0.012, 12)], false, true)
	straight_arm(b, 1.0, PART_STRAIGHT)
	straight_arm(b, -1.0, PART_TWIN_STRAIGHT)
	head(b, 1.0, PART_ALL)
	head(b, -1.0, PART_TWIN)
	# Pedestrian arm toward the pole's back with a small head and lens.
	b.part = PART_PEDESTRIAN
	b.role = ROLE_PAINT
	b.color = Color(0.92, 0.92, 0.92)
	tube(b, [Vector3(0, 1.448, 0), Vector3(0, 1.492, 0)], [shaft_radius(1.47) * 1.09 + 0.004, shaft_radius(1.47) * 1.09 + 0.004], 12, true, true)
	b.color = Color(1, 1, 1)
	tube(b, [Vector3(-0.03, 1.47, 0), Vector3(-0.1, 1.478, 0), Vector3(-0.168, 1.488, 0)], [0.009, 0.0085, 0.008], 8)
	b.role = ROLE_HOUSING
	loft(b, [section(-0.165, 0.022, 1.47, 1.502, 0.008), section(-0.18, 0.028, 1.467, 1.505, 0.01), section(-0.232, 0.028, 1.467, 1.503, 0.01), section(-0.245, 0.02, 1.47, 1.498, 0.008)], true, true)
	b.role = ROLE_LENS
	b.color = Color(1, 0, 0)
	polygon(b, [Vector3(-0.236, 1.4665, -0.021), Vector3(-0.236, 1.4665, 0.021), Vector3(-0.178, 1.4665, 0.021), Vector3(-0.178, 1.4665, -0.021)], Vector3.DOWN)
	# Control box on the pole's back above car height, its conduit down.
	b.part = PART_BOX
	b.role = ROLE_FIXED
	b.color = BOX_GREY
	box(b, Vector3(-0.054, 0.67, 0), Vector3(0.012, 0.068, 0.027), Basis(), [Vector3(1, 0, 0)])
	b.color = DARK
	polygon(b, [Vector3(-0.0662, 0.61, -0.0005), Vector3(-0.0662, 0.61, 0.0005), Vector3(-0.0662, 0.73, 0.0005), Vector3(-0.0662, 0.73, -0.0005)], Vector3.LEFT)
	b.color = BOX_GREY
	tube(b, [Vector3(-0.05, 0.602, 0), Vector3(-0.05, 0.4, 0), Vector3(-0.05, 0.34, 0)], [0.006, 0.006, 0.006], 6)
	# Banner brackets and a bowed double-sided banner facing along the road.
	b.part = PART_BANNER
	b.role = ROLE_PAINT
	b.color = Color(0.9, 0.9, 0.9)
	for y in [1.78, 2.43]:
		tube(b, [Vector3(shaft_radius(y) * 0.9, y, 0), Vector3(0.236, y, 0)], [0.0045, 0.0045], 6, false, true)
		box(b, Vector3(shaft_radius(y) * 1.09 + 0.004, y, 0), Vector3(0.006, 0.012, 0.012))
	b.role = ROLE_BANNER
	for s in [-1.0, 1.0]:
		var first := b.vertices.size()
		for i in 5:
			var u := float(i) / 4.0
			for j in 6:
				var v := float(j) / 5.0
				var bow := 0.006 * sin(PI * u)
				var point := Vector3(lerpf(0.045, 0.226, u), lerpf(2.41, 1.8, v), s * 0.0018 + bow)
				b.add(point, Vector3(0, 0, s), Color(u if s > 0.0 else 1.0 - u, v, 0))
		for i in 4:
			for j in 5:
				var a := first + i * 6 + j
				b.quad(a, a + 6, a + 7, a + 1)
	var mesh := ArrayMesh.new()
	b.commit(mesh, material)
	mesh.resource_name = "mk_city_lamp_near"
	mesh.custom_aabb = envelope()
	return mesh

## Far mesh: hexagonal pole, arm bar, head box and lens; the twin side.
static func far_mesh(material: Material) -> ArrayMesh:
	var b := Builder.new()
	b.role = ROLE_PAINT
	b.color = Color(1, 1, 1)
	var pole := []
	for y in [0.0, 3.17]: pole.append(ring(Vector3(0, y, 0), Vector3.UP, 0.052 if y == 0.0 else 0.03, 6, 0.0))
	loft(b, pole)
	for s in [1.0, -1.0]:
		b.part = PART_ALL if s > 0.0 else PART_TWIN
		b.role = ROLE_PAINT
		b.color = Color(1, 1, 1)
		b.stretch = 2
		b.side = s
		b.span = Vector2(0.0, REF_REACH - SPIGOT)
		box(b, Vector3(s * (REF_REACH - SPIGOT) * 0.5, 3.155, 0), Vector3((REF_REACH - SPIGOT) * 0.5, 0.018, 0.018), Basis(), [Vector3(-s, 0, 0), Vector3(s, 0, 0)])
		b.stretch = 1
		b.role = ROLE_HOUSING
		box(b, Vector3(s * (REF_REACH + 0.002), 3.138, 0), Vector3(0.168, 0.038, 0.058), Basis(), [Vector3.DOWN])
		b.role = ROLE_LENS
		b.color = Color(0.85, 0, 0)
		var x0: float = s * (REF_REACH - 0.14)
		var x1: float = s * (REF_REACH + 0.15)
		polygon(b, [Vector3(x0, 3.1, -0.055), Vector3(x0, 3.1, 0.055), Vector3(x1, 3.1, 0.055), Vector3(x1, 3.1, -0.055)], Vector3.DOWN)
		b.stretch = 0
	var mesh := ArrayMesh.new()
	b.commit(mesh, material)
	mesh.resource_name = "mk_city_lamp_far"
	mesh.custom_aabb = envelope()
	return mesh

## Halo mesh: one quad per head, corners in VERTEX.xy (-1..1), the head side
## in VERTEX.z (city_lamp_halo.gdshader billboards it round the head).
static func halo_mesh(material: Material) -> ArrayMesh:
	var b := Builder.new()
	b.role = ROLE_LENS
	for s in [1.0, -1.0]:
		b.part = PART_ALL if s > 0.0 else PART_TWIN
		var first := b.vertices.size()
		for corner in [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)]:
			b.vertices.append(Vector3(corner.x, corner.y, s)); b.normals.append(Vector3.BACK); b.colors.append(Color.WHITE)
			b.uvs.append(Vector2(ROLE_LENS, b.part)); b.uv2s.append(Vector2(s, 0))
		b.indices.append_array([first, first + 1, first + 2, first, first + 2, first + 3])
	var mesh := ArrayMesh.new()
	b.commit(mesh, material)
	mesh.resource_name = "mk_city_lamp_halo"
	mesh.custom_aabb = envelope().grow(0.6)
	return mesh

## Culling bounds of every design and reach: the contract envelope (twins
## to -(MAX twin reach + tip)).
static func envelope() -> AABB:
	var back := -(1.35 + float(ENVELOPE_CM[1]) * 0.01)
	var tip := MAX_REACH + float(ENVELOPE_CM[1]) * 0.01
	var side := float(ENVELOPE_CM[2]) * 0.01
	return AABB(Vector3(back, -SKIRT, -side), Vector3(tip - back, float(ENVELOPE_CM[3]) * 0.01 + SKIRT, side * 2.0))

## Whether a part survives for design `flags` (city_lamp.gdshader keep()).
static func keep(part: int, flags: int) -> bool:
	var twin := flags & FLAG_TWIN != 0
	var straight := flags & FLAG_STRAIGHT != 0
	match part:
		PART_BENT: return not straight
		PART_STRAIGHT: return straight
		PART_BANNER: return flags & FLAG_BANNER != 0
		PART_PEDESTRIAN: return flags & FLAG_PEDESTRIAN != 0
		PART_BOX: return flags & FLAG_BOX != 0
		PART_TWIN_BENT: return twin and not straight
		PART_TWIN_STRAIGHT: return twin and straight
		PART_ROUND: return flags & FLAG_OCTAGONAL == 0
		PART_OCTAGONAL: return flags & FLAG_OCTAGONAL != 0
		PART_TWIN: return twin
	return true

## A vertex as the shader places it for reach `reach_cm` and `flags`
## (validator mirror of city_lamp.gdshader vertex()); null when collapsed.
static func placed(point: Vector3, uv: Vector2, uv2: Vector2, reach_cm: int, flags: int) -> Variant:
	if not keep(int(round(uv.y)), flags): return null
	return point + Vector3(uv2.x * (float(reach_cm) * 0.01 - REF_REACH), 0, 0)

## Shared materials: the lamp body (city_lamp.gdshader, weather and the
## night lens from the environment context) and the additive halo.
static func material(context: RefCounted = null, halo := false) -> ShaderMaterial:
	var made := ShaderMaterial.new()
	var shader := Shader.new()
	shader.code = (preload("./city_lamp_halo.gdshader") if halo else preload("./city_lamp.gdshader")).code
	made.shader = shader
	if context != null and "texture" in context:
		made.set_shader_parameter("environment_data", context.texture)
		made.set_shader_parameter("environment_enabled", true)
	made.set_shader_parameter("led_color", LED)
	made.set_meta("mapkit_opaque", not halo)
	made.resource_name = "mk_city_lamp_" + ("halo" if halo else "body")
	return made

## One part per call (a material or a mesh); true when the whole set is in
## state.result. The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {}
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "city_lamp_" + step
	var result: Dictionary = state.result
	var made: Resource
	match step:
		"material": made = material(context)
		"halo_material": made = material(context, true)
		"near": made = near_mesh(result.material)
		"far": made = far_mesh(result.material)
		_: made = halo_mesh(result.halo_material)
	result[step] = made
	if lease != null:
		lease.track(made)
		if made is ShaderMaterial: lease.track(made.shader)
	state.step += 1
	if state.step < STEPS.size(): return false
	result.make_read_only()
	return true

## Process-lifetime set for cells without a resource cache (standalone and
## editor previews), one part per call; the set once complete, else {}.
static func step_shared() -> Dictionary:
	if not _shared.is_empty(): return _shared
	if build(_shared_job):
		_shared = _shared_job.result
		_shared_job = {}
	return _shared

## Claims the shared set as a cell begins (chunk_renderer.begin, before any
## upload) when it holds city streetlights and has a resource cache; refused,
## the cell fails (the lamps' proxy faces are hidden, this is their only body).
static func claim(job: Dictionary, objects: Array) -> bool:
	if job.get("resources") == null or job.claims.has(KEY): return true
	if not objects.any(func(object: Dictionary) -> bool: return IDS.has(str(object.asset_id))): return true
	var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
	if key.is_empty(): return false
	job.claims[KEY] = key
	return true

## Shared set for a renderer job: 1 ready, 0 another step is needed, -2 it is
## unavailable (fails the cell).
static func ready(job: Dictionary) -> int:
	if job.has("city_lamps"): return 1
	job.last_phase = "city_lamp_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.city_lamps = made
		return 0
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.city_lamps = made
	return 0

static func _mix(value: int) -> int:
	value = (value ^ (value >> 16)) * 0x45d9f3b & 0xffffffff
	value = (value ^ (value >> 16)) * 0x45d9f3b & 0xffffffff
	return value ^ (value >> 16)

## Design of a lamp: [flags, paint + 4 x banner palette]. The street design
## hashes the lamp's DESIGN_SQUARE_CM square and its road axis (yaw modulo
## 180 degrees in 30 degree steps), so the lamps of one street inside one
## square match on both sides. The square is a fixed world grid, not the
## road: a row changes design where it crosses a square edge, the two sides
## of a street whose centreline runs within half its width of an edge fall
## in different squares, and opposite yaws near a 15 + 30k degree bucket edge
## can differ. Control boxes and a banner street's gaps hash the object
## identity.
static func design(object: Dictionary) -> Array:
	var id := str(object.asset_id)
	var twin: bool = IDS.get(id, [0, false])[1]
	var p: Array = object.get("position", [0, 0, 0])
	var yaw := float(int(object.get("quarter_turns", 0))) * 90.0 + float(object.get("yaw_offset_mdeg", 0)) / 1000.0
	var axis := int(roundf(fposmod(yaw, 180.0) / 30.0)) % 6
	var street := _mix(_mix(_mix(floori(float(p[0]) / DESIGN_SQUARE_CM) & 0xffff) ^ (floori(float(p[2]) / DESIGN_SQUARE_CM) & 0xffff) * 65537) ^ axis * 2654435761)
	var unit := func(value: int, salt: int) -> float: return float(_mix(value ^ salt * 0x9e3779b) & 0xffffff) / 16777216.0
	var own := str(object.get("id", "")).sha256_text().substr(0, 7).hex_to_int()
	var flags := FLAG_TWIN if twin else 0
	if unit.call(street, 1) < STRAIGHT_SHARE: flags |= FLAG_STRAIGHT
	if unit.call(street, 2) < OCTAGONAL_SHARE: flags |= FLAG_OCTAGONAL
	if unit.call(street, 3) < BANNER_STREET_SHARE and unit.call(own, 4) < BANNER_LAMP_SHARE: flags |= FLAG_BANNER
	if not twin and unit.call(street, 5) < PEDESTRIAN_SHARE: flags |= FLAG_PEDESTRIAN
	if unit.call(own, 6) < BOX_SHARE: flags |= FLAG_BOX
	var paint := 0
	var pick: float = unit.call(street, 7)
	while paint < PAINT_SHARES.size() - 1 and pick >= PAINT_SHARES[paint]: paint += 1
	return [flags, paint + 4 * (_mix(street ^ 8) & 3)]

## Display transform of a lamp: position, quarter turns and yaw offset, as
## for its pole box.
static func placement(object: Dictionary) -> Transform3D:
	var yaw := float(int(object.quarter_turns)) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0)
	var p: Array = object.position
	return Transform3D(Basis(Vector3.UP, yaw), Vector3(float(p[0]), float(p[1]), -float(p[2])) * 0.01)

## One admitted step per object: its transform, reach and design join the
## cell's group of its 16 m square (attached one group per step after the
## object pass).
static func plan(job: Dictionary, object: Dictionary) -> void:
	var transform := placement(object)
	var o := transform.origin
	var key := "%d,%d" % [floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
	if not job.has("city_lamp_plan"): job.city_lamp_plan = {}
	if not job.city_lamp_plan.has(key): job.city_lamp_plan[key] = {"parts": [], "ids": PackedStringArray()}
	var b := transform.basis
	var chosen := design(object)
	job.city_lamp_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z,
		float(IDS[str(object.asset_id)][0]), float(chosen[0]), float(chosen[1]), 1.0]))
	job.city_lamp_plan[key].ids.append(str(object.id))

## Near, far and halo MultiMeshes of one planned group. The cell lease
## carries the nodes and instance buffers; meshes and materials belong to the
## shared set. mapkit_object_ids lists the instances' object ids (editor
## picking reads the far node, mapkit_furniture_lod "far").
static func attach_group(job: Dictionary) -> void:
	var key: String = job.city_lamp_plan.keys()[0]
	var group: Dictionary = job.city_lamp_plan[key]
	job.city_lamp_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in LODS:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.use_custom_data = true
		multi.mesh = job.city_lamps[lod]
		multi.instance_count = buffer.size() / FLOATS
		multi.buffer = buffer
		var node := MultiMeshInstance3D.new()
		node.name = "CityLamp_%s_%d" % [lod, job.root.get_child_count()]
		node.multimesh = multi
		node.set_meta("mapkit_static_detail", true)
		node.set_meta("mapkit_decoration", false)
		node.set_meta("mapkit_furniture_lod", lod)
		node.set_meta("mapkit_furniture_kind", "city_streetlight")
		node.set_meta("mapkit_object_ids", group.ids)
		if lod == "halo": node.add_to_group(QUALITY.HALO_GROUP)
		QUALITY.apply_node(node, quality)
		if job.get("lease") != null:
			job.lease.track(node)
			job.lease.track(multi)
		job.root.add_child(node)

## Binds the cell's lamp light map (presentation lamp_map, lamp_map_rect,
## lamp_map_height; mapkit-godot lamp_map.rs) on one of its road or sidewalk
## materials. The first call makes the texture (RG8 without mipmaps, so
## neighbouring cells' border texels match at every distance; tracked by the
## cell lease); a cell without a map leaves the material unlit.
static func bind_light_map(job: Dictionary, material: Material, presentation: Dictionary) -> void:
	if not material is ShaderMaterial: return
	var bytes: Variant = presentation.get("lamp_map", PackedByteArray())
	var rect: Variant = presentation.get("lamp_map_rect", PackedFloat32Array())
	var height: Variant = presentation.get("lamp_map_height", PackedFloat32Array())
	if not bytes is PackedByteArray or not rect is PackedFloat32Array or rect.size() != 4: return
	if not height is PackedFloat32Array or height.size() != 2: return
	var n := int(rect[3])
	if n < 2 or bytes.size() != n * n * 2: return
	if not job.has("lamp_texture"):
		var image := Image.create_from_data(n, n, false, Image.FORMAT_RG8, bytes)
		job.lamp_texture = ImageTexture.create_from_image(image)
		if job.get("lease") != null: job.lease.track(job.lamp_texture)
	material.set_shader_parameter("lamp_enabled", true)
	material.set_shader_parameter("lamp_map", job.lamp_texture)
	material.set_shader_parameter("lamp_map_rect", Vector4(rect[0], rect[1], rect[2], rect[3]))
	material.set_shader_parameter("lamp_map_height", Vector2(height[0], height[1]))
