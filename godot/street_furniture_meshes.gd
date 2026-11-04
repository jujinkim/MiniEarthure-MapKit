extends RefCounted
## Display-only street furniture, built procedurally (no external assets): the
## builtin streetlight, a twin-arm city lamp, and the builtin bench, a slatted
## park bench. Each has a `near` mesh (full detail) and a simple `far` mesh;
## materials are shared painted metal, wood and the lamps' lens.
##
## Streetlight (metres, origin at the foot): a square base filling the pole
## box at car height (plinth plate, skirt, access door with lock, number plate
## and cornice), a tapered transition and collar, a tapered round shaft to
## 4.84 m, a hub and two curved arms carrying one slim luminaire each, whose
## underside diffuser (pale by day) glows white at night (atmosphere_surface
## lighting_role 2, emission only). Everything stays inside the unchanged
## collision boxes (placement.rs: pole 20 x 500 x 20 cm, head 80 x 25 x 40 cm
## at 5 m): base and shaft inside the pole box, arms and heads inside the head
## box. The renderer draws this mesh instead of the generated proxy faces
## (render_memory.hidden_objects); collision, occluders and lamp bindings
## never read those faces.
##
## Bench (0.6 m wide, display only, 40 cm reserved footprint): four seat and
## three backrest slats with gaps, their grain baked into vertex colours
## (lengthwise streaks, darker weathered ends and end grain, per-slat tint) so
## it reads at any orientation; two painted cast side frames with legs, feet,
## seat rail, reclined back support and armrest, and a rear stretcher;
## bevelled edges. The far mesh is a seat, a backrest and two side plates.
##
## A cell instances both kinds per 16 m group as near and far MultiMeshes
## (display_quality.apply_furniture: ranges, shadows, low shows the far mesh
## only), each node listing its placements' object ids (mapkit_object_ids,
## instance order) for editor picking. build() makes one part per call for
## admitted streaming steps (render_resource_cache.prepare_mesh_set);
## step_shared() builds a process-lifetime set for cells without a resource
## cache.

const QUALITY := preload("./display_quality.gd")
const KINDS := {"builtin:streetlight": "streetlight", "builtin:bench": "bench"}
const LODS := ["near", "far"]
const KEY := "builtin:street_furniture"
## Group square: small enough that the group reach of the near ranges
## (display_quality.FURNITURE_REACH, half its diagonal) stays short.
const GROUP_METRES := 16.0
## Collision boxes of builtin:streetlight (placement.rs builtin), metres.
const STREETLIGHT_PROXIES := [AABB(Vector3(-0.1, 0.0, -0.1), Vector3(0.2, 5.0, 0.2)), AABB(Vector3(-0.4, 4.875, -0.2), Vector3(0.8, 0.25, 0.4))]
## Reserved footprint radius of builtin:bench (osm_furniture.py), metres.
const BENCH_RADIUS := 0.4
## Paint (sRGB): streetlight pole, luminaire housing, bench frames.
const POLE := "4a5155"
const HOUSING := "70777b"
const FRAME := "2f4638"
const WOOD := "9c6a3e"
## Lens colour: a pale neutral diffuser by day; its night emission (x 0.9)
## is a white LED glow.
const LENS := Color(0.9, 0.9, 0.86)
## Wood grain (bench slats): end-grain faces and the last END_METRES of each
## slat are darker, lengthwise streaks vary by up to GRAIN.
const END_SHADE := 0.72
const END_METRES := 0.04
const GRAIN := 0.1
const SURFACES := {"paint": {"tile": "metal", "metallic": 0.3, "roughness": 0.55},
	"wood": {"tile": "wood", "metallic": 0.0, "roughness": 0.8},
	"lens": {"tile": "", "metallic": 0.0, "roughness": 0.25}}
const STEPS := ["material_paint", "material_wood", "material_lens", "streetlight_near", "streetlight_far", "bench_near", "bench_far"]
## Memory allowance of one shared set (render_resource_cache): about 4,100
## vertices at <= 56 B and 7,200 indices at 4 B in CPU arrays and GPU buffers
## (0.51 MB, street_furniture_validator measures them), 64 KiB for the three
## materials and one mesh's build arrays.
const SHARED_BYTES := 640 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

