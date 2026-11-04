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
// Highest water surface of the cell in mesh space (set per earth mesh by the
// chunk renderer); below about 15 cm above it the earth reads wet.
instance uniform float shore_level_m = -100000.0;
// Grass ground: broad lush/dry patches, mown clumps and fine blades in world space.
uniform bool lawn = false;
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
// Shared original storefront signs: 16x32 grid, 8 category rows of 64. Each
// 64x20 tile holds its sign in the central 56x16 with edge-repeat gutters.
uniform sampler2D sign_atlas : source_color, filter_linear_mipmap_anisotropic, repeat_disable;
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
// Packed commercial solids also carry 1000 + their top (eaves) height in
// CUSTOM0.x; other meshes read 0 (height -1) and keep floor-pitch storeys.
varying flat float solid_height;
// The record's wall finish slot (-1 none), picked once per solid in vertex().
varying flat int finish_slot;
float facade_hash(vec2 v) { return fract(sin(dot(v,vec2(12.9898,78.233)))*43758.5453); }
// Integer hash (PCG) for discrete facade choices: exact for the 24-bit packed
// seed and integer cells, unlike the sine hash above (continuous noise only).
uint facade_pcg(uint v) { uint s=v*747796405u+2891336453u; uint w=((s>>((s>>28u)+4u))^s)*277803737u; return (w>>22u)^w; }
uint facade_cell(float v) { return uint(int(v)+1048576); }
float facade_unit(uint v) { return float(facade_pcg(v)>>8u)/16777216.0; }
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
    solid_height = solid_seed>=0.0 && CUSTOM0.x>500.0 ? (MODEL_MATRIX * vec4(0.0,CUSTOM0.x-1000.0,0.0,1.0)).y-solid_base : -1.0;
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
    // Screen derivatives in uniform control flow, for the facade sign lookup.
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
    vec3 facade_sign = vec3(0.0);
    float solid_jitter = facade_record ? fract(solid_seed*4096.0) : seed;
    vec3 facade_base = max(base_color.rgb,vec3(0.02));
    if(facade_record && world_normal.y>=0.5) color *= facade_linear(facade_roof[(record>>3)&7],OUTPUT_IS_SRGB)/facade_base;
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
            // Apartment complexes: a coloured column every 5–8 bays and a
            // darker accent base storey give each complex its own identity.
            if(facade_record && facade_kind==1) {
                vec3 accent = facade_linear(facade_accent[(record>>6)&7],OUTPUT_IS_SRGB);
                float column = step(mod(floor(cell.x),float(5+((record>>9)&3))),0.5);
                float base_storey = step(v,floor_pitch*0.92);
                color = mix(color,accent,max(column*(1.0-window),base_storey*0.55)*effect_weight);
                window *= 1.0-base_storey;
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
            int shop_part = -1;
            if(shop) {
                uint slot_layout = facade_pcg(slot_bits^2654435769u);
                shop_part = facade_storefront(slot_s,cell.y,storeys.x,slot_width,slot_width*(0.22+0.28*float(slot_layout%3u)),2.0+float((slot_layout>>2u)&1u),floor_pitch);
                window = (shop_part==2 || shop_part==3 || shop_part==5) ? 1.0 : 0.0;
                spandrel = 0.0;
            } else if(facade_kind==3 && storeys.y<=0.0 && v<solid_height) { window = 0.0; spandrel = 0.0; }
            // The finish sets the wall's roughness, metallic and specular (walls,
            // piers and stall risers; frames, canopy, fascia and signs keep the
            // plain response); far walls fade back to the plain distant response.
            if(finish>=0 && shop_part<=1) {
                surface_roughness = mix(surface_roughness,facade_finish[finish].z,effect_weight);
                surface_metallic = mix(surface_metallic,facade_finish[finish].w,effect_weight);
                surface_specular = mix(surface_specular,facade_finish_look[finish].x,effect_weight);
            }
            if(facade_kind==4) {
                color *= 1.0-0.08*step(0.85,fract(u/(0.6*floor_pitch)));
                window *= step(0.5,facade_hash(vec2(floor(cell.x),floor(cell.y)*3.0)));
            }
            if(facade_kind==1) color *= 1.0-0.14*step(0.94,f.y)*effect_weight;
            if(facade_kind==1 && fract(facade_seed*5.3)<0.5) color *= 1.0+0.08*step(f.y,0.1)*step(1.0,cell.y)*effect_weight;
            if(facade_kind==5) color *= 1.0+0.05*step(f.y,0.08)*effect_weight;
            // Recessed openings: a reveal band of wall inside each pane edge,
            // shaded and (medium/high) tilted towards the opening; the band
            // fades out before it shrinks below about two pixels. Edges on the
            // cell boundary (ribbon windows) are continuous glass, not reveals.
            float reveal = 0.0;
            vec2 reveal_side = vec2(0.0);
            if(!shop && window>0.0) {
                vec2 e0 = (f-pane.xz)*cell_m;
                vec2 e1 = (pane.yw-f)*cell_m;
                if(pane.x<=0.0) e0.x = 1e3;
                if(pane.y>=1.0) e1.x = 1e3;
                if(pane.z<=0.0) e0.y = 1e3;
                if(pane.w>=1.0) e1.y = 1e3;
                float band = (curtain ? 0.015 : 0.06)*floor_pitch;
                float footprint = max(length(wall_dx),length(wall_dy));
                float across = min(e0.x,e1.x);
                float upright = min(e0.y,e1.y);
                float inset = min(across,upright);
                reveal = step(0.0,inset)*step(inset,band)*(1.0-smoothstep(0.2*band,0.4*band,footprint));
                reveal_side = across<upright ? vec2(e0.x<e1.x ? 1.0 : -1.0,0.0) : vec2(0.0,e0.y<e1.y ? 1.0 : -1.0);
                window *= 1.0-reveal;
                color *= 1.0-0.18*reveal*effect_weight;
            }
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
            float near_relief = material_detail_weight(view_distance)*(1.0-window);
            float tone_relief = (detail_quality>0 ? effect_weight : material_detail_weight(view_distance))*(1.0-window);
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
            // Medium/high: glass is darker (little light enters it) and shows
            // what it reflects instead (below).
            if(detail_quality>0) glass *= mix(1.0,GLASS_BODY,weather.w);
            color = mix(color,glass,facade_window*0.85);
            // Glass: smooth (0.08–0.15 per record) with a stronger specular.
            surface_roughness = mix(surface_roughness,0.08+0.07*float((glass_bits>>8u)&255u)/255.0,facade_window);
            surface_specular = mix(surface_specular,0.55,facade_window);
            surface_metallic = mix(surface_metallic,curtain ? 0.35 : 0.0,facade_window);
            if(detail_quality>0 && facade_window>0.0) {
                // Medium/high: each pane sits slightly out of plane (about ±1.5°),
                // so the sky radiance and the reflection below differ per pane.
                // Per-pane differences settle once a pane nears a pixel, so far
                // walls do not sparkle.
                float pane_steady = 1.0-smoothstep(0.15,0.4,max(cell_fw.x,cell_fw.y));
                uint pane_bits = seed_bits^facade_pcg(facade_cell(floor(cell.x))^facade_pcg(facade_cell(floor(cell.y))^1540483477u));
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
            if(shop_part==1) color *= mix(1.0,0.6,effect_weight);
            else if(shop_part==3) color *= mix(1.0,0.55,effect_weight);
            else if(shop_part==4) color = mix(color,vec3(0.10,0.105,0.11),effect_weight);
            else if(shop_part==6) color *= mix(1.0,0.45,effect_weight);
            else if(shop_part==7) {
                // Plain fascia panel per slot: charcoal, off-white or a darker body.
                uint tone = facade_pcg(slot_bits^1013904223u)%3u;
                color = mix(color,tone==0u ? vec3(0.09,0.10,0.11) : (tone==1u ? vec3(0.72,0.71,0.68) : color*0.7),effect_weight);
            }
            // One storefront sign per slot fills the fascia height at the atlas
            // content aspect (56:16), centred in the slot.
            if(signs_enabled && shop_part==7) {
                float pick = facade_unit(slot_bits);
                if(pick<0.85) {
                    float index = min(floor(pick/0.85*512.0),511.0);
                    float sign_height = 0.20*storeys.x;
                    float sign_width = min(slot_width*0.88,sign_height*3.5);
                    vec2 local = vec2((slot_s-(slot_width-sign_width)*0.5)/sign_width,1.0-(v-0.78*storeys.x)/sign_height);
                    if(local.x>0.0 && local.x<1.0 && local.y>0.0 && local.y<1.0) {
                        // Content is the central 56x16 of a 64x20 tile.
                        const vec2 grid = vec2(16.0,32.0);
                        const vec2 content = vec2(56.0/64.0,16.0/20.0);
                        vec2 tile = vec2(mod(index,16.0),floor(index/16.0));
                        vec2 atlas = (tile+(1.0-content)*0.5+clamp(local,vec2(0.0),vec2(1.0))*content)/grid;
                        // Analytic gradients (the slot fract and branches would break
                        // implicit mip selection); far signs settle on one blurred
                        // level instead of crawling.
                        vec3 tangent = normalize(vec3(-n.z,0.0,n.x));
                        vec2 scale = content/grid*vec2(-1.0/sign_width,-1.0/sign_height);
                        vec2 gx = vec2(dot(position_dx,tangent),position_dx.y)*scale;
                        vec2 gy = vec2(dot(position_dy,tangent),position_dy.y)*scale;
                        vec2 texels = vec2(textureSize(sign_atlas,0));
                        float lod = log2(max(max(length(gx*texels),length(gy*texels)),1e-6));
                        vec3 near_sign = textureGrad(sign_atlas,atlas,gx,gy).rgb;
                        // The level where a tile is 16x5 texels (2 at full size, 1 on
                        // the halved low atlas) keeps whole texels inside each tile.
                        vec3 far_sign = textureLod(sign_atlas,(tile+0.5)/grid,max(log2(texels.x/256.0),0.0)).rgb;
                        facade_sign = mix(near_sign,far_sign,smoothstep(2.0,3.5,lod));
                        color = mix(color,facade_sign,effect_weight);
                        surface_roughness = mix(surface_roughness,0.4,effect_weight);
                        facade_window = 0.0;
                    }
                }
            }
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
        if(role == 3) windows = world_normal.y > 0.5 ? 0.0 : 0.15;
        else if(facade_kind>0) windows = facade_wall*mix(0.15,facade_window*facade_lit*facade_glow,effect_weight);
        else if(effect_weight>0.0) windows=mix(0.8,step(0.2,fract(floor(world_position.x*0.7)+floor(world_position.y*0.6)*0.618+seed)),effect_weight);
        EMISSION = enabled ? vec3(1.0,0.63,0.28)*windows*1.2 : vec3(0.0);
        // Storefront signs stay lit through the night.
        if(clock.z < 0.0) EMISSION += facade_sign*0.55*effect_weight;
    } else if(role == 2 && enabled) {
        // Authored bulbs/signs retain their material colour in both LODs.
        EMISSION = enabled ? color*0.9 : vec3(0.0);
    }
    EMISSION += glass_reflection;
}
