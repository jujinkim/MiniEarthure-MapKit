shader_type spatial;
render_mode cull_disabled;
#include "wet_surface.gdshaderinc"
#include "material_detail.gdshaderinc"
#include "facade_signs.gdshaderinc"
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
// Highest water surface of the cell in mesh space (set per earth mesh by the
// chunk renderer); below about 15 cm above it the earth reads wet.
instance uniform float shore_level_m = -100000.0;
// Grass ground: broad lush/dry patches, mown clumps and fine blades in world space.
uniform bool lawn = false;
// Reach of the grass field's blades and the shade under them, set by the
// field on its cells' lawn materials (grass_field.gd; 0 while it shows none):
// the lawn darkens toward the camera as the blades' shade.
uniform float grass_field_distance = 0.0;
uniform float grass_field_shade = 0.0;
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
// Record palettes (sRGB, facade_palette.json) bound per building material:
// body (seed bits 0..2), roof (bits 3..5) and apartment accent (bits 6..8).
uniform vec3 facade_body[8];
uniform vec3 facade_roof[8];
uniform vec3 facade_accent[8];
uniform bool facade_palette = false;
// Wall finishes of the building key (facade_palette.json `finishes`), one per
// record by an integer hash of its seed: x facade tile layer (-1 none), y tile
// size in floors, z roughness, w metallic (at most 0.1: walls keep their
// diffuse energy, so near and distant walls match); look: x specular, y curtain wall,
// z cumulative weight (0..1].
uniform vec4 facade_finish[6];
uniform vec4 facade_finish_look[6];
uniform int facade_finish_count = 0;
// Daylit glass reflects at least this share of what it faces (Schlick base; the
// 1:3 panes have no geometry to reflect, so it stands in for the opposite street).
const float GLASS_REFLECTANCE = 0.10;
// Share of the glass tint left in daylight when the reflection is drawn.
const float GLASS_BODY = 0.45;
// Commercial walls draw procedural storefront signs and window lettering
// (facade_signs.gdshaderinc); environment_materials.bind_signs enables them.
uniform bool signs_enabled = false;
varying vec3 world_position;
varying vec3 world_normal;
varying float seed;
varying float wet_height;
// Generated building solids carry UV2 (seed, 1000 + base metres); -1 elsewhere.
// Every vertex of a solid holds the same values, so they are not interpolated:
// interpolation error would make per-pixel choices (signs, lit windows) flicker.
varying flat float solid_seed;
varying flat float solid_base;
// Packed facade solids (every generated building kind) also carry 1000 + their
// top (eaves) height in CUSTOM0.x, negated on triangles that rise above it
// (gable ends, taller parts of a merged solid, roof slopes); other meshes read
// 0 (height -1), and so do flat roof faces, which carry their roof frame there
// instead (roof_frame below). Commercial storeys fit below the top; every kind
// draws its parapet coping there, but only on walls that end at it (solid_coping 1).
varying flat float solid_height;
varying flat float solid_coping;
// The record's wall finish slot (-1 none), picked once per solid in vertex().
varying flat int finish_slot;
// Flat roof faces of packed solids carry their roof frame in CUSTOM0.x (4000 +
// the facade walls' direction modulo 90°, packed.rs roof_frame, read above
// 3500) instead of a top:
// roof finishes run square to the walls; 0 elsewhere.
varying flat float roof_frame;
float facade_hash(vec2 v) { return fract(sin(dot(v,vec2(12.9898,78.233)))*43758.5453); }
// Integer cells for facade_pcg (facade_signs.gdshaderinc), the hash of every
// discrete facade choice; the sine hash above is for continuous noise only.
uint facade_cell(float v) { return uint(int(v)+1048576); }
// Box-filtered share of the span [a,b] of each unit cell over a footprint of w
// cells centred on x (exact pulse integral, taken relative to x's own cell so
// large wall coordinates keep their precision): thin mullions and spandrels
// settle on their area share instead of aliasing.
float facade_span_integral(float t, float a, float b) { return floor(t)*(b-a)+clamp(fract(t),a,b)-a; }
float facade_span(float x, float a, float b, float w) {
    float t = fract(x);
    w = max(w,1e-4);
    return clamp((facade_span_integral(t+0.5*w,a,b)-facade_span_integral(t-0.5*w,a,b))/w,0.0,1.0);
}
// Box-filtered share of the single interval [a,b] over a footprint w centred
// on x (coping and drip bands at an absolute height).
float facade_band(float x, float a, float b, float w) {
    w = max(w,1e-4);
    return clamp((min(x+0.5*w,b)-max(x-0.5*w,a))/w,0.0,1.0);
}
// What daylit glass reflects along a ray rising by `rise`: the procedural sky's
// own zenith/horizon gradient and cloud grey (atmosphere_sky.gdshader), the
// street and opposite walls below the horizon.
vec3 facade_sky(float rise, float clouds) {
    vec3 sky = mix(vec3(0.16,0.28,0.40),vec3(0.58,0.57,0.51),pow(1.0-max(0.0,rise),4.0));
    sky = mix(sky,vec3(0.27,0.30,0.34),clouds*0.85);
    return mix(sky,vec3(0.10,0.11,0.10),smoothstep(0.0,0.25,-rise));
}
// Commercial storeys (scene metres): (shop storey, upper pitch). With the solid
// height known, a 1.5-floor shop storey and whole storeys fitted below the top
// (pitch 0: a remainder under 0.75 floors is a plain parapet; the 0.01 bias keeps
// exact floor multiples whole despite f32 offsets). Unknown heights keep floor rows.
vec2 facade_storeys(float height, float pitch) {
    if(height<=0.0) return vec2(pitch);
    float ground = min(1.5*pitch,height);
    float upper = height-ground;
    return vec2(ground,upper<0.75*pitch ? 0.0 : upper/max(1.0,floor(upper/pitch+0.01)));
}
// Commercial storey row at v metres above the base: the shop storey is row 0,
// fitted storeys reach the top and floor-pitch rows continue above it (gable
// ends, taller parts of a merged solid).
float facade_row(float v, vec2 storeys, float height, float pitch) {
    if(v<storeys.x) return v/storeys.x;
    if(height<=0.0) return 1.0+(v-storeys.x)/pitch;
    if(v<height) return storeys.y>0.0 ? 1.0+(v-storeys.x)/storeys.y : 1.0;
    return 1.0+(storeys.y>0.0 ? round((height-storeys.x)/storeys.y) : 0.0)+(v-height)/pitch;
}
// Shop storey part at s metres across a sign slot, w = height/shop storey:
// 0 pier/wall, 1 stall riser, 2 display glass, 3 door glass, 4 frame or mullion,
// 5 transom glass, 6 canopy shadow, 7 fascia (the sign band).
int facade_storefront(float s, float w, float ground, float width, float door, float mullions, float pitch) {
    if(w>=0.98) return 0;
    if(w>=0.78) return 7;
    if(w>=0.765) return 6;
    float frame = 0.025*pitch;
    float d = abs(s-door);
    if(w>=0.08 && w<0.72 && d<0.15*pitch) return (d<0.15*pitch-frame && w<0.72-frame/ground) ? 3 : 4;
    if(w<0.08) return 1;
    if(s<0.05*pitch || s>width-0.05*pitch) return 0;
    if(w>=0.10 && w<0.68) {
        float pane = width*0.5/(mullions+1.0);
        return abs(s-round(s/pane)*pane)<0.012*pitch ? 4 : 2;
    }
    if(w>=0.70 && w<0.76) return 5;
    return 4;
}
float lawn_noise(vec2 p) {
    vec2 i=floor(p); vec2 f=fract(p); f=f*f*(3.0-2.0*f);
    float a=facade_hash(i), b=facade_hash(i+vec2(1.0,0.0)), c=facade_hash(i+vec2(0.0,1.0)), d=facade_hash(i+vec2(1.0,1.0));
    return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
}
vec3 facade_linear(vec3 c, bool srgb_output) { return srgb_output ? c : mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c)); }
// Roof finishes of generated buildings (display only): a tint multiplying the
// record's roof palette colour, whose area mean stays about 1 (patterns are
// zero-mean or divided by their mean share, and settle with the pixel
// footprint), so far roofs keep the distant mesh's colour; the finish's
// roughness, metallic and specular; and (high) a world-space normal tilt.
struct RoofFinish { vec3 tint; float roughness; float metallic; float specular; vec3 tilt; };
// Flat roofs drain to roof drains on a 5-8 floor grid (record phase): dark
// stains round each drain grate and dirtier valley lines between them. The
// darkening divided by its area mean (until it settles with the footprint).
float roof_drains(vec2 q, vec2 qfw, uint bits, int quality) {
    float spacing = 5.0+float((bits>>3u)&3u);
    vec2 nd = abs(fract(q/spacing+vec2(facade_unit(bits^13u),facade_unit(bits^17u))+0.5)-0.5)*spacing;
    float settle = 1.0-smoothstep(0.12,0.35,max(qfw.x,qfw.y));
    float valley = max(1.0-smoothstep(0.0,0.18,nd.x),1.0-smoothstep(0.0,0.18,nd.y));
    float ragged = quality>0 ? 0.22*lawn_noise(q*3.0+vec2(float(bits&255u))) : 0.11;
    float stain = 1.0-smoothstep(0.12,0.42+ragged,length(nd));
    float grate = facade_band(nd.x,0.0,0.035,qfw.x)*facade_band(nd.y,0.0,0.035,qfw.y);
    float dark = (0.08*valley+0.26*stain+0.45*grate)*settle;
    float mean = (0.08*0.36/spacing+0.26*0.38/(spacing*spacing))*settle;
    return (1.0-dark)/(1.0-mean);
}
// p world position, n its normal, pdx/pdy its screen derivatives (taken in
// uniform control flow by the caller), palette the record's roof colour
// (sRGB), frame the flat roof frame angle, eaves the eaves height of pitched
// faces (-1e5 unknown), pitch the floor pitch (metres), quality detail_quality.
RoofFinish roof_finish(vec3 p, vec3 n, vec3 pdx, vec3 pdy, vec3 palette, int record, int kind, float frame, float eaves, float pitch, int quality) {
    RoofFinish r = RoofFinish(vec3(1.0),0.85,0.0,0.35,vec3(0.0));
    uint bits = facade_pcg(uint(record)^1380919089u);
    // The palette family: green (urethane coating), red-brown (coated villa
    // roofs and clay tiles) or grey.
    bool green = palette.g>palette.r+0.03 && palette.g>=palette.b-0.02;
    bool red = palette.r>palette.g+0.08;
    float luma = dot(palette,vec3(0.299,0.587,0.114));
    vec3 chroma = (palette-vec3(luma))/max(luma,0.05);
    float shade = 1.0;
    vec3 hue = vec3(1.0);
    if(n.y>0.97) {
        // Flat roofs, in floor pitches along the roof frame.
        vec3 ru = vec3(cos(frame),0.0,sin(frame));
        vec3 rv = vec3(-sin(frame),0.0,cos(frame));
        vec2 q = vec2(dot(p,ru),dot(p,rv))/pitch;
        vec2 qfw = (abs(vec2(dot(pdx,ru),dot(pdx,rv)))+abs(vec2(dot(pdy,ru),dot(pdy,rv))))/pitch;
        float fp = max(qfw.x,qfw.y);
        // Green and red roofs are urethane coated; grey ones per record a grey
        // coating or a concrete slab, on industrial roofs also a sheet
        // membrane, on public roofs a membrane or gravel ballast.
        uint pick = (bits>>8u)%100u;
        int finish = 0;
        if(!green && !red) {
            if(kind==4) finish = pick<50u ? 2 : (pick<80u ? 1 : 0);
            else if(kind==5) finish = pick<35u ? 0 : (pick<70u ? 1 : (pick<85u ? 2 : 3));
            else finish = pick<(kind==2 ? 60u : 50u) ? 0 : 1;
        }
        if(finish==0) {
            // Urethane coating: roller passes 0.12 floors wide along one frame
            // axis (each a little lighter or darker, a lap ridge at its edge),
            // a fine roller stipple, repaired (fresher, more saturated) and
            // sun-faded (chalky, greyer) patches, broad weathering.
            r.roughness = 0.55;
            r.specular = 0.45;
            vec2 rq = (bits&1u)==1u ? q.yx : q;
            vec2 rfw = (bits&1u)==1u ? qfw.yx : qfw;
            float lanes = rq.x/0.12;
            float lane_fw = rfw.x/0.12;
            float lane_tone = facade_unit(bits^facade_pcg(facade_cell(floor(lanes))^2654435761u))-0.5;
            float lap = facade_span(lanes,0.0,0.1,lane_fw)-0.1;
            shade *= 1.0+(0.07*lane_tone+0.06*lap)*(1.0-smoothstep(0.2,0.5,lane_fw));
            if(quality>0) shade *= 1.0+0.05*(lawn_noise(vec2(rq.x*70.0,rq.y*9.0))-0.5)*(1.0-smoothstep(0.004,0.012,fp));
            vec2 pg = q/3.0+vec2(facade_unit(bits^7u),facade_unit(bits^11u));
            vec2 pf = fract(pg);
            vec2 pfw = qfw/3.0;
            uint patch_bits = facade_pcg(bits^facade_pcg(facade_cell(floor(pg.x))^facade_pcg(facade_cell(floor(pg.y))^2246822519u)));
            float steady = 1.0-smoothstep(0.08,0.25,max(pfw.x,pfw.y));
            if(patch_bits%100u<40u) {
                vec2 lo = vec2(0.05)+0.3*vec2(float((patch_bits>>8u)&15u),float((patch_bits>>12u)&15u))/15.0;
                vec2 hi = min(lo+vec2(0.3)+0.45*vec2(float((patch_bits>>16u)&15u),float((patch_bits>>20u)&15u))/15.0,vec2(0.95));
                float inside = facade_band(pf.x,lo.x,hi.x,pfw.x)*facade_band(pf.y,lo.y,hi.y,pfw.y);
                float edge = inside-facade_band(pf.x,lo.x+0.012,hi.x-0.012,pfw.x)*facade_band(pf.y,lo.y+0.012,hi.y-0.012,pfw.y);
                bool fresh = ((patch_bits>>24u)&1u)==0u;
                hue = mix(hue,fresh ? 0.88*(vec3(1.0)+0.45*chroma) : 1.11*(vec3(1.0)-0.45*chroma),inside*steady);
                shade *= 1.0-0.15*edge*steady;
            }
            if(quality>0) shade *= 1.0+0.1*(lawn_noise(q*0.45+vec2(float(bits&1023u)))-0.5);
        } else if(finish==1) {
            // Concrete slab: expansion joints every floor (3 m) with dark
            // sealant, each bay a little lighter or darker, a fine aggregate.
            r.roughness = 0.9;
            r.specular = 0.3;
            vec2 jq = q+vec2(facade_unit(bits^19u),facade_unit(bits^23u));
            float joints = max(facade_span(jq.x,0.0,0.014,qfw.x),facade_span(jq.y,0.0,0.014,qfw.y));
            float bay = facade_unit(bits^facade_pcg(facade_cell(floor(jq.x))^facade_pcg(facade_cell(floor(jq.y))^1013904223u)))-0.5;
            shade *= (1.0+0.07*bay*(1.0-smoothstep(0.15,0.4,fp)))*(1.0-0.5*joints)/(1.0-0.5*0.028);
            if(quality>0) shade *= 1.0+0.07*(lawn_noise(q*45.0)-0.5)*(1.0-smoothstep(0.008,0.025,fp));
        } else if(finish==2) {
            // Sheet membrane: sheets 0.34 floors (1 m) wide with a lighter lap
            // band and a dark seam edge, each sheet slightly different.
            r.roughness = 0.7;
            r.specular = 0.4;
            vec2 mq = (bits&2u)!=0u ? q.yx : q;
            vec2 mfw = (bits&2u)!=0u ? qfw.yx : qfw;
            float sheets = mq.x/0.34+facade_unit(bits^29u);
            float sheet_fw = mfw.x/0.34;
            float lap = facade_span(sheets,0.0,0.07,sheet_fw);
            float seam = facade_span(sheets,0.07,0.085,sheet_fw);
            float tone = facade_unit(bits^facade_pcg(facade_cell(floor(sheets))^3266489917u))-0.5;
            shade *= (1.0+0.04*tone*(1.0-smoothstep(0.2,0.5,sheet_fw)))*(1.0+0.06*lap-0.3*seam)/(1.0+0.06*0.07-0.3*0.015);
        } else {
            // Gravel ballast: pebble grain and broad drifts.
            r.roughness = 0.95;
            r.specular = 0.25;
            if(quality>0) shade *= (1.0+0.05*(lawn_noise(q*2.5+3.7)-0.5))*(1.0+0.18*(lawn_noise(q*35.0)-0.5)*(1.0-smoothstep(0.01,0.03,fp)));
        }
        shade *= roof_drains(q,qfw,bits,quality);
    } else {
        // Pitched faces, in floor pitches across and down the slope.
        vec3 across = normalize(vec3(-n.z,0.0,n.x));
        vec3 down = normalize(cross(across,n));
        if(down.y>0.0) down = -down;
        vec2 q = vec2(dot(p,across),dot(p,down))/pitch;
        vec2 qfw = (abs(vec2(dot(pdx,across),dot(pdx,down)))+abs(vec2(dot(pdy,across),dot(pdy,down))))/pitch;
        float rise = eaves>-1e4 ? (p.y-eaves)/pitch : 1.0;
        // Red roofs: clay tile courses; grey ones asphalt shingles or (per
        // record) standing-seam metal, green ones coloured steel sheet.
        int cover = red ? 0 : (green || (bits&1u)==1u ? 2 : 1);
        if(cover==0) {
            float course = q.y/0.11;
            float cfw = qfw.y/0.11;
            float row = floor(course);
            float tq = q.x/0.085+0.5*mod(row,2.0);
            float tfw = qfw.x/0.085;
            float tile_steady = 1.0-smoothstep(0.2,0.5,max(tfw,cfw));
            float barrel = cos(6.2831853*tq)*(1.0-smoothstep(0.2,0.5,tfw));
            float lip = facade_span(course,0.82,1.0,cfw);
            float tone = facade_unit(bits^facade_pcg(facade_cell(row)^facade_pcg(facade_cell(floor(tq))^1540483477u)))-0.5;
            shade *= (1.0+0.10*barrel+0.12*tone*tile_steady)*(1.0-0.3*lip)/(1.0-0.3*0.18);
            r.roughness = 0.7;
            r.specular = 0.4;
            if(quality>1) r.tilt = across*(-sin(6.2831853*tq))*0.25*(1.0-smoothstep(0.1,0.3,tfw));
        } else if(cover==1) {
            float course = q.y/0.05;
            float cfw = qfw.y/0.05;
            float row = floor(course);
            float tq = q.x/0.11+0.5*mod(row,2.0);
            float tfw = qfw.x/0.11;
            float slot = facade_span(tq,0.0,0.06,tfw)*facade_span(course,0.0,0.7,cfw);
            float shadow = facade_span(course,0.85,1.0,cfw);
            float tone = facade_unit(bits^facade_pcg(facade_cell(row)^facade_pcg(facade_cell(floor(tq))^2166136261u)))-0.5;
            shade *= (1.0+0.14*tone*(1.0-smoothstep(0.2,0.5,max(tfw,cfw))))*(1.0-0.4*slot-0.25*shadow)/(1.0-0.4*0.042-0.25*0.15);
            r.roughness = 0.9;
            r.specular = 0.3;
        } else {
            float sq = q.x/0.15;
            float sfw = qfw.x/0.15;
            float crest = facade_span(sq,0.0,0.04,sfw);
            float shadow = facade_span(sq,0.04,0.09,sfw);
            shade *= 1.0+0.15*crest-0.12*shadow;
            r.roughness = 0.4;
            r.metallic = 0.1;
            r.specular = 0.5;
        }
        // Eaves: a gutter shadow along the eaves line, moss and grime
        // creeping up from it.
        shade *= 1.0-0.35*facade_band(rise,0.0,0.04,max(qfw.y*abs(down.y),1e-4));
        if(quality>0) {
            float moss = (1.0-smoothstep(0.0,0.45,rise))*smoothstep(0.45,0.75,lawn_noise(q*4.0+vec2(float(bits&255u))));
            hue *= mix(vec3(1.0),vec3(0.82,0.92,0.74),0.6*moss);
        }
    }
    r.tint = hue*shade;
    return r;
}
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
    wet_height = VERTEX.y - shore_level_m;
    seed = distant ? UV.x : (INSTANCE_CUSTOM.a > 0.5 ? INSTANCE_CUSTOM.x : building_seed);
    solid_seed = !distant && facade_kind>0 && UV2.y>0.5 ? UV2.x : -1.0;
    solid_base = solid_seed>=0.0 ? (MODEL_MATRIX * vec4(0.0,UV2.y-1000.0,0.0,1.0)).y : 0.0;
    solid_height = solid_seed>=0.0 && abs(CUSTOM0.x)>500.0 && CUSTOM0.x<=3500.0 ? (MODEL_MATRIX * vec4(0.0,abs(CUSTOM0.x)-1000.0,0.0,1.0)).y-solid_base : -1.0;
    solid_coping = solid_height>0.0 && CUSTOM0.x>0.0 ? 1.0 : 0.0;
    roof_frame = solid_seed>=0.0 && CUSTOM0.x>3500.0 ? CUSTOM0.x-4000.0 : 0.0;
    // A record (one site and category) shares one wall finish: the integer PCG
    // of its 12-bit seed against the key's cumulative weights.
    finish_slot = -1;
    if(solid_seed>=0.0 && facade_palette && facade_finish_count>0) {
        float finish_pick = facade_unit(uint(int(floor(solid_seed*4096.0)))^1759714724u);
        finish_slot = facade_finish_count-1;
        for(int i=0;i<6;i++) { if(i<facade_finish_count && finish_pick<facade_finish_look[i].z) { finish_slot=i; break; } }
    }
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
    // Screen derivatives in uniform control flow, for facade footprints (signs,
    // lettering, box-filtered bands) and the lawn relief fade.
    vec3 position_dx=dFdx(world_position);
    vec3 position_dy=dFdy(world_position);
    float effect_weight=surface_effect_weight(view_distance);
    float surface_roughness=roughness;
    float surface_metallic=metallic;
    float surface_specular=0.35;
    // A record (one site and category) shares its palette and wall finish; the
    // distant mesh picks the same body/roof entries, so colours never fade.
    bool facade_record = facade_palette && facade_kind>0 && !distant && solid_seed>=0.0;
    int record = facade_record ? int(floor(solid_seed*4096.0)) : 0;
    bool facade_surface = facade_kind>0 && !distant && abs(world_normal.y)<0.5;
    int finish = facade_record && facade_surface ? finish_slot : -1;
    // Curtain walls (commercial only) are glass and frames without a wall tile.
    bool curtain = finish>=0 && facade_kind==3 && facade_finish_look[max(finish,0)].y>0.5;
    bool facade_relief = finish>=0 && facade_tiles_enabled && facade_finish[max(finish,0)].x>=0.0;
    // Daylit glass adds what it reflects as emission (below), never as paint.
    vec3 glass_reflection = vec3(0.0);
    // Wall plane: u along the wall, v up; screen gradients in metres.
    vec3 wall_tangent = facade_surface ? normalize(vec3(-world_normal.z,0.0,world_normal.x)) : vec3(1.0,0.0,0.0);
    vec2 wall_dx = vec2(dot(position_dx,wall_tangent),position_dx.y);
    vec2 wall_dy = vec2(dot(position_dy,wall_tangent),position_dy.y);
    if(effect_weight>0.0) {
        if(roughness_enabled) surface_roughness*=mix(1.0,dot(texture(roughness_texture,UV),roughness_channel),effect_weight);
        if(normal_enabled) { NORMAL_MAP=texture(normal_texture,UV).rgb; NORMAL_MAP_DEPTH=normal_strength*effect_weight; }
        if(ao_enabled) { AO=mix(1.0,dot(texture(ao_texture,ao_uv2 ? UV2 : UV),ao_channel),effect_weight); AO_LIGHT_AFFECT=ao_light_affect; }
    }
    if(detail_enabled && !facade_relief && !curtain && view_distance<48.0 && effect_weight>0.0) {
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
    // Generated walls: floor bands, window panes and a per-building seed.
    // Palette and window variants fade with effect weight, so far near walls
    // keep the distant mesh's colour. Roofs, bottoms, distant meshes and
    // imported assets keep their own appearance.
    float facade_wall = 0.0;
    float facade_window = 0.0;
    float facade_lit = 0.0;
    // Curtain walls glaze about twice a punched wall's pane area: half as many
    // of their panes are lit, at 60 % strength (offices go dark at night).
    float facade_glow = 1.0;
    float facade_seed = seed;
    // Night emission of signs and window lettering (lit all night).
    vec3 facade_sign = vec3(0.0);
    // A lit room's light colour and brightness, and how its dressing and depth
    // shape it per pixel (facade windows; the far average stays warm).
    vec3 facade_room = vec3(1.0,0.63,0.28);
    vec3 facade_pattern = vec3(1.0);
    float solid_jitter = facade_record ? fract(solid_seed*4096.0) : seed;
    vec3 facade_base = max(base_color.rgb,vec3(0.02));
    if(facade_record && world_normal.y>=0.5) {
        vec3 roof_palette = facade_roof[(record>>3)&7];
        color *= facade_linear(roof_palette,OUTPUT_IS_SRGB)/facade_base;
        // Roof finishes (roof_finish): coatings, slabs, membranes, gravel,
        // tiles, shingles or sheet metal, settling to the palette colour.
        if(effect_weight>0.0) {
            vec3 up = normalize(world_normal);
            RoofFinish roof = roof_finish(world_position,up,position_dx,position_dy,roof_palette,record,facade_kind,roof_frame,up.y<0.97 && solid_height>0.0 ? solid_base+solid_height : -1e5,floor_pitch,detail_quality);
            color *= mix(vec3(1.0),roof.tint,effect_weight);
            surface_roughness = mix(surface_roughness,roof.roughness,effect_weight);
            surface_metallic = mix(surface_metallic,roof.metallic,effect_weight);
            surface_specular = mix(surface_specular,roof.specular,effect_weight);
            if(detail_quality>1 && dot(roof.tilt,roof.tilt)>0.0) NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(roof.tilt*effect_weight,0.0)).xyz);
        }
    }
    if(facade_kind>0 && !distant && abs(world_normal.y)<0.5) {
        facade_wall = 1.0;
        vec3 n = normalize(world_normal);
        // Older packed meshes without UV2 vary each wall plane instead.
        facade_seed = solid_seed>=0.0 ? solid_seed : facade_hash(vec2(round(dot(n,world_position)*4.0),round(n.x*8.0)+round(n.z*8.0)*17.0));
        if(facade_record) color *= facade_linear(facade_body[record&7],OUTPUT_IS_SRGB)/facade_base;
        if(effect_weight>0.0) {
            float variant = facade_record ? float((record>>3)&3) : floor(fract(facade_seed*7.31)*4.0);
            if(!facade_record) {
                vec3 tone = variant<1.0 ? vec3(1.06,1.0,0.90) : (variant<2.0 ? vec3(0.93,0.98,1.06) : (variant<3.0 ? vec3(1.08,0.90,0.80) : vec3(1.07,1.07,1.05)));
                color *= mix(vec3(1.0),tone*(0.92+0.16*fract(facade_seed*13.7)),effect_weight);
            }
            float u = dot(world_position,normalize(vec3(-n.z,0.0,n.x)));
            float v = world_position.y-solid_base;
            float bay = facade_kind==1 ? 1.3 : (facade_kind==2 ? 1.8 : (facade_kind==3 ? 0.8 : (facade_kind==4 ? 1.0 : 1.5)));
            bay *= facade_record ? 0.8+0.5*float((record>>9)&7)/7.0 : 0.8+0.5*fract(facade_seed*3.17);
            // Commercial walls: the shop storey is cell row 0, whole storeys above.
            vec2 storeys = facade_storeys(facade_kind==3 ? solid_height : -1.0,floor_pitch);
            bool shop = facade_kind==3 && v<storeys.x;
            vec2 cell = vec2(u/(floor_pitch*bay),facade_kind!=3 ? v/floor_pitch : facade_row(v,storeys,solid_height,floor_pitch));
            vec2 f = fract(cell);
            // Cell size in metres (fitted commercial storeys are taller than a
            // floor) and a pixel's footprint in cells, from analytic gradients
            // (fract and the piecewise rows would break fwidth).
            vec2 cell_m = vec2(floor_pitch*bay,facade_kind==3 && !shop && v<solid_height && storeys.y>0.0 ? storeys.y : floor_pitch);
            vec2 cell_fw = (abs(wall_dx)+abs(wall_dy))/cell_m;
            // The pixel footprint in metres (signs, lettering, fine detail).
            float footprint = max(length(wall_dx),length(wall_dy));
            vec4 pane = facade_kind==1 ? vec4(0.2,0.75,0.3,0.75) : (facade_kind==2 ? vec4(0.3,0.7,0.3,0.7) : (facade_kind==3 ? vec4(0.1,0.9,0.15,0.85) : (facade_kind==4 ? vec4(0.15,0.85,0.6,0.85) : vec4(0.3,0.7,0.2,0.85))));
            // Apartments, offices and public walls: tall, ribbon or square panes.
            if(facade_kind!=2 && facade_kind!=4) {
                if(variant>=1.0 && variant<2.0) pane = vec4(0.35,0.65,0.12,0.86);
                else if(variant>=2.0 && variant<3.0) pane = vec4(0.0,1.0,0.32,0.76);
                else if(variant>=3.0) pane = vec4(0.25,0.75,0.38,0.80);
            }
            // Curtain walls: glass between slim mullions over a spandrel band.
            if(curtain) pane = vec4(0.04,0.96,0.14,0.97);
            // Pane and spandrel shares are box-filtered over the pixel footprint.
            float window = facade_span(cell.x,pane.x,pane.y,cell_fw.x)*facade_span(cell.y,pane.z,pane.w,cell_fw.y);
            float spandrel = curtain ? facade_span(cell.y,0.0,pane.z,cell_fw.y) : 0.0;
            // Bays a wall leaves blank: apartment base storeys, every other
            // industrial bay, commercial parapets.
            float pane_mask = 1.0;
            // Apartment complexes: a coloured column every 5–8 bays and a
            // darker accent base storey give each complex its own identity.
            if(facade_record && facade_kind==1) {
                vec3 accent = facade_linear(facade_accent[(record>>6)&7],OUTPUT_IS_SRGB);
                float column = step(mod(floor(cell.x),float(5+((record>>9)&3))),0.5);
                float base_storey = step(v,floor_pitch*0.92);
                color = mix(color,accent,max(column*(1.0-window),base_storey*0.55)*effect_weight);
                pane_mask = 1.0-base_storey;
            }
            uint seed_bits = uint(round(clamp(facade_seed,0.0,1.0)*16777216.0));
            // Shop storey, per sign slot (one bay pair; seen from outside, slots
            // read left to right along -u): stall riser, one seeded door,
            // mullioned display windows, transom, canopy shadow and fascia.
            // A parapet too short for a storey stays plain.
            float slot_width = floor_pitch*bay*2.0;
            float slot = floor(-u/slot_width);
            float slot_s = fract(-u/slot_width)*slot_width;
            uint slot_bits = seed_bits^facade_pcg(facade_cell(slot));
            // Sign and window-lettering designs follow the 12-bit record and the
            // slot (as the wall finish does), so they hold when low seed bits wobble.
            uint sign_bits = facade_record ? facade_pcg(uint(record)^1478450703u)^facade_pcg(facade_cell(slot)^3266489917u) : slot_bits;
            int shop_part = -1;
            if(shop) {
                uint slot_layout = facade_pcg(slot_bits^2654435769u);
                shop_part = facade_storefront(slot_s,cell.y,storeys.x,slot_width,slot_width*(0.22+0.28*float(slot_layout%3u)),2.0+float((slot_layout>>2u)&1u),floor_pitch);
                window = (shop_part==2 || shop_part==3 || shop_part==5) ? 1.0 : 0.0;
                spandrel = 0.0;
            } else if(facade_kind==3 && storeys.y<=0.0 && v<solid_height) { pane_mask = 0.0; spandrel = 0.0; }
            // The finish sets the wall's roughness, metallic and specular (walls,
            // piers and stall risers; frames, canopy and fascia keep the plain
            // response, signs their acrylic gloss); far walls fade back to the
            // plain distant response.
            if(finish>=0 && shop_part<=1) {
                surface_roughness = mix(surface_roughness,facade_finish[finish].z,effect_weight);
                surface_metallic = mix(surface_metallic,facade_finish[finish].w,effect_weight);
                surface_specular = mix(surface_specular,facade_finish_look[finish].x,effect_weight);
            }
            if(facade_kind==4) {
                color *= 1.0-0.08*step(0.85,fract(u/(0.6*floor_pitch)));
                pane_mask *= step(0.5,facade_hash(vec2(floor(cell.x),floor(cell.y)*3.0)));
            }
            if(!shop) window *= pane_mask;
            if(facade_kind==1) color *= 1.0-0.14*facade_span(cell.y,0.94,1.0,cell_fw.y)*effect_weight;
            if(facade_kind==1 && fract(facade_seed*5.3)<0.5) color *= 1.0+0.08*facade_span(cell.y,0.0,0.1,cell_fw.y)*step(1.0,cell.y)*effect_weight;
            if(facade_kind==5) color *= 1.0+0.05*facade_span(cell.y,0.0,0.08,cell_fw.y)*effect_weight;
            // Openings, box-filtered in cell units so far walls settle on their
            // area shares: a reveal of wall inside each pane edge (0.06 storeys,
            // 0.015 on curtain walls; edges on the cell boundary, ribbon windows,
            // are continuous), then on punched windows a frame band, mullions or
            // a transom, a sill ledge with its shadow and a lintel shadow.
            float reveal = 0.0;
            vec2 reveal_side = vec2(0.0);
            vec4 glass_rect = pane;
            float lintel = 0.0;
            float frame_share = 0.0;
            // Weathering (medium/high): streaks under sills and the coping, grime at the base.
            float weathering = 0.0;
            float streaks = 1.0-smoothstep(0.012,0.03,footprint);
            uint frame_bits = facade_pcg(uint(record)^2166136261u);
            if(!shop && pane_mask>0.0) {
                float band = (curtain ? 0.015 : 0.06)*floor_pitch;
                vec2 band_c = band/cell_m;
                vec4 open = pane+vec4(pane.x>0.0 ? band_c.x : 0.0,pane.y<1.0 ? -band_c.x : 0.0,pane.z>0.0 ? band_c.y : 0.0,pane.w<1.0 ? -band_c.y : 0.0);
                float opening = facade_span(cell.x,open.x,open.y,cell_fw.x)*facade_span(cell.y,open.z,open.w,cell_fw.y)*pane_mask;
                float glass_share = opening;
                if(!curtain) {
                    // Frames 0.022 storeys wide: white PVC, silver or bronze on homes,
                    // dark or silver aluminium elsewhere (per record).
                    vec2 frame_c = 0.022*floor_pitch/cell_m;
                    glass_rect = open+vec4(frame_c.x,-frame_c.x,frame_c.y,-frame_c.y);
                    float glass_x = facade_span(cell.x,glass_rect.x,glass_rect.y,cell_fw.x);
                    float glass_y = facade_span(cell.y,glass_rect.z,glass_rect.w,cell_fw.y);
                    // Mullions: 1 a sliding pair (homes), 2 a centre bar and a transom
                    // (industrial), and per record on commercial upper storeys and
                    // public walls three panes (3), a transom (4), 2 or a fixed pane (0).
                    int split = facade_kind<=2 ? 1 : (facade_kind==4 ? 2 : ((frame_bits>>4u)%4u==0u ? 3 : ((frame_bits>>4u)%4u==1u ? 4 : ((frame_bits>>4u)%4u==2u ? 2 : 0))));
                    float gx0 = glass_rect.x;
                    float gw = glass_rect.y-glass_rect.x;
                    float bar = 0.8*frame_c.x;
                    float verticals = 0.0;
                    if(split==1 || split==2) verticals = facade_span(cell.x,gx0+0.5*gw-bar,gx0+0.5*gw+bar,cell_fw.x);
                    else if(split==3) verticals = facade_span(cell.x,gx0+gw/3.0-0.75*bar,gx0+gw/3.0+0.75*bar,cell_fw.x)+facade_span(cell.x,gx0+gw*2.0/3.0-0.75*bar,gx0+gw*2.0/3.0+0.75*bar,cell_fw.x);
                    float transom = 0.0;
                    if(split==2 || split==4) {
                        float ty = glass_rect.z+(glass_rect.w-glass_rect.z)*(facade_kind==4 ? 0.5 : 0.72);
                        transom = facade_span(cell.y,ty-0.8*frame_c.y,ty+0.8*frame_c.y,cell_fw.y);
                    }
                    glass_share = max(glass_x*glass_y-verticals*glass_y-transom*glass_x*(1.0-verticals),0.0)*pane_mask;
                    frame_share = max(opening-glass_share,0.0);
                    uint frame_tone = frame_bits%10u;
                    vec3 frame = facade_kind<=2 ? (frame_tone<6u ? vec3(0.76,0.76,0.74) : (frame_tone<8u ? vec3(0.55,0.56,0.58) : vec3(0.27,0.21,0.16))) : (frame_tone<5u ? vec3(0.15,0.16,0.17) : (frame_tone<9u ? vec3(0.56,0.58,0.60) : vec3(0.83,0.83,0.80)));
                    color = mix(color,facade_linear(frame,OUTPUT_IS_SRGB),frame_share*effect_weight);
                    lintel = facade_span(cell.y,glass_rect.w-0.18*(glass_rect.w-glass_rect.z),glass_rect.w,cell_fw.y)*glass_x*pane_mask;
                }
                // The reveal stays wall, 18 % darker; on medium/high its normal
                // tilts towards the opening near the camera, fading out before
                // the band is narrower than about two pixels.
                color *= 1.0-0.18*max(window-opening,0.0)*effect_weight;
                vec2 e0 = (f-pane.xz)*cell_m;
                vec2 e1 = (pane.yw-f)*cell_m;
                if(pane.x<=0.0) e0.x = 1e3;
                if(pane.y>=1.0) e1.x = 1e3;
                if(pane.z<=0.0) e0.y = 1e3;
                if(pane.w>=1.0) e1.y = 1e3;
                float across = min(e0.x,e1.x);
                float upright = min(e0.y,e1.y);
                float inset = min(across,upright);
                reveal = step(0.0,inset)*step(inset,band)*(1.0-smoothstep(0.2*band,0.4*band,footprint))*pane_mask;
                reveal_side = across<upright ? vec2(e0.x<e1.x ? 1.0 : -1.0,0.0) : vec2(0.0,e0.y<e1.y ? 1.0 : -1.0);
                window = glass_share;
                // Sill ledge: a lighter band from just below the pane over half the
                // bottom reveal, 0.03 storeys wider each side, its shadow below.
                if(!curtain && pane.z>0.0) {
                    float ext = 0.03*floor_pitch/cell_m.x;
                    float ledge = 0.035*floor_pitch/cell_m.y;
                    float sill_x = facade_span(cell.x,max(pane.x-ext,0.0),min(pane.y+ext,1.0),cell_fw.x)*pane_mask;
                    float sill = sill_x*facade_span(cell.y,max(pane.z-ledge,0.0),0.5*(pane.z+open.z),cell_fw.y);
                    float under = sill_x*facade_span(cell.y,max(pane.z-2.0*ledge,0.0),max(pane.z-ledge,0.0),cell_fw.y);
                    color = mix(color,mix(color*1.2,facade_linear(vec3(0.74,0.73,0.70),OUTPUT_IS_SRGB),0.5),sill*effect_weight);
                    color *= 1.0-0.28*under*effect_weight;
                    float drop = max(pane.z-2.0*ledge,1e-3);
                    if(detail_quality>0) weathering += 0.16*sill_x*clamp(f.y/drop,0.0,1.0)*facade_span(cell.y,0.0,drop,cell_fw.y)*mix(0.5,smoothstep(0.3,0.8,lawn_noise(vec2(u*17.0,v*0.7+3.0))),streaks);
                }
            }
            // Apartments: balconies on about 45 % of bay columns above the base
            // storey (slab edge, glass balustrade, a top rail); the slab above
            // shades the top of each storey below it.
            if(facade_kind==1 && (solid_height<=0.0 || v<solid_height)) {
                uint balcony_bits = facade_pcg(uint(record)^facade_pcg(facade_cell(floor(cell.x))^3141592653u));
                if(balcony_bits%100u<45u) {
                    float span_x = facade_span(cell.x,0.04,0.96,cell_fw.x);
                    float row_above = solid_height<=0.0 || v+floor_pitch<solid_height ? 1.0 : 0.0;
                    color *= 1.0-0.3*span_x*facade_span(cell.y,0.90,1.0,cell_fw.y)*row_above*effect_weight;
                    if(cell.y>=1.0) {
                        float slab = span_x*facade_span(cell.y,0.0,0.06,cell_fw.y);
                        float guard = span_x*facade_span(cell.y,0.06,0.33,cell_fw.y);
                        float rail = span_x*facade_span(cell.y,0.33,0.36,cell_fw.y);
                        color = mix(color,facade_linear(vec3(0.30,0.38,0.40),OUTPUT_IS_SRGB),0.45*guard*effect_weight);
                        color = mix(color,facade_linear(vec3(0.74,0.73,0.70),OUTPUT_IS_SRGB),slab*effect_weight);
                        color = mix(color,facade_linear((balcony_bits>>8u)%3u==0u ? vec3(0.20,0.21,0.22) : vec3(0.86,0.86,0.84),OUTPUT_IS_SRGB),rail*effect_weight);
                        surface_roughness = mix(surface_roughness,0.15,guard*effect_weight);
                        window *= 1.0-0.5*guard-slab-rail;
                    }
                }
            }
            // Parapet coping at the solid top (every kind with a known top, on
            // walls that end there): a light cap 0.05 storeys high and a drip
            // shadow below it.
            float cap = 0.0;
            float drip = 0.0;
            if(solid_coping>0.5) {
                float fw_v = cell_fw.y*cell_m.y;
                cap = facade_band(v,solid_height-0.05*floor_pitch,solid_height,fw_v);
                drip = facade_band(v,solid_height-0.085*floor_pitch,solid_height-0.05*floor_pitch,fw_v)*(shop_part==7 ? 0.0 : 1.0);
                window *= clamp(1.0-cap-drip,0.0,1.0);
                if(detail_quality>0) {
                    float below = solid_height-0.085*floor_pitch-v;
                    weathering += 0.12*clamp(1.0-below/(0.8*floor_pitch),0.0,1.0)*facade_band(v,solid_height-0.885*floor_pitch,solid_height-0.085*floor_pitch,fw_v)*mix(0.5,smoothstep(0.3,0.8,lawn_noise(vec2(u*13.0+31.7,v*0.5))),streaks);
                }
            }
            // Base grime: mottled value noise (0.4 × 0.25 m cells) settling on
            // its mean once the cells near a pixel (grazing views included).
            if(detail_quality>0) weathering += 0.16*(1.0-smoothstep(0.0,0.3*floor_pitch,v))*mix(1.0,0.7+0.6*lawn_noise(vec2(u*2.5,v*4.0)),1.0-smoothstep(0.05,0.12,footprint));
            // Curtain walls: neutral anodised mullions and transoms, dark coated
            // spandrel glass over each slab (area shares, so far grids settle).
            if(curtain && !shop) {
                float slab = clamp(spandrel/max(1.0-window,1e-3),0.0,1.0);
                color = mix(color,mix(vec3(0.30,0.31,0.33),vec3(0.10,0.11,0.12),slab),effect_weight);
                surface_roughness = mix(surface_roughness,0.22,slab*effect_weight);
                surface_specular = mix(surface_specular,0.5,slab*effect_weight);
            }
            // Wall relief from the finish's facade tile (walls, piers and stall
            // risers; never glass, frames, fascia or signs). The tile's luminance
            // reaches the effect distance on medium/high (mipmapped, anisotropic:
            // only coarse joints and courses remain far); roughness, AO and the
            // bump stay within the 32–48 m detail range.
            float near_relief = material_detail_weight(view_distance)*max(1.0-window-frame_share,0.0);
            float tone_relief = (detail_quality>0 ? effect_weight : material_detail_weight(view_distance))*max(1.0-window-frame_share,0.0);
            if(facade_relief && shop_part<=1 && tone_relief>0.0) {
                float tile_m = max(facade_finish[finish].y*floor_pitch,0.01);
                vec4 detail = textureGrad(facade_tiles,vec3(vec2(u,v)/tile_m,facade_finish[finish].x),wall_dx/tile_m,wall_dy/tile_m);
                color *= mix(1.0,facade_luminance(detail.r),tone_relief);
                surface_roughness = clamp(surface_roughness*mix(1.0,detail.g*2.0,near_relief),0.04,1.0);
                if(detail_quality>0 && near_relief>0.0) {
                    AO = mix(1.0,detail.r/0.65,0.18*near_relief);
                    NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(facade_bump(detail.ba,wall_tangent)*near_relief,0.0)).xyz);
                }
            }
            if(detail_quality>0 && reveal>0.0) NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4((wall_tangent*reveal_side.x+vec3(0.0,reveal_side.y,0.0))*0.6*reveal*effect_weight,0.0)).xyz);
            facade_window = window*effect_weight;
            facade_lit = step(0.35,facade_unit(seed_bits^facade_pcg(facade_cell(floor(cell.x))^facade_pcg(facade_cell(floor(cell.y))))));
            // Open shops light their whole front at night.
            if(shop) facade_lit = 1.0;
            else if(curtain) {
                facade_glow = 0.6;
                facade_lit *= step(0.5,facade_unit(seed_bits^facade_pcg(facade_cell(floor(cell.x))^facade_pcg(facade_cell(floor(cell.y))^2246822519u))));
            }
            float glass_pick = facade_record ? float((record>>6)&3)/4.0+0.1 : fract(facade_seed*11.1);
            vec3 glass = glass_pick<0.4 ? vec3(0.21,0.25,0.29) : (glass_pick<0.7 ? vec3(0.17,0.24,0.25) : vec3(0.25,0.23,0.21));
            // Curtain walls: a reflective coated tint (blue, green, bronze, silver).
            uint glass_bits = facade_pcg(uint(record)^747796405u);
            if(curtain) glass = glass_bits%4u==0u ? vec3(0.16,0.26,0.36) : (glass_bits%4u==1u ? vec3(0.15,0.27,0.25) : (glass_bits%4u==2u ? vec3(0.28,0.22,0.16) : vec3(0.30,0.33,0.36)));
            // Per-pane choices settle once a pane nears a pixel, so far walls do
            // not sparkle.
            float pane_steady = 1.0-smoothstep(0.15,0.4,max(cell_fw.x,cell_fw.y));
            uint pane_bits = seed_bits^facade_pcg(facade_cell(floor(cell.x))^facade_pcg(facade_cell(floor(cell.y))^1540483477u));
            uint room_bits = facade_pcg(pane_bits^2654435761u);
            // Lit rooms (every tier): warm, neutral or cool light (offices cooler),
            // 60–105 % bright per window; shop fronts a warm white.
            bool office = facade_kind==3 || facade_kind==5;
            uint warmth = (room_bits>>4u)%20u;
            facade_room = warmth<(office ? 6u : 12u) ? vec3(1.0,0.63,0.28) : (warmth<(office ? 13u : 18u) ? vec3(0.88,0.70,0.46) : vec3(0.66,0.70,0.72));
            facade_room *= 0.6+0.45*float((room_bits>>9u)&255u)/255.0;
            if(shop) facade_room = vec3(0.55,0.44,0.28);
            // Medium/high, behind punched glass: the room seen along the view ray
            // (its back wall 0.9 storeys deep, framed by ceiling, floor and side
            // walls) and the window dressing at the glass: side curtains (mostly
            // homes), part-drawn slatted blinds (mostly offices) or a sheer. At
            // night they shape and tint the room's light. Both settle with the pane.
            if(detail_quality>0 && !curtain && !shop && window>0.0 && pane_steady>0.0) {
                vec2 glass_span = max(glass_rect.yw-glass_rect.xz,vec2(1e-3));
                vec2 size_m = glass_span*cell_m;
                vec2 gl = clamp((f-glass_rect.xz)/glass_span,0.0,1.0);
                vec2 gl_fw = cell_fw/glass_span;
                vec3 ray = (INV_VIEW_MATRIX*vec4(-VIEW,0.0)).xyz;
                vec2 back = gl+vec2(dot(ray,wall_tangent),ray.y)/max(dot(ray,-n),0.2)*(0.9*floor_pitch)/size_m;
                float edge = 0.05+2.0*max(gl_fw.x,gl_fw.y);
                float ceiling = smoothstep(1.0,1.0+edge,back.y);
                float ground = 1.0-smoothstep(-edge,0.0,back.y);
                float sides = max(smoothstep(1.0,1.0+edge,back.x),1.0-smoothstep(-edge,0.0,back.x));
                uint tone_pick = room_bits%4u;
                vec3 tone = tone_pick==0u ? vec3(0.56,0.51,0.43) : (tone_pick==1u ? vec3(0.60,0.60,0.58) : (tone_pick==2u ? vec3(0.46,0.52,0.58) : vec3(0.60,0.48,0.38)));
                vec3 room = mix(mix(mix(tone,tone*0.8,sides),tone*0.7,ceiling),vec3(0.26,0.19,0.13),ground);
                float room_light = mix(mix(mix(1.0,0.85,sides),1.15,ceiling),0.6,ground);
                uint dressing_pick = (room_bits>>17u)%20u;
                int dressing = facade_kind<=2 ? (dressing_pick<6u ? 0 : (dressing_pick<14u ? 1 : (dressing_pick<16u ? 2 : 3))) : (facade_kind==4 ? (dressing_pick<16u ? 0 : 2) : (dressing_pick<8u ? 0 : (dressing_pick<10u ? 1 : (dressing_pick<19u ? 2 : 3))));
                uint cloth_pick = (room_bits>>22u)%6u;
                vec3 cloth = cloth_pick==0u ? vec3(0.86,0.82,0.72) : (cloth_pick==1u ? vec3(0.72,0.62,0.48) : (cloth_pick==2u ? vec3(0.55,0.66,0.78) : (cloth_pick==3u ? vec3(0.80,0.58,0.58) : (cloth_pick==4u ? vec3(0.56,0.62,0.44) : vec3(0.62,0.62,0.64)))));
                float cover = 0.0;
                vec3 cover_color = cloth;
                vec3 cover_light = vec3(1.0);
                if(dressing==1) {
                    float left = 0.12+0.28*float((room_bits>>25u)&7u)/7.0;
                    float right = 0.12+0.28*float((room_bits>>28u)&7u)/7.0;
                    cover = max(facade_sign_aa(gl.x-left,gl_fw.x),facade_sign_aa(1.0-right-gl.x,gl_fw.x));
                    // Folds every 5 cm, gone before they near two pixels.
                    cover_color = cloth*(0.84+0.16*cos(gl.x*size_m.x*125.66)*(1.0-smoothstep(0.006,0.015,footprint)));
                    cover_light = cloth*0.75;
                } else if(dressing==2) {
                    float drawn = 0.35+0.65*float((room_bits>>25u)&15u)/15.0;
                    cover = facade_sign_aa(1.0-drawn-gl.y,gl_fw.y);
                    float slat = facade_span(gl.y*size_m.y/0.03,0.0,0.75,gl_fw.y*size_m.y/0.03);
                    cover_color = mix(vec3(0.32),vec3(0.72,0.71,0.68),slat);
                    cover_light = vec3(mix(0.75,0.25,slat));
                } else if(dressing==3) {
                    cover = 0.6;
                    cover_color = vec3(0.86,0.85,0.82);
                    cover_light = vec3(0.8);
                }
                vec3 behind = facade_linear(mix(room,cover_color,cover),OUTPUT_IS_SRGB);
                glass = mix(glass,glass*0.6+behind*mix(0.12,0.3,cover),pane_steady);
                facade_pattern = mix(vec3(1.0),mix(vec3(room_light),cover_light,cover),pane_steady);
            }
            // Medium/high: glass is darker (little light enters it) and shows
            // what it reflects instead (below).
            if(detail_quality>0) glass *= mix(1.0,GLASS_BODY,weather.w);
            color = mix(color,glass,facade_window*0.85);
            color *= 1.0-0.3*lintel*effect_weight;
            // Glass: smooth (0.08–0.15 per record) with a stronger specular.
            surface_roughness = mix(surface_roughness,0.08+0.07*float((glass_bits>>8u)&255u)/255.0,facade_window);
            surface_specular = mix(surface_specular,0.55,facade_window);
            surface_metallic = mix(surface_metallic,curtain ? 0.35 : 0.0,facade_window);
            if(detail_quality>0 && facade_window>0.0) {
                // Medium/high: each pane sits slightly out of plane (about ±1.5°),
                // so the sky radiance and the reflection below differ per pane.
                vec2 lean = (vec2(float(pane_bits&255u),float((pane_bits>>8u)&255u))/255.0-0.5)*pane_steady;
                NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4((wall_tangent*lean.x+vec3(0.0,lean.y,0.0))*0.05*facade_window,0.0)).xyz);
                // Daylight: the Fresnel-weighted sky, horizon or street along the
                // reflected ray, added as light (glass is dark; what it shows does
                // not depend on the sun falling on it). Off at night.
                float fresnel = pow(1.0-clamp(dot(NORMAL,VIEW),0.0,1.0),5.0);
                float rise = (INV_VIEW_MATRIX*vec4(reflect(-VIEW,NORMAL),0.0)).y;
                // Each pane also reflects a little more or less (coating, flatness).
                float pane_gain = 1.0+0.6*(float((pane_bits>>16u)&255u)/255.0-0.5)*pane_steady;
                glass_reflection = facade_sky(rise,weather.z)*(GLASS_REFLECTANCE+(1.0-GLASS_REFLECTANCE)*fresnel)*pane_gain*facade_window*weather.w;
            }
            // Window lettering (commercial walls with signs): on the first three
            // upper storeys about 30 % of sign slots carry large glyphs in one ink
            // (white, yellow, red or blue) across the slot's glass, never on its
            // walls; their storey is lit and the letters glow from behind at night.
            if(signs_enabled && facade_kind==3 && !shop && !curtain && window>0.0 && cell.y>=1.0 && cell.y<4.0 && (solid_height<=0.0 || v<solid_height)) {
                uint letter_bits = facade_pcg(sign_bits^facade_pcg(facade_cell(floor(cell.y))^620728307u));
                if(facade_unit(letter_bits)<0.30) {
                    float glyph = 0.42*(glass_rect.w-glass_rect.z)*cell_m.y;
                    int count = clamp(int(floor((slot_width-0.12*floor_pitch-0.8*glyph)/(FACADE_GLYPH_ADVANCE*glyph)))+1,2,6);
                    float run = (float(count-1)*FACADE_GLYPH_ADVANCE+0.8)*glyph;
                    vec2 q = vec2(slot_s-(slot_width-run)*0.5,(glass_rect.z+glass_rect.w)*0.5*cell_m.y+0.45*glyph-f.y*cell_m.y)/glyph;
                    float letters = facade_glyph_run(q,letter_bits,count,0.1,footprint/glyph,false,0,vec2(0.0),vec2(0.0),0.0).x;
                    float lettering = letters*window;
                    uint ink_pick = (letter_bits>>8u)%4u;
                    vec3 ink = facade_linear(ink_pick==0u ? vec3(0.95,0.95,0.92) : (ink_pick==1u ? vec3(0.98,0.82,0.16) : (ink_pick==2u ? vec3(0.86,0.13,0.12) : vec3(0.13,0.36,0.86))),OUTPUT_IS_SRGB);
                    color = mix(color,ink,lettering*effect_weight);
                    glass_reflection *= 1.0-0.8*lettering;
                    // The vinyl blocks the room's light and glows in its own ink.
                    facade_pattern *= 1.0-letters;
                    facade_sign += ink*lettering*0.7;
                    facade_lit = 1.0;
                }
            }
            if(shop_part==1) color *= mix(1.0,0.6,effect_weight);
            else if(shop_part==3) color *= mix(1.0,0.55,effect_weight);
            else if(shop_part==4) color = mix(color,vec3(0.10,0.105,0.11),effect_weight);
            else if(shop_part==6) color *= mix(1.0,0.45,effect_weight);
            float sign_cover = 0.0;
            if(shop_part==7) {
                // Fascia panel per slot: charcoal, off-white or a darker body
                // (channel letters always read on a dark fascia: never the
                // off-white, and the body tone at most as light as charcoal).
                bool has_sign = signs_enabled && facade_unit(sign_bits)<0.85;
                int sign_type = facade_sign_type(sign_bits);
                bool channel = has_sign && sign_type==FACADE_SIGN_CHANNEL;
                uint tone = facade_pcg(slot_bits^1013904223u)%3u;
                if(channel && tone==1u) tone = 0u;
                vec3 body_tone = color*0.7;
                if(channel) body_tone *= min(1.0,0.1/max(facade_sign_luma(body_tone),1e-4));
                color = mix(color,tone==0u ? vec3(0.09,0.10,0.11) : (tone==1u ? vec3(0.72,0.71,0.68) : body_tone),effect_weight);
                // One procedural sign on 85 % of slots (facade_signs.gdshaderinc),
                // centred on the fascia: 0.16 shop storeys high (channel letters
                // 0.18), 80–94 % of the slot wide and at most 8:1; acrylic gloss,
                // a drop shadow on the fascia and (medium/high) a bevelled edge.
                if(has_sign) {
                    float sign_height = (sign_type==FACADE_SIGN_CHANNEL ? 0.18 : 0.16)*storeys.x;
                    float sign_width = min(slot_width*(0.80+0.14*facade_unit(sign_bits^2654435769u)),8.0*sign_height);
                    vec2 local = vec2(slot_s-(slot_width-sign_width)*0.5,0.88*storeys.x+0.5*sign_height-v)/sign_height;
                    FacadeSign face = facade_sign_face(local,sign_bits,sign_width/sign_height,footprint/sign_height,false);
                    vec3 face_color = facade_linear(face.color,OUTPUT_IS_SRGB);
                    color *= 1.0-face.shadow*effect_weight;
                    color = mix(color,face_color,face.body*effect_weight);
                    surface_roughness = mix(surface_roughness,0.3,face.body*effect_weight);
                    surface_specular = mix(surface_specular,0.5,face.body*effect_weight);
                    sign_cover = face.body;
                    facade_sign = face_color*face.glow*face.body*0.55;
                    if(detail_quality>0) NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4((-wall_tangent*face.bevel.x-vec3(0.0,face.bevel.y,0.0))*0.6*effect_weight,0.0)).xyz);
                }
            }
            // Coping and weathering last: they lie over walls, frames and fascia,
            // never over signs or glass; weathering fades with the effect weight.
            color = mix(color,mix(color*1.1,facade_linear(vec3(0.66,0.66,0.64),OUTPUT_IS_SRGB),0.6),cap*(1.0-sign_cover)*effect_weight);
            color *= 1.0-0.3*drip*(1.0-sign_cover)*effect_weight;
            color *= 1.0-clamp(weathering,0.0,0.4)*(1.0-window)*(1.0-sign_cover)*effect_weight;
        }
        color *= facade_record ? 0.96+0.08*solid_jitter : 0.97+0.06*facade_seed;
    }
    if(terrain_surface || (distant && UV.y < -0.5)) {
        float broad=sin(world_position.x*0.055+sin(world_position.z*0.037))*cos(world_position.z*0.063);
        color*=0.96+0.06*broad;
        if(lawn && !distant) {
            // Lush and dry patches (~10 m), clumps (~1.5 m) and blades; the broad
            // patches stay at every distance, finer grain fades with effect weight.
            vec2 g=world_position.xz;
            float patches=lawn_noise(g*0.09)*0.65+lawn_noise(g*0.23+7.3)*0.35;
            color*=mix(vec3(0.86,0.90,0.84),vec3(1.10,1.08,1.02),patches);
            float dry=smoothstep(0.60,0.82,lawn_noise(g*0.05+19.1));
            color=mix(color,color*vec3(1.16,1.08,0.78),dry*0.55);
            float clumps=lawn_noise(g*0.7+3.1);
            float blades=lawn_noise(g*5.3)*0.6+lawn_noise(g*11.0+1.7)*0.4;
            color*=mix(1.0,0.90+0.12*clumps+0.08*blades,effect_weight);
            float clover=smoothstep(0.78,0.92,lawn_noise(g*1.7+11.0));
            color=mix(color,color*vec3(0.82,0.94,0.80),clover*0.5*effect_weight);
            // Shade between the grass field's blades (grass_field.gd): full
            // near the camera, gone where the blades have thinned out, and
            // none on banks too steep for blades (grass_scatter.ground).
            if(grass_field_distance>0.0) color*=1.0-grass_field_shade*smoothstep(0.55,0.65,world_normal.y)*(1.0-smoothstep(grass_field_distance*0.45,grass_field_distance,view_distance));
            // Lawn relief: the fine blade noise gradient tilts the normal a little
            // near the camera and settles flat by 30 m, and wherever a pixel
            // spans a third of the 11/m octave's period or more (grazing views
            // from a car camera), so it never aliases into shimmer.
            vec2 lawn_footprint=(abs(position_dx.xz)+abs(position_dy.xz))*11.0;
            float lawn_relief=0.025*(1.0-smoothstep(8.0,30.0,view_distance))*(1.0-smoothstep(0.3,0.6,max(lawn_footprint.x,lawn_footprint.y)))*effect_weight;
            if(detail_quality>0 && lawn_relief>0.0) {
                vec2 gx=g+vec2(0.04,0.0), gz=g+vec2(0.0,0.04);
                float hx=lawn_noise(gx*5.3)*0.6+lawn_noise(gx*11.0+1.7)*0.4;
                float hz=lawn_noise(gz*5.3)*0.6+lawn_noise(gz*11.0+1.7)*0.4;
                vec2 slope=vec2(hx-blades,hz-blades)/0.04;
                NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(-slope.x*lawn_relief,0.0,-slope.y*lawn_relief,0.0)).xyz);
                color*=1.0-0.05*(1.0-blades)*lawn_relief/0.025;
            }
        }
        float rock=1.0-smoothstep(0.70,0.94,world_normal.y);
        color=mix(color,vec3(0.40,0.37,0.30),rock*0.55);
        float shore=1.0-smoothstep(0.15,0.85,world_position.y);
        color=mix(color,vec3(0.42,0.39,0.29),shore*0.6);
    }
    // Wet bank: earth under and just above the water line darkens and turns
    // glossy (near ground only; dry earth keeps its look).
    float wet_bank = 0.0;
    if(terrain_surface && !distant && shore_level_m > -10000.0) {
        wet_bank = 1.0-smoothstep(0.03,0.16,wet_height);
        color = mix(color,color*vec3(0.56,0.53,0.50),wet_bank);
    }
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    float snow_cover = weather.y * facing_up;
    color *= 1.0 - weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),snow_cover*0.92);

    ALBEDO = color;
    ROUGHNESS = mix(surface_roughness,0.28,weather.x*facing_up);
    ROUGHNESS = mix(ROUGHNESS,0.92,snow_cover);
    ROUGHNESS = mix(ROUGHNESS,0.34,wet_bank*(1.0-snow_cover));
    float puddle = 0.0;
    if(!distant && effect_weight>0.0 && weather.x>0.2) puddle=puddle_mask(world_position,world_normal.y,weather.x,weather.y)*effect_weight;
    ALBEDO *= 1.0-puddle*0.10;
    ROUGHNESS = mix(ROUGHNESS,0.22,puddle);
    SPECULAR = mix(surface_specular,0.50,puddle);
    if(puddle>0.0) NORMAL = normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
    METALLIC = surface_metallic;
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
        vec3 warm = vec3(1.0,0.63,0.28);
        if(role == 3) windows = world_normal.y > 0.5 ? 0.0 : 0.15;
        else if(facade_kind>0) windows = facade_wall*mix(0.15,facade_window*facade_lit*facade_glow,effect_weight);
        else if(effect_weight>0.0) windows=mix(0.8,step(0.2,fract(floor(world_position.x*0.7)+floor(world_position.y*0.6)*0.618+seed)),effect_weight);
        vec3 lit_rooms = warm*windows;
        // Near facade windows take each room's light; far ones the warm average.
        if(role == 1 && facade_kind>0) lit_rooms = facade_wall*mix(warm*0.15,facade_room*facade_pattern*(facade_window*facade_lit*facade_glow),effect_weight);
        EMISSION = enabled ? lit_rooms*1.2 : vec3(0.0);
        // Storefront signs and window lettering stay lit through the night.
        if(clock.z < 0.0) EMISSION += facade_sign*effect_weight;
    } else if(role == 2 && enabled) {
        // Authored bulbs/signs retain their material colour in both LODs.
        EMISSION = enabled ? color*0.9 : vec3(0.0);
    }
    EMISSION += glass_reflection;
}
