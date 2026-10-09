use mapkit_core::*;
use mapkit_package::{water_edit, working::*};

fn snapshot() -> WorkingSnapshot {
    let mut d: MapDocument =
        serde_json::from_str(include_str!("../../../examples/minimal/document.json")).unwrap();
    d.bounds = Bounds {
        min: [0, 0],
        max: [6400, 6400],
    };
    d.cell_size_cm = 3200;
    d.free_roam = true;
    d.roads.clear();
    d.nodes.clear();
    d.buildings.clear();
    d.zones.clear();
    d.attributions.clear();
    WorkingSnapshot::new(d, Resources::default()).unwrap()
}
fn options(mode: &str) -> BrushOptions {
    BrushOptions {
        mode: mode.into(),
        radius_cm: 1600.,
        rate_cm_s: 200.,
        steepness: 0.5,
        strength: 0.5,
        target_cm: None,
    }
}

#[test]
fn timed_brush_shares_seams_and_freezes_workers() {
    let mut s = snapshot();
    let original = s.clone();
    let mut b = Brush::begin(&mut s, [3200., 3200.], options("raise")).unwrap();
    for _ in 0..60 {
        b.step(&mut s, [3200., 3200.], 1. / 60.).unwrap();
    }
    assert_eq!(s.height([3200, 3200]).unwrap(), 200);
    assert_eq!(original.clone().height([3200, 3200]).unwrap(), 0);
    let a = s.grid(Cell { x: 0, y: 0 }).unwrap();
    let c = s.grid(Cell { x: 1, y: 0 }).unwrap();
    for i in 0..a.side {
        assert_eq!(
            a.heights_cm[i * a.side + a.side - 1],
            c.heights_cm[i * c.side]
        );
    }
    assert!(b.delta().len() * 4 < 16 * 1024 * 1024);
    let (d, files) = s.materialize().unwrap();
    assert_eq!(d.heightmaps.len(), 4);
    for h in d.heightmaps {
        let grid = mapkit_package::decode_heightmap(&h, d.cell_size_cm, &files[&h.path]).unwrap();
        assert_eq!(grid.heights_cm, s.tiles[&h.cell].heights_cm);
    }
}
#[test]
fn path_interpolation_flatten_and_sparse_budget() {
    let mut s = snapshot();
    let mut b = Brush::begin(&mut s, [800., 3200.], options("raise")).unwrap();
    b.step(&mut s, [5600., 3200.], 2.).unwrap();
    for x in (1000..5600).step_by(200) {
        assert!(s.height([x, 3200]).unwrap() > 0);
    }
    let target = s.height([3200, 3200]).unwrap();
    let mut flatten = Brush::begin(&mut s, [3200., 3200.], options("flatten")).unwrap();
    flatten.step(&mut s, [4800., 3200.], 1.).unwrap();
    assert_eq!(flatten.options.target_cm, Some(target as f64));
    let mut o = options("raise");
    o.radius_cm = 100.;
    assert!(Brush::begin(&mut s, [0., 0.], o).is_err());
    assert!(b.step(&mut s, [0., 0.], f64::NAN).is_err());
}
fn bowl() -> WorkingSnapshot {
    let mut s = snapshot();
    for y in 0..=32 {
        for x in 0..=32 {
            let h = ((x - 16i32).abs().max((y - 16i32).abs()) * 100) as i64;
            s.set_sample([x, y], h).unwrap();
        }
    }
    s
}
#[test]
fn water_flat_slope_island_open_edge_and_removal() {
    let mut s = snapshot();
    assert!(water_edit::edit(&mut s, Some([1600, 1600]), false)
        .unwrap()
        .is_empty());
    let mut s = bowl();
    let water = water_edit::edit(&mut s, Some([4700, 3200]), false).unwrap();
    assert_eq!(water.len(), 1);
    assert_eq!(water[0].surface_cm, 750);
    assert_eq!(water[0].flow_cm_s, [0, 0]);
    assert!(water[0].contains_horizontal([3200, 3200]));
    s.document.water_bodies = water;
    s.set_sample([16, 16], 1200).unwrap();
    let water = water_edit::edit(&mut s, None, false).unwrap();
    assert!(!water[0].contains_horizontal([3200, 3200]));
    assert!(!water[0].islands.is_empty());
    s.document.water_bodies = water;
    assert!(water_edit::edit(&mut s, Some([3400, 3400]), true)
        .unwrap()
        .is_empty());
    let mut s = snapshot();
    for y in 0..=32 {
        for x in 0..=32 {
            s.set_sample([x, y], x as i64 * 100).unwrap();
        }
    }
    let water = water_edit::edit(&mut s, Some([3300, 3200]), false).unwrap();
    assert_eq!(water[0].surface_cm, 1650);
    assert!(water[0].polygon.iter().any(|p| p[0] == 0));
}
#[test]
fn water_splits_merges_and_higher_overlap_removes_entire_lower_lake() {
    let mut s = bowl();
    s.document.water_bodies = water_edit::edit(&mut s, Some([4700, 3200]), false).unwrap();
    for y in 0..=32 {
        s.set_sample([16, y], 1500).unwrap();
    }
    let split = water_edit::edit(&mut s, None, false).unwrap();
    assert_eq!(split.len(), 2);
    s.document.water_bodies = split;
    let removed = water_edit::edit(&mut s, Some([2600, 3200]), true).unwrap();
    assert_eq!(removed.len(), 1);
    for y in 0..=32 {
        s.set_sample([16, y], (y - 16i32).abs() as i64 * 100)
            .unwrap();
    }
    let merged = water_edit::edit(&mut s, None, false).unwrap();
    assert_eq!(merged.len(), 1);
    s.document.water_bodies = merged;
    let higher = water_edit::edit(&mut s, Some([5100, 3200]), false).unwrap();
    assert_eq!(higher.len(), 1);
    assert_eq!(higher[0].surface_cm, 950);
}
#[test]
fn cancellation_and_resource_provider_are_memory_only() {
    let mut s = bowl();
    let (d, files) = s.materialize().unwrap();
    let resources = Resources {
        memory: files
            .into_iter()
            .map(|(p, b)| (p, std::sync::Arc::new(b)))
            .collect(),
        ..Default::default()
    };
    let mut loaded = WorkingSnapshot::new(d, resources).unwrap();
    assert_eq!(loaded.height([4200, 3200]).unwrap(), 500);
    let token = cancellation::CancellationToken::default();
    token.cancel();
    token.enter();
    assert!(water_edit::edit(&mut s, Some([4700, 3200]), false).is_err());
    cancellation::CancellationToken::leave();
}

