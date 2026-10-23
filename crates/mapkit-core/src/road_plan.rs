//! Shared, cell-independent junction geometry. Authoring IDs and coordinates are immutable.
use crate::roads::hull;
use crate::*;
const MAX_LOCAL_PATCHES: usize = 24_576;
#[derive(Clone)]
pub(crate) struct Patch<'a> {
    pub v: [Vertex; 3],
    pub road: &'a Road,
    pub surface: Surface,
    pub terrain_join: bool,
}
#[derive(Clone)]
pub(crate) struct Edge<'a> {
    pub a: Vertex,
    pub b: Vertex,
    pub station_cm: f64,
    pub road: &'a Road,
}
#[derive(Clone)]
struct Arm<'a> {
    pub road: &'a Road,
    segment: usize,
    end: usize,
    point_index: usize,
    point: Vertex,
    other: Vertex,
}
type Key<'a> = (bool, &'a str, usize);
/// [`Key`] with each ID replaced by its dense rank among the sorted IDs of its
/// kind (node or road): identical order and equality, integer comparisons.
type RankKey = (bool, u32, usize);
/// A junction mouth moves at most half the widest road along its arm, then
/// half a width sideways. Two rounded coordinates add at most two centimetres.
pub(crate) fn influence_margin(d: &MapDocument) -> i64 {
    width_influence_margin(
        d.roads
            .iter()
            .flat_map(|r| &r.widths_cm)
            .copied()
            .max()
            .unwrap_or(0),
    )
}
pub(crate) fn width_influence_margin(maximum_width: u32) -> i64 {
    i64::from(maximum_width) + 224
}
fn key(r: &Road, i: usize) -> Key<'_> {
    if i == 0 {
        (false, &r.from, 0)
    } else if i + 1 == r.points.len() {
        (false, &r.to, 0)
    } else {
        (true, &r.id, i)
    }
}
pub(crate) fn xy(p: Vertex) -> Point {
    [p[0], p[2]]
}
pub(crate) fn orient(a: Vertex, b: Vertex, c: Vertex) -> i128 {
    cross(xy(a), xy(b), xy(c))
}
pub(crate) fn hit(v: &[Vertex], b: &Bounds, margin: i64) -> bool {
    (0..2).all(|a| {
        v.iter().map(|p| p[a * 2]).min().unwrap() - margin <= b.max[a]
            && v.iter().map(|p| p[a * 2]).max().unwrap() + margin >= b.min[a]
    })
}

fn node_widths(d: &MapDocument) -> BTreeMap<&str, (usize, u32)> {
    let mut nodes = BTreeMap::<&str, (usize, u32)>::new();
    for road in &d.roads {
        for id in [&road.from, &road.to] {
            let entry = nodes.entry(id).or_default();
            entry.0 += 1;
            entry.1 = entry.1.max(*road.widths_cm.iter().max().unwrap());
        }
    }
    nodes
}

/// One transient index per immutable validation, not one whole-city plan per
/// building/road pair. Complete endpoint stars and global width influence are
/// retained, including approaches outside the queried bounds.
pub(crate) struct PlanIndex<'a> {
    document: &'a MapDocument,
    bounds: crate::bounds_index::BoundsIndex,
    /// Node ID to its index in `incident`.
    node_index: BTreeMap<&'a str, usize>,
    /// Roads ending at each node, by node index.
    incident: Vec<Vec<usize>>,
    /// Node indices of every road's `from` and `to`, by document index.
    ends: Vec<[usize; 2]>,
    margin: i64,
    /// [`chain`] of every road, by document index.
    chains: Vec<Vec<usize>>,
    /// Dense ranks of node IDs (by node index) and road IDs (by document index).
    node_rank: Vec<u32>,
    road_rank: Vec<u32>,
    /// Junction plans shared by every query of this immutable source.
    junctions: std::cell::RefCell<JunctionCache<'a, RankKey>>,
}

impl<'a> PlanIndex<'a> {
    pub fn new(document: &'a MapDocument) -> Result<Self> {
        let margin = influence_margin(document);
        let mut node_index = BTreeMap::<&str, usize>::new();
        let mut incident = Vec::<Vec<usize>>::new();
        let mut ends = Vec::with_capacity(document.roads.len());
        let mut areas = Vec::with_capacity(document.roads.len());
        for (i, road) in document.roads.iter().enumerate() {
            crate::cancellation::checkpoint()?;
            ends.push([&road.from, &road.to].map(|id| {
                let next = node_index.len();
                let node = *node_index.entry(id).or_insert(next);
                if node == incident.len() { incident.push(Vec::new()); }
                incident[node].push(i);
                node
            }));
            areas.push(Bounds {
                min: std::array::from_fn(|a| road.points.iter().map(|p| p[a*2]).min().unwrap()-margin),
                max: std::array::from_fn(|a| road.points.iter().map(|p| p[a*2]).max().unwrap()+margin),
            });
        }
        let nodes = node_widths(document);
        let chains = document.roads.iter().map(|r| chain(r, &nodes)).collect();
        let mut node_rank = vec![0; node_index.len()];
        for (rank, &node) in node_index.values().enumerate() { node_rank[node] = rank as u32; }
        let mut order: Vec<usize> = (0..document.roads.len()).collect();
        order.sort_by_key(|&i| &document.roads[i].id);
        let mut road_rank = vec![0; order.len()];
        let mut rank = 0;
        for (k, &i) in order.iter().enumerate() {
            if k > 0 && document.roads[i].id != document.roads[order[k - 1]].id { rank += 1; }
            road_rank[i] = rank;
        }
        Ok(Self { document, bounds: crate::bounds_index::BoundsIndex::new(&areas),
            node_index, incident, ends, margin, chains, node_rank, road_rank, junctions: Default::default() })
    }

    /// Edges are those `walls` keeps; one index always passes the same rule.
    pub fn plan(&self, bounds: &Bounds, walls: Walls, work: &mut usize) -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
        let mut selected = BTreeSet::new();
        for index in self.bounds.query(bounds, work)? {
            let road = &self.document.roads[index];
            if !hit(&road.points, bounds, self.margin) { continue; }
            selected.insert(index);
            for node in self.ends[index] {
                selected.extend(self.incident[node].iter().copied());
            }
        }
        // Cached junctions hold the edges of the first rule used on this index.
        let rule = *self.junctions.borrow_mut().walls.get_or_insert(walls);
        self.plan_indices(selected, bounds, walls, (rule == walls).then_some(&self.junctions))
    }

    fn plan_indices(&self, selected: BTreeSet<usize>, bounds: &Bounds, walls: Walls,
        cache: Option<&std::cell::RefCell<JunctionCache<'a, RankKey>>>) -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
        let selected: Vec<_> = selected.into_iter().collect();
        let roads: Vec<_> = selected.iter().map(|&i| &self.document.roads[i]).collect();
        let chains: Vec<_> = selected.iter().map(|&i| self.chains[i].as_slice()).collect();
        let key = |i: usize, point: usize| -> RankKey {
            let (road, r) = (selected[i], roads[i]);
            if point == 0 {
                (false, self.node_rank[self.ends[road][0]], 0)
            } else if point + 1 == r.points.len() {
                (false, self.node_rank[self.ends[road][1]], 0)
            } else {
                (true, self.road_rank[road], point)
            }
        };
        plan_selected(&roads, &chains, &key, bounds, self.margin, true, walls, cache, false)
    }

    /// The plan of one graph node's junction from its incident roads alone.
    fn junction(&self, node: &str, bounds: &Bounds, work: &mut usize) -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
        let roads: BTreeSet<_> = self.node_index.get(node).into_iter()
            .flat_map(|&n| self.incident[n].iter().copied()).collect();
        crate::roads::tick(work, roads.len())?;
        // Only this node's star is complete here; never share these plans.
        self.plan_indices(roads, bounds, Walls::All, None)
    }
}

