extends RefCounted
## Display-only city barrier detail (fences, walls, hedges), built
## procedurally (no external assets): steel bar panels (plain, spear tips or
## a low arch), welded wire mesh with V-folds, chain-link, wooden boards or
## pickets and low pipe rails on square, round or wooden posts; brick,
## painted block, precast and ashlar stone walls with a plinth, a coping or
## soldier course or a giwa tile cap and piers; clipped hedges with a flat
## or rounded top. Native presentation (mapkit-godot barrier_details.rs)
## lists the cell's runs (RUN_FLOATS: the clamped centreline's ground point
## and ground vector, the prism's side vector, the height, the design bits
## of mapkit-package barrier_design.rs, the phase at the start and end
## flags, an index into `barrier_ids`) and posts (POST_FLOATS: ground point,
## x vector per real metre with the rise, side vector, height, bits, post
## kind, size factor, ID index), all inside the generated collision prisms,
## whose faces render_memory.hidden_objects then hides. Collision, surface
## probes, generation and the documents are unchanged.
##
## Mesh frame (barrier_detail.gdshader): x 0..1 along a run (posts: real
## metres along), y 0..1 from the centreline ground to the top, z -0.5..0.5
## across the prism; UV2 holds real-metre offsets to y and z that the vertex
## shader converts with the instance's height, perpendicular width and map
## scale, so rails and posts keep their real sizes and bodies fill the prism
## (posts scale their offsets across by their size factor). Normals are
## authored in that real-metre frame. UV.x is the surface role, UV.y a
## baked shade plus 2 x the part's variant (keep()). Every mesh is symmetric
## in z or reads the same mirrored.
##
## A cell instances per SQUARE_METRES square: near fence runs, solid runs
## (walls and hedges), fence infill (dithered alpha test), fence posts and
## wall piers, and far fence faces (alpha blended: posts, rails and infill
## painted at the fence's far opacity) and far solid boxes (piers, plinth and
## coping painted). display_quality.apply_barrier sets their ranges and
## shadows. Far groups share the near groups' squares, and every group of a
## square carries the same custom AABB (the union of all its roles'
## instances), so Godot measures each role's visibility range to the same
## centre: a far group begins exactly where its square's near groups end and
## no barrier part is drawn twice or not at all at the handover. A cell
## planned on low (which draws no near meshes) instead plans one far fence
## and one far solid group for the whole cell, drawn from 0 on every tier
## until the cell reloads: two draws (one shadow caster) per barrier cell
## instead of one or two per 32 m square. Wall and hedge corner runs
## (barrier_details.rs RUN_CORNER) join the far solid group; hedges and giwa
## caps draw them near too (other walls' corner piers cover the wedge).
## build() makes one part per call for admitted streaming steps
## (render_resource_cache.prepare_mesh_set); step_shared() builds a
## process-lifetime set for cells without a cache.

const QUALITY := preload("./display_quality.gd")
const PALETTE := preload("./barrier_palette.json")
const KEY := "builtin:barrier_details"
const RUN_FLOATS := 13
const POST_FLOATS := 13
const SQUARE_METRES := 32.0
## Group roles: near meshes, then far meshes.
const NEAR_ROLES := ["fence", "solid", "infill", "post", "pier"]
const FAR_ROLES := ["fence_far", "solid_far"]
const STEPS := ["material", "infill_material", "far_material", "fence", "solid", "infill", "post", "pier", "fence_far", "solid_far"]
## Surface roles (UV.x) of barrier_detail.gdshader.
enum Role { RAIL, TUBE, WIRE, BOARD, INFILL, BODY, PLINTH, COPING, GIWA, HEDGE, POST, CAP, PIER, FAR_FENCE, FAR_SOLID, RIDGE }
## Part variants (UV.y / 2): 0 every design; see keep().
enum Variant { ALL = 0, STEEL_BAR = 1, WELDED_MESH = 2, CHAIN_LINK = 3, WOODEN = 4, PIPE_RAIL = 5, BRICK = 6, BLOCK = 7, PRECAST = 8, STONE = 9,
	GIWA = 10, FLAT_COPING = 11, HEDGE = 12, HEDGE_FLAT = 13, HEDGE_ROUND = 14, BOARDS = 15, PICKETS = 16, INFILL = 17, MID_RAIL = 18, WALL = 19,
	BRICK_PIER = 30, BLOCK_PIER = 31, PRECAST_POST = 32, END_PIER = 34, GIWA_END_PIER = 35 }
## Native post kinds (barrier_details.rs).
const POST_LATTICE := 0
const POST_END := 1
const POST_GATE := 2
const POST_CORNER := 3
## Native run flag of a wall or hedge corner run.
const RUN_CORNER := 16
## Real metres the parts reach below the centreline ground.
const FENCE_SINK := 0.12
const SOLID_SINK := 0.2
## Wall body inset per side (real metres): piers, plinth and coping stand out.
const BODY_INSET := 0.05
## Memory allowance of one shared set (render_resource_cache): the measured
## meshes' CPU arrays and GPU buffers (barrier_detail_validator) plus 96 KiB
## for the three materials, their shaders and one mesh's build arrays.
const SHARED_BYTES := 256 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

