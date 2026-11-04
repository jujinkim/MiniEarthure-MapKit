// Display-only park vegetation (vegetation_meshes.gd), shared by two
// variants: vegetation_leaf.gdshader (the opaque body: trunks, branches and the
// crown hull) and vegetation_leaf_cards.gdshader (alpha-scissor LEAF_CARDS).
// Separate preloaded shaders compile when the renderer scripts load, never in
// a streaming step. (Grass is the grass field's own vegetation_grass.gdshader.)
#ifdef LEAF_CARDS
render_mode cull_disabled;
#else
render_mode cull_back;
#endif
// Packed leaf atlas (scripts/richer_assets.py --leaves): R luminance (mean
// 0.65), G yellowing, A coverage. Data, not colour.
uniform sampler2D leaf_atlas : hint_default_white, filter_linear_mipmap, repeat_disable;
// The same atlas, repeating: leafy pattern projected on the body's leaf lobes.
uniform sampler2D leaf_pattern : hint_default_white, filter_linear_mipmap, repeat_enable;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;
// Display quality vegetation_motion (0 on low).
uniform float wind_strength = 0.5;
// Sway of a weight-1 vertex (canopy top) in metres at full strength.
uniform float sway_metres = 0.07;
// Leaf colour (linear) of a placement without instance data (single-node path).
uniform vec3 default_tint = vec3(0.105,0.185,0.075);
// Low quality draws no leaf cards: the body's near hull (UV.y > 0, the dark
// gap colour under the cards) grows back to the full crown around (0, UV.x, 0)
// and takes the far hull's brightness.
uniform bool cards_hidden = false;
varying vec3 tint;
varying vec3 world_position;

void vertex() {
    // INSTANCE_CUSTOM: rgb leaf tint (linear), a = 1 + phase with phase in
    // [0,1). Zero = no data.
    bool has_custom = INSTANCE_CUSTOM.a >= 1.0;
    float phase = has_custom ? fract(INSTANCE_CUSTOM.a)*6.2831853 : 0.0;
    tint = has_custom ? INSTANCE_CUSTOM.rgb : default_tint;
    vec3 origin = MODEL_MATRIX[3].xyz;
    mat3 basis = mat3(MODEL_MATRIX);
    float scale2 = max(dot(basis[0],basis[0]),1e-6);
#ifndef LEAF_CARDS
    if(cards_hidden && UV.y > 0.0) VERTEX = vec3(0.0,UV.x,0.0)+(VERTEX-vec3(0.0,UV.x,0.0))*UV.y;
#endif
    if(wind_strength > 0.0) {
        // Coherent gusts along one world direction, offset per placement;
        // UV2.x is the sway weight measured from the canopy base (0 on the
        // trunk), UV2.y a flutter weight for cards.
        float t = TIME;
        float gust = 0.55+0.45*sin(t*0.37+dot(origin.xz,vec2(0.021,0.013)));
        float bend = (sin(t*1.25+phase+dot(origin.xz,vec2(0.09,0.05)))*0.6+sin(t*2.3+phase*1.7)*0.25+0.35)*gust;
        vec3 offset = normalize(vec3(1.0,0.0,0.35))*bend*UV2.x*sway_metres;
#ifdef LEAF_CARDS
        // A card flutters as one piece (UV2.y = 1 + its phase): no stretching.
        float card = fract(UV2.y)*6.2831853+phase;
        offset += vec3(sin(t*2.1+card),0.4*sin(t*1.7+card*1.3),cos(t*2.4+card))*0.008*UV2.x;
#endif
        // World offset to model space (rotation and uniform scale only).
        VERTEX += transpose(basis)*(offset*wind_strength)/scale2;
    }
    world_position = (MODEL_MATRIX*vec4(VERTEX,1.0)).xyz;
}

void fragment() {
    vec3 albedo = COLOR.rgb;
    float leaf = COLOR.a;
    float yellowing = 0.0;
#ifdef LEAF_CARDS
    vec4 texel = texture(leaf_atlas,UV);
    // Coverage-preserving alpha: box-filtered mipmaps thin the clusters, so
    // raise alpha with the mip level and the cards keep their density far away.
    vec2 texels = UV*vec2(textureSize(leaf_atlas,0));
    float lod = max(0.0,0.5*log2(max(dot(dFdx(texels),dFdx(texels)),dot(dFdy(texels),dFdy(texels)))));
    ALPHA = clamp(texel.a*(1.0+0.3*lod),0.0,1.0);
    // Cards seen edge-on thin out instead of drawing streaks at the silhouette.
    vec3 face = normalize(cross(dFdx(world_position),dFdy(world_position)));
    ALPHA *= smoothstep(0.2,0.45,abs(dot(face,normalize(world_position-INV_VIEW_MATRIX[3].xyz))));
    ALPHA_SCISSOR_THRESHOLD = 0.5;
    albedo *= clamp(texel.r/0.65,0.6,1.35);
    yellowing = texel.g;
#endif
    vec3 world_normal = normalize((INV_VIEW_MATRIX*vec4(NORMAL,0.0)).xyz);
#ifndef LEAF_CARDS
    // Leaf lobes: a triplanar leaf pattern breaks the facets into clusters
    // with darker gaps; it settles to its mean with the mipmaps far away.
    if(leaf > 0.5) {
        vec3 w = pow(abs(world_normal),vec3(4.0)); w /= w.x+w.y+w.z;
        vec2 px = texture(leaf_pattern,world_position.zy*0.6).ra;
        vec2 py = texture(leaf_pattern,world_position.xz*0.6+0.5).ra;
        vec2 pz = texture(leaf_pattern,world_position.xy*0.6+0.25).ra;
        vec2 pattern = px*w.x+py*w.y+pz*w.z;
        albedo *= mix(0.62,1.08,pattern.y)*clamp(pattern.x/0.65,0.8,1.2);
    }
#endif
    vec3 leaf_color = tint;
#ifdef LEAF_CARDS
    // Two-sided cards keep their bent (outward) normal on both sides.
    if(!FRONT_FACING) { NORMAL = -NORMAL; world_normal = -world_normal; }
#endif
    float sky = 0.0;
#ifdef LEAF_CARDS
    // Sky-facing cards at the crown top already catch the full sun: keep their
    // yellowing and back-light low so tops are not washed out.
    sky = smoothstep(0.3,0.9,world_normal.y);
    yellowing *= 1.0-0.7*sky;
#else
    // The low tier's grown hull is the whole crown: far-hull brightness.
    if(cards_hidden && UV.y > 0.0) albedo *= 1.4;
#endif
    leaf_color = mix(leaf_color,leaf_color*vec3(1.35,1.12,0.45),yellowing*0.45);
    albedo = mix(albedo,albedo*leaf_color,leaf);
    float roughness = mix(0.9,0.78,leaf);
    if(environment_enabled) {
        vec4 weather = texelFetch(environment_data,ivec2(0,0),0);
        albedo *= 1.0-weather.x*0.18;
        roughness = mix(roughness,0.45,weather.x*0.6);
        float cover = weather.y*smoothstep(0.25,0.85,world_normal.y);
        albedo = mix(albedo,vec3(0.68,0.76,0.79),cover*0.85);
    }
    ALBEDO = albedo;
    ROUGHNESS = roughness;
    SPECULAR = mix(0.3,0.18,leaf);
    // Thin leaves pass light: back-lit foliage glows warm green.
    BACKLIGHT = albedo*vec3(0.75,0.85,0.45)*leaf*0.35*(1.0-0.6*sky);
}
