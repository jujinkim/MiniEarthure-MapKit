shader_type spatial;
render_mode cull_back;
// Display-only city streetlights (city_lamp_meshes.gd): pole, base, door,
// plates, arms, LED head and lens, banners and boxes in one material. UV.x is
// the surface role, UV.y the part (0 every design; others collapse to a point
// unless keep() matches the design flags), UV2.x the share of the arm stretch
// (head vertices 1, a straight span 0..1, the twin side negative), the vertex
// colour a paint shade, a fixed sRGB colour, the lens module mask or the
// banner's face coordinates by role. INSTANCE_CUSTOM: arm reach (cm), design
// flags, paint + 4 x banner palette, 1. Every choice is an integer of those;
// the banner pattern is box-filtered over the pixel footprint (derivatives in
// uniform control flow), so far lamps never sparkle.
// Environment pixels (environment_materials.gd): (wet, snow, cloud cover,
// daylight) and (seconds, night, sun altitude, sunset seconds). The lens
// lights while the sun is below the horizon, like lighting_role 2.
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;
uniform vec4 led_color : source_color = vec4(1.0, 0.929, 0.839, 1.0);
const float REF_REACH = 1.35;
const int ROLE_PAINT = 0;
const int ROLE_FIXED = 1;
const int ROLE_LENS = 2;
const int ROLE_HOUSING = 3;
const int ROLE_SHEET = 4;
const int ROLE_BANNER = 5;
varying flat vec4 design;
varying vec3 world_normal;
// Paint (sRGB): galvanised, dark grey, bronze, warm white; their head
// housings; banner fields, bands and headers by palette.
const vec3 PAINT[4] = vec3[4](vec3(0.56,0.58,0.60), vec3(0.29,0.315,0.33), vec3(0.235,0.215,0.195), vec3(0.80,0.79,0.75));
const vec3 HOUSING[4] = vec3[4](vec3(0.66,0.68,0.69), vec3(0.30,0.32,0.335), vec3(0.25,0.23,0.21), vec3(0.74,0.745,0.74));
const vec3 FIELD[4] = vec3[4](vec3(0.16,0.42,0.72), vec3(0.17,0.53,0.34), vec3(0.76,0.21,0.21), vec3(0.42,0.29,0.63));
const vec3 BAND[4] = vec3[4](vec3(0.95,0.94,0.90), vec3(0.95,0.94,0.89), vec3(0.96,0.93,0.86), vec3(0.95,0.93,0.89));
const vec3 HEADER[4] = vec3[4](vec3(0.95,0.76,0.18), vec3(0.11,0.30,0.20), vec3(0.23,0.16,0.15), vec3(0.91,0.64,0.23));
vec3 lamp_linear(vec3 c, bool srgb_output) { return srgb_output ? c : mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c)); }
float lamp_band(float x, float a, float b, float w) {
    w = max(w,1e-4);
    return clamp((min(x+0.5*w,b)-max(x-0.5*w,a))/w,0.0,1.0);
}
// Mirror of city_lamp_meshes.gd keep().
bool keep(int part, uint flags) {
    bool twin = (flags & 1u) != 0u;
    bool straight = (flags & 2u) != 0u;
    if(part == 1) return !straight;
    if(part == 2) return straight;
    if(part == 3) return (flags & 4u) != 0u;
    if(part == 4) return (flags & 8u) != 0u;
    if(part == 5) return (flags & 16u) != 0u;
    if(part == 6) return twin && !straight;
    if(part == 7) return twin && straight;
    if(part == 8) return (flags & 32u) == 0u;
    if(part == 9) return (flags & 32u) != 0u;
    if(part == 10) return twin;
    return true;
}
void vertex() {
    design = INSTANCE_CUSTOM;
    uint flags = uint(round(INSTANCE_CUSTOM.y));
    if(!keep(int(round(UV.y)), flags)) VERTEX = vec3(0.0);
    else VERTEX.x += UV2.x * (INSTANCE_CUSTOM.x * 0.01 - REF_REACH);
    world_normal = normalize(MODEL_NORMAL_MATRIX * NORMAL);
}
void fragment() {
    vec4 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0) : vec4(0.0,0.0,0.18,1.0);
    vec4 clock = environment_enabled ? texelFetch(environment_data,ivec2(1,0),0) : vec4(43200.0,0.0,1.0,64800.0);
    // Banner footprint from derivatives in uniform control flow.
    vec2 face = COLOR.rg;
    float fw = max(length(dFdx(face)),length(dFdy(face)));
    int role = int(round(UV.x));
    uint palette = uint(round(design.z));
    int paint = int(palette & 3u);
    int banner = int((palette >> 2u) & 3u);
    vec3 color = lamp_linear(PAINT[paint],OUTPUT_IS_SRGB)*COLOR.r;
    float rough = paint == 0 ? 0.42 : 0.5;
    float metal = paint == 0 ? 0.55 : 0.25;
    float spec = 0.5;
    vec3 glow = vec3(0.0);
    bool night = clock.z < 0.0;
    if(role == ROLE_FIXED) {
        color = lamp_linear(COLOR.rgb,OUTPUT_IS_SRGB);
        rough = 0.6;
        metal = 0.1;
    } else if(role == ROLE_HOUSING) {
        color = lamp_linear(HOUSING[paint],OUTPUT_IS_SRGB)*COLOR.r;
        rough = 0.45;
        metal = 0.35;
    } else if(role == ROLE_SHEET) {
        // Anti-poster sheet: a pale embossed vinyl over the shaft.
        color = lamp_linear(mix(vec3(0.78,0.79,0.78),PAINT[paint],0.25),OUTPUT_IS_SRGB)*COLOR.r;
        rough = 0.7;
        metal = 0.0;
        spec = 0.35;
    } else if(role == ROLE_LENS) {
        // Frosted diffuser by day; LED modules (mask 1) and their gaps at
        // night, a white-hot core over the light colour.
        float lit = COLOR.r;
        color = lamp_linear(mix(vec3(0.22,0.23,0.24),vec3(0.82,0.83,0.82),lit),OUTPUT_IS_SRGB);
        rough = 0.2;
        metal = 0.0;
        spec = 0.6;
        if(night) glow = mix(lamp_linear(led_color.rgb,OUTPUT_IS_SRGB)*0.35,lamp_linear(led_color.rgb,OUTPUT_IS_SRGB)*3.2,lit);
    } else if(role == ROLE_BANNER) {
        // Street banner: a coloured field, a header stripe, a pale band with
        // a field-coloured bar in it, a pale border.
        vec3 field = FIELD[banner];
        float header = lamp_band(face.y,0.0,0.14,fw);
        float band = lamp_band(face.y,0.42,0.62,fw);
        float bar = band*lamp_band(face.y,0.48,0.56,fw)*lamp_band(face.x,0.2,0.8,fw);
        float border = 1.0-lamp_band(face.x,0.05,0.95,fw)*lamp_band(face.y,0.03,0.97,fw);
        vec3 c = mix(field,HEADER[banner],header);
        c = mix(c,BAND[banner],band);
        c = mix(c,field*0.9,bar);
        c = mix(c,BAND[banner],border*0.85);
        color = lamp_linear(c,OUTPUT_IS_SRGB);
        rough = 0.85;
        metal = 0.0;
        spec = 0.3;
    }
    // Weather as on walls: wet darkens and glosses, snow on upward faces.
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*facing_up*0.92);
    ALBEDO = color;
    ROUGHNESS = mix(rough,0.28,weather.x);
    METALLIC = metal;
    SPECULAR = spec;
    EMISSION = glow;
}
