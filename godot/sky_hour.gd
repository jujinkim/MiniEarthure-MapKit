extends RefCounted
## Golden-hour layer for the low sun. Each table is a target the renderer blends
## towards by `golden(deg, cover)`; it never replaces the current colour, so every
## output is unchanged at a sun elevation of 24° and above, below -6° and under
## cloudy, rainy or snowy cover. Mirrored as constants in sky_hour.gdshaderinc for
## the glass reflection; keep both in step (sky_hour_validator compares them).

# Light colour. The renderer keeps its sin(altitude) sun formula above this.
const LOW_SUN_KEYS := [0.0,2.0,6.0,12.0]
const LOW_SUN := [Color(1.0,0.58,0.36),Color(1.0,0.64,0.40),Color(1.0,0.72,0.50),Color(1.0,0.84,0.66)]
const LOW_AMBIENT_KEYS := [0.0,6.0,12.0]
const LOW_AMBIENT := [Color(0.74,0.68,0.70),Color(0.74,0.68,0.70),Color(0.72,0.73,0.66)]
# Sky (atmosphere_sky.gdshader uniforms; the 12° keys sit near today's day sky).
const ZENITH_KEYS := [0.0,2.0,6.0,12.0]
const ZENITH := [Color(0.15,0.19,0.24),Color(0.20,0.25,0.31),Color(0.12,0.255,0.41),Color(0.16,0.28,0.40)]
const HORIZON_KEYS := [0.0,2.0,6.0,12.0]
const HORIZON := [Color(0.46,0.27,0.18),Color(0.52,0.31,0.19),Color(0.62,0.43,0.28),Color(0.58,0.57,0.51)]
# The -6° key equals the sky shader's dusk constant, so twilight below stays as is.
const DUSK_KEYS := [-6.0,0.0,2.0,6.0,12.0]
const DUSK := [Color(0.48,0.21,0.055),Color(0.60,0.20,0.04),Color(0.62,0.22,0.045),Color(0.56,0.22,0.05),Color(0.48,0.21,0.055)]
const HALO_KEYS := [0.0,6.0,12.0]
const HALO := [Color(0.50,0.22,0.06),Color(0.46,0.24,0.08),Color(0.38,0.25,0.12)]
const CLOUD_LIT_KEYS := [0.0,2.0,6.0,12.0]
const CLOUD_LIT := [Color(0.44,0.28,0.20),Color(0.46,0.30,0.21),Color(0.40,0.31,0.27),Color(0.27,0.30,0.34)]
# Today's sky constants (atmosphere_sky.gdshader uniform defaults).
const DAY_ZENITH := Color(0.16,0.28,0.40)
const DAY_HORIZON := Color(0.58,0.57,0.51)
const DUSK_COLOR := Color(0.48,0.21,0.055)
const HALO_COLOR := Color(0.38,0.25,0.12)
const CLOUD_DAY := Color(0.27,0.30,0.34)
# Daylight fog colour; at golden hour it follows HORIZON scaled by FOG / DAY_HORIZON.
const DAY_FOG := Color(0.48,0.49,0.46)

## Golden weight: a plateau of 1 from 0° to 6°, fading out by 24° and below -6°;
## 1 under clear cover (0.18), 0 from cloudy (0.83) up.
static func golden(deg: float, cover: float) -> float:
	return (1.0-smoothstep(6.0,24.0,deg))*smoothstep(-6.0,0.0,deg)*(1.0-smoothstep(0.45,0.80,cover))

## Clamped piecewise-linear table lookup; `keys` ascend and match `colors`.
static func sample(keys: Array, colors: Array, deg: float) -> Color:
	if deg <= float(keys[0]): return colors[0]
	for index in range(1,keys.size()):
		if deg < float(keys[index]):
			var low := float(keys[index-1])
			return (colors[index-1] as Color).lerp(colors[index],(deg-low)/(float(keys[index])-low))
	return colors[colors.size()-1]

## `current` blended towards the table by the golden weight (identity at 0).
static func blend(current: Color, keys: Array, colors: Array, deg: float, weight: float) -> Color:
	return current.lerp(sample(keys,colors,deg),weight)

## Low-sun fog target: the sampled horizon in today's fog-to-horizon ratio, so
## far geometry and the sky behind it stay matched at the display edge.
static func fog(deg: float) -> Color:
	var horizon := sample(HORIZON_KEYS,HORIZON,deg)
	return Color(horizon.r*DAY_FOG.r/DAY_HORIZON.r,horizon.g*DAY_FOG.g/DAY_HORIZON.g,horizon.b*DAY_FOG.b/DAY_HORIZON.b)
