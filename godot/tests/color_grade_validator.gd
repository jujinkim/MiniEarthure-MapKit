extends SceneTree
## Actual framebuffer checks: LUT covers 3D/sky, leaves Canvas UI unchanged,
## preserves black/night and snow white, and uses one bounded texture per scene.
## Golden hour warms the highlights only, keeps the shadow stops and restores the
## day curve exactly at weight 0.
const GRADE := preload("../color_grade.gd")
var failed := false
func _initialize() -> void: run.call_deferred()
func check(ok: bool, label: String) -> void:
	if not ok: failed = true; push_error(label)
func capture() -> Image:
	for frame in 4: await process_frame
	await RenderingServer.frame_post_draw
	return root.get_texture().get_image()
func run() -> void:
	root.size = Vector2i(128,128)
	root.content_scale_size = root.size
	root.content_scale_mode = Window.CONTENT_SCALE_MODE_VIEWPORT
	var world := WorldEnvironment.new()
	var settings := Environment.new()
	world.environment = settings
	settings.background_mode = Environment.BG_COLOR
	settings.background_color = Color(.5,.5,.5)
	settings.adjustment_enabled = true
	var grade := GRADE.new()
	settings.adjustment_color_correction = grade.texture
	root.add_child(world)
	var camera := Camera3D.new(); root.add_child(camera)
	var canvas := CanvasLayer.new(); root.add_child(canvas)
	var hud := ColorRect.new(); hud.color = Color(.25,.5,.75)
	hud.size = Vector2(24,24); canvas.add_child(hud)
	var day := await capture()
	var warm := day.get_pixel(64,64)
	check(warm.r-warm.b>.10 and warm.g>warm.b,"daytime amber/olive grade reaches rendered background")
	check(warm.r>.55 and warm.b>.44,"daylight midtones stay bright and retain colour separation")
	var day_colors := grade.texture.gradient.colors.duplicate()
	settings.background_color = Color(.8,.8,.8)
	var bright_day := (await capture()).get_pixel(64,64)
	grade.update(1,0,1)
	var golden_colors := grade.texture.gradient.colors.duplicate()
	var bright_golden := (await capture()).get_pixel(64,64)
	check(bright_golden.r-bright_golden.b>bright_day.r-bright_day.b+.02,"golden hour warms rendered highlights")
	settings.background_color = Color(.5,.5,.5)
	var golden_mid := (await capture()).get_pixel(64,64)
	check(golden_mid.r-golden_mid.b>warm.r-warm.b,"golden hour warms rendered midtones")
	for index in GRADE.STOPS.size():
		# Recomputed from the stated rule: DAY*(1+0.07w, 1, 1-0.10w), w = smoothstep(0,.6,stop), clamped to 1.
		var stop: float = GRADE.STOPS[index]
		var t := clampf(stop/0.6,0.0,1.0)
		var w := t*t*(3.0-2.0*t)
		var base: Color = GRADE.DAY[index]
		var expected := Color(minf(base.r*(1.0+0.07*w),1.0),minf(base.g,1.0),minf(base.b*(1.0-0.10*w),1.0))
		var got: Color = golden_colors[index]
		check(absf(got.r-expected.r)<1e-5 and absf(got.g-expected.g)<1e-5 and absf(got.b-expected.b)<1e-5,"golden stop %d follows the stated rule" % index)
	check(golden_colors[0]==day_colors[0],"golden keeps the black stop")
	var olive: Color = golden_colors[1]
	var olive_day: Color = day_colors[1]
	check(absf(olive.r-olive_day.r)<.001 and absf(olive.g-olive_day.g)<.001 and absf(olive.b-olive_day.b)<.001 and olive.g>olive.b,"golden keeps the olive shadow stop")
	var top: Color = golden_colors[golden_colors.size()-1]
	check(is_equal_approx(top.r,1.0) and is_equal_approx(top.g,.98) and absf(top.b-.819)<1e-5,"golden highlight clamps to (1, .98, .819)")
	grade.update(1,0,0)
	check(grade.texture.gradient.colors==day_colors,"golden weight 0 restores the day curve exactly")
	var restored := (await capture()).get_pixel(64,64)
	check(restored==warm,"golden weight 0 restores the rendered day pixel")
	grade.update(0,0)
	var night := await capture()
	var cool := night.get_pixel(64,64)
	check(cool.b>cool.r and cool.r>.25,"night cool curve preserves midtone visibility")
	grade.update(1,1)
	var snow := await capture()
	var neutral := snow.get_pixel(64,64)
	check(absf(neutral.r-neutral.b)<.04,"snow suppresses sepia")
	check(day.get_pixel(12,12)==night.get_pixel(12,12) and night.get_pixel(12,12)==snow.get_pixel(12,12),"HUD bypasses 3D grade")
	settings.background_color = Color.BLACK
	grade.update(0,0)
	var black := (await capture()).get_pixel(64,64)
	check(black.r<.01 and black.b<.01,"night preserves true black")
	var original := grade.texture
	for step in 100: grade.update(step/100.0,step/100.0,1.0-step/100.0)
	check(grade.texture==original and grade.texture.width==256,"weather updates reuse bounded LUT")
	canvas.queue_free(); camera.queue_free(); world.queue_free()
	for frame in 4: await process_frame
	print("color_grade_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
