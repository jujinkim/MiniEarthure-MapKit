//! Display-only variants of city barriers (fences, walls, hedges), chosen by
//! an integer hash of the barrier ID and its real height (height × the map
//! scale 1:D). Near detail (mapkit-godot barrier_details.rs and the
//! barrier_detail_meshes.gd set) and the distant mesh (distant.rs) read the
//! same design so both draw the same family and paint. Nothing here enters
//! generation, collision or the generated hash.
use mapkit_core::barrier::BarrierKind;

/// Design bits: kind (0 hedge, 1 fence, 2 wall) in 0..2, family in 2..5,
/// palette in 5..8, top style in 8..10, rail style in 10..12 and the map
/// scale denominator D (1..15) in 12..16. Instances carry them in two
/// 8-bit custom channels, exact on every renderer.
pub type Bits = u32;
pub const KIND_HEDGE: u32 = 0;
pub const KIND_FENCE: u32 = 1;
pub const KIND_WALL: u32 = 2;
/// Fence families: steel bar panels, welded wire mesh, chain-link, wooden
/// boards or pickets, low pipe rails.
pub const STEEL_BAR: u32 = 0;
pub const WELDED_MESH: u32 = 1;
pub const CHAIN_LINK: u32 = 2;
pub const WOODEN: u32 = 3;
pub const PIPE_RAIL: u32 = 4;
/// Wall families: red brick, painted concrete block, precast panels, stone.
pub const BRICK: u32 = 0;
pub const BLOCK: u32 = 1;
pub const PRECAST: u32 = 2;
pub const STONE: u32 = 3;

#[derive(serde::Deserialize)]
struct Palette {
    fence: Vec<Vec<[u8; 3]>>,
    wall: Vec<Vec<[u8; 3]>>,
    hedge: Vec<Vec<[u8; 3]>>,
    bay_m: Bays,
    far_opacity: Vec<f32>,
    board_far_opacity: f32,
    far_backdrop: [u8; 3],
}
#[derive(serde::Deserialize)]
struct Bays {
    fence: Vec<f64>,
    wall: Vec<f64>,
}
fn palette() -> &'static Palette {
    static PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| serde_json::from_str(include_str!("../../../godot/barrier_palette.json")).expect("valid barrier palette"))
}

/// Integer-only PCG hash (the shader's facade_pcg).
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}
fn seed(id: &str) -> u32 {
    u32::from_str_radix(&mapkit_core::sha256(id.as_bytes())[..8], 16).unwrap()
}
/// A pick among weighted choices (per mille) from one hash value.
fn weighted(value: u32, weights: &[(u32, u32)]) -> u32 {
    let total: u32 = weights.iter().map(|w| w.1).sum();
    let mut roll = value % total;
    for (choice, weight) in weights {
        if roll < *weight {
            return *choice;
        }
        roll -= weight;
    }
    weights[weights.len() - 1].0
}

/// The design of a barrier: `height_cm` is its map height, `denominator`
/// the map scale 1:D (1 without a recorded scale).
pub fn design(id: &str, kind: BarrierKind, height_cm: u32, denominator: u64) -> Bits {
    let d = denominator.clamp(1, 15) as u32;
    let real_m = f64::from(height_cm) * f64::from(d) / 100.0;
    let s = seed(id);
    let roll = |k: u32| pcg(s ^ k.wrapping_mul(0x9e37_79b9));
    let (kind_bits, family, top, rails) = match kind {
        BarrierKind::Fence => {
            let family = if real_m < 0.95 {
                weighted(roll(1), &[(PIPE_RAIL, 70), (WOODEN, 30)])
            } else if real_m < 1.6 {
                weighted(roll(1), &[(STEEL_BAR, 45), (WELDED_MESH, 30), (WOODEN, 15), (CHAIN_LINK, 10)])
            } else {
                weighted(roll(1), &[(WELDED_MESH, 50), (CHAIN_LINK, 35), (STEEL_BAR, 15)])
            };
            // Steel bars: plain, spear tips or a low arch per bay; wood: boards or pickets.
            let top = if family == STEEL_BAR { roll(3) % 3 } else { 0 };
            let rails = if family == WOODEN { roll(4) % 2 } else { 0 };
            (KIND_FENCE, family, top, rails)
        }
        BarrierKind::Wall => {
            let family = weighted(roll(1), &[(BRICK, 35), (BLOCK, 30), (PRECAST, 20), (STONE, 15)]);
            // Brick: a concrete cap or a soldier course; stone: 40 % giwa tile coping.
            let top = match family {
                BRICK => roll(3) % 2,
                STONE => u32::from(roll(3) % 10 < 4),
                _ => 0,
            };
            (KIND_WALL, family, top, 0)
        }
        BarrierKind::Hedge => (KIND_HEDGE, roll(1) % 3, u32::from(roll(3) % 10 < 3), 0),
    };
    let palette = roll(2) % entries(kind_bits, family).len() as u32;
    kind_bits | family << 2 | palette << 5 | top << 8 | rails << 10 | d << 12
}

pub fn kind(bits: Bits) -> u32 { bits & 3 }
pub fn family(bits: Bits) -> u32 { bits >> 2 & 7 }
pub fn palette_index(bits: Bits) -> u32 { bits >> 5 & 7 }
pub fn top(bits: Bits) -> u32 { bits >> 8 & 3 }
pub fn rails(bits: Bits) -> u32 { bits >> 10 & 3 }
pub fn denominator(bits: Bits) -> u32 { (bits >> 12 & 15).max(1) }

