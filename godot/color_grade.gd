extends RefCounted
## Scene-owned SDR channel curves for Environment's built-in LUT. Canvas UI is
## composed afterwards. A fixed 256px texture fits the existing atmosphere lease.
const STOPS := [0.0,0.08,0.25,0.5,0.75,1.0]
const DAY := [Color(0.008,0.010,0.008),Color(0.085,0.090,0.068),Color(0.285,0.290,0.235),Color(0.59,0.57,0.47),Color(0.85,0.81,0.70),Color(1.0,0.98,0.91)]
const NIGHT := [Color(0,0,0),Color(0.072,0.081,0.096),Color(0.235,0.25,0.275),Color(0.485,0.505,0.535),Color(0.745,0.765,0.79),Color(1,1,1)]
var texture := GradientTexture1D.new()
var _day := -1.0
var _snow := -1.0
var _golden := -1.0

func _init() -> void:
	texture.width = 256
	texture.use_hdr = true
	texture.gradient = Gradient.new()
	texture.gradient.offsets = PackedFloat32Array(STOPS)
	update(1.0,0.0)

## Golden-hour daylight stop: warmer and less blue as the stop brightens
## (w = smoothstep(0, 0.6, stop)), clamped to 1. Stop 0 is unchanged and stop
## 0.08 moves by under 0.001, so shadows keep their olive; 0.25 and up warm.
static func golden_stop(index: int) -> Color:
	var w := smoothstep(0.0,0.6,float(STOPS[index]))
	var day: Color = DAY[index]
	return Color(minf(day.r*(1.0+0.07*w),1.0),minf(day.g,1.0),minf(day.b*(1.0-0.10*w),1.0))

## `golden` (sky_hour.gd) blends the daylight curves towards golden_stop.
func update(daylight: float, snow_cover: float, golden := 0.0) -> void:
	var day := clampf(daylight,0.0,1.0)
	var snow := clampf(snow_cover,0.0,1.0)
	var warm := clampf(golden,0.0,1.0)
	if absf(day-_day)<0.002 and absf(snow-_snow)<0.002 and absf(warm-_golden)<0.002: return
	_day = day
	_snow = snow
	_golden = warm
	var colors := PackedColorArray()
	for index in STOPS.size():
		var neutral := Color(STOPS[index],STOPS[index],STOPS[index])
		colors.append(NIGHT[index].lerp((DAY[index] as Color).lerp(golden_stop(index),warm).lerp(neutral,snow*0.92),day))
	texture.gradient.colors = colors
