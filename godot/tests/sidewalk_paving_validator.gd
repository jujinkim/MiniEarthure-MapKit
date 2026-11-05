extends SceneTree
## Sidewalk paving and curbs from urban_surface.gdshader, read back from top-down
## orthographic renders (needs --rendered): the granite curb band and its stone
## joints, the soldier course and running-bond block pitch, tactile paving at a
## crossing mouth (and none without one), far blocks settling to their near
## mean, paving past the effect distance drawing the base colour, the low tier keeping the curb band and the pad, frame-to-frame
## stability, and a kerbed road's gutter. Also the frame upload of the shared
## `concrete` batches (render_memory.sidewalk_arrays).
const RENDER := preload("../chunk_renderer.gd")
const PLAN := preload("../render_memory.gd")
const PPM := 200.0 # pixels per metre of the near renders
const W := 1200
const H := 400
const UNIT := 1.0/30.0
var failed := false
var image: Image
var ppm := PPM
var centre := Vector2(6.0,0.8)
var encoded := false

func check(ok: bool, message: String) -> void:
	if not ok: failed = true; push_error(message)

func _initialize() -> void: run.call_deferred()

## Linear albedo at map (x, z) metres, averaged over a (2r+1)^2 pixel window.
func lin(x: float, z: float, r := 0) -> Color:
	var px := floori((x-centre.x)*ppm+W*0.5)
	var py := floori((z-centre.y)*ppm+H*0.5)
	var sum := Color(0,0,0,0)
	var n := 0
	for i in range(-r,r+1):
		for j in range(-r,r+1):
			var qx := px+i
			var qy := py+j
			if qx < 0 or qy < 0 or qx >= W or qy >= H: continue
			var c := image.get_pixel(qx,qy)
			sum += c.srgb_to_linear() if encoded else c
			n += 1
	return sum/maxf(n,1) if n > 0 else Color.BLACK

func yellow(c: Color) -> bool: return c.r > 0.35 and c.g > 0.2 and c.b < 0.12 and c.r > 2.5*c.b+0.1

func lum(c: Color) -> float: return 0.2126*c.r+0.7152*c.g+0.0722*c.b

## Joint pitch (metres) along a scan: the autocorrelation peak of the
## high-passed luminance, so per-block colour does not mask thin joints.
func pitch(a: Vector2, b: Vector2) -> float:
	var steps := int(a.distance_to(b)*ppm)
	var values := PackedFloat32Array()
	for i in steps+1:
		var q := a.lerp(b,float(i)/float(steps))
		values.append(lum(lin(q.x,q.y)))
	var edges := PackedFloat32Array()
	for i in range(2,values.size()-2): edges.append(values[i]-0.5*(values[i-2]+values[i+2]))
	var sums := PackedFloat32Array()
	for k in 92:
		var sum := 0.0
		for i in edges.size()-k: sum += edges[i]*edges[i+k]
		sums.append(sum/float(edges.size()-k))
	var best := 0.0
	for k in range(4,91): best = maxf(best,sums[k])
	# The first strong peak (later ones repeat it), refined between pixels.
	for k in range(4,91):
		if sums[k] >= 0.6*best and sums[k] >= sums[k-1] and sums[k] >= sums[k+1]:
			var curve := sums[k-1]-2.0*sums[k]+sums[k+1]
			var shift := 0.5*(sums[k-1]-sums[k+1])/curve if curve < 0.0 else 0.0
			return (k+clampf(shift,-0.5,0.5))/ppm
	return -1.0

