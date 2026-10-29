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
    let mut tree = 0u64;
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
            let shape = match json_shape(&mut entry, entry_size) {
                Ok(shape) => shape,
                Err(e) if e.code == "E_CANCELLED" => return Err(e),
                // A damaged stream gets no discount. Admission still precedes
                // full decoding and its precise corruption/hash diagnostics.
                Err(_) => JsonShape::default(),
            };
            long_strings += shape.tail;
            tree += shape.tree.unwrap_or(entry_size * LEGACY_TREE_RATE);
        } else if name.ends_with(".glb") {
            tree += entry_size * LEGACY_TREE_RATE;
        }
        if name.ends_with(".glb") || name.ends_with(".png") || name.ends_with(".webp") {
            image_peak = image_peak.max(decoder_peak(&mut entry, &name, entry_size));
        }
        if name.ends_with(".png") {
            pngs += 1;
        }
        metadata += 1024 + name.len() as u64 * 8;
    }
    Ok(planned(expanded, structured, long_strings, tree, pngs, image_peak, metadata, bytes.len() as u64))
}

/// The same planning allowance for in-memory project files and their manifest
/// size; no compressed container exists.
pub(crate) fn files_cost(manifest_bytes: u64, files: &BTreeMap<String, Vec<u8>>) -> Result<ReadCost> {
    let (mut expanded, mut structured, mut long_strings, mut pngs) = (manifest_bytes, manifest_bytes, 0u64, 0u64);
    // The manifest is not scanned here: charge it at the legacy rate.
    let mut tree = manifest_bytes * LEGACY_TREE_RATE;
    let mut image_peak = 8 * 1024 * 1024u64;
    let mut metadata = 1024 * 1024u64 + 1024 + "manifest.json".len() as u64 * 8;
    for (name, bytes) in files {
        let size = bytes.len() as u64;
        expanded += size;
        if name.ends_with(".json") || name.ends_with(".glb") {
            structured += size;
        }
        if name.ends_with(".json") {
            let shape = match json_shape(&mut bytes.as_slice(), size) {
                Ok(shape) => shape,
                Err(e) if e.code == "E_CANCELLED" => return Err(e),
                Err(_) => JsonShape::default(),
            };
            long_strings += shape.tail;
            tree += shape.tree.unwrap_or(size * LEGACY_TREE_RATE);
        } else if name.ends_with(".glb") {
            tree += size * LEGACY_TREE_RATE;
        }
        if name.ends_with(".glb") || name.ends_with(".png") || name.ends_with(".webp") {
            image_peak = image_peak.max(decoder_peak(&mut bytes.as_slice(), name, size));
        }
        if name.ends_with(".png") {
            pngs += 1;
        }
        metadata += 1024 + name.len() as u64 * 8;
    }
    Ok(planned(expanded, structured, long_strings, tree, pngs, image_peak, metadata, 0))
}

/// Unscanned or unusual JSON (and GLB JSON) is charged one Value tree of 34
/// bytes per byte, so its four overlapping trees keep the previous 136 B/B.
const LEGACY_TREE_RATE: u64 = 34;

#[allow(clippy::too_many_arguments)]
fn planned(expanded: u64, structured: u64, long_strings: u64, tree: u64, pngs: u64, image_peak: u64, metadata: u64, compressed: u64) -> ReadCost {
    // Payload vectors, typed document/manifest and their strings. GLB JSON is
    // bounded conservatively by its complete entry size before decoding headers.
    // Immutable core estimate and bridge preparation caches: at most the shared bounded cache capacity,
    // 256 + 4096 bytes each, including serialized keys and allocation overhead, plus the junction plans.
    // Source-proportional spatial/lookup indices plus 32 MiB for up to 20000
    // accepted repetition records, occupied footprints and their broad phase.
    // These bytes remain charged while the prepared source is retained.
    // Long JSON string tails have byte storage, not a Value/tree node per byte.
    // Keep the first 256 encoded bytes of every string at the structural rate
    // for its node/allocation overhead, plus four retained and eight temporary
    // copies of its tail. Expanded input buffers remain charged separately.
    let structure = structured - long_strings;
    let retained = expanded * 2 + structure * 40 + long_strings * 4 + metadata + mapkit_core::PREPARATION_CACHE_BYTES + 32 * 1024 * 1024;
    // Strict JSON key validation builds one serde_json Value tree; typed parsing
    // and the content hash (a typed clone, its Value and the canonical copy of
    // that Value) overlap with it: at most four trees of the scanned bound
    // (`json_shape`), each no smaller than its typed form, plus the canonical
    // bytes they serialize to. Image decoder limits are sequential; heightmap
    // seams retain at most four edges of 513 i64 samples per PNG under v1 limits.
    let transient =
        tree * 4 + structured + compressed * 2 + image_peak.max(track_workspace(structured)) + pngs * (4 * 513 * 8 + 256);
    ReadCost {
        retained_memory_bytes: retained,
        validation_peak_bytes: (retained + transient).max(compressed * 4 + 8 * 1024 * 1024),
    }
}

/// Lexical accounting only: no values or keys are allocated and this does not
/// accept JSON. Encoded bytes bound decoded string storage, including escapes.
/// Unknown/incomplete string syntax retains the old conservative allowance.
/// Size, CRC, duplicate keys, UTF-8, syntax and hashes are still validated by the
/// normal reader. The 64 KiB buffer fits the existing 8 MiB pre-index allowance.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct JsonShape {
    /// Encoded bytes beyond the first 256 of every string (0 when unusual).
    pub tail: u64,
    /// Upper bound of the serde_json `Value` tree's heap bytes; None when the
    /// syntax is unusual or nesting exceeds the parser's depth limit.
    pub tree: Option<u64>,
}