/// Graph nodes joining two or more road ends whose junction the current rules
/// cannot build (for example a source way that doubles back on itself, or
/// approaches too close to form one passage). Each node is planned with only its
/// own incident roads, so a failing neighbour never implicates it. Ordered by
/// node position in the document.
pub fn unbuildable_junctions(d: &MapDocument) -> Result<Vec<String>> {
    let index = PlanIndex::new(d)?;
    let mut ends = BTreeMap::<&str, usize>::new();
    for road in &d.roads {
        for id in [&road.from, &road.to] { *ends.entry(id.as_str()).or_default() += 1; }
    }
    let mut failing = Vec::new();
    let mut work = 0;
    for node in &d.nodes {
        crate::cancellation::checkpoint()?;
        if ends.get(node.id.as_str()).copied().unwrap_or(0) < 2 { continue; }
        let p = node.position;
        let area = Bounds { min: [p[0] - 1, p[2] - 1], max: [p[0] + 1, p[2] + 1] };
        match index.junction(&node.id, &area, &mut work) {
            Ok(_) => {}
            Err(e) if e.code == "E_GEOMETRY" => failing.push(node.id.clone()),
            Err(e) => return Err(e),
        }
        // Each node's plan is local; the shared work counter only reports cost.
        work = 0;
    }
    Ok(failing)
}

#[cfg(test)]
mod index_tests {
    use super::*;

    fn fixture(designed: bool) -> MapDocument {
        let mut d: MapDocument = serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        let template = d.roads[0].clone();
        d.roads.clear();
        for (i, (from, to, points, width)) in [
            ("a", "b", vec![[0, 0, 0], [1000, 0, 120], [5000, 0, 0]], 600),
            ("b", "c", vec![[5000, 0, 0], [7000, 0, 2500]], 900),
            ("b", "e", vec![[5000, 0, 0], [7000, 0, -2500]], 400),
            // The widest road is far outside every local query. Its global
            // influence still affects the existing junction planning rules.
            ("far-a", "far-b", vec![[100000, 0, 100000], [104000, 0, 100000]], 2400),
        ].into_iter().enumerate() {
            let mut r = template.clone();
            r.id = format!("r{i}"); r.from = from.into(); r.to = to.into();
            r.kind = RoadKind::Ground; r.design = None; r.sidewalk_cm = Some(0);
            r.points = points;
            r.widths_cm = vec![width; r.points.len() - 1];
            r.surfaces = vec![Surface::Asphalt; r.points.len() - 1];
            if designed {
                let design = crate::road_design::from_points(&r.points).unwrap();
                crate::road_design::compile(&mut r, design, width).unwrap();
            }
            d.roads.push(r);
        }
        d
    }

    fn assert_same(d: &MapDocument, area: Bounds) {
        let index = PlanIndex::new(d).unwrap();
        let (full_patches, full_edges) = plan(d, &area).unwrap();
        let (local_patches, local_edges) = index.plan(&area, Walls::All, &mut 0).unwrap();
        let patches = |v: Vec<Patch>| v.into_iter().map(|p|
            (p.v, p.road.id.clone(), p.surface, p.terrain_join)).collect::<Vec<_>>();
        let edges = |v: Vec<Edge>| v.into_iter().map(|e|
            (e.a, e.b, e.station_cm.to_bits(), e.road.id.clone())).collect::<Vec<_>>();
        assert_eq!(patches(full_patches), patches(local_patches));
        assert_eq!(edges(full_edges), edges(local_edges));
    }

    #[test]
    fn indexed_plan_keeps_complete_junctions_global_widths_and_design_approaches() {
        for designed in [false, true] {
            let mut d = fixture(designed);
            for reverse in [false, true] {
                if reverse { d.roads.reverse(); }
                for x in (-3000..9000).step_by(500) {
                    assert_same(&d, Bounds { min: [x, -120], max: [x + 90, 180] });
                }
            }
        }
    }

    /// One index answers many queries from shared junction plans (evicted
    /// often under the test cap): every answer equals the cold whole-map plan,
    /// and the deck rule keeps exactly the bridge/elevated edges.
    #[test]
    fn shared_junction_plans_match_cold_plans_and_deck_edges() {
        let faces = |v: &[Patch]| v.iter().map(|p| (p.v, p.road.id.clone(), p.surface, p.terrain_join)).collect::<Vec<_>>();
        let edges = |v: &[Edge]| v.iter().map(|e| (e.a, e.b, e.station_cm.to_bits(), e.road.id.clone())).collect::<Vec<_>>();
        for designed in [false, true] {
            let mut d = fixture(designed);
            if !designed {
                d.roads[1].kind = RoadKind::Bridge;
                d.roads[2].kind = RoadKind::Elevated;
                for road in &mut d.roads[1..3] {
                    road.from = format!("deck-{}", road.from);
                    road.to = format!("deck-{}", road.to);
                    for p in &mut road.points { p[1] = 600; }
                }
            }
            let (all, decks) = (PlanIndex::new(&d).unwrap(), PlanIndex::new(&d).unwrap());
            for pass in 0..2 {
                for x in (-3000..9000).step_by(250) {
                    for y in [-2600, -700, -120, 300, 2400] {
                        let area = Bounds { min: [x, y], max: [x + 400, y + 300] };
                        let (p0, e0) = plan(&d, &area).unwrap();
                        let (p1, e1) = all.plan(&area, Walls::All, &mut 0).unwrap();
                        let (p2, e2) = decks.plan(&area, Walls::Decks, &mut 0).unwrap();
                        assert_eq!(faces(&p0), faces(&p1));
                        assert_eq!(faces(&p0), faces(&p2));
                        assert_eq!(edges(&e0), edges(&e1));
                        let deck: Vec<_> = e0.into_iter().filter(|e| matches!(e.road.kind, RoadKind::Bridge | RoadKind::Elevated)).collect();
                        assert_eq!(edges(&deck), edges(&e2));
                        if pass == 1 {
                            // Another rule on the same index bypasses its cache.
                            let (p3, e3) = decks.plan(&area, Walls::All, &mut 0).unwrap();
                            assert_eq!((faces(&p0), edges(&e1)), (faces(&p3), edges(&e3)));
                        }
                    }
                }
            }
            assert!(!all.junctions.borrow().plans.is_empty());
        }
    }

