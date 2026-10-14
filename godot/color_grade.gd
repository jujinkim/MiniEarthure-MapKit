extends RefCounted
## Scene-owned SDR channel curves for Environment's built-in LUT. Canvas UI is
## composed afterwards. A fixed 256px texture fits the existing atmosphere lease.
const STOPS := [0.0,0.08,0.25,0.5,0.75,1.0]
const DAY := [Color(0.004,0.005,0.006),Color(0.045,0.050,0.038),Color(0.23,0.235,0.15),Color(0.55,0.52,0.36),Color(0.83,0.77,0.57),Color(1.0,0.97,0.83)]
const NIGHT := [Color(0,0,0),Color(0.072,0.081,0.096),Color(0.235,0.25,0.275),Color(0.485,0.505,0.535),Color(0.745,0.765,0.79),Color(1,1,1)]
var texture := GradientTexture1D.new()
var _day := -1.0
var _snow := -1.0

func _init() -> void:
	texture.width = 256
	texture.use_hdr = true
	texture.gradient = Gradient.new()
	texture.gradient.offsets = PackedFloat32Array(STOPS)
	update(1.0,0.0)

func update(daylight: float, snow_cover: float) -> void:
	var day := clampf(daylight,0.0,1.0)
	var snow := clampf(snow_cover,0.0,1.0)
	if absf(day-_day)<0.002 and absf(snow-_snow)<0.002: return
	_day = day
	_snow = snow
	var colors := PackedColorArray()
	for index in STOPS.size():
		var neutral := Color(STOPS[index],STOPS[index],STOPS[index])
		colors.append(NIGHT[index].lerp(DAY[index].lerp(neutral,snow*0.92),day))
	texture.gradient.colors = colors
