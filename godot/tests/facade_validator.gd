extends SceneTree
## Generated building facades: key classification, twelve base colours and the
## record palettes shared with the distant mesh, per-record wall finishes, floor pitch from the recorded
## map scale, shader uniforms, the solid top (CUSTOM0) binding, the
## commercial storey/shop-front layout, the procedural storefront signs
## (facade_signs.gdshaderinc) and window lettering, and the roof finishes
## (flat roofs square to their packed roof frame, pitched faces down their
## slope) keeping the palette's mean colour.
const RENDER := preload("../chunk_renderer.gd")
const CACHE := preload("../render_resource_cache.gd")
const MATERIALS := preload("../environment_materials.gd")
var failed := false

class Lease extends RefCounted:
	var bytes: int
	func _init(amount: int) -> void: bytes = amount
	func track(_value: Object) -> void: pass
	func seal() -> void: pass

func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)

func _initialize() -> void: run.call_deferred()

# GDScript mirror of facade_storeys / facade_row / facade_storefront in atmosphere_surface.gdshader.
static func storeys(height: float, pitch: float) -> Vector2:
	if height <= 0.0: return Vector2(pitch,pitch)
	var ground := minf(1.5*pitch,height)
	var upper := height-ground
	return Vector2(ground,0.0 if upper < 0.75*pitch else upper/maxf(1.0,floorf(upper/pitch+0.01)))

static func row(v: float, fit: Vector2, height: float, pitch: float) -> float:
	if v < fit.x: return v/fit.x
	if height <= 0.0: return 1.0+(v-fit.x)/pitch
	if v < height: return 1.0+(v-fit.x)/fit.y if fit.y > 0.0 else 1.0
	return 1.0+(roundf((height-fit.x)/fit.y) if fit.y > 0.0 else 0.0)+(v-height)/pitch

## Solid height as the shader sees it: two 1000-offset f32 values subtracted.
static func f32_height(base: float, height: float) -> float:
	var offset := PackedFloat32Array([1000.0+base+height,1000.0+base])
	return offset[0]-offset[1]

static func storefront(s: float, w: float, ground: float, width: float, door: float, mullions: float, pitch: float) -> int:
	if w >= 0.98: return 0
	if w >= 0.78: return 7
	if w >= 0.765: return 6
	var frame := 0.025*pitch
	var d := absf(s-door)
	if w >= 0.08 and w < 0.72 and d < 0.15*pitch: return 3 if d < 0.15*pitch-frame and w < 0.72-frame/ground else 4
	if w < 0.08: return 1
	if s < 0.05*pitch or s > width-0.05*pitch: return 0
	if w >= 0.10 and w < 0.68:
		var pane := width*0.5/(mullions+1.0)
		return 4 if absf(s-roundf(s/pane)*pane) < 0.012*pitch else 2
	if w >= 0.70 and w < 0.76: return 5
	return 4

## Runs of `part` along the slot at height w between s0 and s1.
static func runs(part: int, w: float, ground: float, width: float, door: float, mullions: float, pitch: float, s0: float, s1: float) -> int:
	var count := 0
	var inside := false
	var s := s0
	while s < s1:
		var hit := storefront(s,w,ground,width,door,mullions,pitch) == part
		if hit and not inside: count += 1
		inside = hit
		s += 0.0005
	return count

