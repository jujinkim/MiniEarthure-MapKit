shader_type spatial;
render_mode cull_back;
// Display-only city barrier detail (barrier_detail_meshes.gd, native
// barrier_details.rs): fence rails, tubes, boards, wires and posts, wall
// bodies, plinths, copings, giwa caps and piers, hedge bodies, and the far
// meshes, in one material. barrier_detail_meshes.gd prefixes BARRIER_INFILL
// (the near see-through infill: dithered alpha test) or BARRIER_FAR (the
// far fence: alpha blended, never discarded, so tile-based GPUs keep early
// depth on every far mesh); the plain material is opaque.
//
// Mesh coordinates: x runs 0..1 along a run (posts: real metres along),
// y 0..1 from the centreline ground to the top, z -0.5..0.5 across the
// prism; UV2 adds real-metre offsets to y and z, which the vertex shader
// converts with the instance's height, perpendicular width and the map scale
// 1:D, so rails, tubes and posts keep their real sizes on every height and
// thickness while bodies fill the prism. Normals are authored in that real-
// metre frame and corrected for the instance's non-uniform, sheared basis
// (MultiMesh normals take mat3 of the instance). UV.x is the surface role,
// UV.y a baked shade plus 2 x the part's variant (0 every design; others
// collapse to a point unless barrier_keep matches the design bits).
// INSTANCE_CUSTOM: design bits (two 8-bit channels: kind, family, palette;
// top, rails, D), then on runs the phase at the run's start (fraction of a
// bay) and its end flags, on posts the post kind and its size factor. Every
// choice is an integer hash of the bits and the bay index; fine patterns are
// box-filtered over the pixel footprint (derivatives in uniform control
// flow) and settle on their area shares.
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;
// Set by display_quality.apply_material (the material carries mapkit_detail).
uniform int detail_quality = 1;
uniform float surface_effect_distance = 96.0;
// barrier_palette.json (sRGB): fence families x 4 slots, wall families x 4,
// hedge tones; post or pier lattice per family; far opacities.
uniform vec3 barrier_palette[40];
uniform vec3 regional_palette[16];
uniform float fence_bay[5];
uniform float wall_bay[4];
uniform float fence_far_opacity[5];
uniform float board_far_opacity = 0.8;
varying flat vec4 design;
// Real metres: along (bay frame on runs, post-local on posts), height above
// the centreline ground, across from the centreline; world height (courses
// stay level on slopes).
varying vec3 frame_m;
varying float world_y_m;
// Run length, height and perpendicular width (real metres), map scale D.
varying flat vec4 dims;
varying vec3 world_normal;
// The authored (real-frame) normal: end faces run their patterns across.
varying vec3 local_normal;

const int ROLE_RAIL = 0;
const int ROLE_TUBE = 1;
const int ROLE_WIRE = 2;
const int ROLE_BOARD = 3;
const int ROLE_INFILL = 4;
const int ROLE_BODY = 5;
const int ROLE_PLINTH = 6;
const int ROLE_COPING = 7;
const int ROLE_GIWA = 8;
const int ROLE_HEDGE = 9;
const int ROLE_POST = 10;
const int ROLE_CAP = 11;
const int ROLE_PIER = 12;
const int ROLE_FAR_FENCE = 13;
const int ROLE_FAR_SOLID = 14;
const int ROLE_RIDGE = 15;

