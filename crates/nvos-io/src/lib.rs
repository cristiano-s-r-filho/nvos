//! `nvos-io` — virtual hardware devices and interrupt bus for `nvos`.
//!
//! Models virtual timers and DMA disk controllers emitting discrete interrupts.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_truncation
)]

use nvos_core::process::Pid;
use nvos_core::tick::Tick;

pub mod disk_controller;
pub mod timer;

pub use disk_controller::{DiskTransferRequest, VirtualDiskController};
pub use timer::VirtualTimer;

/// Device interrupt signal classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceInterrupt {
    /// Programmable interval timer tick expiration.
    TimerTick,
    /// Direct Memory Access (DMA) storage transfer completed.
    DiskTransferComplete { pid: Pid },
}

/// Simulated hardware interrupt bus.
#[derive(Debug, Default)]
pub struct InterruptBus {
    pending_interrupts: Vec<DeviceInterrupt>,
}

impl InterruptBus {
    /// Create a new empty interrupt bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue an interrupt onto the bus.
    pub fn raise_interrupt(&mut self, interrupt: DeviceInterrupt) {
        self.pending_interrupts.push(interrupt);
    }

    /// Drain all pending interrupts for kernel Phase 2 processing.
    pub fn drain_interrupts(&mut self) -> Vec<DeviceInterrupt> {
        std::mem::take(&mut self.pending_interrupts)
    }

    /// Number of pending interrupts waiting on the bus.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_interrupts.len()
    }
}

/// Unified virtual device manager combining PIT, Disk Controller, and Interrupt Bus.
#[derive(Debug, Default)]
pub struct DeviceManager {
    timer: VirtualTimer,
    disk: VirtualDiskController,
    bus: InterruptBus,
}

impl DeviceManager {
    /// Create a device manager with default components.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Access the virtual timer.
    pub fn timer_mut(&mut self) -> &mut VirtualTimer {
        &mut self.timer
    }

    /// Access the virtual disk controller.
    pub fn disk_mut(&mut self) -> &mut VirtualDiskController {
        &mut self.disk
    }

    /// Schedule a timed disk DMA read/write transfer.
    pub fn schedule_disk_transfer(&mut self, pid: Pid, sectors: Vec<u32>, current_tick: Tick) -> Tick {
        self.disk.schedule_transfer(pid, sectors, current_tick)
    }

    /// Poll all virtual hardware devices at `current_tick` and collect fired interrupts.
    pub fn poll_hardware(&mut self, current_tick: Tick) -> Vec<DeviceInterrupt> {
        // Poll timer
        if self.timer.poll(current_tick) {
            self.bus.raise_interrupt(DeviceInterrupt::TimerTick);
        }

        // Poll disk completions
        let completed_pids = self.disk.poll_completions(current_tick);
        for pid in completed_pids {
            self.bus.raise_interrupt(DeviceInterrupt::DiskTransferComplete { pid });
        }

        self.bus.drain_interrupts()
    }
}
