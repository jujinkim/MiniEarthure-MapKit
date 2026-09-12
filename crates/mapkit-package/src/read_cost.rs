use super::*;

/// Conservative planning allowances, not allocator/RSS measurements. No payload
/// is fully inflated. Bounded prefixes refine decoder workspace; full validation
/// still rejects malformed headers, JSON and image data before use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadCost {
    pub retained_memory_bytes: u64,
    pub validation_peak_bytes: u64,
}

pub fn inspect_read_cost(bytes: &[u8]) -> Result<ReadCost> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(error("E_LIMIT", "compressed package exceeds profile"));
    }
    container::validate(bytes)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    if archive.is_empty() || archive.len() > MAX_FILES + 1 {
        return Err(error("E_LIMIT", "invalid ZIP file count"));
    }
    let mut folded = BTreeSet::new();
    let mut expanded = 0u64;
    let mut structured = 0u64;
    let mut pngs = 0u64;
    let mut image_peak = 8 * 1024 * 1024u64;
    let mut metadata = 1024 * 1024u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_error)?;
        let name = entry.name().to_string();
        let entry_size = entry.size();
        if i == 0 && (name != "manifest.json" || entry.header_start() != 0) {
            return Err(error(
                "E_MANIFEST",
                "manifest.json must be first ZIP entry without prefix",
            ));
        }
        if !safe_path(&name)
            || !folded.insert(name.to_ascii_lowercase())
            || entry.is_dir()
            || entry
                .unix_mode()
                .is_some_and(|m| m & 0o170000 != 0 && m & 0o170000 != 0o100000)
        {
            return Err(error(
                "E_PATH",
                "unsafe, duplicate, case-colliding or non-regular ZIP path",
            ));
        }
        let limit = if i == 0 {
            MAX_MANIFEST_BYTES
        } else if name == "document.json" {
            MAX_DOCUMENT_BYTES
        } else {
            MAX_ENTRY_BYTES
        };
        expanded = expanded
            .checked_add(entry.size())
            .ok_or_else(|| error("E_LIMIT", "expanded size overflow"))?;
        if entry.size() > limit || expanded > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "expanded package exceeds profile"));
        }
        if !matches!(
            entry.compression(),
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            return Err(error("E_ZIP", "unsupported compression"));
        }
        if name.ends_with(".json") || name.ends_with(".glb") {
            structured += entry.size();
        }
        if name.ends_with(".glb") || name.ends_with(".png") || name.ends_with(".webp") {
            image_peak = image_peak.max(decoder_peak(&mut entry, &name, entry_size));
        }
        if name.ends_with(".png") {
            pngs += 1;
        }
        metadata += 1024 + name.len() as u64 * 8;
    }
    // Payload vectors, typed document/manifest and their strings. GLB JSON is
    // bounded conservatively by its complete entry size before decoding headers.
    // Immutable prepared-map estimate cache: at most 16384 entries, 256 bytes each.
    let retained = expanded * 2 + structured * 32 + metadata + 16_384 * 256;
    // Strict JSON key validation, typed parsing and content-hash canonicalization
    // overlap with retained data. Image decoder limits are sequential; heightmap
    // seams retain at most four edges of 513 i64 samples per PNG under v1 limits.
    let transient =
        structured * 96 + bytes.len() as u64 * 2 + image_peak + pngs * (4 * 513 * 8 + 256);
    Ok(ReadCost {
        retained_memory_bytes: retained,
        validation_peak_bytes: (retained + transient).max(bytes.len() as u64 * 4 + 8 * 1024 * 1024),
    })
}

/// Prefix inspection uses < 16 KiB decompressed input plus bounded JSON scratch,
/// covered by the 8 MiB pre-index allowance. Never scan BIN or decode pixels here.
/// Unknown/invalid/larger headers keep the previous 256 MiB image allowance.
pub(super) fn decoder_peak(entry: &mut impl Read, name: &str, size: u64) -> u64 {
    const FULL: u64 = 256 * 1024 * 1024;
    if name.ends_with(".png") {
        let mut h = [0u8; 33];
        if entry.read_exact(&mut h).is_err()
            || &h[..8] != b"\x89PNG\r\n\x1a\n"
            || &h[8..16] != b"\0\0\0\rIHDR"
        {
            return FULL;
        }
        let w = u32::from_be_bytes(h[16..20].try_into().unwrap()) as u64;
        let height = u32::from_be_bytes(h[20..24].try_into().unwrap()) as u64;
        if w == 0 || height == 0 || w > 8192 || height > 8192 {
            return FULL;
        }
        // Existing decoder retains its 64 MiB hard workspace limit. RGBA16 is
        // the largest permitted output; the extra 8 MiB also covers height grids.
        return 72 * 1024 * 1024 + (w * height * 8).min(64 * 1024 * 1024);
    }
    if !name.ends_with(".glb") {
        return FULL;
    }
    let mut h = [0u8; 20];
    if entry.read_exact(&mut h).is_err() || &h[..8] != b"glTF\x02\0\0\0" || &h[16..20] != b"JSON" {
        return FULL;
    }
    let n = u32::from_le_bytes(h[12..16].try_into().unwrap()) as usize;
    if n == 0
        || n > 16 * 1024
        || n % 4 != 0
        || u32::from_le_bytes(h[8..12].try_into().unwrap()) as u64 != size
        || n as u64 + 20 > size
    {
        return FULL;
    }
    let mut bytes = vec![0; n];
    if entry.read_exact(&mut bytes).is_err() {
        return FULL;
    }
    let Ok(value) = parse_resource_json(&bytes) else {
        return FULL;
    };
    if !value.is_object()
        || value
            .get("images")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return FULL;
    }
    8 * 1024 * 1024
}
