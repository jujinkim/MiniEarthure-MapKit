//! Optional disconnected stepping-stone routes. All dimensions are authored cm.
use super::*;
use authoring::{Checkpoint, Connection, Path, Source};

pub const IDS: [&str; 4] = [
    "jump_shortcut_5",
    "jump_shortcut_6",
    "jump_shortcut_7",
    "jump_shortcut_8",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub position_cm: Vertex,
    pub forward: Vertex,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub position_cm: Vertex,
    pub forward: Vertex,
    pub size_cm: Vertex,
    pub jump_height_cm: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub piece: String,
    pub first_sample: usize,
    pub last_sample: usize,
    pub side: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    pub id: String,
    pub flight_piece: String,
    pub start_ring: Gate,
    pub end_ring: Gate,
    pub platforms: Vec<Platform>,
    pub openings: Vec<Opening>,
    pub recovery: Checkpoint,
    pub merge: Checkpoint,
}

#[allow(dead_code)] // Seeded installation returns with the jump rules.
pub(super) fn ticket(seed: u64, ordinal: usize) -> (bool, usize) {
    let mut rng = seed ^ 0x6a75_6d70_3030_3031;
    let mut n = 0;
    for _ in 0..=ordinal {
        loop {
            n = next(&mut rng);
            if n < u64::MAX {
                break;
            }
        }
    }
    (n % 3 == 0, 5 + ((n >> 32) % 4) as usize)
}

fn direction(a: Vertex, b: Vertex) -> Vertex {
    let d = [(b[0] - a[0]) as f64, 0.0, (b[2] - a[2]) as f64];
    let length = libm::sqrt(d[0] * d[0] + d[2] * d[2]);
    d.map(|v| round(v / length * 1e6))
}
fn curve(id: &str, points: Vec<Vertex>, flight: bool) -> authoring::Instance {
    let mut p = authoring::instance(id, if flight { "flight_curve" } else { "free_curve" }, 800);
    p.control_points = points;
    p
}

