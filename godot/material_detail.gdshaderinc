// Shared modest relief. Packed tile: shaded luminance/AO, roughness, pre-baked normal XY.
uniform sampler2D detail_tile : filter_linear_mipmap, repeat_enable;
uniform bool detail_enabled = false;
uniform float detail_scale = 1.0;
vec2 detail_uv(vec3 p, vec3 n) {
    vec3 a=abs(n);
    return (a.y>max(a.x,a.z) ? p.xz : (a.x>a.z ? p.zy : p.xy))*detail_scale;
}
vec3 detail_bump(vec3 p, vec3 n) {
    vec2 d=(texture(detail_tile,detail_uv(p,n)).ba*2.0-1.0)*0.055;
    vec3 a=abs(n);
    return a.y>max(a.x,a.z) ? vec3(-d.x,0.0,-d.y) : (a.x>a.z ? vec3(0.0,-d.y,-d.x) : vec3(-d.x,-d.y,0.0));
}
