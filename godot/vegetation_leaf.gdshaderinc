// Display-only park vegetation (vegetation_meshes.gd), shared by three
// variants: vegetation_leaf.gdshader (the opaque body: trunks, branches and the
// crown hull), vegetation_leaf_cards.gdshader (alpha-scissor LEAF_CARDS) and
// vegetation_grass.gdshader (GRASS tufts). Separate preloaded shaders compile
// when the renderer scripts load, never in a streaming step.
#if defined(LEAF_CARDS) || defined(GRASS)
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
// Sway of a weight-1 vertex (canopy top, blade tip) in metres at full strength.
uniform float sway_metres = 0.07;
// Leaf colour (linear) of a placement without instance data (single-node path).
uniform vec3 default_tint = vec3(0.105,0.185,0.075);
// Grass tufts shrink into the ground between these camera distances (metres).
uniform float grass_fade_start = 30.0;
uniform float grass_fade_end = 40.0;
// Low quality draws no leaf cards: the body's near hull (UV.y > 0, the dark
// gap colour under the cards) grows back to the full crown around (0, UV.x, 0)
// and takes the far hull's brightness.
uniform bool cards_hidden = false;
varying vec3 tint;
varying vec3 world_position;
varying float flower_kind;

// Same sine hash and value noise as atmosphere_surface (facade_hash,
// lawn_noise): tufts follow the lawn's lush and dry patches.
float vegetation_hash(vec2 v) { return fract(sin(dot(v,vec2(12.9898,78.233)))*43758.5453); }
float lawn_noise(vec2 p) {
    vec2 i=floor(p); vec2 f=fract(p); f=f*f*(3.0-2.0*f);
    float a=vegetation_hash(i), b=vegetation_hash(i+vec2(1.0,0.0)), c=vegetation_hash(i+vec2(0.0,1.0)), d=vegetation_hash(i+vec2(1.0,1.0));
    return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
}

void vertex() {
    // INSTANCE_CUSTOM: rgb leaf/grass tint (linear), a = 1 + kind + phase where
    // kind is a grass flower colour (0 none) and phase in [0,1). Zero = no data.
    bool has_custom = INSTANCE_CUSTOM.a >= 1.0;
    float phase = has_custom ? fract(INSTANCE_CUSTOM.a)*6.2831853 : 0.0;
    tint = has_custom ? INSTANCE_CUSTOM.rgb : default_tint;
    flower_kind = has_custom ? floor(INSTANCE_CUSTOM.a)-1.0 : 0.0;
    vec3 origin = MODEL_MATRIX[3].xyz;
    mat3 basis = mat3(MODEL_MATRIX);
    float scale2 = max(dot(basis[0],basis[0]),1e-6);
#ifdef GRASS
    // Flower heads (UV2.y < 0) collapse unless the instance carries a flower.
    if(UV2.y < -0.5 && flower_kind < 0.5) VERTEX = vec3(0.0);
    // Shrink toward the instance origin with distance: no pop at the range end.
    float keep = 1.0-smoothstep(grass_fade_start,grass_fade_end,distance(origin,INV_VIEW_MATRIX[3].xyz));
    VERTEX *= keep;
#endif
#if !defined(LEAF_CARDS) && !defined(GRASS)
    if(cards_hidden && UV.y > 0.0) VERTEX = vec3(0.0,UV.x,0.0)+(VERTEX-vec3(0.0,UV.x,0.0))*UV.y;
#endif
    if(wind_strength > 0.0) {
        // Coherent gusts along one world direction, offset per placement;
        // UV2.x is the sway weight measured from the canopy base (0 on the
        // trunk), UV2.y a flutter weight for cards and blade tips.
        float t = TIME;
        float gust = 0.55+0.45*sin(t*0.37+dot(origin.xz,vec2(0.021,0.013)));
        float bend = (sin(t*1.25+phase+dot(origin.xz,vec2(0.09,0.05)))*0.6+sin(t*2.3+phase*1.7)*0.25+0.35)*gust;
        vec3 offset = normalize(vec3(1.0,0.0,0.35))*bend*UV2.x*sway_metres;
#ifdef LEAF_CARDS
        // A card flutters as one piece (UV2.y = 1 + its phase): no stretching.
        float card = fract(UV2.y)*6.2831853+phase;
        offset += vec3(sin(t*2.1+card),0.4*sin(t*1.7+card*1.3),cos(t*2.4+card))*0.008*UV2.x;
#elif defined(GRASS)
        float flutter = max(UV2.y,0.0);
        offset += vec3(sin(t*3.1+VERTEX.x*3.1+phase),0.0,cos(t*3.6+VERTEX.z*2.9+phase))*flutter*0.012;
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
#if !defined(LEAF_CARDS) && !defined(GRASS)
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
#ifdef GRASS
    // Same broad lush/dry patches as the lawn shader under the tuft.
    vec2 g = world_position.xz;
    float broad = sin(g.x*0.055+sin(g.y*0.037))*cos(g.y*0.063);
    float patches = lawn_noise(g*0.09)*0.65+lawn_noise(g*0.23+7.3)*0.35;
    leaf_color *= (0.96+0.06*broad)*mix(vec3(0.86,0.90,0.84),vec3(1.10,1.08,1.02),patches);
    float dry = smoothstep(0.60,0.82,lawn_noise(g*0.05+19.1));
    leaf_color = mix(leaf_color,leaf_color*vec3(1.16,1.08,0.78),dry*0.55);
    // Petals (COLOR.a 0) take the flower colour of the instance.
    vec3 petals = flower_kind < 1.5 ? vec3(0.86,0.85,0.80) : (flower_kind < 2.5 ? vec3(0.95,0.72,0.10) : vec3(0.52,0.36,0.78));
    if(UV2.y < -0.5) albedo *= petals;
#endif
#if defined(LEAF_CARDS) || defined(GRASS)
    // Two-sided cards and blades keep their bent (outward) normal on both sides.
    if(!FRONT_FACING) { NORMAL = -NORMAL; world_normal = -world_normal; }
#endif
    float sky = 0.0;
#ifdef LEAF_CARDS
    // Sky-facing cards at the crown top already catch the full sun: keep their
    // yellowing and back-light low so tops are not washed out.
    sky = smoothstep(0.3,0.9,world_normal.y);
    yellowing *= 1.0-0.7*sky;
#elif !defined(GRASS)
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