    /// Validated plans skip ends outside the hit segments' bounds without the key
    /// lookup: the same faces and edges, for both edge rules.
    #[test]
    fn validated_plans_match_full_key_lookups() {
        let faces = |v: &[Patch]| v.iter().map(|p| (p.v, p.road.id.clone(), p.surface, p.terrain_join)).collect::<Vec<_>>();
        let edges = |v: &[Edge]| v.iter().map(|e| (e.a, e.b, e.station_cm.to_bits(), e.road.id.clone())).collect::<Vec<_>>();
        for designed in [false, true] {
            let mut d = fixture(designed);
            // Long multi-point roads whose far points leave every query.
            let mut r = d.roads[0].clone();
            r.id = "long".into(); r.from = "c".into(); r.to = "far-a".into();
            r.points = vec![[7000, 0, 2500], [9000, 0, 30000], [60000, 0, 70000], [100000, 0, 100000]];
            r.widths_cm = vec![500; 3]; r.surfaces = vec![Surface::Asphalt; 3]; r.design = None;
            d.roads.push(r);
            let mut nodes = BTreeMap::new();
            for r in &d.roads {
                nodes.insert(r.from.clone(), r.points[0]);
                nodes.insert(r.to.clone(), *r.points.last().unwrap());
            }
            d.nodes = nodes.into_iter().map(|(id, position)| RoadNode { id, position, level: 0 }).collect();
            let cell = d.cell_size_cm as i64;
            d.bounds = Bounds { min: [-cell * (12_000 / cell + 1); 2], max: [cell * (110_000 / cell + 1); 2] };
            d.validate().unwrap();
            for reverse in [false, true] {
                if reverse { d.roads.reverse(); }
                for x in (-3000..105000).step_by(1700) {
                    for y in [-2600, -120, 2400, 30000, 99000] {
                        let area = Bounds { min: [x, y], max: [x + 1500, y + 900] };
                        for ground_edges in [false, true] {
                            let (p0, e0) = plan_all(&d, &area, ground_edges, false).unwrap();
                            let (p1, e1) = plan_all(&d, &area, ground_edges, true).unwrap();
                            assert_eq!((faces(&p0), edges(&e0)), (faces(&p1), edges(&e1)), "{area:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sparse_city_index_keeps_exact_geometry_and_bounded_query_work() {
        let mut d = fixture(false);
        for i in 0..6000 {
            let mut r = d.roads[0].clone();
            let x = 200000 + i * 10000;
            r.id = format!("remote-{i}"); r.from = format!("s{i}"); r.to = format!("t{i}");
            r.points = vec![[x, 0, x], [x + 1000, 0, x]];
            r.widths_cm = vec![600]; r.surfaces = vec![Surface::Asphalt];
            d.roads.push(r);
        }
        let area = Bounds { min: [100, -200], max: [500, 200] };
        assert_same(&d, area.clone());
        let index = PlanIndex::new(&d).unwrap();
        let mut work = 0;
        index.plan(&area, Walls::All, &mut work).unwrap();
        assert!(work < 100, "spatial traversal should skip distant roads: {work}");
        assert_same(&d, Bounds { min: [-100000; 2], max: [-90000; 2] });
    }

    #[test]
    fn rounded_ground_retrace_preserves_both_approaches_and_a_real_cap() {
        let mut d = fixture(false);
        d.roads.truncate(1);
        d.roads[0].points = vec![[-350, 0, -90], [0, 0, 0], [-799, 0, -204]];
        d.roads[0].widths_cm = vec![20; 2];
        d.roads[0].surfaces = vec![Surface::Asphalt; 2];
        let area = Bounds { min: [-1000; 2], max: [1000; 2] };
        let (patches, edges) = plan(&d, &area).unwrap();
        assert!(!patches.is_empty() && !edges.is_empty());
        assert!(patches.iter().all(|p| orient(p.v[0], p.v[1], p.v[2]) != 0));
        assert!(patches.iter().flat_map(|p| p.v).any(|p| p[0] > 0));
        assert_same(&d, area);
    }

    #[test]
    fn surface_plan_keeps_exact_faces_and_structural_boundary_stationing() {
        let mut d = fixture(false);
        d.roads[1].kind = RoadKind::Bridge;
        d.roads[2].kind = RoadKind::Bridge;
        // Separate structural endpoint stars from the ground road.
        for road in &mut d.roads[1..3] {
            road.from = format!("deck-{}", road.from);
            road.to = format!("deck-{}", road.to);
            for p in &mut road.points { p[1] = 600; }
        }
        let bounds = Bounds { min: [-3000; 2], max: [10000; 2] };
        let (all, edges) = plan(&d, &bounds).unwrap();
        let (surface, structural) = surface_plan(&d, &bounds).unwrap();
        let faces = |v: Vec<Patch>| v.into_iter().map(|p|
            (p.v, p.road.id.clone(), p.surface, p.terrain_join)).collect::<Vec<_>>();
        assert_eq!(faces(all), faces(surface));
        let boundary = |v: Vec<Edge>| v.into_iter().filter(|e| e.road.kind != RoadKind::Ground)
            .map(|e| (e.a, e.b, e.station_cm.to_bits(), e.road.id.clone())).collect::<Vec<_>>();
        assert!(!structural.is_empty());
        assert_eq!(boundary(edges), boundary(structural));
    }

    #[test]
    fn dense_ground_bends_do_not_retain_unused_boundary_walls() {
        let mut d = fixture(false);
        let template = d.roads[0].clone();
        d.roads.clear();
        for row in 0..60 {
            let mut r = template.clone();
            r.id = format!("path-{row}"); r.from = format!("a-{row}"); r.to = format!("b-{row}");
            r.points = (0..10).map(|i| [i * 200, 0, row * 100 + (i % 2) * 40]).collect();
            r.widths_cm = vec![20; 9]; r.surfaces = vec![Surface::Asphalt; 9];
            d.roads.push(r);
        }
        let bounds = Bounds { min: [-100; 2], max: [7000; 2] };
        let (patches, edges) = surface_plan(&d, &bounds).unwrap();
        assert!(!patches.is_empty() && edges.is_empty());
        assert!(patches.len() <= MAX_LOCAL_PATCHES);
        assert_eq!(patches.iter().map(|p| &p.road.id).collect::<BTreeSet<_>>().len(), 60);
        // Compare exact per-road plans without requiring a larger workspace.
        let expected: Vec<_> = d.roads.iter().flat_map(|road| {
            let mut one = d.clone(); one.roads = vec![road.clone()];
            plan(&one, &bounds).unwrap().0.into_iter().map(|p| (p.v, p.road.id.clone())).collect::<Vec<_>>()
        }).collect();
        let mut actual: Vec<_> = patches.iter().map(|p| (p.v, p.road.id.clone())).collect();
        let mut expected = expected; expected.sort(); actual.sort();
        assert_eq!(actual, expected);
        // Unmarked imports require neither exterior paint nor ground walls for
        // safety cost estimation; these consumers must not rebuild a full plan.
        assert!(d.road_paint(&bounds).unwrap().edges.is_empty());
    }
}

pub(crate) fn plan<'a>(
    d: &'a MapDocument,
    bounds: &Bounds,
) -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
    plan_all(d, bounds, true, false)
}

/// [`plan`] of a validated `d` (generation only).
pub(crate) fn validated_plan<'a>(d: &'a MapDocument, bounds: &Bounds)
    -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
    plan_all(d, bounds, true, true)
}

/// Ground surface generation consumes no ground boundary walls. Sidewalks have
/// their own expanded plan; structural walls and fixed safety stationing remain.
/// `d` must be validated (generation and its estimate only).
pub(crate) fn surface_plan<'a>(d: &'a MapDocument, bounds: &Bounds)
    -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
    plan_all(d, bounds, false, true)
}

