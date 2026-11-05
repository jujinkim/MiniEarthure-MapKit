extends RefCounted
## Display-only facade fixtures on generated building walls, built
## procedurally (no external assets, no real brand): projecting blade signs
## (vertical lightboxes standing perpendicular to commercial walls on two
## brackets, both broad faces drawn by facade_signs.gdshaderinc in vertical
## mode) and wall-mounted air-conditioner outdoor units (a box with a fan
## grille, side louvres and insulated pipes on a steel rack) under
## upper-storey windows of house, commercial and public walls; and rooftop
## fixtures on flat roofs (mapkit-godot roof_fixtures.rs): stair penthouses
## with a steel door, a canopy and a door lamp, water tanks (a square FRP
## panel tank on a steel stand or a round stainless one), condenser units
## (side-blow or top-blow), parapet rims along the roof edges (with a guard
## railing on part of the roofs), solar panel rows and antenna masts or
## satellite dishes.
##
## Native placement (mapkit-godot facade_fixtures.rs, at presentation) lists
## them in presentation `facade_fixtures`, FLOATS per fixture: the mount point
## (scene metres: on the wall at a wall fixture's bottom centre, on the roof
## at a rooftop fixture's footprint centre or a rim's or solar row's left
## end), yaw (local +z faces out of the wall, out of a penthouse's door side,
## inward from a rim; local +x runs right along it seen from +z), kind (KINDS
## index), scale (the floor pitch in metres), height, 24-bit design bits,
## width and depth (metres; 0 on wall fixtures). Sizes below are in floor
## pitches (metres at 1:3) and match facade_fixtures.rs and roof_fixtures.rs,
## which keep every fixture box (plus a margin) clear of the cell's other
## triangles and inside the cell. Meshes are built at scale 1; instances
## scale them by kind (plan()): a blade sign's height to theirs, penthouses
## and tanks (unit boxes) to their width, height and depth, rims and solar
## rows (unit length along x) to their width; the rest by the floor pitch.
##
## One shared material (facade_fixture.gdshader): surface roles in UV.x, a
## baked shade in UV.y (plus 2 x the mesh variant a part belongs to: 0 every
## variant, n the design's variant n - 1; the vertex shader collapses parts
## of other variants) and face coordinates in UV2 (anchored rail posts and
## legs: their anchor instead, see the shader). Instance custom data carries
## the design bits (three 8-bit channels, exact on every renderer) and the
## blade's height:width aspect, or the floor pitch on rooftop fixtures, whose
## bits also name their building's palette entries (roof_fixtures.rs
## `design`); the material binds the record palettes (facade_palette.json).
## Blade faces glow at night by the storefront signs' rules (lit all night,
## x 0.55, fading with the surface effect distance like the wall signs), most
## penthouse door lamps faintly; weather wets and snows them like walls.
##
## A cell instances each kind per SQUARE_METRES square as near and far
## MultiMeshes (display_quality.apply_fixture: blade signs, penthouses and
## tanks on every tier, the simple far mesh only on low; the other kinds from
## medium). build() makes one part per call for admitted streaming steps
## (render_resource_cache.prepare_mesh_set); step_shared() builds a
## process-lifetime set for cells without a resource cache.

const QUALITY := preload("./display_quality.gd")
const KEY := "builtin:facade_fixtures"
const PALETTE := preload("./facade_palette.json")
const KINDS := ["blade", "ac", "penthouse", "tank", "condenser", "edge", "solar", "mast"]
const LODS := ["near", "far"]
const FLOATS := 10
const SQUARE_METRES := 32.0
## Kinds drawn on low (far meshes only) and kinds whose groups end at the
## full decoration distance (display_quality.apply_fixture).
const LOW_KINDS := ["blade", "penthouse", "tank"]
## Palette keys of rooftop design bits 0..2 (roof_fixtures.rs palette_key).
const PALETTE_KEYS := ["concrete:residential", "concrete:commercial", "concrete:industrial", "concrete:public", "brick", "wood"]
## Blade sign (floor pitches): WIDTH out of the wall from a STANDOFF,
## THICKNESS along the wall; the mesh is BLADE_HEIGHT tall.
const BLADE_WIDTH := 0.2
const BLADE_THICKNESS := 0.06
const BLADE_STANDOFF := 0.04
const BLADE_HEIGHT := 1.1
## Air-conditioner unit and its rack below it (floor pitches).
const AC_WIDTH := 0.27
const AC_HEIGHT := 0.2
const AC_DEPTH := 0.1
const AC_STANDOFF := 0.03
const AC_RACK := 0.03
## Rooftop fixtures (floor pitches, roof_fixtures.rs). Penthouse and tank
## meshes are unit boxes (x, z -0.5..0.5, y 0..1) scaled to their size; rims
## and solar rows run x 0..1 along their length.
const RIM_INSET := 0.015
const RIM_THICKNESS := 0.06
const RIM_HEIGHT := 0.12
const RAIL_HEIGHT := 0.34
const RAIL_POSTS := 32
const CONDENSER_WIDTH := 0.3
const CONDENSER_DEPTH := 0.22
const CONDENSER_HEIGHT := 0.3
const SOLAR_DEPTH := 0.5
const SOLAR_HEIGHT := 0.36
const MAST_FOOTPRINT := 0.3
const ANTENNA_HEIGHT := 0.85
const DISH_HEIGHT := 0.42
## Surface roles (UV.x) of facade_fixture.gdshader.
enum Role { HOUSING, FACE, STEEL, BODY, GRILLE, LOUVRE, PIPE, WALL, CAP, TOP, DOOR, LAMP, FRP, STAINLESS, SOLAR, RAIL, POST, LEG, DISH, FAN }
const STEPS := ["material", "blade_near", "blade_far", "ac_near", "ac_far", "penthouse_near", "penthouse_far", "tank_near", "tank_far",
	"condenser_near", "condenser_far", "edge_near", "edge_far", "solar_near", "solar_far", "mast_near", "mast_far"]
