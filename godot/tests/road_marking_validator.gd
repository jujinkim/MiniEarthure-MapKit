extends SceneTree
## Road paint geometry from urban_surface.gdshader, read back from top-down
## orthographic renders (needs --rendered): Korean crosswalk bars, inbound stop
## lines, solid lines before stop lines, combined arrows kept inside their lanes,
## warning diamonds, the double yellow centre and footprint-filtered far paint on
## a two-way road; one-way, left-hand, short and mid-block rules on two more
## roads; the single yellow on a two-lane road without edge lines (double with
## them); and the detail tier (chips, grime, abrasion) in a near perspective shot.
const RENDER := preload("../chunk_renderer.gd")
const PPM := 20.0 # pixels per metre of the near render
const W := 1200
const H := 240
const CENTER_X := 25.0
var failed := false
var image: Image
var ppm := PPM
var encoded := false
var material: ShaderMaterial
var base_parameters := {}

func check(ok: bool, message: String) -> void:
	if not ok: failed = true; push_error(message)

func _initialize() -> void: run.call_deferred()

func lum(x: float, z: float) -> float:
	var px := floori((x-CENTER_X)*ppm+W*0.5)
	var py := floori(z*ppm+H*0.5)
	if px < 0 or py < 0 or px >= W or py >= H: return 0.0
	return image.get_pixel(px,py).get_luminance()

func linear(x: float, z: float) -> float:
	var px := floori((x-CENTER_X)*ppm+W*0.5)
	var py := floori(z*ppm+H*0.5)
	if px < 0 or py < 0 or px >= W or py >= H: return 0.0
	var value := image.get_pixel(px,py)
	# Compatibility writes albedo-linear values; Forward/Mobile encode sRGB.
	return value.srgb_to_linear().r if encoded else value.r

func painted(x: float, z: float, radius := 0.05) -> bool:
	# Brightest pixel within a small window: thin outlines straddle pixels.
	var best := 0.0
	for i in range(-2,3):
		for j in range(-2,3):
			best = maxf(best,linear(x+i*radius*0.5,z+j*radius*0.5))
	return best > 0.3

## White runs [start, end] in metres along a scan from a to b (step: one pixel).
func runs(a: Vector2, b: Vector2) -> Array:
	var found: Array = []
	var steps := int(a.distance_to(b)*ppm)
	var start := -1.0
	for i in steps+1:
		var t := float(i)/ppm
		var q := a.lerp(b,float(i)/float(steps))
		var on := lum(q.x,q.y) > 0.6
		if on and start < 0.0: start = t
		if not on and start >= 0.0: found.append(Vector2(start,t)); start = -1.0
	if start >= 0.0: found.append(Vector2(start,float(steps)/ppm))
	return found

## Share of a scan with paint in the neighbouring pixels across it: thin lines
## straddle pixel rows, so each step takes its brightest neighbour.
func fraction(a: Vector2, b: Vector2) -> float:
	var steps := int(a.distance_to(b)*ppm)
	var side := (b-a).normalized().orthogonal()/ppm
	var on := 0
	for i in steps+1:
		var q := a.lerp(b,float(i)/float(steps))
		if maxf(linear(q.x,q.y),maxf(linear(q.x+side.x,q.y+side.y),linear(q.x-side.x,q.y-side.y))) > 0.3: on += 1
	return float(on)/float(steps+1)

## Bars across the road at x as (centre z, width) from pixel coverage.
func coverage_runs(x: float, asphalt: float, paint: float) -> Array:
	var found: Array = []
	var amount := 0.0
	var moment := 0.0
	for py in H:
		var z := (py+0.5-H*0.5)/ppm
		var c := clampf((linear(x,z)-asphalt)/(paint-asphalt),0.0,1.0) if absf(z) < 2.7 else 0.0
		if c > 0.03:
			amount += c; moment += c*z
		elif amount > 0.0:
			found.append(Vector2(moment/amount,amount/ppm)); amount = 0.0; moment = 0.0
	return found