func run() -> void:
	if DisplayServer.get_name() == "headless":
		push_error("sidewalk_paving_validator needs --rendered")
		print("sidewalk_paving_validator: FAIL (headless)")
		quit(1)
		return
	frame_uploads()
	root.size = Vector2i(W,H)
	var scene := Node3D.new(); root.add_child(scene)
	var world := WorldEnvironment.new(); world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_color = Color.WHITE
	world.environment.ambient_light_energy = 1.0
	world.environment.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED
	world.environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	scene.add_child(world)
	# Compatibility writes albedo-linear values; Forward/Mobile encode sRGB.
	encoded = RenderingServer.get_current_rendering_method() != "gl_compatibility"
	# A 12 m sidewalk 1.6 m deep beside a curb along +x at z = 0, framed like
	# road_style.rs: UV (along, across), UV2 (offset from a crossing at x = 6, kind).
	var material := ShaderMaterial.new(); material.shader = RENDER.wet_urban_shader()
	var parameters := {"paving":true,"slabs":false,"surface_effect_distance":96.0,"detail_quality":2}
	for key: String in parameters: material.set_shader_parameter(key,parameters[key])
	var sidewalk := MeshInstance3D.new(); scene.add_child(sidewalk)
	sidewalk.material_override = material
	var camera := Camera3D.new(); scene.add_child(camera)
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = H/PPM
	camera.position = Vector3(centre.x,20,centre.y); camera.rotation = Vector3(-PI/2,0,0)
	camera.far = 100.0; camera.current = true
	# Section 3: red-brown mix, herringbone, at a crossing mouth.
	sidewalk.mesh = strip(2,3)
	await capture("crossing-q2")
	crossing_checks("q2")
	var first := image
	await capture("crossing-q2-again")
	var change := 0.0
	for py in range(0,H,3):
		for px in range(0,W,3): change = maxf(change,absf(image.get_pixel(px,py).r-first.get_pixel(px,py).r))
	check(change < 0.01,"paving is stable between frames (max change %.4f)"%change)
	material.set_shader_parameter("detail_quality",0)
	await capture("crossing-q0")
	crossing_checks("q0")
	# Section 4: red-brown mix, running bond, no crossing.
	material.set_shader_parameter("detail_quality",2)
	sidewalk.mesh = strip(1,4)
	# 600 px/m (1.7 mm pixels) resolve the 1–2 mm joints.
	ppm = 600.0; camera.size = H/ppm; centre = Vector2(6.0,0.35); camera.position = Vector3(centre.x,20,centre.y)
	await capture("bond-q2")
	bond_checks()
	ppm = PPM; camera.size = H/PPM; centre = Vector2(6.0,0.8); camera.position = Vector3(centre.x,20,centre.y)
	# Section 0: grey herringbone; the far render matches the near mean.
	sidewalk.mesh = strip(1,0)
	await capture("field-near")
	var near := Color(0,0,0,0)
	var count := 0
	for x in range(36,85):
		for z in range(40,150,4):
			near += lin(x*0.1,z*0.01); count += 1
	near /= count
	ppm = 4.0; camera.size = H/ppm
	await capture("field-far")
	var far := Color(0,0,0,0)
	count = 0
	for x in range(4,9):
		for z in [0.5,0.75,1.0,1.25]:
			far += lin(x+0.5,z); count += 1
	far /= count
	check(absf(lum(far)-lum(near)) < 0.025 and absf(far.r-near.r) < 0.03,"far blocks settle to the near mean (near %s, far %s)"%[str(near),str(far)])
	# Past the effect distance the paving and the curb band draw the material's
	# base colour (the distant mesh's concrete), whatever the section's mix.
	var base := Color(0.25,0.55,0.35)
	material.set_shader_parameter("base_color",base)
	material.set_shader_parameter("surface_effect_distance",4.0)
	sidewalk.mesh = strip(1,3)
	await capture("field-beyond")
	var beyond := Color(0,0,0,0)
	count = 0
	for x in range(4,9):
		for z in [0.05,0.5,1.0,1.25]:
			beyond += lin(x+0.5,z); count += 1
	beyond /= count
	# Lighting scales the albedo evenly: the read-back stays proportional to the
	# base colour (in the read-back's encoding), not to the section's mix.
	var expected := base.srgb_to_linear() if encoded else base
	var ratios := [beyond.r/expected.r, beyond.g/expected.g, beyond.b/expected.b]
	check(ratios.max()/maxf(ratios.min(),1e-4) < 1.08,"paving past the effect distance draws the base colour (expected a multiple of %s, got %s)"%[str(expected),str(beyond)])
	ppm = PPM; camera.size = H/PPM
	sidewalk.queue_free()
	await gutter_checks(scene,camera)
	scene.queue_free(); await process_frame; await process_frame
	print("sidewalk_paving_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## The 12 m x 1.6 m sidewalk top with a frame of `kind` and `section`.
func strip(kind: int, section: int) -> ArrayMesh:
	var vertices := PackedVector3Array()
	var uv := PackedVector2Array()
	var uv2 := PackedVector2Array()
	for corner in [Vector2(0,0),Vector2(12,1.6),Vector2(12,0),Vector2(0,0),Vector2(0,1.6),Vector2(12,1.6)]:
		vertices.append(Vector3(corner.x,0,corner.y))
		uv.append(corner)
		uv2.append(Vector2(corner.x-6.0,kind+8*section))
	var normals := PackedVector3Array(); normals.resize(6); normals.fill(Vector3.UP)
	var arrays := []; arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices; arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_TEX_UV] = uv; arrays[Mesh.ARRAY_TEX_UV2] = uv2
	var mesh := ArrayMesh.new(); mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
	return mesh

