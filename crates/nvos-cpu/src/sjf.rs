//! Shortest Job First (SJF) and Shortest Remaining Time First (SRTF) scheduler.

use std::collections::BTreeMap;
use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;
use crate::{Scheduler, SchedulerError, SchedulingAction};

/// Default estimated burst time assigned when none is known.
pub const DEFAULT_ESTIMATED_BURST: u32 = 10;

/// Shortest Job First / Shortest Remaining Time First scheduler.
///
/// Can be configured as non-preemptive (SJF) or preemptive (SRTF).
/// Tracks estimated remaining burst ticks per PID.
#[derive(Debug)]
pub struct SjfScheduler {
    is_preemptive: bool,
    ready_processes: Vec<Pid>,
    remaining_bursts: BTreeMap<Pid, u32>,
    running: Option<Pid>,
}

impl SjfScheduler {
    /// Create a new Shortest Job First scheduler.
    /// If `is_preemptive` is true, runs as SRTF (preempting if a shorter job arrives).
    #[must_use]
    pub fn new(is_preemptive: bool) -> Self {
        Self {
            is_preemptive,
            ready_processes: Vec::new(),
            remaining_bursts: BTreeMap::new(),
            running: None,
        }
    }

    /// Set or update the estimated remaining burst time for a given PID.
    pub fn set_estimated_burst(&mut self, pid: Pid, burst: u32) {
        self.remaining_bursts.insert(pid, burst.max(1));
        self.sort_ready_queue();
    }

    /// Get the estimated remaining burst time for a given PID.
    #[must_use]
    pub fn remaining_burst(&self, pid: Pid) -> u32 {
        self.remaining_bursts.get(&pid).copied().unwrap_or(DEFAULT_ESTIMATED_BURST)
    }

    /// Number of ready processes.
    #[must_use]
    pub fn ready_count(&self) -> usize {
        self.ready_processes.len()
    }

    fn sort_ready_queue(&mut self) {
        let bursts = &self.remaining_bursts;
        self.ready_processes.sort_by(|&a, &b| {
            let burst_a = bursts.get(&a).copied().unwrap_or(DEFAULT_ESTIMATED_BURST);
            let burst_b = bursts.get(&b).copied().unwrap_or(DEFAULT_ESTIMATED_BURST);
            burst_a.cmp(&burst_b).then_with(|| a.cmp(&b))
        });
    }
}

impl Default for SjfScheduler {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Scheduler for SjfScheduler {
    fn name(&self) -> &'static str {
        if self.is_preemptive { "SRTF" } else { "SJF" }
    }

    fn add_process(&mut self, pid: Pid, _priority: PriorityLevel) -> Result<(), SchedulerError> {
        if self.running == Some(pid) || self.ready_processes.contains(&pid) {
            return Err(SchedulerError::DuplicateProcess(pid));
        }

        self.ready_processes.push(pid);
        self.sort_ready_queue();
        Ok(())
    }

    fn schedule_next(&mut self, _current_tick: Tick) -> Result<Option<Pid>, SchedulerError> {
        if self.ready_processes.is_empty() {
            self.running = None;
            return Ok(None);
        }

        let next_pid = self.ready_processes.remove(0);
        self.running = Some(next_pid);
        Ok(Some(next_pid))
    }

    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError> {
        self.running = running_pid;

        if let Some(pid) = running_pid {
            // Decrement remaining burst estimation
            if let Some(burst) = self.remaining_bursts.get_mut(&pid) {
                *burst = burst.saturating_sub(1);
            }

            // In SRTF mode, check if any ready process has a shorter remaining burst
            if self.is_preemptive && !self.ready_processes.is_empty() {
                let current_burst = self.remaining_burst(pid);
                let best_ready = self.ready_processes[0];
                let best_burst = self.remaining_burst(best_ready);

                if best_burst < current_burst {
                    self.ready_processes.push(pid);
                    self.sort_ready_queue();
                    self.running = None;
                    return Ok(SchedulingAction::Preempt);
                }
            }
        }

        Ok(SchedulingAction::Continue)
    }

    fn on_process_blocked(&mut self, pid: Pid, _reason: BlockReason) -> Result<(), SchedulerError> {
        if self.running == Some(pid) {
            self.running = None;
            Ok(())
        } else if let Some(pos) = self.ready_processes.iter().position(|&p| p == pid) {
            self.ready_processes.remove(pos);
            Ok(())
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }

    fn on_process_terminated(&mut self, pid: Pid) -> Result<(), SchedulerError> {
        self.remaining_bursts.remove(&pid);
        if self.running == Some(pid) {
            self.running = None;
            Ok(())
        } else if let Some(pos) = self.ready_processes.iter().position(|&p| p == pid) {
            self.ready_processes.remove(pos);
            Ok(())
        } else {
            Err(SchedulerError::ProcessNotFound(pid))
        }
    }
}
