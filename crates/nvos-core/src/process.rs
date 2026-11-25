//! Process Control Block and state machine types.
//!
//! Modeled after the Tanenbaum *Modern Operating Systems* PCB specification.
//! All memory handle types are opaque primitives — no types from `nvos-mem`
//! or `nvos-io` appear here.

use std::collections::BTreeMap;
use crate::tape::InstructionToken;
use crate::tick::Tick;

// ---------------------------------------------------------------------------
// Identity primitives
// ---------------------------------------------------------------------------

/// Process Identifier (PID). Unique per simulation run; never reused after
/// a process reaches `Terminated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(pub u32);

/// Parent Process Identifier (PPID). `None` for the root / init process.
pub type PPid = Option<Pid>;

/// Opaque handle to this process's root page table (owned by `nvos-mem`).
/// The kernel passes this ID back to the memory manager; it never interprets
/// the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageTableId(pub u32);

/// Opaque handle to a heap allocation block (owned by `nvos-mem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeapBlockId(pub u32);

/// Open file descriptor (index into the per-process FD table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileDescriptor(pub u32);

/// Inode identifier in the mock filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InodeId(pub u32);

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

/// Scheduling priority level. Lower number = higher priority (0 = highest).
/// Range: 0..=255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PriorityLevel(pub u8);

// ---------------------------------------------------------------------------
// Memory limits
// ---------------------------------------------------------------------------

/// Per-process memory accounting limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLimits {
    /// Maximum virtual address space in bytes.
    pub virtual_address_space: usize,
    /// Maximum resident physical pages at any one time.
    pub max_resident_pages: usize,
    /// Maximum heap allocation in bytes.
    pub max_heap_bytes: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            virtual_address_space: 64 * 1024 * 1024, // 64 MiB
            max_resident_pages: 256,
            max_heap_bytes: 4 * 1024 * 1024, // 4 MiB
        }
    }
}

// ---------------------------------------------------------------------------
// Process state machine
// ---------------------------------------------------------------------------

/// The 7 canonical lifecycle states of a simulated process.
///
/// **State transition authority:** The kernel engine is the sole authority for
/// state transitions. Subsystems emit `KernelRequest` events; the kernel
/// applies them in canonical Phase 2 order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    /// Arrived from scenario; not yet admitted to the scheduler queue.
    New,
    /// In the scheduler ready queue; eligible for CPU dispatch.
    Ready,
    /// Actively executing on the simulated CPU core this tick.
    Running,
    /// Suspended waiting for a hardware or OS event.
    Blocked(BlockReason),
    /// Voluntarily delayed until the given tick (via `Sleep(N)` token).
    Sleeping(Tick),
    /// Execution complete; resources pending reap.
    Terminated(ExitReason),
}

impl ProcessState {
    /// Returns `true` if this process can be selected for CPU dispatch.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns `true` if the process has finished execution.
    #[must_use]
    pub fn is_terminated(&self) -> bool {
        matches!(self, Self::Terminated(_))
    }
}

/// Reason a process entered the `Blocked` state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockReason {
    /// Waiting for a Direct Memory Access (DMA) I/O transfer to complete.
    Io {
        /// Virtual device identifier (assigned by `nvos-io`).
        device_id: u32,
    },
    /// Waiting for a page swap to complete after a Translation Lookaside
    /// Buffer (TLB) miss and page-table miss (page fault).
    PageFault {
        /// The virtual address that triggered the fault.
        virtual_addr: u32,
    },
}

/// Reason a process entered the `Terminated` state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    /// Process called `Exit(code)` or tape was exhausted (implicit code 0).
    Normal(i32),
    /// Process accessed an unmapped or protected virtual address.
    SegmentationFault,
    /// Process executed an undefined instruction token.
    IllegalInstruction,
    /// Process was killed externally (e.g., by a scenario fault injection).
    Killed,
}

// ---------------------------------------------------------------------------
// Process Control Block
// ---------------------------------------------------------------------------

