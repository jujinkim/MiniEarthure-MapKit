shader_type spatial;
// Camera-local grass field (grass_field.gd). Every grass card is computed here
// from its tile (the MultiMesh instance's world corner on its ring's world
// grid) and its slot in the tile mesh (UV.x): integer hashes give its root,
// yaw, size and colour, so cards are anchored to the world and never swim as
// the camera moves. A card is an upright quad of `card_columns` columns, each
// holding at most one curved, tapered blade drawn analytically here (its own
// position, height, lean, width and shade from one hash of the card and
// column); pixels between blades are discarded, so a card shows a row of
// blades for four vertices. Beyond 2 m cards turn up to half way toward the
// camera, so none is seen edge-on. A toroidal array of 8 m blocks (32 x 32
// texels of 0.25 m, R32F) holds the grass ground height under each texel
// centre, or EMPTY where there is no grass ground: a card stands only where
// the bilinear share of grass texels around its root reaches one half, each
// end of it on the ground under that end, and an end over no grass folds back
// to the root, so blades never reach over roads or water. Rings hand over by
// camera distance and thin their cards beyond thin_distance toward an even
// blade cover (a per-card rank thins and grows cards), so tile snapping and
// handovers never pop; a card its rank already thins skips the field lookups.
render_mode cull_disabled;

uniform sampler2DArray field_heights : filter_nearest, repeat_disable;
// Window blocks per axis (grass_field BLOCKS).
uniform int field_blocks = 12;
uniform float tile_metres = 1.0;
uniform int ring_seed = 0;
// Card slots per tile; later slots are flowers (stem and head).
uniform int card_slots = 64;
// Camera distance fades (metres): grow in over x..y (x < 0: none), thin out over z..w.
uniform vec4 fade = vec4(-1.0, 0.0, 5.5, 8.0);
// Camera distance from which cards thin as thin_distance / distance.
uniform float thin_distance = 1000.0;
// Card width (metres), blade columns per card, blade root half width (in
// columns) and the share of columns without a blade.
uniform float card_width = 0.24;
uniform int card_columns = 12;
uniform float blade_fill = 0.32;
uniform float card_gap = 0.15;
// Card height range (metres): its tallest blade, before clumps.
uniform vec2 blade_height = vec2(0.12, 0.15);
// Flower probability of a flower slot inside a flower patch / elsewhere.
uniform vec2 flower_share = vec2(0.35, 0.012);
// Linear lawn colour of the cells (chunk_renderer ground_color, greener).
uniform vec3 grass_tint = vec3(0.13,0.20,0.08);
// Display quality vegetation_motion.
uniform float wind_strength = 0.5;
uniform sampler2D environment_data : filter_nearest, repeat_disable;
uniform bool environment_enabled = false;

const float EMPTY_LIMIT = -1.0e5;

varying vec3 blade_color;
// Card: (column coordinate 0..card_columns, height share 0..1); flowers: x < 0.
varying vec2 card_uv;
varying flat uint card_seed;
varying float blade_t;
varying float petal;

// grass_field / vegetation_meshes.hash32: 32-bit integer hash.
uint hash32(uint v) {
    v = ((v >> 16u) ^ v) * 73244475u;
    v = ((v >> 16u) ^ v) * 73244475u;
    return (v >> 16u) ^ v;
}
float next_unit(inout uint chain) {
    chain = hash32(chain + 1663821227u);
    return float(chain & 16777215u) / 16777216.0;
}
// Same sine hash and value noise as the lawn (atmosphere_surface lawn_noise).
float lawn_hash(vec2 v) { return fract(sin(dot(v,vec2(12.9898,78.233)))*43758.5453); }
float lawn_noise(vec2 p) {
    vec2 i=floor(p); vec2 f=fract(p); f=f*f*(3.0-2.0*f);
    float a=lawn_hash(i), b=lawn_hash(i+vec2(1.0,0.0)), c=lawn_hash(i+vec2(0.0,1.0)), d=lawn_hash(i+vec2(1.0,1.0));
    return mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
}