## Mirrors barrier_detail.gdshader barrier_keep(): whether a part of
## `variant` belongs to a design (`flags`: a post's kind; `height_m` real).
static func keep(variant: int, bits: int, flags: int, height_m: float) -> bool:
	var kind := bits & 3
	var family := (bits >> 2) & 7
	var top := (bits >> 8) & 3
	var rails := (bits >> 10) & 3
	var fence := kind == 1
	var wall := kind == 2
	var giwa := wall and family == 3 and top == 1
	if variant == 0: return true
	if variant >= 1 and variant <= 5: return fence and family == variant - 1
	if variant >= 6 and variant <= 9: return wall and family == variant - 6
	match variant:
		10: return giwa
		11: return wall and not giwa
		12: return kind == 0
		13: return kind == 0 and top == 0
		14: return kind == 0 and top == 1
		15: return fence and family == 3 and rails == 0
		16: return fence and family == 3 and rails == 1
		17: return fence and (family < 3 or (family == 3 and rails == 1))
		18: return fence and family == 0 and height_m >= 1.6
		19: return wall
		30, 31, 32: return wall and family == variant - 30 and flags == 0
		34: return wall and flags != 0 and not giwa
		35: return wall and flags != 0 and giwa
	return false

## The vertex shader's placement of mesh vertex `k` (scene metres) for an
## instance transform and custom data: the validators' CPU mirror of
## barrier_detail.gdshader vertex(); null for a collapsed part.
static func placed(arrays: Array, k: int, transform: Transform3D, custom: Color) -> Variant:
	var points: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var uv: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
	var uv2: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV2]
	var bits := int(roundf(custom.r)) | (int(roundf(custom.g)) << 8)
	var role := int(roundf(uv[k].x))
	var variant := int(floorf(uv[k].y * 0.5))
	var b := transform.basis
	var d := float(maxi((bits >> 12) & 15, 1))
	var h := maxf(b.y.length(), 1e-4)
	var lh := maxf(Vector2(b.x.x, b.x.z).length(), 1e-5)
	var xh := Vector2(b.x.x, b.x.z) / lh
	var w := maxf(absf(xh.x * b.z.z - xh.y * b.z.x), 1e-4)
	var post := role in [Role.POST, Role.CAP, Role.PIER]
	var flags := int(roundf(custom.b if post else custom.a))
	var size := custom.a if post else 1.0
	if not keep(variant, bits, flags, h * d): return null
	var p := points[k]
	p.y = minf(p.y + uv2[k].x / (d * h), 1.0)
	p.z = clampf(p.z + uv2[k].y * size / (d * w), -0.499, 0.499)
	return transform * p

