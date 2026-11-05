//! Display-only night light map of city streetlights (`builtin:city_streetlight_*`,
//! mapkit-core `CITY_STREETLIGHTS`): one RG8 texture per cell whose texels hold
//! the summed ground light of every lamp head whose pool reaches the cell
//! (R) and the light-weighted foot height of those lamps (G, in
//! HEIGHT_STEP_CM steps above the map's `height_base`), neighbour cells' lamps
//! included, so both sides of a cell border compute the same texels. The
//! renderer binds it unmipmapped on the cell's road and sidewalk materials
//! (urban_surface.gdshader `lamp_map`), which add it as emission after sunset
//! and fade it on surfaces far above or below the lamps' feet (decks over a
//! lit road, a road under a lit one). Nothing here enters generation,
//! collision or the package hash.
use mapkit_core::{Bounds, MapDocument, Placement};

/// Texel pitch (cm) of the map. A cell of `cell_size_cm` gets
/// `cell_size_cm / TEXEL_CM + 1` texels a side (at most `MAX_TEXELS`) whose
/// centres run from one cell edge to the other, so the edge texels of two
/// neighbouring cells sample the same border points and match exactly.
pub const TEXEL_CM: i64 = 50;
pub const MAX_TEXELS: i64 = 257;
/// Pool of one head (cm, plan view, around the point under the head centre):
/// half lengths along the road, across toward the road (the arm's side) and
/// back toward the pole. The core (1 - q)^2 falls to zero at q = 1, the soft
/// tail (1 - q/4)^4 at q = 4, so a head lights nothing beyond twice its radii.
pub const POOL_ALONG_CM: f32 = 460.0;
pub const POOL_FRONT_CM: f32 = 300.0;
pub const POOL_BACK_CM: f32 = 190.0;
pub const POOL_CORE: f32 = 0.8;
pub const POOL_TAIL: f32 = 0.22;
/// Height step (cm) of the G channel: 255 steps span about 40 m of foot
/// heights over one cell.
pub const HEIGHT_STEP_CM: i64 = 16;
/// Farthest a lit texel lies from a lamp foot (cm): the longest arm plus twice
/// the longest pool radius; inside the 1,501 cm placement margin of region
/// sources (mapkit-core region.rs), so region cells match the full map.
pub fn reach_cm() -> i64 {
    let arm = mapkit_core::CITY_STREETLIGHTS.iter().map(|(_, reach, _)| *reach).max().unwrap_or(0);
    arm + 2 * POOL_ALONG_CM.max(POOL_FRONT_CM).max(POOL_BACK_CM).ceil() as i64
}

/// One head: the plan point under its centre (map cm, x and z) and the unit
/// direction its arm points (over the road), with the along-road direction
/// its left normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Head {
    pub at: [f64; 2],
    pub out: [f64; 2],
    pub foot: [i64; 2],
    /// Foot height (map cm).
    pub y: i64,
}

/// Plan direction of local `v` (x, z) under the placement pose, as
/// `Placement::transform_point` turns proxies (quarter turns, then yaw).
fn turned(p: &Placement, mut v: [f64; 2]) -> [f64; 2] {
    for _ in 0..p.quarter_turns { v = [-v[1], v[0]]; }
    if p.yaw_offset_mdeg != 0 {
        let angle = f64::from(p.yaw_offset_mdeg).to_radians() / 1000.0;
        let (s, c) = angle.sin_cos();
        v = [v[0] * c - v[1] * s, v[0] * s + v[1] * c];
    }
    v
}

/// Every city streetlight head of the document, sorted by foot x (then the
/// document order), twins contributing two.
pub fn heads(document: &MapDocument) -> Vec<Head> {
    let mut result = Vec::new();
    for p in &document.placements {
        let Some((reach, twin)) = mapkit_core::city_streetlight(&p.asset_id) else { continue };
        let out = turned(p, [1.0, 0.0]);
        let foot = [p.position[0], p.position[2]];
        for sign in if twin { &[1.0, -1.0][..] } else { &[1.0][..] } {
            let d = [out[0] * sign, out[1] * sign];
            result.push(Head { at: [foot[0] as f64 + d[0] * reach as f64, foot[1] as f64 + d[1] * reach as f64], out: d, foot, y: p.position[1] });
        }
    }
    result.sort_by_key(|h| h.foot[0]);
    result
}

