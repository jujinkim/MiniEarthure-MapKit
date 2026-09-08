//! Static, embedded asset admission. Decoding is validation only: no GPU, URLs or scripts.
use super::*;
use gltf::accessor::{DataType, Dimensions};

const IMAGE_BYTES: usize = 64 * 1024 * 1024;
const ELEMENTS: usize = 1_000_000;
struct DecodeBudget {
    remaining: usize,
}
impl DecodeBudget {
    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| limit("package decoded image work exceeds 256 MiB"))?;
        Ok(())
    }
}
fn bad(message: impl Into<String>) -> Error {
    error("E_ASSET", message)
}
fn limit(message: &str) -> Error {
    error("E_LIMIT", message)
}
fn le(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(p..p.checked_add(4).ok_or_else(|| bad("offset overflow"))?)
            .ok_or_else(|| bad("truncated resource"))?
            .try_into()
            .unwrap(),
    ))
}

pub(super) fn png_envelope(bytes: &[u8]) -> Result<()> {
    if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err(bad("invalid PNG signature"));
    }
    let mut p = 8;
    while p < bytes.len() {
        let n = u32::from_be_bytes(
            bytes
                .get(p..p + 4)
                .ok_or_else(|| bad("truncated PNG"))?
                .try_into()
                .unwrap(),
        ) as usize;
        let end = p
            .checked_add(12)
            .and_then(|v| v.checked_add(n))
            .ok_or_else(|| bad("PNG size overflow"))?;
        let chunk = bytes
            .get(p..end)
            .ok_or_else(|| bad("truncated PNG chunk"))?;
        if crc32fast::hash(&chunk[4..8 + n])
            != u32::from_be_bytes(chunk[8 + n..].try_into().unwrap())
        {
            return Err(bad("PNG chunk CRC mismatch"));
        }
        match &chunk[4..8] {
            b"acTL" | b"fcTL" | b"fdAT" => return Err(bad("animated PNG is unsupported")),
            b"IEND" => {
                return if n == 0 && end == bytes.len() {
                    Ok(())
                } else {
                    Err(bad("PNG has trailing bytes"))
                }
            }
            _ => (),
        }
        p = end;
    }
    Err(bad("PNG lacks IEND"))
}
fn validate_png(bytes: &[u8], budget: &mut DecodeBudget) -> Result<()> {
    png_envelope(bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: IMAGE_BYTES });
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info().map_err(|e| bad(e.to_string()))?;
    let info = reader.info();
    if info.width > 8192 || info.height > 8192 || reader.output_buffer_size() > IMAGE_BYTES {
        return Err(limit("image dimensions/decoded bytes exceed profile"));
    }
    budget.charge(reader.output_buffer_size())?;
    let mut data = vec![0; reader.output_buffer_size()];
    reader
        .next_frame(&mut data)
        .map_err(|e| bad(e.to_string()))?;
    reader.finish().map_err(|e| bad(e.to_string()))
}
fn validate_webp(bytes: &[u8], budget: &mut DecodeBudget) -> Result<()> {
    if bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WEBP")
        || le(bytes, 4)? as u64 + 8 != bytes.len() as u64
    {
        return Err(bad("invalid WebP RIFF length/signature"));
    }
    let mut p = 12;
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut extended_flags = None;
    while p < bytes.len() {
        let kind = bytes
            .get(p..p + 4)
            .ok_or_else(|| bad("truncated WebP chunk"))?;
        let n = le(bytes, p + 4)? as usize;
        let end = p
            .checked_add(8)
            .and_then(|v| v.checked_add(n))
            .ok_or_else(|| bad("WebP length overflow"))?;
        let chunk = bytes
            .get(p + 8..end)
            .ok_or_else(|| bad("truncated WebP payload"))?;
        if !seen.insert(kind)
            || !matches!(
                kind,
                b"VP8X" | b"VP8 " | b"VP8L" | b"ALPH" | b"ICCP" | b"EXIF" | b"XMP "
            )
        {
            return Err(bad("unsupported/duplicate WebP chunk"));
        }
        order.push(kind);
        if kind == b"VP8X"
            && (p != 12 || n != 10 || chunk[0] & 0xc3 != 0 || chunk[1..4] != [0, 0, 0])
        {
            return Err(bad("invalid or animated WebP extended header"));
        }
        if kind == b"VP8X" {
            extended_flags = Some(chunk[0]);
        }
        if n % 2 == 1 && bytes.get(end) != Some(&0) {
            return Err(bad("invalid WebP padding"));
        }
        p = end + n % 2;
    }
    if seen.contains(b"VP8 ".as_slice()) == seen.contains(b"VP8L".as_slice()) {
        return Err(bad("WebP requires one static image"));
    }
    if let Some(flags) = extended_flags {
        for (kind, mask) in [(b"ICCP", 0x20), (b"EXIF", 8), (b"XMP ", 4)] {
            if seen.contains(kind.as_slice()) != (flags & mask != 0) {
                return Err(bad("WebP feature flags disagree with chunks"));
            }
        }
        let rank = |kind: &[u8]| match kind {
            b"VP8X" => 0,
            b"ICCP" => 1,
            b"ALPH" => 2,
            b"VP8 " | b"VP8L" => 3,
            b"EXIF" => 4,
            _ => 5,
        };
        if order.windows(2).any(|k| rank(k[0]) >= rank(k[1]))
            || (seen.contains(b"ALPH".as_slice())
                && (!seen.contains(b"VP8 ".as_slice()) || flags & 0x10 == 0))
        {
            return Err(bad("invalid WebP chunk order/alpha"));
        }
    } else if seen.len() != 1 {
        return Err(bad("simple WebP has extended chunks"));
    }
    let mut decoder =
        image_webp::WebPDecoder::new(Cursor::new(bytes)).map_err(|e| bad(e.to_string()))?;
    let (w, h) = decoder.dimensions();
    let output = decoder
        .output_buffer_size()
        .ok_or_else(|| limit("WebP allocation overflow"))?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 || output > IMAGE_BYTES {
        return Err(limit("image dimensions/decoded bytes exceed profile"));
    }
    if decoder.is_animated() {
        return Err(bad("animated WebP is unsupported"));
    }
    if extended_flags.is_some_and(|flags| decoder.has_alpha() != (flags & 0x10 != 0)) {
        return Err(bad("WebP alpha flag mismatch"));
    }
    budget.charge(output)?;
    decoder.set_memory_limit(IMAGE_BYTES);
    decoder
        .read_image(&mut vec![0; output])
        .map_err(|e| bad(e.to_string()))
}

