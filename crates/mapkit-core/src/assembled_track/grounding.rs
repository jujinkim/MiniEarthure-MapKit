//! Seed-only grounding and deterministic, collidable 20cm structural columns.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Support {
    pub piece_index: usize,
    pub shape: CollisionConvex,
}

// Geometry is retained for one piece only. Special meshes use 1/100 cm, exactly
// the public mesh quantization; ordinary centimetre triangles are scaled up.
#[derive(Default)]
struct Mesh {
    faces: Vec<[Vertex; 3]>,
}
impl TrackGeometry for Mesh {
    fn triangle(&mut self, v: [Vertex; 3], _: Surface, _: &str, _: bool) -> Result<()> {
        if self.faces.len() >= 300_000 {
            return Err(error("E_BUDGET", "grounding piece mesh exceeds workspace"));
        }
        self.faces.push(v.map(|p| p.map(|n| n * 100)));
        Ok(())
    }
    fn solid(&mut self, _: &str, _: SolidShape) -> Result<()> {
        Ok(())
    }
}
fn mesh(p: &Piece, index: usize, objects: &[Gimmick]) -> Result<Mesh> {
    let mut out = Mesh::default();
    let mut walls = vec![];
    generate_piece(p, index, &mut out, &[], &mut walls)?;
    super::walls::emit(&walls, &mut out)?;
    for g in objects
        .iter()
        .filter(|g| g.id.starts_with(&format!("track-{index}-")))
    {
        if let Some(track) = &g.track {
            let m = track.mesh();
            let count = out.faces.len() + m.inner.len() + m.shell.len();
            if count > 300_000 {
                return Err(error("E_BUDGET", "grounding piece mesh exceeds workspace"));
            }
            out.faces.reserve_exact(count - out.faces.len());
            for face in m.inner.into_iter().chain(m.shell) {
                cancellation::checkpoint()?;
                out.faces.push(face.map(|v| {
                    add(
                        geometry::rotate3(v, g.rotation_mdeg),
                        g.position.map(|n| n * 100),
                    )
                }));
            }
        }
    }
    Ok(out)
}
fn height(face: &[Vertex; 3], x: f64, z: f64) -> Option<f64> {
    let [a, b, c] = face.map(|v| v.map(|n| n as f64 / 100.0));
    let det = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if det.abs() < 1e-9 {
        return None;
    }
    let u = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / det;
    let v = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / det;
    (u >= -1e-8 && v >= -1e-8 && u + v <= 1.0 + 1e-8)
        .then_some(u * a[1] + v * b[1] + (1.0 - u - v) * c[1])
}
impl Mesh {
    fn bottom(&self, x: i64, z: i64) -> Option<f64> {
        self.faces
            .iter()
            .filter(|f| {
                f.iter().map(|v| v[0]).min().unwrap() <= x * 100
                    && f.iter().map(|v| v[0]).max().unwrap() >= x * 100
                    && f.iter().map(|v| v[2]).min().unwrap() <= z * 100
                    && f.iter().map(|v| v[2]).max().unwrap() >= z * 100
            })
            .filter_map(|f| height(f, x as f64, z as f64))
            .min_by(f64::total_cmp)
    }
    // The lowest point of every intersecting face over the whole square, not
    // just four corner rays. A flat cap cannot penetrate a curved/twisted deck.
    fn cap(&self, x: i64, z: i64) -> Option<i64> {
        for dx in [-10, 10] {
            for dz in [-10, 10] {
                self.bottom(x + dx, z + dz)?;
            }
        }
        let mut lowest = f64::INFINITY;
        for face in &self.faces {
            if (0..3).filter(|&j| j != 1).any(|j| {
                let center = if j == 0 { x } else { z };
                face.iter().map(|v| v[j]).max().unwrap() < (center - 10) * 100
                    || face.iter().map(|v| v[j]).min().unwrap() > (center + 10) * 100
            }) {
                continue;
            }
            let mut poly: Vec<_> = face.iter().map(|v| v.map(|n| n as f64 / 100.0)).collect();
            for (axis, limit, lower) in [
                (0, (x - 10) as f64, true),
                (0, (x + 10) as f64, false),
                (2, (z - 10) as f64, true),
                (2, (z + 10) as f64, false),
            ] {
                let old = std::mem::take(&mut poly);
                for i in 0..old.len() {
                    let (a, b) = (old[i], old[(i + 1) % old.len()]);
                    let inside = |p: [f64; 3]| {
                        if lower {
                            p[axis] >= limit
                        } else {
                            p[axis] <= limit
                        }
                    };
                    if inside(a) {
                        poly.push(a);
                    }
                    if inside(a) != inside(b) {
                        let t = (limit - a[axis]) / (b[axis] - a[axis]);
                        poly.push(std::array::from_fn(|j| a[j] + (b[j] - a[j]) * t));
                    }
                }
            }
            for p in poly {
                lowest = lowest.min(p[1]);
            }
        }
        lowest
            .is_finite()
            .then(|| libm::floor(lowest + 1e-8) as i64)
    }
}
fn bounds(s: &Support) -> (Vertex, Vertex) {
    (
        std::array::from_fn(|j| s.shape.vertices.iter().map(|v| v[j]).min().unwrap()),
        std::array::from_fn(|j| s.shape.vertices.iter().map(|v| v[j]).max().unwrap()),
    )
}
fn column(index: usize, x: i64, z: i64, bottom: i64, top: i64) -> Support {
    let mut shape = box_part([0; 3], [20, 2, 20]);
    for v in &mut shape.vertices {
        v[0] += x;
        v[2] += z;
        v[1] = if v[1] < 0 { bottom } else { top };
    }
    Support {
        piece_index: index,
        shape,
    }
}
fn overlap(a: (Vertex, Vertex), b: (Vertex, Vertex)) -> bool {
    (0..3).all(|j| a.0[j] < b.1[j] && b.0[j] < a.1[j])
}
fn blocked_interval(s: &Sample, next: &Sample, bounds: (Vertex, Vertex)) -> Option<(f64, f64)> {
    // Project the entire column into the real local road frame. Using all eight
    // corners also covers grades, rolls and cell seams conservatively.
    let basis = geometry::basis(s);
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for mask in 0..8 {
        let p: Vertex = std::array::from_fn(|j| {
            if mask & (1 << j) == 0 {
                bounds.0[j]
            } else {
                bounds.1[j]
            }
        });
        for j in 0..3 {
            let v = (0..3)
                .map(|k| (p[k] - s.position_cm[k]) as f64 * basis[k][j])
                .sum::<f64>();
            lo[j] = lo[j].min(v);
            hi[j] = hi[j].max(v);
        }
    }
    let length = distance(s.position_cm, next.position_cm) as f64;
    // Curved quantized segments are short; reserve their full envelope.
    if hi[2] < -150.0 || lo[2] > length + 150.0 || hi[1] <= 1.0 || lo[1] >= 235.0 {
        return None;
    }
    let half = s.lateral_cm.min(next.lateral_cm) as f64;
    (hi[0] > -half && lo[0] < half).then_some((lo[0].max(-half), hi[0].min(half)))
}
fn clear_width(half: f64, mut intervals: Vec<(f64, f64)>) -> f64 {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut end = -half;
    let mut widest: f64 = 0.0;
    for (lo, hi) in intervals {
        widest = widest.max(lo - end);
        end = end.max(hi);
    }
    widest.max(half - end)
}
fn safe(a: &Assembly, candidate: &Support, placed: &[Support], objects: &[Gimmick]) -> bool {
    let area = bounds(candidate);
    if placed.iter().any(|s| overlap(area, bounds(s))) {
        return false;
    }
    // Existing authored/animated obstacles and required action safety volumes.
    if objects.iter().filter(|g| g.track.is_none()).any(|g| {
        g.occupancy_bounds()
            .iter()
            .any(|&(lo, hi)| overlap(area, (lo, hi)))
    }) {
        return false;
    }
    for (i, p) in a.pieces.iter().enumerate() {
        if area.0[0] > p.reserved_max_cm[0]
            || area.1[0] < p.reserved_min_cm[0]
            || area.0[2] > p.reserved_max_cm[2]
            || area.1[2] < p.reserved_min_cm[2]
        {
            continue;
        }
        for (branch, path) in [(false, &p.path), (true, &p.alternate_path)] {
            let mut station = 0;
            for pair in path.windows(2) {
                let (s, t) = (&pair[0], &pair[1]);
                let len = distance(s.position_cm, t.position_cm);
                let at = station;
                station += len;
                let Some(interval) = blocked_interval(s, t, area) else {
                    continue;
                };
                let grid = a
                    .routes
                    .first()
                    .is_some_and(|r| r.pieces.iter().take(START_PIECES).any(|&j| j == i));
                // Never route around columns in flight, a bore, launch/landing
                // pieces or the approved starting runway.
                if grid
                    || s.mode == "flight"
                    || t.mode == "flight"
                    || s.tube_radius_cm > 0
                    || ["jump", "offset_jump", "air_ring", "tube_entry", "tube_exit"]
                        .contains(&p.id.as_str())
                {
                    return false;
                }
                if let Some(source) = a.authoring.as_ref().or(a.seed_source.as_ref()) {
                    let id = &source.instances[i].id;
                    if source.actions.iter().any(|action| {
                        (action.piece == *id && action.sample.abs_diff(at_index(path, at)) <= 12)
                            || action.landing.as_ref().is_some_and(|landing| {
                                landing.piece == *id
                                    && landing.sample.abs_diff(at_index(path, at)) <= 12
                            })
                    }) {
                        return false;
                    }
                }
                let mut intervals = vec![interval];
                for other in placed {
                    if let Some(v) = blocked_interval(s, t, bounds(other)) {
                        intervals.push(v);
                    }
                }
                for o in &a.obstacles {
                    if o.piece_index != i || (o.path == "alternate") != branch {
                        continue;
                    }
                    // Complete moving sweep and approach/landing distance of the
                    // original obstacle. Reserve one connected lateral lane
                    // through the entire 3m neighbourhood, not isolated points.
                    if o.station_cm.abs_diff(at + len / 2) < 350 {
                        if o.jump_station_cm.is_some() {
                            return false;
                        }
                        intervals.push((
                            (o.lateral_cm - o.half_width_cm as i64) as f64,
                            (o.lateral_cm + o.half_width_cm as i64) as f64,
                        ));
                        if o.kind == "slalom_gates" {
                            intervals.push((
                                (-o.lateral_cm - o.half_width_cm as i64) as f64,
                                (-o.lateral_cm + o.half_width_cm as i64) as f64,
                            ));
                        }
                    }
                }
                if clear_width(s.lateral_cm.min(t.lateral_cm) as f64, intervals) < 110.0 {
                    return false;
                }
            }
        }
    }
    true
}
fn at_index(path: &[Sample], station: u64) -> usize {
    let mut at = 0;
    for (i, w) in path.windows(2).enumerate() {
        at += distance(w[0].position_cm, w[1].position_cm);
        if at >= station {
            return i;
        }
    }
    path.len() - 1
}