/// Heads whose foot lies within `reach_cm()` of the square of side `size`
/// at `min` (a superset of the heads that light it).
pub fn near<'a>(heads: &'a [Head], min: [i64; 2], size: i64) -> impl Iterator<Item = &'a Head> {
    let reach = reach_cm();
    let start = heads.partition_point(|h| h.foot[0] < min[0] - reach);
    heads[start..].iter().take_while(move |h| h.foot[0] <= min[0] + size + reach)
        .filter(move |h| h.foot[1] >= min[1] - reach && h.foot[1] <= min[1] + size + reach)
}

/// Side of a cell's map in texels (at least 2).
pub fn texels(document: &MapDocument) -> usize {
    (document.cell_size_cm as i64 / TEXEL_CM + 1).clamp(2, MAX_TEXELS) as usize
}

/// Presentation allowance of one map: its two-byte texels (the source's CPU
/// array), which also cover the renderer's unmipmapped RG8 texture of the
/// same size inside the display estimate's share of the same presentation
/// bytes, plus its record.
pub fn map_bytes(document: &MapDocument) -> u64 {
    let n = texels(document) as u64;
    n * n * 2 + 4096
}

/// The cell's square map rectangle (map cm): the nominal cell square from the
/// cell's minimum corner, even where the map edge clips the cell.
pub fn square(document: &MapDocument, bounds: &Bounds) -> ([i64; 2], i64) {
    (bounds.min, document.cell_size_cm as i64)
}

/// Plan bounding box [x0, z0, x1, z1] (cm) of a head's pool support (q < 4:
/// twice its radii around the point under the head centre).
fn pool_box(head: &Head) -> [f64; 4] {
    let (front, back, along) = (2.0 * POOL_FRONT_CM as f64, 2.0 * POOL_BACK_CM as f64, 2.0 * POOL_ALONG_CM as f64);
    let (o, l) = (head.out, [-head.out[1], head.out[0]]);
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (c, a) in [(front, along), (front, -along), (-back, along), (-back, -along)] {
        let p = [head.at[0] + o[0] * c + l[0] * a, head.at[1] + o[1] * c + l[1] * a];
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    b
}

/// Heads whose pool support box overlaps the square (closed): exactly the
/// heads `map` sums, and `lights` and `map` agree on them.
fn lighting<'a>(heads: &'a [Head], min: [i64; 2], size: i64) -> impl Iterator<Item = &'a Head> {
    let (x0, z0, x1, z1) = (min[0] as f64, min[1] as f64, (min[0] + size) as f64, (min[1] + size) as f64);
    near(heads, min, size).filter(move |h| {
        let b = pool_box(h);
        b[0] <= x1 && b[2] >= x0 && b[1] <= z1 && b[3] >= z0
    })
}

/// Whether any head's pool reaches the square (`map` would return a map):
/// cost() and decorate() list exactly the same cells.
pub fn lights(heads: &[Head], min: [i64; 2], size: i64) -> bool {
    lighting(heads, min, size).next().is_some()
}

/// Ground light of one head at plan offset `d` (cm) from the point under its
/// centre: an ellipse elongated along the road, longer over the road than
/// toward the pole, a bright core and a soft tail.
pub fn pool(head: &Head, d: [f64; 2]) -> f32 {
    let across = (d[0] * head.out[0] + d[1] * head.out[1]) as f32;
    let along = (-d[0] * head.out[1] + d[1] * head.out[0]) as f32;
    let c = across / if across >= 0.0 { POOL_FRONT_CM } else { POOL_BACK_CM };
    let a = along / POOL_ALONG_CM;
    let q = a * a + c * c;
    if q >= 4.0 { return 0.0; }
    let core = if q < 1.0 { (1.0 - q) * (1.0 - q) } else { 0.0 };
    let tail = 1.0 - q * 0.25;
    POOL_CORE * core + POOL_TAIL * tail * tail * tail * tail
}

/// A cell's light map: `texels` row-major RG pairs, `n` a side, and the
/// foot height (map cm, a multiple of HEIGHT_STEP_CM) that G = 0 stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct LampMap {
    pub texels: Vec<u8>,
    pub height_base: i64,
}

