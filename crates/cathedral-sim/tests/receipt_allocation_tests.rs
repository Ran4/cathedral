//! Isolated process: measure live heap allocation for the complete replay window.
use cathedral_sim::receipts::*;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn full_replay_window_allocated_and_encoded_budgets() {
    let payload = json!({"verb":"measurement"});
    let code = "c".repeat(48);
    let message = "m".repeat(256);
    let reference = AffectedRef::new(&"k".repeat(16), &"i".repeat(64)).unwrap();
    let before = LIVE.load(Ordering::SeqCst);
    let mut ledger = CommandLedger::default();
    let mut roots = [OperationId {
        producer: 0,
        sequence: 0,
    }; 16];
    for (producer, root) in roots.iter_mut().enumerate() {
        *root = ledger.reserve_operation(producer as u8).unwrap();
        for step in 0..256 {
            let Admission::New(ticket) = ledger.begin(root.command(step), &payload) else {
                panic!("window admission")
            };
            ledger.finish(
                ticket,
                f64::MAX,
                Outcome::new(ReceiptState::Accepted, &code, &message),
                vec![reference.clone(), reference.clone()],
            );
        }
    }
    for root in roots {
        for step in 0..256 {
            ledger
                .advance(
                    root.command(step),
                    f64::MAX,
                    Outcome::new(ReceiptState::Completed, &code, &message),
                )
                .unwrap();
        }
    }
    let indexed = LIVE.load(Ordering::SeqCst) - before;
    assert!(
        indexed as usize + std::mem::size_of::<CommandLedger>() <= 4 * 1024 * 1024,
        "full lifecycle update index: {indexed}"
    );
    for root in roots {
        ledger.unprotect(root);
    }
    drop(ledger.drain_updates());
    let mut churn_peak = 0isize;
    for index in 0..RECENT_CAPACITY * 2 {
        let id = ledger
            .issue((index % PRODUCER_CAPACITY) as u8)
            .unwrap()
            .command(0);
        let Admission::New(ticket) = ledger.begin(id, &payload) else {
            panic!("churn admission")
        };
        ledger.finish(
            ticket,
            f64::MAX,
            Outcome::new(ReceiptState::Completed, &code, &message),
            vec![reference.clone(), reference.clone()],
        );
        churn_peak = churn_peak.max(LIVE.load(Ordering::SeqCst) - before);
    }
    assert!(
        churn_peak as usize + std::mem::size_of::<CommandLedger>() <= 4 * 1024 * 1024,
        "compacted interleaved window: {churn_peak}"
    );
    let allocated = LIVE.load(Ordering::SeqCst) - before;
    let encoded = ledger.encoded_recent_bytes();
    println!(
        "recent_entries={} encoded_bytes={encoded} live_allocated_bytes={allocated} full_update_index_bytes={indexed} two_cycle_churn_peak_bytes={churn_peak} inline_owner_bytes={}",
        ledger.recent_len(),
        std::mem::size_of::<CommandLedger>()
    );
    assert_eq!(ledger.recent_len(), RECENT_CAPACITY);
    assert!(ledger.is_at_boundary());
    assert!(encoded <= RECENT_CAPACITY * MAX_ENTRY_BYTES);
    assert!(allocated as usize + std::mem::size_of::<CommandLedger>() <= 4 * 1024 * 1024);
}
