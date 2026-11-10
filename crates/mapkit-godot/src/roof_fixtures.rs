//! Display-only rooftop fixtures of one packed cell, placed with the facade
//! fixtures (facade_fixtures.rs: the same clearance test, caps and packed
//! list) on the flat roof faces of generated building solids: stair
//! penthouses with a door on taller roofs, water tanks (a square FRP panel
//! tank on a steel stand or a round stainless one), rows of air-conditioner
//! condensers, solar panel rows on low roofs, an antenna mast or a satellite
//! dish now and then, and a low parapet rim along the roof perimeter (with a
//! guard railing on part of residential and commercial roofs).
//!
//! Inputs are the packed display arrays only. Flat roof faces carry their
//! solid's roof frame in their facade top (packed.rs `roof_frame`: the
//! walls' direction modulo 90°); fixtures stand square to it, at positions
//! from integer PCG hashes of the 24-bit solid seed, the roof height and
//! slot and attempt indices. Each slot (one penthouse, one tank, one
//! condenser group, one solar array, one mast) lists a few alternative
//! positions whose footprint (plus EDGE_CLEARANCE) lies on the roof; the
//! first alternative clear of other triangles and of fixtures kept before it
//! wins. Rims stand on the roof edges that have a facade wall of the same
//! solid under them (never on cell cuts or party walls); at square corners,
//! convex or reflex, neighbouring rims meet without a gap or an overlap.
//!
//! Sizes are in floor pitches (metres at 1:3) and match
//! `godot/facade_fixture_meshes.gd`.
use crate::facade_fixtures::{cell, pcg, unit, on_wall, Candidate, Input, Plane, MARGIN};
use godot::builtin::Vector3;
use std::collections::HashMap;

pub const PENTHOUSE: f32 = 2.0;
pub const TANK: f32 = 3.0;
pub const CONDENSER: f32 = 4.0;
pub const EDGE: f32 = 5.0;
pub const SOLAR: f32 = 6.0;
pub const MAST: f32 = 7.0;
/// Parapet rim (floor pitches): its outer face RIM_INSET inside the wall
/// plane, RIM_THICKNESS deep and RIM_HEIGHT high; a guard railing adds
/// RAIL_HEIGHT above it.
pub const RIM_INSET: f32 = 0.015;
pub const RIM_THICKNESS: f32 = 0.06;
pub const RIM_HEIGHT: f32 = 0.12;
pub const RAIL_HEIGHT: f32 = 0.34;
/// Longest rim instance (longer edges are split evenly) and the shortest
/// rim kept (shorter runs leave a gap).
pub const RIM_SEGMENT: f32 = 24.0;
pub const RIM_SHORTEST: f32 = 0.5;
/// Condenser unit (one fixture each): footprint, height and the spacing of
/// a group's units along it.
pub const CONDENSER_WIDTH: f32 = 0.3;
pub const CONDENSER_DEPTH: f32 = 0.22;
pub const CONDENSER_HEIGHT: f32 = 0.3;
pub const CONDENSER_SPACING: f32 = 0.38;
/// Solar row: tilted panels SOLAR_DEPTH deep, SOLAR_HEIGHT at the back.
pub const SOLAR_DEPTH: f32 = 0.5;
pub const SOLAR_HEIGHT: f32 = 0.36;
pub const SOLAR_ROW_SPACING: f32 = 0.8;
/// Mast footprint and heights (antenna, dish).
pub const MAST_FOOTPRINT: f32 = 0.3;
pub const ANTENNA_HEIGHT: f32 = 0.85;
pub const DISH_HEIGHT: f32 = 0.42;
/// Interior fixtures keep this far (floor pitches) inside the roof outline.
pub const EDGE_CLEARANCE: f32 = 0.2;
/// Alternative positions tried per slot, and kept per slot (on the roof).
const ATTEMPTS: u32 = 24;
const ALTERNATIVES: usize = 4;

/// Palette key of a building material and use (fixture shader slots): 0..3
/// concrete residential, commercial, industrial, public; 4 brick; 5 wood.
pub fn palette_key(material: &str, usage: &str) -> u8 {
    match material {
        "brick" => 4,
        "wood" => 5,
        "concrete" => match usage { "commercial" => 1, "industrial" => 2, "public" => 3, _ => 0 },
        _ => 255,
    }
}

/// One flat roof plane: a solid's horizontal roof triangles at one height.
/// Solids come from facade_fixtures.rs `solids`, not the seed: separate solids
/// of one record with the same top share their seed but keep their own roofs.
struct Roof { triangles: Vec<usize>, object: usize, solid: usize, seed: u32, height: f32, base: f32, frame: f32 }

fn roofs(input: &Input, solid_of: &[usize]) -> Vec<Roof> {
    let mut roofs: Vec<Roof> = Vec::new();
    let mut index = HashMap::<(usize, i64), usize>::new();
    for (t, &object) in input.objects.iter().enumerate() {
        if input.kinds.get(object as usize).copied().unwrap_or(0) == 0 { continue; }
        let top = input.tops[t * 3];
        let seed = input.seeds[t * 3];
        if top < crate::packed::ROOF_FRAME_OFFSET - 500.0 || seed.y <= 0.5 { continue; }
        let y = input.vertices[t * 3].y;
        let key = (solid_of[t], (y * 1000.0).round() as i64);
        let r = *index.entry(key).or_insert_with(|| {
            roofs.push(Roof { triangles: vec![], object: object as usize, solid: solid_of[t], seed: (seed.x.clamp(0.0, 1.0) * 16_777_216.0).round() as u32,
                height: y, base: seed.y - 1000.0, frame: top - crate::packed::ROOF_FRAME_OFFSET });
            roofs.len() - 1
        });
        roofs[r].triangles.push(t);
    }
    roofs
}

