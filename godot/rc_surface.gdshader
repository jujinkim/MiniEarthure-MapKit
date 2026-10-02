shader_type spatial;
render_mode cull_disabled;
uniform vec4 base_color : source_color = vec4(0.1,0.4,0.5,1.0);
uniform vec4 accent : source_color = vec4(1.0,0.7,0.1,1.0);
uniform int theme = 0;
uniform bool wall = false;
uniform bool floor_surface = false;
#include "material_detail.gdshaderinc"
varying vec3 position_world;
varying vec3 normal_world;

void vertex() { position_world = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz; normal_world = normalize(MODEL_NORMAL_MATRIX*NORMAL); }

void fragment() {
	vec3 color = base_color.rgb;
	vec3 p = position_world;
	float grain = fract(sin(dot(floor(p.xz*180.0),vec2(12.9898,78.233)))*43758.5453);
	if(wall) {
		float stripe = step(0.5,fract((p.x+p.z)*0.35));
		color = mix(color,(theme==2 ? vec3(0.05,0.13,0.22) : vec3(0.94,0.96,0.92)),stripe);
		float seam = step(0.96,fract((p.x+p.z)*0.7));
		color *= 1.0-seam*0.18;
	} else if(floor_surface) {
		float tile = step(0.98,fract(p.x/4.0))+step(0.98,fract(p.z/4.0));
		color *= 0.98+0.04*grain;
		if(theme!=1) color *= 1.0-min(tile,1.0)*0.10;
		else color *= 0.93+0.07*sin(p.x*0.8)*sin(p.z*0.8);
	} else if(theme==0) {
		color *= 0.94+0.12*grain;
	} else if(theme==1) {
		color *= 0.90+0.18*grain;
	} else {
		float join = max(step(0.986,fract(p.x/4.0)),step(0.986,fract(p.z/4.0)));
		color *= 1.0-join*0.16;
	}
	float worn = .5+.5*sin(p.x*.81+sin(p.z*.43)+p.y*.7);
	if(wall) color *= .91+.09*worn;
	if(detail_enabled && length(VERTEX)<48.0) {
		vec4 detail=texture(detail_tile,detail_uv(p,normal_world));
		color*=detail.r;
		NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(detail_bump(p,normal_world),0.0)).xyz);
	}
	color = mix(vec3(dot(color,vec3(.2126,.7152,.0722))),color,.72);
	ALBEDO = color*vec3(1.015,1.0,.97);
	ROUGHNESS = (theme==2 && !wall && !floor_surface) ? 0.48 : 0.92;
}