class Builder:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var uv := PackedVector2Array()
	var uv2 := PackedVector2Array()
	var indices := PackedInt32Array()
	var variant := 0
	## Post meshes: x in real metres (runs: 0..1 of the run's length).
	var post := false
	## Nominal real length, height and width of the parts being added (the
	## winding test only: faces wind by their shape at a plausible size).
	var nominal := Vector3(2.0, 1.2, 0.15)
	## A corner (x, [y anchor, y offset], [z anchor, z offset]).
	static func at(x: float, y: Array, z: Array) -> Array: return [x, y, z]
	## Nominal real-metre position of a corner (winding only).
	func _real(c: Array) -> Vector3:
		return Vector3(c[0] * (1.0 if post else nominal.x), c[1][0] * nominal.y + c[1][1], c[2][0] * nominal.z + c[2][1])
	func _vertex(c: Array, normal: Vector3, role: int, shade: float) -> void:
		vertices.append(Vector3(c[0], c[1][0], c[2][0]))
		normals.append(normal.normalized())
		uv.append(Vector2(role, shade + 2.0 * variant))
		uv2.append(Vector2(c[1][1], c[2][1]))
	## Quad a-b-c-d (any winding) facing `normal` (real-metre frame); per-
	## corner normals for smooth parts.
	func quad(corners: Array, normal: Vector3, role: int, shade := 1.0, corner_normals: Array = []) -> void:
		var first := vertices.size()
		for k in 4: _vertex(corners[k], corner_normals[k] if corner_normals.size() == 4 else normal, role, shade)
		var a := _real(corners[0])
		var facing := (_real(corners[1]) - a).cross(_real(corners[2]) - a)
		# Godot front faces wind clockwise seen from their normal side.
		if facing.dot(normal) > 0.0: indices.append_array([first, first + 2, first + 1, first, first + 3, first + 2])
		else: indices.append_array([first, first + 1, first + 2, first, first + 2, first + 3])
	func tri(corners: Array, normal: Vector3, role: int, shade := 1.0) -> void:
		var first := vertices.size()
		for k in 3: _vertex(corners[k], normal, role, shade)
		var a := _real(corners[0])
		var facing := (_real(corners[1]) - a).cross(_real(corners[2]) - a)
		if facing.dot(normal) > 0.0: indices.append_array([first, first + 2, first + 1])
		else: indices.append_array([first, first + 1, first + 2])
	## Box over x0..x1, y0..y1 and z0..z1 (y and z as [anchor, offset]);
	## `skip` lists faces left out (0..5: -x, +x, -y, +y, -z, +z).
	func box(x0: float, x1: float, y0: Array, y1: Array, z0: Array, z1: Array, role: int, skip: Array = [], bottom := 0.75) -> void:
		var xs := [x0, x1]
		var ys := [y0, y1]
		var zs := [z0, z1]
		for face in 6:
			if face in skip: continue
			var axis := face / 2
			var high := face % 2
			var normal := Vector3.ZERO
			normal[axis] = 1.0 if high == 1 else -1.0
			var corners := []
			for k in 4:
				var i := [0, 0, 0]
				i[axis] = high
				i[(axis + 1) % 3] = 1 if k == 1 or k == 2 else 0
				i[(axis + 2) % 3] = 1 if k >= 2 else 0
				corners.append(at(xs[i[0]], ys[i[1]], zs[i[2]]))
			quad(corners, normal, role, 1.0 if axis != 1 else (1.0 if high == 1 else bottom))
	## Tube along x at height `y` ([anchor, offset]) on the centreline.
	func tube_x(x0: float, x1: float, y: Array, radius: float, segments: int, role: int) -> void:
		for k in segments:
			var a0 := TAU * float(k) / float(segments)
			var a1 := TAU * float(k + 1) / float(segments)
			var n0 := Vector3(0.0, sin(a0), cos(a0))
			var n1 := Vector3(0.0, sin(a1), cos(a1))
			var c := func(x: float, n: Vector3) -> Array: return at(x, [y[0], y[1] + n.y * radius], [0.0, n.z * radius])
			quad([c.call(x0, n0), c.call(x1, n0), c.call(x1, n1), c.call(x0, n1)], (n0 + n1) * 0.5, role, 1.0, [n0, n0, n1, n1])
	## Vertical post cylinder about x = 0 from y0 to y1 ([anchor, offset]),
	## topped by a dome of `dome` metres (0: none).
	func cylinder_y(radius: float, y0: Array, y1: Array, segments: int, role: int, dome := 0.0, dome_role := -1) -> void:
		for k in segments:
			var a0 := TAU * float(k) / float(segments)
			var a1 := TAU * float(k + 1) / float(segments)
			var n0 := Vector3(cos(a0), 0.0, sin(a0))
			var n1 := Vector3(cos(a1), 0.0, sin(a1))
			var c := func(y: Array, n: Vector3, r: float) -> Array: return at(n.x * r, y, [0.0, n.z * r])
			quad([c.call(y0, n0, radius), c.call(y0, n1, radius), c.call(y1, n1, radius), c.call(y1, n0, radius)], (n0 + n1) * 0.5, role, 1.0, [n0, n1, n1, n0])
			if dome > 0.0:
				var r := radius * 1.06
				var rings := 2
				for ring in rings:
					var t0 := PI * 0.5 * float(ring) / float(rings)
					var t1 := PI * 0.5 * float(ring + 1) / float(rings)
					var p := func(t: float, n: Vector3) -> Array: return at(n.x * r * cos(t), [y1[0], y1[1] + dome * sin(t)], [0.0, n.z * r * cos(t)])
					var nn := func(t: float, n: Vector3) -> Vector3: return Vector3(n.x * cos(t) / r, sin(t) / dome, n.z * cos(t) / r).normalized()
					if ring == rings - 1:
						tri([p.call(t0, n0), p.call(t0, n1), p.call(t1, n0)], nn.call((t0 + t1) * 0.5, (n0 + n1) * 0.5), dome_role)
					else:
						quad([p.call(t0, n0), p.call(t0, n1), p.call(t1, n1), p.call(t1, n0)], nn.call((t0 + t1) * 0.5, (n0 + n1) * 0.5), dome_role, 1.0,
							[nn.call(t0, n0), nn.call(t0, n1), nn.call(t1, n1), nn.call(t1, n0)])
				# Ledge under the dome.
				quad([at(n0.x * radius, y1, [0.0, n0.z * radius]), at(n1.x * radius, y1, [0.0, n1.z * radius]), at(n1.x * r, y1, [0.0, n1.z * r]), at(n0.x * r, y1, [0.0, n0.z * r])],
					Vector3.UP, dome_role, 0.85)
	func commit(mesh: ArrayMesh, material: Material) -> void:
		var arrays := []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = vertices
		arrays[Mesh.ARRAY_NORMAL] = normals
		arrays[Mesh.ARRAY_TEX_UV] = uv
		arrays[Mesh.ARRAY_TEX_UV2] = uv2
		arrays[Mesh.ARRAY_INDEX] = indices
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mesh.surface_set_material(mesh.get_surface_count() - 1, material)

const G := 0.0
const T := 1.0
## z [anchor, offset] pairs: centred offsets, or the prism's sides inset.
static func zc(offset: float) -> Array: return [0.0, offset]
static func side(sign: float, inset: float) -> Array: return [0.5 * sign, -inset * sign]

## Near fence runs: per family rails, frames, tubes, boards and back rails
## (the infill is its own mesh).
static func fence_run(material: Material) -> ArrayMesh:
	var built := Builder.new()
	built.variant = Variant.STEEL_BAR
	built.box(0.0, 1.0, [T, -0.12], [T, -0.08], zc(-0.012), zc(0.012), Role.RAIL)
	built.box(0.0, 1.0, [G, 0.06], [G, 0.10], zc(-0.012), zc(0.012), Role.RAIL)
	built.variant = Variant.MID_RAIL
	built.box(0.0, 1.0, [0.5, -0.02], [0.5, 0.02], zc(-0.012), zc(0.012), Role.RAIL)
	built.variant = Variant.WELDED_MESH
	built.box(0.0, 1.0, [T, -0.045], [T, -0.005], zc(-0.02), zc(0.02), Role.RAIL)
	built.box(0.0, 1.0, [G, 0.04], [G, 0.08], zc(-0.02), zc(0.02), Role.RAIL)
	built.variant = Variant.CHAIN_LINK
	built.tube_x(0.0, 1.0, [T, -0.035], 0.021, 6, Role.TUBE)
	built.box(0.0, 1.0, [G, 0.05], [G, 0.056], zc(-0.003), zc(0.003), Role.WIRE, [0, 1])
	built.variant = Variant.BOARDS
	for at in [0.2, 0.5, 0.8]:
		built.box(0.0, 1.0, [at, -0.06], [at, 0.06], zc(-0.011), zc(0.011), Role.BOARD)
	built.box(0.0, 1.0, [T, -0.03], [T, 0.0], zc(-0.02), zc(0.02), Role.BOARD, [], 0.6)
	built.variant = Variant.PICKETS
	for at in [0.22, 0.78]:
		built.box(0.0, 1.0, [at, -0.035], [at, 0.035], zc(0.006), zc(0.026), Role.BOARD)
	built.variant = Variant.PIPE_RAIL
	built.tube_x(0.0, 1.0, [T, -0.03], 0.024, 6, Role.TUBE)
	built.tube_x(0.0, 1.0, [0.5, 0.0], 0.024, 6, Role.TUBE)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_fence"
	return mesh