/// Point indices of `r` that bound its planned segments: every point, except
/// that a designed road's junction approaches skip interior samples.
fn chain(r: &Road, nodes: &BTreeMap<&str,(usize,u32)>) -> Vec<usize> {
    if r.design.is_none() {
        // No approach: running lengths never decrease, so every point is kept.
        return (0..r.points.len()).collect();
    }
    let mut distances=vec![0.0];
    for pair in r.points.windows(2){distances.push(distances.last().unwrap()+length(pair[0],pair[1]));}
    let total=*distances.last().unwrap();
    let approach=|id:&str|if r.design.is_some() && nodes[id].0>1 {(nodes[id].1 as f64*1.5+200.0).min(total/4.0)} else {0.0};
    let (start,end)=(approach(&r.from),approach(&r.to));
    (0..r.points.len()).filter(|&i|i==0 || i+1==r.points.len() || distances[i]>=start && total-distances[i]>=end).collect()
}
/// Plan of every road of `d`. Undesigned roads share one identity [`chain`];
/// node widths are only gathered when a designed road needs its approaches.
/// `validated`: road ends lie on their nodes and road IDs are unique, so every
/// [`key`] names one point (see [`plan_selected`]).
fn plan_all<'a>(d: &'a MapDocument, bounds: &Bounds, ground_edges: bool, validated: bool) -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
    let roads: Vec<_> = d.roads.iter().collect();
    let nodes = if roads.iter().any(|r| r.design.is_some()) { node_widths(d) } else { BTreeMap::new() };
    let identity: Vec<usize> = (0..roads.iter().map(|r| r.points.len()).max().unwrap_or(0)).collect();
    let designed: Vec<_> = roads.iter().map(|r| r.design.as_ref().map(|_| chain(r, &nodes))).collect();
    let chains: Vec<_> = roads.iter().zip(&designed)
        .map(|(r, chain)| chain.as_deref().unwrap_or(&identity[..r.points.len()])).collect();
    plan_selected(&roads, &chains, &|i, p| key(roads[i], p), bounds, influence_margin(d), ground_edges, Walls::All, None, validated)
}

/// `chains[i]` is [`chain`] of `roads[i]`; `key(i, point)` orders like [`key`]
/// of `roads[i]` and `point`.
/// `keyed_points`: equal keys always name the same point. A relevant key is then
/// an end of a hit segment, so ends outside those segments' bounds are skipped
/// without the key lookup.
#[allow(clippy::too_many_arguments)]
fn plan_selected<'a, K: Ord + Copy>(roads: &[&'a Road], chains: &[&[usize]], key: &dyn Fn(usize, usize) -> K, bounds: &Bounds,
    margin: i64, ground_edges: bool, keep: Walls, cache: Option<&std::cell::RefCell<JunctionCache<'a, K>>>, keyed_points: bool)
    -> Result<(Vec<Patch<'a>>, Vec<Edge<'a>>)> {
    let mut groups: BTreeMap<K, Vec<Arm>> = BTreeMap::new();
    // Include complete endpoint junctions when a corridor touches this cell.
    let mut relevant = BTreeSet::new();
    let mut local_segments = 0;
    let mut reach: Option<Bounds> = None;
    for (index, &r) in roads.iter().enumerate() {
        for indices in chains[index].windows(2) {
            let s=[r.points[indices[0]],r.points[indices[1]]];
            if hit(&s, bounds, margin) {
                local_segments += 1;
                if local_segments > MAX_LOCAL_PATCHES / 8 {
                    return Err(error("E_BUDGET", "road local segment limit"));
                }
                relevant.insert(key(index, indices[0]));
                relevant.insert(key(index, indices[1]));
                for p in s.map(xy).into_iter().filter(|_| keyed_points) {
                    let area = reach.get_or_insert(Bounds { min: p, max: p });
                    area.min = std::array::from_fn(|a| area.min[a].min(p[a]));
                    area.max = std::array::from_fn(|a| area.max[a].max(p[a]));
                }
            }
        }
    }
    let mut arm_count = 0;
    for (index, &r) in roads.iter().enumerate() {
        for indices in chains[index].windows(2) {
            let i=indices[0];let s=[r.points[i],r.points[indices[1]]];
            for end in 0..2 {
                if keyed_points && !reach.as_ref().is_some_and(|reach| reach.contains(xy(s[end]))) {
                    continue;
                }
                let k = key(index, indices[end]);
                if relevant.contains(&k) {
                    arm_count += 1;
                    if arm_count > MAX_LOCAL_PATCHES {
                        return Err(error("E_BUDGET", "road local arm limit"));
                    }
                    groups.entry(k).or_default().push(Arm {
                        road: r,
                        segment: i,
                        end,
                        point_index:indices[end],
                        point: s[end],
                        other: s[1 - end],
                    });
                }
            }
        }
    }
    let mut mouths: BTreeMap<MouthKey, [Vertex; 2]> = BTreeMap::new();
    let mut patches = vec![];
    let mut walls = vec![];
    let mut wall_count = 0;
    let mut sections=BTreeMap::new();
    for (key, arms) in groups.iter_mut() {
        crate::cancellation::checkpoint()?;
        let cached = cache.and_then(|c| c.borrow().get(key));
        let planned = match cached {
            Some(planned) => planned,
            None => {
                let planned = std::rc::Rc::new(plan_junction(arms, &mut sections, margin, ground_edges, keep)?);
                if let Some(c) = cache { c.borrow_mut().insert(*key, planned.clone()); }
                planned
            }
        };
        mouths.extend(planned.mouths.iter().copied());
        patches.extend(planned.faces.iter().filter(|p| hit(&p.v, bounds, 0)).cloned());
        walls.extend(planned.walls.iter().cloned());
        wall_count += planned.wall_count;
        if patches.len() + wall_count > MAX_LOCAL_PATCHES {
            return Err(error("E_BUDGET", format!("road local junction limit ({} faces, {} boundaries)", patches.len(), wall_count)));
        }
    }
    for (index, &r) in roads.iter().enumerate() {
        for indices in chains[index].windows(2) {
            let i=indices[0];let s=[r.points[i],r.points[indices[1]]];
            if !hit(&s, bounds, margin) {
                continue;
            }
            let a = mouths[&(r.id.as_str(), i, 0)];
            let c = mouths[&(r.id.as_str(), i, 1)];
            let v = [a[0], c[1], c[0], a[1]];
            for t in [[v[0], v[1], v[2]], [v[0], v[2], v[3]]] {
                // These faces were already discarded at the end of planning.
                // Do not retain invisible/degenerate faces against the local
                // workspace while complete outside endpoint stars are resolved.
                if !hit(&t, bounds, 0) || orient(t[0],t[1],t[2]) == 0 { continue; }
                patches.push(Patch {
                    v: t,
                    road: r,
                    surface: r.surfaces[i],
                    terrain_join: false,
                });
            }
            // Keep the road on the left of every exterior edge.
            for (a, b) in [(v[1], v[0]), (v[3], v[2])] {
                if !ground_edges && r.kind == RoadKind::Ground { continue; }
                wall_count += 1;
                if !keep.keep(r) { continue; }
                walls.push(Edge {
                    a,
                    b,
                    station_cm: 0.0,
                    road: r,
                });
            }
            if patches.len() + wall_count > MAX_LOCAL_PATCHES {
                return Err(error("E_BUDGET", "road local corridor limit"));
            }
        }
    }
    patches.retain(|p| hit(&p.v, bounds, 0) && orient(p.v[0], p.v[1], p.v[2]) != 0);
    let order = |p: &Patch| {
        if matches!(p.road.kind, RoadKind::Tunnel | RoadKind::Underpass) {
            0
        } else {
            1
        }
    };
    patches.sort_by(|a, b| (order(a), &a.road.id, a.v).cmp(&(order(b), &b.road.id, b.v)));
    Ok((patches, walls))
}

