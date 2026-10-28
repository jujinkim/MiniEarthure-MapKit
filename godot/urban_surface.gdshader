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
float arrow(float a,float v,bool straight,bool left,bool right) {
    // a: metres back from the arrow tip toward the stop line's far side; v: lane offset (driver's right +).
    float paint=0.0;
    if(straight) {
        if(a>0.35 && a<1.3 && abs(v)<0.04) paint=1.0;
        if(a>=0.0 && a<=0.35 && abs(v)<0.16*(a/0.35)) paint=1.0;
    }
    for(int side=0;side<2;side++) {
        bool turn=side==0 ? left : right;
        if(!turn) continue;
        float sd=side==0 ? -1.0 : 1.0;
        float lateral=sd*v;
        if(a>0.55 && a<1.3 && abs(v)<0.04) paint=1.0;
        if(a>0.5 && a<0.6 && lateral>-0.04 && lateral<0.26) paint=1.0;
        float t=(lateral-0.26)/0.14;
        if(t>=0.0 && t<=1.0 && abs(a-0.55)<0.15*(1.0-t)) paint=1.0;
    }
    return paint;
}
float band(float distance,float width) {
    // Nearest-segment ownership changes at bends and material boundaries.
    // Bound derivative AA so these discontinuities cannot paint remote speckles.
    float aa=max(0.003,min(fwidth(distance),length(fwidth(world_position.xz))));
    return 1.0-smoothstep(width-aa,width+aa,abs(distance));
}
void fragment() {
    vec2 p=world_position.xz;
    float effect_weight=surface_effect_weight(length(VERTEX));
    vec3 color=base_color.rgb;
    float view_distance=length(VERTEX);
    // Small features fade before they could shimmer between pixels.
    float close=effect_weight*(1.0-smoothstep(6.0,24.0,view_distance));
    float wear=0.5;
    float rough_shift=0.0;
    if(effect_weight>0.0) {
        float broad=value_noise(p*0.18)*0.6+value_noise(p*0.55+vec2(11.0,4.0))*0.4;
        float fine=value_noise(p*6.0);
        wear=value_noise(p*2.3+vec2(17.0,3.0));
        if(!paving) {
            // Asphalt: broad tonal blotches, light aggregate, sealed repair
            // patches and thin cracks. Roughness follows the same wear.
            color*=1.0+((broad-0.5)*0.18+(fine-0.5)*0.06)*effect_weight;
            float stone=smoothstep(0.80,0.92,value_noise(p*26.0+vec2(fine*3.0)));
            color=mix(color,color*1.35+0.02,stone*0.35*close);
            vec2 slot=floor(p/3.5);
            vec2 local=fract(p/3.5);
            vec2 extent=0.2+0.5*vec2(urban_hash(slot+1.3),urban_hash(slot+2.7));
            float repaired=step(0.86,urban_hash(slot+vec2(5.0,9.0)))*step(abs(local.x-0.5),extent.x*0.5)*step(abs(local.y-0.5),extent.y*0.5);
            color*=1.0-0.13*repaired*effect_weight;
            // Domain-warped ridges keep cracks jagged and sparse.
            vec2 warped=p*1.3+0.35*vec2(value_noise(p*5.0),value_noise(p*5.0+vec2(5.0,1.0)));
            float ridge=abs(value_noise(warped+vec2(3.0,7.0))-0.5);
            float crack=(1.0-smoothstep(0.0,0.005+fwidth(ridge),ridge))*smoothstep(0.68,0.78,value_noise(p*0.3+vec2(1.0,9.0)));
            color*=1.0-0.28*crack*close;
            rough_shift=(broad-0.5)*0.08-0.05*repaired;
        } else {
            // Paving: per-slab tone and the same broad weathering.
            color*=1.0+((urban_hash(floor(p/0.7))-0.5)*0.08+(broad-0.5)*0.12)*effect_weight;
        }
    }
    float tile_roughness=0.9;
    if(detail_enabled && length(VERTEX)<48.0 && effect_weight>0.0) {
        vec4 detail=texture(detail_tile,detail_uv(world_position,world_normal));
        float relief=material_detail_weight(length(VERTEX))*0.9;
        color*=mix(1.0,detail_luminance(detail.r),relief);
        if(detail_quality>0) tile_roughness=mix(0.9,detail.g,relief);
        if(detail_quality>1) NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(detail_bump(world_position,world_normal)*relief,0.0)).xyz);
        if(detail_quality>0) AO=mix(1.0,detail.r/0.65,0.18*relief);
    }
    if (paving && effect_weight>0.0) {
        vec2 q=abs(fract(p/0.7)-0.5);
        float seams=max(smoothstep(0.475,0.5,q.x),smoothstep(0.475,0.5,q.y));
        color*=1.0-0.13*seams*effect_weight;
    }
    if (marked) {
        float nearest=1e20;
        float across=0.0;
        float station=0.0;
        float half_width=0.0;
        float period=2.4;
        float total=0.0;
        bool body=false;
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
                body=(t>=-0.005 && t<=len+0.005) || (station>0.005 && station<total-0.005);
            }
        }
        float white=0.0;
        float yellow=0.0;
        if(edge_lines) {
            for(int i=0;i<128;i++) {
                if(i>=edge_count) break;
                vec2 a=road_borders[i].xy;
                vec2 delta=road_borders[i].zw-a;
                float len=length(delta);
                vec2 dir=delta/max(len,0.001);
                a+=vec2(dir.y,-dir.x)*0.16;
                float t=clamp(dot(p-a,dir),0.0,len);
                white=max(white,band(length(p-a-dir*t),0.04));
            }
        }
        if(body) {
            for(int lane=1;lane<8;lane++) {
                if(lane>=lanes) break;
                float x=-half_width+2.0*half_width*float(lane)/float(lanes);
                if(center_line && lane*2==lanes) yellow=max(yellow,band(abs(across)-0.07,0.025));
                else if(abs(fract(station/max(period,0.001)+0.5)-0.5)<0.28) white=max(white,band(across-x,0.035));
            }
            if(total>5.0) {
                float e=total-station;
                bool crossing=(crosswalk_start && station>0.3 && station<1.7)||(crosswalk_end && e>0.3 && e<1.7);
                if(crossing && abs(across)<half_width-0.25) {
                    white=step(0.43,fract((across+half_width)/0.65)); yellow=0.0;
                }
                if(abs(across)<half_width-0.25 && ((crosswalk_start && stop_start==0 && station>1.95 && station<2.08)||(crosswalk_end && stop_end==0 && e>1.95 && e<2.08))) white=1.0;
                for(int s=0;s<2;s++) {
                    int mask=s==0 ? stop_start : stop_end;
                    float h=s==0 ? -1.0 : 1.0;
                    if(mask==0 || (one_way!=0 && float(one_way)!=h)) continue;
                    float to_end=s==0 ? station : e;
                    float bar=(s==0 ? crosswalk_start : crosswalk_end) ? 1.95 : 0.55;
                    float r=h*across; // across > 0 is right of the path direction in scene xz
                    float r0=one_way!=0 || left_hand ? -half_width : 0.0;
                    float r1=one_way!=0 || !left_hand ? half_width : 0.0;
                    int count=one_way!=0 ? lanes : max(lanes/2,1);
                    if(r<=r0+0.05 || r>=r1-0.05) continue;
                    if(to_end>bar && to_end<bar+0.14) white=1.0;
                    float w=(r1-r0)/float(count);
                    int k=int(clamp(floor((r-r0)/w),0.0,float(count-1)));
                    float v=r-(r0+(float(k)+0.5)*w);
                    bool turn_left=(mask&1)!=0 && k==0;
                    bool turn_right=(mask&4)!=0 && k==count-1;
                    bool straight=(mask&2)!=0 && (count<=2 || (k>0 && k<count-1));
                    if(count==1) { turn_left=(mask&1)!=0; turn_right=(mask&4)!=0; straight=(mask&2)!=0; }
                    white=max(white,arrow(to_end-(bar+0.6),v,straight,turn_left,turn_right));
                }
            }
        }
        // Worn paint: uneven coverage, scuffed by wheel tracks.
        float track=0.0;
        if(body && effect_weight>0.0) {
            float lane_width=2.0*half_width/float(max(lanes,1));
            float lateral=abs(fract((across+half_width)/max(lane_width,0.01))-0.5);
            track=(1.0-smoothstep(0.04,0.10,abs(lateral-0.24)))*step(abs(across),half_width);
            color*=1.0-0.07*track*effect_weight;
            rough_shift-=0.10*track*effect_weight;
        }
        float coverage=mix(1.0,clamp(0.62+0.5*smoothstep(0.2,0.65,wear)-0.3*track,0.35,1.0),effect_weight);
        color=mix(color,vec3(0.86,0.86,0.81)*(0.96+0.06*wear),white*coverage);
        color=mix(color,vec3(0.85,0.64,0.21)*(0.96+0.06*wear),yellow*coverage);
        rough_shift-=0.25*max(white,yellow)*coverage;
    }
    vec2 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0).xy : vec2(0.0);
    weather.y *= snow_retention_percent * 0.01;
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*0.90);
    ALBEDO=color;
    ROUGHNESS=mix(mix(clamp(tile_roughness+rough_shift,0.4,1.0),0.27,weather.x),0.95,weather.y);
    float puddle=0.0;
    if(effect_weight>0.0 && weather.x>0.2) puddle=puddle_mask(world_position,world_normal.y,weather.x,weather.y)*effect_weight;
    ALBEDO*=1.0-puddle*0.10;
    ROUGHNESS=mix(ROUGHNESS,0.22,puddle);
    SPECULAR=mix(0.35,0.50,puddle);
    if(puddle>0.0) NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
}
