//! Runtime refuses document budgets exactly as before; Authoring warns.
use mapkit_core::*;

fn document() -> MapDocument {
    serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap()
}
fn refused(d: &MapDocument) -> Error {
    d.validate().unwrap_err()
}
fn warned(d: &MapDocument, code: &str) -> Warning {
    let warnings = d.validate_authoring().unwrap();
    warnings.into_iter().find(|w| w.code == code).unwrap()
}

#[test]
fn objects_above_runtime_budget_warn_only_when_authoring() {
    let mut d = document();
    let node = d.nodes[0].clone();
    let extra = 200_001 - d.nodes.len() - d.roads.len();
    for i in 0..extra {
        let mut n = node.clone();
        n.id = format!("extra-{i}");
        d.nodes.push(n);
    }
    d.normalize();
    assert_eq!(refused(&d), error("E_LIMIT", "too many objects"));
    let w = warned(&d, "objects");
    assert_eq!((w.limit, w.runtime_rejects), (200_000, true));
    assert!(w.actual > 200_000);
    // Profile is scoped: the same document is refused again afterwards.
    assert_eq!(refused(&d), error("E_LIMIT", "too many objects"));
}

#[test]
fn single_package_cell_budget_warns_only_when_authoring() {
    let mut d = document();
    d.heightmaps.clear();
    d.cell_size_cm = 200;
    d.bounds.min = [0, 0];
    d.bounds.max = [200 * 129, 200 * 128];
    assert_eq!(refused(&d), error("E_LIMIT", "too many map cells"));
    let w = warned(&d, "map_cells");
    assert_eq!((w.actual, w.limit, w.runtime_rejects), (129 * 128, 16_384, true));
    // Raised anti-OOM guard still refuses in both profiles.
    d.bounds.max = [200 * 512, 200 * 512];
    assert_eq!(d.validate_authoring().unwrap_err(), error("E_LIMIT", "too many map cells"));
}

#[test]
fn junction_arms_warn_only_when_authoring() {
    let mut d = document();
    let r = d.roads.iter().find(|r| r.id == "branch").unwrap().clone();
    for i in 0..33 {
        let mut r = r.clone();
        r.id = format!("duplicate-arm-{i}");
        d.roads.push(r);
    }
    d.normalize();
    assert_eq!(refused(&d), error("E_LIMIT", "road junction exceeds 32 arms"));
    let w = warned(&d, "road_junction_arms");
    assert_eq!((w.limit, w.runtime_rejects), (32, true));
    assert!(w.actual > 32);
}

/// Synthetic 6×6 street grid (the road_course test fixture).
fn street_grid() -> MapDocument {
    let mut d = document();
    let (n, block) = (6, 10_000);
    d.bounds.max = [block * (n - 1) + 8_000, block * (n - 1) + 8_000];
    d.bounds.min = [-8_000, -8_000];
    let id = |x: i64, z: i64| format!("n{x}-{z}");
    let (mut nodes, mut roads) = (vec![], vec![]);
    for x in 0..n { for z in 0..n {
        nodes.push(serde_json::json!({"id":id(x,z),"position":[x*block,0,z*block],"level":0}));
        for (dx, dz) in [(1, 0), (0, 1)] {
            if x + dx >= n || z + dz >= n { continue; }
            roads.push(serde_json::json!({"id":format!("r{x}-{z}-{dx}{dz}"),"from":id(x,z),"to":id(x+dx,z+dz),
                "points":[[x*block,0,z*block],[(x+dx)*block,0,(z+dz)*block]],"widths_cm":[800],"surfaces":["asphalt"],
                "kind":"ground","clearance_cm":null,"sidewalk_cm":null}));
        }
    }}
    d.nodes = serde_json::from_value(nodes.into()).unwrap();
    d.roads = serde_json::from_value(roads.into()).unwrap();
    d
}

#[test]
fn seeded_course_candidates_are_identical_under_authoring() {
    use mapkit_core::{course::Mode, road_course::{suggest, Request}};
    let d = street_grid();
    let r = Request { start_cm: [5_000, 0], heading: [1, 0], mode: Mode::Circuit, duration_seconds: 90,
        min_width_cm: 0, gimmicks: 4, seed: 11, shortcuts: 2 };
    let runtime = suggest(&d, &r).unwrap();
    let (authoring, warnings) = Profile::Authoring.run(|| suggest(&d, &r));
    assert_eq!(authoring.unwrap(), runtime);
    assert!(warnings.is_empty(), "{warnings:?}");
}
