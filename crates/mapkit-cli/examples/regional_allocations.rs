//! Single-threaded indexed full-audit allocation diagnostic.
//!
//! Usage: regional_allocations PACKAGE BUDGET_BYTES
//! Only this opt-in example replaces the Rust allocator. Counts describe live
//! requested Rust heap bytes, excluding allocator metadata, fragmentation, stack,
//! memory maps, external allocations and allocator-internal realloc transients.
//! They are observations, not RSS measurements or validation receipts.
use mapkit_core::{error, Result};
use mapkit_package::indexed::{IndexedReader, ReadEpoch};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct RequestedAllocations;
#[global_allocator]
static ALLOCATOR: RequestedAllocations = RequestedAllocations;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn allocated(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    if ACTIVE.load(Ordering::Relaxed) {
        PEAK.fetch_max(live, Ordering::Relaxed);
    }
}
fn released(bytes: usize) {
    LIVE.fetch_sub(bytes, Ordering::Relaxed);
}

// SAFETY: every allocation is delegated unchanged to System. Counters never
// allocate, format, lock, dereference pointers or panic. Tracking from startup,
// rather than only during ACTIVE, accounts for pre-phase objects freed in-phase.
unsafe impl GlobalAlloc for RequestedAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid nonzero allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout is forwarded with its original alignment.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller provides a live System pointer and its original layout.
        unsafe { System.dealloc(pointer, layout) };
        released(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees a live pointer, its original layout and
        // a valid nonzero new size. A null result leaves the old allocation live.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            if new_size >= layout.size() {
                allocated(new_size - layout.size());
            } else {
                released(layout.size() - new_size);
            }
        }
        resized
    }
}

/// No threads are spawned by this example or its synchronous reader. The
/// process-wide counters include every Rust allocation in this diagnostic, not
/// a guessed subset selected by allocation size or call site.
struct Phase {
    baseline: usize,
}
impl Phase {
    fn begin() -> Self {
        let baseline = LIVE.load(Ordering::Relaxed);
        PEAK.store(baseline, Ordering::Relaxed);
        ACTIVE.store(true, Ordering::Relaxed);
        Self { baseline }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            live: LIVE.load(Ordering::Relaxed),
            peak: PEAK.load(Ordering::Relaxed),
        }
    }
}
impl Drop for Phase {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Relaxed);
    }
}
struct Snapshot {
    live: usize,
    peak: usize,
}

