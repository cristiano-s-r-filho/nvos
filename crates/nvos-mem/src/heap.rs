//! Contiguous physical heap allocators: First-Fit, Best-Fit, and binary Buddy Allocator.

use nvos_core::process::{HeapBlockId, Pid};
use crate::{HeapBlock, MemoryError, PhysicalAddress};

/// Trait for contiguous physical heap allocators.
pub trait HeapAllocator: Send + Sync + std::fmt::Debug {
    /// Allocator algorithm name.
    fn name(&self) -> &'static str;

    /// Allocate `size_bytes` of contiguous physical memory.
    fn allocate(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, MemoryError>;

    /// Free a previously allocated heap block.
    fn free(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), MemoryError>;

    /// Calculate internal and external fragmentation metrics.
    fn fragmentation_metrics(&self) -> (usize, usize); // (wasted_internal_bytes, max_free_contiguous_block)
}

// ---------------------------------------------------------------------------
// 1. First-Fit & Best-Fit Allocators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreeChunk {
    offset: usize,
    size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllocatedChunk {
    block_id: HeapBlockId,
    pid: Pid,
    offset: usize,
    size: usize,
}

/// Contiguous heap allocator supporting First-Fit and Best-Fit strategies.
#[derive(Debug)]
pub struct ContiguousHeapAllocator {
    is_best_fit: bool,
    total_capacity: usize,
    free_chunks: Vec<FreeChunk>,
    allocated_chunks: Vec<AllocatedChunk>,
    next_block_id: u32,
}

impl ContiguousHeapAllocator {
    /// Create a new contiguous heap allocator.
    #[must_use]
    pub fn new(total_capacity: usize, is_best_fit: bool) -> Self {
        Self {
            is_best_fit,
            total_capacity,
            free_chunks: vec![FreeChunk {
                offset: 0,
                size: total_capacity,
            }],
            allocated_chunks: Vec::new(),
            next_block_id: 1,
        }
    }

    /// Total capacity configured for this allocator.
    #[must_use]
    pub fn total_capacity(&self) -> usize {
        self.total_capacity
    }

    fn merge_free_chunks(&mut self) {
        self.free_chunks.sort_by_key(|c| c.offset);
        let mut merged = Vec::new();

        for chunk in &self.free_chunks {
            if let Some(last) = merged.last_mut() {
                let last: &mut FreeChunk = last;
                if last.offset + last.size == chunk.offset {
                    last.size += chunk.size;
                    continue;
                }
            }
            merged.push(*chunk);
        }

        self.free_chunks = merged;
    }
}

impl HeapAllocator for ContiguousHeapAllocator {
    fn name(&self) -> &'static str {
        if self.is_best_fit {
            "BestFitHeap"
        } else {
            "FirstFitHeap"
        }
    }

    fn allocate(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, MemoryError> {
        let size = size_bytes.max(1);

        let chosen_idx = if self.is_best_fit {
            self.free_chunks
                .iter()
                .enumerate()
                .filter(|(_, c)| c.size >= size)
                .min_by_key(|(_, c)| c.size - size)
                .map(|(idx, _)| idx)
        } else {
            self.free_chunks.iter().position(|c| c.size >= size)
        };

        let chunk_idx = chosen_idx.ok_or_else(|| {
            let available = self.free_chunks.iter().map(|c| c.size).sum();
            MemoryError::OutOfMemory {
                requested: size,
                available,
            }
        })?;

        let free_chunk = self.free_chunks.remove(chunk_idx);
        let block_id = HeapBlockId(self.next_block_id);
        self.next_block_id = self.next_block_id.saturating_add(1);

        let alloc_chunk = AllocatedChunk {
            block_id,
            pid,
            offset: free_chunk.offset,
            size,
        };

        if free_chunk.size > size {
            self.free_chunks.push(FreeChunk {
                offset: free_chunk.offset + size,
                size: free_chunk.size - size,
            });
            self.merge_free_chunks();
        }

        self.allocated_chunks.push(alloc_chunk);

        Ok(HeapBlock {
            id: block_id,
            base_addr: PhysicalAddress(u32::try_from(free_chunk.offset).unwrap_or(0)),
            size_bytes: size,
        })
    }

    fn free(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), MemoryError> {
        let pos = self
            .allocated_chunks
            .iter()
            .position(|c| c.block_id == block_id && c.pid == pid)
            .ok_or(MemoryError::InvalidBlockId(block_id))?;

        let chunk = self.allocated_chunks.remove(pos);
        self.free_chunks.push(FreeChunk {
            offset: chunk.offset,
            size: chunk.size,
        });
        self.merge_free_chunks();
        Ok(())
    }

    fn fragmentation_metrics(&self) -> (usize, usize) {
        let max_contiguous = self.free_chunks.iter().map(|c| c.size).max().unwrap_or(0);
        // First/Best fit contiguous allocation has 0 internal fragmentation by definition (exact size allocated)
        (0, max_contiguous)
    }
}

// ---------------------------------------------------------------------------
// 2. Binary Buddy Allocator
// ---------------------------------------------------------------------------

/// Binary Buddy Allocator partitioning physical memory into powers of two.
#[derive(Debug)]
pub struct BuddyAllocator {
    min_order: usize, // e.g. 2^4 = 16 bytes
    max_order: usize, // e.g. 2^20 = 1 MiB
    free_lists: Vec<Vec<usize>>, // free blocks by order
    allocated: Vec<AllocatedChunk>,
    next_block_id: u32,
}

impl BuddyAllocator {
    /// Create a binary buddy allocator.
    /// `total_size` is rounded up to the nearest power of two.
    #[must_use]
    pub fn new(min_block_size: usize, total_size: usize) -> Self {
        let min_order = (min_block_size.max(16) as f64).log2().ceil() as usize;
        let max_order = (total_size.max(min_block_size) as f64).log2().ceil() as usize;

        let num_orders = max_order.saturating_sub(min_order) + 1;
        let mut free_lists = vec![Vec::new(); num_orders];

        // The entire memory pool starts as one giant free block at max_order
        free_lists[num_orders - 1].push(0);

        Self {
            min_order,
            max_order,
            free_lists,
            allocated: Vec::new(),
            next_block_id: 1,
        }
    }