## Near fence infill: two back-to-back faces 8 mm apart over the whole
## height; the shader paints each family's bars, wires, diamonds or
## pickets inside its frame and discards the rest.
static func infill(material: Material) -> ArrayMesh:
	var built := Builder.new()
	built.variant = Variant.INFILL
	for s in [-1.0, 1.0]:
		built.quad([Builder.at(0.0, [G, 0.0], zc(0.004 * s)), Builder.at(1.0, [G, 0.0], zc(0.004 * s)), Builder.at(1.0, [T, 0.0], zc(0.004 * s)),
			Builder.at(0.0, [T, 0.0], zc(0.004 * s))], Vector3(0.0, 0.0, s), Role.INFILL)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_infill"
	return mesh

## Near walls and hedges: the inset wall body over a plinth, a frieze and
## flat coping or a giwa tile cap; hedge bodies with a flat (bevelled) or
## rounded top. The body stands on the plinth top and stops under the giwa
## eave band (flat coping continues it), so no two end faces at x = 0 or 1
## share a plane where a run end shows (line ends, gates).
static func solid_run(material: Material) -> ArrayMesh:
	var built := Builder.new()
	var sink := [G, -SOLID_SINK]
	built.nominal = Vector3(2.0, 2.0, 0.27)
	built.variant = Variant.WALL
	built.box(0.0, 1.0, [G, 0.2], [T, -0.16], side(-1.0, BODY_INSET), side(1.0, BODY_INSET), Role.BODY, [2, 3])
	built.box(0.0, 1.0, sink, [G, 0.2], side(-1.0, 0.02), side(1.0, 0.02), Role.PLINTH, [2])
	built.variant = Variant.FLAT_COPING
	built.box(0.0, 1.0, [T, -0.16], [T, -0.06], side(-1.0, BODY_INSET), side(1.0, BODY_INSET), Role.BODY, [2, 3])
	built.box(0.0, 1.0, [T, -0.06], [T, 0.0], side(-1.0, 0.0), side(1.0, 0.0), Role.COPING, [], 0.6)
	built.variant = Variant.GIWA
	# Eave band, two tile slopes up to the ridge, the ridge cap and gable ends.
	built.box(0.0, 1.0, [T, -0.16], [T, -0.13], side(-1.0, 0.0), side(1.0, 0.0), Role.GIWA, [3], 0.5)
	for s in [-1.0, 1.0]:
		var eave := Builder.at(0.0, [T, -0.13], side(s, 0.0))
		var slope := Vector3(0.0, 0.13, 0.09 * s).normalized()
		built.quad([eave, Builder.at(1.0, [T, -0.13], side(s, 0.0)), Builder.at(1.0, [T, -0.04], zc(0.0)), Builder.at(0.0, [T, -0.04], zc(0.0))], slope, Role.GIWA)
		for x in [0.0, 1.0]:
			built.tri([Builder.at(x, [T, -0.13], side(s, 0.0)), Builder.at(x, [T, -0.04], zc(0.0)), Builder.at(x, [T, -0.13], zc(0.0))], Vector3(-1.0 if x == 0.0 else 1.0, 0.0, 0.0), Role.GIWA, 0.8)
	built.box(0.0, 1.0, [T, -0.05], [T, 0.0], zc(-0.035), zc(0.035), Role.RIDGE, [2], 0.6)
	built.nominal = Vector3(2.0, 0.8, 0.6)
	built.variant = Variant.HEDGE
	built.box(0.0, 1.0, sink, [T, -0.16], side(-1.0, 0.005), side(1.0, 0.005), Role.HEDGE, [2, 3])
	built.variant = Variant.HEDGE_FLAT
	_hedge_top(built, [[0.0, 0.0], [0.0, 0.12], [0.04, 0.16]], 0.16)
	built.variant = Variant.HEDGE_ROUND
	var arc := []
	for k in 4:
		var phi := PI * 0.5 * float(k) / 3.0
		arc.append([0.16 * (1.0 - cos(phi)), 0.16 * sin(phi)])
	_hedge_top(built, arc, 0.16)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_solid"
	return mesh

