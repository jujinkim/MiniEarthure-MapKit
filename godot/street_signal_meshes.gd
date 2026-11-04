extends RefCounted
## Display-only traffic signals and bus shelters, built procedurally (no
## external assets) with street_furniture_meshes.gd's builder and shapes. Each
## kind has a `near` mesh (full detail) and a simple `far` mesh; the shelter's
## glass is its own mesh, drawn at every distance without shadows. Materials
## are one shared set: painted metal, night-lit signs, signal lenses and glass.
##
## Metres, origin at the foot, local +x over the road. The importer turns
## local -z toward the approaching traffic under right-hand driving and toward
## the junction under left-hand driving (osm_furniture.signals), so every head
## is lit on both faces with red at the arm-tip (+x, road-centre) end or on
## top: drivers see a lit face under either convention.
##
## Signals (builtin:signal_h, builtin:signal_v; collision: the 8 x 220 x 8 cm
## pole box, placement.rs): a square base plate with bolts, a flange and a
## tapered round pole with a cap, all inside the pole box, and a push-button
## box inside it as well; a pedestrian signal facing across the road above
## 0.85 m. signal_h carries a tapered mast arm (collar, tie rod, end cap) and,
## hung under it at 1.32 m, a horizontal head; signal_v a vertical head at
## 1.85 m held by brackets above and below it. A head is a bevelled housing
## with three domed lenses (red, amber, green) and a visor over each lens on
## both faces (shorter toward the pole) and a mid-plane backplate with a light
## border. The lens shader cycles the lamps (green 6 s, amber 2 s, red 8 s;
## odd quarter turns of the instance's yaw half a cycle apart, never
## synced): a lit lens glows (emission), an unlit one is dark tinted
## glass; the pedestrian figure walks while the approach is red (blinking for
## its last 2 s).
##
## Bus shelter (builtin:bus_shelter; collision: roof 160 x 4 x 50 cm at 95 cm,
## back wall 160 x 86 x 3 cm, front posts 4 x 90 x 4 cm at x +-78 cm): steel
## front, rear and bay posts, roof beams and rafters, a roof deck sloping to a
## rear gutter behind a coloured fascia with a name strip, glass in two back
## bays and two half-depth side screens with a frosted band, an advertising
## lightbox in the third bay, a timetable panel clamped to its bay posts, a
## hanging route plate and a perforated steel bench. Every vertex stays inside
## the proxies' 160 x 97 x 50 cm envelope; posts, roof and back wall stand
## where their collision boxes are (the bench, side screens and their mid
## posts have none). Signs and posters glow softly at night (atmosphere_surface
## lighting_role 2).
##
## Printed faces (posters, timetable, plates, the fascia strip, the
## pedestrian signal front) are tiled flat faces (compose()) closing open
## boxes, never layers stacked a fraction of a millimetre over another face:
## those flicker in a 24-bit depth buffer at street distances.
##
## The renderer draws these meshes instead of the generated proxy faces
## (render_memory.DRAWN_OVER); collision and occluders never read those faces.
## A cell instances each kind per 32 m group as near and far MultiMeshes, and
## shelter glass once per group at every distance (display_quality
## .apply_furniture: the near mesh within furniture_near, none on low; shadows
## on every tier, none for glass). build() makes one part per call for
## admitted streaming steps (render_resource_cache.prepare_mesh_set);
## step_shared() builds a process-lifetime set for cells without a resource
## cache.

const F := preload("./street_furniture_meshes.gd")
const QUALITY := preload("./display_quality.gd")
const KINDS := {"builtin:signal_h": "signal_h", "builtin:signal_v": "signal_v", "builtin:bus_shelter": "shelter"}
const LODS := ["near", "far"]
## Meshes drawn per kind and LOD; the shelter glass is one node per group
## tagged far (editor picking) and drawn at every distance.
const PARTS := {"signal_h": {"near": ["signal_h_near"], "far": ["signal_h_far"]},
	"signal_v": {"near": ["signal_v_near"], "far": ["signal_v_far"]},
	"shelter": {"near": ["shelter_near"], "far": ["shelter_far", "shelter_glass"]}}
## Lamp phase tie-break (quarter turns): a yaw exactly halfway between two
## quarter turns goes to the next one; the threshold lies between whole
## millidegrees (0.0005 degrees early), far beyond float error.
const PHASE_BIAS := 0.0005 / 90.0
const KEY := "builtin:street_signals"
const GROUP_METRES := 32.0
## Collision boxes (placement.rs builtin), scene metres: map local z is scene -z.
const SIGNAL_POLE := AABB(Vector3(-0.04, 0.0, -0.04), Vector3(0.08, 2.2, 0.08))
const SHELTER_PROXIES := [AABB(Vector3(-0.8, 0.93, -0.25), Vector3(1.6, 0.04, 0.5)), AABB(Vector3(-0.8, 0.04, 0.215), Vector3(1.6, 0.86, 0.03)),
	AABB(Vector3(-0.8, 0.0, -0.23), Vector3(0.04, 0.9, 0.04)), AABB(Vector3(0.76, 0.0, -0.23), Vector3(0.04, 0.9, 0.04))]
const SHELTER_ENVELOPE := AABB(Vector3(-0.8, 0.0, -0.25), Vector3(1.6, 0.97, 0.5))
## Head centres as the earlier renderer drew them.
const H_HEAD := Vector3(1.32, 2.06, 0.0)
const V_HEAD := Vector3(0.0, 1.85, -0.09)
## Lens roles (the lens surface UV.x; vertex colour red x 4 while building).
const RED := 0
const AMBER := 1
const GREEN := 2
const STOP := 3
const WALK := 4
## Paint (sRGB).
const POLE := "4a5055"
const HOUSING := "1f2326"
const BACKPLATE := "131517"
const BORDER := "d6d6cc"
const BUTTON := "c9a227"
const FRAME := "3c444a"
const ROOF := "5a636a"
const FASCIA := "1f4f86"
const STEEL := "9aa1a6"
const STEPS := ["material_paint", "material_sign", "material_lens", "material_glass", "signal_h_near", "signal_h_far",
	"signal_v_near", "signal_v_far", "shelter_near", "shelter_far", "shelter_glass"]