#[test]
fn long_paths_and_many_tiles_use_sample_budget_instead_of_old_caps() {
    let mut s = snapshot();
    s.document.bounds.max = [640_000, 3200];
    let mut b = Brush::begin(&mut s, [1600., 1600.], options("raise")).unwrap();
    b.step(&mut s, [638_400., 1600.], 320.).unwrap();
    assert!(s.modified.len() > 16);
    assert!(b.changes.len() > 2048);
    for x in (3200..638_400).step_by(6400) {
        assert!(s.height([x, 1600]).unwrap() > 0);
    }
}

#[test]
fn rounded_shores_share_vertices_and_complex_lakes_remain_one_operation() {
    let mut s = snapshot();
    let mut b = Brush::begin(&mut s, [3200., 3200.], options("lower")).unwrap();
    b.step(&mut s, [3200., 3200.], 3.).unwrap();
    let water = water_edit::edit(&mut s, Some([4000, 3200]), false).unwrap();
    assert_eq!(
        water.len(),
        1,
        "rounded triangle edges must not leave cracks"
    );
    let mut s = snapshot();
    s.document.bounds.max = [12800, 12800];
    for y in 0..=64 {
        for x in 0..=64 {
            s.set_sample([x, y], if x == 0 { 100 } else { 0 }).unwrap();
        }
    }
    // More islands than one v1 record accepts: split into valid fragments.
    for y in (4..60).step_by(8) {
        for x in (4..60).step_by(8) {
            s.set_sample([x, y], 200).unwrap();
        }
    }
    let water = water_edit::edit(&mut s, Some([100, 100]), false).unwrap();
    assert!(water.len() > 1);
    s.document.water_bodies = water;
    s.composed_document().validate().unwrap();
    assert!(water_edit::edit(&mut s, Some([100, 100]), true)
        .unwrap()
        .is_empty());
}

#[test]
fn blank_and_memory_generators_agree_with_materialized_project() {
    let mut s = snapshot();
    s.generate(Cell { x: 0, y: 0 }, false).unwrap();
    let mut b = Brush::begin(&mut s, [3200., 3200.], options("raise")).unwrap();
    b.step(&mut s, [3200., 3200.], 1.).unwrap();
    s.validate_memory().unwrap();
    let (_, memory) = s.generate(Cell { x: 0, y: 0 }, false).unwrap();
    let (d, files) = s.materialize().unwrap();
    let mut loaded = WorkingSnapshot::new(
        d,
        Resources {
            memory: files
                .into_iter()
                .map(|(p, b)| (p, std::sync::Arc::new(b)))
                .collect(),
            ..Default::default()
        },
    )
    .unwrap();
    let (_, published) = loaded.generate(Cell { x: 0, y: 0 }, false).unwrap();
    assert_eq!(memory.hash().unwrap(), published.hash().unwrap());
}

#[test]
fn partial_edge_keeps_regular_samples_and_exact_water_boundary() {
    let mut s = snapshot();
    s.document.bounds.max = [6300, 6250];
    for y in 0..=32 {
        for x in 0..=32 {
            s.set_sample([x, y], x as i64 * 100).unwrap();
        }
    }
    let water = water_edit::edit(&mut s, Some([6250, 3200]), false).unwrap();
    assert_eq!(water.len(), 1);
    assert_eq!(water[0].surface_cm, s.surface_height([6250, 3200]).unwrap());
    assert_eq!(water[0].polygon.iter().map(|p| p[0]).max(), Some(6250));
    assert_eq!(water[0].polygon.iter().map(|p| p[1]).max(), Some(6250));
    s.document.water_bodies = water;
    s.materialize().unwrap();
    assert!(s.set_sample([-1, 0], 0).is_err());
    assert!(s.set_sample([33, 0], 0).is_err());
    let mut options = options("flatten");
    options.target_cm = Some(f64::NAN);
    assert!(Brush::begin(&mut s, [0., 0.], options).is_err());
}