/// Bounded memo of [`Junction`] plans for one road set, node table, margin and
/// edge rule (one [`PlanIndex`]). Queries near the same junction (every solid
/// of a city block) otherwise rebuild identical fillets and unions. Clearing
/// when full only costs recomputation; results never depend on the cache.
pub(crate) struct JunctionCache<'a, K> {
    walls: Option<Walls>,
    plans: BTreeMap<K, std::rc::Rc<Junction<'a>>>,
    bytes: usize,
}
impl<K> Default for JunctionCache<'_, K> {
    fn default() -> Self { Self { walls: None, plans: BTreeMap::new(), bytes: 0 } }
}
impl<'a, K: Ord> JunctionCache<'a, K> {
    // Tests evict constantly; eviction never changes a plan.
    const MAX_BYTES: usize = if cfg!(test) { 4 * 1024 } else { 16 * 1024 * 1024 };
    fn get(&self, key: &K) -> Option<std::rc::Rc<Junction<'a>>> {
        self.plans.get(key).cloned()
    }
    fn insert(&mut self, key: K, plan: std::rc::Rc<Junction<'a>>) {
        use std::mem::size_of;
        let bytes = 128
            + plan.mouths.len() * size_of::<(MouthKey, [Vertex; 2])>()
            + plan.faces.len() * size_of::<Patch>()
            + plan.walls.len() * size_of::<Edge>();
        if self.bytes + bytes > Self::MAX_BYTES {
            self.plans.clear();
            self.bytes = 0;
        }
        self.bytes += bytes;
        self.plans.insert(key, plan);
    }
}
/// A segment end: road ID, segment, end.
type MouthKey<'a> = (&'a str, usize, usize);
/// Bounds-independent plan of one junction group: the arm mouths, every
/// non-degenerate face and every boundary edge, in planning order. A relevant
/// group always holds the complete endpoint star, so its plan never depends on
/// the query bounds; callers filter faces by bounds afterwards.
struct Junction<'a> {
    mouths: Vec<(MouthKey<'a>, [Vertex; 2])>,
    faces: Vec<Patch<'a>>,
    /// Retained boundary edges ([`Walls`]); `wall_count` counts all of them.
    walls: Vec<Edge<'a>>,
    wall_count: usize,
}
/// Boundary edges a plan returns. Every edge still counts toward the local
/// junction/corridor limits, so the rule never changes a plan's success.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Walls {
    All,
    /// Only bridge and elevated deck edges (the placement overlap rule).
    Decks,
}
impl Walls {
    fn keep(self, road: &Road) -> bool {
        self == Walls::All || matches!(road.kind, RoadKind::Bridge | RoadKind::Elevated)
    }
}
fn plan_junction<'a>(arms: &mut [Arm<'a>], sections: &mut BTreeMap<&'a str, Vec<[Vertex; 2]>>, margin: i64, ground_edges: bool,
    keep: Walls) -> Result<Junction<'a>> {
    let width = |r: &Road, i: usize| r.widths_cm[i] as f64;
    let mut mouths: BTreeMap<MouthKey<'a>, [Vertex; 2]> = BTreeMap::new();
    let mut out_faces = vec![];
    let mut out_walls = vec![];
    let mut wall_count = 0;
    if arms.len()==2 && arms[0].road.id==arms[1].road.id && arms[0].road.design.is_some() && arms[0].point_index==arms[1].point_index {
        for arm in arms.iter() {
            let at=arm.point_index;
            if !sections.contains_key(arm.road.id.as_str()) {sections.insert(arm.road.id.as_str(),crate::road_design::cross_sections(arm.road)?);}
            let mut section=sections[arm.road.id.as_str()][at];
            if arm.end==1 {section.swap(0,1);}
            mouths.insert((&arm.road.id,arm.segment,arm.end),section);
        }
        return Ok(Junction { mouths: mouths.into_iter().collect(), faces: vec![], walls: vec![], wall_count: 0 });
    }
    arms.sort_by_key(|a| (&a.road.id, a.segment, a.end));
    let radius = arms
        .iter()
        .map(|a| width(a.road, a.segment) / 2.0)
        .fold(0.0, f64::max);
    let mut ring = vec![];
    for arm in arms.iter() {
        if arms.len()==1 && arm.road.design.is_some() {
            if !sections.contains_key(arm.road.id.as_str()) {sections.insert(arm.road.id.as_str(),crate::road_design::cross_sections(arm.road)?);}
            let mut mouth=sections[arm.road.id.as_str()][arm.point_index];
            if arm.end==1 {mouth.swap(0,1);}
            mouths.insert((&arm.road.id,arm.segment,arm.end),mouth);ring.extend(mouth);
            continue;
        }
        let dx = (arm.other[0] - arm.point[0]) as f64;
        let dy = (arm.other[2] - arm.point[2]) as f64;
        let len = libm::sqrt(dx * dx + dy * dy);
        let t = if arms.len() > 1 {
            radius.min(len * 0.40) / len
        } else {
            0.0
        };
        let center: Vertex = std::array::from_fn(|i| {
            arm.point[i] + libm::round((arm.other[i] - arm.point[i]) as f64 * t) as i64
        });
        let half = width(arm.road, arm.segment) / 2.0;
        let offset = [
            libm::round(-dy / len * half) as i64,
            libm::round(dx / len * half) as i64,
        ];
        let mouth = [
            [center[0] + offset[0], center[1], center[2] + offset[1]],
            [center[0] - offset[0], center[1], center[2] - offset[1]],
        ];
        mouths.insert((&arm.road.id, arm.segment, arm.end), mouth);
        ring.extend(mouth);
    }
    if arms.len() > 1 {
        // Nearly retraced ground approaches can have identical rounded
        // mouths. Their hull has no depth, although the physical corridor
        // has a real terminal cap at the shared node. Include that cap
        // before the ordinary union/fillet path, preserving both arms.
        if arms.iter().all(|a| a.road.kind == RoadKind::Ground) {
            let first = &arms[0];
            let len = length(first.point, first.other);
            let direction = [
                (first.other[0] - first.point[0]) as f64 / len,
                (first.other[2] - first.point[2]) as f64 / len,
            ];
            let retraced = arms.iter().all(|a| {
                let length = length(a.point, a.other);
                let dot = ((a.other[0] - a.point[0]) as f64 * direction[0]
                    + (a.other[2] - a.point[2]) as f64 * direction[1]) / length;
                dot > 0.9999
            });
            if retraced {
                for side in [-1.0, 1.0] {
                    ring.push([
                        first.point[0] + libm::round(-direction[0] * radius - direction[1] * radius * side) as i64,
                        first.point[1],
                        first.point[2] + libm::round(-direction[1] * radius + direction[0] * radius * side) as i64,
                    ]);
                }
            }
        }
        let ring = hull(ring);
        // Coincident horizontal mouth corners have one shared height even
        // when the incoming grades differ. Preserve the authored node and
        // interpolate each outgoing corridor from this canonical corner.
        for arm in arms.iter() {
            let m = mouths
                .get_mut(&(arm.road.id.as_str(), arm.segment, arm.end))
                .unwrap();
            for p in m {
                if let Some(q) = ring.iter().find(|q| xy(**q) == xy(*p)) {
                    *p = *q;
                }
            }
        }
        let structural = arms.iter().any(|a| a.road.kind != RoadKind::Ground);
        let terrain_join = structural && arms.iter().any(|a| a.road.kind == RoadKind::Ground);
        if structural {
            for arm in arms.iter() {
                let m = mouths[&(arm.road.id.as_str(), arm.segment, arm.end)];
                if !ring
                    .iter()
                    .enumerate()
                    .any(|(i, a)| m.contains(a) && m.contains(&ring[(i + 1) % ring.len()]))
                {
                    return Err(error(
                        "E_GEOMETRY",
                        format!("overlapping structural junction mouths at {} segment {} {:?}; author separated approaches", arm.road.id, arm.segment, arm.point),
                    ));
                }
            }
            let ceilings: BTreeSet<_> = arms
                .iter()
                .filter(|a| a.road.kind == RoadKind::Tunnel)
                .map(|a| a.road.clearance_cm)
                .collect();
            if ceilings.len() > 1 {
                return Err(error("E_GEOMETRY", "joined tunnel clearances must agree"));
            }
        }
        // Extend each internal mouth before rounding. Its straight cross-section
        // remains a shared edge with the corridor; fillets never cap an entrance.
        let original: BTreeMap<_, _> = arms
            .iter()
            .map(|arm| {
                let k = (arm.road.id.as_str(), arm.segment, arm.end);
                (k, mouths[&k])
            })
            .collect();
        let mut corners = Vec::new();
        for i in 0..ring.len() {
            let a = ring[i];
            let c = ring[(i + 1) % ring.len()];
            let owner = arms
                .iter()
                .position(|arm| {
                    original[&(arm.road.id.as_str(), arm.segment, arm.end)].contains(&a)
                })
                .unwrap_or(0); // A retraced cap belongs to the first source arm.
            corners.push(Corner {
                p: a,
                owner,
                round: true,
                seam: false,
            });
            if let Some((index, arm)) = arms.iter().enumerate().find(|(_, arm)| {
                let m = original[&(arm.road.id.as_str(), arm.segment, arm.end)];
                m.contains(&a) && m.contains(&c)
            }) {
                let len = length(arm.point, arm.other);
                let old = radius.min(len * 0.40);
                let extra = (radius + 200.0).min(len * 0.45) - old;
                let shift = |p: Vertex| {
                    std::array::from_fn(|j| {
                        p[j] + libm::round((arm.other[j] - arm.point[j]) as f64 * extra / len)
                            as i64
                    })
                };
                let m = original[&(arm.road.id.as_str(), arm.segment, arm.end)].map(shift);
                mouths.insert((&arm.road.id, arm.segment, arm.end), m);
                corners.last_mut().unwrap().owner = index;
                corners.push(Corner {
                    p: shift(a),
                    owner: index,
                    round: false,
                    seam: true,
                });
                corners.push(Corner {
                    p: shift(c),
                    owner: index,
                    round: false,
                    seam: false,
                });
            } else {
                // Intersect the actual offset boundaries. A convex hull of
                // mouth cross-sections alone cuts off the outside of bends
                // and removes support from the outer lane.
                let exposed = |arm: &Arm| {
                    let m = original[&(arm.road.id.as_str(), arm.segment, arm.end)];
                    (0..ring.len()).any(|j| {
                        m.contains(&ring[j]) && m.contains(&ring[(j + 1) % ring.len()])
                    })
                };
                'pairs: for (ai, aa) in arms.iter().enumerate() {
                    // A mouth hidden inside another ground approach is not
                    // an exterior branch; extending its line makes false lobes.
                    if !exposed(aa) {
                        continue;
                    }
                    if !original[&(aa.road.id.as_str(), aa.segment, aa.end)].contains(&a) {
                        continue;
                    }
                    for bb in arms.iter() {
                        if !exposed(bb) {
                            continue;
                        }
                        if !original[&(bb.road.id.as_str(), bb.segment, bb.end)].contains(&c) {
                            continue;
                        }
                        let u = [
                            (aa.point[0] - aa.other[0]) as f64,
                            (aa.point[2] - aa.other[2]) as f64,
                        ];
                        let v = [
                            (bb.point[0] - bb.other[0]) as f64,
                            (bb.point[2] - bb.other[2]) as f64,
                        ];
                        let cross = |p: [f64; 2], q: [f64; 2]| p[0] * q[1] - p[1] * q[0];
                        let determinant = cross(u, v);
                        if determinant.abs() < 1.0 {
                            continue;
                        }
                        let delta = [(c[0] - a[0]) as f64, (c[2] - a[2]) as f64];
                        let t = cross(delta, v) / determinant;
                        let s = cross(delta, u) / determinant;
                        if t < -1e-9 || s < -1e-9 {
                            continue;
                        }
                        let p = [
                            libm::round(a[0] as f64 + u[0] * t) as i64,
                            libm::round(
                                (a[1] as f64
                                    + (aa.point[1] - aa.other[1]) as f64 * t
                                    + c[1] as f64
                                    + (bb.point[1] - bb.other[1]) as f64 * s)
                                    * 0.5,
                            ) as i64,
                            libm::round(a[2] as f64 + u[1] * t) as i64,
                        ];
                        if length(p, aa.point) > margin as f64 {
                            return Err(geometry_error(aa,p,"offset boundaries exceed junction influence; separate the approaches"));
                        }
                        if xy(p) != xy(a) && xy(p) != xy(c) {
                            corners.push(Corner {
                                p,
                                owner: ai,
                                round: true,
                                seam: false,
                            });
                        }
                        break 'pairs;
                    }
                }
            }
        }
        let corners = resolve_ground_overlaps(corners, arms)?;
        let rounded = rounded_ring(&corners, arms)?;
        // Quantized tangencies can make the node fall outside a tiny fan
        // sector. Triangulate the actual simple polygon, retaining the
        // authored node as an interior height constraint when necessary.
        let center = arms[0].point;
        let mut faces = Vec::new();
        if (0..rounded.len())
            .all(|i| orient(center, rounded[i].p, rounded[(i + 1) % rounded.len()].p) >= 0)
        {
            for i in 0..rounded.len() {
                faces.push((
                    [center, rounded[i].p, rounded[(i + 1) % rounded.len()].p],
                    rounded[i].owner,
                ));
            }
        } else {
            let poly: Vec<_> = rounded.iter().map(|c| xy(c.p)).collect();
            for indices in crate::generation::polygon_triangles(&poly)? {
                let v = indices.map(|i| rounded[i].p);
                let owner = rounded[indices[1]].owner;
                if point_in_polygon(xy(center), &v.map(xy)) && !v.contains(&center) {
                    for i in 0..3 {
                        faces.push(([center, v[i], v[(i + 1) % 3]], owner));
                    }
                } else {
                    faces.push((v, owner));
                }
            }
        }
        for (v, owner) in faces {
            if orient(v[0], v[1], v[2]) != 0 {
                let owner = &arms[owner];
                out_faces.push(Patch {
                    v,
                    road: owner.road,
                    surface: owner.road.surfaces[owner.segment],
                    terrain_join,
                });
            }
        }
        let mut station = 0.0;
        for i in 0..rounded.len() {
            let a = &rounded[i];
            let c = &rounded[(i + 1) % rounded.len()];
            let owner = &arms[a.owner];
            if !a.seam && (ground_edges || owner.road.kind != RoadKind::Ground) {
                wall_count += 1;
                if keep.keep(owner.road) {
                    out_walls.push(Edge {
                        a: a.p,
                        b: c.p,
                        station_cm: station,
                        road: owner.road,
                    });
                }
            }
            station += length(a.p, c.p);
        }
    } else {
        let arm = &arms[0];
        let m = mouths[&(arm.road.id.as_str(), arm.segment, arm.end)];
        // The unconnected end is a real exterior, including the deck cap.
        if !matches!(arm.road.kind, RoadKind::Tunnel | RoadKind::Underpass)
            && (ground_edges || arm.road.kind != RoadKind::Ground) {
            wall_count += 1;
            if keep.keep(arm.road) {
                out_walls.push(Edge {
                    a: m[0],
                    b: m[1],
                    station_cm: 0.0,
                    road: arm.road,
                });
            }
        }
    }
    Ok(Junction { mouths: mouths.into_iter().collect(), faces: out_faces, walls: out_walls, wall_count })
}
#[derive(Clone)]
struct Corner {
    p: Vertex,
    owner: usize,
    round: bool,
    seam: bool,
}
fn geometry_error(arm: &Arm, p: Vertex, message: &str) -> Error {
    error(
        "E_GEOMETRY",
        format!(
            "{} segment {} at {:?}: {}",
            arm.road.id, arm.segment, p, message
        ),
    )
}
pub(crate) fn length(a: Vertex, b: Vertex) -> f64 {
    libm::hypot((b[0] - a[0]) as f64, (b[2] - a[2]) as f64)
}
pub(crate) fn corner_radius(width: u32) -> f64 {
    (width as f64 * 0.15).clamp(20.0, 100.0)
}

