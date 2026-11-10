//! Display-only facade fixtures of one packed cell: projecting blade signs
//! (Korean 돌출간판, vertical lightboxes perpendicular to the wall) on
//! commercial walls and wall-mounted air-conditioner outdoor units under
//! upper-storey windows of house, commercial and public walls; rooftop
//! fixtures (roof_fixtures.rs) join them in one list under the same
//! clearance test and caps.
//!
//! Placement reads only the packed display arrays (scene vertices and
//! normals, object indices, the facade seeds and tops of `packed.rs`) and the
//! floor pitch of the presentation; nothing generated, collided or sent over
//! the network changes. Every choice is an integer PCG hash of the 12-bit
//! record seed and integer slot, column or row indices (no float-order
//! dependence); positions are computed from them in the wall's own frame.
//! Bays and window rects follow the solid's 12-bit look
//! (mapkit-package distant.rs `facade_look`: the solid's own seed for the
//! key's `own_share` of solids, otherwise the record's), as in the shader.
//!
//! Coupling with `godot/atmosphere_surface.gdshader` (keep both in step):
//! - wall frame: u = dot(p, t) with t = normalize(-n.z, 0, n.x) for the wall
//!   normal n, v = height above the solid base (facade seed y - 1000);
//! - bays: floor pitch × base(kind) × (0.8 + 0.5 × ((look >> 9) & 7) / 7);
//!   sign slots are two bays wide and read along -u, so slot boundaries lie at
//!   u = -k × slot width (blade signs stand on them);
//! - commercial storeys: `facade_storeys` (a 1.5-floor shop storey, whole
//!   storeys fitted below the solid top); other kinds use floor-pitch rows;
//! - window rects per kind and look variant ((look >> 3) & 3), the sill
//!   ledge (0.035 floors) and its shadow below it: air-conditioner units hang
//!   in the wall band between the sill shadow and the window of the storey
//!   below; curtain walls (the `facade_palette.json` finish pick) get none.
//!
//! Sizes are in floor pitches (facade_floor_cm / 100: metres at 1:3) and match
//! `godot/facade_fixture_meshes.gd`. A fixture whose box (plus a margin) would
//! touch any other triangle of the cell (neighbouring buildings, roofs, tree
//! trunks, terrain, other walls of its own building) or leave the cell rect is
//! skipped, as is one whose mount would not lie on its own wall.
use godot::builtin::{Vector2, Vector3};
use std::collections::HashMap;

/// Floats per fixture: mount point x, y, z (scene metres: on the wall at the
/// fixture's bottom centre; rooftop fixtures on the roof, at their footprint
/// centre or, for rims and solar rows, their left end), yaw (local +z faces
/// out of the wall, a penthouse's door, inward from a rim), kind, scale (the
/// floor pitch), height, 24-bit design bits, width and depth (metres; 0 for
/// wall fixtures, whose meshes have their own).
pub const FLOATS: usize = 10;
/// At most this many fixtures per cell (blade signs, penthouses, tanks, rims
/// by roof, then hash rank). Measured on the 1:3 real-city cards (128 m
/// cells, rooftop fixtures included): Yeongtong needs at most 633 in nine
/// cells of ten and 2,054 at most, Jongno 1,298 and 5,325; denser cells keep
/// their wall signs, penthouses, tanks and the rims of as many roofs as fit.
pub const MAX_FIXTURES: usize = 1024;
/// Fixtures are instanced per kind and SQUARE_METRES square; at most this many
/// squares per cell hold fixtures (render_memory.gd reserves per square): the
/// ones with the most clear candidates.
pub const MAX_SQUARES: usize = 16;
pub const SQUARE_METRES: f32 = 32.0;
const MAX_CANDIDATES: usize = 16384;
pub const BLADE: f32 = 0.0;
pub const AIR_CONDITIONER: f32 = 1.0;
/// Blade sign (floor pitches): its broad faces stand perpendicular to the
/// wall, WIDTH out from a STANDOFF on two brackets, THICKNESS along the wall.
pub const BLADE_WIDTH: f32 = 0.2;
pub const BLADE_THICKNESS: f32 = 0.06;
pub const BLADE_STANDOFF: f32 = 0.04;
/// Blade heights: 0.9 to 1.3 floors in five steps.
const BLADE_SHARE: u32 = 45;
/// Air-conditioner unit on its rack (floor pitches).
pub const AC_WIDTH: f32 = 0.27;
pub const AC_HEIGHT: f32 = 0.2;
pub const AC_DEPTH: f32 = 0.1;
pub const AC_STANDOFF: f32 = 0.03;
pub const AC_RACK: f32 = 0.03;
const AC_SHARE: u32 = 14;
const AC_PER_WALL: usize = 3;
/// Clearance around every fixture box (floor pitches).
pub(crate) const MARGIN: f32 = 0.03;
/// Candidates not mounted on a wall plane (rooftop fixtures).
pub(crate) const NO_PLANE: usize = u32::MAX as usize - 1;

/// The facade shader's integer hash (facade_signs.gdshaderinc facade_pcg).
pub fn pcg(v: u32) -> u32 {
    let s = v.wrapping_mul(747796405).wrapping_add(2891336453);
    let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277803737);
    (w >> 22) ^ w
}
pub(crate) fn unit(v: u32) -> f32 { (pcg(v) >> 8) as f32 / 16777216.0 }
/// facade_cell of the shader: an integer cell index as an unsigned hash input.
pub(crate) fn cell(index: i64) -> u32 { (index as i32).wrapping_add(1048576) as u32 }

/// chunk_renderer.facade_kind: 1 apartment, 2 house, 3 commercial, 4
/// industrial, 5 public, 0 none.
pub fn facade_kind(material: &str, usage: &str) -> u8 {
    if !["brick", "wood", "concrete"].contains(&material) { return 0; }
    match usage {
        "residential" => if material == "concrete" { 1 } else { 2 },
        "commercial" => 3,
        "industrial" => 4,
        "public" => 5,
        _ => 0,
    }
}

