shader_type spatial;
render_mode cull_disabled;
uniform vec4 base_color : source_color = vec4(0.6,0.6,0.6,1.0);
uniform sampler2D albedo_texture : source_color, filter_linear_mipmap, repeat_enable;
uniform bool textured = false;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform float roughness = 0.85;
uniform float metallic = 0.0;
uniform int lighting_role = 0; // 1 windows, 2 street lamps
uniform bool instanced = false;
uniform bool distant = false;
instance uniform float building_seed = 0.5;
varying vec3 world_position;
varying vec3 world_normal;
varying float seed;
void vertex() {
    world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz;
    world_normal = normalize(MODEL_NORMAL_MATRIX * NORMAL);
    seed = distant ? UV.x : (INSTANCE_CUSTOM.x > 0.0 ? INSTANCE_CUSTOM.x : building_seed);
}
void fragment() {
    vec4 weather = texelFetch(environment_data,ivec2(0,0),0);
    vec4 clock = texelFetch(environment_data,ivec2(1,0),0);
    vec3 color = distant ? COLOR.rgb : base_color.rgb;
    if(textured) color *= texture(albedo_texture,UV).rgb;
    // Large, restrained variation remains stable when the camera moves.
    float grain = fract(sin(dot(floor(world_position.xz*36.0+world_position.y*17.0),vec2(12.9898,78.233)))*43758.5453);
    color *= 0.96 + 0.08*grain;
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    float snow_cover = weather.y * facing_up;
    color *= 1.0 - weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),snow_cover*0.92);
    ALBEDO = color;
    ROUGHNESS = mix(roughness,0.28,weather.x*facing_up);
    ROUGHNESS = mix(ROUGHNESS,0.92,snow_cover);
    METALLIC = metallic;
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