fn entries(kind: u32, family: u32) -> &'static [[u8; 3]] {
    let p = palette();
    match kind {
        KIND_FENCE => &p.fence[family as usize],
        KIND_WALL => &p.wall[family as usize],
        _ => &p.hedge[0],
    }
}

/// Lattice spacing of posts or piers (real metres; walls without piers keep
/// their pattern period). Brick, block and precast patterns repeat an
/// integer number of times per bay, so the pattern stays continuous from a
/// piece's phase.
pub fn bay_m(bits: Bits) -> f64 {
    let bays = &palette().bay_m;
    match kind(bits) {
        KIND_FENCE => bays.fence[family(bits) as usize],
        KIND_WALL => bays.wall[family(bits) as usize],
        _ => 1.0,
    }
}
/// Whether lattice posts (fences) or piers (brick, block, precast walls)
/// stand every bay.
pub fn has_lattice(bits: Bits) -> bool {
    kind(bits) == KIND_FENCE || (kind(bits) == KIND_WALL && family(bits) != STONE)
}
/// Main paint or body colour (sRGB).
pub fn color(bits: Bits) -> [u8; 3] {
    let entries = entries(kind(bits), family(bits));
    entries[(palette_index(bits) as usize).min(entries.len() - 1)]
}
/// Share of a fence's face its parts cover once its infill is unresolved
/// (boards cover most of a wooden fence); walls and hedges are solid.
pub fn far_opacity(bits: Bits) -> f32 {
    if kind(bits) != KIND_FENCE {
        return 1.0;
    }
    if family(bits) == WOODEN && rails(bits) == 0 {
        return palette().board_far_opacity;
    }
    palette().far_opacity[family(bits) as usize]
}
/// Opaque colour of the distant prism: the paint over the far backdrop by
/// the far opacity, so a fence beyond the detail cells reads as the haze the
/// near far mesh draws, not a solid slab.
pub fn distant_color(bits: Bits) -> [u8; 4] {
    let (paint, backdrop, share) = (color(bits), palette().far_backdrop, far_opacity(bits));
    let mix = |i: usize| (f32::from(backdrop[i]) + (f32::from(paint[i]) - f32::from(backdrop[i])) * share + 0.5).floor() as u8;
    [mix(0), mix(1), mix(2), 255]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn designs_follow_kind_real_height_and_scale() {
        let mut counts = std::collections::BTreeMap::new();
        for i in 0..2000 {
            let id = format!("osm:way/{i}");
            // 40 cm map at 1:3 is a 1.2 m fence; 67 cm a 2.0 m one; 27 cm a 0.8 m one.
            for (height, low, high) in [(40, 0.95, 1.6), (67, 1.6, 9.0), (27, 0.0, 0.95)] {
                let bits = design(&id, BarrierKind::Fence, height, 3);
                assert_eq!(kind(bits), KIND_FENCE);
                assert_eq!(denominator(bits), 3);
                assert!((palette_index(bits) as usize) < entries(KIND_FENCE, family(bits)).len());
                *counts.entry((low as u32 * 10 + high as u32, family(bits))).or_insert(0) += 1;
                assert_eq!(bits, design(&id, BarrierKind::Fence, height, 3), "stable");
                let real = f64::from(height) * 3.0 / 100.0;
                assert!(real >= low && real < high);
                if real < 0.95 { assert!(matches!(family(bits), PIPE_RAIL | WOODEN)); }
                if real >= 1.6 { assert!(matches!(family(bits), WELDED_MESH | CHAIN_LINK | STEEL_BAR)); }
            }
            let wall = design(&id, BarrierKind::Wall, 67, 3);
            assert_eq!(kind(wall), KIND_WALL);
            assert!(top(wall) == 0 || matches!(family(wall), BRICK | STONE));
            let hedge = design(&id, BarrierKind::Hedge, 27, 3);
            assert_eq!(kind(hedge), KIND_HEDGE);
            assert!(family(hedge) < 3);
        }
        // Every family of each height band occurs.
        assert!(counts.len() >= 2 + 4 + 3, "{counts:?}");
        // Without a recorded scale the map height is real.
        assert_eq!(denominator(design("a", BarrierKind::Fence, 120, 1)), 1);
        assert_eq!(denominator(design("a", BarrierKind::Fence, 120, 0)), 1);
    }
    #[test]
    fn patterns_repeat_whole_per_bay_and_distant_fences_are_haze() {
        // Brick 0.2 m, block 0.4 m, precast 2.0 m and stone 1.0 m along
        // (barrier_detail.gdshader), fence posts every bay.
        for (family, period) in [(BRICK, 0.2), (BLOCK, 0.4), (PRECAST, 2.0), (STONE, 1.0)] {
            let bits = KIND_WALL | family << 2 | 3 << 12;
            let ratio = bay_m(bits) / period;
            assert!((ratio - ratio.round()).abs() < 1e-9, "{family}: {ratio}");
            assert_eq!(distant_color(bits)[..3], color(bits));
        }
        let fence = design("f", BarrierKind::Fence, 40, 3);
        let distant = distant_color(fence);
        let backdrop = palette().far_backdrop;
        let share = far_opacity(fence);
        assert!(share > 0.2 && share < 0.9);
        for i in 0..3 {
            let expected = f32::from(backdrop[i]) + (f32::from(color(fence)[i]) - f32::from(backdrop[i])) * share;
            assert!((f32::from(distant[i]) - expected).abs() <= 1.0);
        }
    }
}