/// Ordered finish weights of concrete commercial walls, the only key with a
/// curtain wall (facade_palette.json; the Godot JSON parser keeps file order,
/// which serde_json does not, so the object is read in order here).
fn commercial_finishes() -> &'static [(String, f64)] {
    static WEIGHTS: std::sync::OnceLock<Vec<(String, f64)>> = std::sync::OnceLock::new();
    WEIGHTS.get_or_init(|| {
        let text = include_str!("../../../godot/facade_palette.json");
        let from = text.find("\"weights\": {").expect("finish weights");
        let start = from + text[from..].find("\"concrete:commercial\": {").expect("commercial weights");
        let body = &text[start + text[start..].find('{').unwrap() + 1..];
        let body = &body[..body.find('}').unwrap()];
        body.split(',').map(|entry| {
            let (name, weight) = entry.split_once(':').expect("weight entry");
            (name.trim().trim_matches('"').to_string(), weight.trim().parse().expect("numeric weight"))
        }).collect()
    })
}

/// Whether a concrete commercial record's walls are a curtain wall: the
/// vertex shader's finish pick against the cumulative weights.
fn curtain(record: u32) -> bool {
    let weights = commercial_finishes();
    let total: f64 = weights.iter().map(|w| w.1).sum();
    let pick = unit(record ^ 1759714724) as f64;
    let mut cumulative = 0.0;
    for (name, weight) in weights {
        cumulative += weight;
        if pick < (cumulative / total) as f32 as f64 { return name == "curtain"; }
    }
    weights.last().is_some_and(|w| w.0 == "curtain")
}

/// The shader's window rect (x0, x1, y0, y1 in cell units) of a kind and look.
fn pane(kind: u8, look: u32) -> [f32; 4] {
    let mut pane = match kind { 1 => [0.2, 0.75, 0.3, 0.75], 2 => [0.3, 0.7, 0.3, 0.7], 3 => [0.1, 0.9, 0.15, 0.85], 4 => [0.15, 0.85, 0.6, 0.85], _ => [0.3, 0.7, 0.2, 0.85] };
    if kind != 2 && kind != 4 {
        pane = match (look >> 3) & 3 { 1 => [0.35, 0.65, 0.12, 0.86], 2 => [0.0, 1.0, 0.32, 0.76], 3 => [0.25, 0.75, 0.38, 0.80], _ => pane };
    }
    pane
}

/// The 12-bit look of a 24-bit facade seed (record << 12 | solid) on a wall
/// of facade kind `kind` (1, apartments, is concrete:residential).
pub(crate) fn look(seed: u32, kind: u8) -> u32 {
    let share = mapkit_package::distant::facade_own_share(if kind == 1 { "concrete:residential" } else { "default" });
    mapkit_package::distant::facade_look(seed >> 12, seed & 0xFFF, share)
}

/// facade_storeys: (shop storey, fitted upper pitch; 0 for a plain parapet).
fn storeys(height: f32, pitch: f32) -> (f32, f32) {
    let ground = (1.5 * pitch).min(height);
    let upper = height - ground;
    (ground, if upper < 0.75 * pitch { 0.0 } else { upper / (upper / pitch + 0.01).floor().max(1.0) })
}

pub struct Input<'a> {
    /// Scene metres, three per triangle (packed scene_vertices).
    pub vertices: &'a [Vector3],
    /// Per vertex (packed scene_normals: the face normal, out of the solid).
    pub normals: &'a [Vector3],
    /// Per triangle object index.
    pub objects: &'a [i32],
    /// Per vertex (seed, 1000 + base) and ±(1000 + top) (packed facade arrays;
    /// negative on triangles rising above the top).
    pub seeds: &'a [Vector2],
    pub tops: &'a [f32],
    /// Per object facade kind (facade_kind; 0 for non-building objects).
    pub kinds: &'a [u8],
    /// Per object: concrete material (only concrete commercial walls can be curtain walls).
    pub concrete: &'a [bool],
    /// Per object palette key (roof_fixtures::palette_key; 255 none).
    pub keys: &'a [u8],
    /// Floor pitch in scene metres (facade_floor_cm / 100).
    pub pitch: f32,
    /// Cell rect in scene metres: min x, min z, max x, max z.
    pub bounds: [f32; 4],
}

#[derive(Debug, Default, PartialEq)]
pub struct Placed {
    /// FLOATS per fixture.
    pub values: Vec<f32>,
    /// Clear fixtures dropped by MAX_FIXTURES or MAX_SQUARES.
    pub capped: usize,
    /// Candidates whose box hit another triangle or left the cell.
    pub blocked: usize,
}

pub(crate) struct Plane { pub(crate) normal: Vector3, pub(crate) offset: f32, triangles: Vec<usize>, pub(crate) object: usize, pub(crate) seed: u32, record: u32, pub(crate) look: u32, base: f32, height: f32, opposite: Vec<usize> }

/// A fixture box in its own frame (lo..hi; x right, y up, z out). Selection:
/// by class (0 blade, 1 penthouse, 2 tank, 3 rim, 4 the rest), then rank and
/// attempt; a slot (> 0) keeps the first of its alternative attempts that
/// gets a fixture kept, and the units of a group (> 0) may touch.
pub(crate) struct Candidate { plane: usize, kind: f32, pub(crate) origin: Vector3, yaw: f32, height: f32, bits: u32, right: Vector3, out: Vector3, min: [f32; 3], max: [f32; 3], pub(crate) lo: Vector3, pub(crate) hi: Vector3,
    pub(crate) width: f32, pub(crate) depth: f32, pub(crate) class: u8, pub(crate) rank: u32, pub(crate) slot: u32, pub(crate) attempt: u32, pub(crate) group: u32 }

