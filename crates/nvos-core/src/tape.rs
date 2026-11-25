//! Synthetic instruction tape — the abstract workload each process executes.
//!
//! Processes do not execute real machine code. Instead, they consume a
//! sequence of `InstructionToken` values representing CPU bursts, memory
//! operations, I/O requests, and control flow.

use crate::process::FileDescriptor;

/// A single step in a process's synthetic instruction tape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionToken {
    /// Consume N CPU ticks without blocking.
    ///
    /// Multi-tick execution is tracked via `ProcessControlBlock::current_token_progress`.
    /// When the kernel preempts a process mid-burst, `current_token_progress`
    /// preserves the partial count so execution resumes correctly.
    Cpu(u32),

    /// Allocate `bytes` on the simulated physical heap.
    Alloc { bytes: usize },

    /// Free a previously allocated heap block by its opaque index.
    Free { block_id: usize },

    /// Read from a virtual address, triggering a Translation Lookaside Buffer
    /// (TLB) lookup. A TLB miss triggers a page-table walk; a page-table miss
    /// triggers a page fault and transitions the process to `Blocked(PageFault)`.
    MemoryRead { virtual_addr: u32 },

    /// Write to a virtual address (same TLB/page-fault mechanics as `MemoryRead`).
    MemoryWrite { virtual_addr: u32 },

    /// Request a file read.
    ///
    /// The `nvos-fs` crate resolves block IDs synchronously (zero ticks).
    /// Physical data transfer is then dispatched through the `nvos-io` disk
    /// controller, which applies a tick-accurate Direct Memory Access (DMA)
    /// latency and fires a completion interrupt.
    FileRead { fd: FileDescriptor, bytes: usize },

    /// Request a file write (same two-phase mechanics as `FileRead`).
    FileWrite { fd: FileDescriptor, bytes: usize },

    /// Voluntarily suspend this process for N ticks.
    /// Transitions the process to `Sleeping(current_tick + N)`.
    Sleep(u32),

    /// Terminate the process with the given exit status code.
    Exit(i32),

    /// Inject a simulated fault for chaos/integration testing.
    ///
    /// Used by `fault_injection.toml` scenarios to exercise `GuestTrap`
    /// handling and the `catch_unwind` panic boundary in `nvos-kernel`.
    FaultInject {
        /// The subsystem or condition to fault.
        target: FaultTarget,
        /// Subsystem-specific error code passed to the fault handler.
        error_code: u32,
    },
}

/// Fault injection targets for chaos testing via `InstructionToken::FaultInject`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultTarget {
    /// Force a simulated segmentation fault on the executing process.
    /// Results in `Terminated(ExitReason::SegmentationFault)`.
    SegFault,
    /// Inject a disk controller error on the next I/O request from this process.
    DiskError,
    /// Trigger an out-of-memory condition in the physical frame allocator,
    /// forcing maximum eviction pressure.
    MemoryPressure,
}