/// Full Tanenbaum-model Process Control Block.
///
/// Owned exclusively by the kernel arena (`nvos-kernel::arena`). Subsystems
/// receive immutable views and communicate state changes via `KernelRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessControlBlock {
    // --- Identification ---
    /// Unique process identifier.
    pub pid: Pid,
    /// Parent process identifier (PPID). `None` for the root process.
    pub ppid: PPid,
    /// Human-readable process name (from scenario file).
    pub name: String,

    // --- Process State & Scheduling ---
    /// Current lifecycle state.
    pub state: ProcessState,
    /// Current effective scheduling priority.
    pub priority: PriorityLevel,
    /// Base priority (before MLFQ demotion or starvation boost).
    pub base_priority: PriorityLevel,
    /// CPU ticks remaining in the current time quantum.
    pub quantum_remaining: u32,

    // --- Instruction Tape Cursor ---
    /// Index of the currently executing token in `execution_tape`.
    pub program_counter: usize,
    /// Ticks consumed on the *current* token for multi-tick `Cpu(N)` tokens.
    /// Resets to `0` when a token is fully retired.
    /// Without this field, a preempted `Cpu(5)` process loses partial progress.
    pub current_token_progress: u32,

    // --- CPU Accounting (Tanenbaum model) ---
    /// Total CPU ticks consumed across all executions.
    pub cpu_time_consumed: u64,
    /// Tick at which this process first arrived / was admitted.
    pub arrival_tick: Tick,
    /// Tick at which the process reached `Terminated`. `None` while running.
    pub completion_tick: Option<Tick>,

    // --- Memory Space (opaque IDs only — no nvos-mem types) ---
    /// Per-process memory resource limits.
    pub memory_limits: MemoryLimits,
    /// Opaque root page table handle (assigned by `nvos-mem`).
    pub page_table_root: Option<PageTableId>,
    /// Opaque heap block handles currently owned by this process.
    pub allocated_heap_blocks: Vec<HeapBlockId>,

    // --- Filesystem ---
    /// Open file descriptor table: `FileDescriptor → InodeId`.
    pub file_descriptors: BTreeMap<FileDescriptor, InodeId>,

    // --- Instruction Tape ---
    /// Sequence of abstract instructions this process will execute.
    pub execution_tape: Vec<InstructionToken>,
}

impl ProcessControlBlock {
    /// Create a new PCB for a process arriving at the given tick.
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn new(
        pid: Pid,
        ppid: PPid,
        name: impl Into<String>,
        priority: PriorityLevel,
        arrival_tick: Tick,
        tape: Vec<InstructionToken>,
    ) -> Self {
        Self {
            pid,
            ppid,
            name: name.into(),
            state: ProcessState::New,
            priority,
            base_priority: priority,
            quantum_remaining: 0,
            program_counter: 0,
            current_token_progress: 0,
            cpu_time_consumed: 0,
            arrival_tick,
            completion_tick: None,
            memory_limits: MemoryLimits::default(),
            page_table_root: None,
            allocated_heap_blocks: Vec::new(),
            file_descriptors: BTreeMap::new(),
            execution_tape: tape,
        }
    }

    /// Returns the current instruction token without advancing the cursor.
    #[must_use]
    pub fn current_token(&self) -> Option<&InstructionToken> {
        self.execution_tape.get(self.program_counter)
    }

    /// Retire the current token (advance program counter, reset progress).
    /// If the tape is exhausted, returns `false` without panicking.
    pub fn retire_current_token(&mut self) -> bool {
        if self.program_counter < self.execution_tape.len() {
            self.program_counter += 1;
            self.current_token_progress = 0;
            true
        } else {
            false
        }
    }

    /// Returns `true` if the execution tape has been fully consumed.
    #[must_use]
    pub fn tape_exhausted(&self) -> bool {
        self.program_counter >= self.execution_tape.len()
    }
}