/// Wall planes of generated building solids: vertical triangles of one solid
/// (same object and seed) sharing a plane, in first-triangle order. Each
/// plane lists the coincident planes facing the other way (internal walls
/// between a footprint's prisms, party walls of touching buildings): where
/// one covers a mount, the wall is not a facade.
fn planes(input: &Input) -> (Vec<Plane>, Vec<u32>) {
    let mut planes: Vec<Plane> = Vec::new();
    let mut owner = vec![u32::MAX; input.objects.len()];
    let mut solids = HashMap::<(i32, u32, u32), Vec<usize>>::new();
    for (t, &object) in input.objects.iter().enumerate() {
        if input.kinds.get(object as usize).copied().unwrap_or(0) == 0 { continue; }
        let seed = input.seeds[t * 3];
        if seed.y <= 0.5 || input.tops[t * 3].abs() <= 0.5 { continue; }
        let n = input.normals[t * 3];
        let horizontal = Vector3::new(n.x, 0.0, n.z);
        if n.y.abs() > 0.02 || horizontal.length_squared() < 0.9 { continue; }
        let normal = horizontal.normalized();
        let [a, b, c] = [input.vertices[t * 3], input.vertices[t * 3 + 1], input.vertices[t * 3 + 2]];
        if (b - a).cross(c - a).length_squared() < 1e-8 { continue; }
        let offset = normal.dot((a + b + c) / 3.0);
        let key = (object, seed.x.to_bits(), seed.y.to_bits());
        let list = solids.entry(key).or_default();
        let found = list.iter().copied().find(|&p| planes[p].normal.dot(normal) > 0.99995 && (planes[p].offset - offset).abs() < 0.01);
        let p = found.unwrap_or_else(|| {
            let seed_bits = (seed.x.clamp(0.0, 1.0) * 16_777_216.0).round() as u32;
            let base = seed.y - 1000.0;
            let look = look(seed_bits, input.kinds[object as usize]);
            planes.push(Plane { normal, offset, triangles: vec![], object: object as usize, seed: seed_bits, record: seed_bits >> 12, look, base, height: input.tops[t * 3].abs() - 1000.0 - base, opposite: vec![] });
            list.push(planes.len() - 1);
            planes.len() - 1
        });
        planes[p].triangles.push(t);
        owner[t] = p as u32;
    }
    // Coincident planes by their canonical (x-positive) normal and offset in cm.
    let canonical = |p: &Plane| {
        let flip = p.normal.x < -1e-6 || (p.normal.x.abs() <= 1e-6 && p.normal.z < 0.0);
        let (n, d) = if flip { (-p.normal, -p.offset) } else { (p.normal, p.offset) };
        ((n.x * 1000.0).round() as i32, (n.z * 1000.0).round() as i32, (d * 100.0).round() as i64)
    };
    let mut by_plane = HashMap::<(i32, i32, i64), Vec<usize>>::new();
    for (i, p) in planes.iter().enumerate() { by_plane.entry(canonical(p)).or_default().push(i); }
    for i in 0..planes.len() {
        let (a, b, d) = canonical(&planes[i]);
        let mut opposite = Vec::new();
        for key in [(a, b, d - 1), (a, b, d), (a, b, d + 1)] {
            for &j in by_plane.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
                if planes[j].normal.dot(planes[i].normal) < -0.99995 && (planes[j].offset + planes[i].offset).abs() < 0.01 { opposite.push(j); }
            }
        }
        planes[i].opposite = opposite;
    }
    (planes, owner)
}

/// Whether (u, y) lies on the plane's own wall triangles and no coincident
/// wall facing the other way covers it there (an internal or party wall).
pub(crate) fn on_wall(input: &Input, planes: &[Plane], plane: &Plane, tangent: Vector3, u: f32, y: f32) -> bool {
    covered(input, plane, tangent, u, y) && !plane.opposite.iter().any(|&o| covered(input, &planes[o], tangent, u, y))
}
fn covered(input: &Input, plane: &Plane, tangent: Vector3, u: f32, y: f32) -> bool {
    plane.triangles.iter().any(|&t| {
        let p = [0, 1, 2].map(|i| { let v = input.vertices[t * 3 + i]; (v.dot(tangent), v.y) });
        let side = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (u - a.0);
        let (d0, d1, d2) = (side(p[0], p[1]), side(p[1], p[2]), side(p[2], p[0]));
        let e = 1e-6;
        (d0 >= -e && d1 >= -e && d2 >= -e) || (d0 <= e && d1 <= e && d2 <= e)
    })
}

impl Candidate {
    /// A box in the fixture frame (x right along the wall, y up from the
    /// origin, z out of the wall) from lo to hi, in scene metres.
    pub(crate) fn new(plane: usize, kind: f32, origin: Vector3, normal: Vector3, height: f32, bits: u32, lo: Vector3, hi: Vector3) -> Self {
        let right = Vector3::new(normal.z, 0.0, -normal.x);
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for corner in 0..8 {
            let l = Vector3::new(if corner & 1 == 0 { lo.x } else { hi.x }, if corner & 2 == 0 { lo.y } else { hi.y }, if corner & 4 == 0 { lo.z } else { hi.z });
            let p = origin + right * l.x + Vector3::UP * l.y + normal * l.z;
            for (axis, value) in [p.x, p.y, p.z].into_iter().enumerate() { min[axis] = min[axis].min(value); max[axis] = max[axis].max(value); }
        }
        let class = if kind == BLADE { 0 } else { 4 };
        Candidate { plane, kind, origin, yaw: normal.x.atan2(normal.z), height, bits: bits & 0xFF_FFFF, right, out: normal, min, max, lo, hi,
            width: 0.0, depth: 0.0, class, rank: pcg(bits ^ 0x5EED_F1C5), slot: 0, attempt: 0, group: 0 }
    }
    fn overlaps(&self, min: [f32; 3], max: [f32; 3]) -> bool { (0..3).all(|i| self.min[i] <= max[i] && self.max[i] >= min[i]) }
    /// Whether two boxes meet: their height ranges overlap and no axis of
    /// either footprint separates them (separating axes in the ground plane).
    fn meets(&self, other: &Candidate) -> bool {
        if !self.overlaps(other.min, other.max) { return false; }
        let corners = |c: &Candidate| [0, 1, 2, 3].map(|k| c.origin + c.right * if k & 1 == 0 { c.lo.x } else { c.hi.x } + c.out * if k & 2 == 0 { c.lo.z } else { c.hi.z });
        let (a, b) = (corners(self), corners(other));
        ![self.right, self.out, other.right, other.out].iter().any(|axis| {
            let span = |q: &[Vector3; 4]| q.iter().map(|p| p.dot(*axis)).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
            let ((a0, a1), (b0, b1)) = (span(&a), span(&b));
            a1 <= b0 || b1 <= a0
        })
    }
    /// Separating-axis test of a triangle against the box.
    fn hits(&self, triangle: [Vector3; 3]) -> bool {
        let centre = (self.lo + self.hi) * 0.5;
        let half = (self.hi - self.lo) * 0.5;
        let local = triangle.map(|p| { let d = p - self.origin; Vector3::new(d.dot(self.right), d.y, d.dot(self.out)) - centre });
        triangle_box(local, half)
    }
}