func storefront_layout() -> void:
	var pitch := 1.0
	check(storeys(-1.0,pitch) == Vector2(pitch,pitch) and is_equal_approx(row(2.5,storeys(-1.0,pitch),-1.0,pitch),2.5),"unknown solid top keeps floor-pitch rows (the pre-fitting look)")
	# Expected whole upper storeys per height; 0 is a plain parapet.
	var expected := {2.5:1,2.9:1,3.0:1,4.2:2,4.7:3,6.0:4,7.35:5,12.0:10,30.5:29}
	for height: float in expected:
		for base: float in [0.0,37.3,412.77]:
			var h := f32_height(base,height)
			var fit := storeys(h,pitch)
			var count := (h-fit.x)/fit.y
			check(is_equal_approx(fit.x,1.5) and absf(count-roundf(count)) < 1e-3 and int(roundf(count)) == expected[height] and fit.y >= 0.99*pitch and fit.y < 2.0*pitch,"%d whole upper storeys fit a %.2f m solid at base %.2f"%[expected[height],height,base])
			check(absf(row(h-1e-4,fit,h,pitch)-row(h+1e-4,fit,h,pitch)) < 1e-3,"rows continue at floor pitch above a %.2f m top"%height)
	for height: float in [2.0,2.2]:
		for base: float in [0.0,37.3,412.77]:
			var h := f32_height(base,height)
			check(storeys(h,pitch).y == 0.0 and row(h-0.1,storeys(h,pitch),h,pitch) == 1.0,"a %.2f m commercial solid keeps a plain parapet above its shop storey"%height)
	for height: float in [3.5,4.5]:
		for base: float in [0.0,37.3,412.77,1999.99]:
			check(absf(storeys(f32_height(base,height),pitch).y-pitch) < 1e-3,"exact floor multiples above the shop storey stay whole (%.1f m at base %.2f)"%[height,base])
	check(is_equal_approx(storeys(1.0,pitch).x,1.0) and storeys(1.0,pitch).y == 0.0,"a single-floor shop fits its storey to the solid")
	check(is_equal_approx(row(2.5,Vector2(1.5,0.0),1.8,pitch),1.7),"a taller part above a merged solid's top keeps floor-pitch rows")
	var ground := 1.5*pitch
	for width: float in [1.28,1.6,2.08]:
		for choice in 3:
			var door := width*(0.22+0.28*choice)
			for mullions: float in [2.0,3.0]:
				var label := " (slot %.2f, door %d, %d mullions)"%[width,choice,int(mullions)]
				check(runs(3,0.4,ground,width,door,mullions,pitch,0.0,width) == 1,"one door per slot"+label)
				check(runs(3,0.04,ground,width,door,mullions,pitch,0.0,width) == 0 and storefront(door,0.04,ground,width,door,mullions,pitch) == 1,"stall riser below the door"+label)
				check(storefront(door,0.9,ground,width,door,mullions,pitch) == 7 and storefront(door,0.77,ground,width,door,mullions,pitch) == 6,"fascia and canopy shadow over the door"+label)
				if choice == 1: continue # A centred door spans both bays.
				var free_bay := Vector2(width*0.5,width) if choice == 0 else Vector2(0.0,width*0.5)
				check(runs(4,0.4,ground,width,door,mullions,pitch,free_bay.x+0.02,free_bay.y-0.02) == int(mullions),"display windows hold the mullions of a free bay"+label)
				check(runs(2,0.4,ground,width,door,mullions,pitch,free_bay.x,free_bay.y) == int(mullions)+1 and storefront(free_bay.x+width*0.25/(mullions+1.0),0.73,ground,width,door,mullions,pitch) == 5,"display panes and transom glass"+label)
			# Panels (0.16 storeys) and channel letters (0.18) centred on the fascia
			# at 0.88, 80-94 % of the slot wide and at most 8:1.
			for height_share: float in [0.16,0.18]:
				var sign_height := height_share*ground
				for share: float in [0.80,0.94]:
					var sign_width := minf(width*share,8.0*sign_height)
					var low := 0.88-0.5*height_share
					var high := 0.88+0.5*height_share
					check(sign_width <= width*0.94 and sign_width/sign_height <= 8.0 and storefront(width*0.5,low+0.001,ground,width,width*0.5,2.0,pitch) == 7 and storefront(width*0.5,high-0.001,ground,width,width*0.5,2.0,pitch) == 7,"sign within the fascia and slot (slot %.2f, %.2f high, %.2f wide)"%[width,height_share,share])
	var code: String = MATERIALS.new().shader.code
	for marker: String in ["if(height<=0.0) return vec2(pitch);","float ground = min(1.5*pitch,height);","upper<0.75*pitch ? 0.0 : upper/max(1.0,floor(upper/pitch+0.01))","if(height<=0.0) return 1.0+(v-storeys.x)/pitch;","if(v<height) return storeys.y>0.0 ? 1.0+(v-storeys.x)/storeys.y : 1.0;","return 1.0+(storeys.y>0.0 ? round((height-storeys.x)/storeys.y) : 0.0)+(v-height)/pitch;","facade_row(v,storeys,solid_height,floor_pitch)","facade_kind==3 && storeys.y<=0.0 && v<solid_height) { pane_mask = 0.0; spandrel = 0.0; }"]:
		check(code.contains(marker),"shader storeys match the mirror: "+marker)
	for marker: String in ["if(w>=0.78) return 7;","if(w>=0.765) return 6;","d<0.15*pitch-frame && w<0.72-frame/ground) ? 3 : 4;","if(w<0.08) return 1;","if(w>=0.10 && w<0.68) {","abs(s-round(s/pane)*pane)<0.012*pitch ? 4 : 2;","if(w>=0.70 && w<0.76) return 5;","slot_width*(0.22+0.28*float(slot_layout%3u)),2.0+float((slot_layout>>2u)&1u)","float sign_height = (sign_type==FACADE_SIGN_CHANNEL ? 0.18 : 0.16)*storeys.x;","float sign_width = min(slot_width*(0.80+0.14*facade_unit(sign_bits^2654435769u)),8.0*sign_height);","vec2 local = vec2(slot_s-(slot_width-sign_width)*0.5,0.88*storeys.x+0.5*sign_height-v)/sign_height;"]:
		check(code.contains(marker),"shader shop front matches the mirror: "+marker)

