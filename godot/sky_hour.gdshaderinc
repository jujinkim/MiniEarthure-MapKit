// Golden-hour layer for shaders that mirror the procedural sky (glass
// reflection). Constants mirror sky_hour.gd exactly; keep both in step
// (sky_hour_validator parses this file and compares the tables).
const float SKY_HOUR_ZENITH_KEYS[4] = { 0.0, 2.0, 6.0, 12.0 };
const vec3 SKY_HOUR_ZENITH[4] = { vec3(0.15,0.19,0.24), vec3(0.20,0.25,0.31), vec3(0.12,0.255,0.41), vec3(0.16,0.28,0.40) };
const float SKY_HOUR_HORIZON_KEYS[4] = { 0.0, 2.0, 6.0, 12.0 };
const vec3 SKY_HOUR_HORIZON[4] = { vec3(0.46,0.27,0.18), vec3(0.52,0.31,0.19), vec3(0.62,0.43,0.28), vec3(0.58,0.57,0.51) };
const float SKY_HOUR_DUSK_KEYS[5] = { -6.0, 0.0, 2.0, 6.0, 12.0 };
const vec3 SKY_HOUR_DUSK[5] = { vec3(0.48,0.21,0.055), vec3(0.60,0.20,0.04), vec3(0.62,0.22,0.045), vec3(0.56,0.22,0.05), vec3(0.48,0.21,0.055) };
const float SKY_HOUR_HALO_KEYS[3] = { 0.0, 6.0, 12.0 };
const vec3 SKY_HOUR_HALO[3] = { vec3(0.50,0.22,0.06), vec3(0.46,0.24,0.08), vec3(0.38,0.25,0.12) };
const float SKY_HOUR_CLOUD_LIT_KEYS[4] = { 0.0, 2.0, 6.0, 12.0 };
const vec3 SKY_HOUR_CLOUD_LIT[4] = { vec3(0.44,0.28,0.20), vec3(0.46,0.30,0.21), vec3(0.40,0.31,0.27), vec3(0.27,0.30,0.34) };
// Today's daylight sky constants (atmosphere_sky.gdshader uniform defaults).
const vec3 SKY_HOUR_DAY_ZENITH = vec3(0.16,0.28,0.40);
const vec3 SKY_HOUR_DAY_HORIZON = vec3(0.58,0.57,0.51);
const vec3 SKY_HOUR_CLOUD_DAY = vec3(0.27,0.30,0.34);

// Plateau of 1 from 0° to 6°, 0 from 24° up, below -6° and from cloudy cover up.
float sky_hour_golden(float deg, float cover) {
    return (1.0-smoothstep(6.0,24.0,deg))*smoothstep(-6.0,0.0,deg)*(1.0-smoothstep(0.45,0.80,cover));
}
// Clamped piecewise-linear lookups (one per table used here).
vec3 sky_hour_zenith(float deg) {
    vec3 c = SKY_HOUR_ZENITH[0];
    for(int i=1;i<4;i++) c = mix(c,SKY_HOUR_ZENITH[i],clamp((deg-SKY_HOUR_ZENITH_KEYS[i-1])/(SKY_HOUR_ZENITH_KEYS[i]-SKY_HOUR_ZENITH_KEYS[i-1]),0.0,1.0));
    return c;
}
vec3 sky_hour_horizon(float deg) {
    vec3 c = SKY_HOUR_HORIZON[0];
    for(int i=1;i<4;i++) c = mix(c,SKY_HOUR_HORIZON[i],clamp((deg-SKY_HOUR_HORIZON_KEYS[i-1])/(SKY_HOUR_HORIZON_KEYS[i]-SKY_HOUR_HORIZON_KEYS[i-1]),0.0,1.0));
    return c;
}
vec3 sky_hour_cloud_lit(float deg) {
    vec3 c = SKY_HOUR_CLOUD_LIT[0];
    for(int i=1;i<4;i++) c = mix(c,SKY_HOUR_CLOUD_LIT[i],clamp((deg-SKY_HOUR_CLOUD_LIT_KEYS[i-1])/(SKY_HOUR_CLOUD_LIT_KEYS[i]-SKY_HOUR_CLOUD_LIT_KEYS[i-1]),0.0,1.0));
    return c;
}
