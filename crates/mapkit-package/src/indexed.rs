//! Opt-in seekable source artifact. No engine, transport or implicit resident cache.
use super::*;
use flate2::{write::DeflateEncoder, Compression};
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const MAGIC: &[u8; 8] = b"MKREGN01";
const HEADER: u64 = 48;
pub const MAX_REGIONS: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub offset: u64,
    pub compressed_bytes: u64,
    pub size: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionRecord {
    pub cells: CellRegion,
    pub source: usize,
    pub payloads: BTreeMap<String, usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub version: u32,
    /// Metadata-only document. It contains no geometry or resource references.
    pub world: MapDocument,
    pub world_content_hash: String,
    pub side_cells: u32,
    pub authoring_source: usize,
    pub payloads: BTreeMap<String, usize>,
    pub regions: Vec<RegionRecord>,
    pub records: Vec<Record>,
    pub payload_bytes: u64,
}

/// Caller-owned generation. Advancing it invalidates every older ticket, including
/// results that finished before cancellation but have not yet been committed.
#[derive(Clone, Default)]
pub struct ReadEpoch(Arc<AtomicU64>);
#[derive(Clone)]
pub struct ReadTicket {
    epoch: ReadEpoch,
    generation: u64,
}
impl ReadEpoch {
    pub fn begin(&self) -> ReadTicket {
        let generation = self.0.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        ReadTicket {
            epoch: self.clone(),
            generation,
        }
    }
    pub fn cancel(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    pub fn ticket(&self, generation: u64) -> ReadTicket {
        ReadTicket {
            epoch: self.clone(),
            generation,
        }
    }
}
impl ReadTicket {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn check(&self) -> Result<()> {
        if self.epoch.0.load(Ordering::SeqCst) == self.generation {
            Ok(())
        } else {
            Err(error("E_CANCELLED", "stale or cancelled region read"))
        }
    }
}

pub struct RegionSnapshot {
    pub package: Package,
    pub cells: CellRegion,
    pub identity: String,
    pub bytes_read: u64,
    ticket: ReadTicket,
}
impl RegionSnapshot {
    /// Consumers check this again at their serialized commit boundary. Existing
    /// live snapshots remain usable after a later request starts.
    pub fn check_candidate(&self) -> Result<()> {
        self.ticket.check()
    }
}

pub struct IndexedReader<R: Read + Seek> {
    reader: R,
    index: Index,
    identity: String,
    payload_start: u64,
    index_retained: u64,
    artifact_length: u64,
}
fn bad(message: &str) -> Error {
    error("E_INDEX", message)
}
fn hash_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn metadata(d: &MapDocument) -> MapDocument {
    let mut m = d.clone();
    m.nodes.clear();
    m.roads.clear();
    m.surface_areas.clear();
    m.buildings.clear();
    m.zones.clear();
    m.placements.clear();
    m.repetitions.clear();
    m.assets.clear();
    m.heightmaps.clear();
    m
}
fn regions(d: &MapDocument, side: u32) -> Result<Vec<CellRegion>> {
    if !(1..=128).contains(&side) {
        return Err(bad("region side must be 1..128 cells"));
    }
    // The metadata-only document is safe to validate without enumerating cells.
    d.validate_source()?;
    let last = d
        .cell_at(d.bounds.max)
        .ok_or_else(|| bad("invalid world bounds"))?;
    let nx = (last.x as u64 + 1).div_ceil(side as u64);
    let ny = (last.y as u64 + 1).div_ceil(side as u64);
    if nx * ny > MAX_REGIONS as u64 {
        return Err(error("E_LIMIT", "too many storage regions"));
    }
    let mut out = Vec::new();
    for y in 0..ny as i32 {
        for x in 0..nx as i32 {
            let min = Cell {
                x: x * side as i32,
                y: y * side as i32,
            };
            out.push(CellRegion {
                min,
                end: Cell {
                    x: (min.x + side as i32).min(last.x + 1),
                    y: (min.y + side as i32).min(last.y + 1),
                },
            });
        }
    }
    Ok(out)
}
impl Index {
    fn validate(&self, length: u64, payload_start: u64) -> Result<()> {
        if self.version != 1 {
            return Err(error("E_VERSION", "unsupported indexed source version"));
        }
        if canonical(&metadata(&self.world))? != canonical(&self.world)?
            || !hash_valid(&self.world_content_hash)
        {
            return Err(bad("invalid world metadata"));
        }
        let expected = regions(&self.world, self.side_cells)?;
        if self.regions.len() != expected.len()
            || self.records.len() > MAX_REGIONS + MAX_FILES + 1
            || self.records.is_empty()
            || self.payloads.len() >= MAX_FILES
            || payload_start.checked_add(self.payload_bytes) != Some(length)
            || length > MAX_PACKAGE_BYTES
        {
            return Err(bad("record count, topology or artifact length mismatch"));
        }
        let mut offset = 0u64;
        let mut expanded = 0u64;
        for r in &self.records {
            if r.offset != offset
                || r.size > MAX_ENTRY_BYTES
                || !hash_valid(&r.sha256)
                || r.compressed_bytes == 0
                || r.compressed_bytes > MAX_PACKAGE_BYTES
            {
                return Err(bad("invalid record span, size or hash"));
            }
            offset = offset
                .checked_add(r.compressed_bytes)
                .ok_or_else(|| bad("offset overflow"))?;
            expanded = expanded
                .checked_add(r.size)
                .ok_or_else(|| bad("size overflow"))?;
        }
        if offset != self.payload_bytes || expanded > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "indexed payload exceeds profile"));
        }
        let mut used = BTreeSet::new();
        let source = |id: usize| -> Result<()> {
            if self
                .records
                .get(id)
                .is_none_or(|r| r.size == 0 || r.size > MAX_DOCUMENT_BYTES)
            {
                Err(bad("invalid source reference"))
            } else {
                Ok(())
            }
        };
        source(self.authoring_source)?;
        used.insert(self.authoring_source);
        let mut folded = BTreeSet::new();
        for (path, id) in &self.payloads {
            if !safe_path(path)
                || matches!(path.as_str(), "document.json" | "manifest.json")
                || !folded.insert(path.to_ascii_lowercase())
                || *id >= self.records.len()
                || !used.insert(*id)
            {
                return Err(bad("unsafe or aliased payload reference"));
            }
        }
        for (r, expected) in self.regions.iter().zip(expected) {
            source(r.source)?;
            if r.cells != expected || !used.insert(r.source) || r.payloads.len() >= MAX_FILES {
                return Err(bad("duplicate or misplaced region"));
            }
            for (path, id) in &r.payloads {
                if self.payloads.get(path) != Some(id) {
                    return Err(bad("region payload does not match global inventory"));
                }
            }
        }
        if used.len() != self.records.len() {
            return Err(bad("unreferenced record"));
        }
        Ok(())
    }
}
impl<R: Read + Seek> IndexedReader<R> {
    /// Reads exactly the fixed header and bounded front index, no source/payload.
    pub fn open(mut reader: R, memory_limit: u64, expected_index: Option<&str>) -> Result<Self> {
        if memory_limit < 8 * 1024 * 1024 {
            return Err(error("E_MEMORY_BUDGET", "index bootstrap allowance"));
        }
        let length = reader.seek(SeekFrom::End(0)).map_err(io)?;
        if !(HEADER..=MAX_PACKAGE_BYTES).contains(&length) {
            return Err(error("E_LIMIT", "indexed artifact length"));
        }
        reader.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut header = [0; HEADER as usize];
        reader.read_exact(&mut header).map_err(io)?;
        if &header[..8] != MAGIC {
            return Err(error("E_VERSION", "not MKREGN01"));
        }
        let n = u64::from_le_bytes(header[8..16].try_into().unwrap());
        if n == 0 || n > MAX_MANIFEST_BYTES || HEADER + n > length {
            return Err(bad("index size"));
        }
        if n * 128 + 8 * 1024 * 1024 > memory_limit {
            return Err(error("E_MEMORY_BUDGET", "index parsing exceeds allowance"));
        }
        let mut bytes = vec![0; n as usize];
        reader.read_exact(&mut bytes).map_err(io)?;
        let digest = Sha256::digest(&bytes);
        let identity = format!("{digest:x}");
        if digest[..] != header[16..48] || expected_index.is_some_and(|s| s != identity) {
            return Err(error("E_HASH", "index digest mismatch"));
        }
        let mut index: Index = json(&bytes)?;
        index.world = index.world.into_indexed_source()?;
        index.validate(length, HEADER + n)?;
        Ok(Self {
            reader,
            index,
            identity,
            payload_start: HEADER + n,
            index_retained: n * 32 + 1024 * 1024,
            artifact_length: length,
        })
    }
    pub fn index(&self) -> &Index {
        &self.index
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn index_retained_bytes(&self) -> u64 {
        self.index_retained
    }
    pub fn region_for_cell(&self, cell: Cell) -> Result<usize> {
        if !self.index.world.has_cell(cell) {
            return Err(error("E_CELL", "cell outside indexed world"));
        }
        let last = self
            .index
            .world
            .cell_at(self.index.world.bounds.max)
            .unwrap();
        let nx = (last.x as u64 + 1).div_ceil(self.index.side_cells as u64);
        Ok((cell.y as u64 / self.index.side_cells as u64 * nx
            + cell.x as u64 / self.index.side_cells as u64) as usize)
    }
    pub fn region_cost(&self, id: usize) -> Result<ReadCost> {
        let r = self
            .index
            .regions
            .get(id)
            .ok_or_else(|| error("E_CELL", "unknown region"))?;
        Ok(self.cost(r.source, &r.payloads))
    }
    fn cost(&self, source: usize, payloads: &BTreeMap<String, usize>) -> ReadCost {
        let mut expanded = self.index.records[source].size;
        let mut structured = expanded;
        let mut compressed = self.index.records[source].compressed_bytes;
        let mut pngs = 0;
        for (path, id) in payloads {
            let r = &self.index.records[*id];
            expanded += r.size;
            compressed = compressed.max(r.compressed_bytes);
            if path.ends_with(".glb") {
                structured += r.size;
            }
            if path.ends_with(".png") {
                pngs += 1;
            }
        }
        let retained = self.index_retained
            + expanded * 2
            + structured * 32
            + (payloads.len() as u64 + 1) * 4096
            + 16_384 * 256;
        let decoder = if payloads.is_empty() { 8 } else { 256 } * 1024 * 1024;
        ReadCost {
            retained_memory_bytes: retained,
            validation_peak_bytes: retained
                + structured * 96
                + compressed * 2
                + decoder
                + pngs * (4 * 513 * 8 + 256),
        }
    }
    fn record(&mut self, id: usize, ticket: &ReadTicket) -> Result<Vec<u8>> {
        ticket.check()?;
        if self.reader.seek(SeekFrom::End(0)).map_err(io)? != self.artifact_length {
            return Err(bad("artifact length changed after index capture"));
        }
        let r = &self.index.records[id];
        self.reader
            .seek(SeekFrom::Start(self.payload_start + r.offset))
            .map_err(io)?;
        let mut compressed = vec![0; r.compressed_bytes as usize];
        // Short bounded reads permit cancellation without a whole-record read_exact.
        for block in compressed.chunks_mut(64 * 1024) {
            ticket.check()?;
            self.reader.read_exact(block).map_err(io)?;
        }
        let mut decoder = flate2::Decompress::new(false);
        let mut out = Vec::new();
        let mut block = [0; 8192];
        loop {
            ticket.check()?;
            let (before_in, before_out) = (decoder.total_in(), decoder.total_out());
            let status = decoder
                .decompress(
                    &compressed[before_in as usize..],
                    &mut block,
                    flate2::FlushDecompress::None,
                )
                .map_err(zip_error)?;
            if decoder.total_out() > r.size {
                return Err(error("E_LIMIT", "record expanded beyond declaration"));
            }
            out.extend_from_slice(&block[..(decoder.total_out() - before_out) as usize]);
            if status == flate2::Status::StreamEnd {
                if decoder.total_in() != r.compressed_bytes {
                    return Err(bad("compressed record trailing bytes"));
                }
                break;
            }
            if (before_in, before_out) == (decoder.total_in(), decoder.total_out()) {
                return Err(bad("truncated compressed record"));
            }
        }
        ticket.check()?;
        if out.len() as u64 != r.size || sha256(&out) != r.sha256 {
            return Err(error("E_HASH", "record size/hash mismatch"));
        }
        Ok(out)
    }
    fn source_files(
        &mut self,
        source: usize,
        payloads: &BTreeMap<String, usize>,
        ticket: &ReadTicket,
    ) -> Result<(MapDocument, BTreeMap<String, Vec<u8>>)> {
        let bytes = self.record(source, ticket)?;
        let document: MapDocument = json::<MapDocument>(&bytes)?.into_indexed_source()?;
        if canonical(&metadata(&document))? != canonical(&self.index.world)? {
            return Err(bad("source disagrees with indexed world"));
        }
        let mut files = BTreeMap::from([("document.json".into(), bytes)]);
        if references(&document)?
            != payloads
                .keys()
                .cloned()
                .chain(["document.json".into()])
                .collect()
        {
            return Err(error("E_REFERENCE", "region inventory/source mismatch"));
        }
        for (path, id) in payloads {
            files.insert(path.clone(), self.record(*id, ticket)?);
        }
        ticket.check()?;
        Ok((document, files))
    }
    pub fn load_region(
        &mut self,
        id: usize,
        memory_limit: u64,
        ticket: &ReadTicket,
    ) -> Result<RegionSnapshot> {
        ticket.check()?;
        let cost = self.region_cost(id)?;
        if cost.validation_peak_bytes > memory_limit {
            return Err(error(
                "E_MEMORY_BUDGET",
                "region validation exceeds allowance",
            ));
        }
        let region = self.index.regions[id].clone();
        let (d, files) = self.source_files(region.source, &region.payloads, ticket)?;
        let document = PreparedMap::new_region(d, region.cells)?;
        ticket.check()?;
        validate_assets(&document, &files)?;
        ticket.check()?;
        validate_heightmaps_region(&document, &files, Some(region.cells))?;
        ticket.check()?;
        let identity = sha256(&canonical(&(
            "MKREGN01",
            &self.identity,
            region.cells,
            &self.index.records[region.source].sha256,
            GENERATED_VERSION,
        ))?);
        let records: Vec<FileRecord> = files
            .iter()
            .map(|(path, bytes)| FileRecord {
                path: path.clone(),
                size: bytes.len() as u64,
                sha256: sha256(bytes),
            })
            .collect();
        let expanded_bytes: u64 = records.iter().map(|r| r.size).sum();
        let asset_paths: BTreeSet<_> = document.assets.iter().map(|a| &a.path).collect();
        let user_asset_bytes: u64 = records
            .iter()
            .filter(|r| asset_paths.contains(&r.path))
            .map(|r| r.size)
            .sum();
        let bytes_read = std::iter::once(region.source)
            .chain(region.payloads.values().copied())
            .map(|i| self.index.records[i].compressed_bytes)
            .sum();
        let manifest = PackageManifest {
            format: "mkregions-source".into(),
            format_version: 1,
            recipe_version: document.recipe_version,
            generated_version: GENERATED_VERSION,
            map_id: document.map_id.clone(),
            revision: document.revision,
            bounds: document.bounds.clone(),
            cell_size_cm: document.cell_size_cm,
            seed: document.seed,
            theme: document.theme.clone(),
            document: "document.json".into(),
            files: records,
            assets: document.assets.clone(),
            attributions: document.attributions.clone(),
            provenance: document.provenance.clone(),
            world_content_hash: self.index.world_content_hash.clone(),
        };
        let inspection = Inspection {
            package_sha256: identity.clone(),
            world_content_hash: self.index.world_content_hash.clone(),
            package_bytes: bytes_read,
            expanded_bytes,
            base_data_bytes: expanded_bytes - user_asset_bytes,
            user_asset_bytes,
            cell_count: region.cells.cells()?.len(),
            retained_memory_bytes: cost.retained_memory_bytes,
            validation_peak_bytes: cost.validation_peak_bytes,
        };
        ticket.check()?;
        Ok(RegionSnapshot {
            package: Package {
                manifest,
                document,
                files,
                inspection,
            },
            cells: region.cells,
            identity,
            bytes_read,
            ticket: ticket.clone(),
        })
    }
    /// Explicit full audit, separate from partial open/load. Reconstruct the
    /// authoring source and prove every regional closure matches the producer rule.
    pub fn audit(&mut self, memory_limit: u64, ticket: &ReadTicket) -> Result<()> {
        self.audited_files(memory_limit, ticket).map(|_| ())
    }
    /// Recover the canonical authored source and all original payloads, including
    /// unused declared assets. Destination must not already exist.
    pub fn unpack_source(
        &mut self,
        destination: &Path,
        memory_limit: u64,
        ticket: &ReadTicket,
    ) -> Result<()> {
        let files = self.audited_files(memory_limit, ticket)?;
        ticket.check()?;
        unpack_files(&files, destination)
    }
    fn audited_files(
        &mut self,
        memory_limit: u64,
        ticket: &ReadTicket,
    ) -> Result<BTreeMap<String, Vec<u8>>> {
        let source = self.index.authoring_source;
        let payloads = self.index.payloads.clone();
        let cost = self.cost(source, &payloads);
        let max_source = self
            .index
            .regions
            .iter()
            .map(|r| self.index.records[r.source].size)
            .max()
            .unwrap_or(0);
        if cost.validation_peak_bytes + max_source * 128 > memory_limit {
            return Err(error("E_MEMORY_BUDGET", "whole audit exceeds allowance"));
        }
        let (d, files) = self.source_files(source, &payloads, ticket)?;
        d.validate_source()?;
        validate_assets(&d, &files)?;
        validate_heightmaps(&d, &files)?;
        if content_hash(&d, &files)? != self.index.world_content_hash {
            return Err(error("E_HASH", "authoring world identity mismatch"));
        }
        for r in self.index.regions.clone() {
            ticket.check()?;
            let expected = region_source(&d, r.cells)?;
            if self.record(r.source, ticket)? != canonical(&expected)? {
                return Err(error(
                    "E_REFERENCE",
                    "regional source differs from complete dependency closure",
                ));
            }
            let expected_paths = references(&expected)?;
            if r.payloads
                .keys()
                .cloned()
                .chain(["document.json".into()])
                .collect::<BTreeSet<_>>()
                != expected_paths
            {
                return Err(error(
                    "E_REFERENCE",
                    "regional dependency inventory mismatch",
                ));
            }
        }
        ticket.check()?;
        Ok(files)
    }
}
impl IndexedReader<File> {
    pub fn open_path(path: &Path, memory_limit: u64, expected_index: Option<&str>) -> Result<Self> {
        Self::open(File::open(path).map_err(io)?, memory_limit, expected_index)
    }
}

/// Explicit indexed-source authoring entry point; never changes legacy read_project.
pub fn read_source_project(path: &Path) -> Result<(MapDocument, BTreeMap<String, Vec<u8>>)> {
    let root = path.canonicalize().map_err(io)?;
    let doc_path = root.join("document.json").canonicalize().map_err(io)?;
    if !doc_path.starts_with(&root) {
        return Err(error("E_PATH", "source escapes project"));
    }
    let mut d: MapDocument = json(&bounded_read(&doc_path, MAX_DOCUMENT_BYTES)?)?;
    d.normalize();
    d = d.into_indexed_source()?;
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for p in references(&d)? {
        let actual = root.join(&p).canonicalize().map_err(io)?;
        if !actual.starts_with(&root) {
            return Err(error("E_PATH", "payload escapes project"));
        }
        let bytes = if p == "document.json" {
            canonical(&d)?
        } else {
            bounded_read(&actual, MAX_ENTRY_BYTES)?
        };
        total += bytes.len() as u64;
        if total > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "authoring source exceeds byte budget"));
        }
        files.insert(p, bytes);
    }
    Ok((d, files))
}

