//! A4 zero-allocation check. harness = false: a single thread, inputs constructed before the
//! counter is armed, results stored into pre-reserved capacity.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use lp1_node::asert::{asert, AsertOut, Early};
use lp1_node::fixed::U256;

struct Counting;

static ARMED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Ordering::SeqCst) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Ordering::SeqCst) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.load(Ordering::SeqCst) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(p, l, n)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

type In = (U256, i128, i128, i128, i128, i128);

fn inputs() -> Vec<In> {
    let g = 1_700_000_000i128;
    let mut v: Vec<In> = Vec::new();
    for tg in [U256::ONE, U256::pow2(240).unwrap(), U256::MAX] {
        for dt in [
            -257 * 600,
            -256 * 600,
            -1,
            0,
            1,
            4 * 600,
            255 * 600,
            255 * 600 + 599,
            256 * 600,
            1i128 << 62,
            (1i128 << 97) - 1,
            -((1i128 << 97) - 1),
            1i128 << 97,
        ] {
            v.push((tg, g, 10, 600, g + 200 + dt, 20));
        }
        v.push((tg, g, 1000, 1, g + 1000 * 4_294_967_295, 4_294_967_295));
    }
    // The 1027 oracle rows, parsed before arming.
    let path = lp1_node::fixtures::default_dir();
    if let Ok(oracle) = lp1_node::fixtures::read_json(&path, "asert-oracle.json") {
        for r in oracle["rows"].as_array().unwrap() {
            let d = |k: &str| r[k].as_str().unwrap().parse::<i128>().unwrap();
            let tg = U256::from_dec_str(r["targetG"].as_str().unwrap()).unwrap();
            v.push((tg, d("gts"), d("blockTime"), d("tau"), d("ts"), d("h")));
        }
    } else {
        eprintln!("asert_alloc: fixtures/asert-oracle.json missing");
        std::process::exit(1);
    }
    v
}

fn main() {
    let ins = inputs();
    let mut outs: Vec<Option<AsertOut>> = Vec::with_capacity(ins.len());
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.store(true, Ordering::SeqCst);
    for (tg, g, b, tau, ts, h) in ins.iter() {
        outs.push(asert(tg, *g, *b, *tau, *ts, *h));
    }
    ARMED.store(false, Ordering::SeqCst);
    let during = ALLOCS.load(Ordering::SeqCst);

    // The counter itself must observe allocations, or a zero would mean nothing.
    ARMED.store(true, Ordering::SeqCst);
    let probe = vec![7u8; 64];
    ARMED.store(false, Ordering::SeqCst);
    let probe_seen = ALLOCS.load(Ordering::SeqCst) > during;

    let max_shift = outs.iter().flatten().map(|o| o.shift_bits).max().unwrap_or(0);
    let highs = outs.iter().flatten().filter(|o| o.early == Early::High).count();
    let nones = outs.iter().filter(|o| o.is_none()).count();
    println!(
        "{{\"test\":\"asert_alloc\",\"calls\":{},\"allocations\":{during},\"counterProbe\":{probe_seen},\"maxShiftBits\":{max_shift},\"earlyHigh\":{highs},\"none\":{nones},\"probeLen\":{}}}",
        ins.len(),
        probe.len()
    );
    if during != 0 || !probe_seen || max_shift != 512 || nones != 3 {
        eprintln!("asert_alloc FAILED");
        std::process::exit(1);
    }
}
