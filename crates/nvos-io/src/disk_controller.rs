//! Virtual disk controller with seek, rotational delay, and DMA block transfer latency.

use nvos_core::process::Pid;
use nvos_core::tick::Tick;

/// Pending disk DMA transfer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskTransferRequest {
    pub pid: Pid,
    pub target_sectors: Vec<u32>,
    pub completion_tick: Tick,
}

/// Simulated Direct Memory Access (DMA) storage controller.
#[derive(Debug)]
pub struct VirtualDiskController {
    current_head_cylinder: u32,
    sectors_per_cylinder: u32,
    seek_penalty_factor: u64,
    transfer_ticks_per_sector: u64,
    fixed_rotational_delay: u64,
    pending_queue: Vec<DiskTransferRequest>,
}

impl VirtualDiskController {
    /// Create a new virtual disk controller.
    #[must_use]
    pub fn new(sectors_per_cylinder: u32) -> Self {
        Self {
            current_head_cylinder: 0,
            sectors_per_cylinder: sectors_per_cylinder.max(1),
            seek_penalty_factor: 1,
            transfer_ticks_per_sector: 2,
            fixed_rotational_delay: 3,
            pending_queue: Vec::new(),
        }
    }

    /// Schedule a timed sector read/write DMA transfer.
    /// Calculates seek time, rotational delay, and transfer latency deterministically.
    pub fn schedule_transfer(
        &mut self,
        pid: Pid,
        sectors: Vec<u32>,
        current_tick: Tick,
    ) -> Tick {
        if sectors.is_empty() {
            return current_tick;
        }

        let target_cylinder = sectors[0] / self.sectors_per_cylinder;
        let seek_ticks = u64::from(target_cylinder.abs_diff(self.current_head_cylinder))
            .saturating_mul(self.seek_penalty_factor);
        let transfer_ticks = (sectors.len() as u64).saturating_mul(self.transfer_ticks_per_sector);
        let total_latency = seek_ticks
            .saturating_add(self.fixed_rotational_delay)
            .saturating_add(transfer_ticks);

        let completion_tick = current_tick + nvos_core::tick::DurationTicks(total_latency);
        self.current_head_cylinder = target_cylinder;

        self.pending_queue.push(DiskTransferRequest {
            pid,
            target_sectors: sectors,
            completion_tick,
        });

        completion_tick
    }

    /// Poll for all completed transfers at `current_tick`.
    /// Returns the list of PIDs unblocked by disk I/O interrupts.
    pub fn poll_completions(&mut self, current_tick: Tick) -> Vec<Pid> {
        let mut completed = Vec::new();
        let mut remaining = Vec::new();

        for req in self.pending_queue.drain(..) {
            if req.completion_tick <= current_tick {
                completed.push(req.pid);
            } else {
                remaining.push(req);
            }
        }

        self.pending_queue = remaining;
        completed
    }

    /// Count of active pending DMA transfers.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }
}

impl Default for VirtualDiskController {
    fn default() -> Self {
        Self::new(32) // 32 sectors per cylinder
    }
}
