//! Borrowed read-only map selection data; never clones editor geometry or metadata.
use super::*;

#[derive(Debug, Serialize)]
pub struct OverviewRoad<'a> {
    pub id: std::borrow::Cow<'a,str>,
    pub kind: &'a RoadKind,
    /// Authored connectivity; spatial crossings do not imply a junction.
    pub from: &'a str,
    pub to: &'a str,
    pub widths_cm: std::borrow::Cow<'a,[u32]>,
    pub points: std::borrow::Cow<'a,[Vertex]>,
}
#[derive(Debug, Serialize)]
pub struct OverviewBuilding<'a> {
    pub id: &'a str,
    pub footprint: &'a [Point],
    #[serde(skip_serializing_if = "<[Vec<Point>]>::is_empty")]
    pub holes: &'a [Vec<Point>],
}
#[derive(Debug, Serialize)]
pub struct OverviewSource<'a> {
    pub source: &'a str,
    pub license: &'a str,
}
#[derive(Debug, Serialize)]
pub struct MapOverview<'a> {
    pub pois: Vec<&'a poi::PointOfInterest>,
    pub version: u32,
    pub map_id: &'a str,
    pub bounds: &'a Bounds,
    pub roads: Vec<OverviewRoad<'a>>,
    pub buildings: Vec<OverviewBuilding<'a>>,
    pub attributions: Vec<OverviewSource<'a>>,
    pub has_custom_assets: bool,
}
/// Counts for representation-specific allocation policy, not RSS estimates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewCost {
    pub json_bytes: u64,
    pub points_3d: u64,
    pub points_2d: u64,
    pub records: u64,
    pub text_bytes: u64,
}
struct Count(u64);
impl std::io::Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("overview size overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl MapOverview<'_> {
    pub fn cost(&self) -> Result<OverviewCost> {
        let mut count = Count(0);
        // Count escaped JSON exactly without retaining a serialized copy.
        serde_json::to_writer(&mut count, self).map_err(|e| error("E_JSON", e.to_string()))?;
        Ok(OverviewCost {
            json_bytes: count.0,
            points_3d: self.roads.iter().map(|r| r.points.len() as u64).sum(),
            points_2d: self.pois.len() as u64 + self
                .buildings
                .iter()
                .map(|b| (b.footprint.len() + b.holes.iter().map(Vec::len).sum::<usize>()) as u64)
                .sum::<u64>(),
            records: (self.pois.len() + self.roads.len()
                + self.buildings.len()
                + self.attributions.len()
                + self.buildings.iter().map(|b| b.holes.len()).sum::<usize>())
                as u64,
            text_bytes: self.pois.iter().map(|p| (p.id.len() + p.name.len() + p.category.len() + p.source.source.len() + p.source.license.len() + p.source.notice.len()) as u64).sum::<u64>() + self.map_id.len() as u64
                + self
                    .roads
                    .iter()
                    .map(|r| (r.id.len() + r.from.len() + r.to.len()) as u64)
                    .sum::<u64>()
                + self
                    .buildings
                    .iter()
                    .map(|b| b.id.len() as u64)
                    .sum::<u64>()
                + self
                    .attributions
                    .iter()
                    .map(|a| (a.source.len() + a.license.len()) as u64)
                    .sum::<u64>(),
        })
    }
    /// The budget is for the JSON body, excluding an adapter's response envelope.
    pub fn to_json(&self, max_bytes: u64) -> Result<String> {
        if self.cost()?.json_bytes > max_bytes {
            return Err(error("E_MEMORY_BUDGET", "overview JSON exceeds allowance"));
        }
        serde_json::to_string(self).map_err(|e| error("E_JSON", e.to_string()))
    }
}
pub fn overview(d: &MapDocument) -> Result<MapOverview<'_>> {
    d.validate()?;
    overview_validated(d)
}

/// Opt-in indexed source topology. Bounded overview output still has its own cap.
pub fn source_overview(d: &MapDocument) -> Result<MapOverview<'_>> {
    d.validate_source()?;
    overview_validated(d)
}

fn overview_validated(d: &MapDocument) -> Result<MapOverview<'_>> {
    let mut roads: Vec<_> = d
        .roads
        .iter()
        .map(|r| OverviewRoad {
            id: std::borrow::Cow::Borrowed(&r.id),
            kind: &r.kind,
            from: &r.from,
            to: &r.to,
            widths_cm: r.widths_cm.as_slice().into(),
            points: std::borrow::Cow::Borrowed(&r.points),
        })
        .collect();
    let mut buildings: Vec<_> = d
        .buildings
        .iter()
        .map(|b| OverviewBuilding {
            id: &b.id,
            footprint: &b.footprint,
            holes: &b.holes,
        })
        .collect();
    let mut attributions: Vec<_> = d
        .attributions
        .iter()
        .map(|a| OverviewSource {
            source: &a.source,
            license: &a.license,
        })
        .collect();
    if let Some(track)=&d.assembled_track {
        for (index,p) in track.pieces.iter().enumerate() {
            for (suffix,path) in [("",&p.path),("-bridge",&p.alternate_path)] {
                if path.is_empty() { continue; }
                roads.push(OverviewRoad{id:format!("assembled-road-{index}{suffix}").into(),kind:&RoadKind::Elevated,from:"",to:"",
                    widths_cm:path.iter().map(|s|s.lateral_cm*2).collect::<Vec<_>>().into(),
                    points:path.iter().map(|s|s.position_cm).collect::<Vec<_>>().into()});
            }
        }
    }
    roads.sort_by(|a,b| a.id.cmp(&b.id));
    buildings.sort_by_key(|b| b.id);
    attributions.sort_by_key(|a| (a.source, a.license));
    Ok(MapOverview {
        pois: { let mut pois: Vec<_> = d.pois.iter().collect(); pois.sort_by_key(|p| &p.id); pois },
        version: 1,
        map_id: &d.map_id,
        bounds: &d.bounds,
        roads,
        buildings,
        attributions,
        has_custom_assets: !d.assets.is_empty(),
    })
}
