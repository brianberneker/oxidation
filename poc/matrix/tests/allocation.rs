use oxidation_dsp::{
    dsp::core::{ApproximateNode, BorrowedNode, OriginalNode},
    parameters::NodeParameters,
};
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

// Delegate allocation unchanged; count only the processing test's thread.
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
fn node_processing_performs_zero_heap_allocations() {
    let input = [0.001; 1024];
    let mut output = [[0.0; 8]; 1024];
    let mut original = OriginalNode::default();
    let mut borrowed = BorrowedNode::default();
    let mut approximate = ApproximateNode::default();
    let parameters = NodeParameters::new(5, 0, 0).unwrap();
    ALLOCATIONS.with(|count| count.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    (0..100).for_each(|_| {
        original.process(&input, &mut output, parameters);
        for accelerated in [false, true] {
            borrowed.process_with_acceleration(&input, &mut output, parameters, accelerated);
            approximate.process_with_acceleration(&input, &mut output, parameters, accelerated);
        }
    });
    TRACKING.with(|tracking| tracking.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
