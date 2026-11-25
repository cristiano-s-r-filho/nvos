//! Two-level hierarchical page table and software Translation Lookaside Buffer (TLB).
//!
//! 32-bit Virtual Address breakdown:
//! - Bits `[31..22]` (10 bits): Page Directory Index (1024 entries)
//! - Bits `[21..12]` (10 bits): Page Table Index (1024 entries)
//! - Bits `[11..0]` (12 bits): Offset in 4 KB Page

use std::collections::{BTreeMap, VecDeque};
use crate::{FrameId, PhysicalAddress, VirtualAddress};

/// Bitmask and shift constants for 32-bit two-level paging with 4KB pages.
pub const PAGE_OFFSET_MASK: u32 = 0x0000_0FFF;
pub const PAGE_TABLE_MASK: u32 = 0x003F_F000;
pub const PAGE_DIRECTORY_MASK: u32 = 0xFFC0_0000;

pub const PAGE_TABLE_SHIFT: u32 = 12;
pub const PAGE_DIRECTORY_SHIFT: u32 = 22;

/// A single entry in the second-level page table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableEntry {
    pub frame: FrameId,
    pub present: bool,
    pub writable: bool,
    pub dirty: bool,
    pub referenced: bool,
}

impl Default for PageTableEntry {
    fn default() -> Self {
        Self {
            frame: FrameId(0),
            present: false,
            writable: true,
            dirty: false,
            referenced: false,
        }
    }
}

/// Two-level hierarchical page table for a process.
#[derive(Debug, Default)]
pub struct TwoLevelPageTable {
    /// Directory: maps Directory Index (0..1024) -> Table (maps Table Index 0..1024 -> Entry)
    directory: BTreeMap<u32, BTreeMap<u32, PageTableEntry>>,
}

impl TwoLevelPageTable {
    /// Create a new, empty two-level page table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a virtual page number to a physical frame.
    pub fn map_page(&mut self, virtual_addr: VirtualAddress, frame: FrameId, writable: bool) {
        let dir_idx = (virtual_addr.0 & PAGE_DIRECTORY_MASK) >> PAGE_DIRECTORY_SHIFT;
        let tbl_idx = (virtual_addr.0 & PAGE_TABLE_MASK) >> PAGE_TABLE_SHIFT;

        let table = self.directory.entry(dir_idx).or_default();
        table.insert(
            tbl_idx,
            PageTableEntry {
                frame,
                present: true,
                writable,
                dirty: false,
                referenced: true,
            },
        );
    }

    /// Unmap a virtual page, returning the previously mapped entry if present.
    pub fn unmap_page(&mut self, virtual_addr: VirtualAddress) -> Option<PageTableEntry> {
        let dir_idx = (virtual_addr.0 & PAGE_DIRECTORY_MASK) >> PAGE_DIRECTORY_SHIFT;
        let tbl_idx = (virtual_addr.0 & PAGE_TABLE_MASK) >> PAGE_TABLE_SHIFT;

        let table = self.directory.get_mut(&dir_idx)?;
        let entry = table.remove(&tbl_idx);
        if table.is_empty() {
            self.directory.remove(&dir_idx);
        }
        entry
    }

    /// Lookup page table entry for a virtual address.
    #[must_use]
    pub fn lookup(&self, virtual_addr: VirtualAddress) -> Option<&PageTableEntry> {
        let dir_idx = (virtual_addr.0 & PAGE_DIRECTORY_MASK) >> PAGE_DIRECTORY_SHIFT;
        let tbl_idx = (virtual_addr.0 & PAGE_TABLE_MASK) >> PAGE_TABLE_SHIFT;

        let table = self.directory.get(&dir_idx)?;
        let entry = table.get(&tbl_idx)?;
        if entry.present {
            Some(entry)
        } else {
            None
        }
    }

    /// Lookup mutable page table entry for a virtual address (to update dirty/referenced bits).
    pub fn lookup_mut(&mut self, virtual_addr: VirtualAddress) -> Option<&mut PageTableEntry> {
        let dir_idx = (virtual_addr.0 & PAGE_DIRECTORY_MASK) >> PAGE_DIRECTORY_SHIFT;
        let tbl_idx = (virtual_addr.0 & PAGE_TABLE_MASK) >> PAGE_TABLE_SHIFT;

        let table = self.directory.get_mut(&dir_idx)?;
        let entry = table.get_mut(&tbl_idx)?;
        if entry.present {
            Some(entry)
        } else {
            None
        }
    }
}

/// Translation Lookaside Buffer entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlbEntry {
    pub virtual_page: u32,
    pub frame: FrameId,
    pub writable: bool,
}

/// Software Translation Lookaside Buffer (TLB) cache.
#[derive(Debug)]
pub struct SoftwareTlb {
    capacity: usize,
    entries: VecDeque<TlbEntry>,
    hits: u64,
    misses: u64,
}

impl SoftwareTlb {
    /// Create a new TLB with fixed entry capacity (e.g. 8 or 16).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::with_capacity(capacity.max(1)),
            hits: 0,
            misses: 0,
        }
    }

    /// Lookup physical translation in TLB cache.
    pub fn lookup(&mut self, virtual_addr: VirtualAddress) -> Option<PhysicalAddress> {
        let vpage = virtual_addr.0 >> PAGE_TABLE_SHIFT;
        let offset = virtual_addr.0 & PAGE_OFFSET_MASK;

        if let Some(pos) = self.entries.iter().position(|e| e.virtual_page == vpage) {
            self.hits = self.hits.saturating_add(1);
            let entry = self.entries.remove(pos).expect("valid index");
            let paddr = (entry.frame.0 << PAGE_TABLE_SHIFT) | offset;
            self.entries.push_front(entry); // Move to head (LRU)
            Some(PhysicalAddress(paddr))
        } else {
            self.misses = self.misses.saturating_add(1);
            None
        }
    }

    /// Insert a new translation into the TLB cache.
    pub fn insert(&mut self, virtual_addr: VirtualAddress, frame: FrameId, writable: bool) {
        let vpage = virtual_addr.0 >> PAGE_TABLE_SHIFT;
        if let Some(pos) = self.entries.iter().position(|e| e.virtual_page == vpage) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.capacity {
            self.entries.pop_back(); // Evict LRU
        }

        self.entries.push_front(TlbEntry {
            virtual_page: vpage,
            frame,
            writable,
        });
    }

    /// Invalidate all TLB entries (e.g. on context switch or page swap).
    pub fn flush(&mut self) {
        self.entries.clear();
    }

    /// Total TLB hits.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Total TLB misses.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Hit ratio percentage: `(hits / (hits + misses)) * 100.0`.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits.saturating_add(self.misses);
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

impl Default for SoftwareTlb {
    fn default() -> Self {
        Self::new(8)
    }
}
