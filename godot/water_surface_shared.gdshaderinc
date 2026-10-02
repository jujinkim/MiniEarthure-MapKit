uniform sampler2D depth_texture : hint_depth_texture, repeat_disable, filter_nearest;
#ifdef WATER_REFRACTION
uniform sampler2D screen_texture : hint_screen_texture, repeat_disable, filter_linear;
#endif
uniform vec2 flow = vec2(0.0);
uniform int quality_level = 1;
varying vec3 world_position;
void vertex() { world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz; }
float scene_depth(vec2 uv, mat4 inverse_projection) {
    float depth = textureLod(depth_texture, uv, 0.0).r;
#if CURRENT_RENDERER == RENDERER_COMPATIBILITY
    vec3 ndc = vec3(uv, depth) * 2.0 - 1.0;
#else
    vec3 ndc = vec3(uv * 2.0 - 1.0, depth);
#endif
    vec4 view = inverse_projection * vec4(ndc, 1.0);
    return -view.z / view.w;
}
void fragment() {
    float thickness = max(0.0, scene_depth(SCREEN_UV, INV_PROJECTION_MATRIX) + VERTEX.z);
    vec2 p = world_position.xz - flow * TIME * .3;
    vec2 wave = vec2(cos(p.x*3.8+TIME*.7), sin(p.y*4.1-TIME*.6))*.035;
    if(quality_level>0) wave += vec2(sin(p.y*7.1+p.x*1.7-TIME),cos(p.x*6.7-p.y*2.1+TIME*.9))*.018;
    NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(wave.x,0.0,wave.y,0.0)).xyz);
    float deep = 1.0-exp(-thickness*.6);
    vec3 color = mix(vec3(.17,.39,.37),vec3(.035,.15,.20),deep);
    float fresnel = pow(1.0-clamp(dot(NORMAL,VIEW),0.0,1.0),5.0);
    SPECULAR = .65;
    ROUGHNESS = .20;
    METALLIC = .12;
    color = mix(color,vec3(.39,.48,.54),fresnel*.4);
#ifdef WATER_REFRACTION
    vec2 offset = wave*.018*smoothstep(.05,.8,thickness);
    vec2 refracted = clamp(SCREEN_UV+offset,vec2(.001),vec2(.999));
    // Foreground silhouettes must not bleed into the water.
    if(scene_depth(refracted, INV_PROJECTION_MATRIX)+VERTEX.z>0.0) color = mix(textureLod(screen_texture,refracted,0.0).rgb,color,.82);
#endif
    float foam = 1.0-smoothstep(.03,.38,thickness);
    if(quality_level>0) foam *= .6+.4*sin(p.x*12.0+sin(p.y*9.0)+TIME*1.6);
    ALBEDO = mix(color,vec3(.76,.81,.76),foam*.55);
    ALPHA = mix(.58,.92,deep)+fresnel*.05;
}
