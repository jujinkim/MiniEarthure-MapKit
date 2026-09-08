//! Non-inflating, bounded ZIP envelope validation before the adapter builds an index.
//! No gaps, overlapping records, hidden entries, alternate names or trailing payloads.
use super::*;

fn bad() -> Error {
    error("E_ZIP", "invalid, ambiguous or unsupported ZIP envelope")
}
fn part(b: &[u8], p: usize, n: usize) -> Result<&[u8]> {
    b.get(p..p.checked_add(n).ok_or_else(bad)?).ok_or_else(bad)
}
fn u16le(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(part(b, p, 2)?.try_into().unwrap()))
}
fn u32le(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(part(b, p, 4)?.try_into().unwrap()))
}
fn u64le(b: &[u8], p: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(part(b, p, 8)?.try_into().unwrap()))
}
fn index(n: u64) -> Result<usize> {
    usize::try_from(n).map_err(|_| bad())
}
fn extra(b: &[u8]) -> Result<Option<&[u8]>> {
    let mut p = 0;
    let mut zip64 = None;
    let mut ids = BTreeSet::new();
    while p < b.len() {
        let id = u16le(b, p)?;
        let n = u16le(b, p + 2)? as usize;
        let value = part(b, p + 4, n)?;
        // Unicode filename overrides and encryption extras are not this profile.
        if !ids.insert(id) || matches!(id, 0x7075 | 0x9901 | 0x0017) {
            return Err(bad());
        }
        if id == 1 {
            zip64 = Some(value);
        }
        p += 4 + n;
    }
    Ok(zip64)
}
fn size(raw: u32, extended: Option<&[u8]>, cursor: &mut usize) -> Result<u64> {
    if raw != u32::MAX {
        return Ok(raw as u64);
    }
    let value = u64le(extended.ok_or_else(bad)?, *cursor)?;
    *cursor += 8;
    Ok(value)
}

pub(super) fn inflate(bytes: &[u8], entry: &zip::read::ZipFile<'_>) -> Result<Vec<u8>> {
    let source = part(
        bytes,
        index(entry.data_start())?,
        index(entry.compressed_size())?,
    )?;
    let mut output = Vec::new();
    if entry.compression() == zip::CompressionMethod::Stored {
        output.extend_from_slice(source);
    } else {
        let mut decoder = flate2::Decompress::new(false);
        let mut chunk = [0u8; 8192];
        loop {
            let (before_in, before_out) = (decoder.total_in(), decoder.total_out());
            let status = decoder
                .decompress(
                    &source[index(before_in)?..],
                    &mut chunk,
                    flate2::FlushDecompress::None,
                )
                .map_err(zip_error)?;
            if decoder.total_out() > entry.size() {
                return Err(error("E_LIMIT", "ZIP expanded size exceeds declaration"));
            }
            output.extend_from_slice(&chunk[..index(decoder.total_out() - before_out)?]);
            if status == flate2::Status::StreamEnd {
                if decoder.total_in() != source.len() as u64 {
                    return Err(bad());
                }
                break;
            }
            if (before_in, before_out) == (decoder.total_in(), decoder.total_out()) {
                return Err(bad());
            }
        }
    }
    if output.len() as u64 != entry.size() {
        return Err(error("E_LIMIT", "ZIP expanded size mismatch"));
    }
    if crc32fast::hash(&output) != entry.crc32() {
        return Err(bad());
    }
    Ok(output)
}