## Memory allowance of one shared set (render_resource_cache): about 3,700
## vertices at <= 56 B and 5,500 indices at 4 B in CPU arrays and GPU
## buffers (0.46 MB, facade_fixture_validator measures them), 64 KiB for the
## material, its shader and one mesh's build arrays.
const SHARED_BYTES := 640 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

class Builder:
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var uv := PackedVector2Array()
	var uv2 := PackedVector2Array()
	var indices := PackedInt32Array()
	## Parts added while set belong to mesh variant `variant` - 1 (0: every
	## variant); anchored parts (anchor >= 0, rail posts and legs) carry it
	## in UV2.x instead of face coordinates.
	var variant := 0
	var anchor := -1.0
	## Quad a-b-c-d (any winding) facing `normal`, with face coordinates.
	func quad(points: Array, normal: Vector3, role: int, shade: float, face: Array = []) -> void:
		quad_normals(points, [normal, normal, normal, normal], role, shade, face)
	## Quad with per-corner normals (smooth cylinders); winds by their mean.
	func quad_normals(points: Array, corner_normals: Array, role: int, shade: float, face: Array = []) -> void:
		var first := vertices.size()
		var mean := Vector3.ZERO
		for k in 4:
			vertices.append(points[k]); normals.append(corner_normals[k]); mean += corner_normals[k]
			uv.append(Vector2(role, shade + 2.0 * variant))
			uv2.append(Vector2(anchor, 0.0) if anchor >= 0.0 else (face[k] if k < face.size() else Vector2.ZERO))
		# Godot front faces wind clockwise seen from their normal side.
		var facing := (vertices[first + 1] - vertices[first]).cross(vertices[first + 2] - vertices[first])
		if facing.dot(mean) > 0.0:
			indices.append_array([first, first + 2, first + 1, first, first + 3, first + 2])
		else:
			indices.append_array([first, first + 1, first + 2, first, first + 2, first + 3])
	## Triangle a-b-c (any winding) facing `normal`.
	func tri(points: Array, normal: Vector3, role: int, shade: float, face: Array = []) -> void:
		var first := vertices.size()
		for k in 3:
			vertices.append(points[k]); normals.append(normal)
			uv.append(Vector2(role, shade + 2.0 * variant))
			uv2.append(face[k] if k < face.size() else Vector2.ZERO)
		var facing := (vertices[first + 1] - vertices[first]).cross(vertices[first + 2] - vertices[first])
		if facing.dot(normal) > 0.0: indices.append_array([first, first + 2, first + 1])
		else: indices.append_array([first, first + 1, first + 2])
	## Vertical cylinder (smooth sides) about (cx, cz) from y0 to y1, its top
	## capped with `top_role` (-1 none) and the face coordinates of the cap in
	## -1..1.
	func cylinder(cx: float, cz: float, radius: float, y0: float, y1: float, segments: int, role: int, top_role := -1, shade := 1.0) -> void:
		for k in segments:
			var a0 := TAU * float(k) / float(segments)
			var a1 := TAU * float(k + 1) / float(segments)
			var n0 := Vector3(cos(a0), 0.0, sin(a0))
			var n1 := Vector3(cos(a1), 0.0, sin(a1))
			var p0 := Vector3(cx, 0.0, cz) + n0 * radius
			var p1 := Vector3(cx, 0.0, cz) + n1 * radius
			var u0 := float(k) / float(segments)
			var u1 := float(k + 1) / float(segments)
			quad_normals([Vector3(p0.x, y0, p0.z), Vector3(p1.x, y0, p1.z), Vector3(p1.x, y1, p1.z), Vector3(p0.x, y1, p0.z)], [n0, n1, n1, n0], role, shade,
				[Vector2(u0, 1), Vector2(u1, 1), Vector2(u1, 0), Vector2(u0, 0)])
			if top_role >= 0:
				tri([Vector3(cx, y1, cz), Vector3(p0.x, y1, p0.z), Vector3(p1.x, y1, p1.z)], Vector3.UP, top_role, 1.0, [Vector2.ZERO, Vector2(cos(a0), sin(a0)), Vector2(cos(a1), sin(a1))])
	## Box from `lo` to `hi` turned by `basis` about `origin`; `skip` lists
	## faces left out (0..5: -x, +x, -y, +y, -z, +z). Faces carry 0..1 face
	## coordinates (u across, v down); the bottom is darker by `bottom`.
	func box(lo: Vector3, hi: Vector3, role: int, bottom := 0.75, skip: Array = [], basis := Basis(), origin := Vector3.ZERO) -> void:
		for face in 6:
			if face in skip: continue
			var axis := face / 2
			var high := face % 2 == 1
			var normal := Vector3.ZERO
			normal[axis] = 1.0 if high else -1.0
			var a := (axis + 1) % 3
			var b := (axis + 2) % 3
			var corners := []
			for k in 4:
				var p := Vector3.ZERO
				p[axis] = hi[axis] if high else lo[axis]
				p[a] = hi[a] if k == 1 or k == 2 else lo[a]
				p[b] = hi[b] if k >= 2 else lo[b]
				corners.append(origin + basis * p)
			var shade := 1.0 if axis != 1 else (1.0 if high else bottom)
			quad(corners, (basis * normal).normalized(), role, shade, [Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	## Square bar from `a` to `b` of half width `half`, turned so its sides face
	## `side` (perpendicular to the bar).
	func bar(a: Vector3, b: Vector3, half: float, role: int, side := Vector3.UP) -> void:
		var along := (b - a).normalized()
		var x := (side - along * side.dot(along)).normalized()
		var basis := Basis(x, along, x.cross(along))
		box(Vector3(-half, 0.0, -half), Vector3(half, (b - a).length(), half), role, 0.85, [], basis, a)
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

## Blade sign (local metres at scale 1): the lightbox from STANDOFF to
## STANDOFF + WIDTH out of the wall, its broad faces (sign faces, face
## coordinates across from the viewer's left and down from the top on both
## sides, so neither reads mirrored) at +-THICKNESS/2. Near: a raised rim
## around both faces, a top cap, two bracket arms with wall plates. Far: the
## box and one bracket web.
static func blade(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var t := BLADE_THICKNESS * 0.5
	var z0 := BLADE_STANDOFF
	var z1 := BLADE_STANDOFF + BLADE_WIDTH
	var h := BLADE_HEIGHT
	# Narrow faces of the housing, then both sign faces.
	built.box(Vector3(-t, 0.0, z0), Vector3(t, h, z1), Role.HOUSING, 0.7, [0, 1])
	built.quad([Vector3(t, 0.0, z1), Vector3(t, 0.0, z0), Vector3(t, h, z0), Vector3(t, h, z1)], Vector3.RIGHT, Role.FACE, 1.0,
		[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	built.quad([Vector3(-t, 0.0, z0), Vector3(-t, 0.0, z1), Vector3(-t, h, z1), Vector3(-t, h, z0)], Vector3.LEFT, Role.FACE, 1.0,
		[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	if lod == "near":
		var rim := 0.012
		for s in [-1.0, 1.0]:
			var x0: float = s * (t - 0.004)
			var x1: float = s * (t + 0.004)
			var lo_x := minf(x0, x1)
			var hi_x := maxf(x0, x1)
			built.box(Vector3(lo_x, h - rim, z0), Vector3(hi_x, h, z1), Role.HOUSING)
			built.box(Vector3(lo_x, 0.0, z0), Vector3(hi_x, rim, z1), Role.HOUSING)
			built.box(Vector3(lo_x, rim, z1 - rim), Vector3(hi_x, h - rim, z1), Role.HOUSING)
			built.box(Vector3(lo_x, rim, z0), Vector3(hi_x, h - rim, z0 + rim), Role.HOUSING)
		built.box(Vector3(-t - 0.006, h, z0 - 0.004), Vector3(t + 0.006, h + 0.012, z1 + 0.004), Role.HOUSING, 0.6)
		for y in [0.14 * h, 0.86 * h]:
			built.box(Vector3(-0.008, y - 0.008, 0.004), Vector3(0.008, y + 0.008, z0 + 0.03), Role.STEEL)
			built.box(Vector3(-0.025, y - 0.035, 0.0), Vector3(0.025, y + 0.035, 0.006), Role.STEEL)
	else:
		built.box(Vector3(-0.006, 0.1 * h, 0.0), Vector3(0.006, 0.9 * h, z0 + 0.01), Role.STEEL)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_blade_" + lod
	return mesh

## Air-conditioner outdoor unit (local metres at scale 1): the body from
## STANDOFF to STANDOFF + DEPTH out of the wall, its front a fan grille on
## the left and plain body on the right, louvres on its right side. Near: an
## L-bracket rack under it (arms, struts and wall plates within AC_RACK) and
## insulated pipes from its right side into the wall. Far: the body and a
## rack bar.
static func air_conditioner(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var w := AC_WIDTH * 0.5
	var h := AC_HEIGHT
	var z0 := AC_STANDOFF
	var z1 := AC_STANDOFF + AC_DEPTH
	built.box(Vector3(-w, 0.0, z0), Vector3(w, h, z1), Role.BODY, 0.7, [1, 5])
	built.quad([Vector3(w, 0.0, z1), Vector3(w, 0.0, z0), Vector3(w, h, z0), Vector3(w, h, z1)], Vector3.RIGHT, Role.LOUVRE, 0.9,
		[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	# Front: the grille square (face coordinates -1..1) framed by body strips.
	var g0 := -w + 0.02
	var g1 := g0 + 0.17
	var gy0 := 0.015
	var gy1 := gy0 + 0.17
	var front := func(x0: float, y0: float, x1: float, y1: float, role: int, face: Array) -> void:
		built.quad([Vector3(x0, y0, z1), Vector3(x1, y0, z1), Vector3(x1, y1, z1), Vector3(x0, y1, z1)], Vector3.BACK, role, 1.0, face)
	front.call(g0, gy0, g1, gy1, Role.GRILLE, [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)])
	front.call(-w, 0.0, g0, h, Role.BODY, [])
	front.call(g1, 0.0, w, h, Role.BODY, [])
	front.call(g0, 0.0, g1, gy0, Role.BODY, [])
	front.call(g0, gy1, g1, h, Role.BODY, [])
	if lod == "near":
		for x in [-0.095, 0.095]:
			built.box(Vector3(x - 0.007, -0.014, 0.004), Vector3(x + 0.007, 0.0, z1 + 0.008), Role.STEEL)
			built.bar(Vector3(x, -AC_RACK + 0.004, 0.006), Vector3(x, -0.012, z1 - 0.02), 0.005, Role.STEEL, Vector3.RIGHT)
			built.box(Vector3(x - 0.012, -AC_RACK, 0.0), Vector3(x + 0.012, 0.004, 0.005), Role.STEEL)
		# Two insulated pipes out of the right side, turning into the wall.
		for k in 2:
			var y := 0.045 + 0.03 * float(k)
			var r := 0.007 - 0.002 * float(k)
			var x1 := w + 0.016 + 0.008 * float(k)
			built.bar(Vector3(w - 0.004, y, z0 + 0.06), Vector3(x1 + r, y, z0 + 0.06), r, Role.PIPE)
			built.bar(Vector3(x1, y, z0 + 0.06 + r), Vector3(x1, y, 0.002), r, Role.PIPE)
	else:
		built.box(Vector3(-w + 0.03, -0.014, 0.0), Vector3(w - 0.03, 0.0, z1), Role.STEEL)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_ac_" + lod
	return mesh

## Stair penthouse (unit box, door side +z): plastered walls in the building's
## body colour, an overhanging slab whose upper face takes the roof coating,
## a steel door. Near: door jambs and head, a canopy with the door lamp, a
## louvred vent and a vent pipe on the slab. Far: walls, slab and door.
static func penthouse(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var x0 := -0.46
	var x1 := 0.46
	var z0 := -0.44
	var z1 := 0.36
	var wall := 0.9
	var slab := 0.96
	built.box(Vector3(x0, 0.0, z0), Vector3(x1, wall, z1), Role.WALL, 0.75, [2, 3])
	built.box(Vector3(-0.5, wall, -0.48), Vector3(0.5, slab, 0.4), Role.CAP, 0.6, [3])
	built.quad([Vector3(-0.5, slab, -0.48), Vector3(0.5, slab, -0.48), Vector3(0.5, slab, 0.4), Vector3(-0.5, slab, 0.4)], Vector3.UP, Role.TOP, 1.0,
		[Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)])
	# The door (face coordinates 0..1 across, 1 at the sill).
	var d := 0.003
	built.quad([Vector3(-0.17, 0.0, z1 + d), Vector3(0.17, 0.0, z1 + d), Vector3(0.17, 0.72, z1 + d), Vector3(-0.17, 0.72, z1 + d)], Vector3.BACK, Role.DOOR, 1.0,
		[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	if lod == "near":
		built.box(Vector3(-0.2, 0.0, z1), Vector3(-0.17, 0.75, z1 + 0.015), Role.STEEL)
		built.box(Vector3(0.17, 0.0, z1), Vector3(0.2, 0.75, z1 + 0.015), Role.STEEL)
		built.box(Vector3(-0.17, 0.72, z1), Vector3(0.17, 0.75, z1 + 0.015), Role.STEEL)
		built.box(Vector3(-0.27, 0.79, z1), Vector3(0.27, 0.825, 0.5), Role.CAP, 0.55)
		built.box(Vector3(-0.05, 0.745, z1), Vector3(0.05, 0.787, z1 + 0.04), Role.LAMP)
		built.quad([Vector3(x0 - d, 0.55, -0.16), Vector3(x0 - d, 0.55, 0.04), Vector3(x0 - d, 0.75, 0.04), Vector3(x0 - d, 0.75, -0.16)], Vector3.LEFT, Role.LOUVRE, 0.9,
			[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
		built.box(Vector3(0.22, slab, -0.34), Vector3(0.3, 1.0, -0.26), Role.STEEL, 0.8, [2])
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_penthouse_" + lod
	return mesh

## Water tank (unit box): variant 0 a square FRP panel tank on two steel
## stand beams (near: a ladder up its +z side and a manhole on top), variant
## 1 a round stainless tank with a conical crown on four legs (near: two
## hoops and a lid). Far: the tank bodies and the stand.
static func tank(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var near := lod == "near"
	built.variant = 1
	built.box(Vector3(-0.5, 0.14, -0.5), Vector3(0.5, 0.97, 0.45), Role.FRP, 0.8, [2])
	for x in [-0.42, 0.42]:
		built.box(Vector3(x - 0.04, 0.0, -0.47), Vector3(x + 0.04, 0.14, 0.42), Role.STEEL, 0.8, [2])
	if near:
		built.box(Vector3(-0.14, 0.97, -0.2), Vector3(0.14, 1.0, 0.08), Role.FRP, 0.9, [2])
		for x in [-0.09, 0.09]:
			built.box(Vector3(x - 0.008, 0.0, 0.465), Vector3(x + 0.008, 0.99, 0.49), Role.STEEL)
		for k in 9:
			var y := 0.1 + 0.1 * float(k)
			built.box(Vector3(-0.09, y - 0.006, 0.47), Vector3(0.09, y + 0.006, 0.485), Role.STEEL)
	built.variant = 2
	var segments := 16 if near else 10
	built.cylinder(0.0, 0.0, 0.48, 0.1, 0.9, segments, Role.STAINLESS)
	for k in segments:
		var a0 := TAU * float(k) / float(segments)
		var a1 := TAU * float(k + 1) / float(segments)
		var p0 := Vector3(cos(a0) * 0.48, 0.9, sin(a0) * 0.48)
		var p1 := Vector3(cos(a1) * 0.48, 0.9, sin(a1) * 0.48)
		var mid := (a0 + a1) * 0.5
		built.tri([Vector3(0.0, 0.98, 0.0), p0, p1], Vector3(cos(mid) * 0.08, 0.48, sin(mid) * 0.08).normalized(), Role.STAINLESS, 1.0,
			[Vector2(0.5, 0.0), Vector2(float(k) / segments, 1.0), Vector2(float(k + 1) / segments, 1.0)])
	for x in [-0.3, 0.3]:
		for z in [-0.3, 0.3]:
			built.box(Vector3(x - 0.03, 0.0, z - 0.03), Vector3(x + 0.03, 0.1, z + 0.03), Role.STEEL, 0.8, [2])
	if near:
		for y in [0.35, 0.65]:
			built.cylinder(0.0, 0.0, 0.5, y - 0.012, y + 0.012, segments, Role.STAINLESS, -1, 0.85)
		built.cylinder(0.0, 0.0, 0.1, 0.98, 1.0, 8, Role.STAINLESS, Role.STAINLESS, 0.9)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_tank_" + lod
	return mesh

## Condenser unit on two steel sleepers (floor pitches, front +z): variant 0
## side-blow (a fan grille on the front, louvres on the right side), variant
## 1 top-blow (louvred coil sides under a fan grille on top). Near: a pipe
## pair from the back down to the roof. Far: the body.
static func condenser(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var w := CONDENSER_WIDTH * 0.5
	var d := CONDENSER_DEPTH * 0.5
	var h := CONDENSER_HEIGHT
	var base := 0.03
	for x in [-0.1, 0.1]:
		built.box(Vector3(x - 0.015, 0.0, -d), Vector3(x + 0.015, base, d), Role.STEEL, 0.8, [2])
	if lod != "near":
		built.box(Vector3(-w, base, -d), Vector3(w, h, d), Role.BODY, 0.7, [2])
	else:
		built.variant = 1
		built.box(Vector3(-w, base, -d), Vector3(w, h, d), Role.BODY, 0.7, [1, 2, 5])
		built.quad([Vector3(w, base, d), Vector3(w, base, -d), Vector3(w, h, -d), Vector3(w, h, d)], Vector3.RIGHT, Role.LOUVRE, 0.9,
			[Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
		var g := 0.11
		var gx := -0.03
		var gy := base + 0.02 + g
		var front := func(x0: float, y0: float, x1: float, y1: float, role: int, face: Array) -> void:
			built.quad([Vector3(x0, y0, d), Vector3(x1, y0, d), Vector3(x1, y1, d), Vector3(x0, y1, d)], Vector3.BACK, role, 1.0, face)
		front.call(gx - g, gy - g, gx + g, gy + g, Role.GRILLE, [Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)])
		front.call(-w, base, gx - g, h, Role.BODY, [])
		front.call(gx + g, base, w, h, Role.BODY, [])
		front.call(gx - g, base, gx + g, gy - g, Role.BODY, [])
		front.call(gx - g, gy + g, gx + g, h, Role.BODY, [])
		built.variant = 2
		built.box(Vector3(-w, base, -d), Vector3(w, h, d), Role.LOUVRE, 0.8, [2, 3])
		var f := d - 0.005
		built.quad([Vector3(-f, h, -f), Vector3(f, h, -f), Vector3(f, h, f), Vector3(-f, h, f)], Vector3.UP, Role.FAN, 1.0,
			[Vector2(-1, -1), Vector2(1, -1), Vector2(1, 1), Vector2(-1, 1)])
		for strip in [[-w, -f], [f, w]]:
			built.quad([Vector3(strip[0], h, -d), Vector3(strip[1], h, -d), Vector3(strip[1], h, d), Vector3(strip[0], h, d)], Vector3.UP, Role.BODY, 1.0)
		built.quad([Vector3(-f, h, -d), Vector3(f, h, -d), Vector3(f, h, -f), Vector3(-f, h, -f)], Vector3.UP, Role.BODY, 1.0)
		built.quad([Vector3(-f, h, f), Vector3(f, h, f), Vector3(f, h, d), Vector3(-f, h, d)], Vector3.UP, Role.BODY, 1.0)
		built.variant = 0
		for k in 2:
			var x := 0.06 + 0.035 * float(k)
			var r := 0.009 - 0.002 * float(k)
			built.bar(Vector3(x, 0.12, -d + 0.002), Vector3(x, 0.12, -d - 0.02 - r), r, Role.PIPE)
			built.bar(Vector3(x, 0.12 + r, -d - 0.02), Vector3(x, 0.004, -d - 0.02), r, Role.PIPE)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_condenser_" + lod
	return mesh

## Parapet rim along a roof edge (x 0..1 along the edge, floor pitches across
## and up, inward +z): plastered faces in the building's body colour under a
## light coping. Variant 1 adds a guard railing (near: RAIL_POSTS posts the
## vertex shader spreads along the instance's length, a mid rail and a top
## rail; far: none). Far: the rim.
static func edge(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var t := RIM_THICKNESS
	var h := RIM_HEIGHT
	built.box(Vector3(0.0, 0.0, 0.0), Vector3(1.0, h, t), Role.WALL, 0.8, [2, 3])
	built.quad([Vector3(0.0, h, 0.0), Vector3(1.0, h, 0.0), Vector3(1.0, h, t), Vector3(0.0, h, t)], Vector3.UP, Role.CAP, 1.0,
		[Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)])
	if lod == "near":
		built.variant = 2
		var c := 0.5 * t
		for k in RAIL_POSTS:
			built.anchor = float(k)
			built.box(Vector3(-0.011, h, c - 0.011), Vector3(0.011, h + RAIL_HEIGHT - 0.02, c + 0.011), Role.POST, 0.85, [2])
		built.anchor = -1.0
		built.box(Vector3(0.0, h + RAIL_HEIGHT - 0.022, c - 0.016), Vector3(1.0, h + RAIL_HEIGHT, c + 0.016), Role.RAIL, 0.7)
		built.box(Vector3(0.0, h + 0.15, c - 0.008), Vector3(1.0, h + 0.166, c + 0.008), Role.RAIL, 0.7)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_edge_" + lod
	return mesh

## Solar panel row (x 0..1 along the row, floor pitches across and up): the
## panel face tilted toward +z (about 27°) in an aluminium frame. Near: rails
## under its front and back edges and three leg pairs the vertex shader keeps
## their width along any length. Far: the panel.
static func solar(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var f := Vector3(0.0, 0.12, 0.22)
	var b := Vector3(0.0, 0.34, -0.22)
	var n := Vector3(0.0, 0.44, 0.22).normalized()
	var drop := Vector3(0.0, -0.022, 0.0)
	var x := Vector3(1.0, 0.0, 0.0)
	built.quad([f, f + x, b + x, b], n, Role.SOLAR, 1.0, [Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)])
	built.quad([f + drop, f + x + drop, b + x + drop, b + drop], -n, Role.STEEL, 0.6)
	built.quad([f, f + x, f + x + drop, f + drop], Vector3.BACK, Role.STEEL, 0.9)
	built.quad([b, b + x, b + x + drop, b + drop], Vector3.FORWARD, Role.STEEL, 0.8)
	for e: float in [0.0, 1.0]:
		var o := x * e
		built.quad([f + o, b + o, b + o + drop, f + o + drop], Vector3.RIGHT if e > 0.5 else Vector3.LEFT, Role.STEEL, 0.85)
	if lod == "near":
		built.box(Vector3(0.0, 0.082, 0.18), Vector3(1.0, 0.098, 0.2), Role.STEEL)
		built.box(Vector3(0.0, 0.302, -0.2), Vector3(1.0, 0.318, -0.18), Role.STEEL)
		for a in [0.03, 0.5, 0.97]:
			built.anchor = a
			built.box(Vector3(-0.012, 0.0, 0.18), Vector3(0.012, 0.082, 0.2), Role.LEG, 0.85, [2])
			built.box(Vector3(-0.012, 0.0, -0.2), Vector3(0.012, 0.302, -0.18), Role.LEG, 0.85, [2])
			built.anchor = -1.0
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_solar_" + lod
	return mesh

## Antenna mast (variant 0: a pole on a base plate, near: two Yagi booms) or
## satellite dish (variant 1: a short pole, a dish facing +z 35° up and, near,
## its feed arm), floor pitches.
static func mast(lod: String, material: Material) -> ArrayMesh:
	var built := Builder.new()
	var near := lod == "near"
	built.variant = 1
	built.box(Vector3(-0.07, 0.0, -0.07), Vector3(0.07, 0.015, 0.07), Role.STEEL, 0.8, [2])
	built.box(Vector3(-0.01, 0.015, -0.01), Vector3(0.01, ANTENNA_HEIGHT, 0.01), Role.STEEL, 0.85, [2])
	if near:
		for boom in [[0.66, 0.26], [0.79, 0.2]]:
			var y: float = boom[0]
			var half: float = boom[1] * 0.5
			built.bar(Vector3(0.0, y, -half), Vector3(0.0, y, half), 0.004, Role.STEEL, Vector3.UP)
			for k in 5:
				var z := -half + 2.0 * half * float(k) / 4.0
				var reach := 0.07 - 0.008 * float(k)
				built.bar(Vector3(-reach, y + 0.006, z), Vector3(reach, y + 0.006, z), 0.003, Role.STEEL, Vector3.UP)
	built.variant = 2
	built.box(Vector3(-0.06, 0.0, -0.06), Vector3(0.06, 0.015, 0.06), Role.STEEL, 0.8, [2])
	built.box(Vector3(-0.012, 0.015, -0.012), Vector3(0.012, 0.24, 0.012), Role.STEEL, 0.85, [2])
	var centre := Vector3(0.0, 0.28, 0.03)
	var axis := Vector3(0.0, sin(deg_to_rad(35.0)), cos(deg_to_rad(35.0)))
	var u := Vector3.RIGHT
	var v := axis.cross(u)
	var segments := 12 if near else 8
	var bottom := centre - axis * 0.035
	for k in segments:
		var a0 := TAU * float(k) / float(segments)
		var a1 := TAU * float(k + 1) / float(segments)
		var p0 := centre + (u * cos(a0) + v * sin(a0)) * 0.13
		var p1 := centre + (u * cos(a1) + v * sin(a1)) * 0.13
		built.tri([bottom, p0, p1], axis, Role.DISH, 1.0, [Vector2.ZERO, Vector2(cos(a0), sin(a0)), Vector2(cos(a1), sin(a1))])
		built.tri([bottom, p1, p0], -axis, Role.DISH, 0.7)
	if near:
		var foot := centre - v * 0.115
		var feed := centre + axis * 0.12
		built.bar(foot, feed, 0.004, Role.STEEL, Vector3.RIGHT)
		built.box(feed - Vector3(0.012, 0.012, 0.012), feed + Vector3(0.012, 0.012, 0.012), Role.STEEL)
	var mesh := ArrayMesh.new()
	built.commit(mesh, material)
	mesh.resource_name = "mk_fixture_mast_" + lod
	return mesh

## The shared fixture material: facade_fixture.gdshader with the sign
## include inlined (as environment_materials does for walls), reading the
## environment context's weather and night clock when one is given.
static func material(context: RefCounted = null) -> ShaderMaterial:
	var shader := Shader.new()
	shader.code = preload("./facade_fixture.gdshader").code.replace('#include "facade_signs.gdshaderinc"', preload("./facade_signs.gdshaderinc").code)
	var made := ShaderMaterial.new()
	made.shader = shader
	if context != null and "texture" in context:
		made.set_shader_parameter("environment_data", context.texture)
		made.set_shader_parameter("environment_enabled", true)
	# display_quality.apply_material sets its detail tier and effect distance.
	# Record palettes (sRGB) for rooftop fixtures: PALETTE_KEYS x 8 bodies
	# and concrete, brick, wood x 8 roofs, as the building materials bind them.
	var body := PackedVector3Array()
	var roof := PackedVector3Array()
	for key: String in PALETTE_KEYS:
		for rgb: Array in PALETTE.data.body[key]: body.append(Vector3(float(rgb[0]), float(rgb[1]), float(rgb[2])) / 255.0)
	for key: String in ["concrete", "brick", "wood"]:
		for rgb: Array in PALETTE.data.roof[key]: roof.append(Vector3(float(rgb[0]), float(rgb[1]), float(rgb[2])) / 255.0)
	made.set_shader_parameter("fixture_body", body)
	made.set_shader_parameter("fixture_roof", roof)
	made.set_meta("mapkit_detail", true)
	made.set_meta("mapkit_opaque", true)
	made.resource_name = "mk_facade_fixture"
	QUALITY.apply_material(made, QUALITY.active())
	return made

## One part per call (the material or a mesh); true when the whole set is in
## state.result. `lease` (track(Object)) receives every created resource.
## The render_resource_cache.prepare_mesh_set builder signature.
static func build(state: Dictionary, lease: RefCounted = null, context: RefCounted = null) -> bool:
	if not state.has("step"):
		state.step = 0
		state.result = {}
	if state.step >= STEPS.size(): return true
	var step: String = STEPS[state.step]
	state.phase = "fixture_" + step
	var result: Dictionary = state.result
	if step == "material":
		result.material = material(context)
		if lease != null:
			lease.track(result.material)
			lease.track(result.material.shader)
	else:
		var lod := step.get_slice("_", 1)
		var mesh: ArrayMesh
		match step.get_slice("_", 0):
			"blade": mesh = blade(lod, result.material)
			"ac": mesh = air_conditioner(lod, result.material)
			"penthouse": mesh = penthouse(lod, result.material)
			"tank": mesh = tank(lod, result.material)
			"condenser": mesh = condenser(lod, result.material)
			"edge": mesh = edge(lod, result.material)
			"solar": mesh = solar(lod, result.material)
			_: mesh = mast(lod, result.material)
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

## The cell's packed fixture list (presentation `facade_fixtures`), empty
## when the presentation has none.
static func fixtures(chunk: Dictionary) -> PackedFloat32Array:
	var list: Variant = chunk.get("presentation", {}).get("facade_fixtures", PackedFloat32Array())
	return list if list is PackedFloat32Array else PackedFloat32Array()

## Claims the shared set as a cell with fixtures begins (chunk_renderer.begin,
## before any upload) when it has a resource cache. Fixtures are decoration:
## a refused claim skips them (job.fixtures_refused), never the cell.
static func claim(job: Dictionary) -> void:
	if job.get("resources") == null or job.claims.has(KEY) or fixtures(job.chunk).is_empty(): return
	var key: String = job.resources.claim_mesh_set(KEY, SHARED_BYTES)
	if key.is_empty(): job.fixtures_refused = true
	else: job.claims[KEY] = key

## Shared set for a renderer job: 1 ready, 0 another step is needed, -1
## unavailable (the cell skips its fixtures).
static func ready(job: Dictionary) -> int:
	if job.has("fixtures"): return 1
	job.last_phase = "fixture_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.fixtures = made
		return 0
	if not job.claims.has(KEY): return -1
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -1
		job.fixtures = made
	return 0

## Instance groups of a packed list: "<kind>/<square x>,<square z>" ->
## {kind, buffer}: 12 transform floats and 4 custom floats (design bits in
## three 8-bit channels; the blade's aspect or a rooftop fixture's floor
## pitch) per instance, in list order. Instances scale their mesh by kind
## (see the header); a rim's height carries its railing and a 0-2 % step.
static func plan(list: PackedFloat32Array) -> Dictionary:
	var groups := {}
	for i in range(0, list.size() - FLOATS + 1, FLOATS):
		var kind: String = KINDS[clampi(int(list[i + 4]), 0, KINDS.size() - 1)]
		var origin := Vector3(list[i], list[i + 1], list[i + 2])
		var scale := list[i + 5]
		var height := list[i + 6]
		var bits := int(list[i + 7])
		var width := list[i + 8]
		var depth := list[i + 9]
		var size := Vector3(scale, scale, scale)
		match kind:
			"blade": size.y = height / BLADE_HEIGHT
			"penthouse", "tank": size = Vector3(width, height, depth)
			"edge": size = Vector3(width, height / (RIM_HEIGHT + (RAIL_HEIGHT if (bits >> 9) & 3 == 1 else 0.0)), scale)
			"solar": size.x = width
		# Scaled in the fixture's own frame (x along the wall or rim, z out).
		var b := Basis(Vector3.UP, list[i + 3]) * Basis.from_scale(size)
		var aspect := height / (BLADE_WIDTH * scale) if kind == "blade" else (0.0 if kind == "ac" else scale)
		var key := "%s/%d,%d" % [kind, floori(origin.x / SQUARE_METRES), floori(origin.z / SQUARE_METRES)]
		if not groups.has(key): groups[key] = {"kind": kind, "buffer": PackedFloat32Array()}
		groups[key].buffer.append_array(PackedFloat32Array([b.x.x, b.y.x, b.z.x, origin.x, b.x.y, b.y.y, b.z.y, origin.y, b.x.z, b.y.z, b.z.z, origin.z,
			float(bits & 255), float((bits >> 8) & 255), float((bits >> 16) & 255), aspect]))
	return groups

## Near and far MultiMeshes of one planned group. The cell lease carries the
## nodes and instance buffers; meshes and the material belong to the shared set.
static func attach_group(job: Dictionary) -> void:
	var key: String = job.fixture_plan.keys()[0]
	var group: Dictionary = job.fixture_plan[key]
	job.fixture_plan.erase(key)
	var quality := QUALITY.active()
	for lod: String in LODS:
		var multi := MultiMesh.new()
		multi.transform_format = MultiMesh.TRANSFORM_3D
		multi.use_custom_data = true
		multi.mesh = job.fixtures[group.kind + "_" + lod]
		multi.instance_count = group.buffer.size() / 16
		multi.buffer = group.buffer
		var node := MultiMeshInstance3D.new()
		node.name = "Fixture_%s_%s_%d" % [group.kind, lod, job.root.get_child_count()]
		node.multimesh = multi
		node.set_meta("mapkit_static_detail", true)
		node.set_meta("mapkit_decoration", true)
		node.set_meta("mapkit_fixture_lod", lod)
		node.set_meta("mapkit_fixture_kind", group.kind)
		QUALITY.apply_node(node, quality)
		if job.get("lease") != null:
			job.lease.track(node)
			job.lease.track(multi)
		job.root.add_child(node)

## One renderer step after the cell's objects: prepare the shared set, plan
## the groups, then attach one group per call. Returns whether it did work.
static func step(job: Dictionary) -> bool:
	if job.get("fixtures_done", false): return false
	if fixtures(job.chunk).is_empty() or job.get("fixtures_refused", false):
		job.fixtures_done = true
		return false
	if not job.has("fixtures"):
		if ready(job) < 0: job.fixtures_done = true
		return true
	job.last_phase = "instance_upload"
	if not job.has("fixture_plan"):
		job.fixture_plan = plan(fixtures(job.chunk))
		return true
	if not job.fixture_plan.is_empty():
		attach_group(job)
		return true
	job.fixtures_done = true
	return false
