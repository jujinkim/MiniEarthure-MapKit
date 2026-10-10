use super::*;

/// Conservative planning allowances, not allocator/RSS measurements. No payload
/// is retained while scanning. Bounded prefixes refine decoder workspace and a
/// fixed-buffer JSON scan distinguishes long strings from structure. Full
/// validation still rejects malformed headers, JSON and image data before use.
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
    let mut long_strings = 0u64;
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
        } else if name == preview::PATH {
            preview::MAX_BYTES
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
        if name.ends_with(".json") {
            long_strings += match long_string_bytes(&mut entry, entry_size) {
                Ok(tail) => tail,
                Err(e) if e.code == "E_CANCELLED" => return Err(e),
                // A damaged stream gets no discount. Admission still precedes
                // full decoding and its precise corruption/hash diagnostics.
                Err(_) => 0,
            };
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
    // Immutable core estimate and bridge preparation caches: at most the shared bounded cache capacity,
    // 256 + 4096 bytes each, including serialized keys and allocation overhead.
    // Source-proportional spatial/lookup indices plus 32 MiB for up to 20000
    // accepted repetition records, occupied footprints and their broad phase.
    // These bytes remain charged while the prepared source is retained.
    // Long JSON string tails have byte storage, not a Value/tree node per byte.
    // Keep the first 256 encoded bytes of every string at the structural rate
    // for its node/allocation overhead, plus four retained and eight temporary
    // copies of its tail. Expanded input buffers remain charged separately.
    let structure = structured - long_strings;
    let retained = expanded * 2 + structure * 40 + long_strings * 4 + metadata + mapkit_core::PREPARATION_CACHE_BYTES + 32 * 1024 * 1024;
    // Strict JSON key validation, typed parsing and content-hash canonicalization
    // overlap with retained data. Image decoder limits are sequential; heightmap
    // seams retain at most four edges of 513 i64 samples per PNG under v1 limits.
    let transient =
        structure * 96 + long_strings * 8 + bytes.len() as u64 * 2 + image_peak.max(track_workspace(structured)) + pngs * (4 * 513 * 8 + 256);
    Ok(ReadCost {
        retained_memory_bytes: retained,
        validation_peak_bytes: (retained + transient).max(bytes.len() as u64 * 4 + 8 * 1024 * 1024),
    })
}

/// Lexical accounting only: no values or keys are allocated and this does not
/// accept JSON. Encoded bytes bound decoded string storage, including escapes.
/// Unknown/incomplete string syntax retains the old conservative allowance.
/// Size, CRC, duplicate keys, UTF-8, syntax and hashes are still validated by the
/// normal reader. The 64 KiB buffer fits the existing 8 MiB pre-index allowance.
fn long_string_bytes(entry: &mut impl Read, expected: u64) -> Result<u64> {
    let mut buffer = [0u8; 64 * 1024];
    let (mut seen, mut tail, mut length) = (0u64, 0u64, 0u64);
    let (mut quoted, mut escaped, mut invalid) = (false, false, false);
    loop {
        mapkit_core::cancellation::checkpoint()?;
        let count = entry.read(&mut buffer).map_err(zip_error)?;
        if count == 0 { break; }
        seen += count as u64;
        if seen > expected { return Err(error("E_LIMIT", "ZIP decompressed size mismatch")); }
        for &byte in &buffer[..count] {
            if !quoted {
                if byte == b'"' { quoted = true; length = 0; }
            } else if !escaped && byte == b'"' {
                tail += length.saturating_sub(256);
                quoted = false;
            } else {
                length += 1;
                if byte < 0x20 { invalid = true; }
                if escaped {
                    if !matches!(byte, b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u') { invalid = true; }
                    escaped = false;
                } else if byte == b'\\' { escaped = true; }
            }
        }
    }
    if seen != expected { return Err(error("E_LIMIT", "ZIP decompressed size mismatch")); }
    Ok(if quoted || invalid { 0 } else { tail })
}

#[cfg(test)]
mod string_accounting_tests {
    use super::*;
    #[test]
    fn counts_encoded_tails_across_buffer_boundaries_without_parsing_values() {
        let source = serde_json::to_vec(&serde_json::json!({"notice": "한글\\\"\n".repeat(20_000), "short": "text"})).unwrap();
        let encoded = serde_json::to_vec(&"한글\\\"\n".repeat(20_000)).unwrap();
        assert_eq!(long_string_bytes(&mut source.as_slice(), source.len() as u64).unwrap(), encoded.len() as u64 - 2 - 256);
        for bad in [format!("\"{}", "x".repeat(300)), format!("\"{}\\q\"", "x".repeat(300)), format!("\"{}\n\"", "x".repeat(300))] {
            assert_eq!(long_string_bytes(&mut bad.as_bytes(), bad.len() as u64).unwrap(), 0);
        }
        assert!(long_string_bytes(&mut source.as_slice(), 1).is_err());
        assert!(long_string_bytes(&mut source.as_slice(), source.len() as u64 + 1).is_err());
    }
    #[test]
    fn cancellation_interrupts_scanning_at_the_next_bounded_block() {
        struct Cancelling<'a> { bytes: &'a [u8], token: mapkit_core::cancellation::CancellationToken }
        impl Read for Cancelling<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let n = self.bytes.read(buffer)?;
                self.token.cancel();
                Ok(n)
            }
        }
        let source = vec![b' '; 128 * 1024];
        let token = mapkit_core::cancellation::CancellationToken::default();
        let mut reader = Cancelling { bytes: &source, token: token.clone() };
        assert_eq!(token.run(|| long_string_bytes(&mut reader, source.len() as u64)).unwrap_err().code, "E_CANCELLED");
        assert!(mapkit_core::cancellation::checkpoint().is_ok());
    }
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

/// Grounding keeps one bounded piece mesh, including the temporary public tube
/// mesh, at a time. Source-proportional allowance saturates at the enforced mesh
/// workspace limit; charged before parsing/validation in both containers.
pub(crate) fn track_workspace(source_bytes: u64) -> u64 {
    source_bytes.saturating_mul(512).min(96 * 1024 * 1024) + 1024 * 1024
}