/// Akenine-Möller triangle/box overlap; the triangle relative to the box centre.
fn triangle_box(v: [Vector3; 3], h: Vector3) -> bool {
    let e = [v[1] - v[0], v[2] - v[1], v[0] - v[2]];
    let axes = [Vector3::RIGHT, Vector3::UP, Vector3::BACK];
    for edge in e {
        for axis in axes {
            let a = axis.cross(edge);
            if a.length_squared() < 1e-12 { continue; }
            let p = v.map(|x| x.dot(a));
            let r = h.x * a.x.abs() + h.y * a.y.abs() + h.z * a.z.abs();
            if p.iter().copied().fold(f32::MAX, f32::min) > r || p.iter().copied().fold(f32::MIN, f32::max) < -r { return false; }
        }
    }
    for (axis, extent) in [h.x, h.y, h.z].into_iter().enumerate() {
        let p = v.map(|x| [x.x, x.y, x.z][axis]);
        if p.iter().copied().fold(f32::MAX, f32::min) > extent || p.iter().copied().fold(f32::MIN, f32::max) < -extent { return false; }
    }
    let n = e[0].cross(e[1]);
    let r = h.x * n.x.abs() + h.y * n.y.abs() + h.z * n.z.abs();
    n.dot(v[0]).abs() <= r
}

/// Blade signs on one commercial wall plane: one at about 45 % of sign-slot
/// boundaries, over storeys 2–4 (the first three above the shop storey).
fn blades(input: &Input, planes: &[Plane], index: usize, out: &mut Vec<Candidate>) {
    let plane = &planes[index];
    let p = input.pitch;
    let (ground, upper) = storeys(plane.height, p);
    if upper <= 0.0 { return; }
    let bay = p * 0.8 * (0.8 + 0.5 * ((plane.look >> 9) & 7) as f32 / 7.0);
    let slot = bay * 2.0;
    let tangent = Vector3::new(-plane.normal.z, 0.0, plane.normal.x);
    let (umin, umax) = extent(input, plane, tangent);
    let bottom = ground + 0.12 * p;
    let room = (plane.height - 0.12 * p).min(ground + 3.0 * upper) - bottom;
    let record = pcg(plane.record ^ 0x0B1A_DE51);
    for k in (-umax / slot).ceil() as i64..=(-umin / slot).floor() as i64 {
        if out.len() >= MAX_CANDIDATES { return; }
        let bits = pcg(record ^ pcg(cell(k) ^ 2_654_435_769));
        if bits % 100 >= BLADE_SHARE { continue; }
        let height = (p * (0.9 + 0.1 * ((bits >> 8) % 5) as f32)).min(room);
        if height < 0.7 * p { continue; }
        let u = -(k as f32) * slot;
        let half = 0.5 * BLADE_THICKNESS * p + MARGIN * p;
        if u - half < umin || u + half > umax { continue; }
        let y = plane.base + bottom;
        if ![0.1, 0.9].iter().all(|f| on_wall(input, planes, plane, tangent, u, y + f * height)) { continue; }
        let origin = tangent * u + plane.normal * plane.offset + Vector3::UP * y;
        let lo = Vector3::new(-half, -MARGIN * p, 0.01 * p);
        let hi = Vector3::new(half, height + MARGIN * p, (BLADE_STANDOFF + BLADE_WIDTH + MARGIN) * p);
        out.push(Candidate::new(index, BLADE, origin, plane.normal, height, pcg(bits ^ 3_266_489_917), lo, hi));
    }
}

/// Air-conditioner candidates on one house, commercial or public wall plane:
/// about 14 % of upper-storey windows, hanging under the sill shadow and
/// above the window of the storey below, centred or to one side.
fn air_conditioners(input: &Input, planes: &[Plane], index: usize, concrete: bool, kind: u8, out: &mut Vec<Candidate>) {
    let plane = &planes[index];
    let p = input.pitch;
    if kind == 3 && concrete && curtain(plane.record) { return; }
    let window = pane(kind, plane.look);
    let base_bay = match kind { 2 => 1.8, 3 => 0.8, _ => 1.5 };
    let bay = p * base_bay * (0.8 + 0.5 * ((plane.look >> 9) & 7) as f32 / 7.0);
    // Rows: (base, storey height) of each storey whose window sits below the top.
    let mut rows = Vec::new();
    if kind == 3 {
        let (ground, upper) = storeys(plane.height, p);
        if upper <= 0.0 { return; }
        let count = ((plane.height - ground) / upper).round() as i64;
        for r in 2..=count { rows.push((r, ground + (r - 1) as f32 * upper, upper)); }
    } else {
        let mut r = 1;
        while (r as f32 + 1.0) * p <= plane.height + 0.01 * p { rows.push((r, r as f32 * p, p)); r += 1; }
    }
    let tangent = Vector3::new(-plane.normal.z, 0.0, plane.normal.x);
    let (umin, umax) = extent(input, plane, tangent);
    let (w, h) = (AC_WIDTH * p, AC_HEIGHT * p);
    let span = (window[1] - window[0]) * bay;
    if span < w + 0.04 * p { return; }
    let record = pcg(plane.record ^ 0x00AC_0A1C);
    for (r, row, storey) in rows {
        // Under the sill shadow (two 0.035-floor ledges below the window) and
        // over the lintel of the window below, with 0.02 floors to spare.
        let top = row + window[2] * storey - 0.08 * p;
        let floor = row - (1.0 - window[3]) * storey + 0.02 * p + AC_RACK * p;
        if top - floor < h || top > plane.height - 0.1 * p { continue; }
        let row_bits = pcg(cell(r) ^ 1_540_483_477);
        for i in (umin / bay).floor() as i64..=(umax / bay).floor() as i64 {
            if out.len() >= MAX_CANDIDATES { return; }
            let bits = pcg(record ^ pcg(cell(i) ^ row_bits));
            if bits % 100 >= AC_SHARE { continue; }
            let free = 0.5 * (span - w) - 0.02 * p;
            let shift = [0.0, -free, free][((bits >> 8) % 3) as usize];
            let u = (i as f32 + 0.5 * (window[0] + window[1])) * bay + shift;
            if u - 0.5 * w - MARGIN * p < umin || u + 0.5 * w + MARGIN * p > umax { continue; }
            let y = plane.base + top - h;
            let corners = [(-0.45, -AC_RACK * p), (0.45, -AC_RACK * p), (-0.45, h), (0.45, h)];
            if !corners.iter().all(|&(f, dy)| on_wall(input, planes, plane, tangent, u + f * w, y + dy)) { continue; }
            let origin = tangent * u + plane.normal * plane.offset + Vector3::UP * y;
            let lo = Vector3::new(-0.5 * w - MARGIN * p, -(AC_RACK + MARGIN) * p, 0.01 * p);
            let hi = Vector3::new(0.5 * w + MARGIN * p, h + MARGIN * p, (AC_STANDOFF + AC_DEPTH + MARGIN) * p);
            out.push(Candidate::new(index, AIR_CONDITIONER, origin, plane.normal, h, pcg(bits ^ 2_246_822_519), lo, hi));
        }
    }
}