uint barrier_bits(vec4 c) { return uint(round(c.x))|(uint(round(c.y))<<8u); }
float barrier_scale(uint bits) { return float(max((bits>>12u)&15u,1u)); }
// Mirrors barrier_detail_meshes.keep().
bool barrier_keep(int v, uint bits, int flags, float height_m) {
    uint kind = bits&3u;
    uint family = (bits>>2u)&7u;
    uint top = (bits>>8u)&3u;
    uint rails = (bits>>10u)&3u;
    bool fence = kind==1u;
    bool wall = kind==2u;
    bool giwa = wall && family==3u && top==1u;
    if(v==0) return true;
    if(v>=1 && v<=5) return fence && family==uint(v-1);
    if(v>=6 && v<=9) return wall && family==uint(v-6);
    if(v==10) return giwa;
    if(v==11) return wall && !giwa;
    if(v==12) return kind==0u;
    if(v==13) return kind==0u && top==0u;
    if(v==14) return kind==0u && top==1u;
    if(v==15) return fence && family==3u && rails==0u;
    if(v==16) return fence && family==3u && rails==1u;
    if(v==17) return fence && (family<3u || (family==3u && rails==1u));
    if(v==18) return fence && family==0u && height_m>=1.6;
    if(v==19) return wall;
    if(v>=30 && v<=32) return wall && family==uint(v-30) && flags==0;
    if(v==34) return wall && flags!=0 && !giwa;
    if(v==35) return wall && flags!=0 && giwa;
    return false;
}
float barrier_bay(uint bits) {
    uint kind = bits&3u;
    uint family = min((bits>>2u)&7u,4u);
    if(kind==1u) return fence_bay[int(family)];
    if(kind==2u) return wall_bay[int(min(family,3u))];
    return 1.0;
}
vec3 barrier_linear(vec3 c, bool srgb_output) { return srgb_output ? c : mix(c/12.92,pow((c+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),c)); }
uint barrier_pcg(uint v) {
    uint state = v*747796405u+2891336453u;
    uint word = ((state>>((state>>28u)+4u))^state)*277803737u;
    return (word>>22u)^word;
}
float barrier_unit(uint v) { return float(barrier_pcg(v)&65535u)/65535.0; }
// Box-filtered share of [a,b] of each unit cell over a footprint w centred on x.
float barrier_span_integral(float t, float a, float b) { return floor(t)*(b-a)+clamp(fract(t),a,b)-a; }
float barrier_span(float x, float a, float b, float w) {
    w = max(w,1e-4);
    float t = fract(x);
    return clamp((barrier_span_integral(t+0.5*w,a,b)-barrier_span_integral(t-0.5*w,a,b))/w,0.0,1.0);
}
// Box-filtered coverage of a centred line of `width` (pattern units) every unit.
float barrier_lines(float x, float width, float w) { return barrier_span(x+0.5*width,0.0,width,w); }
// A wire or bar at least `boost` of a pixel wide (near infill: the alpha
// test keeps whole lines until the pattern itself goes unresolved; far: 0).
float barrier_wire(float x, float width, float w, float boost) { return barrier_lines(x,max(width,boost*w),w); }
float barrier_band(float x, float a, float b, float w) {
    w = max(w,1e-4);
    return clamp((min(x+0.5*w,b)-max(x-0.5*w,a))/w,0.0,1.0);
}
float barrier_hash3(vec3 p) { return fract(sin(dot(p,vec3(12.9898,78.233,37.719)))*43758.5453); }
// Value noise on world positions (seamless across pieces in any direction).
float barrier_noise3(vec3 p) {
    vec3 i = floor(p); vec3 f = fract(p); f = f*f*(3.0-2.0*f);
    float a = mix(mix(barrier_hash3(i),barrier_hash3(i+vec3(1,0,0)),f.x),mix(barrier_hash3(i+vec3(0,1,0)),barrier_hash3(i+vec3(1,1,0)),f.x),f.y);
    float b = mix(mix(barrier_hash3(i+vec3(0,0,1)),barrier_hash3(i+vec3(1,0,1)),f.x),mix(barrier_hash3(i+vec3(0,1,1)),barrier_hash3(i+vec3(1,1,1)),f.x),f.y);
    return mix(a,b,f.z);
}
float barrier_bayer2(vec2 a) { a = floor(a); return fract(dot(a,vec2(0.5,a.y*0.75))); }
float barrier_bayer4(vec2 a) { return barrier_bayer2(0.5*a)*0.25+barrier_bayer2(a)+1.0/32.0; }
vec3 barrier_paint(uint bits) {
    uint regional=(bits>>16u)&15u;
    if(regional>0u) return regional_palette[int(regional-1u)];
    uint kind = bits&3u;
    uint family = (bits>>2u)&7u;
    uint slot = (bits>>5u)&7u;
    int base = kind==1u ? int(min(family,4u))*4 : (kind==2u ? 20+int(min(family,3u))*4 : 36);
    return barrier_palette[base+int(min(slot,3u))];
}
// Fence infill coverage (0..1) at (u along, v up; real metres) of family f,
// footprint fw (metres); `region` is 1 inside the infill's frame.
float barrier_infill(uint bits, float u, float v, float h, float fw, float bay, float boost, out float region) {
    uint family = (bits>>2u)&7u;
    uint top = (bits>>8u)&3u;
    region = 0.0;
    float coverage = 0.0;
    if(family==0u) {
        // Steel bar panels: 16 mm flat bars every 120 mm between the rails;
        // spear tips above the top rail, or a low arch rail under it.
        float upper = top==1u ? h-0.005 : h-0.12;
        region = barrier_band(v,0.10,upper,fw);
        float pitch = bay/16.0;
        float width = 0.016;
        if(top==1u && v>h-0.08) width = 0.016*clamp((h-0.005-v)/0.075,0.0,1.0);
        coverage = barrier_wire(u/pitch,width/pitch,fw/pitch,boost)*region;
        if(top==2u) {
            float arch = h-0.13-0.10*sin(3.14159*fract(u/bay));
            coverage = max(coverage*step(v,arch+0.012),barrier_band(v,arch-0.012,arch+0.012,fw)*region);
        }
    } else if(family==1u) {
        // Welded mesh: 5 mm wires, verticals at 50 mm and horizontals at
        // 200 mm, with two V-fold bands of doubled wires.
        region = barrier_band(v,0.08,h-0.045,fw);
        float pitch = bay/50.0;
        float vertical = barrier_wire(u/pitch,0.005/pitch,fw/pitch,boost);
        float horizontal = barrier_wire(v/0.2,0.025,fw/0.2,boost);
        float folds = 0.0;
        for(int k=1;k<=2;k++) {
            float at = h*float(k)/3.0;
            folds = max(folds,max(barrier_wire((v-at+0.0095)/0.2,0.025,fw/0.2,boost),barrier_wire((v-at-0.0095)/0.2,0.025,fw/0.2,boost)));
        }
        coverage = max(max(vertical,horizontal),folds)*region;
    } else if(family==2u) {
        // Chain-link: a 50 mm diamond lattice of 3 mm wire.
        region = barrier_band(v,0.05,h-0.035,fw);
        float pitch = bay/42.0;
        float a = (u+v)/pitch;
        float b = (u-v)/pitch;
        float wf = fw*1.4142/pitch;
        coverage = max(barrier_wire(a,0.003/pitch,wf,boost),barrier_wire(b,0.003/pitch,wf,boost))*region;
    } else if(family==3u && ((bits>>10u)&3u)==1u) {
        // Wooden pickets: 90 mm pickets with 60 mm gaps, pointed tops.
        region = barrier_band(v,0.08,h-0.02,fw);
        float pitch = bay/12.0;
        float x = fract(u/pitch+0.5)*pitch-0.5*pitch;
        float tip = h-0.02-0.06*abs(x)/0.045;
        coverage = barrier_lines(u/pitch,0.6,fw/pitch)*barrier_band(v,0.08,tip,fw);
    } else if(family==3u) {
        // Wooden boards (geometry near; their faces far): three 0.12 m
        // boards and a cap rail.
        region = barrier_band(v,0.0,h,fw);
        for(int k=0;k<3;k++) {
            float at = 0.2+0.3*float(k);
            coverage = max(coverage,barrier_band(v,at*h-0.06,at*h+0.06,fw));
        }
        coverage = max(coverage,barrier_band(v,h-0.03,h,fw));
    }
    return coverage;
}
// Far fence frame (posts every bay and rails) coverage: the painted parts
// the far mesh draws in place of the near geometry.
float barrier_far_frame(uint bits, float u, float v, float h, float fw, float bay) {
    uint family = (bits>>2u)&7u;
    float post = barrier_lines(u/bay,(family==3u ? 0.09 : 0.06)/bay,fw/bay)*barrier_band(v,0.0,h,fw);
    float rails = 0.0;
    if(family==0u) rails = max(barrier_band(v,h-0.12,h-0.08,fw),barrier_band(v,0.06,0.10,fw));
    else if(family==1u) rails = max(barrier_band(v,h-0.045,h-0.005,fw),barrier_band(v,0.04,0.08,fw));
    else if(family==2u) rails = barrier_band(v,h-0.056,h-0.014,fw);
    else if(family==3u) rails = (((bits>>10u)&3u)==0u) ? 0.0 : max(barrier_band(v,h*0.22-0.035,h*0.22+0.035,fw),barrier_band(v,h*0.78-0.035,h*0.78+0.035,fw));
    else rails = max(barrier_band(v,h-0.054,h-0.006,fw),barrier_band(v,h*0.5-0.024,h*0.5+0.024,fw));
    return max(post,rails);
}