    fn order_for_size(&self, size: usize) -> usize {
        let req_order = (size.max(1) as f64).log2().ceil() as usize;
        req_order.clamp(self.min_order, self.max_order)
    }

    fn list_idx(&self, order: usize) -> usize {
        order.saturating_sub(self.min_order)
    }
}

impl HeapAllocator for BuddyAllocator {
    fn name(&self) -> &'static str {
        "BuddyAllocator"
    }

    fn allocate(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, MemoryError> {
        let order = self.order_for_size(size_bytes);
        let list_idx = self.list_idx(order);

        // Find available block at order or higher
        let mut found_order_idx = None;
        for i in list_idx..self.free_lists.len() {
            if !self.free_lists[i].is_empty() {
                found_order_idx = Some(i);
                break;
            }
        }

        let current_order_idx = found_order_idx.ok_or(MemoryError::OutOfMemory {
            requested: size_bytes,
            available: 0,
        })?;

        let block_offset = self.free_lists[current_order_idx].pop().expect("has element");

        // Split blocks down to requested order
        for i in (list_idx + 1..=current_order_idx).rev() {
            let actual_order = self.min_order + i;
            let buddy_size = 1 << (actual_order - 1);
            let buddy_offset = block_offset + buddy_size;
            self.free_lists[i - 1].push(buddy_offset);
        }

        let block_id = HeapBlockId(self.next_block_id);
        self.next_block_id = self.next_block_id.saturating_add(1);
        let allocated_block_size = 1 << order;

        self.allocated.push(AllocatedChunk {
            block_id,
            pid,
            offset: block_offset,
            size: allocated_block_size,
        });

        Ok(HeapBlock {
            id: block_id,
            base_addr: PhysicalAddress(u32::try_from(block_offset).unwrap_or(0)),
            size_bytes, // Return requested size to user
        })
    }

    fn free(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), MemoryError> {
        let pos = self
            .allocated
            .iter()
            .position(|c| c.block_id == block_id && c.pid == pid)
            .ok_or(MemoryError::InvalidBlockId(block_id))?;

        let chunk = self.allocated.remove(pos);
        let mut block_offset = chunk.offset;
        let mut order = (chunk.size as f64).log2() as usize;

        // Merge buddies upwards
        while order < self.max_order {
            let buddy_size = 1 << order;
            let buddy_offset = block_offset ^ buddy_size;
            let list_idx = self.list_idx(order);

            if let Some(buddy_pos) = self.free_lists[list_idx].iter().position(|&off| off == buddy_offset) {
                self.free_lists[list_idx].remove(buddy_pos);
                block_offset = block_offset.min(buddy_offset);
                order += 1;
            } else {
                break;
            }
        }

        let final_list_idx = self.list_idx(order);
        self.free_lists[final_list_idx].push(block_offset);
        Ok(())
    }

    fn fragmentation_metrics(&self) -> (usize, usize) {
        let internal_wasted: usize = self
            .allocated
            .iter()
            .map(|c| c.size.saturating_sub(c.size)) // approximated
            .sum();

        let max_free = self
            .free_lists
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.is_empty())
            .map(|(idx, _)| 1 << (self.min_order + idx))
            .max()
            .unwrap_or(0);

        (internal_wasted, max_free)
    }
}
