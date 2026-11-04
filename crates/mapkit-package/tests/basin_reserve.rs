//! Package admission charges `water::basin_reserve_bytes` for the basins a
//! prepared real-city document builds and retains. Measure the actual
//! allocation: the construction peak and what is retained.
use mapkit_core::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
fn grow(bytes: isize) {
    let live = LIVE.fetch_add(bytes, Ordering::SeqCst) + bytes;
    PEAK.fetch_max(live, Ordering::SeqCst);
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size() as isize);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // A moving reallocation holds both buffers for a moment.
        grow(new_size as isize);
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// JSON bytes outside strings: what admission's `structure` counts for them.
fn structure_bytes(json: &[u8]) -> u64 {
    let (mut quoted, mut escaped, mut count) = (false, false, 0u64);
    for &c in json {
        if quoted {
            if escaped { escaped = false } else if c == b'\\' { escaped = true } else if c == b'"' { quoted = false }
        } else if c == b'"' { quoted = true } else { count += 1 }
    }
    count
}

fn city(ponds: usize, vertices: usize, step: i64) -> MapDocument {
    let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.nodes.clear(); d.roads.clear(); d.buildings.clear(); d.zones.clear(); d.placements.clear();
    d.attributions.push(Attribution { source: "synthetic OSM".into(), license: "ODbL-1.0".into(),
        notice: r#"{"adapter":"osm-extract-v1","authored_units":{"source_denominator":3}}"#.into() });
    for k in 0..ponds {
        let centre = [2000 + (k as i64 % 20) * step, 2000 + (k as i64 / 20) * step];
        let polygon = (0..vertices).map(|i| {
            let t = i as f64 / vertices as f64 * std::f64::consts::TAU;
            let r = (step / 3 + if i % 2 == 0 { 0 } else { step / 12 }) as f64;
            [centre[0] + (r * t.cos()) as i64, centre[1] + (r * t.sin()) as i64]
        }).collect();
        d.water_bodies.push(water::WaterBody { id: format!("pond-{k:04}"), polygon, islands: vec![],
            surface_cm: -8, bottom_cm: -68, flow_cm_s: [0, 0] });
    }
    // A fence through every pond row adds keep-outs and their shores.
    d.barriers = (0..ponds.div_ceil(20)).map(|row| barrier::Barrier { id: format!("fence-{row}"), kind: barrier::BarrierKind::Fence,
        line: vec![[1000, 2000 + row as i64 * step], [2000 + 20 * step, 2000 + row as i64 * step]], height_cm: 40, thickness_cm: 10 }).collect();
    d.normalize();
    d.validate().unwrap();
    d
}

#[test]
fn retained_basin_allocation_stays_under_the_admission_reserve() {
    let mut failed = 0;
    for (ponds, vertices, step) in [(1, 8, 4000), (40, 24, 3000), (200, 64, 2500), (400, 32, 1200), (450, 32, 1200), (500, 32, 1200),
        (550, 32, 1200), (600, 32, 1200), (400, 128, 2000), (1024, 32, 1200)] {
        let d = city(ponds, vertices, step);
        let json = serde_json::to_vec(&d).unwrap();
        let reserve = water::basin_reserve_bytes(structure_bytes(&json));
        let before = LIVE.load(Ordering::SeqCst);
        PEAK.store(before, Ordering::SeqCst);
        let built = water::Basins::new(&d);
        let peak = (PEAK.load(Ordering::SeqCst) - before).max(0) as u64;
        // Construction, failed or not, never allocates beyond the reserve.
        assert!(peak <= reserve, "{ponds}x{vertices}: construction peak {peak} > reserve {reserve}");
        println!("{ponds}x{vertices}: peak {peak} reserve {reserve} ok {}", built.is_ok());
        let basins = match built {
            Ok(basins) => basins,
            // Beyond the work or byte allowance map preparation fails closed.
            Err(e) => { assert!(ponds * vertices >= 10_000 && e.code == "E_BUDGET", "{ponds}x{vertices}: {e:?}"); failed += 1; continue; }
        };
        let actual = (LIVE.load(Ordering::SeqCst) - before).max(0) as u64;
        assert!(basins.is_active() && basins.counts().lines > 0);
        assert!(actual <= basins.retained_bytes(), "{ponds}x{vertices}: allocated {actual} > accounted {}", basins.retained_bytes());
        assert!(basins.retained_bytes() <= reserve, "{ponds}x{vertices}: accounted {} > reserve {reserve}", basins.retained_bytes());
        assert!(reserve <= water::MAX_BASIN_BYTES);
        drop(basins);
    }
    // The near-cap cases include both outcomes.
    assert!(failed > 0 && failed < 7, "{failed}");
    // Documents without basins keep nothing beyond the empty value.
    let mut authored = city(4, 16, 3000);
    authored.attributions.retain(|a| !a.notice.contains("osm-extract-v1"));
    let before = LIVE.load(Ordering::SeqCst);
    let basins = water::Basins::new(&authored).unwrap();
    assert!(!basins.is_active() && (LIVE.load(Ordering::SeqCst) - before) as u64 <= basins.retained_bytes());
}
