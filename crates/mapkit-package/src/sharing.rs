//! A selected public course, its existing evidence and optional preview in one v1 package.
use super::*;
use mapkit_core::course::Course;
/// Conservative additional worker allowance, held until all temporary owners drain.
pub fn work_bytes(inspection: &Inspection) -> u64 {
    inspection.validation_peak_bytes.saturating_mul(2)
        .saturating_add(inspection.retained_memory_bytes.saturating_mul(3))
        .saturating_add(inspection.expanded_bytes.saturating_mul(4))
        .saturating_add(inspection.package_bytes.saturating_mul(4))
        .saturating_add(64 * 1024 * 1024)
}
pub fn course_bytes(package: &Package, course: Course, evidence: Option<Vec<u8>>, preview: Option<Vec<u8>>) -> Result<Vec<u8>> {
    mapkit_core::cancellation::checkpoint()?;
    course.validate(&package.inspection.world_content_hash, &package.document.bounds)?;
    let mut files: BTreeMap<String,Vec<u8>>=package.files.iter()
        .filter(|(path,_)| !path.starts_with("course-validation/"))
        .map(|(path,bytes)|(path.clone(),bytes.clone())).collect();
    if let Some(reference)=&course.validation {
        let bytes=evidence.or_else(||package.files.get(&reference.path).cloned())
            .ok_or_else(||error("E_REFERENCE","selected course completion evidence is missing"))?;
        if bytes.len()!=reference.bytes as usize || sha256(&bytes)!=reference.sha256 {
            return Err(error("E_HASH","selected course completion evidence differs"));
        }
        files.insert(reference.path.clone(),bytes);
    } else if evidence.is_some() { return Err(error("E_REFERENCE","unreferenced completion evidence")); }
    if let Some(bytes)=preview { preview::inspect(&bytes)?; files.insert(preview::PATH.into(),bytes); }
    let mut document=package.document.to_document();
    document.courses=vec![course];
    let bytes=pack_inner(document,files,false)?;
    let checked=read_bytes(&bytes)?;
    if checked.inspection.world_content_hash!=package.inspection.world_content_hash {
        return Err(error("E_HASH","sharing changed driving content"));
    }
    Ok(bytes)
}