// Ground height of a world texel (0.25 m grid), EMPTY when not grass. Offsets
// keep every integer positive, so division and modulo are exact floors.
float field_texel(ivec2 texel) {
    ivec2 shifted = texel + ivec2(2097152);
    ivec2 block = shifted / 32;
    ivec2 local = shifted - block * 32;
    ivec2 slot = (block + ivec2((field_blocks - 1) * 65536)) % field_blocks;
    return texelFetch(field_heights, ivec3(local, slot.x + slot.y * field_blocks), 0).r;
}

// (height, grass share) at a world point: bilinear over the four texel centres.
vec2 field_sample(vec2 xz) {
    vec2 p = xz * 4.0 - 0.5;
    vec2 cell = floor(p);
    vec2 f = p - cell;
    ivec2 i = ivec2(cell);
    vec4 h = vec4(field_texel(i), field_texel(i + ivec2(1,0)), field_texel(i + ivec2(0,1)), field_texel(i + ivec2(1,1)));
    vec4 w = vec4((1.0-f.x)*(1.0-f.y), f.x*(1.0-f.y), (1.0-f.x)*f.y, f.x*f.y);
    vec4 valid = step(vec4(EMPTY_LIMIT), h);
    float share = dot(w, valid);
    return vec2(share > 0.0 ? dot(w*valid, h)/share : 0.0, share);
}