pub fn source(count: usize, branch: bool) -> Result<Source> {
    if !(5..=8).contains(&count) {
        return Err(error("E_TRACK_SOURCE", "jump route needs 5..8 platforms"));
    }
    let end = (count as i64 - 1) * 1600 + 4200;
    let lateral = -6000 - (count as i64 - 5) * 1600;
    let mut s = Source::empty();
    s.settings.circuit = false;
    s.instances = vec![
        curve(
            "split",
            vec![[0, 0, 0], [0, 0, 533], [0, 0, 1067], [0, 0, 1600]],
            false,
        ),
        curve(
            "entry",
            vec![[0, 0, 1600], [0, 0, 1867], [0, 0, 2133], [0, 0, 2400]],
            false,
        ),
        curve(
            "corner",
            vec![
                [0, 0, 2400],
                [0, 0, 3400],
                [lateral, 0, 3400],
                [lateral, 0, end / 2],
                [lateral, 0, end - 1800],
                [0, 0, end - 1800],
                [0, 0, end],
            ],
            false,
        ),
        curve(
            "merge",
            vec![
                [0, 0, end],
                [0, 0, end + 533],
                [0, 0, end + 1067],
                [0, 0, end + 1600],
            ],
            false,
        ),
    ];
    s.paths.push(Path {
        id: "base".into(),
        pieces: ["split", "entry", "corner", "merge"]
            .map(String::from)
            .to_vec(),
    });
    if branch {
        s.instances.push(curve(
            "flight",
            vec![
                [0, 0, 1600],
                [1200, 0, 2400],
                [1200, 0, end - 800],
                [0, 0, end],
            ],
            true,
        ));
        s.instances.last_mut().unwrap().width_cm = 400;
        s.paths.push(Path {
            id: "jump".into(),
            pieces: ["split", "flight", "merge"].map(String::from).to_vec(),
        });
        let positions: Vec<Vertex> = (0..count)
            .map(|i| {
                [
                    900 + if i > 0 && i + 1 < count && i % 2 == 0 {
                        50
                    } else {
                        0
                    },
                    -50 + 50 * i as i64 / (count as i64 - 1),
                    2600 + i as i64 * 1600,
                ]
            })
            .collect();
        let route: Vec<_> = [[0, 0, 1600], [900, -50, 2400]]
            .into_iter()
            .chain(positions.iter().copied())
            .chain([[900, 0, end - 800], [0, 0, end]])
            .collect();
        let mut controls = vec![route[0]];
        for w in route.windows(2) {
            controls.push(std::array::from_fn(|k| w[0][k] + (w[1][k] - w[0][k]) / 3));
            controls.push(std::array::from_fn(|k| {
                w[0][k] + (w[1][k] - w[0][k]) * 2 / 3
            }));
            controls.push(w[1]);
        }
        s.instances.last_mut().unwrap().control_points = controls;
        let merge = [0, 0, end];
        let platforms = positions
            .iter()
            .enumerate()
            .map(|(i, &p)| Platform {
                position_cm: p,
                forward: direction(p, positions.get(i + 1).copied().unwrap_or(merge)),
                size_cm: [600, 40, 400],
                jump_height_cm: 125,
            })
            .collect();
        s.jump_shortcuts.push(Shortcut {
            id: "jump".into(),
            flight_piece: "flight".into(),
            start_ring: Gate {
                position_cm: [400, 90, 2100],
                forward: direction([400, 0, 2100], positions[0]),
            },
            end_ring: Gate {
                position_cm: [400, 120, end - 500],
                forward: direction(*positions.last().unwrap(), merge),
            },
            platforms,
            openings: vec![
                Opening {
                    piece: "entry".into(),
                    first_sample: 0,
                    last_sample: authoring::piece(&s.instances[1])?.path.len() - 1,
                    side: 1,
                },
                Opening {
                    piece: "corner".into(),
                    first_sample: authoring::piece(&s.instances[2])?
                        .path
                        .iter()
                        .rposition(|p| p.position_cm[2] < end - 800)
                        .unwrap_or(0),
                    last_sample: authoring::piece(&s.instances[2])?.path.len() - 1,
                    side: 1,
                },
            ],
            recovery: Checkpoint {
                piece: "split".into(),
                sample: authoring::piece(&s.instances[0])?
                    .path
                    .iter()
                    .rposition(|p| p.position_cm[2] <= 1300)
                    .unwrap(),
            },
            merge: Checkpoint {
                piece: "merge".into(),
                sample: 0,
            },
        });
    }
    for path in &s.paths {
        for pair in path.pieces.windows(2) {
            let edge = Connection {
                from: pair[0].clone(),
                to: pair[1].clone(),
            };
            if !s.connections.contains(&edge) {
                s.connections.push(edge);
            }
        }
    }
    s.checkpoints = vec![
        Checkpoint {
            piece: "split".into(),
            sample: 0,
        },
        Checkpoint {
            piece: "merge".into(),
            sample: 1,
        },
    ];
    Ok(s)
}