## A hedge's upper part from `drop` below the top: `profile` lists (inset
## from the side, rise from -drop) pairs from the side up to the crown,
## mirrored on both sides, with a flat crown between and closed ends.
static func _hedge_top(built: Builder, profile: Array, drop: float) -> void:
	var rows := []
	for s in [-1.0, 1.0]:
		var row := []
		for p: Array in profile: row.append(Builder.at(0.0, [T, -drop + float(p[1])], side(s, 0.005 + float(p[0]))))
		rows.append(row)
	for r in 2:
		var s := -1.0 if r == 0 else 1.0
		var row: Array = rows[r]
		for k in row.size() - 1:
			var a: Array = row[k]
			var b: Array = row[k + 1]
			var inward := float(profile[k + 1][0]) - float(profile[k][0])
			var rise := float(profile[k + 1][1]) - float(profile[k][1])
			built.quad([a, Builder.at(1.0, a[1], a[2]), Builder.at(1.0, b[1], b[2]), b], Vector3(0.0, inward, s * rise).normalized(), Role.HEDGE)
	var left: Array = rows[0][-1]
	var right: Array = rows[1][-1]
	built.quad([left, Builder.at(1.0, left[1], left[2]), Builder.at(1.0, right[1], right[2]), right], Vector3.UP, Role.HEDGE)
	for x in [0.0, 1.0]:
		var outline := []
		for corner: Array in rows[0]: outline.append(Builder.at(x, corner[1], corner[2]))
		var back: Array = rows[1].duplicate()
		back.reverse()
		for corner: Array in back: outline.append(Builder.at(x, corner[1], corner[2]))
		var base := Builder.at(x, [T, -drop], zc(0.0))
		for k in outline.size() - 1:
			built.tri([base, outline[k], outline[k + 1]], Vector3(-1.0 if x == 0.0 else 1.0, 0.0, 0.0), Role.HEDGE, 0.85)

## Fence posts per family (real metres along and across; caps at the top)
## and wall piers: brick piers and block pilasters every bay, precast
## H-posts, end, gate and corner piers (full width; their size factor
## scales them along).
static func post_mesh(material: Material) -> ArrayMesh:
	var built := Builder.new()
	built.post = true
	var sink := [G, -FENCE_SINK]
	built.variant = Variant.STEEL_BAR
	built.box(-0.03, 0.03, sink, [T, -0.02], zc(-0.03), zc(0.03), Role.POST, [2, 3])
	built.box(-0.035, 0.035, [T, -0.02], [T, 0.0], zc(-0.035), zc(0.035), Role.CAP, [], 0.6)
	built.variant = Variant.WELDED_MESH
	built.box(-0.03, 0.03, sink, [T, -0.015], zc(-0.02), zc(0.02), Role.POST, [2, 3])
	built.box(-0.035, 0.035, [T, -0.015], [T, 0.0], zc(-0.025), zc(0.025), Role.CAP, [], 0.6)
	built.variant = Variant.CHAIN_LINK
	built.cylinder_y(0.03, sink, [T, -0.032], 8, Role.POST, 0.03, Role.CAP)
	built.variant = Variant.WOODEN
	built.box(-0.045, 0.045, sink, [T, -0.03], zc(-0.045), zc(0.045), Role.POST, [2, 3])
	# Chamfered top: four slopes to a ridge point.
	var apex := Builder.at(0.0, [T, 0.0], zc(0.0))
	var corners := [Builder.at(-0.045, [T, -0.03], zc(-0.045)), Builder.at(0.045, [T, -0.03], zc(-0.045)), Builder.at(0.045, [T, -0.03], zc(0.045)), Builder.at(-0.045, [T, -0.03], zc(0.045))]
	var faces := [Vector3(0.0, 0.045, -0.03), Vector3(0.03, 0.045, 0.0), Vector3(0.0, 0.045, 0.03), Vector3(-0.03, 0.045, 0.0)]
	for k in 4: built.tri([corners[k], corners[(k + 1) % 4], apex], faces[k].normalized(), Role.CAP)
	built.variant = Variant.PIPE_RAIL
	built.cylinder_y(0.024, sink, [T, -0.026], 6, Role.POST, 0.024, Role.CAP)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_post"
	return mesh

static func pier_mesh(material: Material) -> ArrayMesh:
	var built := Builder.new()
	built.post = true
	built.nominal = Vector3(1.0, 2.0, 0.27)
	var sink := [G, -SOLID_SINK]
	built.variant = Variant.BRICK_PIER
	built.box(-0.2, 0.2, sink, [T, -0.06], side(-1.0, 0.002), side(1.0, 0.002), Role.PIER, [2, 3])
	built.variant = Variant.BLOCK_PIER
	built.box(-0.2, 0.2, sink, [T, -0.06], side(-1.0, 0.012), side(1.0, 0.012), Role.PIER, [2, 3])
	built.variant = Variant.PRECAST_POST
	built.box(-0.075, 0.075, sink, [T, -0.06], side(-1.0, 0.002), side(1.0, 0.002), Role.PIER, [2, 3])
	built.variant = Variant.END_PIER
	built.box(-0.2, 0.2, sink, [T, -0.004], side(-1.0, 0.003), side(1.0, 0.003), Role.PIER, [2], 0.7)
	built.variant = Variant.GIWA_END_PIER
	built.box(-0.2, 0.2, sink, [T, -0.13], side(-1.0, 0.003), side(1.0, 0.003), Role.PIER, [2], 0.7)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_pier"
	return mesh

## Far fence faces (blended; posts, rails and infill painted) and far solid
## boxes (walls and hedges; piers, plinth and coping painted).
static func fence_far(material: Material) -> ArrayMesh:
	var built := Builder.new()
	for s in [-1.0, 1.0]:
		built.quad([Builder.at(0.0, [G, 0.0], zc(0.004 * s)), Builder.at(1.0, [G, 0.0], zc(0.004 * s)), Builder.at(1.0, [T, 0.0], zc(0.004 * s)),
			Builder.at(0.0, [T, 0.0], zc(0.004 * s))], Vector3(0.0, 0.0, s), Role.FAR_FENCE)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_fence_far"
	return mesh

