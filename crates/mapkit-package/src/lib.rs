//! Bounded package I/O adapter. The core never opens files or reads a clock.
use mapkit_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_FILES: usize = 8192;
mod read_cost;
pub use read_cost::{inspect_read_cost, ReadCost};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    pub path: String,
    #[schemars(range(max = 134217728))]
    pub size: u64,
    #[schemars(length(equal = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    #[schemars(regex(pattern = "^memap$"))]
    pub format: String,
    #[schemars(range(min = 1, max = 1))]
    pub format_version: u32,
    #[schemars(range(min = 1, max = 1))]
    pub recipe_version: u32,
    #[schemars(range(min = 6, max = 6))]
    pub generated_version: u32,
    #[schemars(length(min = 1, max = 128))]
    pub map_id: String,
    pub revision: u32,
    pub bounds: Bounds,
    #[schemars(range(min = 200, max = 102400))]
    pub cell_size_cm: u32,
    #[schemars(range(max = 9007199254740991u64))]
    pub seed: u64,
    #[schemars(regex(pattern = "^default$"))]
    pub theme: String,
    #[schemars(regex(pattern = r"^document\.json$"))]
    pub document: String,
    #[schemars(length(min = 1, max = 8192))]
    pub files: Vec<FileRecord>,
    pub assets: Vec<Asset>,
    pub attributions: Vec<Attribution>,
    pub provenance: Provenance,
    #[schemars(length(equal = 64), regex(pattern = "^[0-9a-f]{64}$"))]
    pub world_content_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub package_sha256: String,
    pub world_content_hash: String,
    pub package_bytes: u64,
    pub expanded_bytes: u64,
    pub base_data_bytes: u64,
    pub user_asset_bytes: u64,
    pub cell_count: usize,
    pub retained_memory_bytes: u64,
    pub validation_peak_bytes: u64,
}
pub struct Package {
    pub manifest: PackageManifest,
    pub document: MapDocument,
    pub files: BTreeMap<String, Vec<u8>>,
    pub inspection: Inspection,
}
fn io(e: impl std::fmt::Display) -> Error {
    error("E_IO", e.to_string())
}
fn zip_error(e: impl std::fmt::Display) -> Error {
    error("E_ZIP", e.to_string())
}
fn json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    // Reject duplicate object keys before typed deserialization, including nested metadata.
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = serde_json::Value;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("JSON without duplicate keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry::<String, Strict>()? {
                if values.insert(key, value.0).is_some() {
                    return Err(serde::de::Error::custom("duplicate JSON key"));
                }
            }
            Ok(serde_json::Value::Object(values))
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = vec![];
            while let Some(v) = seq.next_element::<Strict>()? {
                values.push(v.0);
            }
            Ok(values.into())
        }
        fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
            Ok(v.into())
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
            Ok(v.into())
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
            Ok(v.into())
        }
        fn visit_f64<E: serde::de::Error>(self, _v: f64) -> std::result::Result<Self::Value, E> {
            Err(E::custom("only integer JSON numbers supported"))
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
            Ok(v.into())
        }
        fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(serde_json::Value::Null)
        }
    }
    struct Strict(serde_json::Value);
    impl<'de> Deserialize<'de> for Strict {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
            d.deserialize_any(Unique).map(Strict)
        }
    }
    let value =
        serde_json::from_slice::<Strict>(bytes).map_err(|e| error("E_JSON", e.to_string()))?;
    serde_json::from_value(value.0).map_err(|e| error("E_JSON", e.to_string()))
}
fn content_hash(document: &MapDocument, files: &BTreeMap<String, Vec<u8>>) -> Result<String> {
    let mut doc = document.clone();
    doc.normalize();
    let mut value = serde_json::to_value(doc).map_err(io)?;
    value.as_object_mut().unwrap().remove("provenance");
    value.as_object_mut().unwrap().remove("attributions");
    let hashes: BTreeMap<_, _> = files
        .iter()
        .filter(|(p, _)| p.as_str() != "document.json")
        .map(|(p, b)| (p, sha256(b)))
        .collect();
    Ok(sha256(&canonical(&(value, hashes))?))
}
fn references(d: &MapDocument) -> Result<BTreeSet<String>> {
    let mut paths = BTreeSet::from(["document.json".into()]);
    let mut folded = BTreeSet::from(["document.json".to_string(), "manifest.json".to_string()]);
    for p in d
        .heightmaps
        .iter()
        .map(|h| &h.path)
        .chain(d.assets.iter().map(|a| &a.path))
    {
        if !safe_path(p) || p == "document.json" || p == "manifest.json" {
            return Err(error("E_PATH", "unsafe/reserved reference"));
        }
        if paths.contains(p) {
            continue;
        }
        if !folded.insert(p.to_ascii_lowercase()) {
            return Err(error("E_PATH", "case-colliding reference"));
        }
        paths.insert(p.clone());
    }
    Ok(paths)
}
fn validate_glb(bytes: &[u8]) -> Result<()> {
    let bad = || error("E_ASSET", "GLB must contain static embedded resources only");
    if bytes.len() < 20
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len()
    {
        return Err(bad());
    }
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" || length > bytes.len() - 20 {
        return Err(bad());
    }
    // GLTF JSON permits floating-point material/vertex parameters, unlike map documents.
    let value: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + length]).map_err(|_| bad())?;
    fn forbidden(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(o) => o.iter().any(|(k, v)| {
                matches!(
                    k.as_str(),
                    "uri"
                        | "extensions"
                        | "extensionsRequired"
                        | "extensionsUsed"
                        | "animations"
                        | "skins"
                ) || forbidden(v)
            }),
            serde_json::Value::Array(a) => a.iter().any(forbidden),
            _ => false,
        }
    }
    if forbidden(&value) {
        return Err(bad());
    }
    Ok(())
}
fn validate_assets(d: &MapDocument, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for a in &d.assets {
        let bytes = &files[&a.path];
        if a.path.ends_with(".glb") {
            validate_glb(bytes)?;
        } else if a.path.ends_with(".png") {
            let mut decoder = png::Decoder::new(Cursor::new(bytes));
            decoder.set_limits(png::Limits {
                bytes: 64 * 1024 * 1024,
            });
            let reader = decoder
                .read_info()
                .map_err(|e| error("E_ASSET", e.to_string()))?;
            if reader.info().width > 8192
                || reader.info().height > 8192
                || reader.output_buffer_size() > 64 * 1024 * 1024
            {
                return Err(error("E_LIMIT", "image dimensions exceed profile"));
            }
        } else if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
            return Err(error("E_ASSET", "invalid WebP header"));
        }
    }
    Ok(())
}
pub fn decode_heightmap(h: &Heightmap, cell_size: u32, bytes: &[u8]) -> Result<HeightGrid> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 8 * 1024 * 1024,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|e| error("E_HEIGHTMAP", e.to_string()))?;
    let side = cell_size / h.spacing_cm + 1;
    let info = reader.info();
    if info.width != side
        || info.height != side
        || info.bit_depth != png::BitDepth::Sixteen
        || info.color_type != png::ColorType::Grayscale
    {
        return Err(error(
            "E_HEIGHTMAP",
            "expected single-channel 16-bit grid with shared edge samples",
        ));
    }
    let mut data = vec![0; reader.output_buffer_size()];
    let result = reader
        .next_frame(&mut data)
        .map_err(|e| error("E_HEIGHTMAP", e.to_string()))?;
    let heights_cm = data[..result.buffer_size()]
        .chunks_exact(2)
        .map(|v| h.offset_cm + u16::from_be_bytes([v[0], v[1]]) as i64 * h.step_cm as i64)
        .collect();
    Ok(HeightGrid {
        side: side as usize,
        heights_cm,
    })
}
fn validate_heightmaps(d: &MapDocument, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    // Keep only seams, rather than every decoded map-sized grid at once.
    // Direction order: west, east, south, north (local y grows north).
    let mut grids = BTreeMap::new();
    for h in &d.heightmaps {
        let g = decode_heightmap(h, d.cell_size_cm, &files[&h.path])?;
        let edges: [Vec<i64>; 4] = [
            (0..g.side).map(|i| g.heights_cm[i * g.side]).collect(),
            (0..g.side)
                .map(|i| g.heights_cm[i * g.side + g.side - 1])
                .collect(),
            g.heights_cm[..g.side].to_vec(),
            g.heights_cm[(g.side - 1) * g.side..].to_vec(),
        ];
        grids.insert(h.cell, (h, edges));
    }
    for c in d.cells() {
        for n in [Cell { x: c.x + 1, y: c.y }, Cell { x: c.x, y: c.y + 1 }] {
            if !d.has_cell(n) {
                continue;
            }
            let (a, b) = (grids.get(&c), grids.get(&n));
            if a.is_none() && b.is_none() {
                continue;
            }
            let spacing = a
                .map(|(h, _)| h.spacing_cm)
                .or_else(|| b.map(|(h, _)| h.spacing_cm))
                .unwrap();
            if a.is_some_and(|(h, _)| h.spacing_cm != spacing)
                || b.is_some_and(|(h, _)| h.spacing_cm != spacing)
            {
                return Err(error("E_SEAM", "adjacent heightmap spacing must match"));
            }
            let count = (d.cell_size_cm / spacing + 1) as usize;
            for i in 0..count {
                let av = a.map_or(d.terrain_base_cm, |(_, g)| {
                    if c.x != n.x {
                        g[1][i]
                    } else {
                        g[3][i]
                    }
                });
                let bv = b.map_or(d.terrain_base_cm, |(_, g)| {
                    if c.x != n.x {
                        g[0][i]
                    } else {
                        g[2][i]
                    }
                });
                if av != bv {
                    return Err(error(
                        "E_SEAM",
                        format!("terrain boundary differs at {c:?} / {n:?}"),
                    ));
                }
            }
        }
    }
    Ok(())
}
fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(io)?;
    if file.metadata().map_err(io)?.len() > limit {
        return Err(error("E_LIMIT", "file exceeds supported profile"));
    }
    let mut bytes = vec![];
    file.take(limit + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > limit {
        return Err(error("E_LIMIT", "file grew beyond limit"));
    }
    Ok(bytes)
}
pub fn read_project(path: &Path) -> Result<(MapDocument, BTreeMap<String, Vec<u8>>)> {
    let root = path.canonicalize().map_err(io)?;
    let doc_path = root.join("document.json").canonicalize().map_err(io)?;
    if !doc_path.starts_with(&root) {
        return Err(error("E_PATH", "document symlink escapes project"));
    }
    let mut d: MapDocument = json(&bounded_read(&doc_path, MAX_DOCUMENT_BYTES)?)?;
    d.normalize();
    d.validate()?;
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for p in references(&d)? {
        let actual = root.join(&p).canonicalize().map_err(io)?;
        if !actual.starts_with(&root) {
            return Err(error("E_PATH", "reference escapes project"));
        }
        let bytes = if p == "document.json" {
            canonical(&d)?
        } else {
            bounded_read(&actual, MAX_ENTRY_BYTES)?
        };
        total += bytes.len() as u64;
        if total > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "project exceeds expanded byte budget"));
        }
        files.insert(p, bytes);
    }
    validate_assets(&d, &files)?;
    validate_heightmaps(&d, &files)?;
    Ok((d, files))
}
pub fn pack_bytes(mut d: MapDocument, mut files: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    d.normalize();
    d.validate()?;
    files.insert("document.json".into(), canonical(&d)?);
    if files.keys().cloned().collect::<BTreeSet<_>>() != references(&d)? {
        return Err(error("E_REFERENCE", "unexpected or missing file"));
    }
    if files.len() > MAX_FILES
        || files.values().any(|b| b.len() as u64 > MAX_ENTRY_BYTES)
        || files.values().map(|b| b.len() as u64).sum::<u64>() > MAX_EXPANDED_BYTES
    {
        return Err(error("E_LIMIT", "package input exceeds limits"));
    }
    validate_assets(&d, &files)?;
    validate_heightmaps(&d, &files)?;
    let manifest = PackageManifest {
        format: "memap".into(),
        format_version: PACKAGE_VERSION,
        recipe_version: RECIPE_VERSION,
        generated_version: GENERATED_VERSION,
        map_id: d.map_id.clone(),
        revision: d.revision,
        bounds: d.bounds.clone(),
        cell_size_cm: d.cell_size_cm,
        seed: d.seed,
        theme: d.theme.clone(),
        document: "document.json".into(),
        files: files
            .iter()
            .map(|(p, b)| FileRecord {
                path: p.clone(),
                size: b.len() as u64,
                sha256: sha256(b),
            })
            .collect(),
        assets: d.assets.clone(),
        attributions: d.attributions.clone(),
        provenance: d.provenance.clone(),
        world_content_hash: content_hash(&d, &files)?,
    };
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    writer
        .start_file("manifest.json", options)
        .map_err(zip_error)?;
    writer.write_all(&canonical(&manifest)?).map_err(io)?;
    for (path, bytes) in files {
        writer.start_file(path, options).map_err(zip_error)?;
        writer.write_all(&bytes).map_err(io)?;
    }
    let bytes = writer.finish().map_err(zip_error)?.into_inner();
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(error("E_LIMIT", "compressed package exceeds profile"));
    }
    Ok(bytes)
}
pub fn read(path: &Path) -> Result<Package> {
    read_bytes(&bounded_read(path, MAX_PACKAGE_BYTES)?)
}
pub fn read_bytes(bytes: &[u8]) -> Result<Package> {
    read_bytes_with_budget(bytes, u64::MAX)
}
/// Reject conservative validation working-set estimates before inflating payloads.
/// This policy limit supplements, and never relaxes, the format's hard limits.
pub fn read_bytes_with_budget(bytes: &[u8], memory_limit: u64) -> Result<Package> {
    // Bound central-directory setup too, before ZipArchive allocates its index.
    if bytes.len() as u64 * 4 + 8 * 1024 * 1024 > memory_limit {
        return Err(error(
            "E_MEMORY_BUDGET",
            "package index exceeds memory allowance",
        ));
    }
    let cost = inspect_read_cost(bytes)?;
    if cost.validation_peak_bytes > memory_limit {
        return Err(error(
            "E_MEMORY_BUDGET",
            "package validation exceeds memory allowance",
        ));
    }
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(error("E_LIMIT", "compressed package exceeds profile"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    if archive.is_empty() || archive.len() > MAX_FILES + 1 {
        return Err(error("E_LIMIT", "invalid ZIP file count"));
    }
    let mut files = BTreeMap::new();
    let mut folded = BTreeSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_error)?;
        let name = entry.name().to_string();
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
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| error("E_LIMIT", "expanded size overflow"))?;
        if entry.size() > limit || total > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "expanded package exceeds profile"));
        }
        if !matches!(
            entry.compression(),
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            return Err(error("E_ZIP", "unsupported compression"));
        }
        let expected = entry.size();
        let mut data = vec![];
        (&mut entry)
            .take(expected + 1)
            .read_to_end(&mut data)
            .map_err(zip_error)?;
        if data.len() as u64 != expected || data.len() as u64 > limit {
            return Err(error("E_LIMIT", "ZIP decompressed size mismatch"));
        }
        files.insert(name, data);
    }
    let manifest: PackageManifest = json(&files.remove("manifest.json").unwrap())?;
    if manifest.format != "memap"
        || manifest.format_version != PACKAGE_VERSION
        || manifest.recipe_version != RECIPE_VERSION
        || manifest.generated_version != GENERATED_VERSION
    {
        return Err(error("E_VERSION", "unsupported package contract"));
    }
    if manifest.document != "document.json" {
        return Err(error("E_MANIFEST", "unsupported document path"));
    }
    let document: MapDocument = json(
        files
            .get("document.json")
            .ok_or_else(|| error("E_REFERENCE", "missing document"))?,
    )?;
    document.validate()?;
    let mut records = BTreeSet::new();
    for f in &manifest.files {
        if !records.insert(f.path.clone()) {
            return Err(error("E_REFERENCE", "duplicate manifest file"));
        }
        let data = files
            .get(&f.path)
            .ok_or_else(|| error("E_REFERENCE", "missing manifest file"))?;
        if data.len() as u64 != f.size || sha256(data) != f.sha256 {
            return Err(error(
                "E_HASH",
                format!("file hash or size mismatch: {}", f.path),
            ));
        }
    }
    if records != references(&document)? || records != files.keys().cloned().collect() {
        return Err(error("E_REFERENCE", "unlisted or missing payload"));
    }
    if manifest.map_id != document.map_id
        || manifest.revision != document.revision
        || manifest.bounds != document.bounds
        || manifest.cell_size_cm != document.cell_size_cm
        || manifest.seed != document.seed
        || manifest.theme != document.theme
        || manifest.assets != document.assets
        || manifest.attributions != document.attributions
        || manifest.provenance != document.provenance
    {
        return Err(error("E_MANIFEST", "manifest and document disagree"));
    }
    if manifest.world_content_hash != content_hash(&document, &files)? {
        return Err(error("E_HASH", "world content hash mismatch"));
    }
    validate_assets(&document, &files)?;
    validate_heightmaps(&document, &files)?;
    let asset_paths: BTreeSet<_> = document.assets.iter().map(|a| a.path.as_str()).collect();
    let user_asset_bytes = files
        .iter()
        .filter(|(p, _)| asset_paths.contains(p.as_str()))
        .map(|(_, b)| b.len() as u64)
        .sum();
    let inspection = Inspection {
        package_sha256: sha256(bytes),
        world_content_hash: manifest.world_content_hash.clone(),
        package_bytes: bytes.len() as u64,
        expanded_bytes: total,
        base_data_bytes: total - user_asset_bytes,
        user_asset_bytes,
        cell_count: document.cells().len(),
        retained_memory_bytes: cost.retained_memory_bytes,
        validation_peak_bytes: cost.validation_peak_bytes,
    };
    Ok(Package {
        manifest,
        document,
        files,
        inspection,
    })
}
impl Package {
    fn generation_heightgrid(&self, cell: Cell) -> Result<Option<HeightGrid>> {
        self.document.heightmaps.iter().find(|h| h.cell == cell)
            .map(|h| decode_heightmap(h, self.document.cell_size_cm, &self.files[&h.path]))
            .transpose()
    }
    pub fn generate(&self, cell: Cell, max_triangles: usize) -> Result<GeneratedChunk> {
        let grid = self.generation_heightgrid(cell)?;
        generate(GenerationInput {
            document: &self.document, cell, heightgrid: grid.as_ref(), max_triangles,
        })
    }
    /// Uses the identical package terrain decoder and generator as ordinary chunks.
    pub fn generate_with_occupancy(&self, cell: Cell, max_triangles: usize, max_solids: usize) -> Result<GeneratedOccupancy> {
        if max_solids > MAX_OCCUPIED_SOLIDS {
            return Err(error("E_BUDGET", "occupancy limit exceeds 200000 solids"));
        }
        let grid = self.generation_heightgrid(cell)?;
        mapkit_core::generate_with_occupancy(GenerationInput {
            document: &self.document, cell, heightgrid: grid.as_ref(), max_triangles,
        }, max_solids)
    }
}

/// No overwrite: caller must choose a new output. A temporary sibling is never a valid package.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io)?;
    let temp = temporary(parent, "file")?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(io)?;
    let result = (|| {
        f.write_all(bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
        // A hard link installs atomically and cannot replace an existing destination.
        fs::hard_link(&temp, path).map_err(io)?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}
fn temporary(parent: &Path, kind: &str) -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    Ok(parent.join(format!(
        ".mapkit-{kind}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )))
}
pub fn unpack(package: &Package, destination: &Path) -> Result<()> {
    // Reserve destination with create_dir, never walk or overwrite an existing path.
    fs::create_dir(destination).map_err(io)?;
    let result = (|| {
        for (path, bytes) in &package.files {
            let out = destination.join(path);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(io)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)
                .map_err(io)?;
            file.write_all(bytes).map_err(io)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}
