shader_type spatial;
render_mode cull_disabled;
#include "wet_surface.gdshaderinc"
uniform vec4 base_color : source_color = vec4(0.19,0.20,0.23,1.0);
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
varying vec3 world_position;
varying vec3 world_normal;
void vertex() { world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz; world_normal=normalize(MODEL_NORMAL_MATRIX*NORMAL); }
float band(float distance,float width) {
    // Nearest-segment ownership changes at bends and material boundaries.
    // Bound derivative AA so these discontinuities cannot paint remote speckles.
    float aa=max(0.003,min(fwidth(distance),length(fwidth(world_position.xz))));
    return 1.0-smoothstep(width-aa,width+aa,abs(distance));
}
void fragment() {
    vec2 p=world_position.xz;
    float noise=fract(sin(dot(floor(p*150.0),vec2(12.9898,78.233)))*43758.5453);
    vec3 color=base_color.rgb*(0.96+0.08*noise);
    if (paving) {
        vec2 q=abs(fract(p/0.7)-0.5);
        float seams=max(smoothstep(0.475,0.5,q.x),smoothstep(0.475,0.5,q.y));
        color*=1.0-0.13*seams;
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
                if(abs(across)<half_width-0.25 && ((crosswalk_start && station>1.95 && station<2.08)||(crosswalk_end && e>1.95 && e<2.08))) white=1.0;
            }
        }
        color=mix(color,vec3(0.92,0.91,0.85),white);
        color=mix(color,vec3(0.93,0.65,0.12),yellow);
    }
    vec2 weather = environment_enabled ? texelFetch(environment_data,ivec2(0,0),0).xy : vec2(0.0);
    color *= 1.0-weather.x*0.22;
    color = mix(color,vec3(0.68,0.76,0.79),weather.y*0.90);
    ALBEDO=color;
    ROUGHNESS=mix(mix(0.9,0.27,weather.x),0.95,weather.y);
    float puddle=puddle_mask(world_position,world_normal.y,weather.x,weather.y);
    ALBEDO*=1.0-puddle*0.10;
    ROUGHNESS=mix(ROUGHNESS,0.18,puddle);
    SPECULAR=mix(0.35,0.55,puddle);
    NORMAL=normalize(NORMAL+(VIEW_MATRIX*vec4(puddle_ripple(world_position,TIME,puddle),0.0)).xyz);
}
