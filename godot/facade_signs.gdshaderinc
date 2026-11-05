// Procedural storefront signs (original, MIT): glyphs of an invented,
// language-neutral script, eight category palettes and pictograms, drawn as
// signed distances with analytic antialiasing from the caller's pixel
// footprint. No font, text, texture or real brand. Included by the surface
// shader for generated walls; any spatial shader can include it on its own
// (it needs nothing else) and call facade_sign_face or facade_glyph_run.
//
// Every choice is an integer hash of the caller's design bits: pass bits from
// flat data and integer cell indices, never from interpolated values.

// Integer hash (PCG) for discrete facade choices: exact for the 24-bit packed
// seed and integer cells, unlike the sine hash of the surface shader
// (continuous noise only).
uint facade_pcg(uint v) { uint s=v*747796405u+2891336453u; uint w=((s>>((s>>28u)+4u))^s)*277803737u; return (w>>22u)^w; }
float facade_unit(uint v) { return float(facade_pcg(v)>>8u)/16777216.0; }

// One sign as seen at a pixel. Colours are sRGB (linearise them like other
// colour constants); shares are 0..1.
struct FacadeSign {
    vec3 color;   // face colour where the sign covers the pixel
    float ink;    // glyph, pictogram and border share of the face (ink × body is the pixel's share)
    float body;   // share covered by the sign: its panel, or channel letters and their rail
    float shadow; // darkening the sign casts on its mount beside the body
    float glow;   // night emission weight (emission = linear colour × glow × body)
    vec2 bevel;   // raised edge: outward slope along the local axes (x along, y down), 0 on the flat face
};

// Sign types: a backlit lightbox panel, channel letters on a backing rail
// over the mount, and a two-line panel (name over a row of small glyph groups).
const int FACADE_SIGN_LIGHTBOX = 0;
const int FACADE_SIGN_CHANNEL = 1;
const int FACADE_SIGN_TWO_LINE = 2;
int facade_sign_type(uint bits) { uint t = facade_pcg(bits^2246822507u)%20u; return t<9u ? FACADE_SIGN_LIGHTBOX : (t<15u ? FACADE_SIGN_CHANNEL : FACADE_SIGN_TWO_LINE); }

// Category palettes (food, cafe, shop, clinic, finance, beauty, academy,
// store): four backgrounds and two inks each, and one pictogram per category
// (bowl, cup, bag, cross, coin, star, book, stripe).
const vec3 FACADE_SIGN_BACK[32] = {
    vec3(0.769,0.157,0.125), vec3(0.886,0.431,0.118), vec3(0.941,0.706,0.157), vec3(0.588,0.118,0.157),
    vec3(0.361,0.235,0.157), vec3(0.157,0.353,0.235), vec3(0.902,0.863,0.784), vec3(0.118,0.118,0.118),
    vec3(0.118,0.431,0.784), vec3(0.902,0.235,0.471), vec3(0.157,0.627,0.471), vec3(0.980,0.784,0.118),
    vec3(0.961,0.961,0.961), vec3(0.863,0.925,0.961), vec3(0.902,0.961,0.922), vec3(1.000,1.000,1.000),
    vec3(0.078,0.157,0.353), vec3(0.039,0.275,0.471), vec3(0.941,0.941,0.941), vec3(0.157,0.157,0.196),
    vec3(0.902,0.471,0.667), vec3(0.588,0.275,0.627), vec3(0.980,0.863,0.902), vec3(0.157,0.118,0.196),
    vec3(0.118,0.353,0.667), vec3(0.157,0.549,0.275), vec3(0.980,0.980,0.941), vec3(0.863,0.471,0.118),
    vec3(0.078,0.588,0.353), vec3(0.902,0.314,0.118), vec3(0.118,0.275,0.627), vec3(0.980,0.980,0.980)
};
const vec3 FACADE_SIGN_INK[16] = {
    vec3(1.000,0.973,0.882), vec3(0.157,0.078,0.039),
    vec3(0.980,0.941,0.863), vec3(0.784,0.588,0.353),
    vec3(1.000,1.000,1.000), vec3(0.078,0.078,0.157),
    vec3(0.784,0.118,0.157), vec3(0.078,0.471,0.314),
    vec3(0.941,0.784,0.314), vec3(1.000,1.000,1.000),
    vec3(1.000,1.000,1.000), vec3(0.471,0.118,0.353),
    vec3(1.000,1.000,1.000), vec3(0.078,0.157,0.353),
    vec3(1.000,1.000,1.000), vec3(0.902,0.157,0.157)
};
// Filled share of each pictogram's unit box (numerically integrated).
const float FACADE_PICTOGRAM_AREA[8] = { 0.321, 0.382, 0.488, 0.326, 0.275, 0.280, 0.272, 0.504 };
// Glyph strokes join neighbouring points of a 3x4 lattice (x 0..2 × 0.4,
// y 0..3 × 0.3, y down) in one of six steps.
const ivec2 FACADE_GLYPH_STEP[6] = { ivec2(1,0), ivec2(0,1), ivec2(1,1), ivec2(1,-1), ivec2(0,2), ivec2(2,0) };
// Glyph boxes are 0.8 × 0.9 glyph units, every 1.04 along a horizontal run
// and every 1.14 down a vertical one.
const float FACADE_GLYPH_ADVANCE = 1.04;
const float FACADE_GLYPH_ADVANCE_DOWN = 1.14;

