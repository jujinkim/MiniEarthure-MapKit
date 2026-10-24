//! One test on purpose: the generation counters are process-wide.
use mapkit_core::assembled_track::{self as track, Length, Settings};

#[test]
fn one_compile_per_generation_and_bounded_clearance_work() {
    let mut within = [0usize; 2];
    let mut total = [0usize; 2];
    for circuit in [true, false] {
        for length in Length::ALL {
            for seed in 0..8 {
                track::reset_generation_counters();
                let (a, stats) = track::assemble_with_stats(&Settings { seed, circuit, length, ..Settings::default() }).unwrap();
                let c = track::generation_counters();
                let label = format!("seed {seed} {circuit} {length:?}: {} pieces, {c:?}, {stats:?}", a.pieces.len());
                assert_eq!(c.assemblies, 1, "{label}");
                // One compile per finished attempt; validation hits the compile cache.
                assert_eq!(c.compiles, 1, "{label}");
                a.validate().unwrap();
                assert_eq!(track::generation_counters().compiles, 1, "{label}");
                // Growth checks a handful of grid candidates per placed piece. A circuit
                // may add bounded closure search (one clearance query per expanded node)
                // and abandoned motifs or attempts; those enter the bound explicitly.
                let n = a.pieces.len();
                assert!(c.conflicts < 40 * (n + stats.pops + stats.nodes / 4), "{label}");
                within[usize::from(circuit)] += usize::from(c.conflicts < 40 * n);
                total[usize::from(circuit)] += 1;
            }
        }
    }
    println!("conflicts < 40·pieces: sprint {}/{}, circuit {}/{}", within[0], total[0], within[1], total[1]);
    assert_eq!(within[0], total[0]);
    assert!(within[1] * 10 >= total[1] * 9);
}