static func solid_far(material: Material) -> ArrayMesh:
	var built := Builder.new()
	built.nominal = Vector3(2.0, 2.0, 0.27)
	built.box(0.0, 1.0, [G, -SOLID_SINK], [T, -0.002], side(-1.0, 0.003), side(1.0, 0.003), Role.FAR_SOLID, [2], 0.85)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_barrier_solid_far"
	return mesh

## One of the three materials from barrier_detail.gdshader: "opaque",
## "infill" (dithered alpha test, near only) or "far" (alpha blended, never
## discarded). The palette, bays and far opacities come from
## barrier_palette.json, the weather from the environment context.
static func material(context: RefCounted = null, mode := "opaque") -> ShaderMaterial:
	var shader := Shader.new()
	var code: String = preload("./barrier_detail.gdshader").code
	if mode == "infill": code = code.replace("render_mode cull_back;", "render_mode cull_back;\n#define BARRIER_INFILL")
	elif mode == "far": code = code.replace("render_mode cull_back;", "render_mode cull_back, blend_mix, depth_draw_never;\n#define BARRIER_FAR")
	shader.code = code
	var made := ShaderMaterial.new()
	made.shader = shader
	if context != null and "texture" in context:
		made.set_shader_parameter("environment_data", context.texture)
		made.set_shader_parameter("environment_enabled", true)
	var palette := PackedVector3Array()
	var table: Dictionary = PALETTE.data
	for group: String in ["fence", "wall", "hedge"]:
		var families: Array = table[group]
		for slots: Array in families:
			for k in 4:
				var rgb: Array = slots[mini(k, slots.size() - 1)]
				palette.append(Vector3(float(rgb[0]), float(rgb[1]), float(rgb[2])) / 255.0)
	while palette.size() < 40: palette.append(palette[palette.size() - 1])
	made.set_shader_parameter("barrier_palette", palette)
	made.set_shader_parameter("fence_bay", PackedFloat32Array(table.bay_m.fence))
	made.set_shader_parameter("wall_bay", PackedFloat32Array(table.bay_m.wall))
	made.set_shader_parameter("fence_far_opacity", PackedFloat32Array(table.far_opacity))
	made.set_shader_parameter("board_far_opacity", float(table.board_far_opacity))
	made.set_meta("mapkit_detail", true)
	made.set_meta("mapkit_opaque", mode != "far")
	made.resource_name = "mk_barrier_" + mode
	QUALITY.apply_material(made, QUALITY.active())
	return made

## One part per call (a material or a mesh); true when the whole set is in
## state.result. `lease` (track(Object)) receives every created resource.
## The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {}
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "barrier_" + step
	var result: Dictionary = state.result
	var made: Resource
	match step:
		"material": made = material(context, "opaque")
		"infill_material": made = material(context, "infill")
		"far_material": made = material(context, "far")
		"fence": made = fence_run(result.material)
		"solid": made = solid_run(result.material)
		"infill": made = infill(result.infill_material)
		"post": made = post_mesh(result.material)
		"pier": made = pier_mesh(result.material)
		"fence_far": made = fence_far(result.far_material)
		_: made = solid_far(result.material)
	result[step] = made
	if lease != null:
		lease.track(made)
		if made is ShaderMaterial: lease.track(made.shader)
	state.step += 1
	if state.step < STEPS.size(): return false
	result.make_read_only()
	return true

## Process-lifetime set for cells without a resource cache (standalone
## scenes; the Editor's previews claim its cached set), one part per call;
## the set once complete, else {}.
static func step_shared() -> Dictionary:
	if not _shared.is_empty(): return _shared
	if build(_shared_job):
		_shared = _shared_job.result
		_shared_job = {}
	return _shared

## The cell's packed lists (presentation barrier_runs / barrier_posts /
## barrier_ids), empty when the presentation has none.
static func runs(chunk: Dictionary) -> PackedFloat32Array:
	var list: Variant = chunk.get("presentation", {}).get("barrier_runs", PackedFloat32Array())
	return list if list is PackedFloat32Array else PackedFloat32Array()
static func posts(chunk: Dictionary) -> PackedFloat32Array:
	var list: Variant = chunk.get("presentation", {}).get("barrier_posts", PackedFloat32Array())
	return list if list is PackedFloat32Array else PackedFloat32Array()
static func ids(chunk: Dictionary) -> PackedStringArray:
	var list: Variant = chunk.get("presentation", {}).get("barrier_ids", PackedStringArray())
	return list if list is PackedStringArray else PackedStringArray()

## Claims the shared set as a cell with barrier detail begins
## (chunk_renderer.begin, before any upload) when it has a resource cache;
## false when refused (the cell fails: its prism faces are hidden).
static func claim(job: Dictionary) -> bool:
	if job.get("resources") == null or job.claims.has(KEY) or runs(job.chunk).is_empty(): return true
	var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
	if key.is_empty(): return false
	job.claims[KEY] = key
	return true

## Shared set for a renderer job: 1 ready, 0 another step is needed, -1
## unavailable.
static func ready(job: Dictionary) -> int:
	if job.has("barrier_set"): return 1
	job.last_phase = "barrier_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.barrier_set = made
		return 0
	if not job.claims.has(KEY): return -1
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -1
		job.barrier_set = made
	return 0