struct Frame { u: Vector3, v: Vector3 }
impl Frame {
    fn new(a: f32) -> Self { Frame { u: Vector3::new(a.cos(), 0.0, a.sin()), v: Vector3::new(-a.sin(), 0.0, a.cos()) } }
    fn at(&self, u: f32, v: f32, y: f32) -> Vector3 { self.u * u + self.v * v + Vector3::UP * y }
    /// The four directions square to the frame: 0 +u, 1 +v, 2 -u, 3 -v.
    fn facing(&self, k: u32) -> Vector3 { [self.u, self.v, -self.u, -self.v][(k & 3) as usize] }
}

/// Whether the scene point (x, z) lies on one of the roof's triangles.
fn on_roof(input: &Input, roof: &Roof, x: f32, z: f32) -> bool {
    roof.triangles.iter().any(|&t| {
        let p = [0, 1, 2].map(|i| { let v = input.vertices[t * 3 + i]; (v.x, v.z) });
        let side = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (z - a.1) - (b.1 - a.1) * (x - a.0);
        let (d0, d1, d2) = (side(p[0], p[1]), side(p[1], p[2]), side(p[2], p[0]));
        let e = 1e-6;
        (d0 >= -e && d1 >= -e && d2 >= -e) || (d0 <= e && d1 <= e && d2 <= e)
    })
}

/// Whether a footprint (centre, right and out axes, half sizes) grown by
/// `margin` lies on the roof: a 4 × 4 grid of sample points over it.
fn fits(input: &Input, roof: &Roof, centre: Vector3, right: Vector3, out: Vector3, half_x: f32, half_z: f32, margin: f32) -> bool {
    (0..16).all(|k| {
        let (i, j) = ((k % 4) as f32 / 3.0 * 2.0 - 1.0, (k / 4) as f32 / 3.0 * 2.0 - 1.0);
        let p = centre + right * (i * (half_x + margin)) + out * (j * (half_z + margin));
        on_roof(input, roof, p.x, p.z)
    })
}

/// The 24-bit design bits of a rooftop fixture: bits 0..2 the palette key,
/// 3..5 the solid look's body and 6..8 its roof palette entries
/// (facade_fixtures.rs `look`, as the walls and roof under it), 9..10 the
/// mesh variant, 11..23 a design hash.
fn design(key: u8, look: u32, variant: u32, hash: u32) -> u32 {
    (key.min(7) as u32) | ((look & 7) << 3) | (((look >> 3) & 7) << 6) | ((variant & 3) << 9) | ((hash & 0x1FFF) << 11)
}

struct Slot<'a> { input: &'a Input<'a>, roof: &'a Roof, frame: &'a Frame, hash: u32, extent: [f32; 4] }

impl Slot<'_> {
    /// Alternative positions of one slot: up to ALTERNATIVES attempts whose
    /// footprint (w × d around its centre, facing one frame direction) lies
    /// on the roof inside EDGE_CLEARANCE, within the slot's share
    /// `[from, to)` of the roof's longer frame axis.
    fn positions(&self, salt: u32, w: f32, d: f32, from: f32, to: f32, facing: Option<u32>) -> Vec<(u32, Vector3, Vector3)> {
        let p = self.input.pitch;
        let [u0, u1, v0, v1] = self.extent;
        let long_u = u1 - u0 >= v1 - v0;
        let mut found = Vec::new();
        for attempt in 0..ATTEMPTS {
            if found.len() >= ALTERNATIVES { break; }
            let h = pcg(self.hash ^ pcg(salt ^ pcg(cell(attempt as i64) ^ 0x7A3C_11E5)));
            let face = facing.unwrap_or(h & 3);
            let out = self.frame.facing(face);
            // The footprint's extent along the frame axes.
            let (eu, ev) = if face & 1 == 0 { (d, w) } else { (w, d) };
            let (a, b) = (unit(h ^ 0x1B87_3593), unit(h ^ 0x0E64_54E1));
            let (mut lo_u, mut hi_u, mut lo_v, mut hi_v) = (u0 + 0.5 * eu, u1 - 0.5 * eu, v0 + 0.5 * ev, v1 - 0.5 * ev);
            if long_u { let s = u1 - u0; lo_u = lo_u.max(u0 + from * s); hi_u = hi_u.min(u0 + to * s); }
            else { let s = v1 - v0; lo_v = lo_v.max(v0 + from * s); hi_v = hi_v.min(v0 + to * s); }
            if hi_u < lo_u || hi_v < lo_v { continue; }
            let centre = self.frame.at(lo_u + a * (hi_u - lo_u), lo_v + b * (hi_v - lo_v), self.roof.height);
            let right = Vector3::new(out.z, 0.0, -out.x);
            if fits(self.input, self.roof, centre, right, out, 0.5 * w, 0.5 * d, EDGE_CLEARANCE * p) { found.push((attempt, centre, out)); }
        }
        found
    }

    /// A box fixture on the roof (lo/hi in its frame, y up from the roof).
    /// `salt` names the slot within its roof (its rank), `slot` and `group`
    /// are its cell-wide identities.
    fn candidate(&self, kind: f32, origin: Vector3, out: Vector3, size: [f32; 3], bits: u32, class: u8, salt: u32, slot: u32, attempt: u32, group: u32) -> Candidate {
        let p = self.input.pitch;
        let [w, h, d] = size;
        let m = MARGIN * p;
        let lo = Vector3::new(-0.5 * w - m, 0.01 * p, -0.5 * d - m);
        let hi = Vector3::new(0.5 * w + m, h + m, 0.5 * d + m);
        let mut c = Candidate::new(crate::facade_fixtures::NO_PLANE, kind, origin, out, h, bits, lo, hi);
        c.width = w;
        c.depth = d;
        c.class = class;
        c.slot = slot;
        c.attempt = attempt;
        c.group = group;
        c.rank = pcg(self.hash ^ pcg(salt ^ 0x0C0F_FEE5));
        c
    }
}

