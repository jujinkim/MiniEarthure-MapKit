// Shared surface detail. Packed tile: shaded luminance/AO, roughness, pre-baked normal XY.
uniform sampler2D detail_tile : filter_linear_mipmap, repeat_enable;
uniform bool detail_enabled = false;
uniform int detail_quality = 1;
uniform float detail_scale = 1.0;
uniform float surface_effect_distance = 96.0;
float surface_effect_weight(float distance_to_camera) {
    return 1.0-smoothstep(surface_effect_distance*0.8,surface_effect_distance,distance_to_camera);
}
float material_detail_weight(float distance_to_camera) {
    return (1.0-smoothstep(32.0,48.0,distance_to_camera))*surface_effect_weight(distance_to_camera);
}
// Packed tiles are centred around 0.65 luminance. Relief must not darken the
// whole material when it enters the detail range or changes quality tier.
float detail_luminance(float value) { return clamp(value/0.65,0.88,1.12); }
vec2 detail_uv(vec3 p, vec3 n) {
    vec3 a=abs(n);
    return (a.y>max(a.x,a.z) ? p.xz : (a.x>a.z ? p.zy : p.xy))*detail_scale;
}
vec3 detail_bump(vec3 p, vec3 n) {
    vec2 d=(texture(detail_tile,detail_uv(p,n)).ba*2.0-1.0)*0.035;
    vec3 a=abs(n);
    return a.y>max(a.x,a.z) ? vec3(-d.x,0.0,-d.y) : (a.x>a.z ? vec3(0.0,-d.y,-d.x) : vec3(-d.x,-d.y,0.0));
}
// Facade relief (generated building walls with a finish): a texture array of
// brick, plaster, formwork concrete, stone and metal tiles packed like the
// racing tiles (G is a roughness factor centred at 0.5). Walls take stronger
// relief than the deliberately flat racing tiles: luminance 75–125 %, bump x0.12.
uniform sampler2DArray facade_tiles : filter_linear_mipmap_anisotropic, repeat_enable;
uniform bool facade_tiles_enabled = false;
const float FACADE_BUMP = 0.12;
float facade_luminance(float value) { return clamp(value/0.65,0.75,1.25); }
// World-space perturbation of a wall from the tile's normal XY (u along the
// wall tangent, v up), x0.12.
vec3 facade_bump(vec2 packed, vec3 tangent) {
    vec2 d=(packed*2.0-1.0)*FACADE_BUMP;
    return -(tangent*d.x+vec3(0.0,d.y,0.0));
}