/// Tangent circle, <=10 degree chords and <=1cm sagitta, then the canonical
/// libm nearest-centimetre rule. Both tangencies use <half of adjacent edges.
pub(crate) fn fillet(a: Vertex, p: Vertex, b: Vertex, radius: f64) -> Vec<Vertex> {
    let (la, lb) = (length(a, p), length(p, b));
    if la < 2.0 || lb < 2.0 {
        return vec![p];
    }
    let u = [(p[0] - a[0]) as f64 / la, (p[2] - a[2]) as f64 / la];
    let v = [(b[0] - p[0]) as f64 / lb, (b[2] - p[2]) as f64 / lb];
    let turn = libm::atan2(u[0] * v[1] - u[1] * v[0], u[0] * v[0] + u[1] * v[1]);
    if turn.abs() < 0.001 {
        return vec![p];
    }
    let tangent = libm::tan(turn.abs() * 0.5);
    let distance = (radius * tangent).min(la * 0.45).min(lb * 0.45);
    let r = distance / tangent;
    let start = [p[0] as f64 - u[0] * distance, p[2] as f64 - u[1] * distance];
    let center = [
        start[0] - u[1] * turn.signum() * r,
        start[1] + u[0] * turn.signum() * r,
    ];
    let theta = libm::atan2(start[1] - center[1], start[0] - center[0]);
    let step = (std::f64::consts::PI / 18.0)
        .min(2.0 * libm::acos((1.0 - 1.0 / r.max(1.0)).clamp(-1.0, 1.0)));
    let n = (libm::ceil(turn.abs() / step) as usize).div_ceil(2) * 2;
    let y0 = p[1] as f64 - (p[1] - a[1]) as f64 * distance / la;
    let y1 = p[1] as f64 + (b[1] - p[1]) as f64 * distance / lb;
    let mut out = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let angle = theta + turn * t;
        let q = [
            libm::round(center[0] + r * libm::cos(angle)) as i64,
            libm::round(y0 + (y1 - y0) * t) as i64,
            libm::round(center[1] + r * libm::sin(angle)) as i64,
        ];
        if out.last() != Some(&q) {
            out.push(q);
        }
    }
    out
}
fn rounded_ring(corners: &[Corner], arms: &[Arm]) -> Result<Vec<Corner>> {
    let mut out = Vec::new();
    for (i, c) in corners.iter().enumerate() {
        let before = &corners[(i + corners.len() - 1) % corners.len()];
        let after = &corners[(i + 1) % corners.len()];
        let radius = corner_radius(arms[c.owner].road.widths_cm[arms[c.owner].segment]);
        let points = if c.round {
            fillet(before.p, c.p, after.p, radius)
        } else {
            vec![c.p]
        };
        for p in points {
            if out.last().is_some_and(|last: &Corner| last.p == p) {
                out.pop();
            }
            out.push(Corner { p, ..c.clone() });
        }
    }
    if out.len() > 1 && out[0].p == out.last().unwrap().p {
        out.pop();
    }
    // Centimetre rounding can collapse a sub-centimetre tangent into A-B-A.
    // Remove only these tiny quantization spikes, preserving the outgoing owner;
    // larger loops still fail the self-intersection diagnostic below.
    loop {
        let spike = (0..out.len()).find(|&i| {
            let before = &out[(i + out.len() - 1) % out.len()];
            let after = &out[(i + 1) % out.len()];
            xy(before.p) == xy(after.p) && length(before.p, out[i].p) <= 2.0
        });
        let Some(i) = spike else {
            break;
        };
        if out.len() <= 3 {
            break;
        }
        let before = (i + out.len() - 1) % out.len();
        let after = (i + 1) % out.len();
        out[before] = out[after].clone();
        for index in [i.max(after), i.min(after)] {
            out.remove(index);
        }
    }
    let out = resolve_ground_overlaps(out, arms)?;
    for i in 0..out.len() {
        crate::cancellation::checkpoint()?;
        let (a, b) = (xy(out[i].p), xy(out[(i + 1) % out.len()].p));
        for j in i + 2..out.len() {
            if i == 0 && j + 1 == out.len() {
                continue;
            }
            let (c, d) = (xy(out[j].p), xy(out[(j + 1) % out.len()].p));
            if a != b && c != d && intersects(a, b, c, d) {
                return Err(geometry_error(
                    &arms[out[i].owner],
                    out[i].p,
                    &format!("rounded boundary self-intersects {:?}", [a, b, c, d]),
                ));
            }
        }
    }
    Ok(out)
}

