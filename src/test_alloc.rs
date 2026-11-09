//! Test-only thread-local heap instrumentation shared by unit-test binaries.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct HeapStats {
    pub allocations: usize,
    pub deallocations: usize,
    pub reallocations: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
}
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    static HEAP: Cell<Option<HeapStats>> = const { Cell::new(None) };
}
pub(crate) fn measured_heap(action: impl FnOnce()) -> HeapStats {
    HEAP.with(|s| {
        assert!(s.get().is_none());
        s.set(Some(HeapStats::default()));
    });
    action();
    HEAP.with(|s| s.replace(None).unwrap())
}
fn heap_update(size: usize, old: usize, reallocation: bool, deallocation: bool) {
    let _ = HEAP.try_with(|cell| {
        if let Some(mut s) = cell.get() {
            if deallocation {
                s.deallocations += 1;
                s.live_bytes = s.live_bytes.saturating_sub(old);
            } else {
                s.allocations += 1;
                if reallocation {
                    s.reallocations += 1;
                }
                s.peak_bytes = s.peak_bytes.max(s.live_bytes.saturating_add(size));
                s.live_bytes = s.live_bytes.saturating_sub(old).saturating_add(size);
            }
            cell.set(Some(s));
        }
    });
}
pub(crate) fn counted_allocations(action: impl FnOnce()) -> usize {
    COUNT.with(|count| {
        assert!(count.get().is_none());
        count.set(Some(0));
    });
    action();
    COUNT.with(|count| count.replace(None).unwrap())
}
struct CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        heap_update(layout.size(), 0, false, false);
        let _ = COUNT.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        heap_update(0, layout.size(), false, true);
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        heap_update(size, layout.size(), true, false);
        let _ = COUNT.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        System.realloc(pointer, layout, size)
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn allocator_records_real_allocation_growth_and_release() {
    let stats = measured_heap(|| {
        let mut values = Vec::<u64>::with_capacity(4);
        values.resize(64, 7);
        std::hint::black_box(&values);
        drop(values);
    });
    assert!(stats.allocations >= 1);
    assert!(stats.reallocations >= 1);
    assert!(stats.deallocations >= 1);
    assert_eq!(stats.live_bytes, 0);
    assert!(stats.peak_bytes >= 64 * std::mem::size_of::<u64>());
}
