shader_type spatial;
render_mode unshaded, cull_disabled, depth_draw_never;
varying float age;
void vertex() { age = INSTANCE_CUSTOM.x; }
void fragment() {
    float radius = length(UV - vec2(0.5))*2.0;
    float ring = 1.0-smoothstep(0.06,0.17,abs(radius-(0.2+age*0.65)));
    ALBEDO=vec3(0.56,0.68,0.76);
    ALPHA=ring*(1.0-age)*0.55;
}
