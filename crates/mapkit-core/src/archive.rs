//! Bounded, disposable generated-cell archive. No cache policy or filesystem I/O.
use crate::*;

const MAGIC: &[u8; 8] = b"MKCELL01";
const HEADER: usize = 156;
/// The archive revision must change if generation semantics change without a
/// public recipe/generated version change. Archives are never map source files.
pub fn archive_key(world: &str, cell: Cell) -> String {
    sha256(format!("MKCELL01:{RECIPE_VERSION}:{GENERATED_VERSION}:{world}:{}:{}", cell.x, cell.y).as_bytes())
}
pub fn archive_limit(cost: &GenerationCost) -> u64 {
    let id = cost.max_object_id_bytes.max(128);
    (HEADER as u64).saturating_add(cost.triangles.saturating_mul(78 + id))
        .saturating_add(cost.objects.saturating_mul(33 + 2 * id))
}
fn invalid() -> Error { error("E_ARCHIVE", "invalid, incompatible or damaged generated-cell archive") }
fn string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}
pub fn encode_archive(chunk: &GeneratedChunk, key: &str, max_bytes: u64) -> Result<Vec<u8>> {
    let size = HEADER + chunk.triangles.iter().map(|t| 78 + t.object_id.len()).sum::<usize>()
        + chunk.objects.iter().map(|o| 33 + o.id.len() + o.asset_id.len()).sum::<usize>();
    if size as u64 > max_bytes { return Err(error("E_BUDGET", "cell archive exceeds byte allowance")); }
    if key.len() != 64 || chunk.format_version != GENERATED_VERSION { return Err(invalid()); }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(chunk.hash()?.as_bytes());
    out.extend_from_slice(&chunk.format_version.to_le_bytes());
    out.extend_from_slice(&chunk.cell.x.to_le_bytes());
    out.extend_from_slice(&chunk.cell.y.to_le_bytes());
    out.extend_from_slice(&(chunk.triangles.len() as u32).to_le_bytes());
    out.extend_from_slice(&(chunk.objects.len() as u32).to_le_bytes());
    for t in &chunk.triangles {
        for v in t.vertices.iter().flatten() { out.extend_from_slice(&v.to_le_bytes()); }
        out.push(match t.surface { Surface::Asphalt => 0, Surface::Concrete => 1, Surface::Dirt => 2, Surface::Gravel => 3, Surface::Grass => 4 });
        out.push(u8::from(t.spawnable));
        string(&mut out, &t.object_id);
    }
    for o in &chunk.objects {
        string(&mut out, &o.id); string(&mut out, &o.asset_id);
        for v in o.position { out.extend_from_slice(&v.to_le_bytes()); }
        out.push(o.quarter_turns);
    }
    Ok(out)
}
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(n).ok_or_else(invalid)?;
        let out = self.bytes.get(self.offset..end).ok_or_else(invalid)?;
        self.offset = end; Ok(out)
    }
    fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn vertex(&mut self) -> Result<Vertex> {
        let mut out = [0; 3];
        for v in &mut out {
            *v = i64::from_le_bytes(self.take(8)?.try_into().unwrap());
            if v.unsigned_abs() > 100_000_000 { return Err(invalid()); }
        }
        Ok(out)
    }
    fn string(&mut self, limit: u64) -> Result<String> {
        let n = self.u32()? as usize;
        if n == 0 || n as u64 > limit { return Err(invalid()); }
        std::str::from_utf8(self.take(n)?).map(str::to_owned).map_err(|_| invalid())
    }
}
pub fn decode_archive(bytes: &[u8], key: &str, cell: Cell, cost: &GenerationCost) -> Result<GeneratedChunk> {
    if bytes.len() < HEADER || bytes.len() as u64 > archive_limit(cost) { return Err(invalid()); }
    let mut r = Reader { bytes, offset: 0 };
    if r.take(8)? != MAGIC || r.take(64)? != key.as_bytes() { return Err(invalid()); }
    let hash = r.take(64)?;
    if r.u32()? != GENERATED_VERSION || r.u32()? as i32 != cell.x || r.u32()? as i32 != cell.y { return Err(invalid()); }
    let triangles = r.u32()? as usize;
    let objects = r.u32()? as usize;
    // Validate counts against both trusted source estimates and remaining bytes
    // before allocating. A tiny corrupt file cannot request a huge vector.
    if triangles as u64 > cost.triangles || objects as u64 > cost.objects
        || triangles as u64 * 79 + objects as u64 * 35 > (bytes.len() - HEADER) as u64 { return Err(invalid()); }
    let mut chunk = GeneratedChunk { format_version: GENERATED_VERSION, cell,
        triangles: Vec::with_capacity(triangles), objects: Vec::with_capacity(objects) };
    let id_limit = cost.max_object_id_bytes.max(128);
    for _ in 0..triangles {
        let vertices = [r.vertex()?, r.vertex()?, r.vertex()?];
        let surface = match r.take(1)?[0] { 0 => Surface::Asphalt, 1 => Surface::Concrete, 2 => Surface::Dirt, 3 => Surface::Gravel, 4 => Surface::Grass, _ => return Err(invalid()) };
        let spawnable = match r.take(1)?[0] { 0 => false, 1 => true, _ => return Err(invalid()) };
        chunk.triangles.push(Triangle { vertices, surface, spawnable, object_id: r.string(id_limit)? });
    }
    for _ in 0..objects {
        let id = r.string(id_limit)?;
        let asset_id = r.string(id_limit)?;
        let position = r.vertex()?;
        let quarter_turns = r.take(1)?[0];
        if quarter_turns > 3 { return Err(invalid()); }
        chunk.objects.push(GeneratedObject { id, asset_id, position, quarter_turns });
    }
    if r.offset != bytes.len() || chunk.hash()?.as_bytes() != hash { return Err(invalid()); }
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_roundtrip_corruption_and_allocation_limits() {
        let chunk = GeneratedChunk { format_version: GENERATED_VERSION, cell: Cell { x: -2, y: 3 },
            triangles: vec![Triangle { vertices: [[-1, 0, 0], [1, 0, 0], [0, 1, 1]], surface: Surface::Gravel, object_id: "도로".into(), spawnable: true }],
            objects: vec![GeneratedObject { id: "tree".into(), asset_id: "builtin.tree".into(), position: [1, 2, 3], quarter_turns: 3 }] };
        let cost = GenerationCost { triangles: 1, objects: 1, occupied_solids: 0, height_samples: 0, max_object_id_bytes: 128 };
        let key = archive_key(&"a".repeat(64), chunk.cell);
        let bytes = encode_archive(&chunk, &key, archive_limit(&cost)).unwrap();
        assert_eq!(decode_archive(&bytes, &key, chunk.cell, &cost).unwrap(), chunk);
        assert_eq!(chunk.hash().unwrap(), sha256(&canonical(&chunk).unwrap()));
        assert!(encode_archive(&chunk, &key, bytes.len() as u64 - 1).is_err());
        assert!(decode_archive(&bytes, &"b".repeat(64), chunk.cell, &cost).is_err());
        assert!(decode_archive(&bytes, &key, Cell { x: 0, y: 0 }, &cost).is_err());
        for n in 0..bytes.len() { assert!(decode_archive(&bytes[..n], &key, chunk.cell, &cost).is_err()); }
        for i in 0..bytes.len() {
            let mut bad = bytes.clone(); bad[i] ^= 255;
            assert!(decode_archive(&bad, &key, chunk.cell, &cost).is_err(), "byte {i}");
        }
        let mut trailing = bytes.clone(); trailing.push(0);
        assert!(decode_archive(&trailing, &key, chunk.cell, &cost).is_err());
        let mut tiny_cost = cost; tiny_cost.triangles = 0;
        assert!(decode_archive(&bytes, &key, chunk.cell, &tiny_cost).is_err());
    }
}