pub(super) fn validate(s: &Source, pieces: &[Piece]) -> Result<()> {
    let fail = || {
        error(
            "E_TRACK_SOURCE",
            "invalid jump shortcut references or geometry",
        )
    };
    let mut ids = std::collections::BTreeSet::new();
    if s.jump_shortcuts.len() > 2 {
        return Err(fail());
    }
    if !s.jump_shortcuts.is_empty()
        && (s.instances.len() != pieces.len()
            || pieces.iter().any(|p| p.path.len() < 2)
            || s.paths
                .iter()
                .flat_map(|p| &p.pieces)
                .any(|id| !s.instances.iter().any(|i| &i.id == id)))
    {
        return Err(fail());
    }
    for j in &s.jump_shortcuts {
        if j.id.is_empty()
            || j.id.len() > 80
            || !ids.insert(&j.id)
            || !(5..=8).contains(&j.platforms.len())
            || !s
                .instances
                .iter()
                .any(|i| i.id == j.flight_piece && i.preset == "flight_curve")
            || j.openings.len() != 2
        {
            return Err(fail());
        }
        for cp in [&j.recovery, &j.merge] {
            let i = s
                .instances
                .iter()
                .find(|i| i.id == cp.piece)
                .ok_or_else(fail)?;
            let p = &pieces[s
                .instances
                .iter()
                .position(|v| v.id == i.id)
                .ok_or_else(fail)?];
            if cp.sample >= p.path.len() || !p.path[cp.sample].safe {
                return Err(fail());
            }
        }
        for o in &j.openings {
            let i = s
                .instances
                .iter()
                .find(|i| i.id == o.piece)
                .ok_or_else(fail)?;
            if ![-1, 1].contains(&o.side)
                || o.first_sample >= o.last_sample
                || o.last_sample
                    >= pieces[s
                        .instances
                        .iter()
                        .position(|v| v.id == i.id)
                        .ok_or_else(fail)?]
                    .path
                    .len()
            {
                return Err(fail());
            }
        }
        for (point, forward) in [&j.start_ring, &j.end_ring]
            .iter()
            .map(|g| (g.position_cm, g.forward))
            .chain(j.platforms.iter().map(|p| (p.position_cm, p.forward)))
        {
            if point.iter().any(|v| v.unsigned_abs() > 9_900_000)
                || forward[1] != 0
                || !(999_000..=1_001_000).contains(&distance([0; 3], forward))
            {
                return Err(fail());
            }
        }
        for p in &j.platforms {
            if p.size_cm != [600, 40, 400] || p.jump_height_cm != 125 {
                return Err(fail());
            }
        }
        for w in j.platforms.windows(2) {
            if !(1500..=1700).contains(&distance(w[0].position_cm, w[1].position_cm))
                || w[0].position_cm[1].abs_diff(w[1].position_cm[1]) > 30
            {
                return Err(fail());
            }
        }
        for w in j.platforms.windows(3) {
            if distance(w[0].position_cm, w[2].position_cm) < 3150 {
                return Err(fail());
            }
        }
        let main = &s.paths.first().ok_or_else(fail)?.pieces;
        let first = main
            .iter()
            .position(|id| id == &j.recovery.piece)
            .ok_or_else(fail)?;
        let last = main
            .iter()
            .position(|id| id == &j.merge.piece)
            .ok_or_else(fail)?;
        if first + 1 >= last
            || !s.paths.iter().any(|p| {
                p.pieces.windows(3).any(|w| {
                    w == [
                        j.recovery.piece.clone(),
                        j.flight_piece.clone(),
                        j.merge.piece.clone(),
                    ]
                })
            })
            || j.openings
                .iter()
                .any(|o| !main[first + 1..last].contains(&o.piece))
        {
            return Err(fail());
        }
        let piece = |id: &str| &pieces[s.instances.iter().position(|i| i.id == id).unwrap()];
        let main_length: u64 = main[first + 1..last]
            .iter()
            .map(|id| {
                piece(id)
                    .path
                    .windows(2)
                    .map(|w| distance(w[0].position_cm, w[1].position_cm))
                    .sum::<u64>()
            })
            .sum();
        let landing = piece(&j.merge.piece).path[j.merge.sample].position_cm;
        let entry = piece(&j.recovery.piece).path.last().unwrap().position_cm;
        let route: Vec<_> = [entry, j.start_ring.position_cm]
            .into_iter()
            .chain(j.platforms.iter().map(|p| p.position_cm))
            .chain([j.end_ring.position_cm, landing])
            .collect();
        if route.windows(2).map(|w| distance(w[0], w[1])).sum::<u64>() * 10 > main_length * 7 {
            return Err(fail());
        }
        for (index, p) in j.platforms.iter().enumerate() {
            let target = j
                .platforms
                .get(index + 1)
                .map_or(landing, |p| p.position_cm);
            let expected = direction(p.position_cm, target);
            if distance(expected, p.forward) > 20_000 {
                return Err(fail());
            }
        }
    }
    Ok(())
}

// Flight pieces are route hints, not solids. Their shared approach/merge can
// overlap their own main branch; installation checks the real platform corridor.
pub(super) fn shared_approach(s: &Source, a: usize, b: usize) -> bool {
    s.jump_shortcuts.iter().any(|j| {
        let other = if s.instances[a].id == j.flight_piece {
            &s.instances[b].id
        } else if s.instances[b].id == j.flight_piece {
            &s.instances[a].id
        } else {
            return false;
        };
        other == &j.recovery.piece
            || other == &j.merge.piece
            || j.openings.iter().any(|o| &o.piece == other)
    })
}

pub(super) fn openings(a: &Assembly, index: usize) -> Vec<&Opening> {
    let Some(s) = a.authoring.as_ref().or(a.seed_source.as_ref()) else {
        return vec![];
    };
    s.jump_shortcuts
        .iter()
        .flat_map(|j| &j.openings)
        .filter(|o| o.piece == s.instances[index].id)
        .collect()
}