class Builder:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var colors := PackedColorArray()
	var indices := PackedInt32Array()
	func add(point: Vector3, normal: Vector3, color: Color) -> int:
		vertices.append(point); normals.append(normal); colors.append(color)
		return vertices.size() - 1
	## Godot front faces wind clockwise seen from their normal side.
	func triangle(a: int, b: int, c: int) -> void:
		var facing := (vertices[b] - vertices[a]).cross(vertices[c] - vertices[a])
		if facing.dot(normals[a] + normals[b] + normals[c]) > 0.0:
			indices.append(a); indices.append(c); indices.append(b)
		else:
			indices.append(a); indices.append(b); indices.append(c)
	func commit(mesh: ArrayMesh, material: Material) -> void:
		if indices.is_empty(): return
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_NORMAL] = normals
		arrays[Mesh.ARRAY_COLOR] = colors
		arrays[Mesh.ARRAY_INDEX] = indices
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mesh.surface_set_material(mesh.get_surface_count() - 1, material)

static func linear(srgb: String, shade := 1.0) -> Color:
	return shaded(Color(srgb).srgb_to_linear(), shade)

static func shaded(color: Color, shade: float) -> Color:
	return Color(color.r * shade, color.g * shade, color.b * shade, 1.0)

## Convex planar polygon (any vertex order) facing `normal`, flat shaded.
static func polygon(builder: Builder, points: Array, normal: Vector3, color: Color) -> void:
	var centre := Vector3.ZERO
	for point: Vector3 in points: centre += point
	centre /= float(points.size())
	var u: Vector3 = ((points[0] as Vector3) - centre).normalized()
	var v := normal.cross(u)
	var ordered := points.duplicate()
	ordered.sort_custom(func(a: Vector3, b: Vector3) -> bool: return atan2((a - centre).dot(v), (a - centre).dot(u)) < atan2((b - centre).dot(v), (b - centre).dot(u)))
	var first := builder.vertices.size()
	for point: Vector3 in ordered: builder.add(point, normal, color)
	for index in range(1, ordered.size() - 1): builder.triangle(first, first + index, first + index + 1)

## Box of half extents `half` at `centre` turned by `basis` (orthonormal), its
## edges chamfered by `bevel` (0: a plain box). `shade` darkens the bottom.
## `grain` >= 0 (a seed) makes it a wooden slat along local x: grained faces.
static func box(builder: Builder, centre: Vector3, half: Vector3, bevel: float, color: Color, basis := Basis(), bottom_shade := 1.0, grain := -1) -> void:
	var place := func(local: Vector3) -> Vector3: return centre + basis * local
	var tint := func(local: Vector3) -> Color: return shaded(color, lerpf(bottom_shade, 1.0, clampf(local.y / maxf(half.y, 1e-4) * 0.5 + 0.5, 0.0, 1.0)))
	var inner := half - Vector3.ONE * bevel
	for axis in 3:
		for sign_value in [-1.0, 1.0]:
			var points := []
			var normal := Vector3.ZERO
			normal[axis] = sign_value
			if grain >= 0:
				_grained_face(builder, place, tint.call(normal * half), half, inner, axis, sign_value, basis * normal, grain)
				continue
			for a in [-1.0, 1.0]:
				for b in [-1.0, 1.0]:
					# The face plane at the full extent, its corners inset by the bevel.
					var local := Vector3(inner.x * a, inner.y * b, inner.z * b)
					if axis == 0: local = Vector3(half.x * sign_value, inner.y * a, inner.z * b)
					elif axis == 1: local = Vector3(inner.x * a, half.y * sign_value, inner.z * b)
					else: local = Vector3(inner.x * a, inner.y * b, half.z * sign_value)
					points.append(place.call(local))
			polygon(builder, points, basis * normal, tint.call(normal * half))
	if bevel <= 0.0: return
	for axes: Vector2i in [Vector2i(0, 1), Vector2i(0, 2), Vector2i(1, 2)]:
		var third := 3 - axes.x - axes.y
		for sa in [-1.0, 1.0]:
			for sb in [-1.0, 1.0]:
				var points := []
				for sc in [-1.0, 1.0]:
					for first_flat in [true, false]:
						var local := Vector3.ZERO
						local[axes.x] = sa * (half[axes.x] if first_flat else inner[axes.x])
						local[axes.y] = sb * (inner[axes.y] if first_flat else half[axes.y])
						local[third] = sc * inner[third]
						points.append(place.call(local))
				var normal := Vector3.ZERO
				normal[axes.x] = sa
				normal[axes.y] = sb
				polygon(builder, points, (basis * normal).normalized(), tint.call(normal.normalized() * half))
	for sx in [-1.0, 1.0]:
		for sy in [-1.0, 1.0]:
			for sz in [-1.0, 1.0]:
				var signs := Vector3(sx, sy, sz)
				var points := [place.call(Vector3(half.x, inner.y, inner.z) * signs), place.call(Vector3(inner.x, half.y, inner.z) * signs), place.call(Vector3(inner.x, inner.y, half.z) * signs)]
				polygon(builder, points, (basis * signs).normalized(), tint.call(signs * half))

