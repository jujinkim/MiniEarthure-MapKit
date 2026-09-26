shader_type spatial;
render_mode cull_disabled, depth_draw_never;
uniform sampler2D depth_texture : hint_depth_texture, repeat_disable, filter_nearest;
uniform vec2 flow = vec2(0.0);
varying vec3 world_position;
void vertex() { world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz; }
void fragment() {
	float depth = textureLod(depth_texture, SCREEN_UV, 0.0).r;
#if CURRENT_RENDERER == RENDERER_COMPATIBILITY
	vec3 ndc = vec3(SCREEN_UV, depth) * 2.0 - 1.0;
#else
	vec3 ndc = vec3(SCREEN_UV * 2.0 - 1.0, depth);
#endif
	vec4 view = INV_PROJECTION_MATRIX * vec4(ndc, 1.0);
	float thickness = max(0.0, -view.z / view.w + VERTEX.z);
	vec2 p = world_position.xz - flow * TIME * 0.3;
	float ripple = sin(p.x * 3.8 + TIME * 1.7) * cos(p.y * 4.1 - TIME * 1.3);
	float foam = (1.0-smoothstep(0.03,0.32,thickness)) * (0.4+0.15*ripple);
	ALBEDO = mix(vec3(0.12,0.52,0.52),vec3(0.025,0.20,0.29),1.0-exp(-thickness*.6));
	ALBEDO = mix(ALBEDO,vec3(.75,.90,.85),foam);
	NORMAL = normalize(NORMAL + vec3(sin(p.x*5.0+TIME)*.055,0.0,cos(p.y*4.0-TIME)*.055));
	ROUGHNESS = .24;
	SPECULAR = .4;
	METALLIC = .08;
	ALPHA = .78 + .14*(1.0-exp(-thickness));
}