## Whether a fence family draws see-through infill (steel bars, welded mesh,
## chain-link, wooden pickets).
static func has_infill(bits: int) -> bool:
	return keep(Variant.INFILL, bits, 0, 0.0)

## Instance groups of the packed lists: "<role>/<square x>,<square z>" ->
## {role, buffer (12 transform and 4 custom floats per instance), ids (the
## barrier ID per instance), faces (editor picking: each run's top, when
## `pick_faces`), bounds ({"box": AABB}, one dictionary shared by every
## group of the square: the union of their instances' boxes)}. Runs join
## their fence or solid near group, the fence infill group and their far
## group by the run's midpoint; posts and piers join the square of the run
## of their barrier they stand on (else their foot's), so the far mesh that
## paints them switches with them. `low` plans the far groups only, one per
## role for the whole cell ("<role>/cell").
static func plan(chunk: Dictionary, pick_faces := false, low := false) -> Dictionary:
	var groups := {}
	plan_part(chunk, groups, 0, entries(chunk), pick_faces, low, {})
	return groups

## Runs, then posts: the entries plan_part walks.
static func entries(chunk: Dictionary) -> int:
	return runs(chunk).size() / RUN_FLOATS + posts(chunk).size() / POST_FLOATS

## Plans entries first..last (runs, then posts) into `groups`, so a renderer
## step plans a bounded part (PLAN_STEP entries; the busiest real-city
## cells list about 500 runs and 850 posts).
const PLAN_STEP := 256
## `owners` carries the planned runs across calls (RUN_GRID metre cells ->
## [barrier ID, origin, ground vector, square]) for the posts' squares.
const RUN_GRID := 4.0
static func plan_part(chunk: Dictionary, groups: Dictionary, first: int, last: int, pick_faces := false, low := false, owners := {}) -> void:
	var names := ids(chunk)
	var list := runs(chunk)
	var standing := posts(chunk)
	var run_count := list.size() / RUN_FLOATS
	# `span` scales x for the instance box: runs span 0..1, posts real metres about 0.
	var square_of := func(at: Vector3) -> String: return "cell" if low else "%d,%d" % [floori(at.x / SQUARE_METRES), floori(at.z / SQUARE_METRES)]
	var add := func(role: String, origin: Vector3, b: Basis, custom: Color, id: String, square: String, span: Vector2) -> Dictionary:
		var key := "%s/%s" % [role, square]
		if not groups.has(key):
			var bounds := {}
			for other: String in NEAR_ROLES + FAR_ROLES:
				if groups.has(other + "/" + square):
					bounds = groups[other + "/" + square].bounds
					break
			groups[key] = {"role": role, "buffer": PackedFloat32Array(), "ids": PackedStringArray(), "faces": PackedVector3Array(), "bounds": bounds}
		var group: Dictionary = groups[key]
		group.buffer.append_array(PackedFloat32Array([b.x.x, b.y.x, b.z.x, origin.x, b.x.y, b.y.y, b.z.y, origin.y, b.x.z, b.y.z, b.z.z, origin.z,
			custom.r, custom.g, custom.b, custom.a]))
		group.ids.append(id)
		var box := AABB(origin + b.x * span.x - b.z * 0.5, Vector3.ZERO)
		for corner in [origin + b.x * span.y - b.z * 0.5, origin + b.x * span.x + b.z * 0.5, origin + b.x * span.y + b.z * 0.5]: box = box.expand(corner)
		box = box.merge(AABB(box.position + Vector3(0.0, -SOLID_SINK, 0.0), box.size + Vector3(0.0, SOLID_SINK + b.y.y * 1.2, 0.0)))
		group.bounds.box = box if not group.bounds.has("box") else (group.bounds.box as AABB).merge(box)
		return group
	for entry in range(first, mini(last, entries(chunk))):
		if entry < run_count:
			var i := entry * RUN_FLOATS
			var origin := Vector3(list[i], list[i + 1], list[i + 2])
			var x := Vector3(list[i + 3], list[i + 4], list[i + 5])
			var b := Basis(x, Vector3(0.0, list[i + 8], 0.0), Vector3(list[i + 6], 0.0, list[i + 7]))
			var bits := int(list[i + 9])
			var custom := Color(float(bits & 255), float((bits >> 8) & 255), list[i + 10], list[i + 11])
			var index := int(list[i + 12])
			var id: String = names[index] if index >= 0 and index < names.size() else ""
			var mid := origin + x * 0.5
			var fence := (bits & 3) == 1
			var roles := ["fence", "fence_far"] if fence else ["solid", "solid_far"]
			if fence and has_infill(bits): roles.append("infill")
			# Wall corner runs draw far only unless their giwa cap needs them near.
			if int(list[i + 11]) & RUN_CORNER != 0 and (bits & 3) == 2 and not keep(Variant.GIWA, bits, 0, 0.0): roles = ["solid_far"]
			if low: roles = roles.filter(func(role: String) -> bool: return role.ends_with("_far"))
			var square: String = square_of.call(mid)
			if not low:
				var lo := Vector2(minf(origin.x, origin.x + x.x), minf(origin.z, origin.z + x.z))
				var hi := Vector2(maxf(origin.x, origin.x + x.x), maxf(origin.z, origin.z + x.z))
				for gx in range(floori(lo.x / RUN_GRID), floori(hi.x / RUN_GRID) + 1):
					for gz in range(floori(lo.y / RUN_GRID), floori(hi.y / RUN_GRID) + 1):
						var cell := Vector2i(gx, gz)
						if not owners.has(cell): owners[cell] = []
						owners[cell].append([id, origin, x, square])
			for role: String in roles:
				var group: Dictionary = add.call(role, origin, b, custom, id, square, Vector2(0.0, 1.0))
				if pick_faces and role.ends_with("_far"):
					var a := origin + Vector3(0.0, list[i + 8], 0.0) - b.z * 0.5
					group.faces.append_array([a, a + x, a + x + b.z, a, a + x + b.z, a + b.z])
		elif not low:
			var i := (entry - run_count) * POST_FLOATS
			var origin := Vector3(standing[i], standing[i + 1], standing[i + 2])
			var b := Basis(Vector3(standing[i + 3], standing[i + 4], standing[i + 5]), Vector3(0.0, standing[i + 8], 0.0), Vector3(standing[i + 6], 0.0, standing[i + 7]))
			var bits := int(standing[i + 9])
			var custom := Color(float(bits & 255), float((bits >> 8) & 255), standing[i + 10], standing[i + 11])
			var index := int(standing[i + 12])
			var id: String = names[index] if index >= 0 and index < names.size() else ""
			# The square of the nearest run of the same barrier under the foot.
			var square: String = square_of.call(origin)
			var best := 0.05
			var home := Vector2i(floori(origin.x / RUN_GRID), floori(origin.z / RUN_GRID))
			for dx in [-1, 0, 1]:
				for dz in [-1, 0, 1]:
					for run: Array in owners.get(home + Vector2i(dx, dz), []):
						if run[0] != id: continue
						var o: Vector3 = run[1]
						var v: Vector3 = run[2]
						var flat := Vector2(v.x, v.z)
						var t := clampf(Vector2(origin.x - o.x, origin.z - o.z).dot(flat) / maxf(flat.length_squared(), 1e-9), 0.0, 1.0)
						var gap := Vector2(origin.x - o.x - v.x * t, origin.z - o.z - v.z * t).length()
						if gap < best:
							best = gap
							square = run[3]
			add.call("post" if (bits & 3) == 1 else "pier", origin, b, custom, id, square, Vector2(-0.6, 0.6))