## One face of a wooden slat along local x (box with `grain`): end faces in
## dark end grain; the long faces in lengthwise streaks (three on the wide
## faces, one on the narrow edges) with their own vertices, each streak
## darkening over the last END_METRES towards both ends.
static func _grained_face(builder: Builder, place: Callable, color: Color, half: Vector3, inner: Vector3, axis: int, sign_value: float, normal: Vector3, seed_value: int) -> void:
	if axis == 0:
		var points := []
		for a in [-1.0, 1.0]:
			for b in [-1.0, 1.0]: points.append(place.call(Vector3(half.x * sign_value, inner.y * a, inner.z * b)))
		polygon(builder, points, normal, shaded(color, END_SHADE))
		return
	var across := 3 - axis
	var cuts := [-1.0, 1.0]
	if half[across] > half[axis]:
		cuts = [-1.0, -0.36 + 0.16 * _noise(seed_value, axis, sign_value, 7), 0.3 + 0.16 * _noise(seed_value, axis, sign_value, 8), 1.0]
	var xs := [-inner.x, -inner.x + END_METRES, inner.x - END_METRES, inner.x]
	var ends := [END_SHADE + 0.1, 1.0, 1.0, END_SHADE + 0.1]
	for band in cuts.size() - 1:
		var streak := _noise(seed_value, axis, sign_value, band)
		var warm := _noise(seed_value, axis, sign_value, band + 4)
		var base := shaded(color, 1.0 + GRAIN * streak)
		base = Color(base.r * (1.0 + 0.04 * warm), base.g, base.b * (1.0 - 0.06 * warm), 1.0)
		var first := builder.vertices.size()
		for k in xs.size():
			for edge in [cuts[band], cuts[band + 1]]:
				var local := Vector3(xs[k], 0.0, 0.0)
				local[axis] = half[axis] * sign_value
				local[across] = inner[across] * float(edge)
				builder.add(place.call(local), normal, shaded(base, ends[k]))
		for k in xs.size() - 1:
			var a := first + k * 2
			builder.triangle(a, a + 1, a + 3)
			builder.triangle(a, a + 3, a + 2)

## Deterministic value in [-1, 1] for a slat face's streak.
static func _noise(seed_value: int, axis: int, sign_value: float, salt: int) -> float:
	var x := sin(float(seed_value * 131 + axis * 37 + int(sign_value + 1.0) * 17 + salt * 7) * 12.9898) * 43758.5453
	return (x - floorf(x)) * 2.0 - 1.0

## Box from `a` to `b` (its local y), `across` its local x direction, of
## width (local x) and depth (local z).
static func bar(builder: Builder, a: Vector3, b: Vector3, across: Vector3, width: float, depth: float, bevel: float, color: Color) -> void:
	var along := (b - a).normalized()
	var x := (across - along * across.dot(along)).normalized()
	box(builder, (a + b) * 0.5, Vector3(width * 0.5, (b - a).length() * 0.5, depth * 0.5), bevel, color, Basis(x, along, x.cross(along)))