// Ground approach envelopes can overlap at acute multi-arm junctions. Resolve
// their winding union before rounding; never use an unrounded fallback outline.
fn resolve_ground_overlaps(corners: Vec<Corner>, arms: &[Arm]) -> Result<Vec<Corner>> {
    let n = corners.len();
    let crossed = (0..n).any(|i| {
        (i + 2..n).any(|j| {
            !(i == 0 && j + 1 == n)
                && intersects(
                    xy(corners[i].p),
                    xy(corners[(i + 1) % n].p),
                    xy(corners[j].p),
                    xy(corners[(j + 1) % n].p),
                )
        })
    });
    if !crossed || arms.iter().any(|a| a.road.kind != RoadKind::Ground) {
        return Ok(corners);
    }
    use i_overlay::{
        core::{
            fill_rule::FillRule,
            overlay::{Overlay, ShapeType},
            overlay_rule::OverlayRule,
        },
        i_float::int::point::IntPoint,
    };
    let mut overlay = Overlay::<i64>::new(n);
    overlay.add_contour(
        &corners
            .iter()
            .map(|c| IntPoint::new(c.p[0], c.p[2]))
            .collect::<Vec<_>>(),
        ShapeType::Subject,
    );
    let twice_area = |c: &Vec<IntPoint<i64>>| -> i128 {
        (0..c.len())
            .map(|i| {
                let a = c[i];
                let b = c[(i + 1) % c.len()];
                a.x as i128 * b.y as i128 - a.y as i128 * b.x as i128
            })
            .sum::<i128>()
            .abs()
    };
    let quantized_sliver = |c: &Vec<IntPoint<i64>>| {
        twice_area(c) <= 4
            || (0..c.len()).any(|i| {
                let a = c[i];
                let b = c[(i + 1) % c.len()];
                let dx = (b.x - a.x) as f64;
                let dy = (b.y - a.y) as f64;
                let len = libm::hypot(dx, dy);
                if len == 0.0 || len > 100.0 {
                    return false;
                }
                let mut low = f64::INFINITY;
                let mut high = f64::NEG_INFINITY;
                for p in c {
                    let distance = ((p.x - a.x) as f64 * dy - (p.y - a.y) as f64 * dx) / len;
                    low = low.min(distance);
                    high = high.max(distance);
                }
                high - low <= 2.0
            })
    };
    let mut shapes = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
    // Integer intersection of centimetre-rounded chords can leave half-square-
    // centimetre slivers. Collapse only these quantization remnants; real holes
    // or disconnected passages still fail below.
    if shapes.len() > 1 {
        shapes.retain(|s| !quantized_sliver(&s[0]));
    }
    for shape in &mut shapes {
        let mut index = 0;
        shape.retain(|c| {
            let keep = index == 0 || !quantized_sliver(c);
            index += 1;
            keep
        });
    }
    if shapes.len() != 1 || shapes[0].len() != 1 {
        return Err(geometry_error(
            &arms[0],
            arms[0].point,
            &format!(
                "junction union disconnects the passage or creates a hole {:?}",
                shapes
                    .iter()
                    .map(|s| s
                        .iter()
                        .map(|c| {
                            let area: i128 = (0..c.len())
                                .map(|i| {
                                    let a = c[i];
                                    let b = c[(i + 1) % c.len()];
                                    a.x as i128 * b.y as i128 - a.y as i128 * b.x as i128
                                })
                                .sum();
                            (c.len(), area)
                        })
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            ),
        ));
    }
    let mut outline: Vec<Point> = shapes[0][0].iter().map(|p| [p.x, p.y]).collect();
    let area: i128 = (0..outline.len())
        .map(|i| {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
        })
        .sum();
    if area < 0 {
        outline.reverse();
    }
    let mut result = Vec::new();
    for i in 0..outline.len() {
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        // Retain original collinear ownership/seam breakpoints removed by union.
        let mut points = vec![a];
        points.extend(corners.iter().map(|c| xy(c.p)).filter(|p| {
            *p != a
                && *p != b
                && cross(a, b, *p) == 0
                && (0..2).all(|k| p[k] >= a[k].min(b[k]) && p[k] <= a[k].max(b[k]))
        }));
        points.sort_by_key(|p| {
            (p[0] - a[0]) as i128 * (b[0] - a[0]) as i128
                + (p[1] - a[1]) as i128 * (b[1] - a[1]) as i128
        });
        points.dedup();
        for (k, p) in points.iter().enumerate() {
            let q = points.get(k + 1).copied().unwrap_or(b);
            let mid = [(p[0] + q[0]) as f64 * 0.5, (p[1] + q[1]) as f64 * 0.5];
            let mut matched = None;
            for j in 0..n {
                let c = &corners[j];
                let d = &corners[(j + 1) % n];
                let dx = (d.p[0] - c.p[0]) as f64;
                let dz = (d.p[2] - c.p[2]) as f64;
                let length2 = dx * dx + dz * dz;
                if length2 == 0.0 {
                    continue;
                }
                let t = ((mid[0] - c.p[0] as f64) * dx + (mid[1] - c.p[2] as f64) * dz) / length2;
                let distance = ((mid[0] - c.p[0] as f64) * dz - (mid[1] - c.p[2] as f64) * dx)
                    .abs()
                    / libm::sqrt(length2);
                if (-0.01..=1.01).contains(&t)
                    && distance <= 1.0
                    && dx * (q[0] - p[0]) as f64 + dz * (q[1] - p[1]) as f64 > 0.0
                {
                    let tp = (((p[0] - c.p[0]) as f64 * dx + (p[1] - c.p[2]) as f64 * dz)
                        / length2)
                        .clamp(0.0, 1.0);
                    matched = Some(Corner {
                        p: [
                            p[0],
                            libm::round(c.p[1] as f64 + (d.p[1] - c.p[1]) as f64 * tp) as i64,
                            p[1],
                        ],
                        owner: c.owner,
                        round: if *p == xy(c.p) { c.round } else { true },
                        seam: c.seam,
                    });
                    break;
                }
            }
            result.push(matched.ok_or_else(|| {
                geometry_error(
                    &arms[0],
                    [p[0], arms[0].point[1], p[1]],
                    "junction union lost boundary ownership",
                )
            })?);
        }
    }
    Ok(result)
}