func capture(label: String) -> void:
	for _frame in 4: await process_frame
	await RenderingServer.frame_post_draw
	image = root.get_texture().get_image()
	var path := OS.get_environment("MAPKIT_SIDEWALK_CAPTURE")
	if not path.is_empty(): image.save_png(path.get_basename()+"-"+label+".png")

func crossing_checks(label: String) -> void:
	# Curb band: neutral granite up to 6 cm (18 cm real), red-brown blocks beyond.
	var granite := lin(4.5,0.03,1)
	check(absf(granite.r-granite.g) < 0.05 and lum(granite) > 0.25 and lum(granite) < 0.6,label+" granite curb band "+str(granite))
	var band := -1.0
	for i in 40:
		var c := lin(4.5,0.002+i*0.004,1)
		if c.r-c.g > 0.04: band = 0.002+i*0.004; break
	check(absf(band-0.06) < 0.012,label+" curb band ends at 6 cm (%.3f)"%band)
	# Curb stones are 1 m (1/3 m at 1:3) long with dark sawn joints.
	var joint := minf(lum(lin(13.0/3.0,0.03)),minf(lum(lin(13.0/3.0-0.005,0.03)),lum(lin(13.0/3.0+0.005,0.03))))
	check(joint < 0.8*lum(lin(4.5,0.03)),label+" curb stone joint at 1/3 m (%.3f vs %.3f)"%[joint,lum(lin(4.5,0.03))])
	# Tactile pad: 30 cm in from the curb, as wide as the crossing, two tiles deep;
	# a guide strip leads in along the crossing centre.
	for q: Vector2 in [Vector2(6.0,0.2),Vector2(5.45,0.15),Vector2(6.55,0.25),Vector2(6.0,0.8),Vector2(6.0,1.4)]:
		check(yellow(lin(q.x,q.y,2)),label+" tactile paving at %s: %s"%[str(q),str(lin(q.x,q.y,2))])
	for q: Vector2 in [Vector2(5.2,0.2),Vector2(6.8,0.2),Vector2(6.0,0.05),Vector2(6.15,0.8),Vector2(5.0,1.0)]:
		check(not yellow(lin(q.x,q.y,1)),label+" no tactile paving at %s"%str(q))

func bond_checks() -> void:
	var any := false
	for x in range(51,70,2):
		for z in range(4,66,4): any = any or yellow(lin(x*0.1,z*0.01))
	check(not any,"no tactile paving away from crossings")
	# Running bond: 20 x 10 cm blocks (2 x 1 units at 1:3) along the curb, rows
	# from 12.7 cm (curb band and soldier course) outward.
	var field := 0.06+2.0*UNIT
	var along := pitch(Vector2(5.1,field+0.5*UNIT),Vector2(6.9,field+0.5*UNIT))
	check(absf(along-2.0*UNIT) < 0.004,"running bond joints every 2 units along the curb (%.4f m)"%along)
	var rows := pitch(Vector2(6.01,field+0.2*UNIT),Vector2(6.01,0.66))
	check(absf(rows-UNIT) < 0.003,"block rows one unit apart (%.4f m)"%rows)
	var soldier := pitch(Vector2(5.1,0.06+UNIT),Vector2(6.9,0.06+UNIT))
	check(absf(soldier-UNIT) < 0.003,"soldier course blocks one unit wide (%.4f m)"%soldier)

