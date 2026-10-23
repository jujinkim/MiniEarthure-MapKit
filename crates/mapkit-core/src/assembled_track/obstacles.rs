//! Deterministic attachments to finalized road surfaces. No road RNG or geometry mutation.
use super::*;

pub const KINDS: &[&str] = &[
    "ramp_low",
    "ramp_standard",
    "ramp_triple",
    "quarterpipe_left",
    "quarterpipe_right",
    "grind_rail",
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
fn automatic_kind(kind: &str, side: i64) -> &str {
    if kind.starts_with("quarterpipe") {
        if side < 0 { "quarterpipe_left" } else { "quarterpipe_right" }
    } else { kind }
}
fn obstacle_rotation(s: &Sample) -> [i32; 3] {
    [round(-libm::asin(s.forward[1] as f64 / 1e6) * 180000.0 / std::f64::consts::PI) as i32,
     round(libm::atan2(s.forward[0] as f64, s.forward[2] as f64) * 180000.0 / std::f64::consts::PI) as i32, 0]
}
/// Actual asymmetric local collision bounds; metadata remains a conservative
/// radius for stored v1 readers. Generation and support clearance share this.
pub(super) fn lateral_bounds(o: &Obstacle) -> (i64, i64) {
    if o.kind.starts_with("quarterpipe") {
        let parts = rc_parts(&o.kind).unwrap();
        let lo = parts.iter().flat_map(|p| &p.vertices).map(|v| v[0]).min().unwrap();
        let hi = parts.iter().flat_map(|p| &p.vertices).map(|v| v[0]).max().unwrap();
        (o.lateral_cm + lo, o.lateral_cm + hi)
    } else { (o.lateral_cm - i64::from(o.half_width_cm), o.lateral_cm + i64::from(o.half_width_cm)) }
}
fn cross2(a: Point, b: Point, c: Point) -> i128 {
    (b[0]-a[0]) as i128*(c[1]-a[1]) as i128 - (b[1]-a[1]) as i128*(c[0]-a[0]) as i128
}
fn convex_footprint(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_unstable();points.dedup();
    let mut hull=Vec::new();
    for &p in &points {
        while hull.len()>=2 && cross2(hull[hull.len()-2],hull[hull.len()-1],p)<=0 {hull.pop();}
        hull.push(p);
    }
    let lower=hull.len();
    for &p in points.iter().rev().skip(1) {
        while hull.len()>lower && cross2(hull[hull.len()-2],hull[hull.len()-1],p)<=0 {hull.pop();}
        hull.push(p);
    }
    hull.pop();hull
}
// A convex footprint must fit the entire (possibly concave) ribbon: checking
// just its corners misses a curved wall or a narrowing between two corners.
fn contains_footprint(boundary: &[Point], footprint: &[Point]) -> bool {
    let n=footprint.len();
    footprint.iter().all(|p| point_in_polygon(*p,boundary))
        && !boundary.iter().any(|p| (0..n).all(|i| cross2(footprint[i],footprint[(i+1)%n],*p)>0))
        && (0..n).all(|i| (0..boundary.len()).all(|j| {
            let (a,b)=(footprint[i],footprint[(i+1)%n]);
            let (c,d)=(boundary[j],boundary[(j+1)%boundary.len()]);
            cross2(a,b,c).signum()*cross2(a,b,d).signum()>=0
                || cross2(c,d,a).signum()*cross2(c,d,b).signum()>=0
        }))
}
fn wall_distance(p: Point, wall: &[Point]) -> f64 {
    wall.windows(2).map(|w| {
        let d=[(w[1][0]-w[0][0]) as f64,(w[1][1]-w[0][1]) as f64];
        let q=[(p[0]-w[0][0]) as f64,(p[1]-w[0][1]) as f64];
        let t=((d[0]*q[0]+d[1]*q[1])/(d[0]*d[0]+d[1]*d[1]).max(1.0)).clamp(0.0,1.0);
        libm::sqrt((q[0]-t*d[0]).powi(2)+(q[1]-t*d[1]).powi(2))
    }).fold(f64::INFINITY,f64::min)
}
fn quarterpipe_lateral(path: &[Sample], s: &Sample, kind: &str, side: i64) -> Option<i64> {
    let rotation=obstacle_rotation(s);
    let edges:Vec<_>=path.iter().map(|s|geometry::ribbon_edges(s,0).map(|v|[v[0],v[2]])).collect();
    let wall:Vec<_>=edges.iter().map(|e|e[usize::from(side>0)]).collect();
    // Reserve a connected 220 cm strip along the opposite wall, following the
    // true ribbon through bends/tapers instead of assuming a symmetric width.
    let available:Vec<_>=edges.iter().map(|e| {
        let mut e=*e;
        let d=[e[1][0]-e[0][0],e[1][1]-e[0][1]];
        let len=libm::sqrt((d[0]*d[0]+d[1]*d[1]) as f64).max(1.0);
        let i=usize::from(side<0);
        for j in 0..2 {e[i][j]+=round(side as f64*d[j] as f64*(PASSAGE_CM+1) as f64/len);}
        e
    }).collect();
    let boundary:Vec<_>=available.iter().map(|e|e[0]).chain(available.iter().rev().map(|e|e[1])).collect();
    let parts=rc_parts(kind)?;
    let points:Vec<_>=parts.iter().flat_map(|p|&p.vertices).map(|&v| {
        let v=geometry::rotate3(v,rotation);[v[0],v[2]]
    }).collect();
    let hull=convex_footprint(points);
    let outside=parts.iter().flat_map(|p|&p.vertices).map(|v|side*v[0]).max().unwrap()*side;
    let nominal=side*(i64::from(s.lateral_cm)-1)-outside;
    for delta in 0..=20 {
        for direction in [-1,1] {
            let lateral=nominal+direction*delta;
            let position=offset(s,lateral);
            let footprint:Vec<_>=hull.iter().map(|p|[p[0]+position[0],p[1]+position[2]]).collect();
            if !contains_footprint(&boundary,&footprint) {continue;}
            if automatic_kind(kind,side)==kind && !(-150..=150).step_by(10).all(|z| {
                let high=add(offset(s,lateral),geometry::rotate3([side*200,200,z],rotation));
                wall_distance([high[0],high[2]],&wall)<=10.0
            }) {continue;}
            return Some(lateral);
        }
    }
    None
}
pub(super) fn sample(path: &[Sample], station: u64) -> Sample {
    let mut at = 0;
    for w in path.windows(2) {
        let len = distance(w[0].position_cm, w[1].position_cm);
        if station <= at + len {
            let t = (station - at) as f64 / len.max(1) as f64;
            let mut out = w[0].clone();
            out.ribbon_cm=None;
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
// A rail is a chord across one supported corner. The 220cm corridor remains
// inside its road, so the lower rail never sends a vehicle through an outer wall.
fn chord_distance(point: Vertex, a: Vertex, b: Vertex) -> (f64,f64) {
    let delta=std::array::from_fn::<_,3,_>(|j|(b[j]-a[j]) as f64);
    let len=delta.iter().map(|v|v*v).sum::<f64>().max(1.0);
    let t=((0..3).map(|j|(point[j]-a[j]) as f64*delta[j]).sum::<f64>()/len).clamp(0.0,1.0);
    let d=std::array::from_fn::<_,3,_>(|j|point[j] as f64-a[j] as f64-delta[j]*t);
    (libm::sqrt(d[0]*d[0]+d[2]*d[2]),d[1].abs())
}
pub(super) fn rail_chord(path: &[Sample], station: u64) -> Option<(Sample,i64)> {
    let span=station.saturating_sub(150).min(length(path).saturating_sub(station+150)).min(800);
    for half in (150..=span).rev().step_by(100) {
        let a=sample(path,station-half);
        let b=sample(path,station+half);
        if dot(a.forward,b.forward)>0.866025 || dot(a.forward,b.forward)<0.0 {continue;}
        if a.normal[1]<999_000 || b.normal[1]<999_000 || a.position_cm[1].abs_diff(b.position_cm[1])>3 {continue;}
        let chord=distance(a.position_cm,b.position_cm);
        if chord*100>=half*2*99 {continue;}
        let count=chord.div_ceil(50).max(1);
        let covered=(0..=count).all(|i| {
            let point=std::array::from_fn(|j|a.position_cm[j]+(b.position_cm[j]-a.position_cm[j])*i as i64/count as i64);
            path.windows(2).any(|w| {
                if w.iter().any(|s|!s.safe || s.normal[1]<999_000) {return false;}
                let (across,height)=chord_distance(point,w[0].position_cm,w[1].position_cm);
                height<=3.0 && across+55.0<=w[0].lateral_cm.min(w[1].lateral_cm) as f64
                    && w[0].lateral_cm.min(w[1].lateral_cm) as i64 >= PASSAGE_CM + 20
            })
        });
        if !covered {continue;}
        let mut frame=a.clone();
        frame.position_cm=std::array::from_fn(|j|(a.position_cm[j]+b.position_cm[j])/2);
        frame.forward=unit(std::array::from_fn(|j|(b.position_cm[j]-a.position_cm[j]) as f64));
        frame.normal=[0,1_000_000,0];
        return Some((frame,chord as i64));
    }
    None
}
fn rail_site(a: &Assembly,site: &Site) -> Option<Obstacle> {
    let p=&a.pieces[site.piece];
    let path=if site.branch {&p.alternate_path} else {&p.path};
    let (frame,len)=rail_chord(path,site.station)?;
    let ends=[-len/2,len/2].map(|d|add(frame.position_cm,frame.forward.map(|v|v*d/1_000_000)));
    for (i,other) in a.pieces.iter().enumerate() {
        for (branch,samples) in [(false,&other.path),(true,&other.alternate_path)] {
            if i==site.piece && branch==site.branch {continue;}
            // The approach/departure shares this road's port; its endpoint
            // width is not a spherical obstacle extending into this corner.
            if [path.first(),path.last()].into_iter().flatten().any(|end|
                [samples.first(),samples.last()].into_iter().flatten().any(|other|authoring::joined(end,other))) {continue;}
            if samples.iter().any(|s| {
                let (across,height)=chord_distance(s.position_cm,ends[0],ends[1]);
                across<55.0+s.lateral_cm as f64 && height<150.0
            }) {return None;}
        }
    }
    Some(Obstacle {kind:"grind_rail".into(),piece_index:site.piece,path:if site.branch {"alternate"} else {"main"}.into(),station_cm:site.station,
        position_cm:frame.position_cm,normal:frame.normal,forward:frame.forward,lateral_cm:0,half_width_cm:20,
        avoid_lateral_cm:-(frame.lateral_cm as i64-50).min(60),jump_station_cm:None,jump_position_cm:None})
}
fn safe_site(a: &Assembly, site: &Site, kind: &str, side: i64) -> Option<Obstacle> {
    if kind=="grind_rail" {return rail_site(a,site);}
    let p = &a.pieces[site.piece];
    let path = if site.branch {
        &p.alternate_path
    } else {
        &p.path
    };
    let s = sample(path, site.station);
    if kind.starts_with("quarterpipe") && s.lateral_cm < 400 { return None; }
    let jump = kind == "jump_barrier";
    let slalom = kind == "slalom_gates";
    let rotating = ["rotating_obstacle", "swing_gates"].contains(&kind);
    let guard = if kind.starts_with("ramp") || kind.starts_with("quarterpipe") || kind=="grind_rail" {
        600
    } else if jump {
        450
    } else if slalom {
        350
    } else {
        250
    };
    if site.station < guard || site.station > length(path).saturating_sub(guard) {
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
    let quarter = kind.starts_with("quarterpipe");
    let sweep = match kind {
        "ramp_low" | "ramp_standard" | "ramp_triple" => 160,
        "quarterpipe_left" | "quarterpipe_right" => 208,
        "grind_rail" => 20,
        "fixed_obstacle" => 20,
        "moving_obstacle" => 40, // 40cm box plus 40cm translation, centred on sweep
        "rotating_obstacle" => 37, // sqrt(20²+30²), full rotation
        "jump_barrier" => (half * 2 - PASSAGE_CM - 10) / 2,
        "slalom_gates" => 70.min((half * 2 - PASSAGE_CM - 20) / 4),
        _ => 70.min((half * 2 - PASSAGE_CM - 10) / 2),
    };
    if sweep < 20 || (!quarter && sweep * 2 + PASSAGE_CM + 10 > half * 2) {
        return None;
    }
    let lateral = if quarter { quarterpipe_lateral(path, &s, kind, side)? }
        else { side * (half - sweep - 10) };
    let avoid = if slalom { 0 } else { -side * (half - PASSAGE_CM / 2) };
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
    let reach = if kind=="ramp_triple" { 330 } else if kind=="grind_rail" { 300 } else if kind.starts_with("quarterpipe") { 150 } else if kind.starts_with("ramp") { 110 } else if slalom {
        140
    } else if rotating {
        sweep
    } else {
        30
    };
    if !quarter {
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
            let clear = if slalom {
                // The same centre lane stays open past both alternating gates.
                across.abs() >= PASSAGE_CM as f64 / 2.0
            } else {
                t.lateral_cm as f64 + side as f64 * across >= PASSAGE_CM as f64
            };
            if !clear || across.abs() + 5.0 > t.lateral_cm as f64 {
                return None;
            }
        }
    }
    }
    let g = gimmick(a, &o, 0);
    let occupied = g.occupancy_bounds();
    // Protect other roads/branches (especially the 2m overpass clearance) and
    // existing action/landing volumes. Safety envelopes enclose the full cycle.
    for (i, other) in a.pieces.iter().enumerate() {
        for (branch, samples) in [(false, &other.path), (true, &other.alternate_path)] {
            if i == site.piece && branch == site.branch {
                continue;
            }
            if samples.iter().any(|t| {
                occupied.iter().any(|(lo,hi)| t.position_cm[1] + 15 >= lo[1]
                    && t.position_cm[1] - 15 <= hi[1]
                    && [0, 2].iter().all(|&j| {
                        t.position_cm[j] + t.lateral_cm as i64 >= lo[j]
                            && t.position_cm[j] - t.lateral_cm as i64 <= hi[j]
                    }))
            }) {
                return None;
            }
        }
    }
    Some(o)
}
pub(super) fn authored(a: &Assembly, index: usize, o: &authoring::Attachment) -> Option<Obstacle> {
    if (o.path == "alternate" && a.pieces[index].alternate_path.len() < 2)
        || a.pieces[index].path.len() < 2
    {
        return None;
    }
    safe_site(
        a,
        &Site {
            piece: index,
            branch: o.path == "alternate",
            station: o.station_cm,
        },
        &o.kind,
        o.side,
    )
}
pub(super) fn place(a: &Assembly) -> Result<(Vec<Obstacle>, u64, u32)> {
    place_reserved(a,&vec![])
}
pub(super) fn place_reserved(a: &Assembly, reserved: &composition::Required) -> Result<(Vec<Obstacle>, u64, u32)> {
    if !a.settings.categories.iter().any(|id| id == "gimmick") {
        return Ok((vec![], 0, 0));
    }
    let mut sites = vec![];
    let mut eligible = 0;
    for (i, p) in a.pieces.iter().enumerate() {
        cancellation::checkpoint()?;
        if !eligible_piece(i, p) || reserved.iter().any(|r|r.contains(&i)) {
            continue;
        }
        for (branch, path) in [(false, &p.path), (true, &p.alternate_path)] {
            if path.len() < 2 {
                continue;
            }
            // Count usable road distance, not the number of candidate centres.
            // Approach/footprint margins constrain placement without silently
            // halving planned density on short, joined road pieces.
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
    let plan = plan::Plan::new(a.settings.seed);
    let spacing = plan.obstacle_spacing;
    let target = ((eligible + spacing / 2) / spacing).max(1) as u32;
    let road_objects = road_gimmicks(a)?;
    let available = crate::gimmick::MAX_GIMMICKS.saturating_sub(road_objects.len());
    let count = (target as usize).min(available).min(sites.len());
    let mut rng = a.settings.seed ^ 0xd1b54a32d192ed03;
    let mut kinds:Vec<_>=KINDS.iter().copied().filter(|k|*k!="quarterpipe_right" && *k!="grind_rail").collect();
    for i in (1..kinds.len()).rev() {let j=next(&mut rng) as usize%(i+1);kinds.swap(i,j);}
    let mut out: Vec<Obstacle> = vec![];
    for bin in 0..count {
        cancellation::checkpoint()?;
        let begin = bin * sites.len() / count;
        let end = (bin + 1) * sites.len() / count;
        let start = next(&mut rng) as usize % (end - begin);
        let side = if next(&mut rng) % 2 == 0 { 1 } else { -1 };
        let seen:std::collections::BTreeSet<_>=out.iter().filter_map(|o|diversity::obstacle_kind(&o.kind)).collect();
        let mut ordered=kinds.clone();
        ordered.sort_by_key(|k|seen.contains(&diversity::obstacle_kind(k).unwrap()));
        // Search an unused type's suitable sites before falling back to a used
        // type. The attachment count and continuous bypass budget stay fixed.
        'search: for kind in ordered {
            for offset in 0..end - begin {
                cancellation::checkpoint()?;
                let site = &sites[begin + (start + offset) % (end - begin)];
                let elapsed:u32=a.pieces.iter().take(site.piece).map(|p|p.reference_msec).sum();
                let role=&plan.section(elapsed,u32::from(a.settings.duration_seconds)*1000).role;
                if matches!(role,plan::Role::Recovery) {continue;}
                let Some(o) = safe_site(a, site, automatic_kind(kind, side), side)
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

    Ok((out, eligible, target))
}
pub(super) fn gimmick(a: &Assembly, o: &Obstacle, index: usize) -> Gimmick {
    let mut s = sample(path(a, o), o.station_cm);
    if o.kind=="grind_rail" {s.position_cm=o.position_cm;s.normal=o.normal;s.forward=o.forward;}
    let half = o.half_width_cm as i64;
    let mut g = Gimmick {
        id: format!("track-obstacle-{index}"),
        position: offset(&s, o.lateral_cm),
        rotation_mdeg: obstacle_rotation(&s),
        scale_per_mille: [1000; 3],
        parts: vec![],
        curved_faces: vec![],
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
    if let Some(parts) = rc_parts(&o.kind) { g.parts = parts; g.color = [90, 180, 200, 255]; }
    if o.kind=="grind_rail" {
        let (_,len)=rail_chord(path(a,o),o.station_cm).expect("validated corner rail");
        g.parts=rail_parts(len);
    }
    if o.kind.starts_with("quarterpipe") {
        g.curved_faces = (0..g.parts.len()).flat_map(|part| [[part as u16,0],[part as u16,1]]).collect();
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
    #[test]
    fn suitable_wide_road_fixture_reaches_the_gimmick_type_goal_without_more_objects() {
        let seed=(0..256).find(|&seed|plan::Plan::new(seed).obstacle_spacing<=2400).unwrap();
        let mut pieces=vec![];let mut at=[0;3];
        for _ in 0..40 {push_piece(&mut pieces,&mut at,0,"sprint_lane",800,true,[0;3]);}
        let mut a=layout::finish(pieces,&Settings{seed,circuit:false,..Settings::default()});
        let (placed,eligible,target)=place(&a).unwrap();
        assert!(placed.len()<=target as usize);
        assert_eq!(target,((eligible+plan::Plan::new(seed).obstacle_spacing/2)/plan::Plan::new(seed).obstacle_spacing).max(1) as u32);
        a.obstacles=placed;
        let summary=generation_summary(&a);
        assert!(summary.gimmicks.used>=summary.gimmicks.target,"{summary:?}");
    }
    fn fixture(id: &str, width: u32) -> Assembly {
        let mut pieces = vec![];
        let mut origin = [0; 3];
        for _ in 0..3 {
            push_piece(&mut pieces, &mut origin, 0, "straight", width, false, [0; 3]);
        }
        push_piece(&mut pieces, &mut origin, 0, id, width, true, [0; 3]);
        layout::finish(
            pieces,
            &Settings {
                circuit: false,
                categories: vec!["gimmick".into()],
                ..Settings::default()
            },
        )
    }
    #[test]
    fn automatic_obstacles_and_straight_fences_are_not_grind_routes() {
        for preset in ["straight", "straight_extra_wide", "gentle90", "right90"] {
            let mut a=fixture(preset,1200);
            a.pieces.resize(8,a.pieces[3].clone());
            for seed in 0..16 {
                a.settings.seed=seed;
                a.obstacles=place(&a).unwrap().0;
                assert!(a.obstacles.iter().all(|o|o.kind!="grind_rail"));
                assert!(grind_lines(&a).is_empty());
            }
        }
    }
    #[test]
    fn quarterpipes_face_outward_touch_the_wall_and_roundtrip_source() {
        for width in [800,1200] {
            for side in [-1,1] {
                let mut a=fixture("sprint_lane",width);
                let kind=automatic_kind("quarterpipe_left",side);
                assert_eq!(kind,if side<0 {"quarterpipe_left"} else {"quarterpipe_right"});
                assert_eq!(automatic_kind("quarterpipe_right",side),kind);
                let o=safe_site(&a,&Site{piece:3,branch:false,station:800},kind,side).unwrap_or_else(||panic!("quarterpipe {width}/{side}"));
                let g=gimmick(&a,&o,0);
                for part in &g.parts {
                    for v in &part.vertices {
                        let x=o.lateral_cm+v[0];
                        assert!(x.abs()<=i64::from(width/2));
                        assert!(i64::from(width/2)+side*x>=PASSAGE_CM);
                        if v[1]==200 {
                            let gap=i64::from(width/2)-side*x;
                            assert!((0..=10).contains(&gap),"{width}/{side}: {gap}");
                        }
                    }
                }
                a.obstacles=vec![o.clone()];
                let mut source=authoring::from_assembly(&a);
                source.grounded_supports=false;
                let compiled=authoring::compile(&source).unwrap();
                assert_eq!(compiled.obstacles,vec![o]);
                assert_eq!(gimmick(&compiled,&compiled.obstacles[0],0),g);
                // Explicit manual handedness is independent of the selected side.
                source.attachments[0].kind=automatic_kind(kind,-side).into();
                let manual=authoring::compile(&source).unwrap();
                assert_eq!(manual.obstacles[0].kind,source.attachments[0].kind);
            }
        }
    }
    #[test]
    fn quarterpipes_reject_curved_walls_tapers_and_pinched_bypass() {
        let site=Site{piece:3,branch:false,station:800};
        assert!(safe_site(&fixture("sprint_lane",600),&site,"quarterpipe_right",1).is_none());
        let mut accepted=0;
        let mut rejected=0;
        for preset in ["gentle90","gentle90_left","right90","right90_left"] {
            let a=fixture(preset,800);
            let site=Site{station:length(&a.pieces[3].path)/2,..site.clone()};
            for side in [-1,1] {
                if let Some(o)=safe_site(&a,&site,automatic_kind("quarterpipe_left",side),side) {
                    accepted+=1;
                    let g=gimmick(&a,&o,0);
                    let wall:Vec<_>=a.pieces[3].path.iter().map(|s|geometry::ribbon_edges(s,0)[usize::from(side>0)])
                        .map(|v|[v[0],v[2]]).collect();
                    for v in g.parts.iter().flat_map(|p|&p.vertices).filter(|v|v[1]==200) {
                        let world=add(g.position,geometry::rotate3(*v,g.rotation_mdeg));
                        assert!(wall_distance([world[0],world[2]],&wall)<=10.0);
                    }
                } else {rejected+=1;}
            }
        }
        assert!(accepted>0 && rejected>0,"{accepted}/{rejected}");
        let mut a=fixture("sprint_lane",800);
        // An abrupt narrowing inside the footprint must be caught even when
        // the station and both end corners are wide enough.
        let origin=a.pieces[3].origin_cm;
        let mut narrow=sample(&a.pieces[3].path,850);
        narrow.position_cm=add(origin,[0,0,850]);narrow.lateral_cm=250;narrow.ribbon_cm=None;
        a.pieces[3].path.push(narrow);
        a.pieces[3].path.sort_by_key(|s|s.position_cm[2]);
        assert!(safe_site(&a,&site,"quarterpipe_right",1).is_none());
        let mut tapered=fixture("sprint_lane",800);
        let mut p=variant("sprint_lane",800,800,1200);
        p.origin_cm=tapered.pieces[3].origin_cm;
        tapered.pieces[3]=materialize(&p);
        assert!(safe_site(&tapered,&site,"quarterpipe_right",1).is_none());
    }
    #[test]
    fn automatic_quarterpipes_keep_handedness_after_yaw_and_source_conversion() {
        let mut found=std::collections::BTreeSet::new();
        for seed in 0..64 {
            let mut a=fixture("sprint_lane",800);
            a.settings.seed=seed;
            a.obstacles=place(&a).unwrap().0;
            for o in &a.obstacles {
                if !o.kind.starts_with("quarterpipe") {continue;}
                let side=if o.lateral_cm<0 {-1} else {1};
                assert_eq!(o.kind,automatic_kind(&o.kind,side));
                found.insert(side);
                let mut source=authoring::from_assembly(&a);
                source.grounded_supports=false;
                assert_eq!(authoring::compile(&source).unwrap().obstacles,a.obstacles);
            }
        }
        assert_eq!(found,[-1,1].into());
        for yaw in [17000,45000,90000,-123000] {
            let mut a=fixture("sprint_lane",800);
            for p in &mut a.pieces {
                p.origin_cm=geometry::rotate3(p.origin_cm,[0,yaw,0]);
                p.rotation_mdeg=[0,yaw,0];
                *p=materialize(p);
            }
            for side in [-1,1] {
                let kind=automatic_kind("quarterpipe_left",side);
                let o=safe_site(&a,&Site{piece:3,branch:false,station:800},kind,side).unwrap_or_else(||panic!("yaw {yaw}/{side}: lateral {:?}",quarterpipe_lateral(&a.pieces[3].path,&sample(&a.pieces[3].path,800),kind,side)));
                let g=gimmick(&a,&o,0);
                let wall:Vec<_>=a.pieces[3].path.iter().map(|s|geometry::ribbon_edges(s,0)[usize::from(side>0)])
                    .map(|v|[v[0],v[2]]).collect();
                for v in g.parts.iter().flat_map(|p|&p.vertices).filter(|v|v[1]==200) {
                    let w=add(g.position,geometry::rotate3(*v,g.rotation_mdeg));
                    assert!(wall_distance([w[0],w[2]],&wall)<=10.0,"{yaw}/{side}");
                }
            }
        }
    }
    #[test]
    fn resolved_attachments_fit_three_widths_and_use_real_motion_geometry() {
        for width in [400, 600, 800] {
            let a = fixture("sprint_lane", width);
            for kind in KINDS {
                if *kind=="grind_rail" {continue;}
                if (kind.starts_with("quarterpipe") && width<800) || (kind.starts_with("ramp") && width<600) {continue;}
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
                    let (lo,hi)=lateral_bounds(&o);
                    assert!(lo>=-(width as i64/2) && hi<=width as i64/2);
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
    fn wider_ramps_and_low_corner_rail_share_geometry() {
        for kind in ["ramp_low","ramp_standard","ramp_triple"] {
            for part in rc_parts(kind).unwrap() {
                assert_eq!(part.vertices.iter().map(|v|v[0]).max().unwrap()-part.vertices.iter().map(|v|v[0]).min().unwrap(),320);
            }
        }
        assert!(rail_chord(&fixture("sprint_lane",400).pieces[3].path,800).is_none());
        for id in ["right90","right90_left","gentle90","gentle90_left"] {
            let mut a=fixture(id,600);
            let station=length(&a.pieces[3].path)/2;
            let o=rail_site(&a,&Site{piece:3,branch:false,station}).unwrap_or_else(||panic!("corner rail {id}"));
            a.obstacles=vec![o.clone()];
            let g=gimmick(&a,&o,0);
            assert!(g.valid());
            assert_eq!(g.parts[0].vertices.iter().map(|v|v[1]).max(),Some(40));
            let line=rail_lines(&a).remove(0);
            let (_,len)=rail_chord(&a.pieces[3].path,station).unwrap();
            assert_eq!(line.control_points,[-len/2,len/2].map(|d|add(g.position,geometry::rotate3([0,41,d],g.rotation_mdeg))).to_vec());
            assert!((len as u64)<2*station);
            assert!(line.control_points.iter().all(|v|v[1]==41));
        }
        let mut a=fixture("sprint_lane",400);
        a.obstacles=vec![Obstacle {station_cm:800,..rail_site(&fixture("gentle90",600),&Site{piece:3,branch:false,station:800}).unwrap()}];
        assert!(a.validate_stored().unwrap_err().to_string().contains("grind rail"));
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
            assert!(place(&a).unwrap().0.is_empty(), "{id}");
        }
        let mut a = fixture("sprint_lane", 400);
        a.pieces.truncate(3);
        assert!(place(&a).unwrap().0.is_empty());
    }
    #[test]
    fn seed_density_budget_and_inert_difficulty() {
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
        let spacing=plan::Plan::new(a.settings.seed).obstacle_spacing;
        for difficulty in ["easy", "normal", "hard"] {
            a.settings.difficulty = difficulty.into();
            let result = place(&a).unwrap();
            assert_eq!(result, place(&a).unwrap());
            assert_eq!(result.2, ((result.1 + spacing / 2) / spacing).max(1) as u32);
            assert_eq!(result.0.len(), result.2 as usize);
            counts.push(result.0.len());
        }
        assert_eq!(counts[0],counts[1]);assert_eq!(counts[1],counts[2]);
        // Existing object cap reduces attachments instead of adding roads.
        let mut budget = a.clone();
        for _ in 0..(crate::gimmick::MAX_GIMMICKS / 3) {
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
        assert!(o.len() + road_gimmicks(&budget).unwrap().len() <= crate::gimmick::MAX_GIMMICKS);
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
        assert!(place(&budget).unwrap().0.is_empty());
    }
}

fn wedge(x: i64, z: i64, width: i64, length: i64, height: i64) -> CollisionConvex {
    CollisionConvex { vertices: vec![[x-width/2,0,z-length/2],[x+width/2,0,z-length/2],[x-width/2,0,z+length/2],[x+width/2,0,z+length/2],[x-width/2,height,z+length/2],[x+width/2,height,z+length/2]],
        faces: vec![[0,1,3],[0,3,2],[0,4,1],[1,4,5],[0,2,4],[1,5,3],[2,3,5],[2,5,4]] }
}
fn rail_parts(length: i64) -> Vec<CollisionConvex> {
    vec![box_part([0,37,0],[12,6,length]),box_part([0,17,-length/3],[10,34,12]),box_part([0,17,length/3],[10,34,12])]
}
fn rc_parts(kind: &str) -> Option<Vec<CollisionConvex>> {
    Some(match kind {
        "ramp_low" => vec![wedge(0,0,320,160,25)],
        "ramp_standard" => vec![wedge(0,0,320,220,60)],
        "ramp_triple" => [-250,0,250].map(|z|wedge(0,z,320,160,25)).to_vec(),
        "grind_rail" => rail_parts(600),
        "quarterpipe_left" | "quarterpipe_right" => {
            let sign=if kind.ends_with("left") {-1.0} else {1.0};
            (0..32).map(|i| {
                let p=|n:f64| {let t=n/32.0*std::f64::consts::FRAC_PI_2; [round(sign*200.0*libm::sin(t)),round(200.0*(1.0-libm::cos(t)))]};
                let a=p(i as f64);let b=p((i+1) as f64);
                let outer=|n:f64| {let t=n/32.0*std::f64::consts::FRAC_PI_2; [round(sign*208.0*libm::sin(t)),round(200.0-208.0*libm::cos(t))]};
                let c=outer(i as f64); let d=outer((i+1) as f64);
                let vertices=vec![[a[0],a[1],-150],[a[0],a[1],150],[b[0],b[1],-150],[b[0],b[1],150],[c[0],c[1],-150],[c[0],c[1],150],[d[0],d[1],-150],[d[0],d[1],150]];
                {
                    let mut shape=CollisionConvex {vertices,faces:vec![[0,2,3],[0,3,1],[4,5,7],[4,7,6],[0,1,5],[0,5,4],[2,6,7],[2,7,3],[0,4,6],[0,6,2],[1,3,7],[1,7,5]]};
                    let center=std::array::from_fn::<_,3,_>(|j|shape.vertices.iter().map(|v|v[j] as f64).sum::<f64>()/8.0);
                    for face in &mut shape.faces {
                        let [a,b,c]=face.map(|n|shape.vertices[n as usize]);
                        let u=std::array::from_fn::<_,3,_>(|j|b[j]-a[j]);let v=std::array::from_fn::<_,3,_>(|j|c[j]-a[j]);
                        let n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
                        if (0..3).map(|j|n[j] as f64*(center[j]-a[j] as f64)).sum::<f64>()>0.0 {face.swap(1,2);}
                    }
                    shape
                }
            }).collect()
        },
        _=>return None,
    })
}

pub(super) fn grind_lines(a: &Assembly) -> Vec<crate::grind::GrindLine> {
    let mut out=a.authoring.as_ref().or(a.seed_source.as_ref()).map_or_else(Vec::new,|s|s.grind_lines.clone());
    if a.authoring.is_some() { return out; }
    for line in rail_lines(a) {
        if !out.iter().any(|existing|existing.id==line.id) { out.push(line); }
    }
    out
}

pub(super) fn rail_lines(a: &Assembly) -> Vec<crate::grind::GrindLine> {
    let mut out=vec![];
    for (i,o) in a.obstacles.iter().enumerate().filter(|(_,o)|o.kind=="grind_rail") {
        let g=gimmick(a,o,i);
        let (_,len)=rail_chord(path(a,o),o.station_cm).expect("validated corner rail");
        // Resolve both from the same quantized transform as the physical beam.
        let points=[-len/2,len/2].map(|d|add(g.position,geometry::rotate3([0,41,d],g.rotation_mdeg)));
        out.push(crate::grind::GrindLine {id:format!("rail-{i}"),control_points:points.to_vec(),up:o.normal,capture_width_cm:30,start_connections:vec![],end_connections:vec![]});
    }
    out
}
