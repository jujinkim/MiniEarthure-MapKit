//! Seeded races on an existing road map: one seed picks the start road, travel
//! direction, gimmicks, shortcuts and the initial time/weather. Read-only, like
//! `suggest`; consumers adopt the course, extras and environment together.
use super::*;

/// Start roads tried in seeded order before the best non-blocking route is kept.
const START_TRIES: usize = 12;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SeedRequest {
    pub seed: u64,
    pub mode: Mode,
    pub duration_seconds: u16,
    #[serde(default)]
    pub min_width_cm: u32,
    #[serde(default)]
    pub gimmicks: u8,
    #[serde(default)]
    pub shortcuts: u8,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SeededRace {
    pub seed: u64,
    /// The resolved start the suggestion used.
    pub start_cm: Point,
    pub heading: [i64; 2],
    pub suggestion: Suggestion,
    /// Seed play environment (`track_seed_defaults`); never written to the map.
    pub time_minutes: u16,
    pub weather: String,
}

/// Each try starts mid-road on a seeded road in a seeded direction. A route within
/// the length tolerance wins; otherwise the first applicable one; otherwise the
/// last try's diagnostics are returned. Extras are proposed for the chosen start only.
pub fn seed(d: &MapDocument, request: &SeedRequest) -> Result<SeededRace> {
    let environment = crate::assembled_track::track_seed_defaults(request.seed)
        .map_err(|e| error("E_ROAD_COURSE_REQUEST", e.message))?;
    let mut roads: Vec<(String, &Road)> = d.roads.iter()
        .filter(|r| r.points.len() >= 2 && r.from != r.to && r.widths_cm.iter().all(|&w| w >= request.min_width_cm))
        .map(|r| (crate::sha256(format!("{}:start:{}", request.seed, r.id).as_bytes()), r)).collect();
    roads.sort_by(|a, b| a.0.cmp(&b.0));
    let base = Request { start_cm: [0, 0], heading: [0, 0], mode: request.mode.clone(), duration_seconds: request.duration_seconds,
        min_width_cm: request.min_width_cm, gimmicks: 0, seed: request.seed, shortcuts: 0 };
    let mut fallback: Option<(Request, Suggestion)> = None;
    let mut chosen: Option<Request> = None;
    for (hash, road) in roads.iter().take(START_TRIES) {
        cancellation::checkpoint()?;
        let (middle, direction) = midpoint(&road.points);
        let sign = if u8::from_str_radix(&hash[..2], 16).unwrap() % 2 == 0 { 1.0 } else { -1.0 };
        let mut attempt = base.clone();
        attempt.start_cm = [middle[0], middle[2]];
        attempt.heading = [libm::round(direction[0] * sign * 1000.0) as i64, libm::round(direction[1] * sign * 1000.0) as i64];
        if !d.bounds.contains(attempt.start_cm) { continue; }
        let result = suggest(d, &attempt)?;
        if !result.applicable { if fallback.is_none() { fallback = Some((attempt, result)); } continue; }
        if !result.diagnostics.iter().any(|x| x.code == "W_ROAD_COURSE_LENGTH") { chosen = Some(attempt); break; }
        if fallback.as_ref().is_none_or(|f| !f.1.applicable) { fallback = Some((attempt, result)); }
    }
    let (mut resolved, empty) = match (chosen, fallback) {
        (Some(r), _) => (r, None),
        (None, Some((r, s))) if s.applicable => (r, None),
        (None, Some((r, s))) => (r, Some(s)),
        (None, None) => return Err(error("E_ROAD_COURSE_START", "the map has no drivable road to start a race")),
    };
    resolved.gimmicks = request.gimmicks;
    resolved.shortcuts = request.shortcuts;
    let suggestion = match empty { Some(s) => s, None => suggest(d, &resolved)? };
    Ok(SeededRace { seed: request.seed, start_cm: resolved.start_cm, heading: resolved.heading, suggestion,
        time_minutes: environment["time_minutes"].as_u64().unwrap() as u16, weather: environment["weather"].as_str().unwrap().into() })
}

/// Horizontal middle of a polyline and its unit travel direction there.
fn midpoint(points: &[Vertex]) -> (Vertex, [f64; 2]) {
    let mut left = polyline(points) / 2.0;
    for w in points.windows(2) {
        let l = len3(w[0], w[1]);
        let [dx, dz] = h(w[0], w[1]);
        let n = dx.hypot(dz);
        if l >= left && n > 0.0 {
            let f = if l > 0.0 { left / l } else { 0.0 };
            return (std::array::from_fn(|k| w[0][k] + libm::round((w[1][k] - w[0][k]) as f64 * f) as i64), [dx / n, dz / n]);
        }
        left -= l;
    }
    (points[0], [1.0, 0.0])
}