/// The RG8 map of the square (row-major, `n` texels a side; rows along map
/// -z = scene +z from the square's scene minimum, i.e. from map z = min +
/// size down; texel k samples the point k / (n - 1) of the way across), or
/// None when no head's pool reaches it. R is the light (the sum saturates
/// at 1); G the light-weighted foot height of the heads lighting the texel
/// (texels without light take a lit neighbour's, so filtering never mixes
/// in a false height). Both are pure functions of the heads and the texel's
/// map point, so border texels of neighbouring cells match.
pub fn map(heads: &[Head], min: [i64; 2], size: i64, n: usize) -> Option<LampMap> {
    let mut sum = vec![0f32; n * n];
    let mut weight = vec![0f64; n * n];
    let mut height = vec![0f64; n * n];
    let pitch = size as f64 / (n - 1) as f64;
    let radius = 2.0 * POOL_ALONG_CM.max(POOL_FRONT_CM).max(POOL_BACK_CM) as f64;
    let top = (min[1] + size) as f64;
    let lit: Vec<&Head> = lighting(heads, min, size).collect();
    let low = lit.iter().map(|h| h.y).min()?;
    let height_base = low.div_euclid(HEIGHT_STEP_CM) * HEIGHT_STEP_CM;
    for head in lit {
        // Texel range of the head's pool box (columns along +x, rows along -z).
        let (c0, c1) = (((head.at[0] - radius - min[0] as f64) / pitch).floor().max(0.0) as usize,
            (((head.at[0] + radius - min[0] as f64) / pitch).ceil().max(-1.0) as i64).min(n as i64 - 1));
        let (r0, r1) = (((top - head.at[1] - radius) / pitch).floor().max(0.0) as usize,
            (((top - head.at[1] + radius) / pitch).ceil().max(-1.0) as i64).min(n as i64 - 1));
        for r in r0 as i64..=r1 {
            let z = top - r as f64 * pitch;
            for c in c0 as i64..=c1 {
                let x = min[0] as f64 + c as f64 * pitch;
                let light = pool(head, [x - head.at[0], z - head.at[1]]);
                if light > 0.0 {
                    let k = r as usize * n + c as usize;
                    sum[k] += light;
                    weight[k] += f64::from(light);
                    height[k] += f64::from(light) * head.y as f64;
                }
            }
        }
    }
    let step = |k: usize| ((height[k] / weight[k] - height_base as f64) / HEIGHT_STEP_CM as f64 + 0.5).floor().clamp(0.0, 255.0) as u8;
    let mut g: Vec<Option<u8>> = (0..n * n).map(|k| (weight[k] > 0.0).then(|| step(k))).collect();
    let filled: Vec<Option<u8>> = (0..n * n).map(|k| g[k].or_else(|| {
        let (r, c) = (k / n, k % n);
        [(c > 0).then(|| k - 1), (c + 1 < n).then(|| k + 1), (r > 0).then(|| k - n), (r + 1 < n).then(|| k + n)].into_iter().flatten().find_map(|j| g[j])
    })).collect();
    g = filled;
    let mut texels = Vec::with_capacity(2 * n * n);
    for k in 0..n * n {
        texels.push((sum[k].clamp(0.0, 1.0) * 255.0).round() as u8);
        texels.push(g[k].unwrap_or(0));
    }
    Some(LampMap { texels, height_base })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> MapDocument {
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap()
    }
    fn lamp(id: &str, asset: &str, x: i64, z: i64, quarter_turns: u8, yaw: i32) -> Placement {
        Placement { id: id.into(), asset_id: asset.into(), position: [x, 0, z], quarter_turns, yaw_offset_mdeg: yaw }
    }

    #[test]
    fn heads_follow_the_pose_and_the_reach_of_each_id() {
        let mut d = document();
        d.placements = vec![lamp("a", "builtin:city_streetlight_135", 1000, 2000, 0, 0),
            lamp("b", "builtin:city_streetlight_twin_75", 0, 0, 1, 0),
            lamp("c", "builtin:city_streetlight_255", 500, 500, 0, 30000),
            lamp("d", "builtin:streetlight", 0, 0, 0, 0)];
        let h = heads(&d);
        assert_eq!(h.len(), 4, "one head per single lamp, two per twin, none for the old streetlight");
        let a = h.iter().find(|h| h.foot == [1000, 2000]).unwrap();
        assert!((a.at[0] - 1135.0).abs() < 1e-9 && (a.at[1] - 2000.0).abs() < 1e-9);
        // A quarter turn maps local +x to map +z (transform_point: x,z -> -z,x).
        let b: Vec<_> = h.iter().filter(|h| h.foot == [0, 0]).collect();
        assert!(b.iter().any(|h| (h.at[1] - 75.0).abs() < 1e-9) && b.iter().any(|h| (h.at[1] + 75.0).abs() < 1e-9));
        let c = h.iter().find(|h| h.foot == [500, 500]).unwrap();
        let p = lamp("c", "builtin:city_streetlight_255", 500, 500, 0, 30000).transform_point_check();
        assert!((c.at[0] - p[0]).abs() < 1.0 && (c.at[1] - p[1]).abs() < 1.0, "the head follows Placement::transform_point: {:?} {:?}", c.at, p);
        assert!(h.windows(2).all(|w| w[0].foot[0] <= w[1].foot[0]));
    }

    trait Check { fn transform_point_check(&self) -> [f64; 2]; }
    impl Check for Placement {
        fn transform_point_check(&self) -> [f64; 2] {
            let v = self.transform_point([255, 0, 0]);
            [v[0] as f64, v[2] as f64]
        }
    }

    #[test]
    fn pools_are_long_along_the_road_and_reach_over_it() {
        let h = Head { at: [0.0, 0.0], out: [1.0, 0.0], foot: [-135, 0], y: 0 };
        assert!((pool(&h, [0.0, 0.0]) - (POOL_CORE + POOL_TAIL)).abs() < 1e-6);
        assert!(pool(&h, [0.0, 400.0]) > pool(&h, [250.0, 0.0]) * 0.5, "longer along the road");
        assert!(pool(&h, [200.0, 0.0]) > pool(&h, [-200.0, 0.0]), "brighter over the road than behind the pole");
        assert_eq!(pool(&h, [0.0, 2.0 * POOL_ALONG_CM as f64 + 1.0]), 0.0);
        assert_eq!(pool(&h, [2.0 * POOL_FRONT_CM as f64 + 1.0, 0.0]), 0.0);
        assert_eq!(pool(&h, [-2.0 * POOL_BACK_CM as f64 - 1.0, 0.0]), 0.0);
        assert!(reach_cm() >= 375 + 2 * 460);
        // Region sources keep every placement within MIN_SOURCE_SPACING_CM +
        // 1000 = 1501 cm of their cells (mapkit-core region.rs), so a region
        // cell sees every lamp that lights it, as the full map does.
        assert!(reach_cm() <= 1501);
    }

    #[test]
    fn neighbouring_cells_share_border_light_and_unlit_cells_get_no_map() {
        let mut d = document();
        d.cell_size_cm = 12800;
        // A lamp 1 m west of the border x = 12800, its arm pointing east.
        d.placements = vec![lamp("a", "builtin:city_streetlight_195", 12700, 6400, 0, 0)];
        let h = heads(&d);
        let n = texels(&d);
        assert_eq!(n, 257);
        let full = map(&h, [0, 0], 12800, n).expect("the lamp's own cell");
        let near = map(&h, [12800, 0], 12800, n).expect("its neighbour within the pool");
        assert!(lights(&h, [12800, 0], 12800) && !lights(&h, [25600, 0], 12800) && map(&h, [25600, 0], 12800, n).is_none());
        let (west, east) = (&full.texels, &near.texels);
        // The last west column and the first east column sample the same
        // border points: identical rows (light and height), lit at the head's
        // row (z 6400).
        let row = (12800 - 6400) / 50;
        for r in 0..n { assert_eq!(west[2 * (r * n + n - 1)..][..2], east[2 * r * n..][..2], "border row {r}"); }
        assert!(east[2 * row * n] > 100, "lit across the border: {}", east[2 * row * n]);
        // Rows run along map -z: rows 20 m away from the head are unlit.
        assert!(east[2 * (row + 40) * n] == 0 && east[2 * (row - 40) * n] == 0);
        assert_eq!(map(&h, [0, 0], 12800, n).as_ref(), Some(&full), "deterministic");
        assert_eq!(map_bytes(&d), 257 * 257 * 2 + 4096);
        d.cell_size_cm = 20000;
        assert_eq!(texels(&d), 257, "larger cells keep at most 257 texels");
        d.cell_size_cm = 1000;
        assert_eq!(texels(&d), 21);
    }

    #[test]
    fn renderers_mirror_the_lamp_contract() {
        // city_lamp_meshes.gd reads each id's reach and twin from its IDS
        // literal, and the shaders and renderer read these presentation keys.
        let meshes = include_str!("../../../godot/city_lamp_meshes.gd");
        for (id, reach, twin) in mapkit_core::CITY_STREETLIGHTS {
            assert!(meshes.contains(&format!("\"{id}\": [{reach}, {twin}]")), "{id}");
        }
        assert!(meshes.contains(&format!("HEAD_CM := {}", mapkit_core::CITY_STREETLIGHT_HEAD_CM)));
        assert!(meshes.contains(&format!("POLE_CM := {}", mapkit_core::CITY_STREETLIGHT_POLE_CM)));
        assert!(meshes.contains(&format!("HEIGHT_CM := {}", mapkit_core::CITY_STREETLIGHT_HEIGHT_CM)));
        let [back, tip, side, top] = mapkit_core::CITY_STREETLIGHT_ENVELOPE_CM;
        assert!(meshes.contains(&format!("ENVELOPE_CM := [{back}, {tip}, {side}, {top}]")));
        assert!(meshes.contains(r#"presentation.get("lamp_map", PackedByteArray())"#) && meshes.contains(r#"presentation.get("lamp_map_rect", PackedFloat32Array())"#));
        let urban = include_str!("../../../godot/urban_surface.gdshader");
        assert!(urban.contains("*(texels-1.0)+0.5)/texels"), "edge-to-edge texel sampling");
        let memory = include_str!("../../../godot/render_memory.gd");
        for (id, _, _) in mapkit_core::CITY_STREETLIGHTS { assert!(memory.contains(&format!("\"{id}\"")), "{id} faces are drawn over"); }
    }

    #[test]
    fn overlapping_pools_saturate() {
        let h = vec![Head { at: [100.0, 100.0], out: [1.0, 0.0], foot: [0, 100], y: 0 }; 3];
        let m = map(&h, [0, 0], 400, 9).unwrap();
        assert_eq!(*m.texels.iter().step_by(2).max().unwrap(), 255);
    }

    #[test]
    fn cells_only_heads_point_away_from_get_no_map() {
        // A 135 lamp 400 cm east of the cell, its arm pointing further east:
        // its foot is within reach_cm() but its pool stops 245 cm past the
        // pole toward the cell, so the cell gets no map and no charge.
        let away = vec![Head { at: [12800.0 + 400.0 + 135.0, 6400.0], out: [1.0, 0.0], foot: [13200, 6400], y: 0 }];
        assert!(near(&away, [0, 0], 12800).next().is_some());
        assert!(!lights(&away, [0, 0], 12800) && map(&away, [0, 0], 12800, 257).is_none());
        // Pointing back west, the same lamp lights the cell's east edge.
        let back = vec![Head { at: [13200.0 - 135.0, 6400.0], out: [-1.0, 0.0], foot: [13200, 6400], y: 0 }];
        assert!(lights(&back, [0, 0], 12800) && map(&back, [0, 0], 12800, 257).is_some());
    }

    #[test]
    fn the_green_channel_holds_the_lit_foot_height() {
        // Two lamps 900 cm apart along the road at foot heights 480 and 160 cm: G under each
        // head decodes to about its own foot, between them to a blend, and
        // unlit neighbours of lit texels repeat a lit neighbour's height.
        let h = vec![Head { at: [1000.0, 1000.0], out: [1.0, 0.0], foot: [865, 1000], y: 480 },
            Head { at: [1000.0, 1900.0], out: [1.0, 0.0], foot: [865, 1900], y: 160 }];
        let m = map(&h, [0, 0], 3200, 65).unwrap();
        assert_eq!(m.height_base, 160);
        let at = |x: i64, z: i64| { let k = ((3200 - z) / 50 * 65 + x / 50) as usize; (m.texels[2 * k], m.height_base + i64::from(m.texels[2 * k + 1]) * HEIGHT_STEP_CM) };
        let (a, b) = (at(1000, 1000), at(1000, 1900));
        assert!(a.0 > 100 && b.0 > 100);
        assert!((a.1 - 480).abs() <= 64 && (b.1 - 160).abs() <= 64, "{a:?} {b:?}");
        let between = at(1000, 1450).1;
        assert!(between > b.1 && between < a.1, "{between}");
        // Base heights are step multiples: a cell of feet at -40 decodes the same.
        let low = vec![Head { y: -40, ..h[0] }];
        assert_eq!(map(&low, [0, 0], 3200, 65).unwrap().height_base, -48);
    }
}