float facade_sign_luma(vec3 c) { return dot(c,vec3(0.299,0.587,0.114)); }
// Analytic antialiasing: share of a pixel of footprint fw inside distance d.
float facade_sign_aa(float d, float fw) { return clamp(0.5-d/max(fw,1e-5),0.0,1.0); }
// Box-filtered share of a pixel of footprint fw over the span |t| <= hw (exact
// pulse integral): lines thinner than a pixel settle on their area share.
float facade_sign_pulse(float t, float hw, float fw) { fw = max(fw,1e-5); return max(min(hw,t+0.5*fw)-max(-hw,t-0.5*fw),0.0)/fw; }
// Box-filtered share of an axis-aligned rectangle (separable pulses).
float facade_sign_rect(vec2 p, vec2 centre, vec2 half_size, float fw) { return facade_sign_pulse(p.x-centre.x,half_size.x,fw)*facade_sign_pulse(p.y-centre.y,half_size.y,fw); }
float facade_sign_box(vec2 p, vec2 centre, vec2 half_size) { vec2 q=abs(p-centre)-half_size; return length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0); }
float facade_sign_segment(vec2 p, vec2 a, vec2 b) { vec2 pa=p-a; vec2 ba=b-a; return length(pa-ba*clamp(dot(pa,ba)/max(dot(ba,ba),1e-6),0.0,1.0)); }
float facade_sign_ring(vec2 p, vec2 centre, float radius, float half_width) { return abs(length(p-centre)-radius)-half_width; }
// Five-pointed star (point up) of outer radius r, inner/outer ratio rf.
float facade_sign_star(vec2 p, float r, float rf) {
    const vec2 k1 = vec2(0.809016994375,-0.587785252292);
    const vec2 k2 = vec2(-0.809016994375,-0.587785252292);
    p.x = abs(p.x);
    p -= 2.0*max(dot(k1,p),0.0)*k1;
    p -= 2.0*max(dot(k2,p),0.0)*k2;
    p.x = abs(p.x);
    p.y -= r;
    vec2 ba = rf*vec2(-k1.y,k1.x)-vec2(0.0,1.0);
    float h = clamp(dot(p,ba)/dot(ba,ba),0.0,r);
    return length(p-ba*h)*sign(p.y*ba.x-p.x*ba.y);
}

