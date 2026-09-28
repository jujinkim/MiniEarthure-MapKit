//! Deterministic attachments to finalized road surfaces. No road RNG or geometry mutation.
use super::*;

pub const KINDS: &[&str] = &[
    "fixed_obstacle",
    "moving_obstacle",
    "rotating_obstacle",
    "jump_barrier",
    "slalom_gates",
    "swing_gates",
    "piston_gates",
];
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Obstacle {
    pub kind: String,
    pub piece_index: usize,
    /// `main` or `alternate`, never an inferred nearest road on another level.
    pub path: String,
    pub station_cm: u64,
    pub position_cm: Vertex,
    pub normal: Vertex,
    pub forward: Vertex,
    pub lateral_cm: i64,
    pub half_width_cm: u32,
    /// Centre of the lane that stays clear for the complete motion cycle.
    pub avoid_lateral_cm: i64,
    pub jump_station_cm: Option<u64>,
    pub jump_position_cm: Option<Vertex>,
}
#[derive(Clone)]
struct Site {
    piece: usize,
    branch: bool,
    station: u64,
}
fn path<'a>(a: &'a Assembly, o: &Obstacle) -> &'a [Sample] {
    let p = &a.pieces[o.piece_index];
    if o.path == "alternate" {
        &p.alternate_path
    } else {
        &p.path
    }
}
fn length(path: &[Sample]) -> u64 {
    path.windows(2)
        .map(|w| distance(w[0].position_cm, w[1].position_cm))
        .sum()
}
fn dot(a: Vertex, b: Vertex) -> f64 {
    (0..3).map(|i| a[i] as f64 * b[i] as f64 / 1e12).sum()
}
fn right(s: &Sample) -> Vertex {
    let n = s.normal.map(|v| v as f64 / 1e6);
    let f = s.forward.map(|v| v as f64 / 1e6);
    unit([
        n[1] * f[2] - n[2] * f[1],
        n[2] * f[0] - n[0] * f[2],
        n[0] * f[1] - n[1] * f[0],
    ])
}
fn offset(s: &Sample, lateral: i64) -> Vertex {
    add(
        s.position_cm,
        right(s).map(|v| round(v as f64 * lateral as f64 / 1e6)),
    )
}
fn sample(path: &[Sample], station: u64) -> Sample {
    let mut at = 0;
    for w in path.windows(2) {
        let len = distance(w[0].position_cm, w[1].position_cm);
        if station <= at + len {
            let t = (station - at) as f64 / len.max(1) as f64;
            let mut out = w[0].clone();
            out.position_cm = std::array::from_fn(|i| {
                round(w[0].position_cm[i] as f64 * (1.0 - t) + w[1].position_cm[i] as f64 * t)
            });
            out.normal = unit(std::array::from_fn(|i| {
                w[0].normal[i] as f64 * (1.0 - t) + w[1].normal[i] as f64 * t
            }));
            out.forward = unit(std::array::from_fn(|i| {
                w[0].forward[i] as f64 * (1.0 - t) + w[1].forward[i] as f64 * t
            }));
            out.lateral_cm = w[0].lateral_cm.min(w[1].lateral_cm);
            return out;
        }
        at += len;
    }
    path.last().unwrap().clone()
}
fn eligible_piece(index: usize, p: &Piece) -> bool {
    index >= 3
        && !p.id.starts_with("cylinder")
        && ![
            "tube_entry",
            "tube_exit",
            "finish_plaza",
            "loop",
            "banked_chicane",
            "jump",
            "offset_jump",
            "air_ring",
            "acceleration_panel",
            "boost_chain",
        ]
        .contains(&p.id.as_str())
}
fn safe_site(a: &Assembly, site: &Site, kind: &str, side: i64) -> Option<Obstacle> {
    let p = &a.pieces[site.piece];
    let path = if site.branch {
        &p.alternate_path
    } else {
        &p.path
    };
    let s = sample(path, site.station);
    let jump = kind == "jump_barrier";
    let slalom = kind == "slalom_gates";
    let rotating = ["rotating_obstacle", "swing_gates"].contains(&kind);
    let guard = if jump {
        450
    } else if slalom {
        350
    } else {
        250
    };
    if site.station < guard || site.station + guard > length(path) {
        return None;
    }
    // World-axis rotating objects require a level support; other attachments
    // use the road's full slope frame, including rising/falling helices.
    if s.normal[1]
        < if rotating {
            1_000_000
        } else if jump {
            999_000
        } else {
            970_000
        }
    {
        return None;
    }
    let mut half = s.lateral_cm as i64;
    for delta in (-(guard as i64)..=guard as i64).step_by(50) {
        let t = sample(path, (site.station as i64 + delta) as u64);
        if t.normal[1] < 970_000
            || t.tube_radius_cm > 0
            || t.mode == "flight"
            || dot(t.forward, s.forward) < if jump || slalom { 0.995 } else { 0.70 }
            || (rotating && t.normal[1] < 1_000_000)
        {
            return None;
        }
        half = half.min(t.lateral_cm as i64);
    }
    let sweep = match kind {
        "fixed_obstacle" => 20,
        "moving_obstacle" => 40, // 40cm box plus 40cm translation, centred on sweep
        "rotating_obstacle" => 37, // sqrt(20²+30²), full rotation
        "jump_barrier" => half - 10,
        _ => 70.min((half * 2 - 110) / 2),
    };
    if sweep < 20 || (!jump && sweep * 2 + 110 > half * 2) {
        return None;
    }
    let lateral = if jump {
        0
    } else {
        side * (half - sweep - 10).min(100)
    };
    let avoid = if jump { 0 } else { -side * (half - 50).min(60) };
    let o = Obstacle {
        kind: kind.into(),
        piece_index: site.piece,
        path: if site.branch { "alternate" } else { "main" }.into(),
        station_cm: site.station,
        position_cm: s.position_cm,
        normal: s.normal,
        forward: s.forward,
        lateral_cm: lateral,
        half_width_cm: sweep as u32,
        avoid_lateral_cm: avoid,
        jump_station_cm: jump.then_some(site.station - 250),
        jump_position_cm: jump.then(|| sample(path, site.station - 250).position_cm),
    };
    // Verify footprint corners against the actual curved/tapered road ribbon.
    // Rotation uses its complete disc, translation its complete lateral sweep.
    let reach = if slalom {
        140
    } else if rotating {
        sweep
    } else {
        30
    };
    for x in [
        lateral - sweep,
        lateral + sweep,
        if slalom { -lateral - sweep } else { lateral },
        if slalom { -lateral + sweep } else { lateral },
    ] {
        for z in [-reach, 0, reach] {
            let at = add(
                offset(&s, x),
                s.forward.map(|v| round(v as f64 * z as f64 / 1e6)),
            );
            let t = sample(path, (site.station as i64 + z) as u64);
            let r = right(&t);
            let d: Vertex = std::array::from_fn(|i| at[i] - t.position_cm[i]);
            let across: f64 = (0..3).map(|i| d[i] as f64 * r[i] as f64 / 1e6).sum();
            if across.abs() + 5.0 > t.lateral_cm as f64 {
                return None;
            }
        }
    }
    let g = gimmick(a, &o, 0);
    // Protect other roads/branches (especially the 2m overpass clearance) and
    // existing action/landing volumes. Safety envelopes enclose the full cycle.
    for (i, other) in a.pieces.iter().enumerate() {
        for (branch, samples) in [(false, &other.path), (true, &other.alternate_path)] {
            if i == site.piece && branch == site.branch {
                continue;
            }
            if samples.iter().any(|t| {
                t.position_cm[1] + 15 >= g.safety_min_cm[1]
                    && t.position_cm[1] - 15 <= g.safety_max_cm[1]
                    && [0, 2].iter().all(|&j| {
                        t.position_cm[j] + t.lateral_cm as i64 >= g.safety_min_cm[j]
                            && t.position_cm[j] - t.lateral_cm as i64 <= g.safety_max_cm[j]
                    })
            }) {
                return None;
            }
        }
    }
    Some(o)
}
pub(super) fn place(a: &Assembly) -> Result<(Vec<Obstacle>, u64, u32)> {
    if !a.settings.gimmicks.iter().any(|id| id == "obstacles") {
        return Ok((vec![], 0, 0));
    }
    let mut sites = vec![];
    let mut eligible = 0;
    for (i, p) in a.pieces.iter().enumerate() {
        cancellation::checkpoint()?;
        if !eligible_piece(i, p) {
            continue;
        }
        for (branch, path) in [(false, &p.path), (true, &p.alternate_path)] {
            if path.len() < 2 {
                continue;
            }
            // Count usable road distance, not the number of candidate centres.
            // Approach/footprint margins constrain placement without silently
            // halving difficulty density on short, joined road pieces.
            eligible += path
                .windows(2)
                .filter(|w| {
                    w.iter().all(|s| {
                        s.normal[1] >= 970_000 && s.tube_radius_cm == 0 && s.mode != "flight"
                    })
                })
                .map(|w| distance(w[0].position_cm, w[1].position_cm))
                .sum::<u64>();
            for station in (250..length(path).saturating_sub(249)).step_by(100) {
                let site = Site {
                    piece: i,
                    branch,
                    station,
                };
                if safe_site(a, &site, "fixed_obstacle", 1).is_some()
                    || safe_site(a, &site, "fixed_obstacle", -1).is_some()
                {
                    sites.push(site);
                }
            }
        }
    }
    let spacing = match a.settings.difficulty.as_str() {
        "easy" => 6400,
        "hard" => 1600,
        _ => 3200,
    };
    let target = ((eligible + spacing / 2) / spacing).max(1) as u32;
    let road_objects = road_gimmicks(a);
    let available = crate::gimmick::MAX_GIMMICKS.saturating_sub(road_objects.len());
    let count = (target as usize).min(available).min(sites.len());
    let mut rng = a.settings.seed ^ 0xd1b54a32d192ed03;
    let mut out: Vec<Obstacle> = vec![];
    for bin in 0..count {
        cancellation::checkpoint()?;
        let begin = bin * sites.len() / count;
        let end = (bin + 1) * sites.len() / count;
        let start = next(&mut rng) as usize % (end - begin);
        let kind_start = next(&mut rng) as usize % KINDS.len();
        let side = if next(&mut rng) % 2 == 0 { 1 } else { -1 };
        'search: for offset in 0..end - begin {
            let site = &sites[begin + (start + offset) % (end - begin)];
            for n in 0..KINDS.len() {
                let Some(o) = safe_site(a, site, KINDS[(kind_start + n) % KINDS.len()], side)
                else {
                    continue;
                };
                let g = gimmick(a, &o, out.len());
                let overlaps = |other: &Gimmick| {
                    (0..3).all(|j| {
                        g.safety_min_cm[j] < other.safety_max_cm[j]
                            && other.safety_min_cm[j] < g.safety_max_cm[j]
                    })
                };
                if road_objects.iter().any(overlaps) {
                    continue;
                }
                if out.iter().enumerate().any(|(i, other)| {
                    overlaps(&gimmick(a, other, i))
                        || (other.piece_index == o.piece_index
                            && other.path == o.path
                            && other.station_cm.abs_diff(o.station_cm) < 900)
                }) {
                    continue;
                }
                out.push(o);
                break 'search;
            }
        }
    }
    if out.is_empty() {
        return Err(error(
            "E_TRACK_OBSTACLES",
            "obstacles selected but no safe attachment fits the finalized roads and object budget",
        ));
    }
    Ok((out, eligible, target))
}
pub(super) fn gimmick(a: &Assembly, o: &Obstacle, index: usize) -> Gimmick {
    let s = sample(path(a, o), o.station_cm);
    let half = o.half_width_cm as i64;
    let mut g = Gimmick {
        id: format!("track-obstacle-{index}"),
        position: offset(&s, o.lateral_cm),
        rotation_mdeg: [
            round(-libm::asin(s.forward[1] as f64 / 1e6) * 180000.0 / std::f64::consts::PI) as i32,
            round(
                libm::atan2(s.forward[0] as f64, s.forward[2] as f64) * 180000.0
                    / std::f64::consts::PI,
            ) as i32,
            0,
        ],
        scale_per_mille: [1000; 3],
        parts: vec![],
        track: None,
        effect: None,
        surface: Surface::Asphalt,
        color: [245, 168, 35, 255],
        motion: Motion {
            kind: MotionKind::Static,
            delta_cm: [0; 3],
            axis: 1,
            period_ms: 4000,
            phase_ms: (index as u32 * 701) % 4000,
            impulse_cmps: [0; 3],
            cooldown_ms: 1500,
        },
        safety_min_cm: [0; 3],
        safety_max_cm: [0; 3],
    };
    let (w, h, d) = match o.kind.as_str() {
        "fixed_obstacle" | "moving_obstacle" => (40, 80, 40),
        "rotating_obstacle" => (40, 80, 60),
        "jump_barrier" => (half * 2, 36, 24),
        "swing_gates" => ((half - 2) * 2, 60, 24),
        _ => (half * 2, 60, 30),
    };
    if o.kind == "slalom_gates" {
        g.parts.push(box_part([0, h / 2, -125], [w, h, d]));
        g.parts
            .push(box_part([-2 * o.lateral_cm, h / 2, 125], [w, h, d]));
    } else {
        g.parts.push(box_part([0, h / 2, 0], [w, h, d]));
    }
    match o.kind.as_str() {
        "moving_obstacle" => {
            g.motion.kind = MotionKind::Translate;
            g.position = offset(&s, o.lateral_cm - 20);
            g.motion.delta_cm = right(&s).map(|v| round(v as f64 * 40.0 / 1e6));
        }
        "rotating_obstacle" | "swing_gates" => {
            g.motion.kind = MotionKind::Rotate;
        }
        "piston_gates" => {
            g.motion.kind = MotionKind::Translate;
            g.motion.delta_cm = s.normal.map(|v| round(v as f64 * 180.0 / 1e6));
            g.color = [160, 85, 235, 255];
        }
        _ => {}
    }
    let radius = g
        .parts
        .iter()
        .flat_map(|p| &p.vertices)
        .map(|v| v.iter().map(|n| n.abs()).sum::<i64>())
        .max()
        .unwrap();
    g.safety_min_cm = std::array::from_fn(|i| g.position[i] - radius + g.motion.delta_cm[i].min(0));
    g.safety_max_cm = std::array::from_fn(|i| g.position[i] + radius + g.motion.delta_cm[i].max(0));
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(id: &str, width: u32) -> Assembly {
        let mut pieces = vec![];
        let mut origin = [0; 3];
        for _ in 0..3 {
            push_piece(&mut pieces, &mut origin, 0, "straight", 400, false, [0; 3]);
        }
        push_piece(&mut pieces, &mut origin, 0, id, width, true, [0; 3]);
        layout::finish(
            pieces,
            &Settings {
                circuit: false,
                gimmicks: vec!["obstacles".into()],
                ..Settings::default()
            },
        )
    }
    #[test]
    fn resolved_attachments_fit_three_widths_and_use_real_motion_geometry() {
        for width in [200, 400, 600] {
            let a = fixture("sprint_lane", width);
            for kind in KINDS {
                let o = safe_site(
                    &a,
                    &Site {
                        piece: 3,
                        branch: false,
                        station: 800,
                    },
                    kind,
                    1,
                )
                .unwrap_or_else(|| panic!("{kind} width={width}"));
                let g = gimmick(&a, &o, 0);
                assert!(g.valid(), "{kind}");
                assert_eq!(g.effect, None);
                assert_eq!(o.position_cm, sample(&a.pieces[3].path, 800).position_cm);
                if *kind == "jump_barrier" {
                    let ys: Vec<_> = g.parts[0].vertices.iter().map(|v| v[1]).collect();
                    assert_eq!(ys.iter().max().unwrap() - ys.iter().min().unwrap(), 36);
                    assert_eq!(o.jump_station_cm, Some(550));
                } else {
                    assert!(o.avoid_lateral_cm.abs() + 45 <= (width / 2) as i64);
                    assert!(o.lateral_cm.abs() + o.half_width_cm as i64 <= (width / 2) as i64 - 10);
                }
                let expected = match *kind {
                    "moving_obstacle" | "piston_gates" => MotionKind::Translate,
                    "rotating_obstacle" | "swing_gates" => MotionKind::Rotate,
                    _ => MotionKind::Static,
                };
                assert_eq!(g.motion.kind, expected);
            }
        }
    }
    #[test]
    fn actual_slopes_curves_helices_and_branch_are_candidates_but_structures_and_grid_are_not() {
        for id in [
            "curve",
            "curve_left",
            "slope_up",
            "slope_down",
            "spiral_up",
            "spiral_down",
            "overpass",
        ] {
            let a = fixture(id, 400);
            let (placed, eligible, _) = place(&a).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(eligible > 0 && placed.iter().all(|o| o.piece_index == 3));
            assert!(placed.iter().any(|o| o.kind != "jump_barrier"));
            if id == "overpass" {
                assert!((400..2800).step_by(100).any(|station| safe_site(
                    &a,
                    &Site {
                        piece: 3,
                        branch: true,
                        station
                    },
                    "fixed_obstacle",
                    1
                )
                .is_some()));
            }
        }
        for id in [
            "loop",
            "cylinder",
            "banked_chicane",
            "jump",
            "offset_jump",
            "air_ring",
            "finish_plaza",
            "tube_entry",
            "tube_exit",
            "acceleration_panel",
            "boost_chain",
        ] {
            let a = fixture(id, 400);
            assert_eq!(place(&a).unwrap_err().code, "E_TRACK_OBSTACLES", "{id}");
        }
        let mut a = fixture("sprint_lane", 400);
        a.pieces.truncate(3);
        assert_eq!(place(&a).unwrap_err().code, "E_TRACK_OBSTACLES");
    }
    #[test]
    fn difficulty_density_budget_and_seed_repeatability() {
        let mut a = fixture("sprint_lane", 400);
        let mut origin = a.pieces.last().unwrap().path.last().unwrap().position_cm;
        for _ in 0..20 {
            push_piece(
                &mut a.pieces,
                &mut origin,
                0,
                "sprint_lane",
                400,
                true,
                [0; 3],
            );
        }
        let mut counts = vec![];
        for (difficulty, spacing) in [("easy", 6400), ("normal", 3200), ("hard", 1600)] {
            a.settings.difficulty = difficulty.into();
            let result = place(&a).unwrap();
            assert_eq!(result, place(&a).unwrap());
            assert_eq!(result.2, ((result.1 + spacing / 2) / spacing).max(1) as u32);
            assert_eq!(result.0.len(), result.2 as usize);
            counts.push(result.0.len());
        }
        assert!(counts[0] < counts[1] && counts[1] < counts[2]);
        // Existing object cap reduces attachments instead of adding roads.
        let mut budget = a.clone();
        for _ in 0..40 {
            push_piece(
                &mut budget.pieces,
                &mut origin,
                0,
                "boost_chain",
                400,
                false,
                [0; 3],
            );
        }
        let (o, _, target) = place(&budget).unwrap();
        assert!(o.len() < target as usize);
        assert!(o.len() + road_gimmicks(&budget).len() <= crate::gimmick::MAX_GIMMICKS);
        for _ in 0..3 {
            push_piece(
                &mut budget.pieces,
                &mut origin,
                0,
                "boost_chain",
                400,
                false,
                [0; 3],
            );
        }
        assert_eq!(place(&budget).unwrap_err().code, "E_TRACK_OBSTACLES");
    }
}