void vertex() {
    design = INSTANCE_CUSTOM;
    uint bits = barrier_bits(INSTANCE_CUSTOM);
    int role = int(round(UV.x));
    int variant = int(floor(UV.y*0.5));
    vec3 bx = MODEL_MATRIX[0].xyz;
    vec3 by = MODEL_MATRIX[1].xyz;
    vec3 bz = MODEL_MATRIX[2].xyz;
    float d = barrier_scale(bits);
    float h = max(length(by),1e-4);
    float lh = max(length(bx.xz),1e-5);
    vec2 xh = bx.xz/lh;
    float w = max(abs(xh.x*bz.z-xh.y*bz.x),1e-4);
    bool post = role==ROLE_POST || role==ROLE_CAP || role==ROLE_PIER;
    int flags = int(round(post ? INSTANCE_CUSTOM.z : INSTANCE_CUSTOM.w));
    float size = post ? INSTANCE_CUSTOM.w : 1.0;
    if(!barrier_keep(variant,bits,flags,h*d)) VERTEX = vec3(0.0);
    else {
        VERTEX.y = min(VERTEX.y+UV2.x/(d*h),1.0);
        VERTEX.z = clamp(VERTEX.z+UV2.y*size/(d*w),-0.499,0.499);
    }
    local_normal = NORMAL;
    // The real-metre frame's normals through the instance's sheared,
    // non-uniform basis (inverse transpose; MultiMesh applies mat3).
    mat3 m = mat3(MODEL_MATRIX);
    NORMAL = inverse(transpose(m)*m)*(NORMAL*vec3(lh*d,h*d,w*d));
    float bay = barrier_bay(bits);
    frame_m = vec3(post ? VERTEX.x*size : INSTANCE_CUSTOM.z*bay+VERTEX.x*lh*d, VERTEX.y*h*d, VERTEX.z*w*d);
    world_y_m = (MODEL_MATRIX*vec4(VERTEX,1.0)).y*d;
    dims = vec4(post ? 0.0 : lh*d, h*d, w*d, d);
    world_normal = normalize(m*NORMAL);
}