// Signed distance to pictogram `kind` in its unit box (y down).
float facade_sign_pictogram(vec2 p, uint kind) {
    if(kind==0u) return min(max(length(p-vec2(0.5,0.48))-0.44,0.48-p.y),facade_sign_box(p,vec2(0.5,0.95),vec2(0.14,0.03)));
    if(kind==1u) return min(facade_sign_box(p,vec2(0.42,0.56),vec2(0.27,0.28))-0.02,max(facade_sign_ring(p,vec2(0.69,0.55),0.15,0.04),0.69-p.x));
    if(kind==2u) return min(facade_sign_box(p,vec2(0.5,0.67),vec2(0.35,0.28))-0.02,max(facade_sign_ring(p,vec2(0.5,0.39),0.19,0.04),p.y-0.39));
    if(kind==3u) return min(facade_sign_box(p,vec2(0.5),vec2(0.12,0.4)),facade_sign_box(p,vec2(0.5),vec2(0.4,0.12)));
    if(kind==4u) return min(facade_sign_ring(p,vec2(0.5),0.37,0.05),facade_sign_segment(p,vec2(0.5,0.3),vec2(0.5,0.7))-0.045);
    if(kind==5u) return facade_sign_star(vec2(p.x-0.5,0.53-p.y),0.46,0.45);
    if(kind==6u) {
        float d = min(facade_sign_segment(p,vec2(0.1,0.22),vec2(0.5,0.3)),facade_sign_segment(p,vec2(0.5,0.3),vec2(0.9,0.22)));
        d = min(d,min(facade_sign_segment(p,vec2(0.9,0.22),vec2(0.9,0.84)),facade_sign_segment(p,vec2(0.9,0.84),vec2(0.5,0.92))));
        d = min(d,min(facade_sign_segment(p,vec2(0.5,0.92),vec2(0.1,0.84)),facade_sign_segment(p,vec2(0.1,0.84),vec2(0.1,0.22))));
        return min(d,facade_sign_segment(p,vec2(0.5,0.3),vec2(0.5,0.92)))-0.04;
    }
    return min(min(facade_sign_box(p,vec2(0.17,0.5),vec2(0.1,0.42)),facade_sign_box(p,vec2(0.5,0.5),vec2(0.1,0.42))),facade_sign_box(p,vec2(0.83,0.5),vec2(0.1,0.42)));
}

// Raised (channel) letters measure their return and shadow at offset points;
// flat ones (zero offsets) evaluate once and repeat the result.
bool facade_sign_raised(vec2 side, vec2 shadow) { return any(notEqual(side,vec2(0.0))) || any(notEqual(shadow,vec2(0.0))); }

// Pictogram coverage at p (box units) and at p-side, p-shadow; settles on its
// filled share over the box once its 0.08-wide strokes approach a pixel.
vec3 facade_sign_pictogram_cover(vec2 p, uint kind, float fw, vec2 side, vec2 shadow, float soft) {
    bool raised = facade_sign_raised(side,shadow);
    float settle = smoothstep(0.04,0.1,fw);
    float area = FACADE_PICTOGRAM_AREA[kind];
    vec3 near = vec3(0.0);
    if(settle<1.0) {
        near.x = facade_sign_aa(facade_sign_pictogram(p,kind),fw);
        near.yz = raised ? vec2(facade_sign_aa(facade_sign_pictogram(p-side,kind),fw),facade_sign_aa(facade_sign_pictogram(p-shadow,kind),fw+soft)) : near.xx;
    }
    vec3 far = vec3(area*facade_sign_aa(facade_sign_box(p,vec2(0.5),vec2(0.5)),fw));
    if(raised) far.yz = area*vec2(facade_sign_aa(facade_sign_box(p-side,vec2(0.5),vec2(0.5)),fw),facade_sign_aa(facade_sign_box(p-shadow,vec2(0.5),vec2(0.5)),fw+soft));
    return mix(near,far,settle);
}

// Ink area of a dash glyph and the expected ink area of a regular one: four
// strokes of mean length 0.517, a dot one time in five, less the 19 % overlap.
float facade_glyph_dash_area(float radius) { return 2.0*radius*0.5+PI*radius*radius; }
float facade_glyph_expected_area(float radius) { return 0.81*(4.0*(2.0*radius*0.517+PI*radius*radius)+0.2*PI*0.01); }

