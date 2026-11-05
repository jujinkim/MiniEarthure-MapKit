shader_type spatial;
render_mode unshaded, blend_add, depth_draw_never, cull_disabled, skip_vertex_transform, shadows_disabled, fog_disabled;
// Night glow of city streetlight heads (city_lamp_meshes.gd halo mesh): one
// camera-facing quad per head (VERTEX.xy its corner, VERTEX.z the head side,
// UV.y the part: the twin side collapses on single lamps), centred just
// under the lens at local (side x reach, 3.09, 0) and pulled 0.2 m toward
// the camera so the housing does not cut it. Its world size grows with
// distance from HALO_SIZE (keeping far heads at about two pixels) while
// its strength falls, so rows of heads stay steady points instead of
// sparkling. Additive, depth-tested, never written to depth; by day every
// vertex collapses (no fragments). INSTANCE_CUSTOM as the lamp body.
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;
uniform vec4 led_color : source_color = vec4(1.0, 0.929, 0.839, 1.0);
uniform float halo_size = 0.18;
uniform float halo_strength = 0.5;
varying vec2 corner;
varying float strength;
void vertex() {
    vec4 clock = environment_enabled ? texelFetch(environment_data,ivec2(1,0),0) : vec4(43200.0,0.0,1.0,64800.0);
    uint flags = uint(round(INSTANCE_CUSTOM.y));
    bool twin_side = round(UV.y) > 0.5;
    corner = VERTEX.xy;
    strength = 0.0;
    if(clock.z >= 0.0 || (twin_side && (flags & 1u) == 0u)) {
        VERTEX = vec3(0.0);
    } else {
        vec3 centre = vec3(VERTEX.z * INSTANCE_CUSTOM.x * 0.01, 3.09, 0.0);
        vec3 view = (MODELVIEW_MATRIX * vec4(centre,1.0)).xyz;
        float dist = max(length(view),0.01);
        float size = max(halo_size,dist*0.0045);
        strength = halo_strength*(halo_size/size);
        view += -view/dist*min(0.2,dist*0.5);
        VERTEX = view+vec3(corner*size,0.0);
    }
    NORMAL = vec3(0.0,0.0,1.0);
}
void fragment() {
    float r2 = dot(corner,corner);
    // A soft bloom falling smoothly to zero at the quad's edge (no visible
    // rim after the sRGB conversion) round a tight core.
    float fall = max(0.0,1.0-r2);
    float fall2 = fall*fall;
    float glow = strength*(0.55*fall2*fall2*fall2+0.45*exp(-16.0*r2)*fall);
    ALBEDO = led_color.rgb*glow;
    ALPHA = 1.0;
}
