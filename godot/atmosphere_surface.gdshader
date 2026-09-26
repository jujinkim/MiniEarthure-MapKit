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
uniform bool terrain_surface = false;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform float roughness = 0.85;
uniform float metallic = 0.0;
uniform int lighting_role = 0; // 1 windows, 2 street lamps
uniform bool instanced = false;
uniform bool distant = false;
uniform bool vertex_tinted = false;
instance uniform float building_seed = 0.5;
varying vec3 world_position;
varying vec3 world_normal;
varying float seed;
void vertex() {
    UV=UV*uv_scale.xy+uv_offset.xy;
    world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz;
    world_normal = normalize(MODEL_NORMAL_MATRIX * NORMAL);
    seed = distant ? UV.x : (INSTANCE_CUSTOM.a > 0.5 ? INSTANCE_CUSTOM.x : building_seed);
}
void fragment() {
    vec4 weather = texelFetch(environment_data,ivec2(0,0),0);
    vec4 clock = texelFetch(environment_data,ivec2(1,0),0);
    vec3 color = distant ? COLOR.rgb : base_color.rgb;
    if(vertex_tinted && !distant) color *= COLOR.rgb;
    if(textured) color *= texture(albedo_texture,UV).rgb;
    float surface_roughness=roughness;
    if(roughness_enabled) surface_roughness*=dot(texture(roughness_texture,UV),roughness_channel);
    if(normal_enabled) { NORMAL_MAP=texture(normal_texture,UV).rgb; NORMAL_MAP_DEPTH=normal_strength; }
    if(ao_enabled) { AO=dot(texture(ao_texture,ao_uv2 ? UV2 : UV),ao_channel); AO_LIGHT_AFFECT=ao_light_affect; }
    if(detail_enabled) {
        vec4 detail=texture(detail_tile,detail_uv(world_position,world_normal));
        color*=detail.r;
        surface_roughness*=detail.g;
        AO=detail.a;
        NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(detail_bump(world_position,world_normal),0.0)).xyz);
    }
    // Large, restrained variation remains stable when the camera moves.
    float grain = fract(sin(dot(floor(world_position.xz*36.0+world_position.y*17.0),vec2(12.9898,78.233)))*43758.5453);
    color *= 0.985 + 0.03*grain;
    if(terrain_surface) {
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
    float puddle = distant ? 0.0 : puddle_mask(world_position,world_normal.y,weather.x,weather.y);
    ALBEDO *= 1.0-puddle*0.10;
    ROUGHNESS = mix(ROUGHNESS,0.18,puddle);
    SPECULAR = mix(0.35,0.55,puddle);
    NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
    METALLIC = metallic*(dot(texture(metallic_texture,UV),metallic_channel)*float(metallic_enabled)+float(!metallic_enabled));
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
    if(role == 1) {
        uint identity = uint(round(seed*16777215.0));
        uint night = uint(max(0.0,clock.y+100000.0));
        float cutoff = float((identity ^ (night*31337u)) % 241u)*60.0;
        enabled = enabled && (clock.x < cutoff || clock.x >= clock.w);
        float windows = step(0.2,fract(floor(world_position.x*0.7)+floor(world_position.y*0.6)*0.618+seed));
        EMISSION = enabled ? vec3(1.0,0.63,0.28)*windows*1.2 : vec3(0.0);
    } else if(role == 2) {
        EMISSION = enabled ? vec3(1.0,0.76,0.45)*2.2 : vec3(0.0);
    }
}
