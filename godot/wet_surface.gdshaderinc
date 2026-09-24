float puddle_mask(vec3 p, float up, float wet, float snow) {
    // Continuous world-space patches; no camera-anchored or cell-boundary grid.
    float broad=sin(p.x*0.67+sin(p.z*0.38))*sin(p.z*0.91+cos(p.x*0.29));
    float detail=sin(p.x*2.3+p.z*1.73)*0.13;
    return smoothstep(0.14,0.42,broad+detail)*smoothstep(0.2,0.85,wet)
        *smoothstep(0.94,0.995,up)*(1.0-snow);
}
vec3 puddle_ripple(vec3 p, float time, float amount) {
    return vec3(cos(p.x*31.0+p.z*17.0-time*9.0),0.0,
        sin(p.x*19.0-p.z*29.0+time*11.0))*amount*0.025;
}
