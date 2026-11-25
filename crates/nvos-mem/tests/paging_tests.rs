use nvos_core::process::Pid;
use nvos_mem::{
    ClockReplacement, ContiguousHeapAllocator, FifoReplacement, FrameId,
    LruReplacement, MemoryFault, MemoryManager, PageReplacementPolicy, SoftwareTlb,
    StandardMemoryManager, TwoLevelPageTable, VirtualAddress,
};

#[test]
fn test_tlb_hit_miss_and_lru_eviction() {
    let mut tlb = SoftwareTlb::new(2); // capacity = 2 entries
    assert_eq!(tlb.hits(), 0);
    assert_eq!(tlb.misses(), 0);

    let vaddr1 = VirtualAddress(0x0001_0042);
    let vaddr2 = VirtualAddress(0x0002_0042);
    let vaddr3 = VirtualAddress(0x0003_0042);

    // Initial lookup misses
    assert!(tlb.lookup(vaddr1).is_none());
    assert_eq!(tlb.misses(), 1);

    // Insert entries 1 and 2
    tlb.insert(vaddr1, FrameId(10), true);
    tlb.insert(vaddr2, FrameId(20), true);

    // Hit on entry 1
    let paddr1 = tlb.lookup(vaddr1).unwrap();
    assert_eq!(paddr1.0, (10 << 12) | 0x042);
    assert_eq!(tlb.hits(), 1);

    // Insert entry 3: should evict entry 2 (since entry 1 was recently accessed)
    tlb.insert(vaddr3, FrameId(30), true);
    assert!(tlb.lookup(vaddr1).is_some());
    assert!(tlb.lookup(vaddr3).is_some());
    assert!(tlb.lookup(vaddr2).is_none()); // Evicted
}

#[test]
fn test_two_level_page_table_mapping_and_unmapping() {
    let mut pt = TwoLevelPageTable::new();
    let vaddr = VirtualAddress(0x1234_5678);
    let frame = FrameId(0xABC);

    assert!(pt.lookup(vaddr).is_none());

    pt.map_page(vaddr, frame, true);
    let entry = pt.lookup(vaddr).unwrap();
    assert_eq!(entry.frame, frame);
    assert!(entry.present);
    assert!(entry.writable);

    let unmapped = pt.unmap_page(vaddr).unwrap();
    assert_eq!(unmapped.frame, frame);
    assert!(pt.lookup(vaddr).is_none());
}

#[test]
fn test_page_fault_resolution_and_fifo_eviction() {
    let fifo = Box::new(FifoReplacement::new());
    let heap = Box::new(ContiguousHeapAllocator::new(1024, false));
    let mut mem = StandardMemoryManager::new("TestMem", 2, 4, fifo, heap); // only 2 frames!

    let pid = Pid(1);
    mem.create_page_table(pid);

    let v1 = VirtualAddress(0x1000);
    let v2 = VirtualAddress(0x2000);
    let v3 = VirtualAddress(0x3000);

    // v1 fault -> allocated frame 0
    let f1 = mem.handle_page_fault(MemoryFault::PageFault { pid, virtual_addr: v1 }).unwrap();
    assert_eq!(f1, FrameId(0));

    // v2 fault -> allocated frame 1
    let f2 = mem.handle_page_fault(MemoryFault::PageFault { pid, virtual_addr: v2 }).unwrap();
    assert_eq!(f2, FrameId(1));

    // Translation for v1 and v2 should hit
    assert!(mem.translate(pid, v1).is_ok());
    assert!(mem.translate(pid, v2).is_ok());

    // v3 fault -> memory full! FIFO evicts v1 (frame 0)
    let f3 = mem.handle_page_fault(MemoryFault::PageFault { pid, virtual_addr: v3 }).unwrap();
    assert_eq!(f3, FrameId(0));

    // Now v3 translates ok, but v1 faults again!
    assert!(mem.translate(pid, v3).is_ok());
    assert!(matches!(mem.translate(pid, v1), Err(MemoryFault::PageFault { .. })));
}

#[test]
fn test_clock_and_lru_page_replacement() {
    let mut clock = ClockReplacement::new();
    let p1 = nvos_mem::ResidentPage {
        pid: Pid(1),
        virtual_addr: VirtualAddress(0x1000),
        frame: FrameId(1),
        load_tick: nvos_core::tick::Tick(0),
        last_accessed: nvos_core::tick::Tick(0),
        reference_bit: true,
        dirty: false,
    };
    clock.record_page_in(p1);

    // Give second chance, then evict
    let victim = clock.select_victim().unwrap();
    assert_eq!(victim.frame, FrameId(1));

    let mut lru = LruReplacement::new();
    lru.record_page_in(p1);
    let lru_victim = lru.select_victim().unwrap();
    assert_eq!(lru_victim.frame, FrameId(1));
}