pub(super) fn apply(a: &mut Assembly) -> Result<()> {
    if a.pieces.is_empty() {
        a.supports.clear();
        return Ok(());
    }
    let road_objects = road_gimmicks(a);
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for (i, p) in a.pieces.iter().enumerate() {
        cancellation::checkpoint()?;
        for v in mesh(p, i, &road_objects)?.faces.iter().flatten() {
            for j in 0..3 {
                low[j] = low[j].min(v[j]);
                high[j] = high[j].max(v[j]);
            }
        }
    }
    if low[1] == i64::MAX {
        a.supports.clear();
        return Ok(());
    }
    let floor = low[1].div_euclid(100);
    a.floor = VenueFloor {
        min_cm: [
            low[0].div_euclid(100) - 1600,
            floor,
            low[2].div_euclid(100) - 1600,
        ],
        max_cm: [
            high[0].div_euclid(100) + 1601,
            floor,
            high[2].div_euclid(100) + 1601,
        ],
    };
    let mut objects = gimmicks(a);
    objects.extend(authoring::action_gimmicks(a)?);
    let mut supports = vec![];
    for (index, p) in a.pieces.iter().enumerate() {
        cancellation::checkpoint()?;
        // A level ordinary ribbon/plaza has exactly the emitted 10cm slab.
        // It is already grounded everywhere; no candidate ray scan is needed.
        if p.path.iter().chain(&p.alternate_path).all(|s| {
            s.position_cm[1] == floor + 10
                && s.normal == [0, 1_000_000, 0]
                && !["flight", "loop", "cylinder", "halfpipe"].contains(&s.mode.as_str())
        }) {
            continue;
        }
        let m = mesh(p, index, &road_objects)?;
        // Deterministic centre-first longitudinal search; every quantized sample
        // is considered, alternating toward entry/exit. Lateral alternatives
        // stay under this piece, including a raised alternate branch.
        let mut sites = vec![];
        for path in [&p.path, &p.alternate_path] {
            let mut order: Vec<_> = (0..path.len()).collect();
            order.sort_by_key(|&i| (i.abs_diff(path.len() / 2), i));
            for i in order {
                let s = &path[i];
                if s.mode == "flight" {
                    continue;
                }
                let basis = geometry::basis(s);
                for lateral in [0, -1, 1, -2, 2] {
                    let side = lateral as f64 * s.lateral_cm.saturating_sub(25) as f64 / 2.0;
                    sites.push((
                        s.position_cm[0] + round(basis[0][0] * side),
                        s.position_cm[2] + round(basis[2][0] * side),
                    ));
                }
            }
        }
        let mut airborne = false;
        let mut selected = None;
        for (x, z) in sites {
            cancellation::checkpoint()?;
            let Some(bottom) = m.bottom(x, z) else {
                continue;
            };
            if bottom <= floor as f64 + 1.0 {
                continue;
            }
            airborne = true;
            let Some(top) = m.cap(x, z) else {
                continue;
            };
            if top <= floor {
                continue;
            }
            let candidate = column(index, x, z, floor, top);
            if safe(a, &candidate, &supports, &objects) {
                selected = Some(candidate);
                break;
            }
        }
        if let Some(s) = selected {
            supports.push(s);
        } else if airborne {
            return Err(error("E_TRACK_SUPPORT",format!("piece {index} ({}): no 20cm support preserving the start/flight/landing space and continuous 110cm passage",p.id)));
        }
    }
    a.supports = supports;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(specs: &[(&str, Vertex, u32)]) -> Assembly {
        let pieces = specs
            .iter()
            .map(|(id, origin, width)| {
                let mut p = variant(id, *width, *width, *width);
                p.origin_cm = *origin;
                materialize(&p)
            })
            .collect();
        layout::finish(
            pieces,
            &Settings {
                circuit: false,
                ..Settings::default()
            },
        )
    }
    #[test]
    fn actual_mesh_ground_and_support_endpoints_across_shapes() {
        for id in [
            "straight",
            "slope_up",
            "slope_down",
            "spiral90_left_up",
            "spiral180_right_down",
            "spiral360_left_up",
            "cylinder",
            "cylinder_curve",
            "loop",
            "overpass",
            "finish_plaza",
        ] {
            if !catalogue_ids().contains(&id) {
                continue;
            }
            let mut a = fixture(&[("straight", [-10000, 0, 0], 400), (id, [0, 600, 0], 400)]);
            a.routes.clear(); // Isolated geometry fixtures, no approved start runway.
            let before = a.pieces.clone();
            apply(&mut a).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(
                a.pieces, before,
                "grounding never translates track/start/finish"
            );
            let objects = road_gimmicks(&a);
            let lowest = a
                .pieces
                .iter()
                .enumerate()
                .flat_map(|(i, p)| mesh(p, i, &objects).unwrap().faces)
                .flatten()
                .map(|v| v[1])
                .min()
                .unwrap();
            assert_eq!(a.floor.min_cm[1], lowest.div_euclid(100), "{id}");
            assert_eq!(
                a.supports.iter().filter(|s| s.piece_index == 1).count(),
                1,
                "{id}"
            );
            for s in &a.supports {
                let (lo, hi) = bounds(s);
                assert_eq!(hi[0] - lo[0], 20);
                assert_eq!(hi[2] - lo[2], 20);
                assert_eq!(lo[1], a.floor.min_cm[1]);
                let m = mesh(&a.pieces[s.piece_index], s.piece_index, &objects).unwrap();
                assert_eq!(Some(hi[1]), m.cap((lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2));
                for x in lo[0]..=hi[0] {
                    for z in lo[2]..=hi[2] {
                        assert!(m.bottom(x, z).unwrap() + 1e-6 >= hi[1] as f64);
                    }
                }
            }
            let first = a.clone();
            apply(&mut a).unwrap();
            assert_eq!(a, first);
        }
    }
    #[test]
    fn flat_track_has_no_columns_and_flight_does_not_lower_floor() {
        let mut a = fixture(&[
            ("straight", [0, 900, 0], 400),
            ("finish_plaza", [10000, 900, 0], 400),
        ]);
        apply(&mut a).unwrap();
        assert_eq!(a.floor.min_cm[1], 890);
        assert!(a.supports.is_empty());
        let mut flight = variant("straight", 400, 400, 400);
        flight.id = "flight_curve".into();
        flight.control_points = vec![
            [0, -1000, 0],
            [0, -500, 500],
            [0, -500, 1000],
            [0, -1000, 1500],
        ];
        a.pieces.push(materialize(&flight));
        apply(&mut a).unwrap();
        assert_eq!(a.floor.min_cm[1], 890);
        assert!(a.supports.is_empty());
    }
    #[test]
    fn partial_road_intrusion_combines_columns_and_obstacle_sweeps() {
        let mut a = fixture(&[("straight", [0, 0, 0], 200)]);
        a.routes.clear();
        let center = column(1, 0, 400, -10, 500);
        assert!(
            !safe(&a, &center, &[], &[]),
            "two 90cm gaps are not a 110cm passage"
        );
        let side = column(1, 65, 400, -10, 500);
        assert!(
            safe(&a, &side, &[], &[]),
            "partial road intrusion is allowed"
        );
        let exact = column(2, -65, 550, -10, 500);
        assert!(safe(&a, &side, &[exact], &[]), "exactly 110cm stays open");
        let other = column(2, -55, 550, -10, 500);
        assert!(
            !safe(&a, &side, &[other], &[]),
            "nearby opposite columns must retain a connected lane"
        );
        a.obstacles.push(Obstacle {
            kind: "fixed_obstacle".into(),
            piece_index: 0,
            path: "main".into(),
            station_cm: 400,
            position_cm: [0, 0, 400],
            normal: [0, 1000000, 0],
            forward: [0, 0, 1000000],
            lateral_cm: -60,
            half_width_cm: 20,
            avoid_lateral_cm: 50,
            jump_station_cm: None,
            jump_position_cm: None,
        });
        assert!(!safe(&a, &side, &[], &[]));
        a.obstacles.clear();
        a.routes.push(authoring::Route {
            id: "base".into(),
            pieces: vec![0],
            estimated_msec: 0,
        });
        assert!(!safe(&a, &side, &[], &[]), "grid may not be narrowed");
    }
    #[test]
    fn impossible_support_rejected_and_slope_still_supported() {
        let mut a = fixture(&[
            ("straight", [0, 400, 0], 400),
            ("straight", [0, 0, 0], 1200),
        ]);
        a.routes[0].pieces = vec![1];
        assert_eq!(apply(&mut a).unwrap_err().code, "E_TRACK_SUPPORT");
        let mut a = fixture(&[("slope_down", [0, 100, 0], 400)]);
        apply(&mut a).unwrap();
        assert_eq!(a.floor.min_cm[1], -10);
        assert_eq!(a.supports.len(), 1);
    }
    #[test]
    fn seed_conversion_preserves_policy_but_independent_manual_does_not_enable_it() {
        let mut a = fixture(&[
            ("straight", [0, 500, 0], 400),
            ("slope_down", [0, 500, 800], 400),
        ]);
        apply(&mut a).unwrap();
        let source = authoring::from_assembly(&a);
        assert!(source.grounded_supports);
        let compiled = authoring::compile(&source).unwrap();
        assert_eq!(compiled.floor, a.floor);
        assert_eq!(compiled.supports, a.supports);
        let mut manual = source;
        manual.grounded_supports = false;
        manual.original_seed = None;
        let ungrounded = authoring::compile(&manual).unwrap();
        assert!(ungrounded.supports.is_empty());
        assert!(ungrounded.floor.min_cm[1] < a.floor.min_cm[1]);
        assert!(!authoring::Source::empty().grounded_supports);
    }
    #[test]
    fn support_cell_seams_share_collision_occupancy_cost_and_identity() {
        let mut source = authoring::Source::empty();
        source.grounded_supports = true;
        source
            .instances
            .push(authoring::instance("low", "straight", 400));
        let mut upper = authoring::instance("upper", "straight", 400);
        upper.position_cm = [12800, 400, 0];
        source.instances.push(upper);
        let mut d = document_from_assembly(authoring::compile(&source).unwrap()).unwrap();
        let support = &d.assembled_track.as_ref().unwrap().supports[0];
        let (lo, hi) = bounds(support);
        let x = (lo[0] + hi[0]) / 2;
        let z = (lo[2] + hi[2]) / 2;
        d.bounds.min[0] = x - d.cell_size_cm as i64 * 2;
        let mut edges = vec![];
        for px in [x - 1, x + 1] {
            let cell = d.cell_at([px, z]).unwrap();
            let cost = crate::estimate_generation(&d, cell, 500_000).unwrap();
            let generated = crate::generate_with_occupancy(
                GenerationInput {
                    document: &d,
                    cell,
                    heightgrid: None,
                    max_triangles: 500_000,
                },
                200_000,
            )
            .unwrap();
            assert!(generated.chunk.triangles.len() as u64 <= cost.triangles);
            assert!(generated.solids.len() as u64 <= cost.occupied_solids);
            let id = "assembled-support-1";
            assert!(
                generated
                    .solids
                    .iter()
                    .any(|s| s.object_id == id
                        && s.shape == SolidShape::Convex(support.shape.clone()))
            );
            let faces: Vec<_> = generated
                .chunk
                .triangles
                .iter()
                .filter(|t| t.object_id == id)
                .collect();
            assert!(!faces.is_empty());
            assert!(faces.iter().all(|t| !t.spawnable && t.contact_class == 0));
            edges.push(
                faces
                    .iter()
                    .flat_map(|t| t.vertices)
                    .filter(|v| v[0] == x)
                    .collect::<std::collections::BTreeSet<_>>(),
            );
        }
        assert!(!edges[0].is_empty());
        assert_eq!(edges[0], edges[1]);
    }
}
