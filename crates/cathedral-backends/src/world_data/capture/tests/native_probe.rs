//! Optional native instrumentation. The evidence runner supplies the reviewed
//! LD_PRELOAD helper; ordinary tests/production never load or call it.
use super::*;

#[repr(C)]
#[derive(Default, Debug)]
struct Stats {
    opens: u64,
    successful: u64,
    reads: u64,
    closes: u64,
    allocations: u64,
    frees: u64,
    live: u64,
    peak_live: u64,
    request_peak: u64,
    usable_peak: u64,
    mismatches: u64,
    read_allocations: u64,
    close_allocations: u64,
    minimum_budget: u64,
}
type Begin = unsafe extern "C" fn(i32, extern "C" fn(*mut libc::c_void) -> u64, *mut libc::c_void);
type End = unsafe extern "C" fn(*mut Stats);

extern "C" fn retained(context: *mut libc::c_void) -> u64 {
    // SAFETY: measured() supplies a live budget reference until end() disarms
    // the helper; all callbacks execute synchronously on this test thread.
    unsafe { (&*context.cast::<CheckpointBudget>()).retained_bytes() as u64 }
}

fn measured<T>(budget: &CheckpointBudget, mode: i32, f: impl FnOnce() -> T) -> (T, Stats) {
    // SAFETY: these two unique symbols and their C ABI/record layout are defined
    // by the pinned evidence helper. The runner verifies its hash before use.
    let (begin, end): (Begin, End) = unsafe {
        let begin = libc::dlsym(libc::RTLD_DEFAULT, c"cathedral_native_begin".as_ptr());
        let end = libc::dlsym(libc::RTLD_DEFAULT, c"cathedral_native_end".as_ptr());
        assert!(
            !begin.is_null() && !end.is_null(),
            "run through the native directory evidence helper"
        );
        (std::mem::transmute(begin), std::mem::transmute(end))
    };
    struct Armed(End);
    impl Drop for Armed {
        fn drop(&mut self) {
            let mut ignored = Stats::default();
            // SAFETY: helper is loaded for process lifetime; discard counters
            // and disarm on panic before the borrowed budget can be destroyed.
            unsafe { (self.0)(&mut ignored) };
        }
    }
    // SAFETY: budget remains borrowed throughout f and the synchronous end.
    unsafe { begin(mode, retained, std::ptr::from_ref(budget).cast_mut().cast()) };
    let armed = Armed(end);
    let result = f();
    let mut stats = Stats::default();
    // SAFETY: writable stack Stats has the exact C layout and remains live.
    unsafe { end(&mut stats) };
    std::mem::forget(armed);
    (result, stats)
}

fn closed(stats: &Stats, expected_budget: usize) {
    assert_eq!(stats.mismatches, 0, "{stats:?}");
    assert_eq!(stats.live, 0);
    assert_eq!(stats.peak_live, 1);
    assert_eq!(stats.successful, stats.closes);
    assert_eq!(stats.closes, stats.frees);
    assert_eq!(stats.allocations, stats.successful);
    assert_eq!(stats.read_allocations, 0);
    assert_eq!(stats.close_allocations, 0);
    assert_eq!(stats.minimum_budget, expected_budget as u64);
    assert!(stats.request_peak <= (1024 * 1024 + 48));
    assert!(stats.usable_peak <= NATIVE_DIRECTORY_BYTES as u64);
}

#[test]
#[ignore = "requires pinned Linux/glibc native allocator evidence runner"]
fn native_directory_allocator_lifetime_probe() {
    let fixture = Fixture::new();
    let (budget, root) = budget();
    // Enough space for the previous Rust-only peak must still refuse before
    // native discovery: the new DIR envelope cannot be borrowed after IO.
    let pressure = budget
        .reserve(
            Cohort::LoadCandidate,
            MAX_RESIDENT_BYTES - root.bytes() - (peak_bytes() - NATIVE_DIRECTORY_BYTES),
        )
        .unwrap();
    let (result, stats) = measured(&budget, 0, || fixture.capture(&budget));
    assert!(matches!(result, Err(SourceCaptureError::Admission)));
    assert_eq!(stats.opens, 0);
    assert_eq!(stats.allocations, 0);
    drop(pressure);

    let expected = root.bytes() + peak_bytes();
    let (result, stats) = measured(&budget, 0, || fixture.capture(&budget));
    let captured = result.unwrap();
    closed(&stats, expected);
    assert_eq!(stats.opens, 3); // root, a, z: aliases must not double-count.
    assert!(stats.reads >= 6);
    use std::os::unix::fs::MetadataExt;
    let block = fs::metadata(fixture.0.join("lore/characters"))
        .unwrap()
        .blksize();
    assert_eq!(stats.request_peak, block.clamp(32768, 1048576) + 48);
    assert_eq!(
        budget.retained_bytes(),
        root.bytes() + captured.charged_bytes()
    );
    assert!(budget.retained_bytes() < expected);
    println!("native_directory success {stats:?} admitted_native={NATIVE_DIRECTORY_BYTES}");
    drop(captured);

    // Force a real opendir malloc failure; libc owns closing its opened fd.
    let fd_count = || fs::read_dir("/proc/self/fd").unwrap().count();
    let before = fd_count();
    let (result, stats) = measured(&budget, 1, || fixture.capture(&budget));
    assert!(matches!(result, Err(SourceCaptureError::Io)));
    assert_eq!(fd_count(), before);
    assert_eq!(stats.opens, 1);
    assert_eq!(stats.allocations, 1);
    assert_eq!(stats.successful, 0);
    assert_eq!(stats.closes, 0);
    assert_eq!(stats.live, 0);
    assert_eq!(stats.mismatches, 0);
    assert_eq!(stats.minimum_budget, expected as u64);
    assert_eq!(budget.retained_bytes(), root.bytes());
    println!("native_directory malloc_failure {stats:?}");

    // Inject the iterator's EIO branch, then prove Rust closes the real stream.
    let (result, stats) = measured(&budget, 2, || fixture.capture(&budget));
    assert!(matches!(result, Err(SourceCaptureError::Io)));
    closed(&stats, expected);
    assert_eq!(stats.reads, 1);
    assert_eq!(budget.retained_bytes(), root.bytes());
    println!("native_directory read_failure {stats:?}");

    // Application early return while a stream is live also closes before release.
    let long = (0..5)
        .map(|_| "x".repeat(220))
        .collect::<Vec<_>>()
        .join("/");
    fixture.write(&format!("lore/characters/{long}/bad.json"), b"bad");
    let (result, stats) = measured(&budget, 0, || fixture.capture(&budget));
    assert!(matches!(result, Err(SourceCaptureError::PathLimit)));
    closed(&stats, expected);
    assert_eq!(budget.retained_bytes(), root.bytes());
    println!("native_directory path_failure {stats:?}");
}
