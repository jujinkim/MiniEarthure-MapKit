extends SceneTree
## Actual framebuffer checks: LUT covers 3D/sky, leaves Canvas UI unchanged,
## preserves black/night and snow white, and uses one bounded texture per scene.
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
	for step in 100: grade.update(step/100.0,step/100.0)
	check(grade.texture==original and grade.texture.width==256,"weather updates reuse bounded LUT")
	canvas.queue_free(); camera.queue_free(); world.queue_free()
	for frame in 4: await process_frame
	print("color_grade_validator: ","FAIL" if failed else "PASS")
	quit(1 if failed else 0)