func run() -> void:
	if DisplayServer.get_name() == "headless":
		push_error("road_marking_validator needs --rendered")
		print("road_marking_validator: FAIL (headless)")
		quit(1)
		return
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
	# One straight 50 m two-way road along +x: 5.4 m wide, four 1.2 m lanes,
	# crosswalks and junction stop lines at both ends (start: left + straight,
	# end: straight + right). across = +z, so the start's inbound half is z < 0.
	var paths := PackedVector4Array([Vector4(0,0,50,0)]); paths.resize(128)
	var metrics := PackedVector4Array([Vector4(5.4,0,50.0/21.0,50)]); metrics.resize(128)
	var borders := PackedVector4Array(); borders.resize(128)
	material = ShaderMaterial.new(); material.shader = RENDER.wet_urban_shader()
	base_parameters = {"marked":true,"road_paths":paths,"road_metrics":metrics,"road_borders":borders,
		"path_count":1,"edge_count":0,"lanes":4,"lane_width":1.2,"center_line":true,"edge_lines":false,
		"crosswalk_start":true,"crosswalk_end":true,"stop_start":8|1|2,"stop_end":8|2|4,"one_way":0,
		"left_hand":false,"surface_effect_distance":0.001,"detail_quality":0}
	use({})
	var mesh := MeshInstance3D.new(); var plane := PlaneMesh.new(); plane.size = Vector2(60,12)
	mesh.mesh = plane; mesh.material_override = material; mesh.position = Vector3(CENTER_X,0,0)
	scene.add_child(mesh)
	var camera := Camera3D.new(); scene.add_child(camera)
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = H/PPM
	camera.position = Vector3(CENTER_X,20,0); camera.rotation = Vector3(-PI/2,0,0)
	camera.far = 100.0; camera.current = true
	for quality in [0,2]:
		material.set_shader_parameter("detail_quality",quality)
		ppm = PPM; camera.size = H/PPM
		await capture("q%d"%quality)
		near_checks("q%d"%quality)
	# Far: 0.5 m pixels hold one to three crosswalk bars each. Box-filtered bars
	# match their analytic area coverage instead of aliasing into bars and gaps.
	var asphalt := linear(25,-2.2)
	var paint := linear(1.0,0.0)
	ppm = 2.0; camera.size = H/ppm
	await capture("far")
	var worst := 0.0
	for py in range(H/2-5,H/2+5):
		var z0 := (py-H*0.5)/ppm
		var covered := 0.0
		for i in range(-6,7):
			covered += maxf(0.0,minf(z0+0.5,i*0.383+0.0835)-maxf(z0,i*0.383-0.0835))
		var expected := lerpf(asphalt,paint,covered/0.5)
		# Pixel column x 1.0–1.5 m lies inside the 0.3–1.7 m crossing.
		worst = maxf(worst,absf(linear(1.25,z0+0.25)-expected))
	check(worst < 0.06,"far crosswalk pixels match bar coverage (worst linear error %.3f)"%worst)
	ppm = PPM; camera.size = H/PPM
	# One-way, six 1.2 m lanes (7.8 m road, shoulders), crosswalks at both ends;
	# the start is the departure end and the end approach has straight only, so
	# its outer lanes have no movement. A 2 m period puts a dash at 1.625–2.375 m.
	use({"road_metrics":PackedVector4Array([Vector4(7.8,0,2.0,50)]),"lanes":6,"one_way":1,"center_line":false,
		"stop_start":8|1|2|4,"stop_end":8|2})
	await capture("one-way")
	one_way_checks()
	# Left-hand two-way 20 m road, lanes sharing the 5.4 m width, edge lines from
	# the borders: a mid-block crosswalk at the start and left + straight at the end.
	var edges := PackedVector4Array([Vector4(0,2.7,50,2.7),Vector4(50,-2.7,0,-2.7)]); edges.resize(128)
	use({"road_paths":PackedVector4Array([Vector4(0,0,20,0)]),"road_metrics":PackedVector4Array([Vector4(5.4,0,20.0/7.0,20)]),
		"road_borders":edges,"edge_count":2,"lane_width":0.0,"edge_lines":true,"left_hand":true,
		"crosswalk_end":false,"stop_start":0,"stop_end":8|1|2})
	await capture("left-hand")
	left_hand_checks()
	# Two-lane two-way 3 m road without crosswalks or stop lines, at 1.25 cm
	# pixels: a single yellow without edge lines, the double yellow with them.
	var two_lane := {"road_metrics":PackedVector4Array([Vector4(3.0,0,50.0/21.0,50)]),"lanes":2,"lane_width":0.0,
		"crosswalk_start":false,"crosswalk_end":false,"stop_start":0,"stop_end":0}
	ppm = 80.0; camera.size = H/ppm
	use(two_lane.merged({"edge_lines":false}))
	await capture("two-lane")
	two_lane_checks(true)
	use(two_lane.merged({"road_borders":PackedVector4Array([Vector4(0,1.5,50,1.5),Vector4(50,-1.5,0,-1.5)]),"edge_count":2,"edge_lines":true}))
	await capture("two-lane-edged")
	two_lane_checks(false)
	ppm = PPM; camera.size = H/PPM
	await detail_checks(camera)
	scene.queue_free(); await process_frame; await process_frame
	print("road_marking_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)

## Base fixture with overrides; arrays are padded to the shader's 128 entries.
func use(overrides: Dictionary) -> void:
	var parameters := base_parameters.duplicate()
	parameters.merge(overrides,true)
	for key: String in parameters:
		var value: Variant = parameters[key]
		if value is PackedVector4Array and value.size() < 128:
			var padded: PackedVector4Array = value.duplicate(); padded.resize(128); value = padded
		material.set_shader_parameter(key,value)

func capture(label: String) -> void:
	for _frame in 4: await process_frame
	await RenderingServer.frame_post_draw
	image = root.get_texture().get_image()
	if label == "q0":
		# A full crosswalk bar: 0.85 linear albedo, 0.93 once sRGB encoded.
		encoded = image.get_pixel(floori((1.0-CENTER_X)*ppm+W*0.5),floori(H*0.5)).r > 0.9
	var path := OS.get_environment("MAPKIT_ROAD_MARKING_CAPTURE")
	if not path.is_empty(): image.save_png(path.get_basename()+"-"+label+".png")

func near_checks(label: String) -> void:
	var base := lum(25,-2.2)
	check(base > 0.05 and base < 0.4 and lum(1.0,0.0) > 0.75,label+" asphalt %.2f and paint render"%base)
	# Crosswalk: 13 whole bars about 0.167 m wide on a 0.383 m pitch (0.5 m bars,
	# 0.65 m gaps real), centred on the road, running 0.3–1.7 m along traffic.
	var asphalt := linear(25,-2.2)
	var paint := linear(1.0,0.0)
	for x in [1.0,49.0]:
		var bars := coverage_runs(x,asphalt,paint)
		var widths_ok := true
		for r: Vector2 in bars: widths_ok = widths_ok and absf(r.y-0.167) < 0.03
		check(bars.size() == 13 and widths_ok,label+" crosswalk bars (centre, width) at x=%.0f: %s"%[x,str(bars)])
		if bars.size() == 13:
			var pitch: float = (bars[12].x-bars[0].x)/12.0
			check(absf(pitch-0.383) < 0.01 and absf(bars[6].x) < 0.02,label+" bar pitch %.3f, centre bar on the axis"%pitch)
	var along := runs(Vector2(0,0),Vector2(3,0))
	check(along.size() == 1 and absf(along[0].x-0.3) < 0.08 and absf(along[0].y-1.7) < 0.08,label+" centre bar runs 0.3–1.7 m along traffic: "+str(along))
	# Stop line 1.95–2.08 m from each end on the inbound half only.
	check(fraction(Vector2(2.015,-2.3),Vector2(2.015,-0.15)) > 0.95,label+" start stop line spans the inbound half")
	check(fraction(Vector2(2.015,0.15),Vector2(2.015,1.05)) < 0.05 and fraction(Vector2(2.015,1.35),Vector2(2.015,2.3)) < 0.05,label+" no start stop line on the outbound half")
	check(fraction(Vector2(47.985,0.15),Vector2(47.985,2.3)) > 0.95 and fraction(Vector2(47.985,-2.3),Vector2(47.985,-1.35)) < 0.05,label+" end stop line on its inbound half")
	# Inbound dividers are solid for 10 m behind the stop line, then 3:5 dashes.
	check(fraction(Vector2(2.3,-1.2),Vector2(11.7,-1.2)) > 0.97,label+" solid inbound divider before the start stop line")
	check(fraction(Vector2(47.7,1.2),Vector2(38.3,1.2)) > 0.97,label+" solid inbound divider before the end stop line")
	var dashed := fraction(Vector2(13.0,-1.2),Vector2(36.8,-1.2))
	check(absf(dashed-0.375) < 0.06,label+" mid-block divider dash ratio %.3f"%dashed)
	var outbound := fraction(Vector2(2.3,1.2),Vector2(11.83,1.2))
	check(absf(outbound-0.375) < 0.08,label+" outbound divider stays dashed near the start: %.3f"%outbound)
	check(fraction(Vector2(0.0,-1.2),Vector2(1.9,-1.2)) < 0.8 and lum(1.85,-1.2) < 0.4,label+" lines end at the stop line")
	# Double yellow centre, 0.09 m apart.
	var yellow := image.get_pixel(floori((25-CENTER_X)*ppm+W*0.5),floori(0.07*ppm+H*0.5))
	check(yellow.r > 0.7 and yellow.r-yellow.b > 0.25 and lum(25,0.0) < 0.4,label+" double yellow centre line")
	# Arrows: tips 0.5 m behind the stop line; the start's inner lane (z=-0.6)
	# combines straight and left, the outer (z=-1.8) is straight, repeated 7.5 m back.
	check(painted(3.6,-1.8) and painted(2.65,-1.8) and not painted(2.65,-1.55),label+" straight arrow shaft and head")
	# Its shaft shifts 7.5 cm right so the left head stops 4 cm short of the yellow.
	check(painted(3.18,-0.3) and not painted(3.18,-0.9) and painted(3.6,-0.675),label+" combined straight + left arrow turns left only")
	var gap := 0.0
	var centre_gap := 0.0
	for i in 9:
		gap = maxf(gap,linear(3.0+i*0.05,-0.125))
		centre_gap = maxf(centre_gap,lum(3.0+i*0.05,-0.02))
	check(gap < 0.5 and centre_gap < 0.4,label+" left head clears the double yellow (%.2f, gap %.2f)"%[gap,centre_gap])
	check(painted(11.1,-1.8) and painted(11.1,-0.675),label+" arrows repeat on the long approach")
	check(not painted(3.6,0.6) and not painted(3.6,1.8),label+" no arrows on the outbound half")
	# End: the outer lane (z=1.8) combines straight and right (driver's right is +z).
	check(painted(46.82,2.1) and not painted(46.82,1.5) and painted(46.4,0.6),label+" outer end lane combines straight + right")
	# Warning diamonds 5.5 m and 17 m behind the stop line in every inbound lane.
	for c in [7.45,18.95]:
		for z in [-0.6,-1.8]:
			check(painted(c-0.75,z) and painted(c+0.75,z) and painted(c,z-0.25) and painted(c,z+0.25) and not painted(c,z,0.02),
				label+" hollow diamond at x=%.2f z=%.1f"%[c,z])
		check(not painted(c,0.6) and not painted(c,1.8),label+" no diamond on the outbound half at x=%.2f"%c)
	check(painted(50-7.45+0.75,0.6) and not painted(50-7.45,0.6,0.02) and painted(50-7.45-0.75,0.6),label+" diamond before the end crosswalk")

func one_way_checks() -> void:
	var label := "one-way"
	var centres := [-3.0,-1.8,-0.6,0.6,1.8,3.0]
	# Departure end: no stop line or arrows; lines stop at the 1.7 m crosswalk edge.
	for c: float in centres:
		check(fraction(Vector2(2.015,c-0.35),Vector2(2.015,c+0.35)) < 0.05,label+" no stop line at the departure end (lane %.1f)"%c)
		check(fraction(Vector2(2.2,c),Vector2(6.0,c)) < 0.05,label+" no arrows at the departure end (lane %.1f)"%c)
	check(fraction(Vector2(0.02,-1.2),Vector2(0.28,-1.2)) < 0.05 and fraction(Vector2(1.76,-1.2),Vector2(1.9,-1.2)) > 0.9,
		label+" divider stops at the departure crosswalk and resumes past 1.7 m")
	# Travel end: the stop line spans the whole road.
	check(fraction(Vector2(47.985,-3.5),Vector2(47.985,3.5)) > 0.95,label+" travel-end stop line spans every lane")
	for z in [-2.4,-1.2,0.0,1.2,2.4]:
		check(fraction(Vector2(47.7,z),Vector2(38.3,z)) > 0.97,label+" divider at %.1f solid before the stop line"%z)
	# Straight only: inner lanes get arrows, the outer lanes stay bare (no stub).
	for c in [-1.8,-0.6,0.6,1.8]:
		check(painted(46.3,c) and painted(38.8,c),label+" straight arrows in lane %.1f"%c)
	for c in [-3.0,3.0]:
		check(fraction(Vector2(45.5,c),Vector2(47.8,c)) < 0.05 and fraction(Vector2(37.9,c),Vector2(40.4,c)) < 0.05,
			label+" no arrow paint in movement-less lane %.1f"%c)
		check(painted(41.8,c) and painted(43.3,c),label+" warning diamond still in lane %.1f"%c)

func left_hand_checks() -> void:
	var label := "left-hand"
	# Start: mid-block crosswalk, no movements: the stop line covers the inbound
	# (here z > 0) half only and no arrows are drawn.
	check(fraction(Vector2(2.015,0.15),Vector2(2.015,2.45)) > 0.95,label+" mid-block stop line on the inbound half")
	check(fraction(Vector2(2.015,-0.15),Vector2(2.015,-2.4)) < 0.05,label+" no mid-block stop line on the outbound half")
	for c in [0.675,2.025]:
		check(fraction(Vector2(2.2,c),Vector2(5.5,c)) < 0.05,label+" no arrows before the mid-block crossing (lane %.3f)"%c)
	check(painted(6.7,0.675) and painted(8.2,0.675) and not painted(7.45,0.675,0.02),label+" warning diamond before the mid-block crossing")
	check(fraction(Vector2(2.3,1.35),Vector2(10.6,1.35)) > 0.97,label+" solid inbound divider before the mid-block stop line")
	# End (0.55 m stop line): on a 20 m road the solid stretch is 45% of it (9 m),
	# so a 2.857 m period leaves its 9.11–10.89 m station gap dashed past 9.45 m.
	check(fraction(Vector2(19.3,-1.35),Vector2(10.7,-1.35)) > 0.97,label+" solid inbound divider before the end stop line")
	check(fraction(Vector2(9.55,-1.35),Vector2(10.2,-1.35)) < 0.1,label+" solid stretch capped at 45% of a short road")
	# The kerb lane (driver's left) combines straight + left; the left head
	# stops before the edge line drawn 16 cm inside the border.
	check(painted(18.2,-2.35) and painted(18.6,-1.93),label+" kerb lane left head and shaft")
	var clear := 0.0
	for i in 9: clear = maxf(clear,linear(18.0+i*0.05,-2.475))
	check(clear < 0.5 and fraction(Vector2(17.0,-2.54),Vector2(19.0,-2.54)) > 0.97,label+" left head clears the edge line (%.2f)"%clear)

func yellow_at(x: float, z: float) -> bool:
	var value := image.get_pixel(floori((x-CENTER_X)*ppm+W*0.5),floori(z*ppm+H*0.5))
	return value.r > 0.7 and value.r-value.b > 0.25

## Single: one 5 cm yellow on the axis, bare at ±4 cm and at ±7 cm, where
## the double yellow (5 cm lines 9 cm apart) lies. Studs stay on the centre
## either way.
func two_lane_checks(single: bool) -> void:
	var label := "two-lane single" if single else "two-lane edged"
	for x in [21.0,26.0,30.5]:
		if single:
			check(yellow_at(x,0.0) and yellow_at(x,0.015) and yellow_at(x,-0.015),label+" yellow on the axis at x=%.1f"%x)
			check(not yellow_at(x,0.04) and not yellow_at(x,-0.04) and lum(x,0.04) < 0.4 and lum(x,-0.04) < 0.4,label+" yellow narrower than 7.5 cm at x=%.1f"%x)
			check(not yellow_at(x,0.07) and not yellow_at(x,-0.07) and lum(x,0.07) < 0.4 and lum(x,-0.07) < 0.4,label+" no paint at ±0.07 at x=%.1f"%x)
		else:
			check(yellow_at(x,0.07) and yellow_at(x,-0.07),label+" double yellow at ±0.07 at x=%.1f"%x)
			check(not yellow_at(x,0.0) and lum(x,0.0) < 0.4,label+" bare axis between the double yellow at x=%.1f"%x)
	# A stud every two dash periods (4.76 m): grey, not yellow, on the axis.
	var stud := image.get_pixel(floori((5.0*100.0/21.0-CENTER_X)*ppm+W*0.5),floori(H*0.5))
	check(stud.r > 0.45 and stud.r-stud.b < 0.15,label+" stud on the centre at x=23.81: "+str(stud))

## Near perspective shot over a crosswalk with surface effects on: quality 2
## chips, grimes and abrades the paint but keeps the bars, quality 0 skips it,
## and repeated frames match (nothing varies with time).
func detail_checks(camera: Camera3D) -> void:
	use({"surface_effect_distance":96.0})
	camera.projection = Camera3D.PROJECTION_PERSPECTIVE; camera.fov = 70.0; camera.near = 0.05
	camera.position = Vector3(1.0,0.8,0.6)
	material.set_shader_parameter("detail_quality",0)
	await capture("detail-q0")
	var low := image
	material.set_shader_parameter("detail_quality",2)
	await capture("detail-q2")
	var high := image
	await capture("detail-q2-again")
	var bar_low := 0.0
	var bar_high := 0.0
	var bar_pixels := 0
	var dented := 0
	var gap_low := 0.0
	var gap_high := 0.0
	var gap_pixels := 0
	var repeat := 0.0
	for py in range(0,H,2):
		for px in range(0,W,2):
			var a := value_of(low,px,py)
			var b := value_of(high,px,py)
			repeat = maxf(repeat,absf(value_of(image,px,py)-b))
			if a > 0.6:
				bar_low += a; bar_high += b; bar_pixels += 1
				if b < a-0.08: dented += 1
			elif a < 0.3:
				gap_low += a; gap_high += b; gap_pixels += 1
	check(bar_pixels > 1000 and gap_pixels > 1000,"detail shot sees bars and gaps (%d, %d)"%[bar_pixels,gap_pixels])
	if bar_pixels > 0 and gap_pixels > 0:
		var share := float(dented)/float(bar_pixels)
		check(bar_high >= 0.85*bar_low,"q2 bars keep their coverage (%.3f of q0)"%(bar_high/maxf(bar_low,1e-6)))
		check(share > 0.005 and share < 0.4,"q2 chips and abrades some bar pixels (%.3f), q0 does not"%share)
		check(gap_high < 0.995*gap_low,"q2 grimes the gaps (%.4f vs %.4f)"%[gap_high/gap_pixels,gap_low/gap_pixels])
	check(repeat < 0.01,"detail paint is stable between frames (max change %.4f)"%repeat)

func value_of(source: Image, px: int, py: int) -> float:
	var value := source.get_pixel(px,py)
	return value.srgb_to_linear().r if encoded else value.r