## Smooth surface through `rings` (equal point counts), outward normals from
## each ring's centre; optional flat caps facing away from the next ring.
static func loft(builder: Builder, rings: Array, color: Color, cap_start := false, cap_end := false, colors := []) -> void:
	var count: int = (rings[0] as Array).size()
	var starts := []
	for index in rings.size():
		var ring: Array = rings[index]
		var centre := Vector3.ZERO
		for point: Vector3 in ring: centre += point
		centre /= float(count)
		var next_ring: Array = rings[mini(index + 1, rings.size() - 1)]
		var previous_ring: Array = rings[maxi(index - 1, 0)]
		var axis := (_centre(next_ring) - _centre(previous_ring)).normalized()
		starts.append(builder.vertices.size())
		var shade: Color = colors[index] if index < colors.size() else color
		for point: Vector3 in ring:
			var out := point - centre
			out = (out - axis * out.dot(axis)).normalized()
			builder.add(point, out, shade)
	for index in rings.size() - 1:
		for k in count:
			var a: int = starts[index] + k
			var b: int = starts[index] + (k + 1) % count
			var c: int = starts[index + 1] + (k + 1) % count
			var d: int = starts[index + 1] + k
			builder.triangle(a, b, c)
			builder.triangle(a, c, d)
	if cap_start: polygon(builder, rings[0], (_centre(rings[0]) - _centre(rings[1])).normalized(), colors[0] if not colors.is_empty() else color)
	if cap_end: polygon(builder, rings[-1], (_centre(rings[-1]) - _centre(rings[-2])).normalized(), colors[-1] if not colors.is_empty() else color)

static func _centre(ring: Array) -> Vector3:
	var centre := Vector3.ZERO
	for point: Vector3 in ring: centre += point
	return centre / float(ring.size())

## Round tube along `centres` with one radius per centre and `sides` facets.
static func tube(builder: Builder, centres: Array, radii: Array, sides: int, color: Color, cap_start := false, cap_end := false, colors := []) -> void:
	var rings := []
	var normal := Vector3.ZERO
	for index in centres.size():
		var forward: Vector3 = ((centres[mini(index + 1, centres.size() - 1)] as Vector3) - (centres[maxi(index - 1, 0)] as Vector3)).normalized()
		if index == 0: normal = (Vector3.FORWARD if absf(forward.dot(Vector3.FORWARD)) < 0.9 else Vector3.RIGHT).cross(forward).normalized()
		else: normal = (normal - forward * normal.dot(forward)).normalized()
		var side := forward.cross(normal)
		var ring := []
		for k in sides:
			var angle := TAU * (float(k) + 0.5) / float(sides)
			ring.append((centres[index] as Vector3) + (normal * cos(angle) + side * sin(angle)) * float(radii[index]))
		rings.append(ring)
	loft(builder, rings, color, cap_start, cap_end, colors)

## Chamfered rectangle across x at `x`: half width (z) `half`, from `bottom`
## to `top`, corners cut by `cut`.
static func section(x: float, half: float, bottom: float, top: float, cut: float) -> Array:
	return [Vector3(x, bottom, -half + cut), Vector3(x, bottom, half - cut), Vector3(x, bottom + cut, half), Vector3(x, top - cut, half),
		Vector3(x, top, half - cut), Vector3(x, top, -half + cut), Vector3(x, top - cut, -half), Vector3(x, bottom + cut, -half)]

static func bezier(a: Vector3, b: Vector3, c: Vector3, t: float) -> Vector3:
	return a * (1.0 - t) * (1.0 - t) + b * 2.0 * t * (1.0 - t) + c * t * t