void vertex() {
    vec3 corner = MODEL_MATRIX[3].xyz;
    ivec2 tile = ivec2(round(corner.xz / tile_metres));
    int slot = int(UV.x + 0.5);
    float kind = UV.y; // 0 card, 1 flower stem, 2 flower head
    vec3 shape = VERTEX;
    uint chain = hash32(uint(tile.x + 1048576) * 73856093u ^ hash32(uint(tile.y + 1048576) * 19349663u ^ hash32(uint(slot) * 83492791u + uint(ring_seed))));
    vec2 root = corner.xz + vec2(next_unit(chain), next_unit(chain)) * tile_metres;
    float rank = next_unit(chain);
    vec3 eye = INV_VIEW_MATRIX[3].xyz;
    float flat_distance = distance(root, eye.xz);
    float grow = 0.0;
    vec2 ground = vec2(0.0);
    float d = flat_distance;
    // The 3D distance is never below the flat one: a card its rank thins
    // there is thinned for good, before any field lookup.
    float upper = (1.0 - smoothstep(fade.z, fade.w, flat_distance)) * min(1.0, thin_distance / max(flat_distance, 0.01));
    if(flat_distance < fade.w && rank < upper) {
        ground = field_sample(root);
        d = length(vec3(root.x - eye.x, ground.x - eye.y, root.y - eye.z));
        float weight = (fade.x < 0.0 ? 1.0 : smoothstep(fade.x, fade.y, d)) * (1.0 - smoothstep(fade.z, fade.w, d));
        // Even cover: past thin_distance a card's share falls as 1 / distance.
        weight *= min(1.0, thin_distance / max(d, 0.01));
        float clumps = lawn_noise(root*0.7+3.1);
        // Gentle density variation with the lawn's clumps; rank thins cards
        // with distance and grows them in from the ground, never popping.
        grow = clamp((weight*mix(0.72, 1.0, clumps) - rank)/0.15, 0.0, 1.0);
        // Ground grass only: half or more of the surrounding texels.
        grow *= smoothstep(0.5, 0.8, ground.y);
        if(kind > 0.5) {
            ivec2 patch = ivec2(floor(root / 4.0));
            uint patch_hash = hash32(uint(patch.x + 1048576) * 73856093u ^ hash32(uint(patch.y + 1048576) * 19349663u));
            float share = float(patch_hash & 65535u) / 65536.0 < 0.2 ? flower_share.x : flower_share.y;
            if(next_unit(chain) >= share) grow = 0.0;
        }
    }
    card_seed = chain;
    if(grow <= 0.0) {
        VERTEX = vec3(0.0);
        blade_color = vec3(0.0);
        card_uv = vec2(-1.0, 0.0);
        blade_t = 0.0;
        petal = 0.0;
    } else {
        vec2 g = root;
        float patches = lawn_noise(g*0.09)*0.65+lawn_noise(g*0.23+7.3)*0.35;
        float clumps = lawn_noise(g*0.7+3.1);
        float yaw = next_unit(chain) * 6.2831853;
        vec3 across = vec3(cos(yaw), 0.0, sin(yaw));
        float wide = next_unit(chain);
        float pick = next_unit(chain);
        float tall = next_unit(chain);
        float lean_share = next_unit(chain) - 0.5;
        float bright = 0.9 + 0.2 * next_unit(chain);
        float hue = next_unit(chain);
        float phase = next_unit(chain) * 6.2831853;
        float t = kind > 1.5 ? 1.0 : shape.y;
        vec3 up = vec3(0.0, 1.0, 0.0);
        float height = mix(blade_height.x, blade_height.y, tall) * mix(0.75, 1.2, clumps) * mix(0.88, 1.08, patches) * mix(0.2, 1.0, grow);
        // Coherent gusts along one world direction (vegetation_leaf), tips only.
        float gust = 0.55+0.45*sin(TIME*0.37+dot(g,vec2(0.021,0.013)));
        float bend = (sin(TIME*1.25+phase+dot(g,vec2(0.09,0.05)))*0.6+sin(TIME*2.3+phase*1.7)*0.25+0.35)*gust;
        vec3 wind = normalize(vec3(1.0,0.0,0.35)) * bend * 0.4 * height * wind_strength + vec3(sin(TIME*3.1+phase),0.0,cos(TIME*3.6+phase)) * 0.06 * height * wind_strength;
        vec3 position;
        vec3 normal;
        if(kind < 0.5) {
            // Turn up to half way toward the camera beyond 2 m (no card seen
            // edge-on), keeping the nearer of its two faces.
            vec2 toward = (eye.xz - root) / max(flat_distance, 0.001);
            vec3 facing_across = vec3(-toward.y, 0.0, toward.x);
            if(dot(facing_across, across) < 0.0) facing_across = -facing_across;
            across = normalize(mix(across, facing_across, 0.5 * smoothstep(2.0, 6.0, d)) + vec3(0.0001, 0.0, 0.0));
            vec3 face = cross(across, up);
            float half_width = card_width * 0.5 * mix(0.8, 1.2, wide) * mix(0.5, 1.0, grow);
            // Each end stands on the ground under it; an end over no grass
            // folds back to the root, its blades with it.
            vec2 end = root + across.xz * shape.x * half_width;
            vec2 end_ground = field_sample(end);
            float column = (shape.x * 0.5 + 0.5) * float(card_columns);
            if(end_ground.y < 0.5) {
                end = root;
                end_ground.x = ground.x;
                column = float(card_columns) * 0.5;
            }
            vec3 base = vec3(end.x, end_ground.x - 0.015, end.y);
            position = base + up * height * t + (face * lean_share * 0.5 * height + wind) * t * t;
            normal = normalize(mix(face, up, 0.6));
            card_uv = vec2(column, t);
        } else {
            vec3 lean_dir = vec3(cos(yaw + 1.5707963), 0.0, sin(yaw + 1.5707963));
            vec3 base = vec3(root.x, ground.x - 0.015, root.y);
            // Flower: a thin stem with a five-petal head tilted 50 degrees
            // toward the camera (and a little its lean), so it shows its face.
            float stem = (0.15 + 0.08 * pick) * mix(0.2, 1.0, grow);
            vec3 tip = base + up * stem + lean_dir * stem * 0.15 * lean_share + wind;
            normal = normalize(mix(cross(across, up), up, 0.6));
            if(kind < 1.5) {
                position = base + across * shape.x * 0.003 + (tip - base) * t;
            } else {
                vec2 toward = eye.xz - root;
                vec3 facing = normalize(mix(lean_dir, vec3(toward.x, 0.0, toward.y) / max(length(toward), 0.001), 0.8) + vec3(0.0001, 0.0, 0.0));
                vec3 face = up * cos(0.87) + facing * sin(0.87);
                vec3 side = normalize(cross(up, facing));
                vec3 rise = cross(face, side);
                position = tip + face * 0.006 * (1.0 - shape.z) + (side * shape.x + rise * shape.y) * 0.035 * mix(0.4, 1.0, grow);
                normal = face;
            }
            card_uv = vec2(-1.0, 0.0);
        }
        VERTEX = position - corner;
        NORMAL = normal;
        // Same broad lush/dry patches as the lawn under the blades.
        float broad = sin(g.x*0.055+sin(g.y*0.037))*cos(g.y*0.063);
        vec3 color = grass_tint * (0.96+0.06*broad) * mix(vec3(0.86,0.90,0.84),vec3(1.10,1.08,1.02),patches) * bright;
        float dry = smoothstep(0.60,0.82,lawn_noise(g*0.05+19.1));
        color = mix(color, color*vec3(1.16,1.08,0.78), dry*0.55);
        // Some cards yellower, some bluer.
        color *= mix(vec3(0.94,1.0,1.08), vec3(1.10,1.04,0.86), hue);
        if(kind > 1.5) {
            // White, yellow or violet heads with a warm centre (the rim
            // vertices, shape z 1, carry the petal colour).
            vec3 petals = pick < 0.45 ? vec3(0.86,0.85,0.80) : (pick < 0.8 ? vec3(0.95,0.72,0.10) : vec3(0.52,0.36,0.78));
            color = shape.z > 0.5 ? petals : mix(petals, vec3(0.95,0.75,0.2), 0.7);
        }
        blade_color = color;
        blade_t = t;
        petal = kind > 1.5 ? 1.0 : 0.0;
    }
}

