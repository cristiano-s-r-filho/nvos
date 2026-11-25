//! Physical frame allocation backed by in-memory storage.
//!
//! Models physical memory frames (each typically 4 KB) using a bitmap to track
//! free and allocated frames.

use crate::FrameId;

/// Default page/frame size in bytes (4 KiB).
pub const PAGE_SIZE: usize = 4096;

/// Default number of physical memory frames (e.g. 256 frames = 1 MiB physical RAM).
pub const DEFAULT_PHYSICAL_FRAMES: usize = 256;

/// Physical memory representation and bitmap frame allocator.
#[derive(Debug)]
pub struct FrameAllocator {
    num_frames: usize,
    frame_size: usize,
    bitmap: Vec<bool>,
    memory_backing: Vec<u8>,
}

impl FrameAllocator {
    /// Create a frame allocator with `num_frames` frames of `frame_size` bytes each.
    #[must_use]
    pub fn new(num_frames: usize, frame_size: usize) -> Self {
        let total_bytes = num_frames.saturating_mul(frame_size);
        Self {
            num_frames,
            frame_size,
            bitmap: vec![false; num_frames], // false = free, true = allocated
            memory_backing: vec![0; total_bytes],
        }
    }

    /// Number of physical frames in the memory pool.
    #[must_use]
    pub fn total_frames(&self) -> usize {
        self.num_frames
    }

    /// Size of each page frame in bytes.
    #[must_use]
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Total memory size in bytes.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.num_frames.saturating_mul(self.frame_size)
    }

    /// Count of currently allocated frames.
    #[must_use]
    pub fn allocated_count(&self) -> usize {
        self.bitmap.iter().filter(|&&used| used).count()
    }

    /// Count of currently free frames.
    #[must_use]
    pub fn free_count(&self) -> usize {
        self.num_frames.saturating_sub(self.allocated_count())
    }

    /// Allocate a free physical frame. Returns `None` if all frames are occupied.
    pub fn allocate_frame(&mut self) -> Option<FrameId> {
        let free_idx = self.bitmap.iter().position(|&used| !used)?;
        self.bitmap[free_idx] = true;
        Some(FrameId(u32::try_from(free_idx).unwrap_or(u32::MAX)))
    }

    /// Free a previously allocated frame. Returns `false` if the frame was already free or out of bounds.
    pub fn free_frame(&mut self, frame: FrameId) -> bool {
        let idx = frame.0 as usize;
        if idx < self.num_frames && self.bitmap[idx] {
            self.bitmap[idx] = false;
            true
        } else {
            false
        }
    }

    /// Check if a frame is currently allocated.
    #[must_use]
    pub fn is_allocated(&self, frame: FrameId) -> bool {
        let idx = frame.0 as usize;
        idx < self.num_frames && self.bitmap[idx]
    }

    /// Read bytes from a physical address. Returns `None` if out of bounds.
    #[must_use]
    pub fn read_bytes(&self, physical_addr: usize, len: usize) -> Option<&[u8]> {
        let end = physical_addr.checked_add(len)?;
        if end <= self.memory_backing.len() {
            Some(&self.memory_backing[physical_addr..end])
        } else {
            None
        }
    }

    /// Write bytes to a physical address. Returns `false` if out of bounds.
    pub fn write_bytes(&mut self, physical_addr: usize, data: &[u8]) -> bool {
        if let Some(end) = physical_addr.checked_add(data.len()) {
            if end <= self.memory_backing.len() {
                self.memory_backing[physical_addr..end].copy_from_slice(data);
                return true;
            }
        }
        false
    }
}

impl Default for FrameAllocator {
    fn default() -> Self {
        Self::new(DEFAULT_PHYSICAL_FRAMES, PAGE_SIZE)
    }
}