## Twin-arm streetlight: surfaces paint and lens.
static func streetlight(lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := Builder.new()
	var lens := Builder.new()
	var pole := linear(POLE)
	var housing := linear(HOUSING)
	if lod == "near":
		# Square base filling the pole box at car height: plinth plate,
		# skirt, body with access door, lock and number plate, cornice; a
		# tapered transition to the collar and the round tapered shaft.
		box(paint, Vector3(0.0, 0.0125, 0.0), Vector3(0.099, 0.0125, 0.099), 0.006, pole, Basis(), 0.7)
		box(paint, Vector3(0.0, 0.055, 0.0), Vector3(0.0985, 0.03, 0.0985), 0.008, linear(POLE, 0.9), Basis(), 0.85)
		box(paint, Vector3(0.0, 0.445, 0.0), Vector3(0.095, 0.36, 0.095), 0.007, pole, Basis(), 0.88)
		box(paint, Vector3(0.0, 0.82, 0.0), Vector3(0.0985, 0.018, 0.0985), 0.007, linear(POLE, 0.95), Basis(), 0.8)
		box(paint, Vector3(0.0, 0.42, 0.0965), Vector3(0.036, 0.2, 0.0015), 0.0015, linear(POLE, 0.93))
		box(paint, Vector3(0.0, 0.47, 0.0985), Vector3(0.006, 0.012, 0.0015), 0.0, linear(POLE, 0.55))
		box(paint, Vector3(0.0965, 0.66, 0.0), Vector3(0.0015, 0.022, 0.032), 0.0, linear("cfcabb"))
		for k in 4:
			var out := Basis(Vector3.UP, float(k) * PI / 2.0) * Vector3.BACK
			var side := Vector3.UP.cross(out)
			var points := [out * 0.09 + side * 0.09 + Vector3.UP * 0.838, out * 0.09 - side * 0.09 + Vector3.UP * 0.838,
				out * 0.066 - side * 0.066 + Vector3.UP * 0.885, out * 0.066 + side * 0.066 + Vector3.UP * 0.885]
			polygon(paint, points, (out * 0.047 + Vector3.UP * 0.024).normalized(), linear(POLE, 0.92))
		polygon(paint, [Vector3(0.066, 0.885, 0.066), Vector3(-0.066, 0.885, 0.066), Vector3(-0.066, 0.885, -0.066), Vector3(0.066, 0.885, -0.066)], Vector3.UP, pole)
		tube(paint, [Vector3(0.0, 0.885, 0.0), Vector3(0.0, 0.9, 0.0), Vector3(0.0, 0.925, 0.0)], [0.064, 0.066, 0.062], 16, linear(POLE, 0.95), false, true)
		tube(paint, [Vector3(0.0, 0.925, 0.0), Vector3(0.0, 2.9, 0.0), Vector3(0.0, 4.84, 0.0)], [0.056, 0.05, 0.044], 14, pole)
		tube(paint, [Vector3(0.0, 4.82, 0.0), Vector3(0.0, 4.9, 0.0), Vector3(0.0, 4.93, 0.0)], [0.05, 0.05, 0.034], 14, linear(POLE, 0.95), true, true)
		for s in [-1.0, 1.0]:
			# A curved arm rising inside the pole box and running out inside
			# the head box, then the luminaire: housing lofted from the arm's
			# neck to a rounded end, the lens underneath.
			var arm := []
			var radii := []
			for step in 8:
				var t := float(step) / 7.0
				arm.append(bezier(Vector3(0.0, 4.885, 0.0), Vector3(s * 0.01, 5.045, 0.0), Vector3(s * 0.18, 5.04, 0.0), t))
				radii.append(lerpf(0.017, 0.014, t))
			tube(paint, arm, radii, 8, pole, false, true)
			var rings := [section(s * 0.165, 0.03, 5.02, 5.05, 0.008), section(s * 0.215, 0.08, 4.997, 5.07, 0.015),
				section(s * 0.355, 0.085, 4.997, 5.068, 0.016), section(s * 0.395, 0.06, 5.006, 5.055, 0.014)]
			loft(paint, rings, housing, true, true)
			box(lens, Vector3(s * 0.285, 4.9925, 0.0), Vector3(0.0675, 0.0035, 0.07), 0.0025, Color.WHITE)
	else:
		box(paint, Vector3(0.0, 0.42, 0.0), Vector3(0.096, 0.42, 0.096), 0.0, pole, Basis(), 0.85)
		tube(paint, [Vector3(0.0, 0.84, 0.0), Vector3(0.0, 4.9, 0.0)], [0.062, 0.045], 6, pole, false, true)
		for s in [-1.0, 1.0]:
			bar(paint, Vector3(0.0, 4.9, 0.0), Vector3(s * 0.18, 5.035, 0.0), Vector3.BACK, 0.028, 0.028, 0.0, pole)
			box(paint, Vector3(s * 0.28, 5.032, 0.0), Vector3(0.115, 0.035, 0.085), 0.0, housing)
			box(lens, Vector3(s * 0.285, 4.993, 0.0), Vector3(0.0675, 0.004, 0.07), 0.0, Color.WHITE)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	lens.commit(mesh, materials.get("lens"))
	mesh.resource_name = "mk_streetlight_" + lod
	return mesh

## Seat slats (36 mm deep, 8 mm gaps) and backrest slats (34 mm, 11 mm gaps).
const SEAT_SLATS := 4
const BACK_SLATS := 3
## Park bench: surfaces wood and paint. Local -z is the backrest side.
static func bench(lod: String, materials: Dictionary) -> ArrayMesh:
	var wood := Builder.new()
	var paint := Builder.new()
	var frame := linear(FRAME)
	# Back support: from the rear leg's top, reclined 11 degrees.
	var back_foot := Vector3(0.0, 0.185, -0.06)
	var back_top := Vector3(0.0, 0.375, -0.098)
	var along := (back_top - back_foot).normalized()
	var facing := Vector3.RIGHT.cross(along).normalized()
	if facing.z < 0.0: facing = -facing
	var back_basis := Basis(Vector3.RIGHT, along, facing)
	if lod == "near":
		for index in SEAT_SLATS:
			var z := -0.084 + 0.018 + float(index) * 0.044
			box(wood, Vector3(0.0, 0.1915, z), Vector3(0.292, 0.0065, 0.018), 0.003, _slat(index), Basis(), 0.85, index)
		for index in BACK_SLATS:
			var centre := back_foot + along * (0.058 + float(index) * 0.045) + facing * 0.015
			box(wood, centre, Vector3(0.292, 0.017, 0.006), 0.003, _slat(SEAT_SLATS + index), back_basis, 0.9, SEAT_SLATS + index)
		for sx in [-1.0, 1.0]:
			var x: float = 0.265 * sx
			var at := func(y: float, z: float) -> Vector3: return Vector3(x, y, z)
			bar(paint, at.call(0.004, 0.07), at.call(0.268, 0.064), Vector3.RIGHT, 0.022, 0.02, 0.004, frame)
			bar(paint, at.call(0.004, -0.068), at.call(0.188, -0.06), Vector3.RIGHT, 0.022, 0.02, 0.004, frame)
			bar(paint, at.call(0.185, -0.06), at.call(0.375, -0.098), Vector3.RIGHT, 0.022, 0.018, 0.004, frame)
			bar(paint, at.call(0.178, -0.078), at.call(0.178, 0.082), Vector3.RIGHT, 0.02, 0.016, 0.003, frame)
			bar(paint, at.call(0.272, -0.07), at.call(0.268, 0.09), Vector3.RIGHT, 0.03, 0.012, 0.004, frame)
			box(paint, at.call(0.262, 0.09), Vector3(0.016, 0.011, 0.011), 0.004, frame)
			for z in [0.07, -0.068]:
				box(paint, at.call(0.006, z), Vector3(0.017, 0.006, 0.02), 0.002, linear(FRAME, 0.8))
		bar(paint, Vector3(-0.255, 0.05, -0.066), Vector3(0.255, 0.05, -0.066), Vector3.BACK, 0.012, 0.012, 0.002, frame)
	else:
		box(wood, Vector3(0.0, 0.1915, 0.0), Vector3(0.292, 0.0065, 0.084), 0.0, _slat(0))
		box(wood, back_foot + along * 0.103 + facing * 0.015, Vector3(0.292, 0.062, 0.006), 0.0, _slat(5), back_basis)
		for sx in [-1.0, 1.0]:
			box(paint, Vector3(0.265 * sx, 0.136, 0.0), Vector3(0.011, 0.136, 0.078), 0.0, frame)
	var mesh := ArrayMesh.new()
	wood.commit(mesh, materials.get("wood"))
	paint.commit(mesh, materials.get("paint"))
	mesh.resource_name = "mk_bench_" + lod
	return mesh

## Wood tint of a slat: brightness and warmth vary slat by slat.
static func _slat(index: int) -> Color:
	var shade: float = [1.0, 0.9, 1.06, 0.95, 1.03, 0.92, 0.98][index % 7]
	var base := linear(WOOD, shade)
	return Color(base.r * (1.0 + 0.04 * float(index % 3 - 1)), base.g, base.b * (1.0 - 0.05 * float(index % 2)), 1.0)

## Shared material of `kind` (SURFACES): the environment context's surface
## shader (weather, detail tiles, night emission of the lens) when given,
## else a plain material for standalone previews.
static func material(kind: String, context: RefCounted = null) -> Material:
	var surface: Dictionary = SURFACES[kind]
	var lens := kind == "lens"
	if context != null and context.has_method("generated_material"):
		var made: ShaderMaterial = context.generated_material(LENS if lens else Color.WHITE, str(surface.tile), float(surface.metallic), float(surface.roughness))
		made.set_shader_parameter("vertex_tinted", not lens)
		made.set_shader_parameter("lighting_role", 2 if lens else 0)
		made.resource_name = "mk_furniture_" + kind
		return made
	var plain := StandardMaterial3D.new()
	plain.albedo_color = LENS if lens else Color.WHITE
	plain.vertex_color_use_as_albedo = not lens
	plain.metallic = float(surface.metallic)
	plain.roughness = float(surface.roughness)
	plain.cull_mode = BaseMaterial3D.CULL_DISABLED
	plain.resource_name = "mk_furniture_" + kind
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
	state.phase = "furniture_" + step
	var result: Dictionary = state.result
	if step.begins_with("material_"):
		var made := material(step.trim_prefix("material_"), context)
		result.materials[step.trim_prefix("material_")] = made
		if lease != null: lease.track(made)
	else:
		var lod := step.get_slice("_", 1)
		var mesh := streetlight(lod, result.materials) if step.begins_with("streetlight") else bench(lod, result.materials)
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
## upload) when it holds streetlights or benches and has a resource cache.
## Refused: false when the cell has streetlights (they would lose their only
## visible body, so the cell fails early); a cell with only benches skips
## them instead (job.furniture_refused).
static func claim(job: Dictionary, objects: Array) -> bool:
	if job.get("resources") == null or job.claims.has(KEY): return true
	var kinds := {}
	for object: Dictionary in objects:
		if KINDS.has(str(object.asset_id)): kinds[str(object.asset_id)] = true
	if kinds.is_empty(): return true
	var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
	if not key.is_empty():
		job.claims[KEY] = key
		return true
	job.furniture_refused = true
	return not kinds.has("builtin:streetlight")

## Shared set for a renderer job: 1 ready, 0 another step is needed, -2 it is
## unavailable (fails the cell, chunk_renderer), -3 refused for a cell of
## benches only (skip them, see claim()).
static func ready(job: Dictionary) -> int:
	if job.has("furniture"): return 1
	if job.get("furniture_refused", false): return -3
	job.last_phase = "furniture_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.furniture = made
		return 0
	# begin() claimed the set (claim) or failed the cell.
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.furniture = made
	return 0

## Display transform of a streetlight or bench: position, quarter turns and
## yaw offset, as for the proxy boxes and other placed assets.
static func placement(object: Dictionary) -> Transform3D:
	var yaw := float(int(object.quarter_turns)) * PI / 2.0 + deg_to_rad(float(object.get("yaw_offset_mdeg", 0)) / 1000.0)
	var p: Array = object.position
	return Transform3D(Basis(Vector3.UP, yaw), Vector3(float(p[0]), float(p[1]), -float(p[2])) * 0.01)

## One admitted step per object: its transform and id join the cell's group
## of its kind and 16 m square (attached one group per step after the object
## pass).
static func plan(job: Dictionary, object: Dictionary) -> void:
	var kind: String = KINDS[str(object.asset_id)]
	var transform := placement(object)
	var o := transform.origin
	var key := "%s/%d,%d" % [kind, floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
	if not job.has("furniture_plan"): job.furniture_plan = {}
	if not job.furniture_plan.has(key): job.furniture_plan[key] = {"kind": kind, "parts": [], "ids": PackedStringArray()}
	var b := transform.basis
	job.furniture_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z]))
	job.furniture_plan[key].ids.append(str(object.id))

## Near and far MultiMeshes of one planned group. The cell lease carries the
## nodes and instance buffers; meshes and materials belong to the shared set.
## mapkit_object_ids lists the instances' object ids (editor picking).
static func attach_group(job: Dictionary) -> void:
	var key: String = job.furniture_plan.keys()[0]
	var group: Dictionary = job.furniture_plan[key]
	job.furniture_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in LODS:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.mesh = job.furniture[group.kind + "_" + lod]
		multi.instance_count = buffer.size() / 12
		multi.buffer = buffer
		var node := MultiMeshInstance3D.new()
		node.name = "Furniture_%s_%s_%d" % [group.kind, lod, job.root.get_child_count()]
		node.multimesh = multi
		node.set_meta("mapkit_static_detail", true)
		node.set_meta("mapkit_decoration", group.kind == "bench")
		node.set_meta("mapkit_furniture_lod", lod)
		node.set_meta("mapkit_furniture_kind", group.kind)
		node.set_meta("mapkit_object_ids", group.ids)
		QUALITY.apply_node(node, quality)
		if job.get("lease") != null:
			job.lease.track(node)
			job.lease.track(multi)
		job.root.add_child(node)
