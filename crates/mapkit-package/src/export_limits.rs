use super::*;

// Keep the export admission rules identical to the reader's inclusive limits.
// Counts let boundary tests exercise the 1 GiB total without allocating it.
pub(super) fn payload_size<'a>(files: impl IntoIterator<Item = (&'a str, u64)>) -> Result<u64> {
    let mut total = 0u64;
    for (index, (path, size)) in files.into_iter().enumerate() {
        let limit = if path == "document.json" {
            MAX_DOCUMENT_BYTES
        } else {
            MAX_ENTRY_BYTES
        };
        total = total
            .checked_add(size)
            .ok_or_else(|| error("E_LIMIT", "expanded size overflow"))?;
        if index >= MAX_FILES || size > limit || total > MAX_EXPANDED_BYTES {
            return Err(error("E_LIMIT", "package input exceeds limits"));
        }
    }
    Ok(total)
}

pub(super) fn including_manifest(payload: u64, manifest: u64) -> Result<()> {
    if manifest > MAX_MANIFEST_BYTES
        || payload
            .checked_add(manifest)
            .is_none_or(|total| total > MAX_EXPANDED_BYTES)
    {
        return Err(error(
            "E_LIMIT",
            "manifest or expanded package exceeds limits",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_file_document_count_and_total_boundaries() {
        for (path, limit) in [
            ("document.json", MAX_DOCUMENT_BYTES),
            ("assets/a.png", MAX_ENTRY_BYTES),
        ] {
            assert_eq!(payload_size([(path, limit)]).unwrap(), limit);
            assert_eq!(
                payload_size([(path, limit + 1)]).unwrap_err().code,
                "E_LIMIT"
            );
        }
        assert!(payload_size(std::iter::repeat_n(("payload", 0), MAX_FILES)).is_ok());
        assert!(payload_size(std::iter::repeat_n(("payload", 0), MAX_FILES + 1)).is_err());
        assert_eq!(
            payload_size(std::iter::repeat_n(("payload", MAX_ENTRY_BYTES), 8)).unwrap(),
            MAX_EXPANDED_BYTES
        );
        assert!(payload_size(std::iter::repeat_n(("payload", MAX_ENTRY_BYTES), 9)).is_err());
        assert!(
            including_manifest(MAX_EXPANDED_BYTES - MAX_MANIFEST_BYTES, MAX_MANIFEST_BYTES).is_ok()
        );
        assert!(including_manifest(
            MAX_EXPANDED_BYTES - MAX_MANIFEST_BYTES + 1,
            MAX_MANIFEST_BYTES
        )
        .is_err());
        assert!(including_manifest(0, MAX_MANIFEST_BYTES + 1).is_err());
        assert!(including_manifest(u64::MAX, 1).is_err());
    }
}
