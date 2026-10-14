// Shared modest relief. Packed tile: shaded luminance/AO, roughness, pre-baked normal XY.
uniform sampler2D detail_tile : filter_linear_mipmap, repeat_enable;
uniform bool detail_enabled = false;
uniform int detail_quality = 1;
uniform float detail_scale = 1.0;
uniform float surface_effect_distance = 96.0;
float surface_effect_weight(float distance_to_camera) {
    return 1.0-smoothstep(surface_effect_distance*0.8,surface_effect_distance,distance_to_camera);
}
// One distance/quality-independent grade for generated, imported and distant
// colours. Keep authored hues; sunlight warms the world, snow/night stay neutral.
vec3 world_surface_color(vec3 color, float snow, float sun_altitude) {
    float warmth=smoothstep(-0.08,0.4,sun_altitude)*(1.0-clamp(snow,0.0,1.0));
    vec3 restrained=mix(vec3(dot(color,vec3(.2126,.7152,.0722))),color,.94);
    return restrained*mix(vec3(1.0),vec3(1.04,1.012,.96),warmth);
}
float material_detail_weight(float distance_to_camera) {
    return (1.0-smoothstep(32.0,48.0,distance_to_camera))*surface_effect_weight(distance_to_camera);
}
// Packed tiles are centred around 7/8 luminance. Relief must not darken the
// whole material when it enters the detail range or changes quality tier.
float detail_luminance(float value) { return clamp(value/0.875,0.75,1.12); }
vec2 detail_uv(vec3 p, vec3 n) {
    vec3 a=abs(n);
    return (a.y>max(a.x,a.z) ? p.xz : (a.x>a.z ? p.zy : p.xy))*detail_scale;
}
vec3 detail_bump(vec3 p, vec3 n) {
    vec2 d=(texture(detail_tile,detail_uv(p,n)).ba*2.0-1.0)*0.055;
    vec3 a=abs(n);
    return a.y>max(a.x,a.z) ? vec3(-d.x,0.0,-d.y) : (a.x>a.z ? vec3(0.0,-d.y,-d.x) : vec3(-d.x,-d.y,0.0));
}
