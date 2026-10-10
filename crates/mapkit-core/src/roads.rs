//! explicit graph aprons and planar terrain/road subdivision.
//! Recipe 1 remains in generation.rs. All cuts use one integer half-plane rule.
use crate::generation::Builder;
use crate::*;

pub(crate) const SCRATCH_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FRAGMENTS: usize = 16_384;
const MAX_WORK: usize = 8_000_000;
type Poly = Vec<Vertex>;
use crate::road_plan::{plan, hit, xy, orient, Patch};
pub(crate) use crate::road_plan::{influence_margin, width_influence_margin};
/// Adjacency uses authored endpoint IDs (and consequently their exact level).
/// A geometric crossing never creates an edge or joins distinct graph nodes.
impl MapDocument {
    pub fn connected_roads(&self, node_id: &str) -> Result<Vec<String>> {
        self.validate()?;
        if !self.nodes.iter().any(|n| n.id == node_id) {
            return Err(error("E_REFERENCE", "unknown road node"));
        }
        let mut ids: Vec<_> = self
            .roads
            .iter()
            .filter(|r| r.from == node_id || r.to == node_id)
            .map(|r| r.id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}
pub(crate) fn validate_graph(d: &MapDocument) -> Result<()> {
    crate::road_design::validate_crossings(d)?;
    let mut degree = BTreeMap::new();
    let mut work=0;
    for r in &d.roads {
        crate::road_design::validate(r)?;
        for id in [&r.from, &r.to] {
            let n = degree.entry(id).or_insert(0usize);
            *n += 1;
            if *n > 32 {
                return Err(error("E_LIMIT", "road junction exceeds 32 arms"));
            }
        }
        let areas:Vec<_>=r.points.windows(2).map(|s|crate::bounds_index::bounds(&[xy(s[0]),xy(s[1])])).collect();
        let index=crate::bounds_index::BoundsIndex::new(&areas);
        for (i,s) in r.points.windows(2).enumerate() {
            for j in index.query(&areas[i],&mut work)?.into_iter().filter(|&j|j>i+1) {
                tick(&mut work,1)?;
                if r.from==r.to && i==0 && j+2==r.points.len() {continue;}
                let (a,b,c,e)=(s[0],s[1],r.points[j],r.points[j+1]);
                if !intersects(xy(a),xy(b),xy(c),xy(e)) {continue;}
                let den=(b[0]-a[0]) as i128*(e[2]-c[2]) as i128-(b[2]-a[2]) as i128*(e[0]-c[0]) as i128;
                if den==0 {return Err(error("E_GEOMETRY",format!("{} segments {i}/{j} at {:?}: overlapping path",r.id,a)));}
                let t=((c[0]-a[0]) as i128*(e[2]-c[2]) as i128-(c[2]-a[2]) as i128*(e[0]-c[0]) as i128) as f64/den as f64;
                let u=((c[0]-a[0]) as i128*(b[2]-a[2]) as i128-(c[2]-a[2]) as i128*(b[0]-a[0]) as i128) as f64/den as f64;
                let p:Vertex=std::array::from_fn(|k|a[k]+libm::round((b[k]-a[k]) as f64*t) as i64);
                let other=c[1] as f64+(e[1]-c[1]) as f64*u;
                if r.kind==RoadKind::Ground || (p[1] as f64-other).abs()<=1.0 {
                    return Err(error("E_GEOMETRY",format!("{} segments {i}/{j} at {:?}: path self-intersects",r.id,p)));
                }
            }
        }
        if r.from == r.to && r.points.len() == 2 {
            return Err(error("E_GEOMETRY", "road loop needs intermediate points"));
        }
    }
    Ok(())
}
pub(crate) fn tick(work: &mut usize, n: usize) -> Result<()> {
    crate::cancellation::checkpoint()?;
    *work = work.saturating_add(n);
    if *work > MAX_WORK {
        Err(error("E_BUDGET", "road subdivision work exceeded"))
    } else {
        Ok(())
    }
}
// Both halves share the exact same intersection, independent of traversal.
pub(crate) fn split(poly: &[Vertex], plane: impl Fn(Vertex) -> i128) -> (Poly, Poly) {
    let mut inside = vec![];
    let mut outside = vec![];
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = plane(a);
        let db = plane(b);
        if da >= 0 {
            inside.push(a);
        }
        if da <= 0 {
            outside.push(a);
        }
        if (da < 0 && db > 0) || (da > 0 && db < 0) {
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            let da = plane(a);
            let db = plane(b);
            let p = std::array::from_fn(|j| {
                ((a[j] as i128 * (da - db) + (b[j] - a[j]) as i128 * da) / (da - db)) as i64
            });
            inside.push(p);
            outside.push(p);
        }
    }
    inside.dedup();
    outside.dedup();
    if inside.len() > 1 && inside.first() == inside.last() {
        inside.pop();
    }
    if outside.len() > 1 && outside.first() == outside.last() {
        outside.pop();
    }
    (inside, outside)
}
pub(crate) fn valid(poly: &[Vertex]) -> bool {
    poly.len() >= 3 && (1..poly.len() - 1).any(|i| orient(poly[0], poly[i], poly[i + 1]) != 0)
}
pub(crate) fn partition(
    poly: &[Vertex],
    clip: &[Vertex; 3],
    work: &mut usize,
) -> Result<(Poly, Vec<Poly>)> {
    tick(work, poly.len() * 3)?;
    let sign = orient(clip[0], clip[1], clip[2]).signum();
    if sign == 0 {
        return Ok((vec![], vec![poly.to_vec()]));
    }
    let mut remaining = poly.to_vec();
    let mut outside = vec![];
    for i in 0..3 {
        let (a, b) = (clip[i], clip[(i + 1) % 3]);
        let (yes, no) = split(&remaining, |p| orient(a, b, p) * sign);
        if valid(&no) {
            outside.push(no);
        }
        remaining = yes;
        if !valid(&remaining) {
            return Ok((vec![], outside));
        }
    }
    Ok((remaining, outside))
}
pub(crate) fn emit(
    b: &mut Builder,
    poly: &[Vertex],
    surface: Surface,
    id: &str,
    spawn: bool,
) -> Result<()> {
    for i in 1..poly.len().saturating_sub(1) {
        b.triangle([poly[0], poly[i], poly[i + 1]], surface, id, spawn)?;
    }
    Ok(())
}
fn floor_plane(v: &[Vertex; 3], p: Vertex) -> i128 {
    let area = orient(v[0], v[1], v[2]);
    let value = orient(v[1], v[2], p) * v[0][1] as i128
        + orient(v[2], v[0], p) * v[1][1] as i128
        + orient(v[0], v[1], p) * v[2][1] as i128;
    (p[1] as i128 * area - value) * area.signum()
}
pub(crate) fn on_plane(v: &[Vertex; 3], p: Vertex) -> Vertex {
    let area = orient(v[0], v[1], v[2]);
    let height = (orient(v[1], v[2], p) * v[0][1] as i128
        + orient(v[2], v[0], p) * v[1][1] as i128
        + orient(v[0], v[1], p) * v[2][1] as i128)
        / area;
    [p[0], height as i64, p[2]]
}
fn designed_plane(v:&[Vertex;3],p:Vertex)->Vertex {
    let area=orient(v[0],v[1],v[2]);
    let numerator=orient(v[1],v[2],p)*v[0][1] as i128+orient(v[2],v[0],p)*v[1][1] as i128+orient(v[0],v[1],p)*v[2][1] as i128;
    let sign=numerator.signum()*area.signum();
    [p[0],(sign*((numerator.abs()+area.abs()/2)/area.abs())) as i64,p[2]]
}
pub(crate) fn hull(mut points: Vec<Vertex>) -> Vec<Vertex> {
    points.sort_by_key(|p| (p[0], p[2], p[1]));
    points.dedup_by_key(|p| (p[0], p[2]));
    if points.len() < 3 {
        return points;
    }
    let mut out = vec![];
    for p in &points {
        while out.len() >= 2 && orient(out[out.len() - 2], out[out.len() - 1], *p) < 0 {
            out.pop();
        }
        out.push(*p);
    }
    let n = out.len();
    for p in points.iter().rev().skip(1) {
        while out.len() > n && orient(out[out.len() - 2], out[out.len() - 1], *p) < 0 {
            out.pop();
        }
        out.push(*p);
    }
    out.pop();
    out
}
// The current arrangement cuts each terrain tile once, before assigning surface identities.
fn ground_tile(
    d: &MapDocument,
    v: [Vertex; 4],
    patches: &[Patch],
    track: &[[Vertex;3]],
    b: &mut Builder,
    work: &mut usize,
) -> Result<()> {
    let bounds = Bounds {
        min: xy(v[0]),
        max: [v[2][0].min(b.bounds.max[0]), v[2][2].min(b.bounds.max[1])],
    };
    if (0..2).any(|i| bounds.min[i] >= bounds.max[i]) {
        return Ok(());
    }
    ground_tile_part(d, v, bounds, patches, track, b, work, 0)
}

// Dense city terrain is divided spatially before an arrangement can exceed its
// local edge/intersection budget. Every leaf samples the original two terrain
// planes; subdivision never bilinearly invents a different slope or elevation.
#[allow(clippy::too_many_arguments)]
fn ground_tile_part(
    d: &MapDocument, v: [Vertex;4], bounds: Bounds, patches: &[Patch],
    track: &[[Vertex;3]], b: &mut Builder, work: &mut usize, depth: u8,
) -> Result<()> {
    tick(
        work,
        patches.len()
            + d.surface_areas
                .iter()
                .map(|a| a.polygon.len())
                .sum::<usize>(),
    )?;
    let terrain = [[v[0], v[1], v[2]], [v[0], v[2], v[3]]];
    let nearby: Vec<_> = patches.iter().filter(|p| hit(&p.v, &bounds, 2)).collect();
    let nearby_track:Vec<_>=track.iter().filter(|f|hit(&f[..],&bounds,2)
        && f.iter().map(|p|p[1]).min().unwrap()<=v.iter().map(|p|p[1]).max().unwrap()+50
        && f.iter().map(|p|p[1]).max().unwrap()>=v.iter().map(|p|p[1]).min().unwrap()-50).collect();
    let mut lines = vec![(xy(v[0]), xy(v[2]))];
    // Only footprint boundaries partition terrain. Rigid pipe meshes contain
    // many tiny internal faces which must not spend the tile edge budget again.
    let mut track_edges=BTreeMap::new();
    for face in &nearby_track {for i in 0..3 {
        let (a,b)=(xy(face[i]),xy(face[(i+1)%3]));
        let edge=if a<b {(a,b)} else {(b,a)};
        *track_edges.entry(edge).or_insert(0usize)+=1;
    }}
    lines.extend(track_edges.into_iter().filter(|(_,count)|*count==1).map(|(edge,_)|edge));
    let mut shared = BTreeMap::<(&str,u8,Point,Point), Vec<&Patch>>::new();
    for patch in &nearby {
        for i in 0..3 {
            let (a,b)=(xy(patch.v[i]),xy(patch.v[(i+1)%3]));
            let (a,b)=if a<b {(a,b)} else {(b,a)};
            shared.entry((&patch.road.id,patch.surface as u8,a,b)).or_default().push(patch);
        }
    }
    for ((_,_,a,b),owners) in shared {
        // Ground follows the terrain, and bridge/underpass footprints do not
        // need their internal fan diagonals to partition that terrain again.
        let redundant=owners.len()==2 && ((owners[0].road.kind!=RoadKind::Tunnel && owners[0].road.design.is_none())
            || owners[1].v.iter().all(|p|floor_plane(&owners[0].v,*p)==0));
        if !redundant { lines.push((a,b)); }
    }
    for patch in &nearby {
        for t in terrain {
            if patch.terrain_join && patch.road.design.is_none() {
                let (inside, _) = partition(&t, &patch.v, work)?;
                if let Some(point) = inside
                    .iter()
                    .find(|p| (on_plane(&patch.v, **p)[1] - on_plane(&t, **p)[1]).abs() > 1)
                {
                    return Err(error("E_GEOMETRY", format!("ground/structure junction apron at {} {:?} must match terrain; author a level approach", patch.road.id, point)));
                }
            }
            if patch.road.kind == RoadKind::Tunnel {
                let ceiling = patch
                    .v
                    .map(|p| [p[0], p[1] + patch.road.clearance_cm.unwrap() as i64, p[2]]);
                let mut crossing = Vec::new();
                for i in 0..3 {
                    let (a, c) = (t[i], t[(i + 1) % 3]);
                    let (da, dc) = (floor_plane(&ceiling, a), floor_plane(&ceiling, c));
                    if da == 0 {
                        crossing.push(xy(a));
                    }
                    if (da < 0 && dc > 0) || (da > 0 && dc < 0) {
                        let p = std::array::from_fn(|j| {
                            ((a[j] as i128 * (da - dc) + (c[j] - a[j]) as i128 * da) / (da - dc))
                                as i64
                        });
                        crossing.push(xy(p));
                    }
                }
                crossing.sort();
                crossing.dedup();
                if crossing.len() >= 2 {
                    lines.push((crossing[0], *crossing.last().unwrap()));
                }
            }
            if patch.road.design.is_some() && matches!(patch.road.kind,RoadKind::Bridge|RoadKind::Elevated) {
                for offset in [-51,51] {
                    let plane=patch.v.map(|p|[p[0],p[1]+offset,p[2]]);
                    let mut crossing=Vec::new();
                    for i in 0..3 {
                        let (a,c)=(t[i],t[(i+1)%3]);
                        let (da,dc)=(floor_plane(&plane,a),floor_plane(&plane,c));
                        if da==0 {crossing.push(xy(a));}
                        if (da<0 && dc>0)||(da>0 && dc<0) {
                            let p=std::array::from_fn(|j|((a[j] as i128*(da-dc)+(c[j]-a[j]) as i128*da)/(da-dc)) as i64);
                            crossing.push(xy(p));
                        }
                    }
                    crossing.sort();crossing.dedup();
                    if crossing.len()>=2 {lines.push((crossing[0],*crossing.last().unwrap()));}
                }
            }
        }
    }
    let paint: Vec<_> = d
        .surface_areas
        .iter()
        .filter(|a| {
            let verts: Vec<_> = a.polygon.iter().map(|p| [p[0], 0, p[1]]).collect();
            hit(&verts, &bounds, 2)
        })
        .collect();
    for area in &paint {
        for i in 0..area.polygon.len() {
            lines.push((area.polygon[i], area.polygon[(i + 1) % area.polygon.len()]));
        }
    }
    let shapes = match crate::road_arrangement::subdivide(&bounds, &lines, work) {
        Ok(shapes) => shapes,
        Err(e) if e.code == "E_BUDGET"
            && matches!(e.message.as_str(), "ground tile arrangement edge limit" | "ground tile intersection limit")
            && depth < 6 && (0..2).all(|i| bounds.max[i]-bounds.min[i]>=4) => {
            let mid:Point=std::array::from_fn(|i|bounds.min[i]+(bounds.max[i]-bounds.min[i])/2);
            // No faces have been emitted yet. Children share exact integer cuts
            // and the same cumulative work/output limits as their parent cell.
            for y in 0..2 {for x in 0..2 {
                let part=Bounds {
                    min:[if x==0 {bounds.min[0]} else {mid[0]},if y==0 {bounds.min[1]} else {mid[1]}],
                    max:[if x==0 {mid[0]} else {bounds.max[0]},if y==0 {mid[1]} else {bounds.max[1]}],
                };
                ground_tile_part(d,v,part,patches,track,b,work,depth+1)?;
            }}
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let height = |p: Point| {
        let t = if (p[0] - v[0][0]) * (v[2][2] - v[0][2])
            >= (p[1] - v[0][2]) * (v[2][0] - v[0][0])
        {
            terrain[0]
        } else {
            terrain[1]
        };
        if let Some(patch) = nearby.iter().find(|patch| patch.road.design.as_ref().is_some_and(|d|d.terrain_policy==crate::road_design::TerrainPolicy::AutoFit)
            && point_in_polygon(p,&patch.v.map(xy))) {
            return designed_plane(&patch.v,[p[0],0,p[1]]);
        }
        let terrain=on_plane(&t,[p[0],0,p[1]]);
        for face in &nearby_track {
            if point_in_polygon(p,&face.map(xy)) {
                let deck=on_plane(face,[p[0],0,p[1]]);
                if (deck[1]-terrain[1]).abs()<=50 {return deck;}
            }
        }
        terrain
    };
    for rings in shapes {
        for triangle in crate::road_arrangement::triangulate(&rings, work)? {
            let center = [
                triangle.iter().map(|p| p[0]).sum::<i64>(),
                triangle.iter().map(|p| p[1]).sum::<i64>(),
            ];
            let t = if (center[0] - 3 * v[0][0]) * (v[2][2] - v[0][2])
                >= (center[1] - 3 * v[0][2]) * (v[2][0] - v[0][0])
            {
                terrain[0]
            } else {
                terrain[1]
            };
            let mut selected = Some((Surface::Grass, "terrain"));
            let mut road = false;
            let mut designed = None;
            for patch in &nearby {
                tick(work, 1)?;
                if !point_in_polygon(center, &patch.v.map(|p| [p[0] * 3, p[2] * 3])) {
                    continue;
                }
                match patch.road.kind {
                    RoadKind::Ground => {
                        if patch.road.design.is_some() {designed=Some(patch.v);}
                        selected = Some((patch.surface, patch.road.id.as_str()));
                        road = true;
                        break;
                    }
                    RoadKind::Underpass => {
                        selected = None;
                        break;
                    }
                    RoadKind::Tunnel => {
                        let ceiling = patch.v.map(|p| {
                            [
                                p[0] * 3,
                                (p[1] + patch.road.clearance_cm.unwrap() as i64) * 3,
                                p[2] * 3,
                            ]
                        });
                        let terrain3 = t.map(|p| p.map(|v| v * 3));
                        if floor_plane(&ceiling, on_plane(&terrain3, [center[0], 0, center[1]]))
                            <= 0
                        {
                            selected = None;
                            break;
                        }
                    }
                    RoadKind::Elevated | RoadKind::Bridge => {
                        let p=[center[0]/3,0,center[1]/3];
                        if patch.v.iter().all(|p| floor_plane(&t, *p) == 0)
                            || patch.road.design.is_some() && (designed_plane(&patch.v,p)[1]-on_plane(&t,p)[1]).abs()<=51 {
                            selected = None;
                            break;
                        }
                    }
                }
            }
            if !road && selected.is_some() {
                for area in &paint {
                    tick(work, area.polygon.len())?;
                    if point_in_polygon(
                        center,
                        &area
                            .polygon
                            .iter()
                            .map(|p| [p[0] * 3, p[1] * 3])
                            .collect::<Vec<_>>(),
                    ) {
                        selected = Some((area.surface, area.id.as_str()));
                        break;
                    }
                }
            }
            let center_point=[center[0]/3,0,center[1]/3];
            if nearby_track.iter().any(|face|point_in_polygon([center[0],center[1]],&face.map(|p|[p[0]*3,p[2]*3]))
                && (on_plane(face,center_point)[1]-on_plane(&t,center_point)[1]).abs()<=50) {
                selected=None;
            }
            if let Some((surface, id)) = selected {
                b.triangle(triangle.map(|p|designed.map_or_else(||height(p),|plane|designed_plane(&plane,[p[0],0,p[1]]))), surface, id, true)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn generate(
    d: &MapDocument,
    bounds: &Bounds,
    grid: Option<&HeightGrid>,
    spacing: i64,
    side: usize,
    b: &mut Builder,
) -> Result<()> {
    let (patches, walls) = plan(d, bounds)?;
    let (source_spacing,source_side)=(spacing,side);
    let spacing=if d.assembled_track.as_ref().is_some_and(|a|a.terrain_integration()) {spacing.min(200)} else {spacing};
    let side=d.cell_size_cm as usize/spacing as usize+1;
    let fitter=crate::road_design::Fitter::new(d,bounds);
    let mut track:Vec<_>=b.chunk.triangles.iter().filter(|t|t.spawnable && t.object_id.starts_with("assembled-road-") && orient(t.vertices[0],t.vertices[1],t.vertices[2])!=0).map(|t|t.vertices).collect();
    if let Some(a)=d.assembled_track.as_ref().filter(|a|a.terrain_integration()) {track.extend(crate::assembled_track::terrain_cut_faces(a,bounds)?);}
    let height = |x: usize, y: usize| fitter.height(
        [bounds.min[0]+x as i64*spacing,bounds.min[1]+y as i64*spacing],
        crate::terrain_height(bounds,source_spacing,source_side,grid,d.terrain_base_cm,[bounds.min[0]+x as i64*spacing,bounds.min[1]+y as i64*spacing]));
    let mut work = 0;
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let px = bounds.min[0] + x as i64 * spacing;
            let py = bounds.min[1] + y as i64 * spacing;
            let v = [
                [px, height(x, y), py],
                [px + spacing, height(x + 1, y), py],
                [px + spacing, height(x + 1, y + 1), py + spacing],
                [px, height(x, y + 1), py + spacing],
            ];

            ground_tile(d, v, &patches, &track, b, &mut work)?;
        }
    }
    for patch in &patches {
        if patch.road.kind == RoadKind::Ground {
            continue;
        }
        let mut face=patch.v;
        if orient(face[0],face[1],face[2])<0 {face.swap(1,2);}
        b.triangle(face, patch.surface, &patch.road.id, true)?;
        if patch.road.kind == RoadKind::Tunnel {
            b.triangle(
                patch
                    .v
                    .map(|p| [p[0], p[1] + patch.road.clearance_cm.unwrap() as i64, p[2]]),
                Surface::Concrete,
                &patch.road.id,
                false,
            )?;
        }
    }
    crate::road_safety::generate(d, &walls, b)?;
    for wall in walls.into_iter().filter(|w| matches!(w.road.kind, RoadKind::Tunnel | RoadKind::Underpass)) {
        if !hit(&[wall.a, wall.b], bounds, 0) {
            continue;
        }
        let h = wall.road.clearance_cm.unwrap() as i64;
        if wall.road.kind == RoadKind::Tunnel {
            b.quad(
                [
                    wall.a,
                    wall.b,
                    [wall.b[0], wall.b[1] + h, wall.b[2]],
                    [wall.a[0], wall.a[1] + h, wall.a[2]],
                ],
                Surface::Concrete,
                &wall.road.id,
                false,
            )?;
        } else {
            for y in 0..side - 1 {
                for x in 0..side - 1 {
                    tick(&mut work, 1)?;
                    let px = bounds.min[0] + x as i64 * spacing;
                    let py = bounds.min[1] + y as i64 * spacing;
                    if !hit(
                        &[wall.a, wall.b],
                        &Bounds {
                            min: [px, py],
                            max: [px + spacing, py + spacing],
                        },
                        0,
                    ) {
                        continue;
                    }
                    let v = [
                        [px, height(x, y), py],
                        [px + spacing, height(x + 1, y), py],
                        [px + spacing, height(x + 1, y + 1), py + spacing],
                        [px, height(x, y + 1), py + spacing],
                    ];
                    for terrain in [[v[0], v[1], v[2]], [v[0], v[2], v[3]]] {
                        let mut line = vec![wall.a, wall.b];
                        for i in 0..3 {
                            line = split(&line, |p| orient(terrain[i], terrain[(i + 1) % 3], p)).0;
                            line.dedup();
                        }
                        if line.len() < 2 {
                            continue;
                        }
                        let a = *line.first().unwrap();
                        let c = *line.last().unwrap();
                        if xy(a) == xy(c) {
                            continue;
                        }
                        let top = |p: Vertex| {
                            let terrain_height = on_plane(&terrain, p)[1];
                            [p[0], (p[1] + h).max(terrain_height), p[2]]
                        };
                        let roof = terrain.map(|p| [p[0], p[1] - h, p[2]]);
                        let (lo, hi) = split(&[a, c], |p| floor_plane(&roof, p));
                        for part in [lo, hi] {
                            if part.len() >= 2 {
                                let a = part[0];
                                let c = *part.last().unwrap();
                                if xy(a) != xy(c) {
                                    b.quad(
                                        [a, c, top(c), top(a)],
                                        Surface::Concrete,
                                        &wall.road.id,
                                        false,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Widened graph aprons cover corners; only restored ground receives
/// sidewalk tops. Road carriageways, independent decks and portals are untouched.
pub(crate) fn sidewalks(d: &MapDocument, b: &mut Builder) -> Result<()> {
    let expanded = Bounds {
        min: b.bounds.min.map(|v| v - 1000),
        max: b.bounds.max.map(|v| v + 1000),
    };
    let (_, boundary) = plan(d, &expanded)?;
    let mut patches = vec![];
    let mut buffers = BTreeMap::<&str, (&Road, Vec<Vec<Point>>)>::new();
    for edge in boundary {
        let width = crate::placement::sidewalk_width(d, edge.road) as i64;
        if width == 0 || !hit(&[edge.a,edge.b],&expanded,width) { continue; }
        let diagonal = libm::round(width as f64 / libm::sqrt(2.0)) as i64;
        let offsets = [[width,0],[diagonal,diagonal],[0,width],[-diagonal,diagonal],
            [-width,0],[-diagonal,-diagonal],[0,-width],[diagonal,-diagonal]];
        let ring=hull([edge.a,edge.b].iter().flat_map(|p|
            offsets.map(|o|[p[0]+o[0],0,p[2]+o[1]])).collect());
        buffers.entry(&edge.road.id).or_insert((edge.road,Vec::new())).1.push(ring.into_iter().map(xy).collect());
    }
    // Dilate the shared exterior, then union before subdividing terrain. An
    // internal triangle fan must not multiply identical sidewalk work.
    use i_overlay::{core::{fill_rule::FillRule,overlay::{Overlay,ShapeType},overlay_rule::OverlayRule},i_float::int::point::IntPoint};
    let mut buffer_work=0;
    for (_, (road,rings)) in buffers {
        let mut overlay=Overlay::<i64>::new(rings.iter().map(Vec::len).sum());
        for ring in rings {
            tick(&mut buffer_work,ring.len())?;
            overlay.add_contour(&ring.into_iter().map(|p|IntPoint::new(p[0],p[1])).collect::<Vec<_>>(),ShapeType::Subject);
        }
        for shape in overlay.overlay(OverlayRule::Subject,FillRule::NonZero) {
            let rings:Vec<Vec<Point>>=shape.into_iter().map(|r|r.into_iter().map(|p|[p.x,p.y]).collect()).collect();
            patches.push((road,rings));
        }
    }
    if patches.is_empty() {
        return Ok(());
    }
    let patch_index = crate::bounds_index::BoundsIndex::new(
        &patches.iter().map(|(_,rings)| crate::bounds_index::bounds(&rings[0])).collect::<Vec<_>>(),
    );
    let exclusions: Vec<_> = d
        .buildings
        .iter()
        .flat_map(|b| std::iter::once(&b.footprint).chain(&b.entrances))
        .collect();
    let exclusion_index = crate::bounds_index::BoundsIndex::new(
        &exclusions
            .iter()
            .map(|p| crate::bounds_index::bounds(p))
            .collect::<Vec<_>>(),
    );
    let ground: Vec<_> = b
        .chunk
        .triangles
        .iter()
        .filter(|t| t.object_id == "terrain" || d.surface_areas.iter().any(|a| a.id == t.object_id))
        .cloned()
        .collect();
    let mut work = 0;
    let mut edges = Vec::new();
    let mut tops = Vec::new();
    for t in ground {
        let area = crate::bounds_index::bounds(&t.vertices.map(xy));
        let nearby = exclusion_index.query(&area, &mut work)?;
        let mut remaining = vec![vec![t.vertices.map(xy).to_vec()]];
        for i in patch_index.query(&area, &mut work)? {
            let (road,rings) = &patches[i];
            // Clip the complete apron before triangulating. Triangulating the
            // entire curved buffer first creates long fan diagonals through
            // every terrain fragment and needlessly multiplies solid faces.
            let mut fitted = sidewalk_overlay(&remaining,rings,OverlayRule::Intersect,&mut work)?;
            remaining = sidewalk_overlay(&remaining,rings,OverlayRule::Difference,&mut work)?;
            for &i in &nearby {
                fitted = sidewalk_overlay(&fitted,&[exclusions[i].clone()],OverlayRule::Difference,&mut work)?;
            }
            let id = format!("{}:sidewalk", road.id);
            let bottom=|p:Point| on_plane(&t.vertices,[p[0],0,p[1]]);
            for rings in fitted {
                for triangle in crate::road_arrangement::triangulate(&rings,&mut work)? {
                    let top=triangle.map(|p| {let p=bottom(p);[p[0],p[1]+12,p[2]]});
                    emit(b,&top,Surface::Concrete,&id,true)?;
                    tops.push(triangle.to_vec());
                }
                for ring in rings {for i in 0..ring.len() {
                    edges.push((bottom(ring[i]),bottom(ring[(i+1)%ring.len()]),road.id.as_str()));
                }}
            }
            if remaining.len() > MAX_FRAGMENTS {
                return Err(error("E_BUDGET", "sidewalk fragments exceeded"));
            }
        }
    }
    sidewalk_boundary_walls(b, &edges, &tops, &mut work).map_err(|mut e| {
        e.message=format!("boundary of {} tops/{} edges, work {work}: {}",tops.len(),edges.len(),e.message);e
    })?;
    Ok(())
}

fn sidewalk_overlay(subjects:&[Vec<Vec<Point>>],clip:&[Vec<Point>],rule:i_overlay::core::overlay_rule::OverlayRule,work:&mut usize)->Result<Vec<Vec<Vec<Point>>>> {
    use i_overlay::{core::{fill_rule::FillRule,overlay::{Overlay,ShapeType}},i_float::int::point::IntPoint};
    if subjects.is_empty(){return Ok(vec![]);}
    let count=subjects.iter().flat_map(|s|s.iter()).chain(clip).map(Vec::len).sum();
    tick(work,count)?;
    let mut overlay=Overlay::<i64>::new(count);
    for (rings,kind) in subjects.iter().map(|s|(s.as_slice(),ShapeType::Subject)).chain(std::iter::once((clip,ShapeType::Clip))) {
        for ring in rings {overlay.add_contour(&ring.iter().map(|p|IntPoint::new(p[0],p[1])).collect::<Vec<_>>(),kind);}
    }
    let out:Vec<Vec<Vec<Point>>>=overlay.overlay(rule,FillRule::NonZero).into_iter().map(|s|s.into_iter().map(|r|r.into_iter().map(|p|[p.x,p.y]).collect()).collect()).collect();
    if out.len()>MAX_FRAGMENTS{return Err(error("E_BUDGET","sidewalk fragments exceeded"));}
    Ok(out)
}

/// Clip fragment edges against one another before making the visible step.
/// Terrain triangles and apron patches split the top into many polygons; a
/// wall on each polygon edge would leave solid vertical faces inside the top.
fn sidewalk_boundary_walls(
    b: &mut Builder,
    edges: &[(Vertex, Vertex, &str)],
    tops: &[Vec<Point>],
    work: &mut usize,
) -> Result<()> {
    use i_overlay::{
        core::{
            fill_rule::FillRule,
            overlay::{Overlay, ShapeType},
            overlay_rule::OverlayRule,
        },
        i_float::int::point::IntPoint,
    };
    let mut overlay = Overlay::<i64>::new(edges.len());
    for top in tops {
        tick(work, top.len())?;
        let mut contour = top.clone();
        let area: i128 = (0..contour.len())
            .map(|i| {
                let (a, c) = (contour[i], contour[(i + 1) % contour.len()]);
                a[0] as i128 * c[1] as i128 - a[1] as i128 * c[0] as i128
            })
            .sum();
        if area < 0 {
            contour.reverse();
        }
        overlay.add_contour(
            &contour
                .iter()
                .map(|p| IntPoint::new(p[0], p[1]))
                .collect::<Vec<_>>(),
            ShapeType::Subject,
        );
    }
    let outlines = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
    let index = crate::bounds_index::BoundsIndex::new(
        &edges
            .iter()
            .map(|(a, c, _)| crate::bounds_index::bounds(&[xy(*a), xy(*c)]))
            .collect::<Vec<_>>(),
    );
    let top_index = crate::bounds_index::BoundsIndex::new(
        &tops
            .iter()
            .map(|poly| crate::bounds_index::bounds(poly))
            .collect::<Vec<_>>(),
    );
    for shape in outlines {
        for ring in shape {
            for i in 0..ring.len() {
                let p = [ring[i].x, ring[i].y];
                let q = [ring[(i + 1) % ring.len()].x, ring[(i + 1) % ring.len()].y];
                if p == q
                    || p[0] == q[0] && (p[0] == b.bounds.min[0] || p[0] == b.bounds.max[0])
                    || p[1] == q[1] && (p[1] == b.bounds.min[1] || p[1] == b.bounds.max[1])
                {
                    continue;
                }
                let coordinate = if p[0] != q[0] { 0 } else { 1 };
                let mut intervals = Vec::new();
                let mut area = crate::bounds_index::bounds(&[p, q]);
                for axis in 0..2 {
                    area.min[axis] -= 2;
                    area.max[axis] += 2;
                }
                for candidate in index.query(&area, work)? {
                    tick(work, 1)?;
                    let (a, c, _) = edges[candidate];
                    let tolerance = 2 * (q[0] - p[0]).abs().max((q[1] - p[1]).abs()) as i128;
                    if cross(p, q, xy(a)).abs() > tolerance
                        || cross(p, q, xy(c)).abs() > tolerance
                        || a[coordinate * 2] == c[coordinate * 2]
                    {
                        continue;
                    }
                    let lo = p[coordinate]
                        .min(q[coordinate])
                        .max(a[coordinate * 2].min(c[coordinate * 2]) - 2);
                    let hi = p[coordinate]
                        .max(q[coordinate])
                        .min(a[coordinate * 2].max(c[coordinate * 2]) + 2);
                    if lo < hi {
                        intervals.push((lo, hi, candidate));
                    }
                }
                intervals.sort_unstable();
                let end = p[coordinate].max(q[coordinate]);
                let mut at = p[coordinate].min(q[coordinate]);
                while at < end {
                    tick(work, 1)?;
                    let covering = intervals
                        .iter()
                        .filter(|(lo, hi, _)| *lo <= at && *hi > at)
                        .max_by_key(|(_, hi, _)| *hi);
                    let (hi, candidate) = if let Some(&(_, hi, candidate)) = covering {
                        (hi, candidate)
                    } else {
                        // Integer overlay intersections can round an outline
                        // endpoint a centimetre off its source edge.
                        let next = intervals
                            .iter()
                            .filter(|(lo, _, _)| *lo > at)
                            .map(|(lo, _, _)| *lo)
                            .min()
                            .unwrap_or(end);
                        let midpoint = std::array::from_fn::<_, 2, _>(|j| {
                            p[j] + (((q[j] - p[j]) as i128
                                * ((at + next) / 2 - p[coordinate]) as i128)
                                / (q[coordinate] - p[coordinate]) as i128)
                                as i64
                        });
                        let nearby = Bounds {
                            min: midpoint.map(|v| v - 25),
                            max: midpoint.map(|v| v + 25),
                        };
                        let candidate = index
                            .query(&nearby, work)?
                            .into_iter()
                            .min_by_key(|&index| {
                                let (a, c, _) = edges[index];
                                let (dx, dz) = (c[0] - a[0], c[2] - a[2]);
                                let length = dx as i128 * dx as i128 + dz as i128 * dz as i128;
                                if length == 0 {
                                    return i128::MAX;
                                }
                                let dot = (midpoint[0] - a[0]) as i128 * dx as i128
                                    + (midpoint[1] - a[2]) as i128 * dz as i128;
                                let squared = if dot < 0 {
                                    (midpoint[0] - a[0]) as i128 * (midpoint[0] - a[0]) as i128
                                        + (midpoint[1] - a[2]) as i128
                                            * (midpoint[1] - a[2]) as i128
                                } else if dot > length {
                                    (midpoint[0] - c[0]) as i128 * (midpoint[0] - c[0]) as i128
                                        + (midpoint[1] - c[2]) as i128
                                            * (midpoint[1] - c[2]) as i128
                                } else {
                                    let area = cross(xy(a), xy(c), midpoint);
                                    area * area / length
                                };
                                squared
                            })
                            .ok_or_else(|| {
                                error("E_GEOMETRY", "sidewalk boundary has no nearby source")
                            })?;
                        (next, candidate)
                    };
                    let (a, c, id) = edges[candidate];
                    let point = |value: i64| -> Vertex {
                        let xy = std::array::from_fn::<_, 2, _>(|j| {
                            p[j] + (((q[j] - p[j]) as i128 * (value - p[coordinate]) as i128)
                                / (q[coordinate] - p[coordinate]) as i128)
                                as i64
                        });
                        let dx = (c[0] - a[0]) as i128;
                        let dz = (c[2] - a[2]) as i128;
                        let dot = (xy[0] - a[0]) as i128 * dx + (xy[1] - a[2]) as i128 * dz;
                        [
                            xy[0],
                            a[1] + (((c[1] - a[1]) as i128 * dot) / (dx * dx + dz * dz)) as i64,
                            xy[1],
                        ]
                    };
                    let (first, last) = if p[coordinate] < q[coordinate] {
                        (point(at), point(hi))
                    } else {
                        (point(hi), point(at))
                    };
                    let middle = [(first[0] + last[0]) / 2, (first[2] + last[2]) / 2];
                    let dx = (last[0] - first[0]).signum();
                    let dz = (last[2] - first[2]).signum();
                    let inside_left = sidewalk_contains(tops, &top_index, [middle[0] - dz, middle[1] + dx], work)?;
                    if inside_left && sidewalk_contains(
                            tops,
                            &top_index,
                            [middle[0] + dz, middle[1] - dx],
                            work,
                        )?
                    {
                        at = hi;
                        continue;
                    }
                    // Generated triangles use the map's downward top winding.
                    // Keep the sidewalk on the left, so the reflected Godot
                    // clockwise face and normal point out of the exposed wall.
                    let (first, last) = if inside_left { (first, last) } else { (last, first) };
                    b.quad(
                        [
                            first,
                            last,
                            [last[0], last[1] + 12, last[2]],
                            [first[0], first[1] + 12, first[2]],
                        ],
                        Surface::Concrete,
                        &format!("{id}:sidewalk"),
                        false,
                    )?;
                    at = hi;
                }
            }
        }
    }
    Ok(())
}

fn sidewalk_contains(
    tops: &[Vec<Point>],
    index: &crate::bounds_index::BoundsIndex,
    point: Point,
    work: &mut usize,
) -> Result<bool> {
    let bounds = Bounds {
        min: point,
        max: point,
    };
    for candidate in index.query(&bounds, work)? {
        tick(work, 1)?;
        let poly = &tops[candidate];
        let mut inside = false;
        for i in 0..poly.len() {
            let a = poly[i];
            let c = poly[(i + 1) % poly.len()];
            let side = cross(a, c, point);
            if side == 0
                && (0..2).all(|axis| {
                    point[axis] >= a[axis].min(c[axis]) && point[axis] <= a[axis].max(c[axis])
                })
            {
                return Ok(true);
            }
            if (a[1] > point[1]) != (c[1] > point[1]) && (side > 0) == (c[1] > a[1]) {
                inside = !inside;
            }
        }
        if inside {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod arrangement_probe {
    use super::*;

    #[test]
    fn simultaneous_integer_cuts_preserve_curved_cell_area() {
        for shift in [[0, 0], [-837, -851], [999_990_000, -999_990_000]] {
            for widths in [
                [601, 799, 503],
                [599, 801, 501],
                [101, 99, 103],
                [997, 503, 799],
            ] {
                check_case(shift, widths);
            }
        }
    }

    fn check_case(shift: Point, widths: [u32; 3]) {
        let mut d: MapDocument =
            serde_json::from_str(include_str!("../../../examples/roads/document.json")).unwrap();
        d.roads.retain(|r| r.id == "ground-west");
        d.nodes
            .retain(|n| ["ground-west-from", "junction"].contains(&n.id.as_str()));
        d.roads[0].points = vec![
            [0, 0, 1000],
            [1801, 0, 1397],
            [3203, 0, 701],
            [5000, 0, 1000],
        ];
        d.roads[0].widths_cm = widths.to_vec();
        d.roads[0].surfaces = vec![Surface::Asphalt; 3];
        let bounds = d.cell_bounds(Cell { x: 0, y: 0 }).unwrap();
        let (patches, _) = plan(&d, &bounds).unwrap();
        let point = |p: Point| [p[0] + shift[0], p[1] + shift[1]];
        let mut work = 0;
        let mut shapes = Vec::new();
        for y in (0..5000).step_by(500) {
            for x in (0..5000).step_by(500) {
                let tile = Bounds {
                    min: [x, y],
                    max: [x + 500, y + 500],
                };
                let mut lines = vec![(point([x, y]), point([x + 500, y + 500]))];
                for patch in patches.iter().filter(|p| hit(&p.v, &tile, 2)) {
                    for i in 0..3 {
                        lines.push((point(xy(patch.v[i])), point(xy(patch.v[(i + 1) % 3]))));
                    }
                }
                let shifted = Bounds {
                    min: point(tile.min),
                    max: point(tile.max),
                };
                shapes.extend(
                    crate::road_arrangement::subdivide(&shifted, &lines, &mut work).unwrap(),
                );
            }
        }
        let mut area = 0i128;
        let mut triangles = Vec::new();
        for shape in &shapes {
            assert_eq!(shape.len(), 1, "probe faces are simply connected");
            let ring = shape[0].clone();
            for t in crate::generation::polygon_triangles(&ring).unwrap() {
                triangles.push(t.map(|i| ring[i]));
            }
            for (index, ring) in shape.iter().enumerate() {
                let a: i128 = (0..ring.len())
                    .map(|i| {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
                    })
                    .sum();
                area += if index == 0 { a.abs() } else { -a.abs() };
            }
        }
        assert_eq!(area, 2 * 5000 * 5000);
        assert_eq!(
            triangles
                .iter()
                .map(|t| cross(t[0], t[1], t[2]).abs())
                .sum::<i128>(),
            area
        );
        let mut edges = BTreeMap::new();
        for triangle in &triangles {
            for i in 0..3 {
                let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
                *edges
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_insert(0) += 1;
            }
        }
        for ((a, b), count) in edges {
            let outer = (0..2).any(|i| a[i] == b[i] && [shift[i], shift[i] + 5000].contains(&a[i]));
            assert_eq!(
                count,
                if outer { 1 } else { 2 },
                "unmatched internal edge {a:?}..{b:?}"
            );
        }
        eprintln!(
            "simultaneous arrangement: {} faces; exact area {}",
            shapes.len(),
            area
        );
    }
}
