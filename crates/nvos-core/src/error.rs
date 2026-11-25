//! Two-tier error hierarchy for the nvos simulator.
//!
//! **`GuestTrap`** — recoverable process-level faults (segfault, bad fd, OOM).
//!   These terminate the offending process but leave the engine running.
//!
//! **`KernelEngineError`** — engine-level failures (subsystem returned `Err`,
//!   arena corruption, invariant violation). These halt the simulation.
//!
//! All pluggable subsystem calls are wrapped in `nvos-kernel::panic_boundary`
//! which converts `panic!` unwinds into `KernelEngineError::Panic`.

use std::fmt;

// ---------------------------------------------------------------------------
// Top-level simulation error
// ---------------------------------------------------------------------------

/// Top-level simulation error; wraps either a guest trap or an engine error.
#[derive(Debug)]
pub enum SimulationError {
    /// A process-level fault occurred (see [`GuestTrap`]).
    Guest(GuestTrap),
    /// The kernel engine encountered an unrecoverable error (see [`KernelEngineError`]).
    Engine(KernelEngineError),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Guest(t) => write!(f, "GuestTrap: {t}"),
            Self::Engine(e) => write!(f, "KernelEngineError: {e}"),
        }
    }
}

impl std::error::Error for SimulationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Guest(t) => Some(t),
            Self::Engine(e) => Some(e),
        }
    }
}

impl From<GuestTrap> for SimulationError {
    fn from(t: GuestTrap) -> Self { Self::Guest(t) }
}

impl From<KernelEngineError> for SimulationError {
    fn from(e: KernelEngineError) -> Self { Self::Engine(e) }
}

// ---------------------------------------------------------------------------
// Guest-side traps (process-level, recoverable)
// ---------------------------------------------------------------------------

/// A recoverable process-level fault. The offending process is terminated;
/// the simulation engine continues running other processes.
#[derive(Debug)]
pub struct GuestTrap {
    /// The process that triggered the trap.
    pub pid: u32,
    /// The specific fault condition.
    pub reason: TrapReason,
}

impl fmt::Display for GuestTrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pid={} reason={}", self.pid, self.reason)
    }
}

impl std::error::Error for GuestTrap {}

/// Specific cause of a [`GuestTrap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapReason {
    /// Process accessed an unmapped or write-protected virtual address.
    SegmentationFault { virtual_addr: u32 },
    /// Process referenced an invalid or already-closed file descriptor.
    BadFileDescriptor { fd: u32 },
    /// Physical frame allocator could not satisfy the allocation request.
    OutOfMemory { requested_bytes: usize },
    /// Process executed an `InstructionToken` variant that is not valid in
    /// its current state (e.g., `Free` on an ID it does not own).
    IllegalInstruction { token_index: usize },
    /// A `FaultInject` token was processed (explicit chaos-test trap).
    FaultInjected { error_code: u32 },
}

impl fmt::Display for TrapReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SegmentationFault { virtual_addr } =>
                write!(f, "SegFault @ vaddr=0x{virtual_addr:08x}"),
            Self::BadFileDescriptor { fd } =>
                write!(f, "BadFd fd={fd}"),
            Self::OutOfMemory { requested_bytes } =>
                write!(f, "OOM requested={requested_bytes}B"),
            Self::IllegalInstruction { token_index } =>
                write!(f, "IllegalInstruction token_index={token_index}"),
            Self::FaultInjected { error_code } =>
                write!(f, "FaultInjected code={error_code}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Engine-side errors (simulation-level, fatal)
// ---------------------------------------------------------------------------

/// An unrecoverable engine-level error. When this occurs, the simulation
/// halts and the error is reported. The host process does **not** crash.
#[derive(Debug)]
pub struct KernelEngineError {
    /// The subsystem that produced the error.
    pub subsystem: &'static str,
    /// Detailed error description.
    pub kind: EngineErrorKind,
}

impl fmt::Display for KernelEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.subsystem, self.kind)
    }
}

impl std::error::Error for KernelEngineError {}

/// Specific kind of [`KernelEngineError`].
#[derive(Debug)]
pub enum EngineErrorKind {
    /// A subsystem call panicked; the panic was caught by `catch_unwind`.
    Panic { message: String },
    /// A subsystem's `apply_mutations` returned `Err`.
    SubsystemFault { detail: String },
    /// The process arena detected an invariant violation.
    ArenaCorruption { detail: String },
    /// Tick overflow (should be unreachable with `u64`).
    TickOverflow,
}

impl fmt::Display for EngineErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panic { message } => write!(f, "panic caught: {message}"),
            Self::SubsystemFault { detail } => write!(f, "subsystem fault: {detail}"),
            Self::ArenaCorruption { detail } => write!(f, "arena corruption: {detail}"),
            Self::TickOverflow => write!(f, "tick counter overflow"),
        }
    }
}