/// Rooftop candidates of every flat roof of the cell (after the wall ones).
pub(crate) fn candidates(input: &Input, planes: &[Plane], solid_of: &[usize], out: &mut Vec<Candidate>, max: usize) {
    let p = input.pitch;
    let mut next_slot = 1u32;
    for roof in roofs(input, solid_of) {
        if out.len() >= max { return; }
        let kind = input.kinds[roof.object];
        let key = input.keys.get(roof.object).copied().unwrap_or(255);
        let look = crate::facade_fixtures::look(roof.seed, crate::facade_fixtures::share(input, roof.object));
        let frame = Frame::new(roof.frame);
        let storeys = (roof.height - roof.base) / p;
        // Extent in the frame and area.
        let (mut extent, mut area) = ([f32::MAX, f32::MIN, f32::MAX, f32::MIN], 0.0);
        for &t in &roof.triangles {
            let [a, b, c] = [0, 1, 2].map(|i| input.vertices[t * 3 + i]);
            area += 0.5 * (b - a).cross(c - a).length();
            for q in [a, b, c] {
                let (u, v) = (q.dot(frame.u), q.dot(frame.v));
                extent = [extent[0].min(u), extent[1].max(u), extent[2].min(v), extent[3].max(v)];
            }
        }
        let floors = area / (p * p);
        let hash = pcg(roof.seed ^ pcg(cell((roof.height * 100.0).round() as i64) ^ 0x2F00_F1A7));
        let slot = Slot { input, roof: &roof, frame: &frame, hash, extent };
        let chance = |salt: u32, percent: u32| pcg(hash ^ salt) % 100 < percent;
        let add = |list: Vec<Candidate>, out: &mut Vec<Candidate>| { for c in list { if out.len() < max { out.push(c); } } };
        // Stair penthouses: on roofs three storeys up or more; long apartment
        // slabs have one per stair core (up to three along their length).
        if storeys >= 2.9 && floors >= 5.0 {
            let count = if kind == 1 { 1 + (floors >= 70.0) as u32 + (floors >= 160.0) as u32 } else { 1 };
            for s in 0..count {
                let h = pcg(hash ^ pcg(cell(s as i64) ^ 0x5E17_0001));
                let w = p * (0.95 + 0.1 * (h % 4) as f32);
                let d = p * (1.2 + 0.1 * ((h >> 2) % 5) as f32);
                let tall = p * (0.88 + 0.04 * ((h >> 5) % 4) as f32);
                let id = next_slot; next_slot += 1;
                let list = slot.positions(0x5E17_0000 ^ s, w, d, s as f32 / count as f32, (s + 1) as f32 / count as f32, None).into_iter()
                    .map(|(attempt, centre, facing)| slot.candidate(PENTHOUSE, centre, facing, [w, tall, d], design(key, look, 0, h >> 8), 1, 0x5E17_0000 ^ s, id, attempt, 0)).collect();
                add(list, out);
            }
        }
        // Water tanks: square FRP panel tanks (2 or 3 panels a side) or round
        // stainless ones on apartments, offices and public roofs four storeys
        // up or more; a small round tank on some houses and workshops.
        let tanks = match kind {
            1 | 3 | 5 if storeys >= 3.9 && floors >= 8.0 => 1 + (floors >= 60.0 && chance(0x7A11_0002, 50)) as u32,
            2 if storeys >= 2.9 && floors >= 4.0 && chance(0x7A11_0003, 40) => 1,
            4 if floors >= 8.0 && chance(0x7A11_0004, 30) => 1,
            _ => 0,
        };
        for s in 0..tanks {
            let h = pcg(hash ^ pcg(cell(s as i64) ^ 0x7A11_0001));
            let round = kind == 2 || h % 100 < 35;
            let (w, d, tall) = if round {
                let r = if kind == 2 { p * (0.38 + 0.04 * ((h >> 8) % 3) as f32) } else { p * (0.5 + 0.06 * ((h >> 8) % 4) as f32) };
                (r, r, if kind == 2 { p * 0.55 } else { p * (0.75 + 0.05 * ((h >> 12) % 3) as f32) })
            } else {
                let panels = |b: u32| p * if (h >> b) & 1 == 0 { 2.0 / 3.0 } else { 1.0 };
                (panels(8), panels(9), p * (0.12 + if (h >> 10) & 1 == 0 { 2.0 / 3.0 } else { 1.0 }))
            };
            let id = next_slot; next_slot += 1;
            let list = slot.positions(0x7A11_0000 ^ s, w, d, 0.0, 1.0, None).into_iter()
                .map(|(attempt, centre, facing)| slot.candidate(TANK, centre, facing, [w, tall, d], design(key, look, round as u32, h >> 13), 2, 0x7A11_0000 ^ s, id, attempt, 0)).collect();
            add(list, out);
        }
        // Condenser groups: a row of 2-5 units on commercial, public and
        // industrial roofs, one or two on some apartments and houses.
        let (groups, units) = match kind {
            3 if chance(0xC0DE_0002, 70) => (1 + (floors >= 40.0 && chance(0xC0DE_0003, 50)) as u32, (2, 5)),
            5 if chance(0xC0DE_0004, 60) => (1, (2, 4)),
            4 if chance(0xC0DE_0005, 50) => (1, (2, 3)),
            1 if chance(0xC0DE_0006, 15) => (1, (1, 2)),
            2 if chance(0xC0DE_0007, 25) => (1, (1, 1)),
            _ => (0, (0, 0)),
        };
        for s in 0..groups {
            let h = pcg(hash ^ pcg(cell(s as i64) ^ 0xC0DE_0001));
            let n = units.0 + h % (units.1 - units.0 + 1);
            let variant = (h >> 4) & 1; // 0 side-blow, 1 top-blow
            let w = p * (CONDENSER_SPACING * (n - 1) as f32 + CONDENSER_WIDTH);
            let d = p * CONDENSER_DEPTH;
            let id = next_slot; next_slot += 1;
            let mut list = Vec::new();
            for (attempt, centre, facing) in slot.positions(0xC0DE_0000 ^ s, w, d, 0.0, 1.0, None) {
                let right = Vector3::new(facing.z, 0.0, -facing.x);
                for k in 0..n {
                    let offset = p * CONDENSER_SPACING * (k as f32 - 0.5 * (n - 1) as f32);
                    let bits = design(key, look, variant, pcg(h ^ k) >> 8);
                    list.push(slot.candidate(CONDENSER, centre + right * offset, facing, [p * CONDENSER_WIDTH, p * CONDENSER_HEIGHT, d], bits, 4, 0xC0DE_0000 ^ s, id, attempt, id));
                }
            }
            add(list, out);
        }
        // Solar arrays on low roofs: 1-3 rows facing the frame direction
        // nearest the scene's south (+z), rows 1.2-3 floors long.
        let solar = match kind { 2 | 4 | 5 => 35, 1 => 8, 3 => 12, _ => 0 };
        if storeys <= 5.5 && floors >= 6.0 && chance(0x5014_0002, solar) {
            let h = pcg(hash ^ 0x5014_0001);
            let rows = 1 + h % 3;
            let length = p * (1.2 + 0.3 * ((h >> 2) % 7) as f32);
            let face = (0..4u32).max_by(|&a, &b| frame.facing(a).z.total_cmp(&frame.facing(b).z)).unwrap();
            let d = p * (SOLAR_ROW_SPACING * (rows - 1) as f32 + SOLAR_DEPTH);
            let id = next_slot; next_slot += 1;
            let mut list = Vec::new();
            for (attempt, centre, facing) in slot.positions(0x5014_0000, length, d, 0.0, 1.0, Some(face)) {
                for k in 0..rows {
                    let offset = p * SOLAR_ROW_SPACING * (k as f32 - 0.5 * (rows - 1) as f32);
                    // Rows are instanced from their left end (mesh x 0..1).
                    let right = Vector3::new(facing.z, 0.0, -facing.x);
                    let mut c = slot.candidate(SOLAR, centre + facing * offset, facing, [length, p * SOLAR_HEIGHT, p * SOLAR_DEPTH], design(key, look, 0, pcg(h ^ k) >> 8), 4, 0x5014_0000, id, attempt, id);
                    c.origin -= right * (0.5 * length);
                    c.lo.x += 0.5 * length;
                    c.hi.x += 0.5 * length;
                    list.push(c);
                }
            }
            add(list, out);
        }
        // An antenna mast or a satellite dish on about 15 % of roofs.
        if storeys >= 1.9 && floors >= 3.0 && chance(0x3A57_0002, 15) {
            let h = pcg(hash ^ 0x3A57_0001);
            let dish = h % 100 < 40;
            let side = p * MAST_FOOTPRINT;
            let id = next_slot; next_slot += 1;
            let list = slot.positions(0x3A57_0000, side, side, 0.0, 1.0, None).into_iter()
                .map(|(attempt, centre, facing)| slot.candidate(MAST, centre, facing, [side, p * if dish { DISH_HEIGHT } else { ANTENNA_HEIGHT }, side], design(key, look, dish as u32, h >> 8), 4, 0x3A57_0000, id, attempt, 0)).collect();
            add(list, out);
        }
        // Parapet rims on the perimeter edges, a guard railing on about 35 %
        // of residential and commercial roofs.
        let railing = matches!(kind, 1 | 2 | 3) && chance(0xED6E_0002, 35);
        let edge_rank = pcg(hash ^ 0xED6E_0001);
        let edges = perimeter(input, planes, &roof);
        for (e, &(a, b, inward)) in edges.iter().enumerate() {
            if out.len() >= max { return; }
            let length = (b - a).length();
            let along = (b - a) / length;
            // Corners: keep ends on the cell rect just inside it and run on
            // where a collinear rim continues. At a square corner (turn within
            // about 26° of 90°) the rims meet without a gap or an overlap: at a
            // convex corner this rim ends at the next one's inner face and the
            // next starts at this one's outer face; at a reflex corner this one
            // runs on across the next one's strip, which starts at its outer
            // face. Other corners trim the end by a thickness.
            let next = edges.iter().find(|&&(c, _, _)| (c - b).length() < 1e-3).map(|&(c, d, _)| (d - c).normalized());
            let previous = edges.iter().find(|&&(_, d, _)| (d - a).length() < 1e-3).map(|&(c, d, w)| ((d - c).normalized(), w));
            let continues = next.is_some_and(|d| d.dot(along) > 0.9999);
            let square = |d: Vector3, e: Vector3| d.dot(e).abs() < 0.45;
            let on_rect = |q: Vector3| [q.x - input.bounds[0], q.z - input.bounds[1], input.bounds[2] - q.x, input.bounds[3] - q.z].iter().any(|d| d.abs() < 1e-3);
            let start = if on_rect(a) { 0.005 * p } else {
                match previous { Some((d, w)) if square(d, along) => if along.dot(w) > 0.0 { RIM_INSET * p } else { -RIM_INSET * p }, _ => 0.0 }
            };
            let end = length - if on_rect(b) { 0.005 * p } else if continues { 0.0 } else {
                match next { Some(d) if square(d, along) => if d.dot(inward) > 0.0 { (RIM_INSET + RIM_THICKNESS) * p } else { -(RIM_INSET + RIM_THICKNESS) * p }, _ => RIM_THICKNESS * p }
            };
            if end - start < RIM_SHORTEST * p { continue; }
            let pieces = ((end - start) / (RIM_SEGMENT * p)).ceil().max(1.0) as u32;
            let piece = (end - start) / pieces as f32;
            let right = Vector3::new(inward.z, 0.0, -inward.x);
            let flip = right.dot(along) < 0.0;
            let h = pcg(edge_rank ^ pcg(cell(e as i64) ^ 0xED6E_0003));
            // A 0-2 % height step between edges keeps meeting rim tops from
            // sharing a plane where an odd (not square) corner overlaps them.
            let jitter = 1.0 + 0.01 * (h % 3) as f32;
            let tall = p * (RIM_HEIGHT + if railing { RAIL_HEIGHT } else { 0.0 }) * jitter;
            for k in 0..pieces {
                let (s0, s1) = (start + piece * k as f32, start + piece * (k + 1) as f32);
                let origin = a + along * if flip { s1 } else { s0 } + inward * (RIM_INSET * p);
                let lo = Vector3::new(0.0, 0.005 * p, 0.0);
                let hi = Vector3::new(piece, tall + MARGIN * p, (RIM_THICKNESS + MARGIN) * p);
                let mut c = Candidate::new(crate::facade_fixtures::NO_PLANE, EDGE, origin, inward, tall, design(key, look, railing as u32, h >> 8), lo, hi);
                c.width = piece;
                c.class = 3;
                c.rank = edge_rank;
                c.attempt = e as u32 * 64 + k;
                out.push(c);
                if out.len() >= max { return; }
            }
        }
    }
}

