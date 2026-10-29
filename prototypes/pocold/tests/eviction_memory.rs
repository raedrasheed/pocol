//! Allocation regression for eviction bookkeeping high-water, including retained capacity.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::{HashSet, VecDeque},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn add(n: usize) {
    let v = LIVE.fetch_add(n, Ordering::SeqCst) + n;
    PEAK.fetch_max(v, Ordering::SeqCst);
}
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            add(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Ordering::SeqCst);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = System.realloc(p, l, n);
        if !q.is_null() {
            add(n);
            LIVE.fetch_sub(l.size(), Ordering::SeqCst);
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
#[test]
fn eviction_high_water_covers_growth_and_trim_without_raising_budget() {
    use pocold::resources::eviction_bookkeeping_bytes as bound;
    assert_eq!(bound(u128::MAX, 0), None);
    assert_eq!(bound(0, u128::MAX), None);
    for n in [1024usize, 1025, 1792, 1793, 2048, 3585, 8193, 9217, 16385] {
        let base = LIVE.load(Ordering::SeqCst);
        PEAK.store(base, Ordering::SeqCst);
        let mut set = HashSet::new();
        let mut fifo = VecDeque::new();
        for i in 0..n {
            let mut h = [0u8; 32];
            h[..8].copy_from_slice(&(i as u64).to_le_bytes());
            set.insert(h);
            fifo.push_back(h);
        }
        while fifo.len() > 1024 {
            set.remove(&fifo.pop_front().unwrap());
        }
        let held = LIVE.load(Ordering::SeqCst) - base;
        let peak = PEAK.load(Ordering::SeqCst) - base;
        let model = bound((n - 1024) as u128, 0).unwrap();
        assert!(peak as u128 <= model, "n={n} peak={peak} bound={model}");
        assert!(held as u128 <= model, "n={n} held={held} bound={model}");
        if n == 1025 {
            assert!(
                held > 1024 * 96,
                "positive control: reproduce old accounting defect"
            );
        }
        drop(set);
        drop(fifo);
    }
    assert!(bound(2048, 2048).unwrap() > bound(2048, 0).unwrap());
}
