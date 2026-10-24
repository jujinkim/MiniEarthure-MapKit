//! Release timing gate: `cargo test --release -p mapkit-core --test track_generation_speed -- --ignored --nocapture`.
use mapkit_core::assembled_track::{self as track, authoring, Length, Settings};
use std::time::Instant;

#[test]
#[ignore = "release timing run"]
fn long_tracks_generate_quickly() {
    for circuit in [true, false] {
        let mut all = vec![];
        for seed in 1..=8 {
            let s = Settings { seed, circuit, length: Length::Long, ..Settings::default() };
            let start = Instant::now();
            let a = track::assemble(&s).unwrap();
            let total = start.elapsed().as_secs_f64() * 1000.0;
            // The same compile without grounding isolates the grounding share.
            let mut source = a.seed_source.clone().unwrap();
            let start = Instant::now();
            std::thread::spawn({ let source = source.clone(); move || authoring::compile(&source).unwrap() }).join().unwrap();
            let grounded = start.elapsed().as_secs_f64() * 1000.0;
            source.grounded_supports = false;
            let start = Instant::now();
            std::thread::spawn(move || authoring::compile(&source).unwrap()).join().unwrap();
            let grounding = grounded - start.elapsed().as_secs_f64() * 1000.0;
            println!("{} long seed {seed}: {total:.0} ms total, {:.0} ms without grounding (grounding {grounding:.0} ms), {:.1} m, {} pieces",
                if circuit { "circuit" } else { "sprint" }, total - grounding, a.length_cm as f64 / 100.0, a.pieces.len());
            all.push(total);
        }
        let worst = all.iter().cloned().fold(0.0, f64::max);
        println!("{}: mean {:.0} ms, max {worst:.0} ms", if circuit { "circuit" } else { "sprint" }, all.iter().sum::<f64>() / all.len() as f64);
        assert!(worst < 500.0, "slowest long {} generation {worst:.0} ms", if circuit { "circuit" } else { "sprint" });
    }
}
