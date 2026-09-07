use super::*;

/// Conservative planning allowances, not allocator/RSS measurements. No payload
/// is inflated and no source metadata is trusted as a memory authorization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadCost {
    pub retained_memory_bytes: u64,
    pub validation_peak_bytes: u64,
}

pub fn inspect_read_cost(bytes: &[u8]) -> Result<ReadCost> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(error("E_LIMIT", "compressed package exceeds profile"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    if archive.is_empty() || archive.len() > MAX_FILES + 1 {
        return Err(error("E_LIMIT", "invalid ZIP file count"));
    }
    let mut folded = BTreeSet::new();
    let mut expanded = 0u64;
    let mut structured = 0u64;
    let mut pngs = 0u64;
    let mut images = false;
    let mut metadata = 1024 * 1024u64;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(zip_error)?;
        let name = entry.name();
        if i == 0 && (name != "manifest.json" || entry.header_start() != 0) {
            return Err(error(
                "E_MANIFEST",
                "manifest.json must be first ZIP entry without prefix",
            ));
        }
        if !safe_path(name)
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
        if name.ends_with(".png") {
            pngs += 1;
            images = true;
        }
        if name.ends_with(".webp") {
            images = true;
        }
        metadata += 1024 + name.len() as u64 * 8;
    }
    // Payload vectors, typed document/manifest and their strings. GLB JSON is
    // bounded conservatively by its complete entry size before decoding headers.
    let retained = expanded * 2 + structured * 32 + metadata;
    // Strict JSON key validation, typed parsing and content-hash canonicalization
    // overlap with retained data. Image decoder limits are sequential; heightmap
    // seams retain at most four edges of 513 i64 samples per PNG under v1 limits.
    let transient = structured * 96
        + bytes.len() as u64 * 2
        + if images {
            80 * 1024 * 1024
        } else {
            8 * 1024 * 1024
        }
        + pngs * (4 * 513 * 8 + 256);
    Ok(ReadCost {
        retained_memory_bytes: retained,
        validation_peak_bytes: (retained + transient).max(bytes.len() as u64 * 4 + 8 * 1024 * 1024),
    })
}