pub(super) fn gimmicks(s: &Source) -> Vec<Gimmick> {
    let mut out = vec![];
    for j in &s.jump_shortcuts {
        let make = |suffix: &str,
                    position: Vertex,
                    forward: Vertex,
                    parts: Vec<CollisionConvex>,
                    kind: MotionKind| {
            let effect = if kind == MotionKind::Static {
                None
            } else {
                Some(Effect {
                    strength_percent: 100,
                    jump_height_cm: 125,
                    ring_radius_cm: 250,
                })
            };
            Gimmick {
                id: format!("jump-{}-{suffix}", j.id),
                position,
                rotation_mdeg: [
                    0,
                    round(
                        libm::atan2(forward[0] as f64, forward[2] as f64) * 180000.0
                            / std::f64::consts::PI,
                    ) as i32,
                    0,
                ],
                scale_per_mille: [1000; 3],
                parts,
                curved_faces: vec![],
                track: None,
                effect,
                surface: Surface::Asphalt,
                color: [35, 210, 190, 255],
                motion: Motion {
                    kind,
                    delta_cm: [0; 3],
                    axis: 1,
                    period_ms: 4000,
                    phase_ms: 0,
                    impulse_cmps: [0; 3],
                    cooldown_ms: 100,
                },
                safety_min_cm: position.map(|v| v - 1800),
                safety_max_cm: position.map(|v| v + 1800),
            }
        };
        for (name, g) in [("start", &j.start_ring), ("end", &j.end_ring)] {
            let mut parts = vec![];
            for side in [-1, 1] {
                parts.push(box_part([side * 265, 0, 0], [30, 560, 30]));
                parts.push(box_part([0, side * 265, 0], [500, 30, 30]));
            }
            out.push(make(
                name,
                g.position_cm,
                g.forward,
                parts,
                MotionKind::AirRing,
            ));
        }
        for (i, p) in j.platforms.iter().enumerate() {
            out.push(make(
                &format!("solid-{i}"),
                p.position_cm,
                p.forward,
                vec![box_part([0, -20, 0], p.size_cm)],
                MotionKind::Static,
            ));
            out.push(make(
                &format!("pad-{i}"),
                p.position_cm,
                p.forward,
                vec![box_part([0, 1, 0], [p.size_cm[0], 2, p.size_cm[2]])],
                MotionKind::JumpHeight,
            ));
        }
    }
    out
}

#[allow(dead_code)]
pub(super) fn install(
    source: &Source,
    fragment: &Source,
    ids: &[String],
    prefix: &str,
) -> Result<Option<Source>> {
    let attempt = || -> Result<Source> {
        let j = &fragment.jump_shortcuts[0];
        let main = fragment.instances[1..3]
            .iter()
            .map(|i| {
                authoring::piece(i).map(|p| {
                    p.path
                        .windows(2)
                        .map(|w| distance(w[0].position_cm, w[1].position_cm))
                        .sum::<u64>()
                })
            })
            .sum::<Result<u64>>()?;
        let path: Vec<_> = std::iter::once(j.start_ring.position_cm)
            .chain(j.platforms.iter().map(|p| p.position_cm))
            .chain([
                j.end_ring.position_cm,
                authoring::piece(&fragment.instances[3])?.path[j.merge.sample].position_cm,
            ])
            .collect();
        let length = path.windows(2).map(|w| distance(w[0], w[1])).sum::<u64>() + 800;
        if length * 10 > main * 7 {
            return Err(error(
                "E_TRACK_CLEARANCE",
                "jump route must save 30% including entry/merge",
            ));
        }
        let mut candidate = source.clone();
        templates::merge(&mut candidate, fragment, ids, prefix);
        candidate = corner_shortcuts::with_checkpoints(candidate)?;
        let a = authoring::compile(&candidate)?;
        document_from_assembly(a)?;
        Ok(candidate)
    };
    match crate::profile::runtime(attempt) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.code == "E_CANCELLED" => Err(e),
        Err(_) => Ok(None),
    }
}