fn extent(input: &Input, plane: &Plane, tangent: Vector3) -> (f32, f32) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &t in &plane.triangles {
        for i in 0..3 { let u = input.vertices[t * 3 + i].dot(tangent); lo = lo.min(u); hi = hi.max(u); }
    }
    (lo, hi)
}

/// Places the cell's facade fixtures (at most `cap`), blade signs first, each
/// kind in hash-rank order within the MAX_SQUARES busiest squares.
pub fn place(input: &Input, cap: usize) -> Placed { place_in(input, cap, MAX_SQUARES) }
fn place_in(input: &Input, cap: usize, max_squares: usize) -> Placed {
    let mut placed = Placed::default();
    if input.pitch <= 0.0 || input.vertices.len() != input.objects.len() * 3 || input.seeds.len() != input.vertices.len() || input.tops.len() != input.vertices.len() { return placed; }
    let (planes, owner) = planes(input);
    let mut candidates = Vec::new();
    for index in 0..planes.len() {
        if input.kinds[planes[index].object] == 3 { blades(input, &planes, index, &mut candidates); }
    }
    for index in 0..planes.len() {
        let object = planes[index].object;
        if !matches!(input.kinds[object], 2 | 3 | 5) { continue; }
        air_conditioners(input, &planes, index, input.concrete.get(object).copied().unwrap_or(false), input.kinds[object], &mut candidates);
    }
    crate::roof_fixtures::candidates(input, &planes, &mut candidates, MAX_CANDIDATES);
    if candidates.is_empty() { return placed; }
    // Clearance: candidates outside the cell rect, or whose box meets any
    // triangle other than their own wall's, are blocked. Candidates sit in a
    // 4 m grid; each triangle visits the candidates its bounds reach.
    let [x0, z0, x1, z1] = input.bounds;
    let mut blocked: Vec<bool> = candidates.iter().map(|c| c.min[0] < x0 || c.min[2] < z0 || c.max[0] > x1 || c.max[2] > z1).collect();
    const GRID: f32 = 4.0;
    let mut grid = HashMap::<(i32, i32), Vec<usize>>::new();
    for (i, c) in candidates.iter().enumerate() {
        for gx in (c.min[0] / GRID).floor() as i32..=(c.max[0] / GRID).floor() as i32 {
            for gz in (c.min[2] / GRID).floor() as i32..=(c.max[2] / GRID).floor() as i32 { grid.entry((gx, gz)).or_default().push(i); }
        }
    }
    let mut seen = vec![usize::MAX; candidates.len()];
    for t in 0..input.objects.len() {
        let triangle = [input.vertices[t * 3], input.vertices[t * 3 + 1], input.vertices[t * 3 + 2]];
        let min = [0, 1, 2].map(|a| triangle.iter().map(|v| [v.x, v.y, v.z][a]).fold(f32::MAX, f32::min));
        let max = [0, 1, 2].map(|a| triangle.iter().map(|v| [v.x, v.y, v.z][a]).fold(f32::MIN, f32::max));
        let mut test = |i: usize| {
            if blocked[i] || seen[i] == t || owner[t] == candidates[i].plane as u32 { return; }
            seen[i] = t;
            if candidates[i].overlaps(min, max) && candidates[i].hits(triangle) { blocked[i] = true; }
        };
        let (gx0, gx1) = ((min[0] / GRID).floor() as i64, (max[0] / GRID).floor() as i64);
        let (gz0, gz1) = ((min[2] / GRID).floor() as i64, (max[2] / GRID).floor() as i64);
        if (gx1 - gx0 + 1) * (gz1 - gz0 + 1) > candidates.len() as i64 {
            for i in 0..candidates.len() { test(i); }
        } else {
            for gx in gx0..=gx1 { for gz in gz0..=gz1 {
                if let Some(list) = grid.get(&(gx as i32, gz as i32)) { for &i in list { test(i); } }
            } }
        }
    }
    // Caps never follow generation order. Clear candidates are taken by
    // class (blade signs, penthouses, tanks, rims), then by a hash rank (of
    // their design bits on walls, so a wall's three air conditioners spread
    // over its storeys and bays; of the slot or roof on roofs); the squares
    // holding fixtures are then the `max_squares` showing the most of them
    // (ties by a hash of the square), the busiest blocks of a large cell.
    let square_of = |c: &Candidate| ((c.origin.x / SQUARE_METRES).floor() as i32, (c.origin.z / SQUARE_METRES).floor() as i32);
    let mut order: Vec<usize> = (0..candidates.len()).filter(|&i| !blocked[i]).collect();
    placed.blocked += candidates.len() - order.len();
    order.sort_by_key(|&i| (candidates[i].class, candidates[i].rank, candidates[i].attempt, i));
    let mut kept: Vec<usize> = Vec::new();
    let mut per_wall = HashMap::<usize, usize>::new();
    let mut chosen = HashMap::<u32, u32>::new();
    for i in order {
        let c = &candidates[i];
        if c.slot != 0 && chosen.get(&c.slot).is_some_and(|&a| a != c.attempt) { continue; }
        if c.kind == AIR_CONDITIONER {
            if kept.iter().any(|&a| candidates[a].overlaps(c.min, c.max)) { placed.blocked += 1; continue; }
            let count = per_wall.entry(c.plane).or_default();
            if *count >= AC_PER_WALL { continue; }
            *count += 1;
        } else if c.plane == NO_PLANE && c.class != 3 && kept.iter().any(|&a| (c.group == 0 || candidates[a].group != c.group) && candidates[a].class != 3 && candidates[a].meets(c)) {
            placed.blocked += 1;
            continue;
        }
        if c.slot != 0 { chosen.entry(c.slot).or_insert(c.attempt); }
        kept.push(i);
    }
    let mut shown = HashMap::<(i32, i32), usize>::new();
    for &i in &kept { *shown.entry(square_of(&candidates[i])).or_default() += 1; }
    let mut ranked: Vec<((i32, i32), usize)> = shown.into_iter().collect();
    ranked.sort_by_key(|&((x, z), count)| (std::cmp::Reverse(count), pcg(cell(x as i64) ^ pcg(cell(z as i64) ^ 0x0F1C_5A3E)), x, z));
    let squares: Vec<(i32, i32)> = ranked.iter().take(max_squares).map(|r| r.0).collect();
    for i in kept {
        let c = &candidates[i];
        if placed.values.len() >= cap.saturating_mul(FLOATS) || !squares.contains(&square_of(c)) { placed.capped += 1; continue; }
        placed.values.extend([c.origin.x, c.origin.y, c.origin.z, c.yaw, c.kind, input.pitch, c.height, c.bits as f32, c.width, c.depth]);
    }
    debug_assert_eq!(placed.values.len() % FLOATS, 0);
    placed
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Scene arrays of a generated chunk, as packed.rs builds them.
    pub(crate) struct Scene { pub(crate) vertices: Vec<Vector3>, pub(crate) normals: Vec<Vector3>, pub(crate) objects: Vec<i32>, pub(crate) seeds: Vec<Vector2>, pub(crate) tops: Vec<f32>,
        pub(crate) kinds: Vec<u8>, pub(crate) concrete: Vec<bool>, pub(crate) keys: Vec<u8>, pub(crate) bounds: [f32; 4] }

    pub(crate) fn scene(chunk: &mapkit_core::GeneratedChunk, bounds: &mapkit_core::Bounds) -> Scene {
        let mut ids = Vec::<String>::new();
        let mut objects = Vec::new();
        let (mut vertices, mut normals) = (Vec::new(), Vec::new());
        for triangle in &chunk.triangles {
            let (points, normal) = crate::packed::scene_triangle(triangle);
            vertices.extend(points);
            normals.extend([normal; 3]);
            let index = ids.iter().position(|id| id == &triangle.object_id).unwrap_or_else(|| { ids.push(triangle.object_id.clone()); ids.len() - 1 });
            objects.push(index as i32);
        }
        let mut kinds = vec![0u8; ids.len()];
        let mut concrete = vec![false; ids.len()];
        let mut keys = vec![255u8; ids.len()];
        let mut prisms = Vec::new();
        for prism in &chunk.building_prisms {
            let id = ids.iter().position(|id| id == &prism.object_id).unwrap();
            kinds[id] = facade_kind(prism.material.as_str(), prism.usage.as_str());
            concrete[id] = prism.material.as_str() == "concrete";
            keys[id] = crate::roof_fixtures::palette_key(prism.material.as_str(), prism.usage.as_str());
            prisms.push(id as i32);
        }
        let (seeds, tops) = crate::packed::facade_seeds(chunk, &objects, &prisms);
        let s = mapkit_core::WORLD_SCALE as f32 / 100.0;
        Scene { vertices, normals, objects, seeds, tops, kinds, concrete, keys,
            bounds: [bounds.min[0] as f32 * s, -bounds.max[1] as f32 * s, bounds.max[0] as f32 * s, -bounds.min[1] as f32 * s] }
    }

    pub(crate) fn input(s: &Scene, pitch: f32) -> Input<'_> {
        Input { vertices: &s.vertices, normals: &s.normals, objects: &s.objects, seeds: &s.seeds, tops: &s.tops, kinds: &s.kinds, concrete: &s.concrete, keys: &s.keys, pitch, bounds: s.bounds }
    }

    /// The minimal example with the given buildings, generated at cell (0, 0).
    pub(crate) fn generated(buildings: &[(&str, [i64; 4], i64, &str, &str)]) -> (mapkit_core::GeneratedChunk, mapkit_core::Bounds) {
        let rings: Vec<(&str, Vec<[i64; 2]>, i64, &str, &str)> = buildings.iter().map(|(id, [x0, y0, x1, y1], height, usage, material)| (*id, vec![[*x0, *y0], [*x1, *y0], [*x1, *y1], [*x0, *y1]], *height, *usage, *material)).collect();
        generated_footprints(&rings)
    }

    /// The minimal example with buildings of the given footprints, at cell (0, 0).
    pub(crate) fn generated_footprints(buildings: &[(&str, Vec<[i64; 2]>, i64, &str, &str)]) -> (mapkit_core::GeneratedChunk, mapkit_core::Bounds) {
        let mut d: mapkit_core::MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
        let template = d.buildings[0].clone();
        d.buildings = buildings.iter().map(|(id, footprint, height, usage, material)| {
            let mut b = template.clone();
            b.id = id.to_string();
            b.footprint = footprint.clone();
            b.height_cm = *height as _;
            b.usage = serde_json::from_value(serde_json::json!(usage)).unwrap();
            b.material = serde_json::from_value(serde_json::json!(material)).unwrap();
            b
        }).collect();
        let cell = mapkit_core::Cell { x: 0, y: 0 };
        let chunk = mapkit_core::generate(mapkit_core::GenerationInput { document: &d, cell, heightgrid: None, max_triangles: 500_000 }).unwrap();
        (chunk, d.cell_bounds(cell).unwrap())
    }

    pub(crate) fn fixtures(values: &[f32]) -> Vec<&[f32]> { values.chunks(FLOATS).collect() }
    /// Wall fixtures only (blade signs and air-conditioner units).
    fn walls(values: &[f32]) -> Vec<&[f32]> { values.chunks(FLOATS).filter(|f| f[4] <= AIR_CONDITIONER).collect() }

    #[test]
    fn pcg_matches_the_shader_hash() {
        // facade_pcg of facade_signs.gdshaderinc, evaluated by hand.
        let s = 1u32.wrapping_mul(747796405).wrapping_add(2891336453);
        let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277803737);
        assert_eq!(pcg(1), (w >> 22) ^ w);
        assert_eq!(cell(-1), 1048575);
    }

    #[test]
    fn commercial_finish_weights_keep_file_order() {
        let names: Vec<&str> = commercial_finishes().iter().map(|w| w.0.as_str()).collect();
        assert_eq!(names, ["plaster", "concrete", "stone", "metal", "curtain"]);
        let table: serde_json::Value = serde_json::from_str(include_str!("../../../godot/facade_palette.json")).unwrap();
        for (name, weight) in commercial_finishes() {
            assert_eq!(table["finishes"]["weights"]["concrete:commercial"][name].as_f64(), Some(*weight));
        }
        let curtains = (0..4096).filter(|&r| curtain(r)).count();
        assert!((900..1150).contains(&curtains), "about a quarter of records are curtain walls: {curtains}");
    }

    #[test]
    fn commercial_walls_get_outward_blade_signs_and_houses_none() {
        // A 20 m x 40 m commercial block 15 m tall (1:3 cards: 100 cm floors),
        // and a house and an apartment block far from it.
        let (chunk, bounds) = generated(&[("shop", [30000, 30000, 32000, 34000], 1500, "commercial", "concrete"),
            ("home", [10000, 10000, 11200, 10800], 500, "residential", "brick"),
            ("flats", [40000, 40000, 42000, 42000], 3000, "residential", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        let list = fixtures(&placed.values);
        let blades: Vec<_> = list.iter().filter(|f| f[4] == BLADE).collect();
        assert!(blades.len() >= 8, "blade signs on the shop's walls: {}", blades.len());
        for f in &blades {
            let (x, z) = (f[0], -f[2]);
            assert!((300.0..=320.0).contains(&x) && (300.0..=340.0).contains(&z), "on the shop: {f:?}");
            assert!([300.0, 320.0].iter().any(|w| (x - w).abs() < 1e-3) || [300.0, 340.0].iter().any(|w| (z - w).abs() < 1e-3), "on an outer wall face: {f:?}");
            // Local +z (sin yaw, cos yaw) leaves the footprint.
            let out = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let (ox, oz) = (x + out.x * 0.5, -(f[2] + out.z * 0.5));
            assert!(!((300.0..320.0).contains(&ox) && (300.0..340.0).contains(&oz)), "faces out of the wall: {f:?}");
            assert!(f[1] >= 1.5 && f[1] + f[6] <= 15.0 - 0.1 && (0.7..=1.3).contains(&f[6]), "storeys 2-4 below the top: {f:?}");
            assert!(f[5] == 1.0 && f[7] < 16_777_216.0 && f[7] == f[7].round());
        }
        // Blade signs stand on sign-slot boundaries of the shader layout.
        for f in &blades {
            let n = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let u = Vector3::new(f[0], f[1], f[2]).dot(Vector3::new(-n.z, 0.0, n.x));
            let seed = s.seeds[s.objects.iter().position(|&o| s.kinds[o as usize] == 3).unwrap() * 3].x;
            let look = look((seed * 16_777_216.0).round() as u32, 3);
            let slot = 2.0 * 0.8 * (0.8 + 0.5 * ((look >> 9) & 7) as f32 / 7.0);
            assert!(((-u / slot) - (-u / slot).round()).abs() < 1e-3, "on a slot boundary: {u} / {slot}");
        }
        for f in &walls(&placed.values) {
            let (x, z) = (f[0], -f[2]);
            assert!(!((395.0..=425.0).contains(&x) && (395.0..=425.0).contains(&z)), "nothing on the apartment block: {f:?}");
            if (95.0..=115.0).contains(&x) { assert_eq!(f[4], AIR_CONDITIONER, "houses carry no blade signs"); }
        }
        assert_eq!(place(&input(&s, 1.0), MAX_FIXTURES), placed, "deterministic");
    }

    #[test]
    fn air_conditioners_hang_under_windows_of_houses_and_public_walls() {
        let mut found = 0;
        for (i, usage) in ["public", "residential"].iter().enumerate() {
            let material = if *usage == "residential" { "brick" } else { "concrete" };
            let buildings: Vec<(String, [i64; 4])> = (0..12).map(|k| (format!("b{i}-{k}"), [27000 + k * 1600, 10000, 28200 + k * 1600, 13000])).collect();
            let list: Vec<_> = buildings.iter().map(|(id, r)| (id.as_str(), *r, 900i64, *usage, material)).collect();
            let (chunk, bounds) = generated(&list);
            let s = scene(&chunk, &bounds);
            let placed = place(&input(&s, 1.0), MAX_FIXTURES);
            assert!(fixtures(&placed.values).iter().all(|f| f[4] != BLADE), "{usage} walls carry no blade signs");
            for f in walls(&placed.values) {
                assert!(f[1] >= 0.9 && f[1] + f[6] <= 9.0, "an upper storey: {f:?}");
                // Under the sill shadow of its storey (house/public rows are one floor).
                let top = f[1] + f[6];
                let pane_z = top - top.floor();
                assert!(pane_z <= 0.38 - 0.08 + 1e-3, "below the window: {f:?}");
                found += 1;
            }
        }
        assert!(found >= 4, "a few units: {found}");
    }

    #[test]
    fn a_neighbouring_box_or_the_cell_edge_blocks_a_fixture() {
        let (chunk, bounds) = generated(&[("shop", [30000, 30000, 32000, 34000], 1500, "commercial", "concrete")]);
        let mut s = scene(&chunk, &bounds);
        let free = place(&input(&s, 1.0), MAX_FIXTURES);
        let first = fixtures(&free.values)[0].to_vec();
        // A thin vertical sheet 10 cm out from the first fixture's wall, across it.
        let n = Vector3::new(first[3].sin(), 0.0, first[3].cos());
        let right = Vector3::new(n.z, 0.0, -n.x);
        let o = Vector3::new(first[0], first[1], first[2]) + n * 0.1;
        let sheet = [o - right * 0.5, o + right * 0.5 + Vector3::UP * 2.0, o + right * 0.5];
        s.vertices.extend(sheet);
        s.normals.extend([n; 3]);
        s.objects.push(s.kinds.len() as i32);
        s.kinds.push(0);
        s.concrete.push(false);
        s.seeds.extend([Vector2::ZERO; 3]);
        s.tops.extend([0.0; 3]);
        let blocked = place(&input(&s, 1.0), MAX_FIXTURES);
        assert!(!fixtures(&blocked.values).iter().any(|f| f[..3] == first[..3]), "the sheet blocks the fixture");
        assert_eq!(blocked.blocked, free.blocked + 1);
        // A cell rect that ends inside the building keeps no fixture across it.
        let mut cut = input(&s, 1.0);
        cut.bounds[2] = 310.0;
        assert!(fixtures(&place(&cut, MAX_FIXTURES).values).iter().all(|f| f[0] < 310.0));
    }

    #[test]
    fn the_cap_and_non_facade_solids() {
        let (chunk, bounds) = generated(&[("shop", [30000, 30000, 32000, 34000], 1500, "commercial", "concrete")]);
        let s = scene(&chunk, &bounds);
        let all = place(&input(&s, 1.0), MAX_FIXTURES);
        let capped = place(&input(&s, 1.0), 3);
        assert_eq!(capped.values.len(), 3 * FLOATS);
        assert!(capped.capped >= all.values.len() / FLOATS - 3, "reports the capped fixtures: {} of {}", capped.capped, all.values.len() / FLOATS);
        assert_eq!(capped.values[..], all.values[..3 * FLOATS], "the cap keeps the first fixtures");
        // Industrial and apartment walls, and solids without facade seeds, get none.
        for (usage, material) in [("industrial", "concrete"), ("residential", "concrete")] {
            let (chunk, bounds) = generated(&[("b", [30000, 30000, 32000, 34000], 1500, usage, material)]);
            let s = scene(&chunk, &bounds);
            assert!(walls(&place(&input(&s, 1.0), MAX_FIXTURES).values).is_empty(), "{usage} {material}");
        }
        let mut bare = scene(&chunk, &bounds);
        bare.seeds.iter_mut().for_each(|v| *v = Vector2::ZERO);
        assert!(place(&input(&bare, 1.0), MAX_FIXTURES).values.is_empty(), "no seeds, no fixtures");
    }

    #[test]
    fn a_large_cell_keeps_its_busiest_squares_whatever_the_building_order() {
        // A 512 m cell: 42 commercial blocks (four long ones), each in its own
        // 32 m square (64 m apart, clear of the example's two roads at 256 m),
        // more squares than MAX_SQUARES.
        let at = |k: i64| { let grid = [0i64, 6400, 12800, 19200, 32000, 38400, 44800]; (grid[(k % 7) as usize], grid[(k / 7) as usize]) };
        let mut buildings = Vec::new();
        for k in 0..42i64 {
            let (x, y) = at(k);
            let r = if k % 11 == 0 { [x + 100, y + 600, x + 3100, y + 2600] } else { [x + 1000, y + 1000, x + 1800, y + 1800] };
            buildings.push((format!("b{k}"), r));
        }
        let run = |list: &[(String, [i64; 4])], cap: usize, squares: usize| {
            let rows: Vec<_> = list.iter().map(|(id, r)| (id.as_str(), *r, 1500i64, "commercial", "brick")).collect();
            let (chunk, bounds) = generated(&rows);
            let s = scene(&chunk, &bounds);
            place_in(&input(&s, 1.0), cap, squares)
        };
        let placed = run(&buildings, MAX_FIXTURES, MAX_SQUARES);
        let square = |f: &[f32]| ((f[0] / SQUARE_METRES).floor() as i32, (f[2] / SQUARE_METRES).floor() as i32);
        let count = |p: &Placed| { let mut m = HashMap::<(i32, i32), usize>::new(); for f in fixtures(&p.values) { *m.entry(square(f)).or_default() += 1; } m };
        assert_eq!(count(&placed).len(), MAX_SQUARES, "the square cap binds: {} capped", placed.capped);
        assert!(placed.capped > 0);
        // The kept squares are the busiest of the uncapped placement.
        let all = count(&run(&buildings, usize::MAX, usize::MAX));
        let chosen = count(&run(&buildings, usize::MAX, MAX_SQUARES));
        assert!(all.len() > MAX_SQUARES && chosen.len() == MAX_SQUARES);
        let least = chosen.keys().map(|k| all[k]).min().unwrap();
        assert!(all.iter().filter(|(k, _)| !chosen.contains_key(k)).all(|(_, n)| *n <= least), "busiest squares kept: {chosen:?} of {all:?}");
        assert!(chosen.iter().all(|(k, n)| all[k] == *n), "a kept square keeps all its fixtures");
        let mut reversed = buildings.clone();
        reversed.reverse();
        let sorted = |p: &Placed| { let mut v: Vec<Vec<u32>> = fixtures(&p.values).iter().map(|f| f.iter().map(|x| x.to_bits()).collect()).collect(); v.sort(); v };
        assert_eq!(sorted(&run(&reversed, MAX_FIXTURES, MAX_SQUARES)), sorted(&placed), "the same fixtures in any building order");
    }

    #[test]
    fn a_wall_spreads_its_air_conditioners_over_its_storeys() {
        // Long, tall public walls: far more clear candidates than three a wall.
        let buildings: Vec<(String, [i64; 4])> = (0..6).map(|k| (format!("p{k}"), [3000 + k * 6000, 10000, 6000 + k * 6000, 11000])).collect();
        let rows: Vec<_> = buildings.iter().map(|(id, r)| (id.as_str(), *r, 1200i64, "public", "concrete")).collect();
        let (chunk, bounds) = generated(&rows);
        let s = scene(&chunk, &bounds);
        let units: Vec<f32> = fixtures(&place(&input(&s, 1.0), MAX_FIXTURES).values).iter().filter(|f| f[4] == AIR_CONDITIONER).map(|f| f[1]).collect();
        assert!(units.len() >= 12, "units on most walls: {}", units.len());
        let high = units.iter().filter(|y| **y > 4.0).count();
        assert!(3 * high >= units.len(), "kept units reach the upper storeys: {high} of {}", units.len());
    }

    #[test]
    fn fixture_sizes_match_the_godot_meshes() {
        let meshes = include_str!("../../../godot/facade_fixture_meshes.gd");
        for (name, value) in [("BLADE_WIDTH", BLADE_WIDTH), ("BLADE_THICKNESS", BLADE_THICKNESS), ("BLADE_STANDOFF", BLADE_STANDOFF),
            ("AC_WIDTH", AC_WIDTH), ("AC_HEIGHT", AC_HEIGHT), ("AC_DEPTH", AC_DEPTH), ("AC_STANDOFF", AC_STANDOFF), ("AC_RACK", AC_RACK)] {
            assert!(meshes.contains(&format!("const {name} := {value}")), "{name} = {value} in facade_fixture_meshes.gd");
        }
        assert!(meshes.contains(&format!("const FLOATS := {FLOATS}")) && meshes.contains(&format!("const SQUARE_METRES := {SQUARE_METRES:.1}")));
    }
}
