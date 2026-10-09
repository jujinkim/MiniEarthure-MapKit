//! Optional bounded static preview. Integrity-bound, excluded from driving identity.
use super::*;
pub const PATH: &str = "preview.png";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const MAX_SIDE: u32 = 512;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    #[schemars(regex(pattern = r"^preview\.png$"))]
    pub path: String,
    #[schemars(regex(pattern = "^image/png$"))]
    pub media_type: String,
    #[schemars(range(min = 1, max = 512))]
    pub width: u32,
    #[schemars(range(min = 1, max = 512))]
    pub height: u32,
}
pub fn inspect(bytes: &[u8]) -> Result<Preview> {
    if bytes.len() as u64 > MAX_BYTES { return Err(error("E_LIMIT", "preview exceeds 1 MiB")); }
    assets::png_envelope(bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 4 * 1024 * 1024 });
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info().map_err(io)?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE || reader.output_buffer_size() > 2 * 1024 * 1024 {
        return Err(error("E_LIMIT", "preview dimensions exceed 512 pixels"));
    }
    let mut output = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut output).map_err(io)?;
    reader.finish().map_err(io)?;
    Ok(Preview { path: PATH.into(), media_type: "image/png".into(), width, height })
}
pub(crate) fn from_files(files: &BTreeMap<String, Vec<u8>>) -> Result<Option<Preview>> {
    files.get(PATH).map(|bytes| inspect(bytes)).transpose()
}
/// Deterministic map-only thumbnail: roads and selected gates, never player UI/data.
pub fn overview(package: &Package, course: &mapkit_core::course::Course) -> Result<Vec<u8>> {
    const SIDE: usize = 384;
    let mut pixels = vec![0u8; SIDE * SIDE * 3];
    for p in pixels.chunks_exact_mut(3) { p.copy_from_slice(&[22, 33, 45]); }
    let bounds = &package.document.bounds;
    let span = ((bounds.max[0]-bounds.min[0]).max(bounds.max[1]-bounds.min[1]) as f64).max(1.0);
    let point = |v: [i64; 3]| -> (i32,i32) {
        let x = (v[0] as f64 - (bounds.min[0] as f64 + bounds.max[0] as f64)*0.5)/span;
        let y = (v[2] as f64 - (bounds.min[1] as f64 + bounds.max[1] as f64)*0.5)/span;
        ((SIDE as f64*0.5+x*340.0).round().clamp(-768.0,1152.0) as i32,
         (SIDE as f64*0.5+y*340.0).round().clamp(-768.0,1152.0) as i32)
    };
    let mut dot = |x: i32, y: i32, radius: i32, color: [u8;3]| {
        for dy in -radius..=radius { for dx in -radius..=radius {
            let (x,y)=(x+dx,y+dy);
            if (0..SIDE as i32).contains(&x) && (0..SIDE as i32).contains(&y) {
                let i=(y as usize*SIDE+x as usize)*3; pixels[i..i+3].copy_from_slice(&color);
            }
        }}
    };
    let overview = mapkit_core::overview(&package.document)?;
    for road in overview.roads {
        mapkit_core::cancellation::checkpoint()?;
        for pair in road.points.windows(2) {
            let (a,b)=(point(pair[0]),point(pair[1]));
            let steps=(a.0-b.0).abs().max((a.1-b.1).abs()).max(1).min(768);
            for i in 0..=steps {
                dot(a.0+(b.0-a.0)*i/steps,a.1+(b.1-a.1)*i/steps,1,[136,191,205]);
            }
        }
    }
    for (i,gate) in course.definition.checkpoints.iter().enumerate() {
        let (x,y)=point(gate.position_cm);
        dot(x,y,3,if i==0 {[255,203,79]} else {[243,247,249]});
    }
    let mut bytes=Vec::new();
    { let mut encoder=png::Encoder::new(&mut bytes,SIDE as u32,SIDE as u32);
      encoder.set_color(png::ColorType::Rgb); encoder.set_depth(png::BitDepth::Eight);
      encoder.write_header().map_err(io)?.write_image_data(&pixels).map_err(io)?; }
    inspect(&bytes)?;
    Ok(bytes)
}