/// Perimeter runs of a roof (start, end, inward normal), in triangle order:
/// edges with a facade wall of the same solid under them, outside any wall
/// facing the other way (party and internal walls) and off the cell rect
/// (cut faces close a solid there), with collinear consecutive edges (within
/// about 2°) merged into one run.
fn perimeter(input: &Input, planes: &[Plane], roof: &Roof) -> Vec<(Vector3, Vector3, Vector3)> {
    let p = input.pitch;
    let walls: Vec<&Plane> = planes.iter().filter(|w| w.solid == roof.solid).collect();
    let mut edges = Vec::new();
    for &t in &roof.triangles {
        for i in 0..3 {
            let (a, b) = (input.vertices[t * 3 + i], input.vertices[t * 3 + (i + 1) % 3]);
            let length = (b - a).length();
            if length < 1e-3 { continue; }
            let middle = (a + b) * 0.5;
            let wall = walls.iter().find(|w| {
                (w.normal.dot(middle) - w.offset).abs() < 0.005 && w.normal.dot(b - a).abs() < 1e-3 * length && {
                    let tangent = Vector3::new(-w.normal.z, 0.0, w.normal.x);
                    on_wall(input, planes, w, tangent, middle.dot(tangent), middle.y - 0.05 * p)
                }
            });
            let [x0, z0, x1, z1] = input.bounds;
            let cut = [(a.x - x0, b.x - x0), (a.z - z0, b.z - z0), (x1 - a.x, x1 - b.x), (z1 - a.z, z1 - b.z)].iter().any(|(d, e)| d.abs() < 1e-3 && e.abs() < 1e-3);
            if let Some(w) = wall.filter(|_| !cut) { edges.push((a, b, -w.normal)); }
        }
    }
    // Merge runs: from each edge no collinear edge leads into, follow the
    // collinear edges starting where it ends.
    let key = |q: Vector3| ((q.x * 1000.0).round() as i64, (q.z * 1000.0).round() as i64);
    let mut starts = HashMap::<(i64, i64), Vec<usize>>::new();
    for (i, e) in edges.iter().enumerate() { starts.entry(key(e.0)).or_default().push(i); }
    let along = |e: &(Vector3, Vector3, Vector3)| (e.1 - e.0).normalized();
    let next = |i: usize| starts.get(&key(edges[i].1)).and_then(|list| list.iter().copied().find(|&j| j != i && along(&edges[j]).dot(along(&edges[i])) > 0.9994));
    let mut led = vec![false; edges.len()];
    for i in 0..edges.len() { if let Some(j) = next(i) { led[j] = true; } }
    let mut runs = Vec::new();
    let mut used = vec![false; edges.len()];
    // Runs start where no collinear edge leads in; a second pass takes any
    // edge left over (none in practice).
    for pass in 0..2 {
        for i in 0..edges.len() {
            if used[i] || (pass == 0 && led[i]) { continue; }
            let (start, inward) = (edges[i].0, edges[i].2);
            let mut end = edges[i].1;
            used[i] = true;
            let mut at = i;
            while let Some(j) = next(at) { if used[j] { break; } used[j] = true; end = edges[j].1; at = j; }
            runs.push((start, end, inward));
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facade_fixtures::tests::{fixtures, generated, input, scene};
    use crate::facade_fixtures::{place, FLOATS, MAX_FIXTURES};

    fn kinds_of(values: &[f32]) -> HashMap<i32, usize> {
        let mut m = HashMap::new();
        for f in fixtures(values) { *m.entry(f[4] as i32).or_default() += 1; }
        m
    }

    #[test]
    fn tall_roofs_get_a_penthouse_tanks_and_rims_inside_their_outline() {
        // A 20 m x 40 m apartment slab 30 floors up and a 12 m x 12 m office
        // block 15 floors up (1:3 cards: 100 cm floors).
        let (chunk, bounds) = generated(&[("flats", [30000, 30000, 32000, 34000], 3000, "residential", "concrete"),
            ("office", [10000, 10000, 11200, 11200], 1500, "commercial", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        let kinds = kinds_of(&placed.values);
        assert!(kinds.get(&(PENTHOUSE as i32)).copied().unwrap_or(0) >= 2, "a penthouse on each roof: {kinds:?}");
        assert!(kinds.get(&(TANK as i32)).copied().unwrap_or(0) >= 2, "tanks: {kinds:?}");
        assert!(kinds.get(&(EDGE as i32)).copied().unwrap_or(0) >= 8, "rims on both roofs: {kinds:?}");
        for f in fixtures(&placed.values) {
            let kind = f[4];
            if kind < PENTHOUSE { continue; }
            let (x, z) = (f[0], -f[2]);
            let on_flats = (300.0..=320.0).contains(&x) && (300.0..=340.0).contains(&z);
            let on_office = (100.0..=112.0).contains(&x) && (100.0..=112.0).contains(&z);
            assert!(on_flats || on_office, "on a roof: {f:?}");
            let roof = if on_flats { 30.0 } else { 15.0 };
            assert!((f[1] - roof).abs() < 1e-3, "stands on the roof plane: {f:?}");
            assert!(f[5] == 1.0 && f[7] < 16_777_216.0 && f[7] == f[7].round(), "scale and bits: {f:?}");
            if kind == EDGE { continue; }
            // Interior fixtures: their whole footprint inside the outline by
            // EDGE_CLEARANCE (frame square to the walls: angle 0 here).
            let out = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let right = Vector3::new(out.z, 0.0, -out.x);
            let (w, d) = (f[8], f[9]);
            let start = if kind == SOLAR { 0.0 } else { -0.5 * w };
            for (i, j) in [(0.0, -0.5), (1.0, -0.5), (0.0, 0.5), (1.0, 0.5)] {
                let q = Vector3::new(f[0], 0.0, f[2]) + right * (start + i * w) + out * (j * d);
                let (qx, qz) = (q.x, -q.z);
                let (x0, x1, z0, z1) = if on_flats { (300.0, 320.0, 300.0, 340.0) } else { (100.0, 112.0, 100.0, 112.0) };
                assert!(qx >= x0 + EDGE_CLEARANCE - 1e-3 && qx <= x1 - EDGE_CLEARANCE + 1e-3 && qz >= z0 + EDGE_CLEARANCE - 1e-3 && qz <= z1 - EDGE_CLEARANCE + 1e-3, "inside the outline: {f:?} corner {q:?}");
            }
        }
        // Rims run along the outer walls, just inside them, facing inward.
        for f in fixtures(&placed.values).into_iter().filter(|f| f[4] == EDGE) {
            let inward = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let (x, z) = (f[0] - inward.x * RIM_INSET, -(f[2] - inward.z * RIM_INSET));
            assert!([300.0, 320.0, 100.0, 112.0].iter().any(|w| (x - w).abs() < 1e-3) || [300.0, 340.0, 100.0, 112.0].iter().any(|w| (z - w).abs() < 1e-3), "on a wall line: {f:?}");
            assert!(f[8] >= 0.3 && f[8] <= RIM_SEGMENT + 1e-3, "rim length: {f:?}");
        }
        assert_eq!(place(&input(&s, 1.0), MAX_FIXTURES), placed, "deterministic");
    }

    #[test]
    fn interior_fixtures_never_overlap_and_rims_meet_at_corners() {
        let (chunk, bounds) = generated(&[("flats", [30000, 30000, 33000, 36000], 3000, "residential", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        let boxes: Vec<(Vector3, Vector3, Vector3, f32, f32, f32)> = fixtures(&placed.values).into_iter().filter(|f| f[4] >= PENTHOUSE && f[4] != EDGE).map(|f| {
            let out = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let right = Vector3::new(out.z, 0.0, -out.x);
            let centre = Vector3::new(f[0], 0.0, f[2]) + if f[4] == SOLAR { right * (0.5 * f[8]) } else { Vector3::ZERO };
            (centre, right, out, f[8] * 0.5, f[9] * 0.5, f[1])
        }).collect();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                // Square-to-the-frame footprints: separated on one frame axis.
                let apart = [Vector3::RIGHT, Vector3::BACK].iter().any(|axis| {
                    let extent = |c: &(Vector3, Vector3, Vector3, f32, f32, f32)| c.1.dot(*axis).abs() * c.3 + c.2.dot(*axis).abs() * c.4;
                    (a.0.dot(*axis) - b.0.dot(*axis)).abs() >= extent(a) + extent(b) - 1e-3
                });
                assert!(apart, "interior fixtures overlap: {a:?} {b:?}");
            }
        }
        // Rim segments: their footprints (inset strips) never overlap.
        let rims: Vec<_> = fixtures(&placed.values).into_iter().filter(|f| f[4] == EDGE).collect();
        assert_eq!(rims.len(), 2 * 2 + 2 * 3, "outer walls split into pieces of at most RIM_SEGMENT: {}", rims.len());
        let total: f32 = rims.iter().map(|f| f[8]).sum();
        assert!((total - (2.0 * (30.0 + 60.0) - 4.0 * (2.0 * RIM_INSET + RIM_THICKNESS))).abs() < 1e-2, "rims cover the perimeter minus the corner squares: {total}");
        // Each corner: one rim ends at the next one's inner face, the next
        // starts at this one's outer face.
        let ends: Vec<(Vector3, Vector3)> = rims.iter().map(|f| {
            let inward = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let right = Vector3::new(inward.z, 0.0, -inward.x);
            let a = Vector3::new(f[0], 0.0, f[2]) - inward * RIM_INSET;
            (a, a + right * f[8])
        }).collect();
        for (x, z) in [(300.0f32, 300.0f32), (330.0, 300.0), (330.0, 360.0), (300.0, 360.0)] {
            let corner = Vector3::new(x, 0.0, -z);
            let at = |d: f32| ends.iter().filter(|(a, b)| [*a, *b].iter().any(|q| ((*q - corner).length() - d).abs() < 1e-3)).count();
            assert!(at(RIM_INSET) == 1 && at(RIM_INSET + RIM_THICKNESS) == 1, "corner {x},{z}: {} start at the outer face, {} end at the inner face", at(RIM_INSET), at(RIM_INSET + RIM_THICKNESS));
        }
        assert_disjoint_and_closed(&rims, &[(300.0, 300.0), (330.0, 300.0), (330.0, 360.0), (300.0, 360.0)], &[]);
    }

    /// Rim footprints (axis-aligned strips here) never overlap, and the strip
    /// square at each map corner (convex or reflex, inward on both walls) is
    /// covered by exactly one rim: no notch, no double cover.
    fn assert_disjoint_and_closed(rims: &[&[f32]], convex: &[(f32, f32)], reflex: &[(f32, f32)]) {
        let boxes: Vec<[f32; 4]> = rims.iter().map(|f| {
            let inward = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            let right = Vector3::new(inward.z, 0.0, -inward.x);
            let (o, e) = (Vector3::new(f[0], 0.0, f[2]), Vector3::new(f[0], 0.0, f[2]) + right * f[8] + inward * RIM_THICKNESS);
            [o.x.min(e.x), o.z.min(e.z), o.x.max(e.x), o.z.max(e.z)]
        }).collect();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let overlap = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0) * (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
                assert!(overlap < 1e-5, "rims overlap by {overlap}: {a:?} {b:?}");
            }
        }
        let d = RIM_INSET + 0.5 * RIM_THICKNESS;
        for (k, &(x, z)) in convex.iter().chain(reflex).enumerate() {
            // The two walls' inward offsets: towards the roof's middle at a
            // convex corner, away from the notch's outside at a reflex one.
            let corners = [(x - d, z - d), (x + d, z - d), (x + d, z + d), (x - d, z + d)];
            let covered: Vec<usize> = corners.iter().map(|&(px, pz)| boxes.iter().filter(|b| px > b[0] && px < b[2] && -pz > b[1] && -pz < b[3]).count()).collect();
            if k < convex.len() {
                assert!(covered.iter().filter(|&&c| c == 1).count() == 1 && covered.iter().all(|&c| c <= 1), "convex corner {x},{z}: {covered:?}");
            } else {
                assert!(covered.iter().filter(|&&c| c == 1).count() == 3 && covered.iter().all(|&c| c <= 1), "reflex corner {x},{z}: {covered:?}");
            }
        }
    }

    #[test]
    fn rims_close_reflex_corners_of_an_l_shaped_roof() {
        // An L footprint: five convex corners and one reflex at (310, 310) m.
        let (chunk, bounds) = crate::facade_fixtures::tests::generated_footprints(&[
            ("ell", vec![[30000, 30000], [33000, 30000], [33000, 31000], [31000, 31000], [31000, 33000], [30000, 33000]], 900, "residential", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        let rims: Vec<_> = fixtures(&placed.values).into_iter().filter(|f| f[4] == EDGE).collect();
        assert!(rims.len() >= 6, "a rim on every wall: {rims:?}");
        assert_disjoint_and_closed(&rims, &[(300.0, 300.0), (330.0, 300.0), (330.0, 310.0), (310.0, 330.0), (300.0, 330.0)], &[(310.0, 310.0)]);
    }

    #[test]
    fn rims_skip_cell_cuts_and_internal_walls_and_merge_collinear_edges() {
        // A footprint with a collinear vertex on its south wall and one across
        // the cell's east edge (x 512 m), whose cut face closes it there. The
        // triangulation's diagonals stand on internal walls (pairs facing
        // each other): no rims there.
        let (chunk, bounds) = crate::facade_fixtures::tests::generated_footprints(&[
            ("a", vec![[30000, 30000], [31000, 30000], [32000, 30000], [32000, 32000], [30000, 32000]], 900, "commercial", "concrete"),
            ("cut", vec![[50000, 30000], [53000, 30000], [53000, 32000], [50000, 32000]], 900, "public", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        let rims: Vec<_> = fixtures(&placed.values).into_iter().filter(|f| f[4] == EDGE).collect();
        let on_a = rims.iter().filter(|f| f[0] < 400.0).count();
        assert_eq!(on_a, 4, "one rim per wall of the first, the collinear south edge merged: {rims:?}");
        let cut: Vec<_> = rims.iter().filter(|f| f[0] > 400.0).collect();
        assert_eq!(cut.len(), 3, "three walls of the cut building: {cut:?}");
        for f in &rims {
            let inward = Vector3::new(f[3].sin(), 0.0, f[3].cos());
            assert!(inward.x.abs() > 0.999 || inward.z.abs() > 0.999, "no rim on a diagonal: {f:?}");
            let right = Vector3::new(inward.z, 0.0, -inward.x);
            assert!(f[0].max(f[0] + right.x * f[8]) <= 512.0, "inside the cell: {f:?}");
            assert!(!(inward.x < -0.999 && (f[0] - 512.0).abs() < 0.1), "no rim on the cut face: {f:?}");
        }
    }

    #[test]
    fn a_neighbouring_tower_blocks_rooftop_fixtures() {
        // A low block with a tall tower standing 10 cm off its roof's middle.
        let low = generated(&[("low", [30000, 30000, 31200, 31200], 600, "public", "concrete")]);
        let s = scene(&low.0, &low.1);
        let free = place(&input(&s, 1.0), MAX_FIXTURES);
        let roof: Vec<_> = fixtures(&free.values).into_iter().filter(|f| f[4] >= PENTHOUSE && f[4] != EDGE).map(|f| f.to_vec()).collect();
        assert!(!roof.is_empty(), "the low roof carries fixtures");
        // A vertical sheet across the first fixture's centre blocks it there.
        let mut s = s;
        let f = &roof[0];
        let o = Vector3::new(f[0], f[1], f[2]);
        let sheet = [o + Vector3::new(-2.0, 0.0, 0.0), o + Vector3::new(2.0, 3.0, 0.0), o + Vector3::new(2.0, 0.0, 0.0)];
        s.vertices.extend(sheet);
        s.normals.extend([Vector3::BACK; 3]);
        s.objects.push(s.kinds.len() as i32);
        s.kinds.push(0);
        s.concrete.push(false);
        s.keys.push(255);
        s.seeds.extend([godot::builtin::Vector2::ZERO; 3]);
        s.tops.extend([0.0; 3]);
        let blocked = place(&input(&s, 1.0), MAX_FIXTURES);
        assert!(!fixtures(&blocked.values).iter().any(|g| g[..3] == f[..3]), "the sheet blocks it");
    }

    #[test]
    fn separate_same_height_blocks_of_one_record_keep_their_own_roofs() {
        // Two 20 m x 40 m blocks of one apartment complex record, 30 floors
        // up and 20 m apart: one solid seed (record and top), two roofs, each
        // with its own stair penthouses (three along a block) and tanks.
        let blocks = [[30000, 30000, 32000, 34000], [34000, 30000, 36000, 34000]];
        let (chunk, bounds) = crate::facade_fixtures::tests::generated_blocks("complex", &blocks, 3000, "residential", "concrete");
        let s = scene(&chunk, &bounds);
        let input = input(&s, 1.0);
        let solid_of = crate::facade_fixtures::solids(&input);
        let roofs = roofs(&input, &solid_of);
        assert_eq!(roofs.len(), 2, "one roof per block");
        assert_eq!(roofs[0].seed, roofs[1].seed, "one solid seed");
        assert_ne!(roofs[0].solid, roofs[1].solid, "two solids");
        let which: Vec<usize> = roofs.iter().map(|r| crate::facade_fixtures::tests::block_of(&input, &r.triangles, &blocks).expect("a roof on one block")).collect();
        assert_eq!(which, [0, 1]);
        for roof in &roofs { assert!((roof.height - 30.0).abs() < 1e-3); }
        let placed = place(&input, MAX_FIXTURES);
        let mut count = [[0; 2]; 2];
        for f in fixtures(&placed.values) {
            if f[4] != PENTHOUSE && f[4] != TANK { continue; }
            let (x, z) = (f[0] * 100.0, -f[2] * 100.0);
            let block = blocks.iter().position(|&[x0, y0, x1, y1]| x >= x0 as f32 && x <= x1 as f32 && z >= y0 as f32 && z <= y1 as f32).expect("on a block");
            count[(f[4] == TANK) as usize][block] += 1;
        }
        assert!(count[0].iter().all(|&n| n == 3), "three stair penthouses on each block: {count:?}");
        assert!(count[1].iter().all(|&n| n >= 1), "tanks on each block: {count:?}");
    }

    #[test]
    fn low_roofs_get_no_penthouse_and_bits_carry_the_palette() {
        let (chunk, bounds) = generated(&[("shed", [30000, 30000, 32000, 32000], 200, "industrial", "concrete")]);
        let s = scene(&chunk, &bounds);
        let placed = place(&input(&s, 1.0), MAX_FIXTURES);
        assert!(!fixtures(&placed.values).iter().any(|f| f[4] == PENTHOUSE), "two storeys: no stair penthouse");
        let seed = (s.seeds[s.objects.iter().position(|&o| s.kinds[o as usize] == 4).unwrap() * 3].x * 16_777_216.0).round() as u32;
        let look = crate::facade_fixtures::look(seed, mapkit_package::distant::facade_own_share("concrete:industrial"));
        for f in fixtures(&placed.values).into_iter().filter(|f| f[4] >= PENTHOUSE) {
            let bits = f[7] as u32;
            assert_eq!(bits & 7, 2, "concrete industrial palette key");
            assert_eq!((bits >> 3) & 7, look & 7);
            assert_eq!((bits >> 6) & 7, (look >> 3) & 7);
        }
        assert_eq!(FLOATS, 10);
    }

    #[test]
    fn sizes_match_the_godot_meshes() {
        let meshes = include_str!("../../../godot/facade_fixture_meshes.gd");
        for (name, value) in [("RIM_INSET", RIM_INSET), ("RIM_THICKNESS", RIM_THICKNESS), ("RIM_HEIGHT", RIM_HEIGHT), ("RAIL_HEIGHT", RAIL_HEIGHT),
            ("CONDENSER_WIDTH", CONDENSER_WIDTH), ("CONDENSER_DEPTH", CONDENSER_DEPTH), ("CONDENSER_HEIGHT", CONDENSER_HEIGHT),
            ("SOLAR_DEPTH", SOLAR_DEPTH), ("SOLAR_HEIGHT", SOLAR_HEIGHT), ("MAST_FOOTPRINT", MAST_FOOTPRINT), ("ANTENNA_HEIGHT", ANTENNA_HEIGHT), ("DISH_HEIGHT", DISH_HEIGHT)] {
            assert!(meshes.contains(&format!("const {name} := {value}")), "{name} = {value} in facade_fixture_meshes.gd");
        }
        assert!(meshes.contains("const KINDS := [\"blade\", \"ac\", \"penthouse\", \"tank\", \"condenser\", \"edge\", \"solar\", \"mast\"]"));
    }
}