pub fn pack_source(
    mut d: MapDocument,
    mut files: BTreeMap<String, Vec<u8>>,
    side_cells: u32,
) -> Result<Vec<u8>> {
    d.normalize();
    d = d.into_indexed_source()?;
    files.insert("document.json".into(), canonical(&d)?);
    export_limits::payload_size(files.iter().map(|(p, b)| (p.as_str(), b.len() as u64)))?;
    if files.keys().cloned().collect::<BTreeSet<_>>() != references(&d)? {
        return Err(error("E_REFERENCE", "source inventory mismatch"));
    }
    validate_assets(&d, &files)?;
    validate_heightmaps(&d, &files)?;
    let mut index = Index {
        version: 1,
        world: metadata(&d),
        world_content_hash: content_hash(&d, &files)?,
        side_cells,
        authoring_source: 0,
        payloads: BTreeMap::new(),
        regions: vec![],
        records: vec![],
        payload_bytes: 0,
    };
    let mut payload = Vec::new();
    let mut expanded = 0u64;
    fn append(
        index: &mut Index,
        payload: &mut Vec<u8>,
        expanded: &mut u64,
        bytes: &[u8],
    ) -> Result<usize> {
        *expanded += bytes.len() as u64;
        if bytes.len() as u64 > MAX_ENTRY_BYTES || *expanded > MAX_EXPANDED_BYTES {
            return Err(error(
                "E_LIMIT",
                "regional source duplication exceeds expanded profile",
            ));
        }
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(9));
        encoder.write_all(bytes).map_err(io)?;
        let compressed = encoder.finish().map_err(io)?;
        if payload.len() as u64 + compressed.len() as u64 > MAX_PACKAGE_BYTES {
            return Err(error(
                "E_LIMIT",
                "indexed source exceeds compressed profile",
            ));
        }
        let id = index.records.len();
        index.records.push(Record {
            offset: payload.len() as u64,
            compressed_bytes: compressed.len() as u64,
            size: bytes.len() as u64,
            sha256: sha256(bytes),
        });
        payload.extend_from_slice(&compressed);
        Ok(id)
    }
    append(
        &mut index,
        &mut payload,
        &mut expanded,
        &files["document.json"],
    )?;
    for (path, bytes) in &files {
        if path == "document.json" {
            continue;
        }
        let id = append(&mut index, &mut payload, &mut expanded, bytes)?;
        index.payloads.insert(path.clone(), id);
    }
    for cells in regions(&index.world, side_cells)? {
        let source = region_source(&d, cells)?;
        // Region validation has all existing geometry/ownership/work limits.
        source.validate_source()?;
        let bytes = canonical(&source)?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(error("E_LIMIT", "regional document exceeds profile"));
        }
        let payloads = references(&source)?
            .into_iter()
            .filter(|p| p != "document.json")
            .map(|p| {
                let id = index.payloads[&p];
                (p, id)
            })
            .collect();
        let source = append(&mut index, &mut payload, &mut expanded, &bytes)?;
        index.regions.push(RegionRecord {
            cells,
            source,
            payloads,
        });
    }
    index.payload_bytes = payload.len() as u64;
    let bytes = canonical(&index)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(error("E_LIMIT", "regional index exceeds profile"));
    }
    index.validate(
        HEADER + bytes.len() as u64 + index.payload_bytes,
        HEADER + bytes.len() as u64,
    )?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(&bytes));
    out.extend_from_slice(&bytes);
    out.extend_from_slice(&payload);
    Ok(out)
}
