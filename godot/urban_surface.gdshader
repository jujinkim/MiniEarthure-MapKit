shader_type spatial;
render_mode cull_disabled;
#include "wet_surface.gdshaderinc"
#include "material_detail.gdshaderinc"
uniform vec4 base_color : source_color = vec4(0.19,0.20,0.23,1.0);
uniform float snow_retention_percent : hint_range(0.0,100.0) = 100.0;
uniform bool environment_enabled = false;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool paving = false;
uniform bool marked = false;
uniform vec4 road_paths[128];
uniform vec4 road_metrics[128];
uniform vec4 road_borders[128];
uniform int path_count = 0;
uniform int edge_count = 0;
uniform int lanes = 2;
// Painted lane width in metres; 0 shares the road width between the lanes.
// With it, lanes stay centred, edge lines bound them and the rest is shoulder.
uniform float lane_width = 0.0;
uniform float median_width = 0.0;
uniform int forward_lanes = 0;
uniform bool center_line = true;
uniform bool edge_lines = true;
uniform bool crosswalk_start = false;
uniform bool crosswalk_end = false;
// Junction-end lane arrows (bits 1 left, 2 straight, 4 right; 8 junction) and
// inbound stop lines; one_way 1 along the points, -1 against them.
uniform int stop_start = 0;
uniform int stop_end = 0;
uniform int one_way = 0;
uniform bool left_hand = false;
// Concrete carriageways keep cast slabs; other paving lays blocks. Kerbed marked
// roads draw a concrete gutter along their carriageway edges.
uniform bool slabs = false;
uniform bool curbed = false;
// Night pools of the cell's city streetlights (city_lamp_meshes.gd
// bind_light_map; mapkit-godot lamp_map.rs): an RG8 map (light, lit foot
// height) whose texels sample the cell square edge to edge (rect: scene x0,
// z0, size m, texels; height: G = 0 scene y, m per step), unmipmapped so
// neighbouring cells meet on identical border texels at every distance.
// Surfaces well above the lamps' feet (a deck over the road, above the
// heads) or below them (a road under a lit one) stay dark.
uniform bool lamp_enabled = false;
uniform sampler2D lamp_map : filter_linear, repeat_disable;
uniform vec4 lamp_map_rect = vec4(0.0,0.0,1.0,2.0);
uniform vec2 lamp_map_height = vec2(0.0,0.16);
uniform vec4 lamp_color : source_color = vec4(1.0,0.929,0.839,1.0);
varying vec3 world_position;
varying vec3 world_normal;
void vertex() { world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz; world_normal=normalize(MODEL_NORMAL_MATRIX*NORMAL); }
float urban_hash(vec2 v) { return fract(sin(dot(v,vec2(127.1,311.7)))*43758.5453); }
float value_noise(vec2 v) {
    vec2 i=floor(v);
    vec2 f=fract(v);
    f=f*f*(3.0-2.0*f);
    return mix(mix(urban_hash(i),urban_hash(i+vec2(1.0,0.0)),f.x),mix(urban_hash(i+vec2(0.0,1.0)),urban_hash(i+vec2(1.0,1.0)),f.x),f.y);
}
// Paint is box filtered over the pixel footprint (metres along a direction), so
// every mark is antialiased near and settles to its area coverage far away.
// The footprint comes from world-position derivatives, which stay continuous
// where nearest-segment ownership changes at bends.
float paint_footprint(vec2 n,vec2 dx,vec2 dy) { return clamp(abs(dot(dx,n))+abs(dot(dy,n)),0.001,8.0); }
float box_cover(float x,float lo,float hi,float f) { return clamp((min(x+0.5*f,hi)-max(x-0.5*f,lo))/f,0.0,1.0); }
float half_plane(float d,float f) { return clamp(0.5-d/f,0.0,1.0); }
float stripe_integral(float x,float duty) { return floor(x)*duty+min(fract(x),duty); }
// Square wave on where fract(x)<duty; f is the footprint in periods.
float stripes(float x,float duty,float f) {
    f=max(f,1e-4);
    return clamp((stripe_integral(x+0.5*f,duty)-stripe_integral(x-0.5*f,duty))/f,0.0,1.0);
}
// Direction arrow 1.7 m long (5 m at 1:3): u metres forward from its base toward
// the stop line, l lane offset (driver's right +), fu/fl footprints; room_l and
// room_r are the clear widths left and right of the lane centre. Straight and
// turn parts combine on one shaft. A turn-only shaft prefers 12 cm away from its
// turn; any shaft shifts, and turn heads shorten, until the arrow fits its room.
float arrow(float u,float l,bool straight,bool left,bool right,float fu,float fl,float room_l,float room_r) {
    const float LEN=1.7;
    const float SHAFT=0.03;
    const float HEAD=0.5;
    const float HEAD_W=0.17;
    const float TURN_U=0.75;
    const float RADIUS=0.2;
    const float TURN_HEAD=0.34;
    const float TURN_W=0.17;
    if(!(straight || left || right)) return 0.0;
    float side_reach=straight ? HEAD_W : SHAFT;
    float reach_l=left ? RADIUS+TURN_HEAD : side_reach;
    float reach_r=right ? RADIUS+TURN_HEAD : side_reach;
    float excess=max(reach_l+reach_r-room_l-room_r,0.0);
    float head_l=clamp(TURN_HEAD-(right ? 0.5 : 1.0)*excess,0.12,TURN_HEAD);
    float head_r=clamp(TURN_HEAD-(left ? 0.5 : 1.0)*excess,0.12,TURN_HEAD);
    if(left) reach_l=RADIUS+head_l;
    if(right) reach_r=RADIUS+head_r;
    float lo=reach_l-room_l;
    float hi=room_r-reach_r;
    float preferred=!straight && left!=right ? (left ? 0.12 : -0.12) : 0.0;
    float x=l-(lo<=hi ? clamp(preferred,lo,hi) : 0.5*(lo+hi));
    float top=straight ? LEN-HEAD+0.02 : TURN_U+SHAFT;
    float paint=box_cover(u,0.0,top,fu)*box_cover(x,-SHAFT,SHAFT,fl);
    if(straight) {
        vec2 n=normalize(vec2(HEAD_W,HEAD));
        float edge=half_plane(n.x*(u-LEN)+n.y*abs(x),n.x*fu+n.y*fl);
        paint=max(paint,edge*half_plane(LEN-HEAD-u,fu));
    }
    for(int side=0;side<2;side++) {
        if(!(side==0 ? left : right)) continue;
        float t=(side==0 ? -1.0 : 1.0)*x;
        float turn_head=side==0 ? head_l : head_r;
        // Quarter bend from the shaft, then a head pointing across the lane.
        vec2 q=vec2(u-TURN_U,t-RADIUS);
        float radius=max(length(q),1e-4);
        float fr=abs(q.x/radius)*fu+abs(q.y/radius)*fl;
        float bend=half_plane(abs(radius-RADIUS)-SHAFT,fr)*half_plane(TURN_U-u,fu)*half_plane(t-RADIUS,fl);
        vec2 n=normalize(vec2(turn_head,TURN_W));
        float head=half_plane(n.x*abs(u-TURN_U-RADIUS)+n.y*(t-RADIUS-turn_head),n.x*fu+n.y*fl)*half_plane(RADIUS-0.01-t,fl);
        paint=max(paint,max(bend,head));
    }
    return paint;
}
// Crosswalk warning diamond outline (Korean 횡단보도 예고), 1.5 m by 0.5 m.
float diamond(float du,float l,float fu,float fl) {
    const float DL=0.75;
    const float DW=0.25;
    float k=inversesqrt(DL*DL+DW*DW);
    float d=(abs(du)*DW+abs(l)*DL-DL*DW)*k;
    return half_plane(abs(d)-0.025,(DW*fu+DL*fl)*k);
}
// ---- Sidewalk paving and curbs (display only) ----
// Sidewalk tops and walls carry a curb frame (mapkit-godot road_style.rs): UV is
// (along, across from the curb) on tops and (along, rise) on walls; UV2 is
// (offset from a crosswalk centre, kind + 8 * section). Kinds: 0 none (world-
// aligned paving), 1 top, 2 top at a crossing mouth, 3 curb face, 4 other edge.
// Sizes are 1:3: a 10 cm block unit, an 18 cm curb top and 1 m curb stones.
const float PAVE_UNIT=1.0/30.0;
const float CURB_TOP=0.06;
const float CURB_STONE=1.0/3.0;
const float SOLDIER_END=CURB_TOP+2.0/30.0;
uint pave_hash(ivec2 c,uint salt) {
    uint h=uint(c.x)*73856093u^uint(c.y)*19349663u^salt*83492791u;
    h=(h^(h>>13u))*1274126177u;
    return h^(h>>16u);
}
float pave_unit(ivec2 c,uint salt) { return float(pave_hash(c,salt)&65535u)/65535.0; }
// Korean paving mixes: grey, red-brown with grey, ochre, charcoal with grey.
vec3 pave_palette(int palette,int i) {
    if(palette==1) return i==0 ? vec3(0.20,0.075,0.05) : (i==1 ? vec3(0.15,0.056,0.04) : vec3(0.20,0.13,0.10));
    if(palette==2) return i==0 ? vec3(0.38,0.28,0.14) : (i==1 ? vec3(0.31,0.23,0.13) : vec3(0.43,0.37,0.26));
    if(palette==3) return i==0 ? vec3(0.09,0.09,0.085) : (i==1 ? vec3(0.14,0.14,0.13) : vec3(0.24,0.24,0.22));
    return i==0 ? vec3(0.30,0.30,0.29) : (i==1 ? vec3(0.24,0.24,0.23) : vec3(0.37,0.36,0.34));
}
// Mix shares 45/35/20 %; the mean is what a block field settles to far away.
vec3 pave_mean(int palette) { return 0.45*pave_palette(palette,0)+0.35*pave_palette(palette,1)+0.2*pave_palette(palette,2); }
// One block of a bond in block units: id, kind salt, local position and size.
struct PaveBlock { ivec2 id; uint salt; vec2 local; vec2 size; };
// 90-degree herringbone of 2x1 blocks: diagonal bands four units wide, each a
// staircase of horizontal blocks with vertical blocks beside it.
PaveBlock herringbone(vec2 x) {
    vec2 c=floor(x);
    float band=floor((c.x-c.y)/4.0);
    float m=c.x-c.y-4.0*band;
    float xs=x.x-4.0*band;
    if(m<1.5) return PaveBlock(ivec2(int(band),int(c.y)),1u,vec2(xs-c.y,x.y-c.y),vec2(2.0,1.0));
    float k=m<2.5 ? c.y : c.y+1.0;
    return PaveBlock(ivec2(int(band),int(k)),2u,vec2(xs-k-2.0,x.y-k+1.0),vec2(1.0,2.0));
}
PaveBlock running_bond(vec2 x) {
    float row=floor(x.y);
    float shifted=x.x+mod(row,2.0);
    float column=floor(shifted/2.0);
    return PaveBlock(ivec2(int(column),int(row)),3u,vec2(shifted-2.0*column,x.y-row),vec2(2.0,1.0));
}
PaveBlock squares(vec2 x,float side,uint salt) {
    vec2 c=floor(x/side);
    return PaveBlock(ivec2(c),salt,x-c*side,vec2(side));
}
// Granite curb stone at (along, other) metres: per-stone tone, black, white and
// feldspar grains (1.5 mm cells, faded with the footprint), sawn joints 1 m apart.
vec3 granite(vec2 q,float f,bool fine) {
    ivec2 stone=ivec2(int(floor(q.x/CURB_STONE)),0);
    vec3 c=vec3(0.37,0.37,0.36)*(0.92+0.14*pave_unit(stone,11u));
    if(fine) {
        float grain=1.0-smoothstep(0.0008,0.003,f);
        if(grain>0.0) {
            ivec2 cell=ivec2(floor(q/0.0015));
            float g=pave_unit(cell,13u);
            vec3 speck=g<0.08 ? vec3(0.06) : (g>0.93 ? vec3(0.58,0.57,0.55) : (g>0.86 ? vec3(0.42,0.33,0.30) : c));
            c=mix(c,speck,grain);
        }
    }
    float joint=stripes(q.x/CURB_STONE-0.0045,0.009,f/CURB_STONE);
    return mix(c,vec3(0.16,0.15,0.14),joint*0.85);
}
// Sidewalk top: curb band, a soldier course and the section's bond, tactile
// paving at crossing mouths. st (along, across) metres, f its pixel footprint,
// crossing/fc the crosswalk offset and its footprint. Returns albedo; slope is
// the height gradient over st and rough the roughness.
vec3 sidewalk_top(vec2 st,float f,float crossing,float fc,int kind,int section,float detail,float close,inout vec2 slope,inout float rough) {
    float w=abs(st.y);
    int palette=(section/3)%4;
    int bond=section%3;
    bool fine=detail_quality>0;
    float units=f/PAVE_UNIT;
    // Block field (soldier course along the curb, then the bond); far away it
    // settles to the mix mean with the joints' mean share.
    PaveBlock b;
    if(w<SOLDIER_END) b=squares(vec2(st.x/PAVE_UNIT,(w-CURB_TOP)*0.5/PAVE_UNIT),1.0,4u);
    else if(bond==0) b=herringbone(vec2(st.x,w-SOLDIER_END)/PAVE_UNIT);
    else if(bond==1) b=running_bond(vec2(st.x,w-SOLDIER_END)/PAVE_UNIT);
    else b=squares(vec2(st.x,w-SOLDIER_END)/PAVE_UNIT,2.0,5u);
    if(w<SOLDIER_END) { b.local.y*=2.0; b.size=vec2(1.0,2.0); }
    float h=pave_unit(b.id,b.salt+uint(section)*7u);
    float pick=pave_unit(b.id,b.salt+101u);
    int tone=pick<0.45 ? 0 : (pick<0.8 ? 1 : 2);
    vec3 mean=pave_mean(palette);
    vec3 block=pave_palette(palette,tone)*(0.94+0.12*pave_unit(b.id,b.salt+57u));
    float vary=(1.0-smoothstep(0.6,2.5,units))*detail;
    float edge=min(min(b.local.x,b.size.x-b.local.x),min(b.local.y,b.size.y-b.local.y));
    const float JOINT=0.035;
    float sharp=1.0-smoothstep(JOINT-0.5*units,JOINT+0.5*units,edge);
    float share=1.0-(b.size.x-2.0*JOINT)*(b.size.y-2.0*JOINT)/(b.size.x*b.size.y);
    float joint=mix(sharp,share,smoothstep(0.25,0.9,units));
    vec3 sand=vec3(0.30,0.27,0.21);
    if(fine && vary>0.0) {
        // Replaced (newer), sunken and cracked blocks; chamfered edges.
        if(h>0.93) block=pave_palette(palette,(tone+1)%3)*1.1;
        float bevel=1.0-smoothstep(0.0,0.12,edge);
        block*=1.0-0.18*bevel*(1.0-smoothstep(0.1,0.5,units));
        if(h<0.02) block*=0.82;
        if(h>=0.02 && h<0.035) {
            vec2 n=normalize(vec2(pave_unit(b.id,71u)-0.5,pave_unit(b.id,73u)-0.5)+vec2(0.01));
            float d=abs(dot(b.local-0.5*b.size,n)+0.15*(value_noise(b.local*9.0+vec2(h*40.0))-0.5));
            block*=1.0-0.5*(1.0-smoothstep(0.015,0.015+units,d))*(1.0-smoothstep(0.2,0.6,units));
        }
        // Grime collects along joints.
        sand=mix(sand,vec3(0.11,0.10,0.09),value_noise(st*13.0));
        if(detail_quality>1 && close>0.0) {
            vec2 tilt=(vec2(pave_unit(b.id,31u),pave_unit(b.id,37u))-0.5)*(h<0.02 ? 0.12 : 0.03);
            // Chamfers slope toward the joints (block units).
            vec2 toward=vec2(b.local.x<0.5*b.size.x ? -1.0 : 1.0,b.local.y<0.5*b.size.y ? -1.0 : 1.0);
            vec2 chamfer=vec2(1.0-smoothstep(0.0,0.1,min(b.local.x,b.size.x-b.local.x)),1.0-smoothstep(0.0,0.1,min(b.local.y,b.size.y-b.local.y)));
            slope+=(tilt-0.25*toward*chamfer)*close*(1.0-smoothstep(0.15,0.4,units));
        }
    }
    vec3 c=mix(mean,block,vary);
    // Recessed sand joints read darker than the block faces beside them.
    c=mix(c,min(sand,c)*0.55,joint*mix(0.6,0.9,vary));
    rough=0.9+0.05*joint;
    // Tactile paving at a crossing mouth: dotted warning tiles 30 cm from the
    // curb, as wide as the crossing and two tiles deep, and a linear guide
    // strip leading in from the sidewalk; 30 cm tiles (0.1 m at 1:3).
    if(kind==2) {
        float pad=box_cover(crossing,-0.7,0.7,fc)*box_cover(w,0.1,0.3,f);
        float strip=box_cover(crossing,-0.05,0.05,fc)*box_cover(w,0.3,1.6,f);
        float tactile=max(pad,strip);
        if(tactile>0.0) {
            vec2 t=vec2(crossing+0.05,w-0.1)/0.1;
            vec2 local=fract(t);
            float ft=max(fc,f)/0.1;
            float seam=max(stripes(t.x-0.015,0.03,ft),stripes(t.y-0.015,0.03,ft));
            vec3 yellow=vec3(0.78,0.52,0.045)*(0.95+0.08*pave_unit(ivec2(floor(t)),17u)*vary);
            if(fine && ft<0.5) {
                // Raised dots (5 x 5 per tile) or four raised bars along the walk.
                float relief;
                vec2 g;
                if(pad>=strip) {
                    vec2 q=fract(local*5.0)-0.5;
                    float r=length(q);
                    relief=1.0-smoothstep(0.28-2.5*ft,0.28+2.5*ft,r);
                    g=r>0.001 ? q/r : vec2(0.0);
                } else {
                    float q=fract(local.x*4.0)-0.5;
                    relief=1.0-smoothstep(0.2-2.0*ft,0.2+2.0*ft,abs(q));
                    g=vec2(sign(q),0.0);
                }
                float fade=1.0-smoothstep(0.15,0.5,ft);
                // Rims of the raised studs catch shade on their lower side.
                float rim=relief*(1.0-relief)*4.0;
                yellow*=1.0-0.25*rim*fade;
                if(detail_quality>1) slope-=g*rim*0.35*fade*close;
            }
            yellow=mix(yellow,vec3(0.22,0.17,0.07),seam*0.6);
            c=mix(c,yellow,tactile);
            rough=mix(rough,0.62,tactile);
        }
    }
    // Granite curb top, an 18 cm band along the curb with a worn arris.
    float curb=box_cover(w,-1.0,CURB_TOP,f);
    if(curb>0.0) {
        vec3 g=granite(vec2(st.x,w),f,fine);
        float arris=box_cover(w,-1.0,0.004,f);
        g*=1.0+0.12*arris*detail;
        // A mortar line between the curb and the soldier course.
        g=mix(g,vec3(0.17,0.16,0.14),box_cover(w,CURB_TOP-0.003,CURB_TOP+0.001,f)*0.7);
        c=mix(c,g,curb);
        rough=mix(rough,0.72,curb);
        if(detail_quality>1) slope+=vec2(0.0,0.5)*arris*close;
    }
    return c;
}
// Kerbside gutter on a carriageway: e metres in from its edge, s the station;
// a 36 cm concrete strip with joints every metre, a damp line at the curb and a
// dark seam where the asphalt meets it.
vec3 gutter(vec3 asphalt,float e,float s,float fa,float fs,float weight) {
    const float WIDTH=0.12;
    float strip=box_cover(e,-1.0,WIDTH,fa);
    if(strip<=0.0) return asphalt;
    vec3 c=vec3(0.23,0.228,0.218)*(0.94+0.1*value_noise(vec2(s*3.0,e*20.0))*weight);
    c=mix(c,vec3(0.15,0.145,0.135),stripes(s/CURB_STONE-0.004,0.008,fs/CURB_STONE)*0.7);
    // Water runs along the curb: a darker damp band and sediment.
    c*=1.0-0.35*box_cover(e,-1.0,0.02,fa)-0.12*box_cover(e,0.02,0.05,fa)*weight;
    vec3 result=mix(asphalt,c,strip);
    return mix(result,vec3(0.06,0.06,0.065),box_cover(e,WIDTH-0.004,WIDTH+0.002,fa)*0.6);
}
void fragment() {
    vec2 p=world_position.xz;
    // Uniform control flow: paint footprints and the paint relief below.
    vec2 pdx=dFdx(p);
    vec2 pdy=dFdy(p);
    // Paving frame and its footprints (uniform control flow): sidewalk tops lay
    // blocks in (along, across), walls in (along, rise), other paving in world xz.
    float frame=paving && !slabs ? floor(UV2.y+0.5) : 0.0;
    int kind=int(mod(frame,8.0));
    int section=int(floor(frame/8.0));
    vec2 st=kind==0 ? p : UV;
    vec2 sdx=dFdx(st);
    vec2 sdy=dFdy(st);
    float sf=clamp(max(abs(sdx.x)+abs(sdy.x),abs(sdx.y)+abs(sdy.y)),1e-5,8.0);
    float cf=clamp(abs(dFdx(UV2.x))+abs(dFdy(UV2.x)),1e-5,8.0);
    float effect_weight=surface_effect_weight(length(VERTEX));
    vec3 color=base_color.rgb;
    float view_distance=length(VERTEX);
    // Small features fade before they could shimmer between pixels.
    float close=effect_weight*(1.0-smoothstep(6.0,24.0,view_distance));
    vec2 pave_slope=vec2(0.0);
    float pave_rough=0.9;
    if(paving && !slabs) {
        if(kind==1 || kind==2) {
            vec2 slope=vec2(0.0);
            // Palettes settle to the material's base colour (the distant mesh's
            // concrete) with the effect weight, so near and distant cells meet.
            color=mix(base_color.rgb,sidewalk_top(st,sf,UV2.x,cf,kind,section,effect_weight,close,slope,pave_rough),effect_weight);
            pave_slope=vec2(slope.x,slope.y*(st.y<0.0 ? -1.0 : 1.0));
        } else if(kind>=3) {
            // Curb faces are granite like the curb top (joints line up); other
            // sidewalk edges show cut blocks. Both gather grime at the base.
            color=mix(base_color.rgb,kind==3 ? granite(st,sf,detail_quality>0) : pave_mean((section/3)%4)*0.8,effect_weight);
            float rise=max(st.y,0.0);
            color*=1.0-(0.35*(1.0-smoothstep(0.0,0.012,rise))+0.15*box_cover(rise,-1.0,0.003,sf))*effect_weight;
            pave_rough=kind==3 ? 0.75 : 0.92;
        } else {
            // World-aligned 30 cm squares in the base tone.
            PaveBlock b=squares(p/PAVE_UNIT,3.0,6u);
            float units=sf/PAVE_UNIT;
            float vary=(1.0-smoothstep(0.6,2.5,units))*effect_weight;
            float edge=min(min(b.local.x,b.size.x-b.local.x),min(b.local.y,b.size.y-b.local.y));
            float joint=mix(1.0-smoothstep(0.035-0.5*units,0.035+0.5*units,edge),1.0-pow(1.0-0.07/3.0,2.0),smoothstep(0.25,0.9,units));
            color*=mix(1.0,0.93+0.12*pave_unit(b.id,6u),vary);
            color=mix(color,color*0.6,joint*0.6);
        }
    }
    float wear=0.5;
    float rough_shift=0.0;
    float stone=0.0;
    float crack=0.0;
    if(effect_weight>0.0) {
        float broad=value_noise(p*0.18)*0.6+value_noise(p*0.55+vec2(11.0,4.0))*0.4;
        float fine=value_noise(p*6.0);
        wear=value_noise(p*2.3+vec2(17.0,3.0));
        if(!paving) {
            // Asphalt: broad tonal blotches, light aggregate, sealed repair
            // patches and thin cracks. Roughness follows the same wear.
            color*=1.0+((broad-0.5)*0.18+(fine-0.5)*0.06)*effect_weight;
            stone=smoothstep(0.80,0.92,value_noise(p*26.0+vec2(fine*3.0)));
            color=mix(color,color*1.35+0.02,stone*0.35*close);
            vec2 slot=floor(p/3.5);
            vec2 local=fract(p/3.5);
            vec2 extent=0.2+0.5*vec2(urban_hash(slot+1.3),urban_hash(slot+2.7));
            float repaired=step(0.86,urban_hash(slot+vec2(5.0,9.0)))*step(abs(local.x-0.5),extent.x*0.5)*step(abs(local.y-0.5),extent.y*0.5);
            color*=1.0-0.13*repaired*effect_weight;
            // Domain-warped ridges keep cracks jagged and sparse.
            vec2 warped=p*1.3+0.35*vec2(value_noise(p*5.0),value_noise(p*5.0+vec2(5.0,1.0)));
            float ridge=abs(value_noise(warped+vec2(3.0,7.0))-0.5);
            crack=(1.0-smoothstep(0.0,0.005+fwidth(ridge),ridge))*smoothstep(0.68,0.78,value_noise(p*0.3+vec2(1.0,9.0)));
            color*=1.0-0.28*crack*close;
            rough_shift=(broad-0.5)*0.08-0.05*repaired;
        } else {
            // Paving: broad weathering; cast slabs also vary per slab.
            color*=1.0+((slabs ? (urban_hash(floor(p/0.7))-0.5)*0.08 : 0.0)+(broad-0.5)*0.12)*effect_weight;
        }
    }
    float tile_roughness=0.9;
    float tile_tone=1.0;
    if(detail_enabled && length(VERTEX)<48.0 && effect_weight>0.0) {
        vec4 detail=texture(detail_tile,detail_uv(world_position,world_normal));
        float relief=material_detail_weight(length(VERTEX))*0.9;
        tile_tone=mix(1.0,detail_luminance(detail.r),relief);
        color*=tile_tone;
        if(detail_quality>0) tile_roughness=mix(0.9,detail.g,relief);
        if(detail_quality>1) NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(detail_bump(world_position,world_normal)*relief,0.0)).xyz);
        if(detail_quality>0) AO=mix(1.0,detail.r/0.65,0.18*relief);
    }
    if (paving && slabs && effect_weight>0.0) {
        vec2 q=abs(fract(p/0.7)-0.5);
        float seams=max(smoothstep(0.475,0.5,q.x),smoothstep(0.475,0.5,q.y));
        color*=1.0-0.13*seams*effect_weight;
    }
    float paint_amount=0.0;
    float stud=0.0;
    vec3 paint_tilt=vec3(0.0);
    if (marked) {
        float nearest=1e20;
        float across=0.0;
        float station=0.0;
        float half_width=0.0;
        float period=2.4;
        float total=0.0;
        bool body=false;
        vec2 along=vec2(1.0,0.0);
        for(int i=0;i<128;i++) {
            if(i>=path_count) break;
            vec2 a=road_paths[i].xy;
            vec2 delta=road_paths[i].zw-a;
            float len=length(delta);
            vec2 dir=delta/max(len,0.001);
            float t=dot(p-a,dir);
            vec2 separation=p-a-dir*clamp(t,0.0,len);
            float distance=length(separation);
            if(distance<nearest) {
                nearest=distance;
                across=sign(dot(p-a,vec2(-dir.y,dir.x)))*distance;
                station=road_metrics[i].y+clamp(t,0.0,len);
                half_width=road_metrics[i].x*0.5;
                period=road_metrics[i].z;
                total=road_metrics[i].w;
                along=dir;
                body=(t>=-0.005 && t<=len+0.005) || (station>0.005 && station<total-0.005);
            }
        }
        // Footprints along the path (station) and across it.
        float fs=paint_footprint(along,pdx,pdy);
        float fa=paint_footprint(vec2(-along.y,along.x),pdx,pdy);
        float fine_fade=1.0-smoothstep(0.004,0.012,max(fs,fa));
        if(curbed && body) color=gutter(color,half_width-abs(across),station,fa,fs,effect_weight);
        float white=0.0;
        float yellow=0.0;
        float median=one_way==0 ? min(median_width,max(0.0,2.0*half_width-0.02)) : 0.0;
        float lane_half=lane_width>0.0 ? min(half_width-0.5*median,0.5*lane_width*float(max(lanes,1))) : half_width;
        int forwards=forward_lanes>0 ? forward_lanes : max(lanes/2,1);
        int left_count=left_hand ? forwards : lanes-forwards;
        float centre_x=one_way==0 && forward_lanes>0 ? -lane_half+2.0*lane_half*float(left_count)/float(max(lanes,1)) : 0.0;
        float median_mask=median>0.0 ? 1.0-box_cover(across,centre_x-0.5*median,centre_x+0.5*median,fa) : 1.0;
        // Remove the unpainted centre band from lane coordinates only. Gutter and
        // physical road edges above keep their original coordinates.
        float physical_across=across;
        across-=sign(across-centre_x)*0.5*median;
        bool shoulder=lane_width>0.0 && lane_half+0.5*median<half_width-0.12;
        if(edge_lines && !shoulder) {
            for(int i=0;i<128;i++) {
                if(i>=edge_count) break;
                vec2 a=road_borders[i].xy;
                vec2 delta=road_borders[i].zw-a;
                float len=length(delta);
                vec2 dir=delta/max(len,0.001);
                vec2 inward=vec2(dir.y,-dir.x);
                a+=inward*0.16;
                float t=clamp(dot(p-a,dir),0.0,len);
                white=max(white,box_cover(length(p-a-dir*t),-0.03,0.03,paint_footprint(inward,pdx,pdy)));
            }
        }
        if(body) {
            // Inbound half in the driver's frame r=h*across (right +); h is -1 at
            // the start, +1 at the end. One-way roads stop only at the travel end.

            // Arrows keep 4 cm clear of dividers, the double yellow and edge lines
            // (about 11 cm from a single yellow).
            float centre_clear=0.135;
            float outer_clear=!edge_lines ? lane_half-0.04 : (shoulder ? lane_half-0.07 : min(lane_half-0.07,half_width-0.23));
            bool crossed_end[2];
            crossed_end[0]=crosswalk_start && total>5.0;
            crossed_end[1]=crosswalk_end && total>5.0;
            // Korean no-lane-change lines: the last 10 m (30 m at 1:3) before a stop line are solid.
            float solid_length=min(10.0,0.45*total);
            float keep=1.0;
            float crossing=0.0;
            float bars=0.0;
            float to_end[2];
            float stop_bar[2];
            bool stopped[2];
            for(int s=0;s<2;s++) {
                float h=s==0 ? -1.0 : 1.0;
                float r0=one_way!=0 || left_hand ? -lane_half : h*centre_x;
                float r1=one_way!=0 || !left_hand ? lane_half : h*centre_x;
                int count=one_way!=0 ? lanes : (forward_lanes>0 ? (s==1 ? forwards : lanes-forwards) : max(lanes/2,1));
                float lane_span=(r1-r0)/float(max(count,1));
                float stop_lo=r0+(one_way==0 && !left_hand ? 0.1 : 0.03);
                float stop_hi=r1-(one_way==0 && left_hand ? 0.1 : 0.03);
                float d=s==0 ? station : total-station;
                bool crossed=crossed_end[s];
                int mask=s==0 ? stop_start : stop_end;
                bool stop=(mask!=0 || crossed) && total>5.0 && (one_way==0 || float(one_way)==h);
                float bar=crossed ? 1.95 : 0.55;
                to_end[s]=d;
                stop_bar[s]=bar;
                stopped[s]=stop;
                // Lines end at the stop line, or at the crosswalk on a departure end.
                float cut=stop ? bar : (crossed ? 1.7 : 0.0);
                if(cut>0.0) keep*=clamp((d-cut)/fs+0.5,0.0,1.0);
                // Branch windows widen by half the footprint so filtered edges are never cut.
                if(crossed && d<1.725+0.5*fs) {
                    // Korean zebra: 0.5 m bars and 0.65 m gaps (1:3) along traffic,
                    // whole bars centred on the road over a 1.4 m deep crossing.
                    const float PITCH=0.383;
                    const float BAR=0.167;
                    float zone=box_cover(d,0.3,1.7,fs);
                    float limit=floor((half_width-0.2-0.5*BAR)/PITCH)*PITCH+0.5*BAR;
                    // Integrate the finite stripe train over the clipped footprint;
                    // multiplying two filtered masks darkens the outer bar.
                    float lo=max(physical_across-0.5*fa,-limit);
                    float hi=min(physical_across+0.5*fa,limit);
                    float span=max(0.0,hi-lo);
                    float b=span/fa*stripes((0.5*(lo+hi)+0.5*BAR)/PITCH,BAR/PITCH,span/PITCH);
                    float index=floor(physical_across/PITCH+0.5);
                    // Per-bar wear; chipped ends and edges; grime in the gaps.
                    b*=mix(1.0,0.82+0.18*urban_hash(vec2(index,total*0.37+float(s)*5.0)),effect_weight);
                    if(detail_quality>0 && fine_fade>0.0) {
                        float inside=min(0.5*BAR-abs(physical_across-index*PITCH),min(d-0.3,1.7-d));
                        float chip=(1.0-smoothstep(0.0,0.04,inside))*smoothstep(0.58,0.74,value_noise(p*34.0+vec2(index*3.1,0.0)));
                        b*=1.0-chip*fine_fade*effect_weight;
                        color*=1.0-0.09*zone*(1.0-b)*(0.4+0.6*value_noise(p*9.0))*close;
                    }
                    crossing=max(crossing,zone);
                    bars=max(bars,b*zone);
                }
                if(stop && d<bar+18.0+0.5*(fs+3.0*fa)) {
                    float r=h*across;
                    white=max(white,box_cover(d,bar,bar+0.13,fs)*box_cover(r,stop_lo,stop_hi,fa));
                    if(r>r0 && r<r1) {
                        int k=int(clamp(floor((r-r0)/lane_span),0.0,float(count-1)));
                        float centre=r0+(float(k)+0.5)*lane_span;
                        float v=r-centre;
                        bool turn_left=(mask&1)!=0 && k==0;
                        bool turn_right=(mask&4)!=0 && k==count-1;
                        bool straight=(mask&2)!=0 && (count<=2 || (k>0 && k<count-1));
                        if(count==1) { turn_left=(mask&1)!=0; turn_right=(mask&4)!=0; straight=(mask&2)!=0; }
                        if(mask!=0 && (straight || turn_left || turn_right)) {
                            // Clear widths beside the lane centre: a lane's outer side
                            // meets the centre line (r 0; double or single yellow)
                            // or the edge line.
                            float left_limit=k>0 ? r0+float(k)*lane_span+0.07 : (one_way==0 && abs(r0-h*centre_x)<0.001 ? r0+centre_clear : -outer_clear);
                            float right_limit=k<count-1 ? r0+float(k+1)*lane_span-0.07 : (one_way==0 && abs(r1-h*centre_x)<0.001 ? r1-centre_clear : outer_clear);
                            float room_l=centre-left_limit;
                            float room_r=right_limit-centre;
                            // Arrows 0.5 m behind the stop line, repeated 7.5 m back on
                            // long approaches; each needs room before the far end's marks.
                            float u=bar+2.2-d;
                            float far_clear=crossed_end[1-s] ? 2.1 : 0.7;
                            float reach=0.5*fs+0.01;
                            if(total>bar+2.2+far_clear && u>-reach && u<1.7+reach) white=max(white,arrow(u,v,straight,turn_left,turn_right,fs,fa,room_l,room_r));
                            if(total>bar+14.0 && u>-7.5-reach && u<-5.8+reach) white=max(white,arrow(u+7.5,v,straight,turn_left,turn_right,fs,fa,room_l,room_r));
                        }
                        if(crossed) {
                            // Warning diamonds 5.5 m and 17 m before the stop line (16 and 51 m real).
                            float reach=0.84+0.5*(fs+3.0*fa);
                            if(total>bar+9.5 && abs(d-bar-5.5)<reach) white=max(white,diamond(d-bar-5.5,v,fs,fa));
                            if(total>bar+21.0 && abs(d-bar-17.0)<reach) white=max(white,diamond(d-bar-17.0,v,fs,fa));
                        }
                    }
                }
            }
            if(edge_lines && shoulder) white=max(white,box_cover(abs(across)-lane_half,-0.03,0.03,fa)*keep);
            // Dividers: Korean urban 3:5 dashes in each fitted period, solid on
            // the inbound approach; even lane counts paint a double yellow centre,
            // two-lane roads without edge lines a single one.
            const float DUTY=0.375;
            bool single_yellow=lanes==2 && !edge_lines;
            float dash=stripes(station/max(period,0.001)+0.5*DUTY,DUTY,fs/max(period,0.001));
            for(int lane=1;lane<16;lane++) {
                if(lane>=lanes) break;
                float x=-lane_half+2.0*lane_half*float(lane)/float(lanes);
                if(center_line && (forward_lanes>0 ? lane==left_count : lane*2==lanes)) {
                    float centre_paint=single_yellow ? box_cover(across-centre_x,-0.025,0.025,fa) : box_cover(across-centre_x,0.045,0.095,fa)+box_cover(across-centre_x,-0.095,-0.045,fa);
                    yellow=max(yellow,centre_paint*keep);
                } else {
                    float pattern=dash;
                    for(int s=0;s<2;s++) {
                        float h=s==0 ? -1.0 : 1.0;
                        if(stopped[s] && (one_way!=0 || (left_hand ? h*(x-centre_x)<-0.01 : h*(x-centre_x)>0.01))) pattern=max(pattern,box_cover(to_end[s],stop_bar[s],stop_bar[s]+solid_length,fs));
                    }
                    white=max(white,box_cover(across,x-0.028,x+0.028,fa)*pattern*keep);
                }
            }
            // Reflective studs on the centre, one per two dash periods.
            if(center_line && (forward_lanes>0 || lanes%2==0)) {
                float spacing=2.0*max(period,0.001);
                stud=stripes(station/spacing+0.02/spacing,0.04/spacing,fs/spacing)*box_cover(across-centre_x,-0.02,0.02,fa)*keep*(1.0-crossing);
            }
            white=max(white*(1.0-crossing),bars);
            yellow*=1.0-crossing;
            white*=median_mask; yellow*=median_mask; stud*=median_mask;
        }
        // Worn paint: uneven coverage, scuffed by wheel tracks; aggregate and
        // cracks of the asphalt show through.
        float track=0.0;
        if(body && effect_weight>0.0) {
            float lane_span=2.0*lane_half/float(max(lanes,1));
            float lateral=abs(fract((across+lane_half)/max(lane_span,0.01))-0.5);
            track=(1.0-smoothstep(0.04,0.10,abs(lateral-0.24)))*step(abs(across),lane_half);
            color*=1.0-0.07*track*effect_weight;
            rough_shift-=0.10*track*effect_weight;
        }
        float coverage=mix(1.0,clamp(0.62+0.5*smoothstep(0.2,0.65,wear)-0.3*track,0.35,1.0),effect_weight);
        coverage*=1.0-(0.75*crack+0.3*stone)*close;
        // Fine abrasion specks near the camera (2.5 cm, 7.5 cm real).
        if(detail_quality>0 && fine_fade>0.0) coverage*=1.0-0.5*smoothstep(0.6,0.85,value_noise(p*40.0+vec2(7.0,2.0)))*fine_fade*close;
        // Thermoplastic keeps a little of the asphalt texture; tyres grey it.
        float grime=(0.96+0.06*wear)*mix(1.0,tile_tone,0.4)*(1.0-0.16*track*effect_weight);
        color=mix(color,vec3(0.86,0.86,0.81)*grime,white*coverage);
        color=mix(color,vec3(0.85,0.64,0.21)*grime,yellow*coverage);
        color=mix(color,vec3(0.62,0.60,0.55),stud);
        paint_amount=max(white,yellow)*coverage;
        rough_shift-=0.25*paint_amount;
        // Raised thermoplastic edges: the mask gradient in world xz tilts the
        // normal over the paint's 1 mm (3 mm real) rim, near the camera only.
        float mask=max(white,yellow);
        vec2 dm=vec2(dFdx(mask),dFdy(mask));
        float det=pdx.x*pdy.y-pdx.y*pdy.x;
        if(detail_quality>0 && view_distance<8.0 && abs(det)>1e-12) {
            vec2 gradient=vec2(dm.x*pdy.y-dm.y*pdx.y,dm.y*pdx.x-dm.x*pdy.x)/det;
            vec2 slope=gradient*0.001*(1.0-smoothstep(3.0,8.0,view_distance));
            float slope_length=length(slope);
            if(slope_length>0.4) slope*=0.4/slope_length;
            paint_tilt=vec3(-slope.x,0.0,-slope.y);
        }
    }
    vec2 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0).xy : vec2(0.0);
    float sun_altitude = environment_enabled ? texelFetch(environment_data,ivec2(1,0),0).z : 1.0;
    weather.y *= snow_retention_percent * 0.01;
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*0.90);
    ALBEDO=color;
    if(paving && !slabs) rough_shift+=pave_rough-0.9;
    ROUGHNESS=mix(mix(clamp(tile_roughness+rough_shift,0.4,1.0),0.27,weather.x),0.95,weather.y);
    float puddle=0.0;
    if(effect_weight>0.0 && weather.x>0.2) puddle=puddle_mask(world_position,world_normal.y,weather.x,weather.y)*effect_weight;
    ALBEDO*=1.0-puddle*0.10;
    ROUGHNESS=mix(ROUGHNESS,0.22,puddle);
    SPECULAR=mix(mix(0.35,0.45,paint_amount*(1.0-weather.y)),0.50,puddle);
    if(paint_amount>0.0 && detail_quality>0) {
        NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(paint_tilt*(1.0-weather.y),0.0)).xyz);
        if(view_distance<3.0) {
            // Glass beads: fixed 8 mm world cells (integer hash, no time), only
            // where a cell spans several pixels.
            ivec2 cell=ivec2(floor(p/0.008));
            uint bead=uint(cell.x)*73856093u^uint(cell.y)*19349663u;
            bead=(bead^(bead>>13u))*1274126177u;
            bead^=bead>>16u;
            float glint=step(0.965,float(bead&1023u)/1023.0)*paint_amount*(1.0-weather.y)
                *(1.0-smoothstep(0.0015,0.004,length(pdx)+length(pdy)))*(1.0-smoothstep(1.5,3.0,view_distance));
            ROUGHNESS=mix(ROUGHNESS,0.08,glint);
            SPECULAR=mix(SPECULAR,0.9,glint);
            ALBEDO+=0.05*glint;
        }
    }
    // Studs return headlight-like amber at night; snow buries them.
    if(stud>0.0) EMISSION=vec3(1.0,0.62,0.18)*stud*0.45*(1.0-smoothstep(-0.08,0.02,sun_altitude))*(1.0-weather.y);
    // Streetlight pools after sunset, when the lenses light: the surface's
    // own albedo lit (paint brighter than asphalt, snow brightest) plus a
    // faint sheen that grows on wet ground.
    if(lamp_enabled && sun_altitude<0.0) {
        float texels=lamp_map_rect.w;
        vec2 lamp_uv=((world_position.xz-lamp_map_rect.xy)/lamp_map_rect.z*(texels-1.0)+0.5)/texels;
        vec2 lamp=texture(lamp_map,lamp_uv).rg;
        float rise=world_position.y-(lamp_map_height.x+lamp.g*255.0*lamp_map_height.y);
        float lit=lamp.r*(1.0-smoothstep(2.4,3.0,rise))*smoothstep(-2.6,-1.6,rise);
        EMISSION+=lamp_color.rgb*lit*(ALBEDO*1.5+vec3(0.012+0.05*weather.x));
    }
    if(puddle>0.0) NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
    // Block tilt, chamfers, tactile studs and the curb arris: the st height
    // gradient maps to world xz through the frame's derivative Jacobian.
    float det=pdx.x*pdy.y-pdx.y*pdy.x;
    if(detail_quality>1 && (kind==1 || kind==2) && dot(pave_slope,pave_slope)>0.0 && abs(det)>1e-12) {
        mat2 across_world=mat2(sdx,sdy)*inverse(mat2(pdx,pdy));
        vec2 g=transpose(across_world)*pave_slope*(1.0-weather.y);
        float g_length=length(g);
        if(g_length>0.5) g*=0.5/g_length;
        NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(-g.x,0.0,-g.y,0.0)).xyz);
    }
}