// One glyph (`bits`) in its 0.8 × 0.9 box: 3–5 strokes of radius r with
// rounded caps (and a dot one time in five). Returns the distances at p,
// p-side and p-shadow (`raised`; otherwise the distance at p three times);
// `area` is its ink area (capsules less their 19 % average overlap). A dash
// (`dash`) is one short centred stroke.
vec3 facade_glyph(vec2 p, vec2 side, vec2 shadow, uint bits, float radius, bool dash, bool raised, out float area) {
    vec3 d = vec3(1e3);
    if(dash) {
        vec2 a = vec2(0.15,0.45); vec2 b = vec2(0.65,0.45);
        area = facade_glyph_dash_area(radius);
        d.x = facade_sign_segment(p,a,b);
        d.yz = raised ? vec2(facade_sign_segment(p-side,a,b),facade_sign_segment(p-shadow,a,b)) : d.xx;
        return d-radius;
    }
    int count = 3+int(facade_pcg(bits^1759714724u)%3u);
    float total = 0.0;
    for(int k=0;k<5;k++) {
        if(k>=count) break;
        uint h = facade_pcg(bits+uint(k)*2654435769u);
        ivec2 step_ = FACADE_GLYPH_STEP[int((h>>16u)%6u)];
        int i = min(int(h%3u),2-step_.x);
        int j = int((h>>8u)%4u);
        j = step_.y>0 ? min(j,3-step_.y) : max(j,-step_.y);
        vec2 a = vec2(float(i)*0.4,float(j)*0.3);
        vec2 b = a+vec2(step_)*vec2(0.4,0.3);
        d.x = min(d.x,facade_sign_segment(p,a,b));
        if(raised) d.yz = min(d.yz,vec2(facade_sign_segment(p-side,a,b),facade_sign_segment(p-shadow,a,b)));
        total += 2.0*radius*length(b-a)+PI*radius*radius;
    }
    d -= radius;
    uint dot_bits = facade_pcg(bits^742938285u);
    if(dot_bits%5u==0u) {
        vec2 c = vec2(float((dot_bits>>8u)%3u)*0.4,float((dot_bits>>12u)%4u)*0.3);
        d.x = min(d.x,length(p-c)-0.1);
        if(raised) d.yz = min(d.yz,vec2(length(p-side-c),length(p-shadow-c))-0.1);
        total += PI*0.01;
    }
    area = 0.81*total;
    return raised ? d : d.xxx;
}

