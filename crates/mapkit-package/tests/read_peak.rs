//! Measured allocator peak of reading a package versus the planned
//! `validation_peak_bytes`, for adversarial JSON shapes and (optionally) local
//! packages named in MAPKIT_PEAK_PACKAGES (comma separated; never committed).
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            if size > layout.size() {
                let now = CURRENT.fetch_add(size - layout.size(), Ordering::Relaxed) + size - layout.size();
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                CURRENT.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        q
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// (measured peak above the starting allocation, planned peak, read ok)
fn measure(bytes: &[u8]) -> (u64, u64, bool) {
    let (peak, _, planned, _, ok) = measure_all(bytes);
    (peak, planned, ok)
}

/// (peak, retained while the package lives, planned peak, planned retained, ok)
fn measure_all(bytes: &[u8]) -> (u64, u64, u64, u64, bool) {
    let cost = mapkit_package::inspect_read_cost(bytes).ok();
    let start = CURRENT.load(Ordering::Relaxed);
    PEAK.store(start, Ordering::Relaxed);
    let result = mapkit_package::read_bytes(bytes);
    let peak = PEAK.load(Ordering::Relaxed) - start;
    let retained = CURRENT.load(Ordering::Relaxed).saturating_sub(start);
    let ok = result.is_ok();
    drop(result);
    (peak as u64, retained as u64, cost.as_ref().map_or(0, |c| c.validation_peak_bytes), cost.as_ref().map_or(0, |c| c.retained_memory_bytes), ok)
}

/// The minimal example package with its document replaced: the valid manifest
/// keeps the reader going until the document itself is parsed.
fn package(document: &[u8]) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/minimal");
    let (doc, files) = mapkit_package::read_project(&root).unwrap();
    let original = std::io::Cursor::new(mapkit_package::pack_bytes(doc, files).unwrap());
    let mut archive = zip::ZipArchive::new(original).unwrap();
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
            zip.start_file(name.as_str(), options).unwrap();
            zip.write_all(if name == "document.json" { document } else { &data }).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

/// JSON shapes that maximise parser allocation per byte within the document limit.
fn adversarial(size: usize) -> Vec<(&'static str, Vec<u8>)> {
    let repeat = |item: &str, open: &str, close: &str| {
        let mut s = String::with_capacity(size + 16);
        s.push_str(open);
        while s.len() + item.len() + close.len() < size {
            if s.len() > open.len() { s.push(','); }
            s.push_str(item);
        }
        s.push_str(close);
        s.into_bytes()
    };
    vec![
        ("one-key objects", repeat(r#"{"a":0}"#, "[", "]")),
        ("empty objects", repeat("{}", "[", "]")),
        ("empty arrays", repeat("[]", "[", "]")),
        ("small integers", repeat("0", "[", "]")),
        ("integer triples", repeat("[0,0,0]", "[", "]")),
        ("nested pairs", repeat("[[0],[0]]", "[", "]")),
        ("short strings", repeat(r#""a""#, "[", "]")),
        ("wide object", {
            let mut s = String::from("{");
            let mut i = 0usize;
            while s.len() + 16 < size {
                if i > 0 { s.push(','); }
                s.push_str(&format!("\"{i:x}\":0"));
                i += 1;
            }
            s.push('}');
            s.into_bytes()
        }),
    ]
}

#[test]
fn planned_read_peak_bounds_adversarial_json_shapes() {
    for (name, document) in adversarial(4 * 1024 * 1024) {
        let bytes = package(&document);
        let (peak, planned, _) = measure(&bytes);
        println!("ADVERSARIAL {name}: json={} measured={} planned={} ratio={:.2}", document.len(), peak, planned, planned as f64 / peak.max(1) as f64);
        assert!(planned >= peak, "{name}: planned {planned} < measured {peak}");
    }
}

#[test]
fn measured_local_packages() {
    let Ok(paths) = std::env::var("MAPKIT_PEAK_PACKAGES") else { return };
    for path in paths.split(',') {
        let bytes = std::fs::read(path).unwrap();
        let (peak, retained, planned, planned_retained, ok) = measure_all(&bytes);
        println!("PACKAGE {path}: ok={ok} measured={peak} planned={planned} ratio={:.2} retained={retained} planned_retained={planned_retained}", planned as f64 / peak.max(1) as f64);
        assert!(ok && planned >= peak && planned_retained >= retained);
    }
}