## Memory allowance of one shared set (render_resource_cache): about 7,100
## vertices at <= 56 B and 13,500 indices at 4 B in CPU arrays and GPU buffers
## (0.90 MB; street_signal_validator measures them), 64 KiB for the four
## materials, the lens shader and one mesh's build arrays.
const SHARED_BYTES := 960 * 1024
static var _shared: Dictionary = {}
static var _shared_job: Dictionary = {}

const LENS_SHADER := """shader_type spatial;
render_mode cull_back;
varying float role;
varying vec2 lens;
varying float phase;
void vertex() {
	role = UV.x;
	lens = UV2;
	// Odd quarter turns run half a cycle apart: the nearest quarter turn of
	// the instance's yaw (phase(); ties at 45 degrees to the next). Not
	// instance custom data: it drops instanced vertex tints on Compatibility.
	float yaw = atan(-MODEL_MATRIX[0].z, MODEL_MATRIX[0].x);
	phase = mod(abs(floor(yaw / (PI * 0.5) + 0.5 + %s)), 2.0) * 0.5;
}
float segment(vec2 p, vec2 a, vec2 b) {
	vec2 pa = p - a, ba = b - a;
	return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}
// Pedestrian figure in the lens square (u right, v up): standing or walking.
float figure(vec2 p, bool walking) {
	float d = length(p - vec2(0.5, 0.82)) - 0.085;
	d = min(d, segment(p, vec2(0.5, 0.68), vec2(0.5, 0.44)) - 0.07);
	if (walking) {
		d = min(d, segment(p, vec2(0.5, 0.44), vec2(0.34, 0.14)) - 0.045);
		d = min(d, segment(p, vec2(0.5, 0.44), vec2(0.67, 0.15)) - 0.045);
		d = min(d, segment(p, vec2(0.5, 0.64), vec2(0.33, 0.48)) - 0.035);
		d = min(d, segment(p, vec2(0.5, 0.64), vec2(0.68, 0.5)) - 0.035);
	} else {
		d = min(d, segment(p, vec2(0.45, 0.44), vec2(0.45, 0.14)) - 0.04);
		d = min(d, segment(p, vec2(0.55, 0.44), vec2(0.55, 0.14)) - 0.04);
		d = min(d, segment(p, vec2(0.38, 0.66), vec2(0.37, 0.44)) - 0.03);
		d = min(d, segment(p, vec2(0.62, 0.66), vec2(0.63, 0.44)) - 0.03);
	}
	return 1.0 - smoothstep(-0.01, 0.01, d);
}
void fragment() {
	float t = fract(TIME / 16.0 + phase) * 16.0;
	bool walk = t >= 8.0 && (t < 14.0 || fract(TIME * 1.5) < 0.5);
	// Hues pre-compensated for the ACES tonemapper (no glow): brighter or
	// less saturated emission washes green to mint and amber to lemon.
	vec3 hue = vec3(0.0, 0.55, 0.3);
	float gain = 0.75;
	bool on = false;
	float core = 1.0 - 0.45 * lens.x * lens.x;
	if (role < 0.5) { hue = vec3(1.0, 0.0, 0.0); gain = 0.9; on = t >= 8.0; }
	else if (role < 1.5) { hue = vec3(1.0, 0.25, 0.0); gain = 0.9; on = t >= 6.0 && t < 8.0; }
	else if (role < 2.5) { on = t < 6.0; }
	else if (role < 3.5) { hue = vec3(1.0, 0.0, 0.0); gain = 0.9; on = !walk; core = figure(lens, false); }
	else { on = walk; core = figure(lens, true); }
	float lit = on ? core : 0.0;
	// Lit lenses carry little albedo: sunlight on them would add white.
	ALBEDO = hue * (0.03 + 0.012 * core + 0.04 * lit) + vec3(0.008);
	EMISSION = hue * lit * gain;
	ROUGHNESS = 0.28;
	SPECULAR = 0.45;
}
"""

static func linear(srgb: String, shade := 1.0) -> Color:
	return F.linear(srgb, shade)

## Lens surface: the vertex colours (role x 4, then the lens coordinates:
## radial, or u and v) also go to UV (role) and UV2 (lens coordinates), which
## the lens shader reads: every renderer passes UVs through MultiMeshes alike.
static func commit_lenses(builder: F.Builder, mesh: ArrayMesh, material: Material) -> void:
	var roles := PackedVector2Array()
	var coordinates := PackedVector2Array()
	for color: Color in builder.colors:
		roles.append(Vector2(roundf(color.r * 4.0), 0.0))
		coordinates.append(Vector2(color.g, color.b))
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = builder.vertices
	arrays[Mesh.ARRAY_NORMAL] = builder.normals
	arrays[Mesh.ARRAY_COLOR] = builder.colors
	arrays[Mesh.ARRAY_TEX_UV] = roles
	arrays[Mesh.ARRAY_TEX_UV2] = coordinates
	arrays[Mesh.ARRAY_INDEX] = builder.indices
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	mesh.surface_set_material(mesh.get_surface_count() - 1, material)

## Lens vertex colour: role, then the lens coordinates (radial, or u and v).
static func lens_color(role: int, u: float, v := 0.0) -> Color:
	return Color(float(role) / 4.0, u, v, 1.0)

## Domed round lens facing `normal` (`side` its horizontal axis).
static func disc(builder: F.Builder, centre: Vector3, normal: Vector3, side: Vector3, radius: float, role: int, sides := 16) -> void:
	var up := normal.cross(side).normalized()
	var first := builder.add(centre + normal * radius * 0.1, normal, lens_color(role, 0.0))
	for k in sides:
		var angle := TAU * float(k) / float(sides)
		var out := side * cos(angle) + up * sin(angle)
		builder.add(centre + out * radius, (normal * 0.8 + out * 0.6).normalized(), lens_color(role, 1.0))
	for k in sides: builder.triangle(first, first + 1 + k, first + 1 + (k + 1) % sides)