## Both upload paths bind the same attributes: every generated facade kind the
## solid seed (UV2) and top (CUSTOM0, one float: commercial storeys and the
## parapet coping of all kinds) without UV (world-space materials); other
## batches keep UV and no facade arrays.
func top_binding() -> void:
	var vertices := PackedVector3Array()
	for x: float in [0.0,40.0]: vertices.append_array(PackedVector3Array([Vector3(x,0,0),Vector3(x+2,0,0),Vector3(x,3,0),Vector3(x+2,0,0),Vector3(x+2,3,0),Vector3(x,3,0)]))
	vertices.append_array(PackedVector3Array([Vector3(80,0,0),Vector3(81,0,0),Vector3(80,0,-1)]))
	var normals := PackedVector3Array(); var uv := PackedVector2Array(); var seeds := PackedVector2Array(); var tops := PackedFloat32Array()
	for i in vertices.size():
		normals.append(Vector3.BACK if i < 12 else Vector3.UP)
		uv.append(Vector2(vertices[i].x,vertices[i].y))
		seeds.append(Vector2(0.25 if i < 6 else 0.5,1000.0) if i < 12 else Vector2.ZERO)
		tops.append(1003.0 if i < 12 else 0.0)
	var chunk := {"cell":Vector2i.ZERO,"triangles":[{"surface":"concrete","object_id":"shop"},{"surface":"concrete","object_id":"shop"},{"surface":"concrete","object_id":"flat"},{"surface":"concrete","object_id":"flat"},{"surface":"grass","object_id":"ground"}],"objects":[],
		"scene_vertices":vertices,"scene_normals":normals,"wall_uv":uv,"facade_seeds":seeds,"facade_tops":tops,"presentation":{"proxy_materials":{"shop":"concrete:commercial","flat":"concrete:residential"},"facade_floor_cm":100}}
	check(RENDER.FACADE_TOP_FORMAT == Mesh.ARRAY_CUSTOM_R_FLOAT << Mesh.ARRAY_FORMAT_CUSTOM0_SHIFT,"solid tops upload as one float")
	# Expected per mesh by its first vertex x: [UV, UV2, CUSTOM0].
	var expected := {0.0:[false,true,true],40.0:[false,true,true],80.0:[true,false,false]}
	for prepared in [false,true]:
		var input := chunk.duplicate(true)
		if prepared:
			input.render_batches = RENDER.PLAN.prepare_batches(input)
			check(input.render_batches.size() == 3,"three prepared batches")
			for batch: Dictionary in input.render_batches:
				var first := int(batch.vertices[0].x/40.0)*6
				var count: int = batch.vertices.size()
				var want: Array = expected.get(batch.vertices[0].x,[])
				check(not want.is_empty() and batch.uv == (uv.slice(first,first+count) if want[0] else PackedVector2Array()) and batch.uv2 == (seeds.slice(first,first+count) if want[1] else PackedVector2Array()) and batch.custom0 == (tops.slice(first,first+count) if want[2] else PackedFloat32Array()),"prepared %s batch carries %s"%[batch.key,want])
		var parent := Node3D.new(); root.add_child(parent)
		var job := RENDER.begin(input,parent)
		for _step in 50:
			if RENDER.advance(job): break
		var found := 0
		for node: Node in job.root.get_children():
			if node is not MeshInstance3D or node.mesh is not ArrayMesh: continue
			var format: int = node.mesh.surface_get_format(0)
			var x: float = node.mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX][0].x
			if not expected.has(x): continue
			found += 1
			var custom := format & Mesh.ARRAY_FORMAT_CUSTOM0 != 0 and (format >> Mesh.ARRAY_FORMAT_CUSTOM0_SHIFT) & Mesh.ARRAY_FORMAT_CUSTOM_MASK == Mesh.ARRAY_CUSTOM_R_FLOAT
			var have := [format & Mesh.ARRAY_FORMAT_TEX_UV != 0,format & Mesh.ARRAY_FORMAT_TEX_UV2 != 0,custom]
			check(have == expected[x] and (format & Mesh.ARRAY_FORMAT_CUSTOM0 != 0) == custom,"mesh at x %d binds %s (prepared %s)"%[int(x),expected[x],prepared])
		check(job.error.is_empty() and found == 3,"three facade test meshes (prepared %s): %s"%[prepared,job.error])
		if not job.done: RENDER.cancel(job)
		parent.queue_free(); job.clear()
	await process_frame
	await process_frame

# GDScript mirror of facade_pcg / facade_unit and the per-record finish pick.
static func pcg(v: int) -> int:
	var s := (v * 747796405 + 2891336453) & 0xFFFFFFFF
	var w := (((s >> ((s >> 28) + 4)) ^ s) * 277803737) & 0xFFFFFFFF
	return ((w >> 22) ^ w) & 0xFFFFFFFF

static func finish_of(look: PackedVector4Array, record: int) -> int:
	var pick := float(pcg(record ^ 1759714724) >> 8) / 16777216.0
	for i in look.size():
		if pick < look[i].z: return i
	return look.size() - 1

