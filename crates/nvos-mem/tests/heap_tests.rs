use nvos_core::process::Pid;
use nvos_mem::{BuddyAllocator, ContiguousHeapAllocator, HeapAllocator, MemoryError};

#[test]
fn test_first_fit_and_best_fit_allocation() {
    let mut first_fit = ContiguousHeapAllocator::new(100, false);
    let pid = Pid(1);

    let b1 = first_fit.allocate(pid, 20).unwrap();
    let b2 = first_fit.allocate(pid, 30).unwrap();
    let b3 = first_fit.allocate(pid, 50).unwrap();

    assert_eq!(b1.size_bytes, 20);
    assert_eq!(b2.size_bytes, 30);
    assert_eq!(b3.size_bytes, 50);

    // Memory is full
    let oom = first_fit.allocate(pid, 10).unwrap_err();
    assert!(matches!(oom, MemoryError::OutOfMemory { .. }));

    // Free middle block (30 bytes)
    first_fit.free(pid, b2.id).unwrap();

    // Best-fit test
    let mut best_fit = ContiguousHeapAllocator::new(200, true);
    let x1 = best_fit.allocate(pid, 40).unwrap();
    let x2 = best_fit.allocate(pid, 60).unwrap();
    best_fit.free(pid, x1.id).unwrap(); // 40 byte hole at offset 0
    let x3 = best_fit.allocate(pid, 30).unwrap(); // fits best in 40 byte hole
    assert_eq!(x3.base_addr.0, 0);
    best_fit.free(pid, x2.id).unwrap();
    best_fit.free(pid, x3.id).unwrap();
}

#[test]
fn test_buddy_allocator_split_and_coalesce() {
    // 256 bytes total, min block = 16 bytes
    let mut buddy = BuddyAllocator::new(16, 256);
    let pid = Pid(10);

    // Request 30 bytes -> rounded up to 32 (order 5)
    let b1 = buddy.allocate(pid, 30).unwrap();
    assert_eq!(b1.base_addr.0, 0);

    // Request 15 bytes -> rounded up to 16 (order 4)
    let b2 = buddy.allocate(pid, 15).unwrap();
    assert_eq!(b2.base_addr.0, 32);

    // Free b1 and b2
    buddy.free(pid, b1.id).unwrap();
    buddy.free(pid, b2.id).unwrap();

    // Coalescing allows allocating full 256 byte block again
    let b_full = buddy.allocate(pid, 256).unwrap();
    assert_eq!(b_full.base_addr.0, 0);
    buddy.free(pid, b_full.id).unwrap();
}