## Flat lens square (pedestrian figure, far heads) of half size `half`.
static func square(builder: F.Builder, centre: Vector3, normal: Vector3, right: Vector3, half: float, role: int) -> void:
	var up := normal.cross(right).normalized()
	if up.dot(Vector3.UP) < 0.0: up = -up
	var first := builder.vertices.size()
	for corner: Vector2 in [Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)]:
		builder.add(centre + right * (corner.x * 2.0 - 1.0) * half + up * (corner.y * 2.0 - 1.0) * half, normal, lens_color(role, corner.x, corner.y))
	builder.triangle(first, first + 1, first + 2)
	builder.triangle(first, first + 2, first + 3)

## Quad with one colour per corner (a, b, c, d in order), facing `normal`.
static func quad(builder: F.Builder, points: Array, normal: Vector3, colors: Array) -> void:
	var first := builder.vertices.size()
	for index in 4: builder.add(points[index], normal, colors[index])
	builder.triangle(first, first + 1, first + 2)
	builder.triangle(first, first + 2, first + 3)

## Axis-aligned rectangle in the plane through `centre` facing `normal`
## (±x or ±z), `half` its half size along (horizontal, up); vertical gradient
## from `bottom` to `top`.
static func panel(builder: F.Builder, centre: Vector3, normal: Vector3, half: Vector2, bottom: Color, top: Color) -> void:
	var right := Vector3(-normal.z, 0.0, normal.x)
	quad(builder, [centre - right * half.x - Vector3.UP * half.y, centre + right * half.x - Vector3.UP * half.y,
		centre + right * half.x + Vector3.UP * half.y, centre - right * half.x + Vector3.UP * half.y], normal, [bottom, bottom, top, top])

## Axis-aligned box of half extents `half` at `centre` without the faces
## whose outward normals are in `open`: a sleeve or tray that a composed face
## closes (no face sits behind it).
static func open_box(builder: F.Builder, centre: Vector3, half: Vector3, open: Array, color: Color) -> void:
	for axis in 3:
		for s: float in [-1.0, 1.0]:
			var normal := Vector3.ZERO
			normal[axis] = s
			if open.has(normal): continue
			var points := []
			for a: float in [-1.0, 1.0]:
				for b: float in [-1.0, 1.0]:
					var local := Vector3.ZERO
					local[axis] = s * half[axis]
					local[(axis + 1) % 3] = a * half[(axis + 1) % 3]
					local[(axis + 2) % 3] = b * half[(axis + 2) % 3]
					points.append(centre + local)
			F.polygon(builder, points, normal, color)

## Flat face through `centre` facing `normal` (horizontal), `half` its half
## size along (the viewer's right, up), tiled without overlaps: `rects`
## ([u0, v0, u1, v1, bottom colour, top colour, builder]; a later rect lies on
## top of an earlier one, a null builder leaves a hole the caller fills) over
## a vertical base gradient from `bottom` to `top` in `base`. Tiles of one
## owner merge along rows and over rows of the same layout.
static func compose(base: F.Builder, centre: Vector3, normal: Vector3, half: Vector2, bottom: Color, top: Color, rects: Array) -> void:
	var right := Vector3.UP.cross(normal).normalized()
	var us := [-half.x, half.x]
	var vs := [-half.y, half.y]
	for rect: Array in rects:
		us.append_array([rect[0], rect[2]])
		vs.append_array([rect[1], rect[3]])
	us = _breaks(us, half.x)
	vs = _breaks(vs, half.y)
	var bands := []
	for j in vs.size() - 1:
		var v := (float(vs[j]) + float(vs[j + 1])) * 0.5
		var segments := []
		for i in us.size() - 1:
			var u := (float(us[i]) + float(us[i + 1])) * 0.5
			var owner := -1
			for index in range(rects.size() - 1, -1, -1):
				var rect: Array = rects[index]
				if u > rect[0] and u < rect[2] and v > rect[1] and v < rect[3]:
					owner = index
					break
			if not segments.is_empty() and segments[-1][2] == owner: segments[-1][1] = us[i + 1]
			else: segments.append([us[i], us[i + 1], owner])
		if not bands.is_empty() and bands[-1][2] == segments: bands[-1][1] = vs[j + 1]
		else: bands.append([vs[j], vs[j + 1], segments])
	for band: Array in bands:
		for segment: Array in band[2]:
			var owner: int = segment[2]
			var builder = base if owner < 0 else rects[owner][6]
			if builder == null: continue
			var span := Vector2(-half.y, half.y) if owner < 0 else Vector2(rects[owner][1], rects[owner][3])
			var low: Color = bottom if owner < 0 else rects[owner][4]
			var high: Color = top if owner < 0 else rects[owner][5]
			var c0 := low.lerp(high, inverse_lerp(span.x, span.y, band[0]))
			var c1 := low.lerp(high, inverse_lerp(span.x, span.y, band[1]))
			var a := centre + right * float(segment[0])
			var b := centre + right * float(segment[1])
			quad(builder, [a + Vector3.UP * float(band[0]), b + Vector3.UP * float(band[0]), b + Vector3.UP * float(band[1]), a + Vector3.UP * float(band[1])], normal, [c0, c0, c1, c1])

## A printed face framed by a coplanar ring: four `frame` quads in `base`
## between `outer` and `inner` half sizes, and compose() of the print (its
## gradient and `rects`) in `printed` inside.
static func framed(base: F.Builder, printed: F.Builder, centre: Vector3, normal: Vector3, outer: Vector2, inner: Vector2, frame: Color, bottom: Color, top: Color, rects: Array) -> void:
	var right := Vector3.UP.cross(normal).normalized()
	var at := func(u: float, v: float) -> Vector3: return centre + right * u + Vector3.UP * v
	var colors := [frame, frame, frame, frame]
	for s: float in [-1.0, 1.0]:
		quad(base, [at.call(-outer.x, s * inner.y), at.call(outer.x, s * inner.y), at.call(outer.x, s * outer.y), at.call(-outer.x, s * outer.y)], normal, colors)
		quad(base, [at.call(s * inner.x, -inner.y), at.call(s * outer.x, -inner.y), at.call(s * outer.x, inner.y), at.call(s * inner.x, inner.y)], normal, colors)
	compose(printed, centre, normal, inner, bottom, top, rects)