fn forbidden(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(o) => o.iter().any(|(k, v)| {
            matches!(
                k.as_str(),
                "uri"
                    | "extensions"
                    | "extensionsRequired"
                    | "extensionsUsed"
                    | "animations"
                    | "skins"
                    | "weights"
                    | "targets"
                    | "cameras"
                    | "camera"
                    | "skin"
                    | "extras"
                    | "script"
                    | "scripts"
            ) || forbidden(v)
        }),
        serde_json::Value::Array(a) => a.iter().any(forbidden),
        _ => false,
    }
}
fn validate_glb(bytes: &[u8], budget: &mut DecodeBudget) -> Result<()> {
    if bytes.get(..4) != Some(b"glTF")
        || le(bytes, 4)? != 2
        || le(bytes, 8)? as usize != bytes.len()
    {
        return Err(bad("invalid GLB header"));
    }
    let n = le(bytes, 12)? as usize;
    let end = 20usize
        .checked_add(n)
        .ok_or_else(|| bad("GLB length overflow"))?;
    if n == 0 || n % 4 != 0 || bytes.get(16..20) != Some(b"JSON") {
        return Err(bad("GLB requires aligned JSON first"));
    }
    let json_bytes = bytes
        .get(20..end)
        .ok_or_else(|| bad("truncated GLB JSON"))?;
    let value = parse_resource_json(json_bytes).map_err(|e| bad(e.message))?;
    if forbidden(&value) {
        return Err(bad("GLB requires static embedded declarative resources"));
    }
    if let Some(nodes) = value.get("nodes").and_then(|v| v.as_array()) {
        for node in nodes {
            if node.get("matrix").is_some()
                && ["translation", "rotation", "scale"]
                    .iter()
                    .any(|k| node.get(k).is_some())
            {
                return Err(bad("GLB node mixes matrix and TRS"));
            }
            if let Some(q) = node.get("rotation").and_then(|v| v.as_array()) {
                let norm: f64 = q
                    .iter()
                    .map(|n| n.as_f64().unwrap_or(f64::NAN).powi(2))
                    .sum();
                if !norm.is_finite() || (norm - 1.0).abs() > 0.0001 {
                    return Err(bad("GLB rotation is not unit length"));
                }
            }
        }
    }
    let bin = if end == bytes.len() {
        &[][..]
    } else {
        let len = le(bytes, end)? as usize;
        if len % 4 != 0
            || bytes.get(end + 4..end + 8) != Some(b"BIN\0")
            || end.checked_add(8).and_then(|v| v.checked_add(len)) != Some(bytes.len())
        {
            return Err(bad("GLB permits only one aligned BIN after JSON"));
        }
        &bytes[end + 8..]
    };
    // Validate references/types with the independent glTF schema implementation.
    // Do not call gltf::import: this adapter must never resolve URLs or files.
    let parsed: gltf::json::Root = serde_json::from_value(value).map_err(|e| bad(e.to_string()))?;
    // gltf-json 1.4.1's POSITION validation hook indexes before reporting a bad
    // reference. Guard it explicitly: hostile input must not panic the worker.
    for mesh in &parsed.meshes {
        for primitive in &mesh.primitives {
            if primitive
                .attributes
                .values()
                .any(|i| i.value() >= parsed.accessors.len())
            {
                return Err(bad("GLB attribute references missing accessor"));
            }
        }
    }
    let doc = gltf::Document::from_json(parsed).map_err(|e| bad(e.to_string()))?;
    for material in doc.materials() {
        let pbr = material.pbr_metallic_roughness();
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        if !pbr
            .base_color_factor()
            .into_iter()
            .chain(material.emissive_factor())
            .chain([pbr.metallic_factor(), pbr.roughness_factor()])
            .all(unit)
            || material
                .alpha_cutoff()
                .is_some_and(|v| !v.is_finite() || v < 0.0)
            || material
                .normal_texture()
                .is_some_and(|v| !v.scale().is_finite())
            || material
                .occlusion_texture()
                .is_some_and(|v| !unit(v.strength()))
        {
            return Err(bad("GLB material parameter outside declarative range"));
        }
    }
    if doc.buffers().len() > 1 {
        return Err(bad("GLB permits one embedded buffer"));
    }
    let buffer_len = doc.buffers().next().map_or(0, |b| b.length());
    if buffer_len > bin.len()
        || bin.len() - buffer_len > 3
        || bin[buffer_len..].iter().any(|b| *b != 0)
    {
        return Err(bad("GLB buffer length/padding mismatch"));
    }
    if [
        doc.nodes().len(),
        doc.accessors().len(),
        doc.views().len(),
        doc.meshes().len(),
        doc.images().len(),
        doc.textures().len(),
        doc.materials().len(),
        doc.samplers().len(),
        doc.scenes().len(),
    ]
    .into_iter()
    .any(|n| n > 8192)
    {
        return Err(limit("too many GLB records"));
    }
    for view in doc.views() {
        if view
            .offset()
            .checked_add(view.length())
            .is_none_or(|v| v > buffer_len)
        {
            return Err(bad("GLB buffer view outside BIN"));
        }
    }
    let mut elements = 0usize;
    for a in doc.accessors() {
        elements = elements
            .checked_add(a.count())
            .ok_or_else(|| limit("GLB element overflow"))?;
        if elements > ELEMENTS {
            return Err(limit("too many GLB accessor elements"));
        }
        // Sparse/matrix accessors are outside the current static mesh profile.
        if a.sparse().is_some()
            || !matches!(
                a.dimensions(),
                Dimensions::Scalar | Dimensions::Vec2 | Dimensions::Vec3 | Dimensions::Vec4
            )
            || a.count() == 0
        {
            return Err(bad("unsupported GLB accessor"));
        }
        let view = a.view().ok_or_else(|| bad("accessor lacks buffer view"))?;
        let width = a.size();
        let stride = view.stride().unwrap_or(width);
        let component = a.data_type().size();
        if stride < width
            || stride % component != 0
            || a.offset() % component != 0
            || view
                .offset()
                .checked_add(a.offset())
                .is_none_or(|n| n % component != 0)
            || (a.count() - 1)
                .checked_mul(stride)
                .and_then(|n| n.checked_add(a.offset()))
                .and_then(|n| n.checked_add(width))
                .is_none_or(|n| n > view.length())
        {
            return Err(bad("GLB accessor range/alignment invalid"));
        }
        if a.data_type() == DataType::F32 {
            for i in 0..a.count() {
                let start = view.offset() + a.offset() + i * stride;
                for b in bin[start..start + width].chunks_exact(4) {
                    if !f32::from_le_bytes(b.try_into().unwrap()).is_finite() {
                        return Err(bad("nonfinite GLB accessor value"));
                    }
                }
            }
        }
    }
    let mut primitives = 0;
    let mut draw_elements = 0usize;
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            primitives += 1;
            if primitives > 8192 {
                return Err(limit("too many GLB primitives"));
            }
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err(bad("static mesh profile requires triangles"));
            }
            let position = primitive
                .get(&gltf::Semantic::Positions)
                .ok_or_else(|| bad("mesh lacks POSITION"))?;
            if position.data_type() != DataType::F32
                || position.dimensions() != Dimensions::Vec3
                || position.normalized()
            {
                return Err(bad("POSITION must be float VEC3"));
            }
            for (semantic, a) in primitive.attributes() {
                if a.count() != position.count() {
                    return Err(bad("vertex attribute counts disagree"));
                }
                let valid = match semantic {
                    gltf::Semantic::Positions | gltf::Semantic::Normals => {
                        a.data_type() == DataType::F32
                            && a.dimensions() == Dimensions::Vec3
                            && !a.normalized()
                    }
                    gltf::Semantic::Tangents => {
                        a.data_type() == DataType::F32
                            && a.dimensions() == Dimensions::Vec4
                            && !a.normalized()
                    }
                    gltf::Semantic::TexCoords(_) => {
                        a.dimensions() == Dimensions::Vec2 && attribute_type(&a)
                    }
                    gltf::Semantic::Colors(_) => {
                        matches!(a.dimensions(), Dimensions::Vec3 | Dimensions::Vec4)
                            && attribute_type(&a)
                    }
                    _ => false,
                };
                if !valid {
                    return Err(bad("unsupported GLB vertex attribute"));
                }
            }
            draw_elements = draw_elements
                .checked_add(primitive.indices().map_or(position.count(), |a| a.count()))
                .ok_or_else(|| limit("GLB draw count overflow"))?;
            if draw_elements > ELEMENTS {
                return Err(limit("too many GLB draw elements"));
            }
            let reader = primitive.reader(|_| Some(&bin[..buffer_len]));
            let count = if let Some(a) = primitive.indices() {
                if a.dimensions() != Dimensions::Scalar
                    || !matches!(a.data_type(), DataType::U8 | DataType::U16 | DataType::U32)
                    || a.normalized()
                    || a.view().unwrap().stride().is_some()
                {
                    return Err(bad("invalid GLB index accessor"));
                }
                let indices = reader
                    .read_indices()
                    .ok_or_else(|| bad("cannot decode GLB indices"))?;
                if indices.into_u32().any(|i| i as usize >= position.count()) {
                    return Err(bad("GLB index outside vertices"));
                }
                a.count()
            } else {
                position.count()
            };
            if count % 3 != 0 {
                return Err(bad("incomplete GLB triangle"));
            }
        }
    }
    // Validate every node, including unreferenced trees, without recursive traversal.
    let mut parents = vec![0u8; doc.nodes().len()];
    let mut edges = vec![Vec::new(); parents.len()];
    for node in doc.nodes() {
        if node
            .transform()
            .matrix()
            .iter()
            .flatten()
            .any(|n| !n.is_finite())
        {
            return Err(bad("invalid GLB node transform"));
        }
        for child in node.children() {
            if parents[child.index()] != 0 {
                return Err(bad("GLB node has multiple parents"));
            }
            parents[child.index()] = 1;
            edges[node.index()].push(child.index());
        }
    }
    for scene in doc.scenes() {
        let mut roots = BTreeSet::new();
        for root in scene.nodes() {
            if parents[root.index()] != 0 || !roots.insert(root.index()) {
                return Err(bad("invalid GLB scene root"));
            }
        }
    }
    let mut ready: Vec<_> = parents
        .iter()
        .enumerate()
        .filter_map(|(i, &p)| (p == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        ready.extend(&edges[i]);
    }
    if visited != parents.len() {
        return Err(bad("cyclic GLB node graph"));
    }
    let mut decoded_images = BTreeSet::new();
    for image in doc.images() {
        match image.source() {
            gltf::image::Source::View {
                view,
                mime_type: "image/png",
            } => {
                if decoded_images.insert((view.offset(), view.length())) {
                    validate_png(&bin[view.offset()..view.offset() + view.length()], budget)?;
                }
            }
            _ => {
                return Err(bad(
                    "GLB embedded images must be PNG; external resources unsupported",
                ))
            }
        }
    }
    Ok(())
}
fn attribute_type(a: &gltf::Accessor<'_>) -> bool {
    (a.data_type() == DataType::F32 && !a.normalized())
        || (matches!(a.data_type(), DataType::U8 | DataType::U16) && a.normalized())
}
pub(super) fn validate_assets(d: &MapDocument, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let mut seen = BTreeSet::new();
    let mut budget = DecodeBudget {
        remaining: 256 * 1024 * 1024,
    };
    for a in &d.assets {
        if !seen.insert(&a.path) {
            continue;
        }
        let bytes = files
            .get(&a.path)
            .ok_or_else(|| error("E_REFERENCE", "missing asset"))?;
        if a.path.ends_with(".glb") {
            validate_glb(bytes, &mut budget)?;
        } else if a.path.ends_with(".png") {
            validate_png(bytes, &mut budget)?;
        } else if a.path.ends_with(".webp") {
            validate_webp(bytes, &mut budget)?;
        } else {
            return Err(bad("unsupported asset type"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_image_decode_allowance_is_inclusive_and_charged_before_pixels() {
        let mut bytes = vec![];
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[0; 4])
                .unwrap();
        }
        let mut budget = DecodeBudget { remaining: 4 };
        validate_png(&bytes, &mut budget).unwrap();
        assert_eq!(budget.remaining, 0);
        assert_eq!(
            validate_png(&bytes, &mut budget).unwrap_err().code,
            "E_LIMIT"
        );
    }
}
