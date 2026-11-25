//! Memory bridge integrating `nvos-mem` into the discrete-event `KernelEngine`.
//!
//! Handles translation lookups during Phase 2 Step 3 and page fault resolution.

use nvos_core::error::{EngineErrorKind, KernelEngineError};
use nvos_core::process::{HeapBlockId, Pid};
use nvos_mem::{
    FrameId, HeapBlock, MemoryFault, MemoryManager, PhysicalAddress, VirtualAddress,
};
use crate::panic_boundary::safe_subsystem_dispatch;

/// Bridge integrating `nvos-mem` into the kernel tick loop.
pub struct MemoryBridge {
    manager: Box<dyn MemoryManager>,
}

impl MemoryBridge {
    /// Create a new memory bridge around a boxed `MemoryManager`.
    pub fn new(manager: Box<dyn MemoryManager>) -> Self {
        Self { manager }
    }

    /// Subsystem name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.manager.name()
    }

    /// Translate virtual address to physical address for the currently executing process.
    pub fn translate(
        &mut self,
        pid: Pid,
        vaddr: VirtualAddress,
    ) -> Result<PhysicalAddress, MemoryFault> {
        self.manager.translate(pid, vaddr)
    }

    /// Resolve page fault trapped by address translation.
    pub fn handle_page_fault(&mut self, fault: MemoryFault) -> Result<FrameId, KernelEngineError> {
        let name = self.manager.name();
        safe_subsystem_dispatch(name, || {
            self.manager.handle_page_fault(fault).map_err(|e| KernelEngineError {
                subsystem: "nvos-mem",
                kind: EngineErrorKind::SubsystemFault { detail: e.to_string() },
            })
        })
    }

    /// Allocate contiguous physical heap block.
    pub fn allocate_heap(&mut self, pid: Pid, size_bytes: usize) -> Result<HeapBlock, KernelEngineError> {
        let name = self.manager.name();
        safe_subsystem_dispatch(name, || {
            self.manager.allocate_heap(pid, size_bytes).map_err(|e| KernelEngineError {
                subsystem: "nvos-mem",
                kind: EngineErrorKind::SubsystemFault { detail: e.to_string() },
            })
        })
    }

    /// Free contiguous physical heap block.
    pub fn free_heap(&mut self, pid: Pid, block_id: HeapBlockId) -> Result<(), KernelEngineError> {
        let name = self.manager.name();
        safe_subsystem_dispatch(name, || {
            self.manager.free_heap(pid, block_id).map_err(|e| KernelEngineError {
                subsystem: "nvos-mem",
                kind: EngineErrorKind::SubsystemFault { detail: e.to_string() },
            })
        })
    }
}