## One planned group's MultiMesh, its custom AABB the square's shared
## bounds (every role measured to the same centre). The cell lease carries
## the node and its instance buffer; meshes and materials belong to the
## shared set.
static func attach_group(job: Dictionary) -> void:
	var key: String = job.barrier_plan.keys()[0]
	var group: Dictionary = job.barrier_plan[key]
	job.barrier_plan.erase(key)
	var multi := MultiMesh.new()
	multi.transform_format = MultiMesh.TRANSFORM_3D
	multi.use_custom_data = true
	multi.mesh = job.barrier_set[group.role]
	multi.instance_count = group.buffer.size() / 16
	multi.buffer = group.buffer
	if group.get("bounds", {}).has("box"): multi.custom_aabb = group.bounds.box
	var node := MultiMeshInstance3D.new()
	node.name = "Barrier_%s_%d" % [group.role, job.root.get_child_count()]
	node.multimesh = multi
	# Parts below the ground and real-size offsets reach past the unit mesh bounds.
	node.extra_cull_margin = 0.5
	node.set_meta("mapkit_static_detail", true)
	# Barriers replace solid, colliding faces (like streetlights and signals).
	node.set_meta("mapkit_decoration", false)
	node.set_meta("mapkit_barrier_lod", "far" if group.role.ends_with("_far") else "near")
	node.set_meta("mapkit_barrier_role", group.role)
	if str(key).ends_with("/cell"): node.set_meta("mapkit_barrier_low_plan", true)
	node.set_meta("mapkit_object_ids", group.ids)
	if not (group.faces as PackedVector3Array).is_empty(): node.set_meta("mapkit_pick_faces", group.faces)
	QUALITY.apply_node(node, QUALITY.active())
	if job.get("lease") != null:
		job.lease.track(node)
		job.lease.track(multi)
	job.root.add_child(node)

## One renderer step after the cell's objects: prepare the shared set, plan
## PLAN_STEP entries per call, then attach one group per call. Returns whether it did work;
## job.barriers_failed when the shared set is unavailable (the renderer
## fails the cell: the prism faces are hidden).
static func step(job: Dictionary) -> bool:
	if job.get("barriers_done", false): return false
	if runs(job.chunk).is_empty():
		job.barriers_done = true
		return false
	if not job.has("barrier_set"):
		if ready(job) < 0:
			job.barriers_failed = true
			job.barriers_done = true
		return true
	job.last_phase = "instance_upload"
	if not job.has("barrier_plan"):
		job.barrier_plan = {}
		job.barrier_cursor = 0
		# The tier when planning starts decides the plan for the whole cell.
		job.barrier_low = int(QUALITY.active().get("level", 1)) == 0
	if int(job.barrier_cursor) < entries(job.chunk):
		# Editor and standalone previews (no lease) keep run tops for picking.
		if not job.has("barrier_owners"): job.barrier_owners = {}
		plan_part(job.chunk, job.barrier_plan, int(job.barrier_cursor), int(job.barrier_cursor) + PLAN_STEP, job.get("lease") == null, bool(job.barrier_low), job.barrier_owners)
		job.barrier_cursor = int(job.barrier_cursor) + PLAN_STEP
		return true
	if not job.barrier_plan.is_empty():
		attach_group(job)
		return true
	job.barriers_done = true
	return false
