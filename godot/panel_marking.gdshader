shader_type spatial;
render_mode cull_disabled;
uniform vec4 panel_color : source_color = vec4(1.0, 0.44, 0.14, 1.0);
uniform vec4 panel_rect; // Local X/Z footprint, shared across all clipped parts.
uniform bool jump_panel = false;
varying vec2 panel_uv;
varying float top_face;
void vertex() {
    panel_uv = (VERTEX.xz - panel_rect.xy) / max(panel_rect.zw, vec2(0.001));
    top_face = step(0.4, NORMAL.y);
}
void fragment() {
    vec2 p = vec2(panel_uv.x, 1.0-panel_uv.y);
    float x = abs(p.x-0.5);
    float mark;
    if (jump_panel) {
        // Broad upward arrow rising from two launch bars.
        float head = step(abs(p.y-(0.76-x*0.85)), 0.055)*step(x,0.34);
        float stem = step(x,0.055)*step(0.28,p.y)*step(p.y,0.72);
        float bars = (step(abs(p.y-0.16),0.025)+step(abs(p.y-0.24),0.025))*step(x,0.30);
        mark = min(1.0,head+stem+bars);
    } else {
        // Two open forward chevrons; dimensions follow the whole panel.
        mark = min(1.0,step(abs(p.y-(0.43-x*0.55)),0.055)+step(abs(p.y-(0.76-x*0.55)),0.055))*step(x,0.37);
    }
    float border = step(min(min(p.x,1.0-p.x),min(p.y,1.0-p.y)),0.035);
    float ink = max(mark,border)*top_face;
    ALBEDO = mix(panel_color.rgb*0.12,panel_color.rgb,ink);
    EMISSION = panel_color.rgb*ink*0.35;
    ROUGHNESS = 0.72;
}
