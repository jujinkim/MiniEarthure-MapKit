shader_type spatial;
render_mode cull_disabled;
#include "wet_surface.gdshaderinc"
#include "material_detail.gdshaderinc"
uniform vec4 base_color : source_color = vec4(0.6,0.6,0.6,1.0);
uniform sampler2D albedo_texture : source_color, filter_linear_mipmap, repeat_enable;
uniform bool textured = false;
uniform sampler2D normal_texture : hint_normal, filter_linear_mipmap, repeat_enable;
uniform sampler2D ao_texture : filter_linear_mipmap, repeat_enable;
uniform sampler2D roughness_texture : filter_linear_mipmap, repeat_enable;
uniform sampler2D metallic_texture : filter_linear_mipmap, repeat_enable;
uniform bool normal_enabled = false;
uniform bool ao_enabled = false;
uniform bool roughness_enabled = false;
uniform bool metallic_enabled = false;
uniform bool ao_uv2 = false;
uniform float normal_strength = 1.0;
uniform float ao_light_affect = 0.0;
uniform vec4 ao_channel = vec4(1.0,0.0,0.0,0.0);
uniform vec4 roughness_channel = vec4(0.0,1.0,0.0,0.0);
uniform vec4 metallic_channel = vec4(0.0,0.0,1.0,0.0);
uniform vec3 uv_scale = vec3(1.0);
uniform vec3 uv_offset = vec3(0.0);
uniform int water_kind = 0; // 1 flowing, 2 still; authored vertex colours encode depth
uniform bool vegetation = false;
uniform float wind_strength = 0.5;
uniform bool terrain_surface = false;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform float roughness = 0.85;
uniform float metallic = 0.0;
uniform int lighting_role = 0; // 1 windows, 2 street lamps (distant UV.y also carries 3 native building walls)
uniform bool instanced = false;
uniform bool distant = false;
uniform bool vertex_tinted = false;
// Opaque instances carry this identity in INSTANCE_CUSTOM. A material uniform
// supports the exceptional non-instanced path without a global instance block.
uniform float building_seed = 0.5;
// Procedural facades on generated building walls: 1 apartment, 2 house,
// 3 commercial, 4 industrial, 5 public. Floor pitch is in scene metres (3 m ÷ D).
uniform int facade_kind = 0;
uniform float floor_pitch = 3.0;
varying vec3 world_position;
varying vec3 world_normal;
varying float seed;
float facade_hash(vec2 v) { return fract(sin(dot(v,vec2(12.9898,78.233)))*43758.5453); }
void vertex() {
    UV=UV*uv_scale.xy+uv_offset.xy;
    world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz;
    // Small leaf-only motion; rooted trunks and collision remain fixed.
    if(vegetation && wind_strength>0.0) {
        float bend=min(max(VERTEX.y,0.0)*0.02,0.07)*wind_strength;
        VERTEX.xz+=vec2(sin(TIME*1.3+world_position.x*0.7+world_position.z*0.3),cos(TIME*0.9+world_position.z*0.8))*bend;
        world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz;
    }
    world_normal = normalize(MODEL_NORMAL_MATRIX * NORMAL);
    seed = distant ? UV.x : (INSTANCE_CUSTOM.a > 0.5 ? INSTANCE_CUSTOM.x : building_seed);
}
void fragment() {
    vec4 weather = texelFetch(environment_data,ivec2(0,0),0);
    vec4 clock = texelFetch(environment_data,ivec2(1,0),0);
    vec3 color = distant ? COLOR.rgb : base_color.rgb;
    // Distant packed colours and colour properties both use sRGB. The source
    // hint converts uniforms in Forward/Mobile; vertex attributes need it here.
    if(distant && !OUTPUT_IS_SRGB) {
        color=mix(color/12.92,pow((color+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),color));
    }
    if(vertex_tinted && !distant) color *= COLOR.rgb;
    if(textured) color *= texture(albedo_texture,UV).rgb;
    float view_distance=length(VERTEX);
    float effect_weight=surface_effect_weight(view_distance);
    float surface_roughness=roughness;
    if(effect_weight>0.0) {
        if(roughness_enabled) surface_roughness*=mix(1.0,dot(texture(roughness_texture,UV),roughness_channel),effect_weight);
        if(normal_enabled) { NORMAL_MAP=texture(normal_texture,UV).rgb; NORMAL_MAP_DEPTH=normal_strength*effect_weight; }
        if(ao_enabled) { AO=mix(1.0,dot(texture(ao_texture,ao_uv2 ? UV2 : UV),ao_channel),effect_weight); AO_LIGHT_AFFECT=ao_light_affect; }
    }
    if(detail_enabled && view_distance<48.0 && effect_weight>0.0) {
        vec4 detail=texture(detail_tile,detail_uv(world_position,world_normal));
        float relief=material_detail_weight(view_distance)*(vegetation ? 0.30 : 0.9);
        color*=mix(1.0,detail_luminance(detail.r),relief);
        if(detail_quality>0) surface_roughness*=mix(1.0,detail.g,relief);
        if(detail_quality>0) AO=mix(1.0,detail.r/0.65,0.18*relief);
        if(detail_quality>1) NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(detail_bump(world_position,world_normal)*relief,0.0)).xyz);
    }
    // Large, restrained variation remains stable when the camera moves.
    if(effect_weight>0.0 && !vegetation) {
        float grain = fract(sin(dot(floor(world_position.xz*36.0+world_position.y*17.0),vec2(12.9898,78.233)))*43758.5453);
        color *= 1.0 + (0.016*grain-0.008)*effect_weight;
    }
    // Generated walls: floor bands, window panes and a per-face seed. Roofs,
    // bottoms, distant meshes and imported assets keep their own appearance.
    float facade_wall = 0.0;
    float facade_window = 0.0;
    float facade_lit = 0.0;
    float facade_seed = seed;
    if(facade_kind>0 && !distant && abs(world_normal.y)<0.5) {
        facade_wall = 1.0;
        vec3 n = normalize(world_normal);
        facade_seed = facade_hash(vec2(round(dot(n,world_position)*4.0),round(n.x*8.0)+round(n.z*8.0)*17.0));
        if(effect_weight>0.0) {
            float u = dot(world_position,normalize(vec3(-n.z,0.0,n.x)));
            float v = world_position.y;
            float bay = facade_kind==1 ? 1.3 : (facade_kind==2 ? 1.8 : (facade_kind==3 ? 0.8 : (facade_kind==4 ? 1.0 : 1.5)));
            vec2 cell = vec2(u/(floor_pitch*bay),v/floor_pitch);
            vec2 f = fract(cell);
            vec4 pane = facade_kind==1 ? vec4(0.2,0.75,0.3,0.75) : (facade_kind==2 ? vec4(0.3,0.7,0.3,0.7) : (facade_kind==3 ? vec4(0.1,0.9,0.15,0.85) : (facade_kind==4 ? vec4(0.15,0.85,0.6,0.85) : vec4(0.3,0.7,0.2,0.85))));
            float window = step(pane.x,f.x)*step(f.x,pane.y)*step(pane.z,f.y)*step(f.y,pane.w);
            if(facade_kind==3 && v<floor_pitch) window = step(0.12,f.y)*step(f.y,0.9);
            if(facade_kind==4) {
                color *= 1.0-0.08*step(0.85,fract(u/(0.6*floor_pitch)));
                window *= step(0.5,facade_hash(vec2(floor(cell.x),floor(cell.y)*3.0)));
            }
            if(facade_kind==1) color *= 1.0-0.14*step(0.94,f.y);
            if(facade_kind==5) color *= 1.0+0.05*step(f.y,0.08);
            facade_window = window*effect_weight;
            facade_lit = step(0.35,facade_hash(floor(cell)+facade_seed*7.0));
            color = mix(color,vec3(0.21,0.25,0.29),facade_window*0.85);
            surface_roughness = mix(surface_roughness,0.32,facade_window);
        }
        color *= 0.97+0.06*facade_seed;
    }
    if(terrain_surface || (distant && UV.y < -0.5)) {
        float broad=sin(world_position.x*0.055+sin(world_position.z*0.037))*cos(world_position.z*0.063);
        color*=0.96+0.06*broad;
        float rock=1.0-smoothstep(0.70,0.94,world_normal.y);
        color=mix(color,vec3(0.40,0.37,0.30),rock*0.55);
        float shore=1.0-smoothstep(0.15,0.85,world_position.y);
        color=mix(color,vec3(0.42,0.39,0.29),shore*0.6);
    }
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    float snow_cover = weather.y * facing_up;
    color *= 1.0 - weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),snow_cover*0.92);

    ALBEDO = color;
    ROUGHNESS = mix(surface_roughness,0.28,weather.x*facing_up);
    ROUGHNESS = mix(ROUGHNESS,0.92,snow_cover);
    float puddle = 0.0;
    if(!distant && effect_weight>0.0 && weather.x>0.2) puddle=puddle_mask(world_position,world_normal.y,weather.x,weather.y)*effect_weight;
    ALBEDO *= 1.0-puddle*0.10;
    ROUGHNESS = mix(ROUGHNESS,0.22,puddle);
    SPECULAR = mix(0.35,0.50,puddle);
    if(puddle>0.0) NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
    METALLIC = metallic;
    if(metallic_enabled && effect_weight>0.0) METALLIC*=mix(1.0,dot(texture(metallic_texture,UV),metallic_channel),effect_weight);
    if(water_kind>0) {
        float speed=water_kind==1 ? 0.7 : 0.12;
        vec2 p=world_position.xz;
        vec3 ripple=vec3(sin(p.x*5.0+p.y*1.2+TIME*speed)*0.025,0.0,cos(p.y*6.0-TIME*speed)*0.025);
        NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(ripple,0.0)).xyz);
        ALBEDO=color*0.58*(0.99+0.01*sin(p.x*3.0+p.y*2.0+TIME*speed));
        ROUGHNESS=water_kind==1 ? 0.23 : 0.16;
        SPECULAR=0.65;
        METALLIC=0.12;
    }
    bool enabled = clock.z < 0.0;
    int role = distant ? int(round(UV.y)) : lighting_role;
    if((role == 1 || role == 3) && enabled) {
        // Role 3: distant native building prisms (walls only, averaged windows).
        uint identity = uint(round((facade_kind>0 ? facade_seed : seed)*16777215.0));
        uint night = uint(max(0.0,clock.y+100000.0));
        float cutoff = float((identity ^ (night*31337u)) % 241u)*60.0;
        enabled = enabled && (clock.x < cutoff || clock.x >= clock.w);
        float windows = 0.8;
        if(role == 3) windows = world_normal.y > 0.5 ? 0.0 : 0.15;
        else if(facade_kind>0) windows = facade_wall*mix(0.15,facade_window*facade_lit,effect_weight);
        else if(effect_weight>0.0) windows=mix(0.8,step(0.2,fract(floor(world_position.x*0.7)+floor(world_position.y*0.6)*0.618+seed)),effect_weight);
        EMISSION = enabled ? vec3(1.0,0.63,0.28)*windows*1.2 : vec3(0.0);
    } else if(role == 2 && enabled) {
        // Authored bulbs/signs retain their material colour in both LODs.
        EMISSION = enabled ? color*0.9 : vec3(0.0);
    }
}