## Wall finishes: the palette table in shader slots, curtain walls only on
## commercial keys, one deterministic finish per record near its weight.
func finishes(job: Dictionary, context: RefCounted) -> void:
	var table: Dictionary = RENDER.FACADE_PALETTE.data.finishes
	var code: String = MATERIALS.new().shader.code
	check(code.contains("varying flat int finish_slot;") and code.contains("float finish_pick = facade_unit(uint(int(floor(solid_seed*4096.0)))^1759714724u);") and code.contains("if(i<facade_finish_count && finish_pick<facade_finish_look[i].z) { finish_slot=i; break; }") and code.contains("int finish = facade_record && facade_surface ? finish_slot : -1;"),"vertex() picks the finish once per record with the integer hash (mirrored here)")
	check(code.contains("textureGrad(facade_tiles,") and code.contains("facade_luminance(detail.r)") and code.contains("facade_bump(detail.ba,wall_tangent)"),"walls sample the facade relief tiles with analytic gradients")
	check(code.contains("if(facade_relief && shop_part<=1 && tone_relief>0.0) {"),"the facade tile is fetched only where relief shows")
	check(code.contains("detail_enabled && !facade_relief && !curtain"),"finished walls and curtain walls skip the racing tile")
	check(code.contains("if(finish>=0 && shop_part<=1) {"),"the finish response spares frames, canopy, fascia and signs")
	check(code.contains("float window = facade_span(cell.x,pane.x,pane.y,cell_fw.x)*facade_span(cell.y,pane.z,pane.w,cell_fw.y);"),"panes, mullions and spandrels are box-filtered over the pixel footprint")
	check(code.contains("EMISSION += glass_reflection;") and code.contains("*facade_window*weather.w;"),"daylight glass reflection is light, scaled by the environment's daylight")
	# Known answers pin the hash itself (an independent PCG reference), so the
	# mirror and the shader source cannot drift together unnoticed.
	check(pcg(0 ^ 1759714724) == 3561045559 and pcg(1 ^ 1759714724) == 1454332419 and pcg(4095 ^ 1759714724) == 462394992,"facade_pcg known answers")
	var commercial: Dictionary = RENDER.facade_finishes("concrete:commercial")
	check([0,1,2,4095].map(func(record): return commercial.names[finish_of(commercial.look,record)]) == ["curtain","concrete","plaster","plaster"],"commercial finish known answers for records 0, 1, 2 and 4095")
	for key: String in ["concrete:residential","concrete:commercial","concrete:industrial","concrete:public","brick:residential","brick:commercial","wood:residential","concrete:school"]:
		var bound: Dictionary = RENDER.facade_finishes(key)
		var fields := key.split(":")
		var weights: Dictionary = table.weights.get(fields[0] if fields[0] in ["brick","wood"] else key,{})
		if RENDER.facade_kind(key) == 0: weights = {}
		var material: ShaderMaterial = RENDER._generated_material(job,key,{"facade_floor_cm":100})
		if RENDER.facade_kind(key) == 0:
			check(material.get_shader_parameter("facade_finish_count") == null,"no finish without a facade: "+key)
			continue
		check(material.get_shader_parameter("facade_finish_count") == bound.names.size() and material.get_shader_parameter("facade_finish") == bound.finish and material.get_shader_parameter("facade_finish_look") == bound.look,"walls bind their finish slots: "+key)
		check(material.get_shader_parameter("facade_tiles") == context.facades and material.get_shader_parameter("facade_tiles_enabled") == true,"walls bind the shared facade tile array: "+key)
		check(bound.names.size() == weights.size() - (1 if weights.has("curtain") and RENDER.facade_kind(key) != 3 else 0) and bound.names.size() <= 6,"finish count follows the weights: "+key)
		check(bound.names.is_empty() or is_equal_approx(bound.look[bound.look.size()-1].z,1.0),"cumulative weights end at 1: "+key)
		var total := 0.0
		for name: String in bound.names: total += float(weights[name])
		var counts := {}
		for i in bound.names.size():
			var kind: Dictionary = table.kinds[bound.names[i]]
			check(bound.finish[i].is_equal_approx(Vector4(float(MATERIALS.FACADE_LAYERS.find(kind.tile)),float(kind.tile_floors),float(kind.roughness),float(kind.metallic))) and is_equal_approx(bound.look[i].x,float(kind.specular)),"finish %s slot follows the table: %s"%[bound.names[i],key])
			check(bound.finish[i].x >= 0.0 if bound.look[i].y == 0.0 else bound.finish[i].x == -1.0,"finish %s names a facade tile layer (curtain walls none)"%bound.names[i])
			check(bound.finish[i].w <= 0.1001,"finish %s keeps the wall's diffuse brightness (metallic at most 0.1)"%bound.names[i])
			check(bound.look[i].y == (1.0 if kind.get("curtain_wall",false) else 0.0) and (bound.look[i].y == 0.0 or RENDER.facade_kind(key) == 3),"curtain walls only on commercial walls: "+key)
		for record in 4096:
			var pick := finish_of(bound.look,record)
			counts[pick] = int(counts.get(pick,0)) + 1
		for i in bound.names.size():
			var share := float(counts.get(i,0)) / 4096.0
			check(absf(share - float(weights[bound.names[i]]) / total) < 0.03,"record finish shares follow the weights (%s %s %.3f)"%[key,bound.names[i],share])
	check(RENDER.facade_finishes("wood:residential").names.is_empty(),"wood walls keep their racing detail tile")

## Mean output luminance of the rows showing wall heights low..high metres on
## the finish_pixels quad (2 m tall from 1 m, centred columns only).
static func band_luminance(picture: Image, low: float, high: float) -> float:
	var total := 0.0
	var count := 0
	var h := picture.get_height()
	var w := picture.get_width()
	for y in range(ceili((3.0-high)/2.0*h),floori((3.0-low)/2.0*h)):
		for x in range(w/2-int(h*0.4),w/2+int(h*0.4)):
			var c := picture.get_pixel(x,y)
			total += c.r*0.2126+c.g*0.7152+c.b*0.0722
			count += 1
	return total/maxf(count,1)

## Rendered runs only: the shader's own pick agrees with finish_of on records
## of every commercial finish (curtain walls show dark spandrel glass where the
## other finishes show wall), daylight glass reflection is added light, each
## wall band keeps its brightness near and beyond the effect distance, and far
## walls of every finish are identical.
func finish_pixels(job: Dictionary, context: RefCounted) -> void:
	if DisplayServer.get_name() == "headless": return
	root.size = Vector2i(128,128)
	var scene := Node3D.new(); root.add_child(scene)
	var world := WorldEnvironment.new(); world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_color = Color.WHITE
	world.environment.ambient_light_energy = 1.0
	world.environment.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED
	scene.add_child(world)
	var camera := Camera3D.new(); camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = 2.0; camera.far = 1000.0
	scene.add_child(camera); camera.current = true
	var material: ShaderMaterial = RENDER._generated_material(job,"concrete:commercial",{"facade_floor_cm":100})
	var bound: Dictionary = RENDER.facade_finishes("concrete:commercial")
	var node := MeshInstance3D.new(); scene.add_child(node)
	node.material_override = material
	var far := []
	# One record per finish, all with body entry 0 and the same jitter.
	for record in [0,8,16,24,40]:
		var finish: String = bound.names[finish_of(bound.look,record)]
		var arrays := []; arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = PackedVector3Array([Vector3(-1,1,0),Vector3(1,1,0),Vector3(1,3,0),Vector3(-1,1,0),Vector3(1,3,0),Vector3(-1,3,0)])
		var normals := PackedVector3Array(); var seeds := PackedVector2Array()
		for i in 6: normals.append(Vector3.BACK); seeds.append(Vector2((float(record)+0.5)/4096.0,1000.0))
		arrays[Mesh.ARRAY_NORMAL] = normals; arrays[Mesh.ARRAY_TEX_UV2] = seeds
		var mesh := ArrayMesh.new(); mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays)
		node.mesh = mesh
		var sample := {}
		for distance in [4.0,250.0]:
			camera.position = Vector3(0,2,distance)
			for daylight in [1.0,0.0]:
				context.update(43200,0,0,0.8,1080,0.18,daylight)
				for frame in 4: await process_frame
				await RenderingServer.frame_post_draw
				var picture := root.get_texture().get_image()
				# The wall band under every pane variant (0.02–0.09 storey) and a glass band.
				sample["%d/%d"%[int(distance),int(daylight)]] = Vector2(band_luminance(picture,1.02,1.09),band_luminance(picture,1.45,1.6))
		print("FACADE_FINISH record=%d finish=%s near_day=%s near_dark=%s far=%s" % [record,finish,sample["4/1"],sample["4/0"],sample["250/1"]])
		var near: Vector2 = sample["4/1"]
		check((near.x < 0.45) == (finish == "curtain"),"rendered pick matches finish_of for record %d (%s, wall band %.3f)"%[record,finish,near.x])
		# Daylight glass trades body tint for reflected light: a curtain wall's
		# glass band brightens clearly, punched walls never darken.
		check(near.y > sample["4/0"].y+(0.05 if finish == "curtain" else -0.005),"daylight glass reflects as added light (record %d)"%record)
		if finish != "curtain": check(absf(near.x/maxf(sample["250/1"].x,0.01)-1.0) < 0.15,"%s wall keeps its brightness beyond the effect distance (%.3f near, %.3f far)"%[finish,near.x,sample["250/1"].x])
		far.append(sample["250/1"])
	for value: Vector2 in far: check(value.distance_to(far[0]) < 0.02,"far walls of every finish match")
	node.free()
	scene.queue_free()
	await process_frame