## Sorted distinct breakpoints within ±limit.
static func _breaks(values: Array, limit: float) -> Array:
	var sorted := []
	for value: float in values: sorted.append(clampf(value, -limit, limit))
	sorted.sort()
	var result := []
	for value: float in sorted:
		if result.is_empty() or value - float(result[-1]) > 1e-5: result.append(value)
	return result

## Visor over a lens: a hood shell (outer and inner) around its upper half
## from the face at `centre` out along `normal`, longer at the top.
static func visor(builder: F.Builder, centre: Vector3, normal: Vector3, side: Vector3, radius: float, length: float, color: Color) -> void:
	var count := 9
	var rings := {"outer": [], "inner": [], "tip_outer": [], "tip_inner": []}
	for k in count:
		var angle := lerpf(-0.3, PI + 0.3, float(k) / float(count - 1))
		var out := side * cos(angle) + Vector3.UP * sin(angle)
		var reach := length * (0.4 + 0.6 * clampf(sin(angle), 0.0, 1.0))
		rings.outer.append([centre + out * radius, out])
		rings.inner.append([centre + out * (radius - 0.003), -out])
		rings.tip_outer.append([centre + normal * reach + out * radius * 1.06, out])
		rings.tip_inner.append([centre + normal * reach + out * (radius * 1.06 - 0.003), -out])
	for pair: Array in [["outer", "tip_outer"], ["inner", "tip_inner"], ["tip_outer", "tip_inner"]]:
		var a: Array = rings[pair[0]]
		var b: Array = rings[pair[1]]
		var rim: bool = pair[0] == "tip_outer"
		var start := builder.vertices.size()
		for k in count:
			builder.add(a[k][0], normal if rim else a[k][1], color)
			builder.add(b[k][0], normal if rim else b[k][1], color)
		for k in count - 1:
			var i := start + k * 2
			builder.triangle(i, i + 1, i + 3)
			builder.triangle(i, i + 3, i + 2)

## Vehicle signal head at `centre`: lenses along `along` (RIGHT or UP), lit on
## the faces in `faces` (±1 along z); `reds` the end (±1 along `along`) of the
## red lens per face, `hoods` the visor length per face. Near heads get domed
## lenses, visors and a mid-plane backplate whose light border is a ring of
## bars beside the plate (not over it); far heads a box with flat 8-sided
## lenses.
static func head(paint: F.Builder, lenses: F.Builder, centre: Vector3, along: Vector3, faces: Array, reds: Array, hoods: Array, near: bool) -> void:
	var across := Vector3.UP if along == Vector3.RIGHT else Vector3.RIGHT
	var half := (along * 0.215 + across * 0.06 + Vector3.BACK * 0.03).abs()
	var housing := linear(HOUSING)
	if near:
		F.box(paint, centre, half, 0.012, housing, Basis(), 0.8)
		var length := 0.245
		var width := 0.09
		var rim := 0.012
		F.box(paint, centre, (along * (length - rim) + across * (width - rim) + Vector3.BACK * 0.003).abs(), 0.0, linear(BACKPLATE))
		var border := linear(BORDER)
		for s: float in [-1.0, 1.0]:
			F.box(paint, centre + across * s * (width - rim * 0.5), (along * length + across * rim * 0.5 + Vector3.BACK * 0.0036).abs(), 0.0, border)
			F.box(paint, centre + along * s * (length - rim * 0.5), (along * rim * 0.5 + across * (width - rim) + Vector3.BACK * 0.0036).abs(), 0.0, border)
	else:
		F.box(paint, centre, half + (along * 0.01 + across * 0.01).abs(), 0.0, housing)
	for index in faces.size():
		var s: float = faces[index]
		var normal := Vector3.BACK * s
		for k: int in [-1, 0, 1]:
			var role := AMBER if k == 0 else (RED if k == int(reds[index]) else GREEN)
			# The lens rim stands 1.5 mm off the housing face.
			var at := centre + along * 0.135 * float(k) + normal * 0.0315
			if near:
				disc(lenses, at, normal, Vector3.RIGHT, 0.042, role)
				visor(paint, at - normal * 0.0015, normal, Vector3.RIGHT, 0.05, float(hoods[index]), housing)
			else:
				disc(lenses, at, normal, Vector3.RIGHT, 0.04, role, 8)

## Round tapered pole with a base plate, bolts, flange and cap and a
## push-button box, all inside the 8 cm pole box, and a pedestrian signal
## facing +x above 0.85 m: an open housing closed by a tiled front with two
## lens squares.
static func pole(paint: F.Builder, lenses: F.Builder, near: bool) -> void:
	var color := linear(POLE)
	if not near:
		F.tube(paint, [Vector3(0.0, 0.0, 0.0), Vector3(0.0, 1.0, 0.0), Vector3(0.0, 2.2, 0.0)], [0.034, 0.03, 0.026], 6, color, false, true)
		F.box(paint, Vector3(0.072, 0.92, 0.0), Vector3(0.022, 0.075, 0.04), 0.0, linear(HOUSING))
		return
	F.box(paint, Vector3(0.0, 0.012, 0.0), Vector3(0.039, 0.012, 0.039), 0.004, color, Basis(), 0.7)
	for sx: float in [-1.0, 1.0]:
		for sz: float in [-1.0, 1.0]:
			F.box(paint, Vector3(0.03 * sx, 0.027, 0.03 * sz), Vector3(0.004, 0.003, 0.004), 0.0, linear(POLE, 0.7))
	F.tube(paint, [Vector3(0.0, 0.024, 0.0), Vector3(0.0, 0.05, 0.0), Vector3(0.0, 0.07, 0.0)], [0.036, 0.034, 0.03], 16, linear(POLE, 0.88))
	F.tube(paint, [Vector3(0.0, 0.06, 0.0), Vector3(0.0, 1.2, 0.0), Vector3(0.0, 2.17, 0.0)], [0.03, 0.027, 0.024], 14, color)
	F.tube(paint, [Vector3(0.0, 2.165, 0.0), Vector3(0.0, 2.185, 0.0), Vector3(0.0, 2.2, 0.0)], [0.027, 0.027, 0.012], 14, linear(POLE, 0.95), true, true)
	# Pedestrian signal: housing on two brackets, stop over walk, two hoods.
	var housing := linear(HOUSING)
	for y: float in [0.87, 0.97]:
		F.tube(paint, [Vector3(0.0, y - 0.014, 0.0), Vector3(0.0, y, 0.0), Vector3(0.0, y + 0.014, 0.0)], [0.031, 0.031, 0.031], 12, color, true, true)
		F.box(paint, Vector3(0.04, y, 0.0), Vector3(0.014, 0.008, 0.012), 0.0, color)
	open_box(paint, Vector3(0.072, 0.92, 0.0), Vector3(0.022, 0.075, 0.04), [Vector3.RIGHT], housing)
	var front := Vector3(0.094, 0.92, 0.0)
	compose(paint, front, Vector3.RIGHT, Vector2(0.04, 0.075), housing, housing,
		[[-0.031, 0.004, 0.031, 0.066, housing, housing, null], [-0.031, -0.066, 0.031, -0.004, housing, housing, null]])
	square(lenses, front + Vector3.UP * 0.035, Vector3.RIGHT, Vector3.FORWARD, 0.031, STOP)
	square(lenses, front - Vector3.UP * 0.035, Vector3.RIGHT, Vector3.FORWARD, 0.031, WALK)
	F.box(paint, Vector3(0.1, 0.995, 0.0), Vector3(0.012, 0.002, 0.04), 0.0, housing)
	F.box(paint, Vector3(0.1, 0.92, 0.0), Vector3(0.012, 0.0015, 0.036), 0.0, housing)
	# Push-button box on the sidewalk side, inside the pole box (no collision
	# of its own): a yellow case, the button and a label.
	F.box(paint, Vector3(-0.034, 0.36, 0.0), Vector3(0.0055, 0.036, 0.022), 0.002, linear(BUTTON), Basis(), 0.85)
	F.box(paint, Vector3(-0.0395, 0.372, 0.0), Vector3(0.0012, 0.009, 0.009), 0.0, linear(HOUSING))
	F.box(paint, Vector3(-0.0395, 0.343, 0.0), Vector3(0.001, 0.008, 0.014), 0.0, linear(BORDER))

