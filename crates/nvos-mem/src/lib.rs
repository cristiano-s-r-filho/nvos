//! `nvos-mem` — dual-layer memory management for `nvos`.
//!
//! Provides:
//! - Layer 1: Two-level hierarchical page tables, software TLB, and page replacement (FIFO, LRU, Clock).
//! - Layer 2: Contiguous physical heap allocation (First-Fit, Best-Fit, and Buddy Allocator).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::collapsible_if,
    clippy::struct_excessive_bools,
    clippy::unnecessary_lazy_evaluations,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;
use std::fmt;
use nvos_core::process::{HeapBlockId, PageTableId, Pid};
use nvos_core::tick::Tick;

pub mod frame_allocator;
pub mod heap;
pub mod page_table;
pub mod replacement;

pub use frame_allocator::{FrameAllocator, DEFAULT_PHYSICAL_FRAMES, PAGE_SIZE};
pub use heap::{BuddyAllocator, ContiguousHeapAllocator, HeapAllocator};
pub use page_table::{PageTableEntry, SoftwareTlb, TwoLevelPageTable};
pub use replacement::{
    ClockReplacement, FifoReplacement, LruReplacement, PageReplacementPolicy, ResidentPage,
};

/// 32-bit virtual memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VirtualAddress(pub u32);

/// 32-bit physical memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PhysicalAddress(pub u32);

/// Physical frame index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FrameId(pub u32);

/// Contiguous physical heap memory block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapBlock {
    pub id: HeapBlockId,
    pub base_addr: PhysicalAddress,
    pub size_bytes: usize,
}

/// Errors produced by memory management operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// Physical memory is exhausted.
    OutOfMemory { requested: usize, available: usize },
    /// Invalid heap block identifier.
    InvalidBlockId(HeapBlockId),
    /// Process has no initialized page table root.
    MissingPageTable(Pid),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory { requested, available } => {
                write!(
                    f,
                    "physical memory exhausted: requested {requested} bytes, available {available} bytes"
                )
            }
            Self::InvalidBlockId(id) => write!(f, "invalid heap block ID {id:?}"),
            Self::MissingPageTable(pid) => write!(f, "page table root missing for PID {pid:?}"),
        }
    }
}

impl std::error::Error for MemoryError {}

/// Memory faults emitted during address translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryFault {
    /// Virtual address is unmapped in the page table (triggers page swap).
    PageFault { pid: Pid, virtual_addr: VirtualAddress },
    /// Virtual address write attempted on read-only page.
    ProtectionViolation { pid: Pid, virtual_addr: VirtualAddress },
}

impl fmt::Display for MemoryFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PageFault { pid, virtual_addr } => {
                write!(f, "PageFault: pid={pid:?} vaddr=0x{:08x}", virtual_addr.0)
            }
            Self::ProtectionViolation { pid, virtual_addr } => {
                write!(
                    f,
                    "ProtectionViolation: pid={pid:?} vaddr=0x{:08x}",
                    virtual_addr.0
                )
            }
        }
    }
}

impl std::error::Error for MemoryFault {}

/// Report of internal and external fragmentation metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FragmentationReport {
    pub internal_wasted_bytes: usize,
    pub max_free_contiguous_block: usize,
    pub total_free_bytes: usize,
    pub allocated_frames: usize,
    pub free_frames: usize,
}

/// Comprehensive memory manager combining Layer 1 (Paging + TLB) and Layer 2 (Heap).
pub trait MemoryManager: Send + Sync {
    /// Name of active memory configuration.
    fn name(&self) -> &'static str;

    // --- Layer 1: Virtual Paging & TLB ---
    /// Translate virtual address to physical address via TLB and Page Tables.
    fn translate(&mut self, pid: Pid, virtual_addr: VirtualAddress) -> Result<PhysicalAddress, MemoryFault>;

    /// Evict a victim page according to replacement policy and prepare page-in.
    fn handle_page_fault(&mut self, fault: MemoryFault) -> Result<FrameId, MemoryError>;

    // --- Layer 2: Contiguous Physical Heap ---
    /// Allocate contiguous physical memory block.
    fn allocate_heap(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, MemoryError>;

    /// Free contiguous physical memory block.
    fn free_heap(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), MemoryError>;

    /// Calculate active fragmentation metrics.
    fn fragmentation_metrics(&self) -> FragmentationReport;
}

/// Unified standard dual-layer memory manager implementation.
#[derive(Debug)]
pub struct StandardMemoryManager {
    name: &'static str,
    frames: FrameAllocator,
    tlb: SoftwareTlb,
    page_tables: BTreeMap<Pid, TwoLevelPageTable>,
    replacement_policy: Box<dyn PageReplacementPolicy>,
    heap_allocator: Box<dyn HeapAllocator>,
}

impl StandardMemoryManager {
    /// Create a standard dual-layer memory manager.
    pub fn new(
        name: &'static str,
        num_frames: usize,
        tlb_capacity: usize,
        replacement_policy: Box<dyn PageReplacementPolicy>,
        heap_allocator: Box<dyn HeapAllocator>,
    ) -> Self {
        Self {
            name,
            frames: FrameAllocator::new(num_frames, PAGE_SIZE),
            tlb: SoftwareTlb::new(tlb_capacity),
            page_tables: BTreeMap::new(),
            replacement_policy,
            heap_allocator,
        }
    }