void fragment() {
    vec3 albedo = blade_color;
    float t = blade_t;
    // Column footprint of a pixel (in uniform control flow).
    float pixel = fwidth(card_uv.x);
    if(card_uv.x >= 0.0) {
        // The blade of this column or a neighbour covering the pixel: a curved
        // blade, parallel then tapering to its tip, at least about a pixel wide.
        float column = card_uv.x;
        float share = card_uv.y;
        int here = int(floor(column));
        bool hit = false;
        float shade = 1.0;
        for(int k = -1; k <= 1; k++) {
            int c = here + k;
            if(c < 0 || c >= card_columns) continue;
            uint h = hash32(card_seed ^ (uint(c) * 2654435761u + 40503u));
            float present = float(h & 255u) / 255.0;
            if(present < card_gap) continue;
            float top = mix(0.45, 1.0, (present - card_gap) / (1.0 - card_gap));
            float s = share / top;
            if(s >= 1.0) continue;
            float offset = float((h >> 8u) & 255u) / 255.0 - 0.5;
            float lean = float((h >> 16u) & 255u) / 255.0 - 0.5;
            float wide = float((h >> 24u) & 255u) / 255.0;
            float centre = float(c) + 0.5 + offset * 0.5 + lean * 1.2 * s * s;
            float half_width = max(blade_fill * mix(0.7, 1.25, wide) * clamp((1.0 - s) * 2.5, 0.0, 1.0), pixel * 0.6 * step(s, 0.95));
            if(abs(column - centre) < half_width) {
                hit = true;
                t = s;
                shade = 0.86 + 0.28 * wide;
            }
        }
        if(!hit) discard;
        albedo *= shade;
    }
    // Roots sit in the lawn's shade; tips catch the light and dry a little.
    if(petal < 0.5) {
        albedo *= mix(0.5, 1.12, t);
        albedo = mix(albedo, albedo*vec3(1.08,1.04,0.82), t*t*0.35);
    }
    if(!FRONT_FACING) NORMAL = -NORMAL;
    vec3 world_normal = normalize((INV_VIEW_MATRIX*vec4(NORMAL,0.0)).xyz);
    float roughness = 0.85;
    if(environment_enabled) {
        vec4 weather = texelFetch(environment_data,ivec2(0,0),0);
        albedo *= 1.0-weather.x*0.18;
        roughness = mix(roughness,0.45,weather.x*0.6);
        float cover = weather.y*smoothstep(0.25,0.85,world_normal.y);
        albedo = mix(albedo,vec3(0.68,0.76,0.79),cover*0.85);
    }
    ALBEDO = albedo;
    ROUGHNESS = roughness;
    SPECULAR = 0.25;
    // Thin blades pass light: back-lit grass glows warm green.
    BACKLIGHT = albedo*vec3(0.75,0.85,0.45)*0.3*(1.0-petal);
}
