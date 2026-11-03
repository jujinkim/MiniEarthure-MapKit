extends SceneTree
## Generated building facades: key classification, twelve base colours and the
## record palettes shared with the distant mesh, per-record wall finishes, floor pitch from the recorded
## map scale, shader uniforms, the solid top (CUSTOM0) binding and the
## commercial storey/shop-front layout.
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
			# The sign fills the fascia at the atlas content aspect, centred in the slot.
			var sign_height := 0.20*ground
			var sign_width := minf(width*0.88,sign_height*3.5)
			check(absf(sign_width/sign_height/3.5-1.0) <= 0.05 and sign_width <= width*0.88 and storefront(width*0.5,0.78+0.5*0.20,ground,width,width*0.5,2.0,pitch) == 7,"sign aspect 56:16 within the fascia (slot %.2f)"%width)
	var code: String = MATERIALS.new().shader.code
	for marker: String in ["if(height<=0.0) return vec2(pitch);","float ground = min(1.5*pitch,height);","upper<0.75*pitch ? 0.0 : upper/max(1.0,floor(upper/pitch+0.01))","if(height<=0.0) return 1.0+(v-storeys.x)/pitch;","if(v<height) return storeys.y>0.0 ? 1.0+(v-storeys.x)/storeys.y : 1.0;","return 1.0+(storeys.y>0.0 ? round((height-storeys.x)/storeys.y) : 0.0)+(v-height)/pitch;","facade_row(v,storeys,solid_height,floor_pitch)","facade_kind==3 && storeys.y<=0.0 && v<solid_height) { window = 0.0; spandrel = 0.0; }"]:
		check(code.contains(marker),"shader storeys match the mirror: "+marker)
	for marker: String in ["if(w>=0.78) return 7;","if(w>=0.765) return 6;","d<0.15*pitch-frame && w<0.72-frame/ground) ? 3 : 4;","if(w<0.08) return 1;","if(w>=0.10 && w<0.68) {","abs(s-round(s/pane)*pane)<0.012*pitch ? 4 : 2;","if(w>=0.70 && w<0.76) return 5;","slot_width*(0.22+0.28*float(slot_layout%3u)),2.0+float((slot_layout>>2u)&1u)","float sign_height = 0.20*storeys.x;","float sign_width = min(slot_width*0.88,sign_height*3.5);"]:
		check(code.contains(marker),"shader shop front matches the mirror: "+marker)

## Both upload paths bind the same attributes: commercial facades the solid seed
## (UV2) and top (CUSTOM0, one float), other facades the seed only, both without
## UV (world-space materials); other batches keep UV and no facade arrays.
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
	var expected := {0.0:[false,true,true],40.0:[false,true,false],80.0:[true,false,false]}
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

func run() -> void:
	var cache := CACHE.new(func(bytes): return Lease.new(bytes))
	cache.environment_profile = {"lights":[]}
	var context: RefCounted = cache.environment_context()
	check(context != null,"shared environment context")
	var job := {"resources":cache,"urban_shader":cache.urban_shader()}
	var names := {}
	for uniform: Dictionary in MATERIALS.new().shader.get_shader_uniform_list(): names[uniform.name] = true
	check(names.has("facade_kind") and names.has("floor_pitch") and names.has("lighting_role") and names.has("sign_atlas"),"surface shader compiles with the facade uniforms")
	check(names.has("facade_body") and names.has("facade_roof") and names.has("facade_accent") and names.has("facade_palette"),"surface shader compiles with the record palette uniforms")
	check(names.has("facade_finish") and names.has("facade_finish_look") and names.has("facade_finish_count") and names.has("facade_tiles") and names.has("facade_tiles_enabled"),"surface shader compiles with the finish and facade tile uniforms")
	check(context.facades is Texture2DArray and context.facades.get_layers() == MATERIALS.FACADE_LAYERS.size(),"shared facade tile array")
	finishes(job,context)
	await finish_pixels(job,context)
	# Per-solid choices must not depend on interpolated values: flat seeds and an
	# integer hash keep each sign slot and lit window one choice across its pixels.
	var code: String = MATERIALS.new().shader.code
	check(code.contains("varying flat float solid_seed;") and code.contains("varying flat float solid_base;") and code.contains("varying flat float solid_height;"),"solid seed, base and height are flat varyings")
	check(code.contains("CUSTOM0.x>500.0 ? (MODEL_MATRIX * vec4(0.0,CUSTOM0.x-1000.0,0.0,1.0)).y-solid_base : -1.0;"),"solid height reads the offset top in CUSTOM0 and falls back without it")
	check(code.contains("float pick = facade_unit(") and code.contains("facade_lit = step(0.35,facade_unit("),"sign picks and lit windows use the integer hash")
	check(code.contains("textureGrad(sign_atlas") and not code.contains("texture(sign_atlas"),"signs sample with analytic gradients")
	var kinds := {"concrete:residential":1,"brick:residential":2,"wood:residential":2,"concrete:commercial":3,"brick:commercial":3,"wood:industrial":4,"concrete:public":5,"brick:home":0,"concrete":0,"barrier:wall":0,"rc:wall":0,"builtin:tree":0}
	for key: String in kinds:
		check(RENDER.facade_kind(key) == kinds[key],"facade kind "+key)
		check(RENDER.PLAN.facade_arrays(key) == (0 if kinds[key] == 0 else (2 if kinds[key] == 3 else 1)),"batch facade arrays follow the facade kind: "+key)
	var scaled: ShaderMaterial = RENDER._generated_material(job,"concrete:commercial",{"facade_floor_cm":100})
	check(scaled.get_shader_parameter("facade_kind") == 3 and is_equal_approx(scaled.get_shader_parameter("floor_pitch"),1.0) and scaled.get_shader_parameter("lighting_role") == 1,"generated walls carry the facade kind, 1:3 floor pitch and night windows")
	check(scaled.get_shader_parameter("signs_enabled") == true and scaled.get_shader_parameter("sign_atlas") == context.signs,"commercial walls draw the shared sign atlas")
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