    /// Register/initialize a page table for a newly spawned process.
    pub fn create_page_table(&mut self, pid: Pid) -> PageTableId {
        self.page_tables.insert(pid, TwoLevelPageTable::new());
        PageTableId(pid.0)
    }

    /// Access the underlying TLB metrics.
    #[must_use]
    pub fn tlb(&self) -> &SoftwareTlb {
        &self.tlb
    }

    /// Map a virtual page to a physical frame directly.
    pub fn map_page(&mut self, pid: Pid, vaddr: VirtualAddress, frame: FrameId, writable: bool) {
        let pt = self.page_tables.entry(pid).or_default();
        pt.map_page(vaddr, frame, writable);
        self.tlb.insert(vaddr, frame, writable);
        self.replacement_policy.record_page_in(ResidentPage {
            pid,
            virtual_addr: vaddr,
            frame,
            load_tick: Tick::ZERO,
            last_accessed: Tick::ZERO,
            reference_bit: true,
            dirty: false,
        });
    }
}

impl MemoryManager for StandardMemoryManager {
    fn name(&self) -> &'static str {
        self.name
    }

    fn translate(&mut self, pid: Pid, virtual_addr: VirtualAddress) -> Result<PhysicalAddress, MemoryFault> {
        // Step 1: Check TLB
        if let Some(paddr) = self.tlb.lookup(virtual_addr) {
            return Ok(paddr);
        }

        // Step 2: Page Table Walk
        let pt = self.page_tables.get_mut(&pid).ok_or(MemoryFault::PageFault {
            pid,
            virtual_addr,
        })?;

        if let Some(entry) = pt.lookup(virtual_addr) {
            let paddr = (entry.frame.0 << page_table::PAGE_TABLE_SHIFT) | (virtual_addr.0 & page_table::PAGE_OFFSET_MASK);
            let frame = entry.frame;
            let writable = entry.writable;

            // Update TLB on miss
            self.tlb.insert(virtual_addr, frame, writable);
            self.replacement_policy.record_access(frame, Tick::ZERO, false);
            Ok(PhysicalAddress(paddr))
        } else {
            Err(MemoryFault::PageFault { pid, virtual_addr })
        }
    }

    fn handle_page_fault(&mut self, fault: MemoryFault) -> Result<FrameId, MemoryError> {
        match fault {
            MemoryFault::PageFault { pid, virtual_addr } => {
                // Try allocating a free frame first
                let frame = if let Some(free_frame) = self.frames.allocate_frame() {
                    free_frame
                } else {
                    // Memory full: Evict victim frame via replacement policy
                    let victim = self.replacement_policy.select_victim().ok_or_else(|| {
                        MemoryError::OutOfMemory {
                            requested: PAGE_SIZE,
                            available: 0,
                        }
                    })?;

                    // Unmap victim from its process's page table
                    if let Some(victim_pt) = self.page_tables.get_mut(&victim.pid) {
                        victim_pt.unmap_page(victim.virtual_addr);
                    }
                    self.tlb.flush();
                    victim.frame
                };

                // Map new page in faulting process's page table
                let pt = self.page_tables.entry(pid).or_default();
                pt.map_page(virtual_addr, frame, true);
                self.tlb.insert(virtual_addr, frame, true);

                self.replacement_policy.record_page_in(ResidentPage {
                    pid,
                    virtual_addr,
                    frame,
                    load_tick: Tick::ZERO,
                    last_accessed: Tick::ZERO,
                    reference_bit: true,
                    dirty: false,
                });

                Ok(frame)
            }
            MemoryFault::ProtectionViolation { .. } => Err(MemoryError::OutOfMemory {
                requested: 0,
                available: 0,
            }),
        }
    }

    fn allocate_heap(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, MemoryError> {
        self.heap_allocator.allocate(pid, size_bytes)
    }

    fn free_heap(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), MemoryError> {
        self.heap_allocator.free(pid, block_id)
    }

    fn fragmentation_metrics(&self) -> FragmentationReport {
        let (internal, max_contiguous) = self.heap_allocator.fragmentation_metrics();
        FragmentationReport {
            internal_wasted_bytes: internal,
            max_free_contiguous_block: max_contiguous,
            total_free_bytes: self.frames.free_count().saturating_mul(PAGE_SIZE),
            allocated_frames: self.frames.allocated_count(),
            free_frames: self.frames.free_count(),
        }
    }
}