void fragment() {
    vec4 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0) : vec4(0.0,0.0,0.18,1.0);
    // Footprints (real metres) from derivatives in uniform control flow.
    vec2 uv_m = vec2(frame_m.x,frame_m.y);
    float fw = max(length(dFdx(uv_m)),length(dFdy(uv_m)));
    float fy = max(abs(dFdx(world_y_m)),abs(dFdy(world_y_m)));
    float view_distance = length(VERTEX);
    // World position in real metres: continuous noise across pieces.
    vec3 world = (INV_VIEW_MATRIX*vec4(VERTEX,1.0)).xyz*dims.w;
    float effect = 1.0-smoothstep(surface_effect_distance*0.8,surface_effect_distance,view_distance);
    int role = int(round(UV.x));
    float shade = UV.y-2.0*floor(UV.y*0.5);
    uint bits = barrier_bits(design);
    uint kind = bits&3u;
    uint family = (bits>>2u)&7u;
    uint top = (bits>>8u)&3u;
    float bay = barrier_bay(bits);
    float h = dims.y;
    // Faces looking along the barrier (ends, pier sides) run their
    // patterns across it instead.
    float u = abs(local_normal.x) > 0.5 ? frame_m.z+0.37 : frame_m.x;
    float v = frame_m.y;
    vec3 paint = barrier_paint(bits);
    // Per-barrier tone. Bay frames agree across pieces only modulo a bay,
    // so patterns repeat whole per bay and noise follows world positions.
    float tint = barrier_unit(bits*2654435761u+uint(round(design.x*7.0+design.y)));
    float dither_mix = 0.0;
    vec3 color = paint;
    float rough = 0.5;
    float metal = 0.0;
    float spec = 0.4;
    float alpha = 1.0;
    float grime = 1.0-0.18*(1.0-smoothstep(0.0,0.25,v))*effect;
    if(role==ROLE_RAIL || role==ROLE_TUBE || role==ROLE_WIRE || role==ROLE_POST || role==ROLE_CAP) {
        if(kind==1u && family==3u) {
            // Wooden posts and rails: grain along the part, weathered ends.
            float grain = barrier_noise3(world*vec3(3.0,60.0,3.0));
            color = paint*(0.82+0.3*grain)*grime;
            rough = 0.85;
            spec = 0.3;
        } else {
            // Powder-coated or galvanised steel, rust and dirt near the ground.
            bool galvanised = kind==1u && family==2u && ((bits>>5u)&7u)==0u;
            color = paint*grime;
            rough = galvanised ? 0.45 : 0.55;
            metal = galvanised ? 0.6 : 0.25;
            spec = 0.5;
            if(role==ROLE_CAP) color *= 0.92;
        }
    } else if(role==ROLE_BOARD) {
        // Horizontal boards: grain, a darker edge line and per-board tone.
        float grain = barrier_noise3(world*vec3(2.5,90.0,2.5));
        float board = barrier_unit((bits*747796405u)^uint(floor(v/0.3)+11.0));
        color = paint*(0.8+0.25*grain)*(0.92+0.16*board)*grime;
        rough = 0.85;
        spec = 0.3;
    } else if(role==ROLE_INFILL || role==ROLE_FAR_FENCE) {
        float region;
        float near_infill = role==ROLE_INFILL ? 1.0 : 0.0;
        float coverage = barrier_infill(bits,u,v,h,fw,bay,0.8*near_infill,region);
        float opacity = kind==1u && family==3u && ((bits>>10u)&3u)==0u ? board_far_opacity : fence_far_opacity[int(min(family,4u))];
        // As the pattern goes unresolved (fewer than six pixels a mesh) it
        // settles to the fence's far opacity, the share the far mesh and the
        // distant prism draw (near: an ordered dither).
        float pitch = bay/(family==1u ? 50.0 : (family==2u ? 42.0 : (family==3u ? 12.0 : 16.0)));
        float resolved = smoothstep(2.5,6.0,pitch/max(fw,1e-5));
        dither_mix = 1.0-resolved;
        coverage = mix(opacity*region,coverage,resolved);
        bool wooden = family==3u;
        color = wooden ? paint*(0.85+0.2*barrier_noise3(world*vec3(20.0,4.0,20.0))) : paint*0.95;
        rough = wooden ? 0.85 : 0.5;
        metal = wooden ? 0.0 : 0.3;
        if(role==ROLE_FAR_FENCE) {
            float frame = barrier_far_frame(bits,u,v,h,fw,bay);
            alpha = clamp(max(frame,family==4u ? 0.0 : coverage),0.0,1.0);
            color = paint*grime;
        } else alpha = coverage;
    } else if(role==ROLE_BODY || role==ROLE_PIER || role==ROLE_FAR_SOLID || role==ROLE_PLINTH || role==ROLE_COPING) {
        if(kind==0u) {
            // Far hedge.
            float leaves = barrier_noise3(world*9.0)*0.6+barrier_noise3(world*23.0)*0.4;
            color = paint*(0.72+0.45*leaves)*(0.8+0.2*smoothstep(0.0,0.5,v));
            rough = 0.85;
            spec = 0.25;
        } else {
            // Wall courses on world height (level on slopes), joints along
            // the bay frame; per-unit tones hash the unit index modulo the bay.
            float wy = world_y_m;
            vec3 mortar = vec3(0.63,0.62,0.59);
            uint units = uint(round(bay/(family==0u ? 0.2 : (family==1u ? 0.4 : 1.0))));
            if(family==0u) {
                float course = floor(wy/0.067);
                float shift = mod(course,2.0)*0.5;
                float along = u/0.2+shift;
                float joint = max(barrier_lines(wy/0.067,0.15,fy/0.067),barrier_lines(along,0.05,fw/0.2));
                float brick = barrier_unit((uint(mod(floor(along),float(max(units,1u))))*31u+uint(mod(course,64.0)))^(bits*97u));
                color = mix(paint*(0.86+0.24*brick),mortar,joint*0.85);
                rough = 0.85;
            } else if(family==1u) {
                float course = floor(wy/0.2);
                float along = u/0.4+mod(course,2.0)*0.5;
                float joint = max(barrier_lines(wy/0.2,0.05,fy/0.2),barrier_lines(along,0.025,fw/0.4));
                float block = barrier_unit((uint(mod(floor(along),float(max(units,1u))))*13u+uint(mod(course,64.0)))^(bits*31u));
                color = paint*(0.95+0.07*block)*(1.0-0.22*joint);
                rough = 0.9;
            } else if(family==2u) {
                // Precast panels stacked 0.5 m high between H-posts, with
                // formwork mottling.
                float joint = barrier_lines(v/0.5,0.02,fw/0.5);
                float mottle = barrier_noise3(world*6.0)*0.6+barrier_noise3(world*21.0)*0.4;
                color = paint*(0.9+0.16*mottle)*(1.0-0.3*joint);
                rough = 0.9;
            } else {
                // Ashlar: 0.3 m courses of 0.4-1.0 m stones with dark joints.
                float course = floor(wy/0.3);
                float row = barrier_unit(uint(mod(course,64.0))^(bits*13u));
                float along = u/0.75+row;
                float joint = max(barrier_lines(wy/0.3,0.04,fy/0.3),barrier_lines(along,0.02,fw/0.75));
                float stone = barrier_unit((uint(mod(floor(along),4.0))*7u+uint(mod(course,64.0)))^(bits*53u));
                float grain = barrier_noise3(world*14.0);
                color = mix(paint*(0.9+0.14*stone)*(0.93+0.1*grain),vec3(0.42,0.41,0.39),joint*0.75);
                rough = 0.88;
            }
            color *= grime;
            if(role==ROLE_PLINTH) {
                // A darker base band, damp near the ground.
                color = mix(vec3(0.52,0.51,0.48),color,0.35)*(0.82+0.18*smoothstep(0.0,0.15,v));
                rough = 0.92;
            } else if(role==ROLE_COPING) {
                // Concrete cap, or a soldier course on half the brick walls.
                bool soldier = family==0u && top==1u;
                float joint = barrier_lines(u/(bay/45.0),0.15,fw/(bay/45.0));
                color = soldier ? mix(paint*0.92,mortar,joint*0.8) : (family==3u ? paint*1.05 : vec3(0.72,0.72,0.69));
                rough = 0.8;
            } else if(role==ROLE_PIER && family==2u) {
                // Precast H-posts: concrete with the panel groove.
                color = vec3(0.66,0.66,0.63)*(1.0-0.35*barrier_band(abs(u),0.0,0.012,fw));
            } else if(role==ROLE_PIER && family==1u) {
                color *= 1.04;
            }
            if(role==ROLE_FAR_SOLID) {
                // Far walls: piers every bay, the plinth and coping bands.
                bool lattice = family!=3u;
                float pier = lattice ? barrier_lines(u/bay,(family==2u ? 0.15 : 0.4)/bay,fw/bay) : 0.0;
                color = mix(color,color*1.06,pier);
                color = mix(color,vec3(0.52,0.51,0.48),barrier_band(v,-1.0,0.2,fw)*0.65);
                float cap = barrier_band(v,h-(top==1u && family==3u ? 0.14 : 0.06),h+1.0,fw);
                color = mix(color,top==1u && family==3u ? vec3(0.24,0.26,0.28) : vec3(0.72,0.72,0.69),cap);
            }
        }
    } else if(role==ROLE_GIWA || role==ROLE_RIDGE) {
        // Giwa tiles: convex tile rows every 0.25 m along, darker eaves.
        float rows = 0.75+0.25*abs(sin(3.14159*u/(bay/12.0)));
        float ends = barrier_band(v,h-0.17,h-0.12,fw);
        color = vec3(0.24,0.26,0.28)*(role==ROLE_RIDGE ? 0.85 : rows)*(1.0-0.25*ends);
        rough = 0.55;
        spec = 0.45;
    } else if(role==ROLE_HEDGE) {
        // Clipped leaves: leaf clumps on the world surface (two octaves),
        // deep gaps darker, sun-lit clump tops lighter and a bumped normal
        // from the clump field (finite differences along the face), shade
        // toward the foot and a fresher clipped top.
        vec3 wn = normalize(world_normal);
        vec3 t1 = normalize(cross(wn,abs(wn.y)<0.9 ? vec3(0.0,1.0,0.0) : vec3(1.0,0.0,0.0)));
        vec3 t2 = cross(wn,t1);
        float leaves = barrier_noise3(world*16.0)*0.6+barrier_noise3(world*41.0)*0.4;
        float e = 0.012;
        float lx = barrier_noise3((world+t1*e)*16.0)*0.6+barrier_noise3((world+t1*e)*41.0)*0.4;
        float ly = barrier_noise3((world+t2*e)*16.0)*0.6+barrier_noise3((world+t2*e)*41.0)*0.4;
        float relief = (1.0-smoothstep(0.03,0.12,fw))*effect;
        vec3 bumped = normalize(wn-(t1*(lx-leaves)+t2*(ly-leaves))/e*0.035*relief);
        NORMAL = normalize((VIEW_MATRIX*vec4(bumped,0.0)).xyz);
        float gaps = 1.0-smoothstep(0.18,0.42,leaves);
        float top_light = smoothstep(h-0.2,h,v);
        color = paint*mix(0.78,1.22,smoothstep(0.25,0.85,leaves))*(1.0-0.55*gaps)*(0.7+0.3*smoothstep(0.0,0.45,v))*(1.0+0.1*top_light);
        color = mix(color,color*vec3(1.05,1.08,0.85),0.25*barrier_noise3(world*3.0));
        rough = 0.8;
        spec = 0.3;
    }
    color = barrier_linear(color*shade*(0.96+0.08*tint),OUTPUT_IS_SRGB);
    // Weather: wet darkens and glosses, snow settles on upward faces.
    float facing_up = smoothstep(0.35,0.9,world_normal.y);
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*facing_up*0.92);
    ALBEDO = color;
    ROUGHNESS = mix(rough,0.28,weather.x);
    METALLIC = metal;
    SPECULAR = spec;
#ifdef BARRIER_INFILL
    // Crisp alpha test where the pattern is resolved, an ordered dither as
    // it settles on its share (near meshes only).
    if(alpha < mix(0.5,barrier_bayer4(FRAGCOORD.xy),dither_mix)) discard;
#endif
#ifdef BARRIER_FAR
    ALPHA = alpha;
#endif
}