static func picture_stats(picture: Image) -> Array:
	var sum := Vector3.ZERO
	var squares := 0.0
	var count := picture.get_width() * picture.get_height()
	for y in picture.get_height():
		for x in picture.get_width():
			var c := picture.get_pixel(x, y)
			sum += Vector3(c.r, c.g, c.b)
			var l := c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722
			squares += l * l
	var mean := sum / float(count)
	var luma := mean.x * 0.2126 + mean.y * 0.7152 + mean.z * 0.0722
	return [mean, sqrt(maxf(squares / float(count) - luma * luma, 0.0))]

## Roof finishes (roof_finish): flat roofs of green, grey and red records
## (urethane coatings, concrete slabs, membranes, gravel) carrying a packed
## roof frame, and a pitched face of a red record (clay tiles), seen from
## above at 4 m show their pattern and keep the palette's mean colour, which
## is all a roof shows beyond the effect distance.
func roof_pixels(job: Dictionary, context: RefCounted) -> void:
	var code: String = MATERIALS.new().shader.code
	check(code.contains("varying flat float roof_frame;") and code.contains("roof_frame = solid_seed>=0.0 && CUSTOM0.x>3500.0 ? CUSTOM0.x-4000.0 : 0.0;"),"flat roofs read their packed frame (CUSTOM0 above 3500) as a flat varying")
	check(code.contains("RoofFinish roof = roof_finish(world_position,up,position_dx,position_dy,"),"roof finishes take the uniform-flow position derivatives")
	if DisplayServer.get_name() == "headless": return
	root.size = Vector2i(128,128)
	var scene := Node3D.new(); root.add_child(scene)
	var world := WorldEnvironment.new(); world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	world.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	world.environment.ambient_light_color = Color.WHITE
	world.environment.ambient_light_energy = 1.0
	world.environment.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED
	scene.add_child(world)
	var camera := Camera3D.new(); camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.size = 14.0; camera.far = 1000.0
	scene.add_child(camera); camera.current = true
	var node := MeshInstance3D.new(); scene.add_child(node)
	context.update(43200,0,0,0.8,1080,0.18,1.0)
	# [key, record, pitched]: green, grey and red roof entries of several kinds.
	var cases := [["concrete:residential",0,false],["concrete:residential",8,false],["concrete:residential",16,false],["concrete:residential",24,false],
		["concrete:industrial",16,false],["concrete:industrial",48,false],["concrete:public",16,false],["concrete:public",112,false],["brick:residential",0,false],["brick:residential",0,true],["brick:residential",48,true]]
	for case: Array in cases:
		node.material_override = RENDER._generated_material(job,case[0],{"facade_floor_cm":100})
		var pitched: bool = case[2]
		var normal := Vector3(0.0,0.894,0.447).normalized() if pitched else Vector3.UP
		var slope := 0.5 if pitched else 0.0
		var arrays := []; arrays.resize(Mesh.ARRAY_MAX)
		var corners := [Vector3(-8,0,-8),Vector3(8,0,-8),Vector3(8,0,8),Vector3(-8,0,-8),Vector3(8,0,8),Vector3(-8,0,8)]
		var vertices := PackedVector3Array(); var normals := PackedVector3Array(); var seeds := PackedVector2Array(); var tops := PackedFloat32Array()
		for c: Vector3 in corners:
			vertices.append(Vector3(c.x,(8.0-c.z)*slope,c.z)); normals.append(normal)
			seeds.append(Vector2((float(case[1])+0.5)/4096.0,1000.0-3.0))
			tops.append(-1003.0 if pitched else 4000.3)
		arrays[Mesh.ARRAY_VERTEX] = vertices; arrays[Mesh.ARRAY_NORMAL] = normals; arrays[Mesh.ARRAY_TEX_UV2] = seeds; arrays[Mesh.ARRAY_CUSTOM0] = tops
		var mesh := ArrayMesh.new(); mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES,arrays,[],{},RENDER.FACADE_TOP_FORMAT)
		node.mesh = mesh
		var stats := {}
		for distance in [4.0,250.0]:
			camera.position = (Vector3(0.0,4.0,0.0) if pitched else Vector3.ZERO) + normal * distance
			camera.look_at(camera.position - normal,Vector3.FORWARD)
			for frame in 4: await process_frame
			await RenderingServer.frame_post_draw
			stats[distance] = picture_stats(root.get_texture().get_image())
		var near: Vector3 = stats[4.0][0]
		var far: Vector3 = stats[250.0][0]
		var label := "%s record %d%s" % [case[0],case[1]," pitched" if pitched else ""]
		print("ROOF_FINISH %s near=%s sd=%.4f far=%s sd=%.4f" % [label,near,stats[4.0][1],far,stats[250.0][1]])
		check(stats[4.0][1] > stats[250.0][1] + 0.004,"the roof shows its finish near: "+label)
		check(maxf(absf(near.x-far.x),maxf(absf(near.y-far.y),absf(near.z-far.z))) < 0.035,"the roof keeps its palette mean beyond the effect distance: "+label)
	node.free()
	scene.queue_free()
	await process_frame

