shader_type spatial;
render_mode cull_back;
// Display-only facade and rooftop fixtures (facade_fixture_meshes.gd): blade
// sign housings and faces, brackets, air-conditioner bodies, fan grilles,
// louvres and pipes; penthouse walls, slabs, doors and lamps, FRP and
// stainless tanks, parapet rims and railings, solar panels and dishes, in one
// material. UV.x is the surface role, UV.y a baked shade plus 2 x the mesh
// variant of the part (0 every variant), UV2 the face coordinates (rail posts
// and legs: their anchor); INSTANCE_CUSTOM carries the design bits (three
// 8-bit channels) and the blade's height:width aspect or a rooftop fixture's
// floor pitch. Every choice is an integer hash of those bits; fine patterns
// are box-filtered over the pixel footprint (derivatives taken in uniform
// control flow) and settle on their area shares, so far fixtures never
// sparkle.
#include "facade_signs.gdshaderinc"
// Environment pixels (environment_materials.gd): (wet, snow, cloud cover,
// daylight) and (seconds, night, sun altitude, sunset seconds).
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;
// Set by display_quality.apply_material (the material carries mapkit_detail).
uniform int detail_quality = 1;
uniform float surface_effect_distance = 96.0;
// Record palettes (sRGB, facade_palette.json): six keys of eight bodies and
// three materials of eight roofs; rooftop design bits name the entries.
uniform vec3 fixture_body[48];
uniform vec3 fixture_roof[24];
varying flat vec4 design;
varying vec3 world_normal;
// Mesh-space position in metres of the instance (scaled, before rotation)
// and the mesh normal: patterns on stretched rims, tanks and panels.
varying vec3 local_m;
varying vec3 local_n;
const int ROLE_HOUSING = 0;
const int ROLE_FACE = 1;
const int ROLE_STEEL = 2;
const int ROLE_BODY = 3;
const int ROLE_GRILLE = 4;
const int ROLE_LOUVRE = 5;
const int ROLE_PIPE = 6;
const int ROLE_WALL = 7;
const int ROLE_CAP = 8;
const int ROLE_TOP = 9;
const int ROLE_DOOR = 10;
const int ROLE_LAMP = 11;
const int ROLE_FRP = 12;
const int ROLE_STAINLESS = 13;
const int ROLE_SOLAR = 14;
const int ROLE_RAIL = 15;
const int ROLE_POST = 16;
const int ROLE_LEG = 17;
const int ROLE_DISH = 18;
const int ROLE_FAN = 19;
vec3 fixture_linear(vec3 c, bool srgb_output) { return srgb_output ? c : mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c)); }
// Box-filtered share of [a,b] of each unit cell over a footprint w centred on x.
float fixture_span_integral(float t, float a, float b) { return floor(t)*(b-a)+clamp(fract(t),a,b)-a; }
float fixture_span(float x, float a, float b, float w) {
    float t = fract(x);
    w = max(w,1e-4);
    return clamp((fixture_span_integral(t+0.5*w,a,b)-fixture_span_integral(t-0.5*w,a,b))/w,0.0,1.0);
}
float fixture_band(float x, float a, float b, float w) {
    w = max(w,1e-4);
    return clamp((min(x+0.5*w,b)-max(x-0.5*w,a))/w,0.0,1.0);
}
float fixture_noise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f*f*(3.0-2.0*f);
    float a = fract(sin(dot(i,vec2(12.9898,78.233)))*43758.5453);
    float b = fract(sin(dot(i+vec2(1.0,0.0),vec2(12.9898,78.233)))*43758.5453);
    float c = fract(sin(dot(i+vec2(0.0,1.0),vec2(12.9898,78.233)))*43758.5453);
    float d = fract(sin(dot(i+vec2(1.0,1.0),vec2(12.9898,78.233)))*43758.5453);
    return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
}
// The fan grille of AC and condenser units (face coordinates -1..1): a bezel
// ring, wire rings and a cross over a dark shroud, three blades and a hub.
vec3 fixture_grille(vec2 face, float fw, vec3 body) {
    float r = length(face);
    vec3 shroud = vec3(0.07,0.075,0.08);
    float blades = fixture_span(atan(face.y,face.x)*0.477465+r*0.35,0.0,0.42,fw*0.477465/max(r,0.05)+fw*0.35);
    vec3 fan = mix(shroud,vec3(0.30,0.31,0.32),blades*fixture_band(r,0.16,0.86,fw));
    fan = mix(fan,vec3(0.40,0.41,0.42),fixture_band(r,0.0,0.16,fw));
    float wires = max(fixture_span(r/0.15,0.0,0.16,fw/0.15),max(fixture_band(face.x,-0.02,0.02,fw),fixture_band(face.y,-0.02,0.02,fw)));
    vec3 grille = mix(fan,vec3(0.55,0.56,0.57),wires*0.85);
    float inside = fixture_band(r,0.0,0.9,fw);
    float bezel = fixture_band(r,0.9,1.0,fw);
    return mix(body,grille,inside)*(1.0-0.2*bezel);
}
void vertex() {
    design = INSTANCE_CUSTOM;
    uint bits = uint(round(INSTANCE_CUSTOM.x))|(uint(round(INSTANCE_CUSTOM.y))<<8u)|(uint(round(INSTANCE_CUSTOM.z))<<16u);
    int role = int(round(UV.x));
    // Parts of another mesh variant than the design's collapse to a point.
    int part = int(floor(UV.y*0.5));
    if(part>0 && uint(part-1)!=((bits>>9u)&3u)) VERTEX = vec3(0.0);
    // Rails and solar rows stretch x 0..1 over their length; posts and legs
    // keep their width: x = anchor + offset x pitch / length. Posts spread
    // about every 0.6 floor pitches, at most 31 intervals, inset at the ends.
    float sx = length(MODEL_MATRIX[0].xyz);
    float sy = length(MODEL_MATRIX[1].xyz);
    float sz = length(MODEL_MATRIX[2].xyz);
    float ratio = sy/max(sx,1e-4);
    if(role==ROLE_POST && part>0 && uint(part-1)==((bits>>9u)&3u)) {
        float intervals = clamp(round(1.0/(0.6*ratio)),1.0,31.0);
        if(UV2.x>intervals+0.5) VERTEX = vec3(0.0);
        else VERTEX.x = mix(0.015*ratio,1.0-0.015*ratio,UV2.x/intervals)+VERTEX.x*ratio;
    } else if(role==ROLE_LEG) VERTEX.x = UV2.x+VERTEX.x*ratio;
    local_m = VERTEX*vec3(sx,sy,sz);
    local_n = NORMAL;
    world_normal = normalize(MODEL_NORMAL_MATRIX*NORMAL);
}
void fragment() {
    vec4 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0) : vec4(0.0,0.0,0.18,1.0);
    vec4 clock = environment_enabled ? texelFetch(environment_data,ivec2(1,0),0) : vec4(43200.0,0.0,1.0,64800.0);
    // Footprints from derivatives in uniform control flow.
    vec2 face = UV2;
    vec2 face_dx = dFdx(face);
    vec2 face_dy = dFdy(face);
    vec3 m_dx = dFdx(local_m);
    vec3 m_dy = dFdy(local_m);
    float view_distance = length(VERTEX);
    float effect = 1.0-smoothstep(surface_effect_distance*0.8,surface_effect_distance,view_distance);
    int role = int(round(UV.x));
    float shade = UV.y-2.0*floor(UV.y*0.5);
    uint bits = uint(round(design.x))|(uint(round(design.y))<<8u)|(uint(round(design.z))<<16u);
    float aspect = max(design.w,1.0);
    // Rooftop fixtures: the floor pitch, and the building's palette entries.
    float pitch = max(design.w,0.05);
    uint key = min(bits&7u,5u);
    vec3 building_body = fixture_body[int(key*8u+((bits>>3u)&7u))];
    vec3 building_roof = fixture_roof[int((key<4u ? 0u : key-3u)*8u+((bits>>6u)&7u))];
    float fw = max(length(face_dx),length(face_dy));
    // The pixel footprint in metres and a face's in-plane coordinates in
    // metres (from the mesh normal's dominant axis).
    float m_fw = max(length(m_dx),length(m_dy));
    vec3 an = abs(local_n);
    vec2 plane_m = an.y>max(an.x,an.z) ? local_m.xz : (an.x>an.z ? local_m.zy : local_m.xy);
    uint tone = facade_pcg(bits^1013904223u)%3u;
    // Blade housings: charcoal, silver or white enamel; unit bodies: ivory,
    // light grey or beige.
    vec3 housing = tone==0u ? vec3(0.13,0.13,0.14) : (tone==1u ? vec3(0.62,0.63,0.64) : vec3(0.86,0.86,0.84));
    vec3 body = tone==0u ? vec3(0.84,0.83,0.78) : (tone==1u ? vec3(0.68,0.69,0.68) : vec3(0.79,0.75,0.65));
    vec3 color = fixture_linear(housing,OUTPUT_IS_SRGB);
    float rough = 0.45;
    float metal = 0.2;
    float spec = 0.45;
    vec3 glow = vec3(0.0);
    // Mottled weathering of plaster and concrete (settles with the footprint).
    float mottle = mix(1.0,0.9+0.2*fixture_noise(plane_m/(0.35*pitch)),(1.0-smoothstep(0.04,0.12,m_fw/pitch))*effect);
    if(role==ROLE_FACE) {
        // Both broad faces: the vertical sign, x across (0..1) and y down
        // (0..aspect) in units of the sign's width; the housing shows where
        // the design leaves it (channel letters, rounded corners).
        vec2 local = vec2(face.x,face.y*aspect);
        float footprint = max(length(vec2(face_dx.x,face_dx.y*aspect)),length(vec2(face_dy.x,face_dy.y*aspect)));
        FacadeSign s = facade_sign_face(local,bits,aspect,footprint,true);
        vec3 face_color = fixture_linear(s.color,OUTPUT_IS_SRGB);
        color = mix(color*(1.0-s.shadow),face_color,s.body);
        rough = mix(rough,0.3,s.body);
        spec = mix(spec,0.5,s.body);
        metal = mix(metal,0.0,s.body);
        glow = face_color*s.glow*s.body*0.55;
    } else if(role==ROLE_STEEL || role==ROLE_LEG) {
        color = fixture_linear(vec3(0.24,0.25,0.26),OUTPUT_IS_SRGB);
        rough = 0.55;
        metal = 0.5;
    } else if(role==ROLE_BODY || role==ROLE_LOUVRE) {
        color = fixture_linear(body,OUTPUT_IS_SRGB);
        rough = 0.5;
        metal = 0.0;
        spec = 0.4;
        // Louvres: slats every tenth of the face, settling on their share.
        if(role==ROLE_LOUVRE) color *= mix(0.5,1.0,fixture_span(face.y*10.0,0.0,0.55,fw*10.0));
    } else if(role==ROLE_GRILLE || role==ROLE_FAN) {
        color = fixture_linear(fixture_grille(face,fw,body),OUTPUT_IS_SRGB);
        rough = 0.6;
        metal = 0.25*fixture_band(length(face),0.0,0.9,fw);
    } else if(role==ROLE_PIPE) {
        // Pipe insulation tape.
        color = fixture_linear(vec3(0.80,0.79,0.74),OUTPUT_IS_SRGB);
        rough = 0.8;
        metal = 0.0;
        spec = 0.3;
    } else if(role==ROLE_WALL) {
        // Penthouse walls and rims: the building's plaster, a grime band at
        // the foot and faint streaks under the slab or coping.
        float v = local_m.y/pitch;
        float foot = 1.0-0.14*(1.0-smoothstep(0.0,0.08,v));
        color = fixture_linear(building_body,OUTPUT_IS_SRGB)*mottle*foot;
        rough = 0.88;
        metal = 0.0;
        spec = 0.3;
    } else if(role==ROLE_CAP) {
        color = fixture_linear(vec3(0.70,0.70,0.67),OUTPUT_IS_SRGB)*mottle;
        rough = 0.85;
        metal = 0.0;
        spec = 0.3;
    } else if(role==ROLE_TOP) {
        // The penthouse slab in the roof's coating, a little weathered.
        color = fixture_linear(building_roof,OUTPUT_IS_SRGB)*mix(1.0,0.92+0.12*fixture_noise(local_m.xz/(0.5*pitch)),effect);
        rough = 0.6;
        metal = 0.0;
        spec = 0.4;
    } else if(role==ROLE_DOOR) {
        // Steel door: green, grey, brown or blue-grey paint, a darker frame
        // rebate, a lever handle and a low louvre (face coordinates).
        uint paint = facade_pcg(bits^2246822519u)%4u;
        vec3 door = paint==0u ? vec3(0.20,0.32,0.26) : (paint==1u ? vec3(0.44,0.46,0.47) : (paint==2u ? vec3(0.33,0.23,0.17) : vec3(0.32,0.38,0.44)));
        float frame = 1.0-fixture_band(face.x,0.06,0.94,fw)*fixture_band(face.y,0.05,1.0,fw);
        float handle = fixture_band(face.x,0.78,0.88,fw)*fixture_band(face.y,0.46,0.50,fw);
        float louvre = fixture_band(face.x,0.2,0.8,fw)*fixture_band(face.y,0.8,0.93,fw)*fixture_span(face.y*60.0,0.0,0.5,fw*60.0);
        color = fixture_linear(door*(1.0-0.35*frame-0.4*louvre)+vec3(0.5)*handle,OUTPUT_IS_SRGB);
        rough = 0.5;
        metal = 0.3;
        // A burning lamp washes the door below it (seen from above too,
        // where the canopy hides the lamp itself).
        if(facade_pcg(bits^3266489917u)%10u<7u) glow = color*fixture_linear(vec3(1.0,0.82,0.58),OUTPUT_IS_SRGB)*(1.6-1.2*face.y);
    } else if(role==ROLE_LAMP) {
        color = fixture_linear(vec3(0.92,0.90,0.82),OUTPUT_IS_SRGB);
        rough = 0.3;
        metal = 0.0;
        // Most door lamps burn faintly all night.
        if(facade_pcg(bits^3266489917u)%10u<7u) glow = fixture_linear(vec3(1.0,0.82,0.58),OUTPUT_IS_SRGB)*1.6;
    } else if(role==ROLE_FRP) {
        // FRP panel tank: 1 m (a third of a floor) square panels, each with a
        // raised flange border (a lighter line and its shadow) and a domed
        // centre; ivory, pale grey or sky blue resin.
        uint resin = facade_pcg(bits^1540483477u)%3u;
        vec3 frp = resin==0u ? vec3(0.82,0.81,0.74) : (resin==1u ? vec3(0.74,0.76,0.75) : vec3(0.62,0.74,0.80));
        vec2 panel = plane_m/(pitch/3.0);
        float pfw = m_fw/(pitch/3.0);
        float flange = max(fixture_span(panel.x+0.03,0.0,0.06,pfw),fixture_span(panel.y+0.03,0.0,0.06,pfw));
        vec2 pf = fract(panel)-0.5;
        float dome = (1.0-smoothstep(0.1,0.5,length(pf)))*(1.0-smoothstep(0.2,0.6,pfw));
        color = fixture_linear(frp*(1.0+0.08*dome-0.18*flange)*mottle,OUTPUT_IS_SRGB);
        rough = 0.55;
        metal = 0.0;
        spec = 0.45;
    } else if(role==ROLE_STAINLESS) {
        // Brushed stainless: faint seams every quarter floor up the shell.
        float seam = fixture_span(local_m.y/(0.25*pitch),0.0,0.02,m_fw/(0.25*pitch));
        color = fixture_linear(vec3(0.70,0.71,0.72)*(1.0-0.25*seam),OUTPUT_IS_SRGB);
        rough = 0.32;
        metal = 0.85;
        spec = 0.5;
    } else if(role==ROLE_SOLAR) {
        // Modules a third of a floor wide (silver frames between them), each
        // a 6 x 10 grid of dark blue cells with thin light gaps.
        vec2 q = vec2(local_m.x/(pitch/3.0),face.y*1.6);
        vec2 qfw = vec2(m_fw/(pitch/3.0),fw*1.6);
        float frames = max(fixture_span(q.x+0.02,0.0,0.04,qfw.x),fixture_band(face.y,0.0,0.03,fw)+fixture_band(face.y,0.97,1.0,fw));
        float gaps = max(fixture_span(q.x*6.0,0.0,0.08,qfw.x*6.0),fixture_span(q.y*6.25,0.0,0.08,qfw.y*6.25));
        vec3 cells = mix(vec3(0.05,0.08,0.16),vec3(0.30,0.33,0.40),gaps*0.6);
        color = fixture_linear(mix(cells,vec3(0.66,0.68,0.70),clamp(frames,0.0,1.0)),OUTPUT_IS_SRGB);
        rough = mix(0.12,0.4,frames);
        metal = mix(0.1,0.6,frames);
        spec = 0.6;
    } else if(role==ROLE_RAIL || role==ROLE_POST) {
        // Guard railings: galvanised, green, white or black paint.
        uint paint = facade_pcg(bits^747796405u)%4u;
        vec3 rail = paint==0u ? vec3(0.60,0.61,0.60) : (paint==1u ? vec3(0.20,0.36,0.27) : (paint==2u ? vec3(0.84,0.84,0.82) : vec3(0.13,0.13,0.14)));
        color = fixture_linear(rail,OUTPUT_IS_SRGB);
        rough = 0.5;
        metal = paint==0u ? 0.6 : 0.2;
    } else if(role==ROLE_DISH) {
        color = fixture_linear(vec3(0.86,0.86,0.84),OUTPUT_IS_SRGB);
        rough = 0.5;
        metal = 0.0;
    }
    color *= shade;
    // Weather as on walls: wet darkens and gloss, snow on upward faces.
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*facing_up*0.92);
    ALBEDO = color;
    ROUGHNESS = mix(rough,0.28,weather.x);
    METALLIC = metal;
    SPECULAR = spec;
    // Sign faces glow all night like the storefront signs, fading with the
    // surface effect distance as the wall signs do; door lamps likewise.
    if(clock.z<0.0) EMISSION = glow*effect;
}