pub(super) fn validate(bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(error("E_LIMIT", "compressed package exceeds profile"));
    }
    let end = (bytes.len().saturating_sub(65557)..bytes.len().saturating_sub(21))
        .rev()
        .find(|&p| {
            bytes.get(p..p + 4) == Some(b"PK\x05\x06")
                && u16le(bytes, p + 20).is_ok_and(|n| p + 22 + n as usize == bytes.len())
        })
        .ok_or_else(bad)?;
    if u16le(bytes, end + 4)? != 0
        || u16le(bytes, end + 6)? != 0
        || u16le(bytes, end + 8)? != u16le(bytes, end + 10)?
    {
        return Err(bad());
    }
    let mut count = u16le(bytes, end + 10)? as u64;
    let mut length = u32le(bytes, end + 12)? as u64;
    let mut start = u32le(bytes, end + 16)? as u64;
    let mut directory_end = end;
    if count == 65535
        || length == u32::MAX as u64
        || start == u32::MAX as u64
        || (end >= 20 && bytes.get(end - 20..end - 16) == Some(b"PK\x06\x07"))
    {
        let locator = end.checked_sub(20).ok_or_else(bad)?;
        if part(bytes, locator, 4)? != b"PK\x06\x07"
            || u32le(bytes, locator + 4)? != 0
            || u32le(bytes, locator + 16)? != 1
        {
            return Err(bad());
        }
        let z = index(u64le(bytes, locator + 8)?)?;
        if part(bytes, z, 4)? != b"PK\x06\x06"
            || u64le(bytes, z + 4)? != 44
            || z.checked_add(56) != Some(locator)
            || u32le(bytes, z + 16)? != 0
            || u32le(bytes, z + 20)? != 0
            || u64le(bytes, z + 24)? != u64le(bytes, z + 32)?
        {
            return Err(bad());
        }
        let (zc, zl, zs) = (
            u64le(bytes, z + 32)?,
            u64le(bytes, z + 40)?,
            u64le(bytes, z + 48)?,
        );
        if (count != 65535 && count != zc)
            || (length != u32::MAX as u64 && length != zl)
            || (start != u32::MAX as u64 && start != zs)
        {
            return Err(bad());
        }
        (count, length, start, directory_end) = (zc, zl, zs, z);
    }
    if count == 0 || count > (MAX_FILES + 1) as u64 {
        return Err(error("E_LIMIT", "invalid ZIP file count"));
    }
    if start.checked_add(length) != Some(directory_end as u64) {
        return Err(bad());
    }
    let mut p = index(start)?;
    let mut names = BTreeSet::new();
    let mut spans = Vec::new();
    for i in 0..count {
        let c = part(bytes, p, 46)?;
        if &c[..4] != b"PK\x01\x02" || u16le(c, 34)? != 0 {
            return Err(bad());
        }
        let (flags, method) = (u16le(c, 8)?, u16le(c, 10)?);
        if flags & !0x080e != 0 || !matches!(method, 0 | 8) || (method == 0 && flags & 6 != 0) {
            return Err(bad());
        }
        let (nn, en, cn) = (
            u16le(c, 28)? as usize,
            u16le(c, 30)? as usize,
            u16le(c, 32)? as usize,
        );
        let name = part(bytes, p + 46, nn)?;
        let path = std::str::from_utf8(name).map_err(|_| error("E_PATH", "non-ASCII ZIP path"))?;
        let mode = u32le(c, 38)? >> 16;
        if !safe_path(path)
            || !names.insert(path.to_ascii_lowercase())
            || (c[5] == 3 && mode & 0o170000 != 0 && mode & 0o170000 != 0o100000)
            || u32le(c, 38)? & 0x10 != 0
        {
            return Err(error("E_PATH", "unsafe, duplicate or non-regular ZIP path"));
        }
        let ex = extra(part(bytes, p + 46 + nn, en)?)?;
        let mut cursor = 0;
        let expanded = size(u32le(c, 24)?, ex, &mut cursor)?;
        let compressed = size(u32le(c, 20)?, ex, &mut cursor)?;
        let offset = size(u32le(c, 42)?, ex, &mut cursor)?;
        if ex.is_some_and(|e| e.len() != cursor) {
            return Err(bad());
        }
        let l = index(offset)?;
        if i == 0 && (path != "manifest.json" || l != 0) {
            return Err(error(
                "E_MANIFEST",
                "manifest.json must be first without prefix",
            ));
        }
        let h = part(bytes, l, 30)?;
        if &h[..4] != b"PK\x03\x04" || h[6..10] != c[8..12] {
            return Err(bad());
        }
        let (ln, le) = (u16le(h, 26)? as usize, u16le(h, 28)? as usize);
        if part(bytes, l + 30, ln)? != name {
            return Err(bad());
        }
        let lex = extra(part(bytes, l + 30 + ln, le)?)?;
        let mut lc = 0;
        let lexp = size(u32le(h, 22)?, lex, &mut lc)?;
        let lcomp = size(u32le(h, 18)?, lex, &mut lc)?;
        if lex.is_some_and(|e| e.len() != lc) {
            return Err(bad());
        }
        if flags & 8 == 0 {
            if h[14..18] != c[16..20] || lexp != expanded || lcomp != compressed {
                return Err(bad());
            }
        } else if (u32le(h, 14)? != 0 && h[14..18] != c[16..20])
            || (lexp != 0 && lexp != expanded)
            || (lcomp != 0 && lcomp != compressed)
        {
            return Err(bad());
        }
        let data = l.checked_add(30 + ln + le).ok_or_else(bad)?;
        let mut next = data.checked_add(index(compressed)?).ok_or_else(bad)?;
        if method == 0 && compressed != expanded {
            return Err(bad());
        }
        if flags & 8 != 0 {
            // Descriptor width follows the local ZIP64 size fields, not file size.
            let wide = u32le(h, 18)? == u32::MAX || u32le(h, 22)? == u32::MAX;
            let descriptor = |q: usize| -> Result<usize> {
                if u32le(bytes, q)? != u32le(c, 16)? {
                    return Err(bad());
                }
                let (cs, us, n) = if wide {
                    (u64le(bytes, q + 4)?, u64le(bytes, q + 12)?, 20)
                } else {
                    (u32le(bytes, q + 4)? as u64, u32le(bytes, q + 8)? as u64, 12)
                };
                if cs != compressed || us != expanded {
                    return Err(bad());
                }
                Ok(q + n)
            };
            next = if part(bytes, next, 4)? == b"PK\x07\x08" {
                descriptor(next + 4).or_else(|_| descriptor(next))?
            } else {
                descriptor(next)?
            };
        }
        if next > index(start)? {
            return Err(bad());
        }
        spans.push((l, next));
        p = p.checked_add(46 + nn + en + cn).ok_or_else(bad)?;
        if p > directory_end {
            return Err(bad());
        }
    }
    if p != directory_end {
        return Err(bad());
    }
    spans.sort_unstable();
    let mut next = 0;
    for (a, b) in spans {
        if a != next {
            return Err(bad());
        }
        next = b;
    }
    if next != index(start)? {
        return Err(bad());
    }
    Ok(())
}