# GDScript mirror of the sign and lettering choices (atmosphere_surface.gdshader,
# facade_signs.gdshaderinc).
static func cell(v: int) -> int: return (v + 1048576) & 0xFFFFFFFF
static func unit(v: int) -> float: return float(pcg(v) >> 8) / 16777216.0
static func sign_bits(seed: float, slot: int) -> int:
	var record := floori(PackedFloat32Array([seed])[0] * 4096.0)
	return pcg(record ^ 1478450703) ^ pcg(cell(slot) ^ 3266489917)
static func sign_type(bits: int) -> int:
	var t := pcg(bits ^ 2246822507) % 20
	return 0 if t < 9 else (1 if t < 15 else 2)
static func lettered(bits: int, row: int) -> bool: return unit(pcg(bits ^ pcg(cell(row) ^ 620728307))) < 0.30

## Sign and lettering choices: integer hashes of the 12-bit record and the slot
## (steady when the seed's float wobbles by 1e-7), their shares, the lettering
## gate (commercial upper storeys 1-3, glass only) and coping for every kind.
func signs(code: String) -> void:
	check(code.contains("uint sign_bits = facade_record ? facade_pcg(uint(record)^1478450703u)^facade_pcg(facade_cell(slot)^3266489917u) : slot_bits;"),"sign designs hash the record and slot (mirrored here)")
	check(code.contains("int facade_sign_type(uint bits) { uint t = facade_pcg(bits^2246822507u)%20u; return t<9u ? FACADE_SIGN_LIGHTBOX : (t<15u ? FACADE_SIGN_CHANNEL : FACADE_SIGN_TWO_LINE); }"),"sign types follow the mirrored hash")
	check(code.contains("uint letter_bits = facade_pcg(sign_bits^facade_pcg(facade_cell(floor(cell.y))^620728307u));") and code.contains("if(facade_unit(letter_bits)<0.30) {"),"window lettering picks slot storeys with the mirrored hash")
	check(not code.contains("#include"),"every include joins the environment shader")
	var steady := true
	# 1e-7 is 1.7 steps of the 24-bit seed: solids within two steps of a record
	# boundary cannot hold, any other can.
	for record: int in [0,1,777,2048,4095]:
		for solid: int in [2,100,2047,4093]:
			var seed := float((record << 12) | solid) / 16777216.0
			for slot: int in [-3,0,5]:
				var bits := sign_bits(seed,slot)
				for delta: float in [-1e-7,1e-7]:
					var moved := sign_bits(seed+delta,slot)
					if moved != bits or sign_type(moved) != sign_type(bits) or lettered(moved,2) != lettered(bits,2): steady = false
	check(steady,"sign choices hold under +-1e-7 seed changes")
	check(sign_bits(0.5/4096.0,0) != sign_bits(1.5/4096.0,0) and sign_bits(0.5/4096.0,0) != sign_bits(0.5/4096.0,1),"records and slots draw different designs")
	var shown := 0
	var types := [0,0,0]
	var letters := 0
	var storeys := 0
	for record in 2048:
		for slot in range(-4,4):
			var bits := sign_bits((float(record)+0.5)/4096.0,slot)
			if unit(bits) < 0.85:
				shown += 1
				types[sign_type(bits)] += 1
			for row in [1,2,3]:
				storeys += 1
				if lettered(bits,row): letters += 1
	var share := float(shown)/16384.0
	check(absf(share-0.85) < 0.02,"85 %% of slots carry a sign (%.3f)"%share)
	check(absf(float(types[0])/shown-0.45) < 0.03 and absf(float(types[1])/shown-0.30) < 0.03 and absf(float(types[2])/shown-0.25) < 0.03,"lightbox, channel and two-line shares %s"%[types])
	var lettering := float(letters)/storeys
	check(lettering >= 0.25 and lettering <= 0.35,"window lettering on 25-35 %% of slot storeys (%.3f)"%lettering)
	check(code.contains("if(signs_enabled && facade_kind==3 && !shop && !curtain && window>0.0 && cell.y>=1.0 && cell.y<4.0 && (solid_height<=0.0 || v<solid_height)) {") and code.contains("float lettering = letters*window;") and code.contains("facade_pattern *= 1.0-letters;"),"window lettering only on the glass of commercial upper storeys 1-3")
	check(code.contains("if(solid_coping>0.5) {") and code.contains("cap = facade_band(v,solid_height-0.05*floor_pitch,solid_height,fw_v);") and not code.contains("facade_kind==3 && solid_height>0.0"),"parapet coping at the top of every facade kind")
	check(code.contains("solid_coping = solid_height>0.0 && CUSTOM0.x>0.0 ? 1.0 : 0.0;"),"no coping on walls rising above the top (negated top: gable ends, taller merged parts)")

