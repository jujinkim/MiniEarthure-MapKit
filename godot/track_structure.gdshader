shader_type spatial;
render_mode cull_disabled;
uniform vec4 base_color : source_color = vec4(.35,.38,.41,1.0);
uniform float roughness = .72;
uniform float metallic = 0.0;
varying vec3 local_position;
varying vec3 local_normal;
void vertex() {
	local_position=VERTEX;
	local_normal=NORMAL;
}
void fragment() {
	// Recessed panel joins, fastening bands and a darker underside use the
	// original mesh. No added surface can narrow the bore or alter collision.
	vec3 p=local_position;
	float side=1.0-abs(normalize(local_normal).y);
	float seam=step(.975,fract((p.x+p.z)*.5));
	float band=step(.94,fract(p.y*.5));
	float fade=1.0-smoothstep(30.0,90.0,length(VERTEX));
	vec3 color=base_color.rgb*(1.0-seam*.22*fade);
	color=mix(color,base_color.rgb*.62,side*band*.5);
	color*=mix(.82,1.0,smoothstep(-.8,.4,local_normal.y));
	ALBEDO=color;
	ROUGHNESS=roughness;
	METALLIC=metallic;
}
