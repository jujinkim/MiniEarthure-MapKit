shader_type spatial;
// Street-life voxel figures (pedestrian_meshes.gd, street_life_field.gd).
// UV: limb, colour slot; UV2: pivot height, pivot x (see
// pedestrian_meshes.gd). INSTANCE_CUSTOM: walkers (top palette index / 16,
// bottom palette index / 16, gait phase, alpha); pigeons (feather tone,
// flap 0..1, wing phase, alpha). Alpha below 1 fades by a 4 x 4 dither.
render_mode depth_draw_opaque;

// Muted Korean street palette (sRGB): charcoal, black, navy, slate, light
// grey, off-white, beige, camel, khaki, olive, brown, denim, burgundy, dusty
// pink, mustard, forest.
const vec3 PALETTE[16] = {
	vec3(0.231, 0.243, 0.267), vec3(0.141, 0.149, 0.169), vec3(0.180, 0.231, 0.333), vec3(0.357, 0.396, 0.447),
	vec3(0.663, 0.667, 0.651), vec3(0.847, 0.831, 0.784), vec3(0.776, 0.710, 0.592), vec3(0.639, 0.510, 0.365),
	vec3(0.545, 0.522, 0.400), vec3(0.365, 0.384, 0.278), vec3(0.416, 0.302, 0.227), vec3(0.298, 0.396, 0.514),
	vec3(0.427, 0.227, 0.251), vec3(0.702, 0.584, 0.569), vec3(0.643, 0.553, 0.298), vec3(0.247, 0.322, 0.275)
};
const vec3 SKIN = vec3(0.788, 0.631, 0.518);
const vec3 HAIR = vec3(0.110, 0.102, 0.102);
const vec3 SHOE = vec3(0.165, 0.165, 0.173);
const vec3 FEATHER_LIGHT = vec3(0.545, 0.561, 0.596);
const vec3 FEATHER_DARK = vec3(0.435, 0.455, 0.502);
const vec3 WING = vec3(0.353, 0.373, 0.412);
const vec3 HEAD = vec3(0.290, 0.310, 0.361);
const vec3 BEAK = vec3(0.788, 0.761, 0.722);
const float BAYER[16] = {0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0};

varying vec3 local;
varying flat int slot;
varying flat vec4 custom;

vec3 to_linear(vec3 c) {
	return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}

void vertex() {
	local = VERTEX;
	int limb = int(UV.x + 0.5);
	float pivot_y = UV2.x;
	float pivot_x = UV2.y;
	slot = int(UV.y + 0.5);
	custom = INSTANCE_CUSTOM;
	float phase = INSTANCE_CUSTOM.z * TAU;
	if (limb >= 1 && limb <= 4) {
		// Legs and arms swing about the hip / shoulder axis, opposite pairs.
		float side = (limb == 1 || limb == 4) ? 1.0 : -1.0;
		float a = sin(phase) * side * (limb <= 2 ? 0.45 : 0.35);
		float c = cos(a);
		float s = sin(a);
		vec2 yz = vec2(VERTEX.y - pivot_y, VERTEX.z);
		VERTEX.y = pivot_y + yz.x * c - yz.y * s;
		VERTEX.z = yz.x * s + yz.y * c;
		NORMAL = vec3(NORMAL.x, NORMAL.y * c - NORMAL.z * s, NORMAL.y * s + NORMAL.z * c);
	} else if (limb == 5 || limb == 6) {
		// Wings flap about the body side: both rise together.
		float a = (0.2 + 0.95 * sin(phase)) * INSTANCE_CUSTOM.y * (limb == 5 ? -1.0 : 1.0);
		float c = cos(a);
		float s = sin(a);
		vec2 xy = vec2(VERTEX.x - pivot_x, VERTEX.y - pivot_y);
		VERTEX.x = pivot_x + xy.x * c - xy.y * s;
		VERTEX.y = pivot_y + xy.x * s + xy.y * c;
		NORMAL = vec3(NORMAL.x * c - NORMAL.y * s, NORMAL.x * s + NORMAL.y * c, NORMAL.z);
	}
}

void fragment() {
	ivec2 p = ivec2(FRAGCOORD.xy) % 4;
	if (custom.w < 1.0 && custom.w * 16.0 <= BAYER[p.x + p.y * 4] + 0.5) discard;
	vec3 c = SKIN;
	if (slot == 0) c = PALETTE[clamp(int(custom.x * 16.0), 0, 15)];
	else if (slot == 1) c = local.y < 0.035 ? SHOE : PALETTE[clamp(int(custom.y * 16.0), 0, 15)];
	else if (slot == 2) c = local.y > 0.93 ? HAIR : SKIN;
	else if (slot == 3) c = HAIR;
	else if (slot == 4) c = SHOE;
	else if (slot == 5) c = mix(FEATHER_DARK, FEATHER_LIGHT, custom.x);
	else if (slot == 6) c = WING;
	else if (slot == 7) c = local.z < -0.066 ? BEAK : HEAD;
	ALBEDO = to_linear(c);
	ROUGHNESS = 0.85;
	SPECULAR = 0.2;
}