const SIGN_SHADER := """shader_type spatial;
render_mode unshaded, cull_disabled;
%s
uniform uint bits = 0u;
uniform float aspect = 5.0;
uniform bool vertical = false;
uniform float forced = 0.0;
uniform bool ink_out = false;
void fragment() {
	vec2 extent = vertical ? vec2(2.0,aspect+1.0) : vec2(aspect+1.0,2.0);
	vec2 local = UV*extent-0.5;
	vec2 dx = dFdx(local);
	vec2 dy = dFdy(local);
	float fw = forced>0.0 ? forced : max(length(dx),length(dy));
	FacadeSign face = facade_sign_face(local,bits,aspect,fw,vertical);
	vec3 c = ink_out ? vec3(face.ink*face.body) : face.color*face.body;
	ALBEDO = OUTPUT_IS_SRGB ? c : mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c));
}"""

static func picture_mean(picture: Image) -> Color:
	var total := Color(0,0,0,0)
	for y in picture.get_height():
		for x in picture.get_width(): total += picture.get_pixel(x,y)
	return total/float(picture.get_width()*picture.get_height())

## Rendered runs only: the shared sign function alone (any shader can include
## it) draws each type deterministically, and a sign whose glyph runs have
## settled (a forced far footprint) keeps the colour and ink means of its crisp
## near drawing, horizontal and vertical.
func sign_pixels() -> void:
	if DisplayServer.get_name() == "headless": return
	var scene := Node3D.new(); root.add_child(scene)
	var world := WorldEnvironment.new(); world.environment = Environment.new()
	world.environment.background_mode = Environment.BG_COLOR
	world.environment.background_color = Color.BLACK
	scene.add_child(world)
	var camera := Camera3D.new(); camera.projection = Camera3D.PROJECTION_ORTHOGONAL; camera.position = Vector3(0,0,2)
	scene.add_child(camera); camera.current = true
	var shader := Shader.new(); shader.code = SIGN_SHADER % preload("../facade_signs.gdshaderinc").code
	var material := ShaderMaterial.new(); material.shader = shader
	var node := MeshInstance3D.new(); node.material_override = material; scene.add_child(node)
	var designs := []
	var bits := 7
	while designs.size() < 6:
		bits = pcg(bits)
		if designs.filter(func(d): return sign_type(d) == sign_type(bits)).size() < 2: designs.append(bits)
	var cases := []
	for d: int in designs: cases.append([d,false,5.0])
	cases.append([designs[0],true,3.0]); cases.append([designs[2],true,3.0])
	for entry: Array in cases:
		var vertical: bool = entry[1]
		var aspect: float = entry[2]
		var extent := Vector2(2.0,aspect+1.0) if vertical else Vector2(aspect+1.0,2.0)
		root.size = Vector2i(roundi(extent.x*70.0),roundi(extent.y*70.0))
		camera.size = extent.y
		var quad := QuadMesh.new(); quad.size = extent; node.mesh = quad
		material.set_shader_parameter("bits",entry[0]); material.set_shader_parameter("aspect",aspect); material.set_shader_parameter("vertical",vertical)
		var means := {}
		var first: Image
		for pass_ in ["near","near_again","settled","far","near_ink","settled_ink"]:
			material.set_shader_parameter("forced",0.6 if pass_ == "far" else (0.14 if pass_.begins_with("settled") else 0.0))
			material.set_shader_parameter("ink_out",pass_.ends_with("ink"))
			for frame in 3: await process_frame
			await RenderingServer.frame_post_draw
			var picture := root.get_texture().get_image()
			if pass_ == "near": first = picture
			if pass_ == "near_again": check(picture.get_data() == first.get_data(),"sign %d draws deterministically"%entry[0])
			means[pass_] = picture_mean(picture)
		var near: Color = means.near
		var settled: Color = means.settled
		var far: Color = means.far
		var label := "sign %d type %d %s"%[entry[0],sign_type(entry[0]),"vertical" if vertical else "horizontal"]
		print("FACADE_SIGN %s near=%s settled=%s far=%s ink=%.3f/%.3f"%[label,near,settled,far,means.near_ink.r,means.settled_ink.r])
		var body := 1.0/(extent.x*extent.y)*aspect
		check(means.near_ink.r > 0.02*body and means.near_ink.r < 0.6*body,"ink coverage mean in bounds: "+label)
		# Strokes settled on their glyph means (footprint 0.14): ink and colour
		# keep the crisp means; glyph runs settled too (0.6, a sign a few pixels
		# high): the colour still does.
		check(absf(means.near_ink.r-means.settled_ink.r) < 0.02 and maxf(absf(near.r-settled.r),maxf(absf(near.g-settled.g),absf(near.b-settled.b))) < 0.03,"glyphs settling on their means keep the ink and colour: "+label)
		check(maxf(absf(near.r-far.r),maxf(absf(near.g-far.g),absf(near.b-far.b))) < 0.05,"settled far colour keeps the near mean: "+label)
	node.free()
	scene.queue_free()
	await process_frame