## Arm-tip end of the red lens (+x along a horizontal head, top along a
## vertical one) on both faces: the road-centre side for drivers under either
## driving convention.
const RED_ENDS := [1, 1]

## signal_h or signal_v: surfaces paint and lens.
static func signal_mesh(kind: String, lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var lenses := F.Builder.new()
	var near := lod == "near"
	var color := linear(POLE)
	pole(paint, lenses, near)
	if kind == "signal_h":
		if near:
			F.tube(paint, [Vector3(0.0, 2.14, 0.0), Vector3(0.7, 2.14, 0.0), Vector3(1.39, 2.14, 0.0)], [0.026, 0.022, 0.018], 10, color)
			F.tube(paint, [Vector3(1.385, 2.14, 0.0), Vector3(1.398, 2.14, 0.0), Vector3(1.405, 2.14, 0.0)], [0.021, 0.021, 0.012], 10, linear(POLE, 0.9), true, true)
			F.tube(paint, [Vector3(0.0, 2.105, 0.0), Vector3(0.0, 2.14, 0.0), Vector3(0.0, 2.175, 0.0)], [0.032, 0.032, 0.032], 14, linear(POLE, 0.92), true, true)
			F.tube(paint, [Vector3(0.0, 1.915, 0.0), Vector3(0.0, 1.93, 0.0), Vector3(0.0, 1.945, 0.0)], [0.03, 0.03, 0.03], 12, linear(POLE, 0.92), true, true)
			F.tube(paint, [Vector3(0.02, 1.93, 0.0), Vector3(0.22, 2.03, 0.0), Vector3(0.42, 2.127, 0.0)], [0.008, 0.008, 0.008], 6, color)
			F.box(paint, Vector3(0.42, 2.14, 0.0), Vector3(0.012, 0.026, 0.026), 0.003, linear(POLE, 0.92))
			# Head clamps on the arm and hangers down to the housing.
			for x: float in [1.17, 1.37]:
				F.box(paint, Vector3(x, 2.14, 0.0), Vector3(0.01, 0.025, 0.025), 0.003, linear(POLE, 0.92))
				F.box(paint, Vector3(x, 2.125, 0.0), Vector3(0.008, 0.012, 0.018), 0.0, linear(HOUSING))
		else:
			F.box(paint, Vector3(0.7, 2.14, 0.0), Vector3(0.7, 0.022, 0.022), 0.0, color)
		head(paint, lenses, H_HEAD, Vector3.RIGHT, [-1.0, 1.0], RED_ENDS, [0.05, 0.05], near)
	else:
		# Brackets from the pole to the plate's top and bottom edges, clear of
		# the lenses on the face toward the pole.
		for y: float in ([1.597, 2.103] if near else [1.617, 2.083]):
			if near: F.tube(paint, [Vector3(0.0, y - 0.015, 0.0), Vector3(0.0, y, 0.0), Vector3(0.0, y + 0.015, 0.0)], [0.031, 0.031, 0.031], 12, linear(POLE, 0.92), true, true)
			F.box(paint, Vector3(0.0, y, -0.057), Vector3(0.01, 0.008, 0.035), 0.002 if near else 0.0, color)
		# The pole-side visors stay 7 mm clear of the pole.
		head(paint, lenses, V_HEAD, Vector3.UP, [-1.0, 1.0], RED_ENDS, [0.05, 0.025], near)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	commit_lenses(lenses, mesh, materials.get("lens"))
	mesh.resource_name = "mk_%s_%s" % [kind, lod]
	return mesh

## Roof deck sloping from 0.958 m at the front to 0.94 m at the back.
const ROOF_SLOPE := 0.018 / 0.486
## Frosted band across the glass (y from, to).
const FROSTED := Vector2(0.47, 0.5)

## Bus shelter: surfaces paint and sign (night lit).
static func shelter(lod: String, materials: Dictionary) -> ArrayMesh:
	var paint := F.Builder.new()
	var signs := F.Builder.new()
	var frame := linear(FRAME)
	var roof := linear(ROOF)
	var fascia := linear(FASCIA)
	var steel := linear(STEEL, 0.8)
	var white := [Color(0.86, 0.87, 0.85), Color(0.92, 0.93, 0.91)]
	# Fascia: an open tray closed by its tiled front (name strip, near: logo).
	open_box(paint, Vector3(0.0, 0.946, -0.2425), Vector3(0.8, 0.021, 0.0055), [Vector3.FORWARD], fascia)
	var strip := [[-0.34, -0.013, 0.34, 0.013, white[0], white[1], signs]]
	if lod == "near": strip.append([0.25, -0.011, 0.33, 0.011, linear("2a6fd1"), linear("2a6fd1"), signs])
	compose(paint, Vector3(0.0, 0.946, -0.248), Vector3.FORWARD, Vector2(0.8, 0.021), fascia, fascia, strip)
	# Frosted band across each pane (both faces), splitting the glass (glass()).
	var frost := linear("9fb0b9")
	var band := (FROSTED.x + FROSTED.y) * 0.5
	var band_half := (FROSTED.y - FROSTED.x) * 0.5
	for s: float in [-1.0, 1.0]:
		for x: float in [-0.52, 0.0]:
			panel(paint, Vector3(x, band, 0.23 + s * 0.001), Vector3.BACK * s, Vector2(0.244, band_half), frost, frost)
		for sx: float in [-1.0, 1.0]:
			panel(paint, Vector3(0.78 * sx + s * 0.001, band, 0.112), Vector3.RIGHT * s, Vector2(0.1, band_half), frost, frost)
	# Lightbox in the right bay: an open frame closed by a poster each side.
	open_box(paint, Vector3(0.52, 0.465, 0.23), Vector3(0.245, 0.41, 0.0125), [Vector3.FORWARD, Vector3.BACK], steel)
	for s: float in [-1.0, 1.0]:
		framed(paint, signs, Vector3(0.52, 0.465, 0.23 + s * 0.0125), Vector3.BACK * s, Vector2(0.245, 0.41), Vector2(0.22, 0.375), steel,
			linear("1d9fb8"), linear("0b2a5b"), poster(signs) if lod == "near" else [])
	if lod == "near":
		for sx: float in [-1.0, 1.0]:
			F.box(paint, Vector3(0.78 * sx, 0.45, -0.21), Vector3(0.017, 0.45, 0.017), 0.004, frame)
			F.box(paint, Vector3(0.78 * sx, 0.004, -0.21), Vector3(0.02, 0.004, 0.02), 0.0, linear(FRAME, 0.75))
			F.box(paint, Vector3(0.78 * sx, 0.45, 0.23), Vector3(0.015, 0.45, 0.0125), 0.003, frame)
			F.box(paint, Vector3(0.26 * sx, 0.45, 0.23), Vector3(0.012, 0.45, 0.0115), 0.003, frame)
			F.box(paint, Vector3(0.78 * sx, 0.45, 0.005), Vector3(0.011, 0.45, 0.011), 0.003, frame)
			# Side screen rails, side beam, rafter, side fascia.
			F.box(paint, Vector3(0.78 * sx, 0.095, 0.112), Vector3(0.009, 0.007, 0.104), 0.0, frame)
			F.box(paint, Vector3(0.78 * sx, 0.882, 0.112), Vector3(0.009, 0.008, 0.104), 0.0, frame)
			F.box(paint, Vector3(0.78 * sx, 0.915, 0.01), Vector3(0.017, 0.015, 0.24), 0.003, frame)
			F.box(paint, Vector3(0.26 * sx, 0.917, 0.01), Vector3(0.01, 0.013, 0.22), 0.0, frame)
			for inward: bool in [false, true]:
				var x := (0.7995 if not inward else 0.7935) * sx
				var normal := Vector3.RIGHT * sx * (-1.0 if inward else 1.0)
				F.polygon(paint, [Vector3(x, 0.925, -0.237), Vector3(x, 0.925, 0.237), Vector3(x, 0.9395, 0.237), Vector3(x, 0.9565, -0.237)], normal, fascia)
		# Back rails, beams, roof deck, rear gutter.
		F.box(paint, Vector3(0.0, 0.048, 0.23), Vector3(0.78, 0.008, 0.011), 0.0, frame)
		F.box(paint, Vector3(0.0, 0.885, 0.23), Vector3(0.78, 0.009, 0.011), 0.0, frame)
		F.box(paint, Vector3(0.0, 0.915, -0.21), Vector3(0.8, 0.015, 0.017), 0.003, frame)
		F.box(paint, Vector3(0.0, 0.915, 0.23), Vector3(0.8, 0.015, 0.0125), 0.003, frame)
		F.box(paint, Vector3(0.0, 0.949, 0.0), Vector3(0.795, 0.006, 0.243), 0.002, roof, Basis(Vector3.RIGHT, atan(ROOF_SLOPE)), 0.82)
		F.box(paint, Vector3(0.0, 0.932, 0.2435), Vector3(0.8, 0.004, 0.0065), 0.0, frame)
		F.box(paint, Vector3(0.0, 0.941, 0.2485), Vector3(0.8, 0.009, 0.0015), 0.0, frame)
		# Timetable in the left back bay: a shallow tray closed by the printed
		# sheet 6 mm in front of its back, on two rails to the bay posts.
		var sheet := Vector3(-0.56, 0.52, 0.2165)
		open_box(paint, sheet + Vector3.BACK * 0.003, Vector3(0.135, 0.135, 0.003), [Vector3.FORWARD], linear(STEEL, 0.7))
		var rows := [[-0.125, 0.092, 0.125, 0.125, linear(FASCIA), linear(FASCIA), signs]]
		for row in 6:
			var u := 0.015 - 0.02 * float(row % 2)
			var w := 0.09 - 0.02 * float(row % 2)
			var v := 0.065 - 0.03 * float(row)
			rows.append([u - w, v - 0.004, u + w, v + 0.004, Color(0.08, 0.09, 0.1), Color(0.08, 0.09, 0.1), signs])
		framed(paint, signs, sheet, Vector3.FORWARD, Vector2(0.135, 0.135), Vector2(0.125, 0.125), linear(STEEL, 0.7), Color(0.84, 0.84, 0.8), Color(0.9, 0.9, 0.86), rows)
		for y: float in [0.43, 0.61]:
			F.box(paint, Vector3(-0.73, y, 0.2195), Vector3(0.035, 0.006, 0.003), 0.0, frame)
			F.box(paint, Vector3(-0.3485, y, 0.2195), Vector3(0.0765, 0.006, 0.003), 0.0, frame)
		# Route plate hanging at the front left: a sleeve closed by both faces.
		for z: float in [-0.23, -0.18]:
			F.box(paint, Vector3(-0.7, 0.89, z), Vector3(0.003, 0.01, 0.003), 0.0, frame)
		open_box(paint, Vector3(-0.7, 0.83, -0.205), Vector3(0.004, 0.05, 0.038), [Vector3.RIGHT, Vector3.LEFT], frame)
		for s: float in [-1.0, 1.0]:
			framed(paint, signs, Vector3(-0.7 + s * 0.004, 0.83, -0.205), Vector3.RIGHT * s, Vector2(0.038, 0.05), Vector2(0.036, 0.048), frame, linear("1f5aa8"), linear("1f5aa8"), route_plate(signs))
		# Perforated steel bench: four slats cantilevered from two legs (plain
		# boxes: millimetre bevels do not show at this scale).
		for index in 4:
			F.box(paint, Vector3(-0.24, 0.148, 0.12 + 0.025 * float(index)), Vector3(0.41, 0.004, 0.0105), 0.0, linear(STEEL, 1.0 - 0.03 * float(index)), Basis(), 0.7)
		for x: float in [-0.58, 0.1]:
			F.box(paint, Vector3(x, 0.072, 0.195), Vector3(0.009, 0.072, 0.009), 0.0, frame)
			F.box(paint, Vector3(x, 0.138, 0.155), Vector3(0.008, 0.006, 0.05), 0.0, frame)
			F.box(paint, Vector3(x, 0.003, 0.195), Vector3(0.016, 0.003, 0.016), 0.0, linear(FRAME, 0.75))
		F.box(paint, Vector3(-0.24, 0.13, 0.2), Vector3(0.41, 0.005, 0.005), 0.0, linear(STEEL))
	else:
		F.box(paint, Vector3(0.0, 0.947, 0.006), Vector3(0.8, 0.021, 0.244), 0.0, roof)
		for sx: float in [-1.0, 1.0]:
			F.box(paint, Vector3(0.78 * sx, 0.463, -0.21), Vector3(0.017, 0.463, 0.017), 0.0, frame)
			F.box(paint, Vector3(0.78 * sx, 0.463, 0.23), Vector3(0.015, 0.463, 0.0125), 0.0, frame)
		F.box(paint, Vector3(0.0, 0.048, 0.23), Vector3(0.78, 0.008, 0.011), 0.0, frame)
		# Bench: a seat slab on two legs.
		F.box(paint, Vector3(-0.24, 0.148, 0.155), Vector3(0.41, 0.006, 0.05), 0.0, linear(STEEL, 0.85))
		for x: float in [-0.58, 0.1]:
			F.box(paint, Vector3(x, 0.071, 0.195), Vector3(0.009, 0.071, 0.009), 0.0, frame)
	var mesh := ArrayMesh.new()
	paint.commit(mesh, materials.get("paint"))
	signs.commit(mesh, materials.get("sign"))
	mesh.resource_name = "mk_shelter_" + lod
	return mesh

## Lightbox poster rects (compose() over its teal to deep blue picture): an
## orange picture, a white band with lines of copy and a yellow badge.
static func poster(signs: F.Builder) -> Array:
	var rects := [[-0.07, -0.01, 0.15, 0.25, linear("f0a23a"), linear("f7d36b"), signs],
		[-0.22, -0.24, 0.22, -0.1, Color(0.86, 0.86, 0.84), Color(0.92, 0.92, 0.9), signs]]
	var ink := Color(0.1, 0.12, 0.16)
	for row in 3:
		var v := -(0.13 + 0.03 * float(row))
		var w := 0.14 - 0.03 * float(row)
		rects.append([-0.04 - w, v - 0.006, -0.04 + w, v + 0.006, ink, ink, signs])
	rects.append([0.1, -0.35, 0.2, -0.27, linear("f2c230"), linear("f2c230"), signs])
	return rects

## Route plate face rects (compose() over its blue): a white bus pictogram.
static func route_plate(signs: F.Builder) -> Array:
	var blue := linear("1f5aa8")
	var white := Color(0.9, 0.92, 0.94)
	var dark := Color(0.05, 0.08, 0.14)
	return [[-0.024, -0.012, 0.024, 0.028, white, white, signs],
		[-0.019, 0.009, 0.019, 0.023, blue, blue, signs], [-0.019, -0.017, -0.009, -0.007, dark, dark, signs],
		[0.009, -0.017, 0.019, -0.007, dark, dark, signs], [-0.026, -0.039, 0.026, -0.029, white, white, signs]]

## Shelter glass: two back bays and the half-depth side screens (one
## surface), each pane split around the frosted band (shelter()).
static func glass(materials: Dictionary) -> ArrayMesh:
	var builder := F.Builder.new()
	var tint := Color.WHITE
	for x: float in [-0.52, 0.0]:
		for span: Vector2 in [Vector2(0.056, FROSTED.x), Vector2(FROSTED.y, 0.876)]:
			panel(builder, Vector3(x, (span.x + span.y) * 0.5, 0.23), Vector3.FORWARD, Vector2(0.244, (span.y - span.x) * 0.5), tint, tint)
	for sx: float in [-1.0, 1.0]:
		for span: Vector2 in [Vector2(0.1, FROSTED.x), Vector2(FROSTED.y, 0.876)]:
			panel(builder, Vector3(0.78 * sx, (span.x + span.y) * 0.5, 0.112), Vector3.RIGHT * sx, Vector2(0.1, (span.y - span.x) * 0.5), tint, tint)
	var mesh := ArrayMesh.new()
	builder.commit(mesh, materials.get("glass"))
	mesh.resource_name = "mk_shelter_glass"
	return mesh

## Shared material of `kind`: paint (street_furniture_meshes painted metal),
## sign (the environment surface lit at night through lighting_role 2), lens
## (the cycling signal shader) or glass (translucent, a sharp sky reflection).
## Without an environment context (standalone previews) plain materials.
static func material(kind: String, context: RefCounted = null) -> Material:
	var made: Material
	match kind:
		"paint":
			made = F.material("paint", context)
		"sign":
			if context != null and context.has_method("generated_material"):
				var lit: ShaderMaterial = context.generated_material(Color.WHITE, "", 0.0, 0.4)
				lit.set_shader_parameter("vertex_tinted", true)
				lit.set_shader_parameter("lighting_role", 2)
				made = lit
			else:
				var plain := StandardMaterial3D.new()
				plain.vertex_color_use_as_albedo = true
				plain.roughness = 0.4
				plain.cull_mode = BaseMaterial3D.CULL_DISABLED
				made = plain
		"lens":
			var shader := Shader.new()
			shader.code = LENS_SHADER % ("%.9f" % PHASE_BIAS)
			var lens := ShaderMaterial.new()
			lens.shader = shader
			made = lens
		_:
			var pane := StandardMaterial3D.new()
			pane.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
			# A cool tint at 0.45 with a sharp sky reflection and rim. Face on
			# against sunlit pavement the blend happens before tonemapping, so
			# the panes darken it only slightly; the frosted band and frames
			# carry the reading there.
			pane.albedo_color = Color(0.18, 0.26, 0.3, 0.45)
			pane.metallic = 0.25
			pane.metallic_specular = 1.0
			pane.roughness = 0.04
			pane.rim_enabled = true
			pane.rim = 0.5
			pane.rim_tint = 0.6
			pane.cull_mode = BaseMaterial3D.CULL_DISABLED
			made = pane
	made.resource_name = "mk_signal_" + kind
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
	state.phase = "signals_" + step
	var result: Dictionary = state.result
	if step.begins_with("material_"):
		var made := material(step.trim_prefix("material_"), context)
		result.materials[step.trim_prefix("material_")] = made
		if lease != null:
			lease.track(made)
			if step == "material_lens": lease.track(made.shader)
	else:
		var mesh: ArrayMesh
		if step == "shelter_glass": mesh = glass(result.materials)
		elif step.begins_with("shelter"): mesh = shelter(step.get_slice("_", 1), result.materials)
		else: mesh = signal_mesh(step.left(8), step.get_slice("_", 2), result.materials)
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
## upload) when it holds signals or shelters and has a resource cache; false
## when the cache refuses it (the cell fails before uploading anything: its
## signals and shelters would lose their only visible body).
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
## unavailable, which fails the cell (chunk_renderer): its signals and
## shelters would lose their only visible body. A refused claim already
## failed the cell in begin().
static func ready(job: Dictionary) -> int:
	if job.has("signals"): return 1
	job.last_phase = "signal_preparation"
	if job.get("resources") == null:
		var made := step_shared()
		if not made.is_empty(): job.signals = made
		return 0
	# begin() claimed the set (claim) or failed the cell.
	if not job.claims.has(KEY): return -2
	if job.resources.prepare_mesh_set(job.claims[KEY], build):
		var made: Dictionary = job.resources.mesh_set(job.claims[KEY])
		if made.is_empty(): return -2
		job.signals = made
	return 0

## Lamp phase of a signal as the lens shader derives it from the instance's
## yaw: 0.5 when its nearest quarter turn is odd (a yaw exactly halfway goes
## to the next one), so the quarter turns decide within the importer's ±45
## degree offsets and the actual heading decides for larger editor offsets.
static func phase(object: Dictionary) -> float:
	var x := F.placement(object).basis.x
	var quarters := floorf(atan2(-x.z, x.x) / (PI * 0.5) + 0.5 + PHASE_BIAS)
	return fmod(absf(quarters), 2.0) * 0.5

## One admitted step per object: its transform and id join the cell's group
## of its kind and 32 m square.
static func plan(job: Dictionary, object: Dictionary) -> void:
	var kind: String = KINDS[str(object.asset_id)]
	var transform := F.placement(object)
	var o := transform.origin
	var key := "%s/%d,%d" % [kind, floori(o.x / GROUP_METRES), floori(o.z / GROUP_METRES)]
	if not job.has("signal_plan"): job.signal_plan = {}
	if not job.signal_plan.has(key): job.signal_plan[key] = {"kind": kind, "parts": [], "ids": PackedStringArray()}
	var b := transform.basis
	job.signal_plan[key].parts.append(PackedFloat32Array([b.x.x, b.y.x, b.z.x, o.x, b.x.y, b.y.y, b.z.y, o.y, b.x.z, b.y.z, b.z.z, o.z]))
	job.signal_plan[key].ids.append(str(object.id))

## Near and far MultiMeshes of one planned group (shelter glass one more,
## shadowless node). The cell lease carries the nodes and instance buffers;
## meshes and materials belong to the shared set. mapkit_object_ids lists the
## instances' object ids (editor picking, as for street furniture).
static func attach_group(job: Dictionary) -> void:
	var key: String = job.signal_plan.keys()[0]
	var group: Dictionary = job.signal_plan[key]
	job.signal_plan.erase(key)
	var buffer := PackedFloat32Array()
	for part: PackedFloat32Array in group.parts: buffer.append_array(part)
	var quality := QUALITY.active()
	for lod: String in LODS:
		for mesh_key: String in PARTS[group.kind][lod]:
			var multi := MultiMesh.new()
			multi.transform_format = MultiMesh.TRANSFORM_3D
			multi.mesh = job.signals[mesh_key]
			multi.instance_count = buffer.size() / 12
			multi.buffer = buffer
			var node := MultiMeshInstance3D.new()
			node.name = "Signals_%s_%d" % [mesh_key if mesh_key.ends_with(lod) else mesh_key + "_" + lod, job.root.get_child_count()]
			node.multimesh = multi
			node.set_meta("mapkit_static_detail", true)
			node.set_meta("mapkit_decoration", false)
			node.set_meta("mapkit_furniture_lod", lod)
			node.set_meta("mapkit_furniture_kind", group.kind)
			node.set_meta("mapkit_object_ids", group.ids)
			if mesh_key == "shelter_glass": node.set_meta("mapkit_furniture_glass", true)
			QUALITY.apply_node(node, quality)
			if job.get("lease") != null:
				job.lease.track(node)
				job.lease.track(multi)
			job.root.add_child(node)