// A run of `count` glyphs from q = (0,0), the top-left of the first glyph box,
// in glyph units (y down; vertical runs stack upright glyphs downwards).
// fw: the pixel footprint in glyph units (computed by the caller in uniform
// control flow). Glyph g uses facade_pcg(bits ^ g-hash); every `dash_every`-th
// glyph (0: none) is a dash. Returns the ink coverage at q, q-side and
// q-shadow (the shadow edge softened by `soft`).
// Each glyph owns a cell from mid-gap to mid-gap (0.12 before its box, for
// both advances), so its strokes, round caps and their antialiasing stay in
// its own cell; a raised glyph's return and shadow reaching into the next
// cell are added there. As strokes approach a pixel a glyph settles on its
// own mean over its cell, and as glyphs do the run settles on the expected
// mean (dashes counted) over the same cells, so far signs never crawl and
// keep their brightness through both settles.
const float FACADE_GLYPH_LEAD = 0.12;
vec3 facade_glyph_run(vec2 q, uint bits, int count, float radius, float fw, bool vertical, int dash_every, vec2 side, vec2 shadow, float soft) {
    bool raised = facade_sign_raised(side,shadow);
    float advance = vertical ? FACADE_GLYPH_ADVANCE_DOWN : FACADE_GLYPH_ADVANCE;
    vec2 box = vec2(0.8,0.9);
    vec2 pad = box+2.0*radius;
    float stroke_settle = smoothstep(0.5,1.2,fw/(2.0*radius));
    float run_settle = smoothstep(0.25,0.6,fw/advance);
    // The run's cells: along, from LEAD before the first box to LEAD before
    // the box after the last; across, the padded box.
    float run_along = float(count)*advance;
    vec2 tile_centre = vertical ? vec2(box.x*0.5,run_along*0.5-FACADE_GLYPH_LEAD) : vec2(run_along*0.5-FACADE_GLYPH_LEAD,box.y*0.5);
    vec2 tile_half = vertical ? vec2(pad.x*0.5,run_along*0.5) : vec2(run_along*0.5,pad.y*0.5);
    vec3 tile = vec3(facade_sign_aa(facade_sign_box(q,tile_centre,tile_half),fw));
    if(raised) tile.yz = vec2(facade_sign_aa(facade_sign_box(q-side,tile_centre,tile_half),fw),facade_sign_aa(facade_sign_box(q-shadow,tile_centre,tile_half),fw+soft));
    float cell_area = advance*(vertical ? pad.x : pad.y);
    int dashes = dash_every>0 ? count/dash_every : 0;
    float run_mean = min((float(count-dashes)*facade_glyph_expected_area(radius)+float(dashes)*facade_glyph_dash_area(radius))/(float(count)*cell_area),1.0);
    vec3 cover = vec3(0.0);
    if(run_settle<1.0) {
        float along = vertical ? q.y : q.x;
        float g = floor((along+FACADE_GLYPH_LEAD)/advance);
        vec2 step_ = vertical ? vec2(0.0,advance) : vec2(advance,0.0);
        vec3 d = vec3(1e3);
        float mean = run_mean;
        if(g>=0.0 && g<float(count)) {
            float area;
            d = facade_glyph(q-g*step_,side,shadow,facade_pcg(bits^(uint(int(g))*668265263u+374761393u)),radius,dash_every>0 && int(g)%dash_every==dash_every-1,raised,area);
            mean = min(area/cell_area,1.0);
        }
        if(raised && g>=1.0 && g<=float(count)) {
            // The previous glyph's return and shadow (offset down-right) reach
            // past its cell by at most this much along the run.
            vec2 reach2 = max(side,shadow);
            float reach = (vertical ? box.y+reach2.y : box.x+reach2.x)+radius+0.5*(fw+soft)-advance;
            if(along-g*advance<reach) {
                float area;
                float h = g-1.0;
                vec3 prev = facade_glyph(q-h*step_,side,shadow,facade_pcg(bits^(uint(int(h))*668265263u+374761393u)),radius,dash_every>0 && int(h)%dash_every==dash_every-1,raised,area);
                d.yz = min(d.yz,prev.yz);
            }
        }
        vec3 near = vec3(facade_sign_aa(d.x,fw),facade_sign_aa(d.y,fw),facade_sign_aa(d.z,fw+soft));
        cover = mix(near,mean*tile,stroke_settle);
    }
    return mix(cover,run_mean*tile,run_settle);
}

