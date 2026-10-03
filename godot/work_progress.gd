extends Control
## Stage-local progress shared by consumers. The caller translates the stage name.
var progress: Dictionary = {}
var reduce_motion := false
var phase := 0.0
var diameter := 72.0
var show_text := true
var color := Color("dfff00")

func _ready() -> void:
	custom_minimum_size = Vector2.ONE * diameter
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func set_progress(value: Dictionary) -> void:
	progress = value.duplicate()
	queue_redraw()

func fraction() -> float:
	var total: Variant = progress.get("total")
	if total == null or float(total) <= 0: return -1.0
	return clampf(float(progress.get("completed",0))/float(total),0,1)

func _process(delta: float) -> void:
	if not is_visible_in_tree() or reduce_motion or fraction() >= 0: return
	phase = wrapf(phase+delta*3,0,TAU)
	queue_redraw()

func _draw() -> void:
	var center := size*.5
	var value := fraction()
	draw_arc(center,maxf(5, diameter * 0.5 - 7),0,TAU,64,Color(1,1,1,.2),4,true)
	var start := -PI/2 if value >= 0 or reduce_motion else phase
	var end := start+TAU*value if value >= 0 else start+PI*1.3
	if end > start: draw_arc(center,maxf(5, diameter * 0.5 - 7),start,end,64,color,4,true)
	if not show_text: return
	var text := "%d%%" % floori(value*100) if value >= 0 else "—"
	var font := get_theme_default_font()
	var width := font.get_string_size(text,HORIZONTAL_ALIGNMENT_LEFT,-1,16).x
	draw_string(font,center+Vector2(-width*.5,6),text,HORIZONTAL_ALIGNMENT_LEFT,-1,16,Color.WHITE)

static func stage_key(stage: String) -> String:
	return {"preparing":"Preparing generation", "searching":"Searching for a track", "geometry":"Building track surfaces", "validation":"Validating track", "package_validation":"Validating package", "packing":"Packing map", "saving":"Saving map", "reading":"Reading map", "copying":"Copying map", "validating":"Validating package", "preview":"Preparing preview", "preview_meshes":"Preparing preview", "ready":"Ready"}.get(stage,"Preparing generation")
