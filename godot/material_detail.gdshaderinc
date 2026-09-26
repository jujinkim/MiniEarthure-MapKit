// Shared modest relief. Packed tile: luminance, roughness, height, occlusion.
uniform sampler2D detail_tile : filter_linear_mipmap, repeat_enable;
uniform bool detail_enabled = false;
uniform float detail_scale = 1.0;
vec2 detail_uv(vec3 p, vec3 n) {
    vec3 a=abs(n);
    return (a.y>max(a.x,a.z) ? p.xz : (a.x>a.z ? p.zy : p.xy))*detail_scale;
}
vec3 detail_bump(vec3 p, vec3 n) {
    vec2 uv=detail_uv(p,n);
    float h=texture(detail_tile,uv).b;
    vec2 d=vec2(texture(detail_tile,uv+vec2(1.0/256.0,0.0)).b-h,
                texture(detail_tile,uv+vec2(0.0,1.0/256.0)).b-h)*0.035;
    vec3 a=abs(n);
    return a.y>max(a.x,a.z) ? vec3(-d.x,0.0,-d.y) : (a.x>a.z ? vec3(0.0,-d.y,-d.x) : vec3(-d.x,-d.y,0.0));
}