// Storefront sign face at `local`, in units of the sign's short side: a
// horizontal sign spans x 0..aspect (along) and y 0..1 (down); a vertical
// (blade) sign spans x 0..1 (across) and y 0..aspect (down), its glyphs
// stacked upright. Pixels outside the sign return body 0 and the shadow it
// casts on its mount. bits: the design (category, background, border style,
// pictogram, glyph count and type, all from facade_pcg). footprint: the
// pixel footprint in local units, from derivatives taken by the caller in
// uniform control flow. Vertical signs draw two-line designs as lightboxes.
FacadeSign facade_sign_face(vec2 local, uint bits, float aspect, float footprint, bool vertical) {
    FacadeSign s;
    float fw = max(footprint,1e-5);
    int kind = facade_sign_type(bits);
    if(vertical && kind==FACADE_SIGN_TWO_LINE) kind = FACADE_SIGN_LIGHTBOX;
    uint category = (bits>>3u)&7u;
    vec3 back = FACADE_SIGN_BACK[int(category*4u+((bits>>6u)&3u))];
    vec3 ink_a = FACADE_SIGN_INK[int(category*2u)];
    vec3 ink_b = FACADE_SIGN_INK[int(category*2u+1u)];
    float luma = facade_sign_luma(back);
    vec3 ink = abs(facade_sign_luma(ink_a)-luma)>=abs(facade_sign_luma(ink_b)-luma) ? ink_a : ink_b;
    int style = int((bits>>9u)&3u);
    if(kind==FACADE_SIGN_CHANNEL) style = 0;
    bool pictogram = (bits>>11u)%10u<7u;
    int wanted = 2+int((bits>>14u)%5u);
    // Layout along the run (x of a horizontal sign, y of a vertical one) and
    // across it, in local units.
    vec2 axis = vertical ? local.yx : local;
    float glyph = kind==FACADE_SIGN_TWO_LINE ? 0.50 : (kind==FACADE_SIGN_CHANNEL ? (vertical ? 0.80 : 0.70) : (vertical ? 0.76 : 0.62));
    float picto = kind==FACADE_SIGN_TWO_LINE ? 0.56 : 0.68;
    float advance = (vertical ? FACADE_GLYPH_ADVANCE_DOWN : FACADE_GLYPH_ADVANCE)*glyph;
    float box_along = (vertical ? 0.9 : 0.8)*glyph;
    float box_across = (vertical ? 0.8 : 0.9)*glyph;
    float start = 0.12+(style==3 ? 0.2 : 0.0);
    float avail = aspect-start-0.12;
    float picto_len = picto+0.14*glyph;
    if(avail<picto_len+box_along) pictogram = false;
    if(!pictogram) picto_len = 0.0;
    int count = clamp(min(wanted,int(floor((avail-picto_len-box_along)/advance))+1),1,7);
    float run_len = picto_len+float(count-1)*advance+box_along;
    float along0 = start+max(avail-run_len,0.0)*0.5;
    float centre = kind==FACADE_SIGN_TWO_LINE ? 0.36 : (style==2 ? 0.44 : 0.5);
    float radius = kind==FACADE_SIGN_CHANNEL ? 0.095 : 0.08;
    // Raised channel letters show a darker return below-right and cast a
    // soft shadow further out (local units).
    vec2 side = kind==FACADE_SIGN_CHANNEL ? vec2(0.016,0.026) : vec2(0.0);
    vec2 shadow = kind==FACADE_SIGN_CHANNEL ? vec2(0.04,0.07) : vec2(0.0);
    vec2 run_origin = vertical ? vec2(centre-box_across*0.5,along0+picto_len) : vec2(along0+picto_len,centre-box_across*0.5);
    vec3 letters = facade_glyph_run((local-run_origin)/glyph,bits,count,radius,fw/glyph,vertical,0,side/glyph,shadow/glyph,0.05/glyph);
    if(pictogram) {
        vec2 picto_origin = vertical ? vec2(centre-picto*0.5,along0) : vec2(along0,centre-picto*0.5);
        vec3 icon = facade_sign_pictogram_cover((local-picto_origin)/picto,category,fw/picto,side/picto,shadow/picto,0.05/picto);
        letters = 1.0-(1.0-letters)*(1.0-icon);
    }
    float centre_glow = 1.0-0.5*length((axis-vec2(aspect*0.5,0.5))/vec2(aspect*0.5,0.5));
    if(kind==FACADE_SIGN_CHANNEL) {
        // Letters in the most saturated bright colour of the design, raised
        // on a thin dark rail behind the run; the mount shows between them.
        vec3 letter = back;
        float best = -1.0;
        for(int i=0;i<3;i++) {
            vec3 c = i==0 ? back : (i==1 ? ink_a : ink_b);
            float score = facade_sign_luma(c)<0.25 ? -1.0 : max(c.r,max(c.g,c.b))-min(c.r,min(c.g,c.b))+0.5*facade_sign_luma(c);
            if(score>best) { best = score; letter = c; }
        }
        if(best<0.0) letter = vec3(1.0);
        vec2 rail_centre = vertical ? vec2(centre,along0+run_len*0.5) : vec2(along0+run_len*0.5,centre);
        vec2 rail_half = vertical ? vec2(0.05,run_len*0.5+0.05) : vec2(run_len*0.5+0.05,0.05);
        // Box-filtered (the rail is 0.1 thick): sub-pixel at distance, it
        // settles on its area share instead of crawling.
        float rail = facade_sign_rect(local,rail_centre,rail_half,fw);
        float rail_shadow = facade_sign_rect(local-vec2(0.0,0.03),rail_centre,rail_half,fw+0.03);
        float sides = max(letters.y-letters.x,0.0);
        s.body = clamp(letters.x+(1.0-letters.x)*max(sides,rail),0.0,1.0);
        vec3 back_part = mix(vec3(0.17,0.17,0.18),letter*0.45,sides/max(sides+rail,1e-4));
        s.color = mix(back_part,letter,letters.x/max(s.body,1e-4));
        s.ink = letters.x/max(s.body,1e-4);
        s.glow = 1.25*letters.x/max(s.body,1e-4);
        s.shadow = 0.6*max(letters.z,rail_shadow)*(1.0-s.body);
        s.bevel = vec2(0.0);
        return s;
    }
    // Panel signs: a slightly rounded panel, its border style, the name run and
    // (two-line) a sub line of small glyph groups with dashes.
    vec2 half_size = vertical ? vec2(0.5,aspect*0.5) : vec2(aspect*0.5,0.5);
    float panel_d = facade_sign_box(local,half_size,half_size-0.04)-0.04;
    s.body = facade_sign_aa(panel_d,fw);
    float border = 0.0;
    if(style==1) {
        float ring = abs(facade_sign_box(local,half_size,half_size-0.08))-0.02;
        border = mix(facade_sign_aa(ring,fw),0.08*(aspect+0.68)/aspect,smoothstep(0.5,1.2,fw/0.04));
    } else if(style==2 && kind!=FACADE_SIGN_TWO_LINE) {
        float bar = vertical ? local.x-0.86 : local.y-0.86;
        border = facade_sign_aa(-bar,fw);
    } else if(style==3) {
        float bar = (vertical ? local.y : local.x)-0.2;
        border = facade_sign_aa(bar,fw);
    }
    float sub = 0.0;
    if(kind==FACADE_SIGN_TWO_LINE) {
        float small = 0.17;
        int groups = clamp(int(floor((run_len+0.24*small)/(FACADE_GLYPH_ADVANCE*small))),4,14);
        float sub_len = float(groups-1)*FACADE_GLYPH_ADVANCE*small+0.8*small;
        vec2 sub_origin = vec2(start+(avail-sub_len)*0.5,0.78-0.45*small);
        sub = facade_glyph_run((local-sub_origin)/small,bits^1013904223u,groups,0.085,fw/small,false,4,vec2(0.0),vec2(0.0),0.0).x;
        // A hairline divider (box-filtered, so it settles on its share).
        if(style==2) border = facade_sign_rect(local,vec2(aspect*0.5,0.63),vec2(run_len*0.5,0.012),fw);
    }
    // Coverages combine as a union (exact when crisp, the expected overlap once settled).
    s.ink = clamp(1.0-(1.0-letters.x)*(1.0-sub)*(1.0-border),0.0,1.0);
    s.color = mix(back,ink,s.ink);
    // Backlit at night, a little brighter in the middle: dark ink blocks the
    // light, bright ink on a dark panel glows instead.
    bool bright_ink = facade_sign_luma(ink)>luma;
    float face_glow = bright_ink ? 0.35 : 0.85+0.3*clamp(centre_glow,0.0,1.0);
    s.glow = mix(face_glow,bright_ink ? 1.25 : 0.12,s.ink);
    // A soft drop shadow below-right on the mount, and a bevelled edge.
    s.shadow = 0.5*facade_sign_aa(facade_sign_box(local-vec2(0.025,0.06),half_size,half_size-0.04)-0.04,fw+0.06)*(1.0-s.body);
    vec2 q = abs(local-half_size)-(half_size-0.04);
    vec2 outward = q.x>q.y ? vec2(sign(local.x-half_size.x),0.0) : vec2(0.0,sign(local.y-half_size.y));
    s.bevel = outward*(1.0-smoothstep(0.0,0.07,-panel_d))*s.body*(1.0-smoothstep(0.02,0.04,fw));
    return s;
}