/// Exact authored solid occupancy and sampled entry/flight/landing vehicle space.
pub(super) fn clear(a: &Assembly, jump: &Shortcut) -> Result<()> {
    let candidate = a.authoring.as_ref().or(a.seed_source.as_ref()).unwrap();
    let base = &candidate.paths[0].pieces;
    let first = base
        .iter()
        .position(|id| id == &jump.recovery.piece)
        .ok_or_else(|| error("E_TRACK_SOURCE", "jump entry is not on main path"))?;
    let last = base
        .iter()
        .position(|id| id == &jump.merge.piece)
        .ok_or_else(|| error("E_TRACK_SOURCE", "jump merge is not on main path"))?;
    if first >= last {
        return Err(error("E_TRACK_SOURCE", "jump entry must precede merge"));
    }
    let ids = &base[first..=last];
    let mut solids = gimmicks(candidate);
    solids.extend(super::gimmicks(a)?);
    solids.extend(candidate.structures.clone());
    let mut boxes: Vec<_> = solids
        .iter()
        .filter(|g| !g.id.starts_with(&format!("jump-{}-", jump.id)))
        .flat_map(|g| g.occupancy_bounds())
        .collect();
    boxes.extend(a.supports.iter().map(|support| {
        let v = &support.shape.vertices;
        (
            std::array::from_fn(|k| v.iter().map(|p| p[k]).min().unwrap()),
            std::array::from_fn(|k| v.iter().map(|p| p[k]).max().unwrap()),
        )
    }));
    for g in gimmicks(candidate)
        .iter()
        .filter(|g| g.id.starts_with(&format!("jump-{}-solid-", jump.id)))
    {
        if g.occupancy_bounds().iter().any(|(lo, hi)| {
            boxes
                .iter()
                .any(|(b, c)| (0..3).all(|k| lo[k] < c[k] && b[k] < hi[k]))
        }) {
            return Err(error("E_TRACK_CLEARANCE", "platform solid is occupied"));
        }
    }
    // Reserve the complete entry, airborne corridor and supported merge.
    let points: Vec<_> = std::iter::once(jump.start_ring.position_cm)
        .chain(jump.platforms.iter().map(|p| p.position_cm))
        .chain([
            jump.end_ring.position_cm,
            a.pieces[candidate
                .instances
                .iter()
                .position(|i| i.id == jump.merge.piece)
                .unwrap()]
            .path[jump.merge.sample]
                .position_cm,
        ])
        .collect();
    for w in points.windows(2) {
        for n in 0..=distance(w[0], w[1]).div_ceil(50) {
            cancellation::checkpoint()?;
            let count = distance(w[0], w[1]).div_ceil(50).max(1);
            let p: Vertex =
                std::array::from_fn(|k| w[0][k] + (w[1][k] - w[0][k]) * n as i64 / count as i64);
            if boxes.iter().any(|(lo, hi)| {
                (0..3).all(|k| {
                    p[k] - if k == 1 { 10 } else { 220 } < hi[k]
                        && lo[k] < p[k] + if k == 1 { 240 } else { 220 }
                })
            }) {
                return Err(error("E_TRACK_CLEARANCE", "jump corridor occupied"));
            }
            for (i, road) in a.pieces.iter().enumerate() {
                if candidate.instances[i].id == jump.flight_piece {
                    continue;
                }
                if ids.contains(&candidate.instances[i].id)
                    && (distance(p, jump.start_ring.position_cm) < 1200
                        || distance(p, *points.last().unwrap()) < 1600)
                {
                    continue;
                }
                if road.path.iter().any(|r| {
                    r.mode != "flight"
                        && (r.position_cm[1] - p[1]).abs() < 300
                        && (r.position_cm[0] - p[0]).pow(2) + (r.position_cm[2] - p[2]).pow(2)
                            < (r.lateral_cm as i64 + 220).pow(2)
                }) {
                    return Err(error("E_TRACK_CLEARANCE", "jump corridor intersects road"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installs_on_clear_long_main() {
        let base = source(5, false).unwrap();
        let fragment = source(5, true).unwrap();
        assert!(install(&base, &fragment, &base.paths[0].pieces, "j0")
            .unwrap()
            .is_some());
    }
    #[test]
    fn independent_tickets_and_atomic_rejection() {
        let tickets: Vec<_> = (0..900).map(|seed| ticket(seed, 0)).collect();
        let selected = tickets.iter().filter(|v| v.0).count();
        assert!((240..360).contains(&selected));
        assert_eq!(
            tickets,
            (0..900).map(|seed| ticket(seed, 0)).collect::<Vec<_>>()
        );
        assert_ne!(
            tickets,
            (0..900).map(|seed| ticket(seed, 1)).collect::<Vec<_>>()
        );
        let fragment = source(5, true).unwrap();
        let base = source(5, false).unwrap();
        for at in [
            fragment.jump_shortcuts[0].start_ring.position_cm,
            fragment.jump_shortcuts[0].platforms[2].position_cm,
            [0, 100, 10600],
        ] {
            let mut blocked = base.clone();
            let mut obstacle = gimmicks(&fragment)[2].clone();
            obstacle.id = "authored-blocker".into();
            obstacle.position = at;
            obstacle.parts = vec![box_part([0, 100, 0], [200; 3])];
            obstacle.safety_min_cm = at.map(|v| v - 300);
            obstacle.safety_max_cm = at.map(|v| v + 300);
            blocked.structures.push(obstacle);
            assert!(install(&blocked, &fragment, &base.paths[0].pieces, "one")
                .unwrap()
                .is_none());
            assert!(blocked.jump_shortcuts.is_empty());
            assert_eq!(blocked.paths, base.paths);
        }
    }
    #[test]
    fn two_jumps_and_rail_share_all_checkpoint_combinations() {
        let first = source(5, true).unwrap();
        let both = templates::append(&first, "jump_shortcut_5", "merge", "two").unwrap();
        let both = corner_shortcuts::with_checkpoints(both).unwrap();
        assert_eq!(both.jump_shortcuts.len(), 2);
        assert_eq!(both.paths.len(), 4);
        let mixed = templates::append(&both, "grind_s_right", "two-merge", "rail").unwrap();
        let mixed = corner_shortcuts::with_checkpoints(mixed).unwrap();
        let a = authoring::compile(&mixed).unwrap();
        authoring::executable(&a).unwrap();
        document_from_assembly(a.clone()).unwrap();
        assert_eq!(mixed.paths.len(), 8);
        for route in &mixed.paths {
            let order: Vec<_> = mixed
                .checkpoints
                .iter()
                .map(|cp| {
                    (
                        route.pieces.iter().position(|p| p == &cp.piece).unwrap(),
                        cp.sample,
                    )
                })
                .collect();
            assert!(order.windows(2).all(|w| w[0] < w[1]));
        }
        assert_eq!(
            authoring::compile(&authoring::from_assembly(&a)).unwrap(),
            a
        );
        let third = templates::append(&both, "jump_shortcut_5", "two-merge", "third").unwrap();
        assert!(authoring::compile(&third).is_err());
    }
    #[test]
    fn openings_remove_shared_wall_geometry_without_removing_road() {
        #[derive(Default)]
        struct Mesh {
            road: Vec<[Vertex; 3]>,
            wall: Vec<[Vertex; 3]>,
            solids: Vec<CollisionConvex>,
        }
        impl TrackGeometry for Mesh {
            fn triangle(&mut self, v: [Vertex; 3], _: Surface, id: &str, _: bool) -> Result<()> {
                if id.starts_with("assembled-wall-") {
                    self.wall.push(v);
                } else {
                    self.road.push(v);
                }
                Ok(())
            }
            fn solid(&mut self, id: &str, shape: SolidShape) -> Result<()> {
                if id.starts_with("assembled-wall-") {
                    if let SolidShape::Convex(c) = shape {
                        self.solids.push(c);
                    }
                }
                Ok(())
            }
        }
        let s = source(5, true).unwrap();
        let a = authoring::compile(&s).unwrap();
        let mut meshes = vec![];
        for gap in [false, true] {
            let mut mesh = Mesh::default();
            let mut walls = vec![];
            let openings = if gap {
                vec![&s.jump_shortcuts[0].openings[0]]
            } else {
                vec![]
            };
            generate_piece(&a.pieces[1], 1, &mut mesh, &[], &openings, &mut walls).unwrap();
            if gap {
                assert!(walls
                    .iter()
                    .all(|w| w.rings.iter().flatten().all(|v| v[0] < 0)));
            }
            walls::emit(&walls, &mut mesh).unwrap();
            meshes.push(mesh);
        }
        assert_eq!(meshes[0].road, meshes[1].road);
        assert_eq!(
            meshes[0]
                .wall
                .iter()
                .filter(|f| f.iter().all(|v| v[0] < 0))
                .collect::<Vec<_>>(),
            meshes[1].wall.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            meshes[0]
                .solids
                .iter()
                .filter(|s| s.vertices.iter().all(|v| v[0] < 0))
                .collect::<Vec<_>>(),
            meshes[1].solids.iter().collect::<Vec<_>>()
        );
        assert!(a.floor.min_cm[1] <= -450);
    }
}
