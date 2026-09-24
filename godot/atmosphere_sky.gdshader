shader_type sky;
uniform vec3 sun_direction = vec3(0.0,1.0,0.0);
uniform vec3 moon_direction = vec3(0.0,-1.0,0.0);
uniform float daylight = 1.0;
uniform float moon_phase = 1.0;
uniform float cloud_cover = 0.15;
uniform float cloud_motion = 0.0;
uniform float aurora = 0.0;
float hash(vec2 p) { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float noise(vec2 p) {
    vec2 f=fract(p); f=f*f*(3.0-2.0*f); vec2 i=floor(p);
    return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x),f.y);
}
float dome_noise(vec3 p) {
    vec3 i=floor(p); vec3 f=fract(p); f=f*f*(3.0-2.0*f);
    vec2 a=i.xz+vec2(37.0,17.0)*i.y;
    vec2 b=a+vec2(37.0,17.0);
    float low=mix(mix(hash(a),hash(a+vec2(1,0)),f.x),mix(hash(a+vec2(0,1)),hash(a+vec2(1,1)),f.x),f.z);
    float high=mix(mix(hash(b),hash(b+vec2(1,0)),f.x),mix(hash(b+vec2(0,1)),hash(b+vec2(1,1)),f.x),f.z);
    return mix(low,high,f.y);
}
void sky() {
    float horizon=pow(1.0-max(0.0,EYEDIR.y),4.0);
    vec3 day=mix(vec3(0.17,0.29,0.40),vec3(0.50,0.53,0.53),horizon);
    vec3 night=mix(vec3(0.001,0.003,0.008),vec3(0.007,0.012,0.025),horizon);
    float dusk=(1.0-smoothstep(0.0,0.28,abs(sun_direction.y)))*horizon;
    vec3 color=mix(night,day,daylight);
    color+=vec3(0.36,0.11,0.035)*dusk*max(0.0,dot(normalize(vec3(EYEDIR.x,0.0,EYEDIR.z)),normalize(vec3(sun_direction.x,0.001,sun_direction.z))));
    float sun=smoothstep(0.9997,0.99985,dot(EYEDIR,sun_direction));
    float moon=smoothstep(0.9995,0.9997,dot(EYEDIR,moon_direction));
    vec3 moon_right=normalize(cross(moon_direction,abs(moon_direction.y)>0.99?vec3(0.0,0.0,1.0):vec3(0.0,1.0,0.0)));
    vec3 moon_up=normalize(cross(moon_right,moon_direction));
    vec2 disc=vec2(dot(EYEDIR,moon_right),dot(EYEDIR,moon_up))/0.027;
    float depth=sqrt(max(0.0,1.0-dot(disc,disc)));
    vec3 normal=disc.x*moon_right+disc.y*moon_up-depth*moon_direction;
    float lit=smoothstep(-0.025,0.025,dot(normal,sun_direction));
    color+=vec3(2.3,1.85,1.25)*sun+vec3(0.45,0.52,0.64)*moon*lit*(1.0-daylight);
    vec2 stars=floor(SKY_COORDS*vec2(1700.0,850.0));
    color+=vec3(step(0.9985,hash(stars)))*0.45*(1.0-daylight);
    // Sample a curved dome instead of a clamped plane. No horizon projection
    // discontinuity; rotated octaves and broad filtered edges suppress grids.
    vec3 p=normalize(EYEDIR+vec3(0.0,0.35,0.0))*4.2+vec3(cloud_motion,0.0,cloud_motion*0.19);
    float n=dome_noise(p)*0.64+dome_noise(p.zxy*2.03+vec3(7.1,13.7,2.5))*0.26+dome_noise(p.yzx*3.97)*0.1;
    float edge=0.10+0.06*horizon;
    float clouds=smoothstep(1.0-cloud_cover-edge,1.0-cloud_cover+edge,n)*smoothstep(-0.04,0.12,EYEDIR.y);
    float curtain=exp(-pow((EYEDIR.y-(0.3+0.05*sin(EYEDIR.x*14.0+cloud_motion*0.3)))*12.0,2.0));
    color+=aurora*curtain*(0.45+0.55*sin(EYEDIR.z*45.0+cloud_motion))*vec3(0.04,0.35,0.2)*(1.0-daylight);
    color*=1.0-cloud_cover*0.20;
    color=mix(color,mix(vec3(0.008,0.012,0.019),vec3(0.27,0.30,0.34),daylight),clouds);
    if(EYEDIR.y<0.0) color=mix(vec3(0.002),vec3(0.08,0.10,0.08),daylight);
    COLOR=color;
}
