use super::core::{DspCore, ProcessingParameters};
use oxidation_dsp::parameters::NodeParameters;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record_allocation() {
    TRACKING.with(|tracking| {
        if tracking.get() {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
    });
}

// Keep the system allocation contract; only count the current test's thread.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn integrated_processing_and_mode_changes_do_not_allocate() {
    let mut core = DspCore::new();
    core.init(44100.0, true).unwrap();
    ALLOCATIONS.with(|count| count.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    (0..1000).for_each(|block| {
        core.process_block(
            &mut [0.001; 64],
            &mut [0.002; 64],
            ProcessingParameters {
                node: NodeParameters::new(5, 0, 0).unwrap(),
                damping_bypass: false,
                predelay: 0,
                wet: 0.35,
                front_back: 0.0,
                bottom_top: 0.0,
                distance: 0.0,
                original_order: block % 2 == 0,
                accelerated: block % 3 == 0,
            },
        );
    });
    TRACKING.with(|tracking| tracking.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