## A kerbed marked road: a 36 cm (0.12 m) concrete gutter inside each edge, a
## damp line at the curb and a dark seam to the asphalt; none when not kerbed.
func gutter_checks(scene: Node3D, camera: Camera3D) -> void:
	var paths := PackedVector4Array([Vector4(0,0,12,0)]); paths.resize(128)
	var metrics := PackedVector4Array([Vector4(5.4,0,2.4,12)]); metrics.resize(128)
	var borders := PackedVector4Array(); borders.resize(128)
	var material := ShaderMaterial.new(); material.shader = RENDER.wet_urban_shader()
	var parameters := {"marked":true,"curbed":true,"road_paths":paths,"road_metrics":metrics,"road_borders":borders,"path_count":1,
		"lanes":2,"center_line":true,"edge_lines":false,"surface_effect_distance":96.0,"detail_quality":2,"base_color":Color(0.19,0.20,0.23)}
	for key: String in parameters: material.set_shader_parameter(key,parameters[key])
	var road := MeshInstance3D.new(); road.mesh = PlaneMesh.new(); road.mesh.size = Vector2(12,5.4)
	road.position = Vector3(6,0,0); road.material_override = material; scene.add_child(road)
	centre = Vector2(6.0,2.0); camera.position = Vector3(centre.x,20,centre.y)
	await capture("gutter")
	var asphalt := lum(lin(4.5,2.2,2))
	var strip := lum(lin(4.5,2.64,2))
	check(strip > asphalt+0.03,"gutter strip lighter than asphalt (%.3f vs %.3f)"%[strip,asphalt])
	check(lum(lin(4.5,2.695)) < 0.85*strip,"damp line along the curb")
	check(lum(lin(4.5,2.579)) < 0.7*strip,"dark seam where the asphalt meets the gutter")
	material.set_shader_parameter("curbed",false)
	await capture("no-gutter")
	check(absf(lum(lin(4.5,2.64,2))-lum(lin(4.5,2.2,2))) < 0.05,"no gutter on a road without a sidewalk")
	road.queue_free()

## render_memory.sidewalk_arrays: framed triangles of a `concrete` batch take
## their frame as UV/UV2, the rest keep their UV and a zero UV2.
func frame_uploads() -> void:
	var frames := PackedVector4Array([Vector4(1,2,3,9),Vector4(4,5,6,9),Vector4(7,8,9,9)])
	var vertices := PackedVector3Array(); vertices.resize(6)
	var normals := PackedVector3Array(); normals.resize(6); normals.fill(Vector3.UP)
	var uv := PackedVector2Array([Vector2(10,10),Vector2(11,11),Vector2(12,12),Vector2(13,13),Vector2(14,14),Vector2(15,15)])
	for i in 6: vertices[i] = Vector3(i%3,0,i/3)
	var chunk := {"triangles":[{"surface":"concrete","object_id":"terrain"},{"surface":"concrete","object_id":"r:sidewalk"}],"objects":[],
		"scene_vertices":vertices,"scene_normals":normals,"wall_uv":uv,
		"presentation":{"sidewalk_triangles":PackedInt32Array([1]),"sidewalk_frames":frames}}
	var framed: Array = PLAN.sidewalk_arrays(chunk,PackedInt32Array([0,1]),uv)
	check(framed.size() == 2 and framed[0][0] == Vector2(10,10) and framed[0][3] == Vector2(1,2) and framed[0][5] == Vector2(7,8)
		and framed[1][0] == Vector2.ZERO and framed[1][4] == Vector2(6,9),"framed triangles take their frame, others keep UV: "+str(framed))
	check(PLAN.sidewalk_arrays(chunk,PackedInt32Array([0]),uv.slice(0,3)).is_empty(),"no frame upload without a framed triangle")
	var batches: Array = PLAN.prepare_batches(chunk)
	check(batches.size() == 1 and batches[0].key == "concrete" and batches[0].uv2.size() == 6 and batches[0].uv2[3] == Vector2(3,9),
		"prepared concrete batches carry the frames")
