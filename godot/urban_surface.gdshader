shader_type spatial;
render_mode cull_disabled;
uniform vec4 base_color : source_color = vec4(0.19,0.20,0.23,1.0);
uniform bool paving = false;
uniform bool marked = false;
uniform vec2 road_start;
uniform vec2 road_end;
uniform float road_width = 5.0;
uniform int lanes = 2;
uniform bool center_line = true;
uniform bool edge_lines = true;
uniform bool crosswalk_start = false;
uniform bool crosswalk_end = false;
uniform float start_gap = 0.0;
uniform float end_gap = 0.0;
varying vec3 world_position;
void vertex() { world_position = (MODEL_MATRIX * vec4(VERTEX,1.0)).xyz; }
float band(float distance,float width) {
    float aa=max(fwidth(distance),0.003);
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
        vec2 delta=road_end-road_start;
        float len=length(delta);
        vec2 direction=delta/max(len,0.001);
        vec2 relative=p-road_start;
        float along=dot(relative,direction);
        float across=dot(relative,vec2(-direction.y,direction.x));
        float half_width=road_width*0.5;
        bool body=along>start_gap && along<len-end_gap;
        float white=0.0;
        float yellow=0.0;
        if(body) {
            if(edge_lines) white=max(white,band(abs(across)-(half_width-0.16),0.04));
            for(int lane=1;lane<8;lane++) {
                if(lane>=lanes) break;
                float x=-half_width+road_width*float(lane)/float(lanes);
                if(center_line && lane*2==lanes) yellow=max(yellow,band(abs(across)-0.07,0.025));
                else if(fract(along/2.4)<0.56) white=max(white,band(across-x,0.035));
            }
            if(len>start_gap+end_gap+5.0) {
                float s=along-start_gap;
                float e=len-end_gap-along;
                bool crossing=(crosswalk_start && s>0.3 && s<1.7)||(crosswalk_end && e>0.3 && e<1.7);
                if(crossing && abs(across)<half_width-0.25) {
                    white=step(0.43,fract((across+half_width)/0.65)); yellow=0.0;
                }
                if(abs(across)<half_width-0.25 && ((crosswalk_start && s>1.95 && s<2.08)||(crosswalk_end && e>1.95 && e<2.08))) white=1.0;
            }
        }
        color=mix(color,vec3(0.92,0.91,0.85),white);
        color=mix(color,vec3(0.93,0.65,0.12),yellow);
    }
    ALBEDO=color;
    ROUGHNESS=0.9;
}