/// serde_json refuses nesting deeper than 128 before allocating further.
const MAX_DEPTH: usize = 128;
/// `Value` is 32 bytes. A non-empty `Vec<Value>` grows by doubling from 4.
fn array_heap(n: u64) -> u64 {
    if n == 0 { 0 } else { 32 * n.max(4).next_power_of_two() }
}
/// `BTreeMap<String, Value>` (serde_json without `preserve_order`): one 632-byte
/// leaf up to 11 entries; larger maps keep leaves at least five entries full
/// plus internal nodes (728 bytes).
fn object_heap(n: u64) -> u64 {
    if n == 0 { 0 } else if n <= 11 { 632 } else { 640 + 160 * n }
}

pub(crate) fn json_shape(entry: &mut impl Read, expected: u64) -> Result<JsonShape> {
    let mut buffer = [0u8; 64 * 1024];
    let (mut seen, mut tail, mut length) = (0u64, 0u64, 0u64);
    let (mut quoted, mut escaped, mut invalid) = (false, false, false);
    // Container stack (object?, entries or elements) and the root slot.
    let mut stack = [(false, 0u64); MAX_DEPTH + 1];
    let (mut depth, mut tree, mut scalar, mut deep) = (0usize, 32u64, false, false);
    let element = |stack: &mut [(bool, u64)], depth: usize| if depth > 0 && !stack[depth - 1].0 { stack[depth - 1].1 += 1 };
    loop {
        mapkit_core::cancellation::checkpoint()?;
        let count = entry.read(&mut buffer).map_err(zip_error)?;
        if count == 0 { break; }
        seen += count as u64;
        if seen > expected { return Err(error("E_LIMIT", "ZIP decompressed size mismatch")); }
        for &byte in &buffer[..count] {
            if quoted {
                if !escaped && byte == b'"' {
                    tail += length.saturating_sub(256);
                    tree += length;
                    quoted = false;
                } else {
                    length += 1;
                    if byte < 0x20 { invalid = true; }
                    if escaped {
                        if !matches!(byte, b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u') { invalid = true; }
                        escaped = false;
                    } else if byte == b'\\' { escaped = true; }
                }
                continue;
            }
            match byte {
                b'"' => { scalar = false; quoted = true; length = 0; element(&mut stack, depth); }
                b'[' | b'{' => {
                    scalar = false;
                    element(&mut stack, depth);
                    if depth == MAX_DEPTH { deep = true; continue; }
                    if deep { continue; }
                    stack[depth] = (byte == b'{', 0);
                    depth += 1;
                }
                b']' | b'}' => {
                    scalar = false;
                    if deep { continue; }
                    if depth == 0 || stack[depth - 1].0 != (byte == b'}') { invalid = true; continue; }
                    depth -= 1;
                    let (object, n) = stack[depth];
                    tree += if object { object_heap(n) } else { array_heap(n) };
                }
                b':' => { scalar = false; if depth > 0 && stack[depth - 1].0 { stack[depth - 1].1 += 1 } }
                b',' | b' ' | b'\n' | b'\r' | b'\t' => scalar = false,
                _ => {
                    // Numbers and literals: count their first byte only.
                    if !scalar { element(&mut stack, depth); }
                    scalar = true;
                }
            }
        }
    }
    if seen != expected { return Err(error("E_LIMIT", "ZIP decompressed size mismatch")); }
    let unusual = quoted || invalid;
    Ok(JsonShape { tail: if unusual { 0 } else { tail }, tree: (!unusual && !deep && depth == 0).then_some(tree) })
}

#[cfg(test)]
fn long_string_bytes(entry: &mut impl Read, expected: u64) -> Result<u64> {
    json_shape(entry, expected).map(|shape| shape.tail)
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
    fn value_tree_bound_follows_vectors_maps_and_strings() {
        let shape = |text: &str| json_shape(&mut text.as_bytes(), text.len() as u64).unwrap();
        // Root slot, one array of three (capacity 4), no strings.
        assert_eq!(shape("[0, 1, 2]").tree, Some(32 + 128));
        // Five elements grow to capacity 8; nested pairs keep their own vectors.
        assert_eq!(shape("[[0],[1],{},[],5]").tree, Some(32 + 256 + 2 * 128));
        // One map leaf, key and string bytes, the inner array.
        assert_eq!(shape(r#"{"ab": "xyz", "c": [true]}"#).tree, Some(32 + 632 + 2 + 3 + 1 + 128));
        let wide = format!("{{{}}}", (0..12).map(|i| format!("\"{i:x}\":0")).collect::<Vec<_>>().join(","));
        assert_eq!(shape(&wide).tree, Some(32 + 640 + 160 * 12 + 12));
        // Unusual input and nesting beyond serde_json's depth limit fall back.
        assert_eq!(shape(&format!("{}{}", "[".repeat(129), "]".repeat(129))).tree, None);
        assert!(shape(&format!("{}{}", "[".repeat(128), "]".repeat(128))).tree.is_some());
        for bad in ["[1", "]", "[}", "\"open"] { assert_eq!(shape(bad).tree, None, "{bad}"); }
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
