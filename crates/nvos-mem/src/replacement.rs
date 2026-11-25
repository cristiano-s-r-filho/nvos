//! Page replacement policies: FIFO, Least Recently Used (LRU), and Second-Chance Clock.

use std::collections::{BTreeMap, VecDeque};
use nvos_core::process::Pid;
use nvos_core::tick::Tick;
use crate::{FrameId, VirtualAddress};

/// Record of a resident page frame tracked by replacement policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidentPage {
    pub pid: Pid,
    pub virtual_addr: VirtualAddress,
    pub frame: FrameId,
    pub load_tick: Tick,
    pub last_accessed: Tick,
    pub reference_bit: bool,
    pub dirty: bool,
}

/// Abstract page replacement policy trait.
pub trait PageReplacementPolicy: Send + Sync + std::fmt::Debug {
    /// Policy name (e.g. "FIFO", "LRU", "Clock").
    fn name(&self) -> &'static str;

    /// Register a newly allocated/swapped-in page.
    fn record_page_in(&mut self, page: ResidentPage);

    /// Record a page reference/access to update LRU timestamp or Clock reference bit.
    fn record_access(&mut self, frame: FrameId, current_tick: Tick, is_write: bool);

    /// Select and remove a victim frame to evict when memory is full.
    fn select_victim(&mut self) -> Option<ResidentPage>;

    /// Invalidate a resident page when freed explicitly.
    fn remove_frame(&mut self, frame: FrameId) -> Option<ResidentPage>;
}

// ---------------------------------------------------------------------------
// 1. FIFO (First-In, First-Out)
// ---------------------------------------------------------------------------

/// First-In, First-Out page replacement.
#[derive(Debug, Default)]
pub struct FifoReplacement {
    queue: VecDeque<ResidentPage>,
}

impl FifoReplacement {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PageReplacementPolicy for FifoReplacement {
    fn name(&self) -> &'static str {
        "FIFO"
    }

    fn record_page_in(&mut self, page: ResidentPage) {
        self.queue.push_back(page);
    }

    fn record_access(&mut self, frame: FrameId, _current_tick: Tick, is_write: bool) {
        if is_write {
            if let Some(page) = self.queue.iter_mut().find(|p| p.frame == frame) {
                page.dirty = true;
            }
        }
    }

    fn select_victim(&mut self) -> Option<ResidentPage> {
        self.queue.pop_front()
    }

    fn remove_frame(&mut self, frame: FrameId) -> Option<ResidentPage> {
        let pos = self.queue.iter().position(|p| p.frame == frame)?;
        self.queue.remove(pos)
    }
}

// ---------------------------------------------------------------------------
// 2. LRU (Least Recently Used)
// ---------------------------------------------------------------------------

/// Least Recently Used page replacement based on discrete access ticks.
#[derive(Debug, Default)]
pub struct LruReplacement {
    pages: BTreeMap<FrameId, ResidentPage>,
}

impl LruReplacement {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PageReplacementPolicy for LruReplacement {
    fn name(&self) -> &'static str {
        "LRU"
    }

    fn record_page_in(&mut self, page: ResidentPage) {
        self.pages.insert(page.frame, page);
    }

    fn record_access(&mut self, frame: FrameId, current_tick: Tick, is_write: bool) {
        if let Some(page) = self.pages.get_mut(&frame) {
            page.last_accessed = current_tick;
            if is_write {
                page.dirty = true;
            }
        }
    }

    fn select_victim(&mut self) -> Option<ResidentPage> {
        if self.pages.is_empty() {
            return None;
        }

        // Find frame with minimum last_accessed timestamp
        let oldest_frame = self
            .pages
            .values()
            .min_by_key(|p| p.last_accessed)
            .map(|p| p.frame)?;

        self.pages.remove(&oldest_frame)
    }

    fn remove_frame(&mut self, frame: FrameId) -> Option<ResidentPage> {
        self.pages.remove(&frame)
    }
}

// ---------------------------------------------------------------------------
// 3. Second-Chance Clock Algorithm
// ---------------------------------------------------------------------------

/// Second-Chance Clock algorithm using circular hand and reference bits.
#[derive(Debug, Default)]
pub struct ClockReplacement {
    pages: Vec<ResidentPage>,
    hand: usize,
}

impl ClockReplacement {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PageReplacementPolicy for ClockReplacement {
    fn name(&self) -> &'static str {
        "Clock"
    }

    fn record_page_in(&mut self, mut page: ResidentPage) {
        page.reference_bit = true;
        self.pages.push(page);
    }

    fn record_access(&mut self, frame: FrameId, _current_tick: Tick, is_write: bool) {
        if let Some(page) = self.pages.iter_mut().find(|p| p.frame == frame) {
            page.reference_bit = true;
            if is_write {
                page.dirty = true;
            }
        }
    }

    fn select_victim(&mut self) -> Option<ResidentPage> {
        if self.pages.is_empty() {
            return None;
        }

        let n = self.pages.len();
        for _ in 0..(2 * n) {
            if self.hand >= self.pages.len() {
                self.hand = 0;
            }

            if self.pages[self.hand].reference_bit {
                // Give second chance, clear reference bit
                self.pages[self.hand].reference_bit = false;
                self.hand = (self.hand + 1) % self.pages.len();
            } else {
                // Found victim
                let victim = self.pages.remove(self.hand);
                if self.hand >= self.pages.len() && !self.pages.is_empty() {
                    self.hand = 0;
                }
                return Some(victim);
            }
        }

        // Fallback: remove first
        Some(self.pages.remove(0))
    }

    fn remove_frame(&mut self, frame: FrameId) -> Option<ResidentPage> {
        let pos = self.pages.iter().position(|p| p.frame == frame)?;
        let victim = self.pages.remove(pos);
        if self.hand >= self.pages.len() && !self.pages.is_empty() {
            self.hand = 0;
        }
        Some(victim)
    }
}