func run() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile = {"lights":[]}
	var context: RefCounted = cache.environment_context()
	check(context != null,"shared environment context")
	var job := {"resources":cache,"urban_shader":cache.urban_shader()}
	var names := {}
	for uniform: Dictionary in MATERIALS.new().shader.get_shader_uniform_list(): names[uniform.name] = true
	check(names.has("facade_kind") and names.has("floor_pitch") and names.has("lighting_role") and names.has("signs_enabled") and not names.has("sign_atlas"),"surface shader compiles with the facade uniforms (procedural signs, no sign atlas)")
	check(names.has("facade_body") and names.has("facade_roof") and names.has("facade_accent") and names.has("facade_palette"),"surface shader compiles with the record palette uniforms")
	check(names.has("facade_finish") and names.has("facade_finish_look") and names.has("facade_finish_count") and names.has("facade_tiles") and names.has("facade_tiles_enabled"),"surface shader compiles with the finish and facade tile uniforms")
	check(context.facades is Texture2DArray and context.facades.get_layers() == MATERIALS.FACADE_LAYERS.size(),"shared facade tile array")
	finishes(job,context)
	await finish_pixels(job,context)
	await roof_pixels(job,context)
	# Per-solid choices must not depend on interpolated values: flat seeds and an
	# integer hash keep each sign slot and lit window one choice across its pixels.
	var code: String = MATERIALS.new().shader.code
	check(code.contains("varying flat float solid_seed;") and code.contains("varying flat float solid_base;") and code.contains("varying flat float solid_height;") and code.contains("varying flat float solid_coping;"),"solid seed, base, height and coping flag are flat varyings")
	check(code.contains("abs(CUSTOM0.x)>500.0 && CUSTOM0.x<=3500.0 ? (MODEL_MATRIX * vec4(0.0,abs(CUSTOM0.x)-1000.0,0.0,1.0)).y-solid_base : -1.0;"),"solid height reads the offset top (either sign) in CUSTOM0 and falls back without it or on a roof frame")
	check(code.contains("bool has_sign = signs_enabled && facade_unit(sign_bits)<0.85;") and code.contains("facade_lit = step(0.35,facade_unit("),"sign picks and lit windows use the integer hash")
	check(code.contains("FacadeSign face = facade_sign_face(local,sign_bits,sign_width/sign_height,footprint/sign_height,false);") and code.contains("float footprint = max(length(wall_dx),length(wall_dy));") and not code.contains("sign_atlas"),"signs are drawn by the shared sign function from the uniform-flow footprint")
	signs(code)
	await sign_pixels()
	var kinds := {"concrete:residential":1,"brick:residential":2,"wood:residential":2,"concrete:commercial":3,"brick:commercial":3,"wood:industrial":4,"concrete:public":5,"brick:home":0,"concrete":0,"barrier:wall":0,"rc:wall":0,"builtin:tree":0}
	for key: String in kinds:
		check(RENDER.facade_kind(key) == kinds[key],"facade kind "+key)
		check(RENDER.PLAN.facade_arrays(key) == (0 if kinds[key] == 0 else 1),"every facade kind carries the solid seed and top: "+key)
	var scaled: ShaderMaterial = RENDER._generated_material(job,"concrete:commercial",{"facade_floor_cm":100})
	check(scaled.get_shader_parameter("facade_kind") == 3 and is_equal_approx(scaled.get_shader_parameter("floor_pitch"),1.0) and scaled.get_shader_parameter("lighting_role") == 1,"generated walls carry the facade kind, 1:3 floor pitch and night windows")
	check(scaled.get_shader_parameter("signs_enabled") == true and scaled.get_shader_parameter("sign_atlas") == null,"commercial walls draw procedural signs (no texture bound)")
	var table: Dictionary = RENDER.FACADE_PALETTE.data
	var commercial: Dictionary = RENDER.facade_palette("concrete:commercial")
	check(scaled.get_shader_parameter("facade_palette") == true and scaled.get_shader_parameter("facade_body") == commercial.body and scaled.get_shader_parameter("facade_roof") == commercial.roof and scaled.get_shader_parameter("facade_accent") == commercial.accent,"generated walls bind their record palettes")
	for key: String in ["concrete:residential","concrete:commercial","concrete:industrial","concrete:public","brick:residential","wood:industrial","concrete:school"]:
		var palette: Dictionary = RENDER.facade_palette(key)
		var fields := key.split(":")
		var body_key: String = fields[0] if fields[0] != "concrete" else ("concrete:" + fields[1] if fields[1] in ["commercial","industrial","public"] else "concrete:residential")
		var rgb: Array = table.body[body_key][5]
		check(palette.body.size() == 8 and palette.roof.size() == 8 and palette.accent.size() == 8,"eight palette entries "+key)
		check(palette.body[5].is_equal_approx(Vector3(rgb[0],rgb[1],rgb[2])/255.0),"body palette follows facade_palette.json "+key)
		var roof: Array = table.roof[fields[0] if fields[0] != "concrete" else "concrete"][2]
		check(palette.roof[2].is_equal_approx(Vector3(roof[0],roof[1],roof[2])/255.0),"roof palette follows facade_palette.json "+key)
	var unscaled: ShaderMaterial = RENDER._generated_material(job,"brick:residential",{})
	check(is_equal_approx(unscaled.get_shader_parameter("floor_pitch"),3.0) and unscaled.get_shader_parameter("facade_kind") == 2,"missing scale provenance keeps 3 m floors")
	check(unscaled.get_shader_parameter("signs_enabled") == null,"homes carry no storefront signs")
	var plain: ShaderMaterial = RENDER._generated_material(job,"brick:home",{})
	check(plain.get_shader_parameter("facade_kind") == null and plain.get_shader_parameter("lighting_role") == null,"unknown use keeps a plain unlit wall")
	var ground: ShaderMaterial = RENDER._generated_material(job,"grass",{})
	check(ground.get_shader_parameter("facade_kind") == null,"ground keeps no facade")
	var standalone: Material = RENDER._generated_material({},"concrete:public",{"facade_floor_cm":100})
	check(standalone is StandardMaterial3D,"standalone preview without a context keeps its source material")
	var colours := {}
	for material: String in RENDER.BUILDING_BASE:
		for usage: String in RENDER.BUILDING_TINT:
			colours[RENDER.material_color(material+":"+usage).to_html(false)] = true
	check(colours.size() == 12,"twelve distinct material/use colours")
	check(RENDER.material_color("concrete:residential").to_html(false) == "d4cab8" and RENDER.material_color("brick:commercial").to_html(false) == "ae8273","palette matches the distant mesh (mapkit-package distant.rs)")
	check(RENDER.material_color("concrete:residential") != RENDER.material_color("concrete:commercial"),"uses differ within one material")
	job.clear(); context = null; scaled = null; unscaled = null; plain = null; ground = null
	cache.shutdown()
	storefront_layout()
	await top_binding()
	print("facade_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