/// Preserve bounded result metadata while dropping the returned heap-owned
/// summary/error. Copying this stack value does not alter measured allocations.
struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
    truncated: bool,
}
impl<const N: usize> Text<N> {
    fn new(value: &str) -> Self {
        let mut len = value.len().min(N);
        while !value.is_char_boundary(len) {
            len -= 1;
        }
        let mut bytes = [0; N];
        bytes[..len].copy_from_slice(&value.as_bytes()[..len]);
        Self {
            bytes,
            len,
            truncated: len < value.len(),
        }
    }
    fn as_str(&self) -> &str {
        // Constructed only from a UTF-8 prefix ending at a character boundary.
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

struct Outcome {
    accepted: bool,
    package_sha256: Text<64>,
    world_content_hash: Text<64>,
    verification: Text<32>,
    error_code: Text<64>,
    error_message: Text<512>,
    source_owned: Option<u64>,
    validation_peak: Option<u64>,
    retained: Option<u64>,
}
impl Outcome {
    fn capture(result: &Result<serde_json::Value>) -> Self {
        match result {
            Ok(summary) => Self {
                accepted: true,
                package_sha256: Text::new(summary["package_sha256"].as_str().unwrap_or("")),
                world_content_hash: Text::new(summary["world_content_hash"].as_str().unwrap_or("")),
                verification: Text::new(summary["verification"].as_str().unwrap_or("")),
                error_code: Text::new(""),
                error_message: Text::new(""),
                source_owned: summary["audit_source_owned_bytes"].as_u64(),
                validation_peak: summary["validation_peak_bytes"].as_u64(),
                retained: summary["retained_memory_bytes"].as_u64(),
            },
            Err(failure) => Self {
                accepted: false,
                package_sha256: Text::new(""),
                world_content_hash: Text::new(""),
                verification: Text::new(""),
                error_code: Text::new(&failure.code),
                error_message: Text::new(&failure.message),
                source_owned: None,
                validation_peak: None,
                retained: None,
            },
        }
    }
}

fn delta(live: usize, baseline: usize) -> i64 {
    // Rust allocations cannot occupy more than the process's address space;
    // clamp only for completeness on platforms with an unsigned 64-bit usize.
    (live as i128 - baseline as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(error(
            "E_USAGE",
            "regional_allocations PACKAGE BUDGET_BYTES",
        ));
    }
    let memory_limit: u64 = args[2]
        .parse()
        .map_err(|_| error("E_USAGE", "BUDGET_BYTES must be an unsigned integer"))?;
    // Index opening is deliberately outside the measured phase and uses a
    // separate allowance, so even zero-byte audit rejection can be diagnosed.
    const OPEN_ALLOWANCE: u64 = 1024 * 1024 * 1024;
    let mut reader = IndexedReader::open_path(Path::new(&args[1]), OPEN_ALLOWANCE, None)?;
    let index_identity = reader.identity().to_owned();
    let index_retained = reader.index_retained_bytes();
    let preflight = reader.audit_preflight_bytes();
    let index_bound = reader.audit_peak_bytes();
    let regions = reader.index().regions.len();
    let epoch = ReadEpoch::default();
    let ticket = epoch.begin();

    let phase = Phase::begin();
    let result = reader.audit_summary(memory_limit, &ticket);
    let after_audit = phase.snapshot();
    let outcome = Outcome::capture(&result);
    drop(result);
    let after_result_release = phase.snapshot();
    // Retain epoch/ticket until after the reader-release sample so its negative
    // delta reports reader ownership, independently of cancellation state.
    drop(reader);
    let after_reader_release = phase.snapshot();
    let baseline = phase.baseline;
    drop(phase);
    drop(ticket);
    drop(epoch);

    // Constructing JSON and initializing stdout happens after measurement.
    // A rejected audit is a successful diagnostic invocation (exit status 0);
    // accepted=false plus the original error identifies the rejection.
    println!(
        "{}",
        serde_json::json!({
            "diagnostic": "regional-audit-requested-allocations-v1",
            "package": args[1],
            "audit_budget_bytes": memory_limit,
            "open_budget_bytes": OPEN_ALLOWANCE,
            "index_sha256": index_identity,
            "package_sha256": outcome.accepted.then(|| outcome.package_sha256.as_str()),
            "world_content_hash": outcome.accepted.then(|| outcome.world_content_hash.as_str()),
            "regions": regions,
            "accepted": outcome.accepted,
            "verification": outcome.accepted.then(|| outcome.verification.as_str()),
            "error_code": (!outcome.accepted).then(|| outcome.error_code.as_str()),
            "error_message": (!outcome.accepted).then(|| outcome.error_message.as_str()),
            "error_text_truncated": outcome.error_code.truncated || outcome.error_message.truncated,
            "index_retained_bound_bytes": index_retained,
            "audit_preflight_bytes": preflight,
            "audit_index_bound_bytes": index_bound,
            "audit_source_owned_bytes": outcome.source_owned,
            "validation_peak_bytes": outcome.validation_peak,
            "retained_memory_bytes": outcome.retained,
            "tracked_requested_baseline_bytes": baseline,
            "tracked_requested_audit_peak_bytes": after_audit.peak,
            "tracked_requested_audit_peak_delta_bytes": after_audit.peak.saturating_sub(baseline),
            "tracked_requested_live_after_audit_bytes": after_audit.live,
            "tracked_requested_live_after_audit_delta_bytes": delta(after_audit.live, baseline),
            "tracked_requested_live_after_result_release_bytes": after_result_release.live,
            "tracked_requested_live_after_result_release_delta_bytes": delta(after_result_release.live, baseline),
            "tracked_requested_live_after_reader_release_bytes": after_reader_release.live,
            "tracked_requested_live_after_reader_release_delta_bytes": delta(after_reader_release.live, baseline),
            "tracked_requested_reader_released_bytes": after_result_release.live.saturating_sub(after_reader_release.live),
            "measurement_scope": "Rust requested live heap; index opened before phase; includes returned summary until its explicit release; excludes allocator overhead, stack, external allocations and RSS",
        })
    );
    Ok(())
}
